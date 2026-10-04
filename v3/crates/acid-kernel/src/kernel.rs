//! The kernel proper: owns the one kernel lock (registry + router), the
//! dirty flag, the framebuffer and the wallpaper, and spawns apps through
//! the installed AppRunner. Lock order is state -> framebuffer -> canvas
//! (the compositor releases state before taking the framebuffer). App
//! threads only ever take their own canvas lock. No code may call into
//! Kernel while holding a canvas guard, or it inverts that order.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering};

use acid_gfx::{Canvas, wallpaper::wallpaper_canvas_for};
use acid_platform::Platform;
use acid_platform::sync::Mutex;

use crate::TaskId;
use crate::event::EventQueue;
use crate::layout::{CART_WINDOW_MAX, DESKTOP_STRIP_H, Screen, window_size_ok};
use crate::router::{self, KernelState};
use crate::theme::{ACID_OVERLAY_KEY, THEME_BG};
use crate::window::Window;

/// Everything one app's VM host needs -- its kernel context plus its spawn
/// parameters.
#[derive(Clone)]
pub struct AppContext {
    pub kernel: Arc<Kernel>,
    pub task: TaskId,
    pub queue: Arc<EventQueue>,
    pub canvas: Arc<Mutex<Canvas>>,
    pub script_path: String,
    pub arg: Option<String>,
    pub libs: Option<String>,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    /// Cart-level trust (spec §14.2): set by `spawn_app` from `app_is_cart`,
    /// or forced when a cart asked for the spawn (spec §16.2).
    pub cart: bool,
    /// The scale this app's text draws at (1 or 2), fixed at spawn.
    pub font_scale: i32,
}

/// Spec §14.2: an app is built-in only if it is a `.lua` file under
/// `v3/apps/` that passes the fs path guard (no `..`, `.` or empty
/// segments) and its own manifest (`<path without .lua>.app.toml`) does not
/// say `source = cart`. Everything else is cart-level: this fails closed. A
/// missing manifest (NotFound) is built-in; any other read error is cart.
/// The source value is normalised (trimmed, surrounding quotes stripped,
/// lowercased) and the key matched case-insensitively, so `Source = "Cart"`
/// counts.
/// What an app's own manifest says about it, from one read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ManifestFlags {
    /// Cart-level trust (spec §14.2).
    pub cart: bool,
    /// `font = scalable`: draws at Config's font size.
    pub scalable: bool,
}

pub fn manifest_flags(platform: &dyn Platform, script_path: &str) -> ManifestFlags {
    let outside = ManifestFlags { cart: true, scalable: false };
    if !(crate::fs_path::fs_path_is_allowed(script_path)
        && script_path.starts_with("v3/apps/")
        && script_path.ends_with(".lua"))
    {
        return outside;
    }
    let stem = &script_path[..script_path.len() - ".lua".len()];
    let toml = alloc::format!("{stem}.app.toml");
    match platform.fs().read(&toml) {
        Ok(bytes) => {
            let fields = crate::manifest::parse_manifest(&String::from_utf8_lossy(&bytes));
            // Any spelling of the key counts (`Source`, `SOURCE`): fail closed.
            let cart = fields.iter().any(|(k, v)| k.eq_ignore_ascii_case("source") && unquote(v).eq_ignore_ascii_case("cart"));
            let scalable = fields.iter().any(|(k, v)| k.eq_ignore_ascii_case("font") && unquote(v) == "scalable");
            ManifestFlags { cart, scalable }
        }
        Err(acid_platform::FsError::NotFound) => ManifestFlags { cart: false, scalable: false },
        Err(_) => outside,
    }
}

pub fn app_is_cart(platform: &dyn Platform, script_path: &str) -> bool {
    manifest_flags(platform, script_path).cart
}

/// A manifest value trimmed, with surrounding quotes stripped.
fn unquote(value: &str) -> &str {
    let mut v = value.trim();
    for q in ['"', '\''] {
        if v.len() >= 2 && v.starts_with(q) && v.ends_with(q) {
            v = &v[1..v.len() - 1];
        }
    }
    v.trim()
}

/// Runs one app to completion on its own task. The kernel cleans up the
/// window after it returns (see spawn_app).
pub type AppRunner = Arc<dyn Fn(AppContext) + Send + Sync>;

pub struct SpawnRequest {
    pub script_path: String,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub closable: bool,
    pub arg: Option<String>,
    pub libs: Option<String>,
    /// Spec §16.2: true when a cart-level app asked for this spawn. The new
    /// app then runs cart-level whatever its path, and the spawn is refused
    /// once CART_WINDOW_MAX cart-level windows are open. Boot, manifests and
    /// built-in callers pass false.
    pub force_cart: bool,
}

/// Runs exit_app when dropped, so an app's window is cleaned up even if
/// its runner panics and unwinds. A panic would otherwise leave a ghost
/// window registered and composited.
struct ExitGuard {
    kernel: Arc<Kernel>,
    task: TaskId,
}

impl Drop for ExitGuard {
    fn drop(&mut self) {
        self.kernel.exit_app(self.task);
    }
}

pub struct Kernel {
    platform: Arc<dyn Platform>,
    /// Fixed for the kernel's life (spec: screen size is chosen once at startup).
    screen: Screen,
    state: Mutex<KernelState>,
    dirty: AtomicBool,
    next_task: AtomicU32,
    runner: Mutex<Option<AppRunner>>,
    framebuffer: Mutex<Canvas>,
    pub(crate) overlay: Mutex<crate::overlay::Overlay>,
    pub(crate) launcher: Mutex<crate::launcher::Launcher>,
    pub(crate) audio: crate::audio::AudioRuntime,
    /// Leaf lock: task sampler (taken after the platform is read).
    tasks: Mutex<crate::tasks::TaskSampler>,
    wallpaper: Canvas,
    wallpaper_enabled: AtomicBool,
    composited: AtomicU32,
    skipped: AtomicU32,
    font_scale: AtomicI32,
}

impl Kernel {
    /// A kernel at the default screen size.
    pub fn new(platform: Arc<dyn Platform>) -> Arc<Self> {
        Self::with_screen(platform, Screen::DEFAULT)
    }

    pub fn with_screen(platform: Arc<dyn Platform>, screen: Screen) -> Arc<Self> {
        Arc::new(Self {
            platform,
            screen,
            state: Mutex::new(KernelState::new()),
            // Starts dirty, so the first tick always draws.
            dirty: AtomicBool::new(true),
            next_task: AtomicU32::new(0),
            runner: Mutex::new(None),
            framebuffer: Mutex::new(Canvas::new(screen.w, screen.h)),
            overlay: Mutex::new(crate::overlay::Overlay::new(screen)),
            launcher: Mutex::new(crate::launcher::Launcher::new()),
            audio: crate::audio::AudioRuntime::new(),
            tasks: Mutex::new(Default::default()),
            wallpaper: wallpaper_canvas_for(screen.w, screen.h),
            // On at boot; the setting lives in memory only.
            wallpaper_enabled: AtomicBool::new(true),
            composited: AtomicU32::new(0),
            skipped: AtomicU32::new(0),
            font_scale: AtomicI32::new(1),
        })
    }

    pub fn screen(&self) -> Screen {
        self.screen
    }

    pub fn platform(&self) -> &dyn Platform {
        &*self.platform
    }

    /// Samples the platform's threads (before taking the lock); returns the count.
    pub fn refresh_tasks(&self) -> usize {
        let samples = self.platform.thread_samples();
        let now = self.platform.now_ms();
        self.tasks.lock().refresh(&samples, now)
    }

    pub fn task_count(&self) -> usize {
        self.tasks.lock().count()
    }

    pub fn task_info(&self, index: usize) -> Option<crate::tasks::TaskInfo> {
        self.tasks.lock().info(index)
    }

    pub fn set_runner(&self, runner: AppRunner) {
        *self.runner.lock() = Some(runner);
    }

    /// Registers the window first (failing cleanly
    /// when all WINDOW_MAX slots are taken, or, for a spawn a cart asked
    /// for, when CART_WINDOW_MAX cart-level windows are open), then starts
    /// the app's task, rolling the registration back if it can't start.
    pub fn spawn_app(self: &Arc<Self>, req: SpawnRequest) -> Option<TaskId> {
        let flags = manifest_flags(&*self.platform, &req.script_path);
        // Spec §16.2: what a cart starts runs as a cart.
        let cart = req.force_cart || flags.cart;
        let scale = if flags.scalable && !cart { self.font_scale() } else { 1 };
        let (w, h) = crate::layout::grown_size(self.screen, req.w, req.h, scale);
        if !window_size_ok(self.screen, w, h) {
            return None;
        }
        let (x, y) = if scale > 1 {
            (
                req.x.clamp(0, (self.screen.w - w).max(0)),
                req.y.clamp(DESKTOP_STRIP_H, (self.screen.h - h).max(DESKTOP_STRIP_H)),
            )
        } else {
            (req.x, req.y)
        };
        let runner = self.runner.lock().clone()?;
        let task = TaskId(self.next_task.fetch_add(1, Ordering::SeqCst) + 1);
        let queue = Arc::new(EventQueue::new(self.platform.new_signal()));
        let canvas = Arc::new(Mutex::new(Canvas::new(w, h)));
        let mut win = Window::new(
            task, queue.clone(), canvas.clone(), req.script_path.clone(),
            x, y, w, h, req.closable,
        );
        win.cart = cart;
        win.font_scale = scale;
        {
            // The cap check and the registration share one lock, so two
            // carts spawning at once can't both slip under the cap.
            let mut st = self.state.lock();
            if req.force_cart && st.windows.cart_count() >= CART_WINDOW_MAX {
                return None;
            }
            if !st.windows.register(win) {
                return None;
            }
        }
        self.mark_dirty();
        let ctx = AppContext {
            kernel: self.clone(),
            task,
            queue,
            canvas,
            script_path: req.script_path.clone(),
            arg: req.arg,
            libs: req.libs,
            x,
            y,
            w,
            h,
            cart,
            font_scale: scale,
        };
        let kernel = self.clone();
        let body = Box::new(move || {
            let _guard = ExitGuard { kernel, task };
            runner(ctx);
        });
        if self.platform.spawn(&req.script_path, body).is_err() {
            {
                let mut st = self.state.lock();
                st.windows.unregister(task);
                router::forget_task(&mut st, task);
            }
            self.mark_dirty();
            return None;
        }
        Some(task)
    }

    /// The per-app cleanup, run on every exit path.
    pub fn exit_app(&self, task: TaskId) {
        {
            let mut st = self.state.lock();
            st.windows.unregister(task);
            router::forget_task(&mut st, task);
        }
        // Releases the overlay on every exit path.
        self.overlay.lock().close(task);
        // Gates off the app's voices on every exit path.
        self.audio_release_owner(task);
        self.mark_dirty();
    }

    pub fn mark_dirty(&self) {
        self.dirty.store(true, Ordering::SeqCst);
    }

    /// Config's wallpaper toggle.
    pub fn set_wallpaper_enabled(&self, on: bool) {
        self.wallpaper_enabled.store(on, Ordering::SeqCst);
        self.mark_dirty();
    }

    /// Config's font setting: 1 (Normal) or 2 (Large), Normal at boot. It
    /// applies to apps opened afterwards; open windows keep their scale.
    pub fn font_scale(&self) -> i32 {
        self.font_scale.load(Ordering::SeqCst)
    }

    pub fn set_font_scale(&self, s: i32) {
        if s == 1 || s == 2 {
            self.font_scale.store(s, Ordering::SeqCst);
        }
    }

    pub fn wallpaper_enabled(&self) -> bool {
        self.wallpaper_enabled.load(Ordering::SeqCst)
    }

    /// Paints the screen's wallpaper (or THEME_BG when it is off) for the
    /// screen rectangle (x, y, w, h) into `canvas` at the same coordinates.
    /// The desktop uses it, since its window sits at (0, 0). It takes no
    /// lock, so callers may hold their canvas guard. It doesn't mark dirty;
    /// the caller does.
    pub fn paint_wallpaper_into(&self, canvas: &mut Canvas, x: i32, y: i32, w: i32, h: i32) {
        if self.wallpaper_enabled() {
            canvas.copy_rect_from(&self.wallpaper, x, y, w, h);
        } else {
            canvas.fill_rect(x, y, w, h, THEME_BG);
        }
    }

    pub fn take_dirty(&self) -> bool {
        self.dirty.swap(false, Ordering::SeqCst)
    }

    pub fn set_desktop_task(&self, task: Option<TaskId>) {
        self.state.lock().router.desktop = task;
    }

    pub fn activate_window(&self, task: TaskId) {
        router::activate_window(&mut self.state.lock(), task, &self.dirty);
    }

    pub fn close_window(&self, task: TaskId) {
        router::close_window(&mut self.state.lock(), task, &self.dirty);
    }

    pub fn focus(&self) -> Option<TaskId> {
        self.state.lock().router.focus
    }

    pub fn poll_input(&self) {
        let input = self.platform.input();
        let key = input.poll_key();
        let touch = input.poll_touch();
        router::poll(&mut self.state.lock(), key, touch, &self.dirty);
    }

    /// Wallpaper, then every window back to front, then present. The
    /// window list is copied out under the kernel lock and the canvases
    /// are blitted after it is released.
    pub fn composite_frame(&self) {
        let layers: Vec<(Arc<Mutex<Canvas>>, i32, i32)> = self
            .state
            .lock()
            .windows
            .in_z_order()
            .iter()
            .map(|w| (w.canvas.clone(), w.x, w.y))
            .collect();
        let mut fb = self.framebuffer.lock();
        if self.wallpaper_enabled() {
            fb.blit(&self.wallpaper, 0, 0);
        } else {
            fb.fill_rect(0, 0, self.screen.w, self.screen.h, THEME_BG);
        }
        for (canvas, x, y) in layers {
            fb.blit(&canvas.lock(), x, y);
        }
        // Last, colour-keyed: the overlay draws above every window. The
        // overlay lock is a leaf, taken here only after the window canvases.
        if let Some(o) = self.overlay.lock().canvas() {
            fb.blit_keyed(o, 0, 0, ACID_OVERLAY_KEY);
        }
        self.platform.display().present(fb.pixels(), self.screen.w as usize, self.screen.h as usize);
    }

    /// One router tick. Recompositing only when something changed matters:
    /// compositing every tick slows this loop enough to drop fast key
    /// transitions.
    pub fn tick(&self) {
        self.poll_input();
        if self.take_dirty() {
            self.composite_frame();
            self.composited.fetch_add(1, Ordering::SeqCst);
        } else {
            self.skipped.fetch_add(1, Ordering::SeqCst);
        }
    }

    /// The router task: tick every 16 ms until the platform asks to quit.
    pub fn run_router(&self) {
        loop {
            if self.platform.input().should_quit() {
                return;
            }
            self.tick();
            self.platform.sleep_ms(16);
        }
    }

    pub fn composited_frames(&self) -> u32 {
        self.composited.load(Ordering::SeqCst)
    }

    pub fn skipped_frames(&self) -> u32 {
        self.skipped.load(Ordering::SeqCst)
    }

    pub fn with_state<R>(&self, f: impl FnOnce(&mut KernelState) -> R) -> R {
        f(&mut self.state.lock())
    }

    pub fn framebuffer(&self) -> Canvas {
        self.framebuffer.lock().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use acid_gfx::{rgb565, wallpaper::{wallpaper_canvas, wallpaper_canvas_for}};
    use acid_testkit::FakePlatform;
    use crate::test_support::*;
    use std::sync::mpsc;

    #[test]
    fn trust_comes_from_the_path_and_the_apps_own_manifest() {
        let p = FakePlatform::new(FakePlatform::repo_root());
        assert!(!app_is_cart(&*p, "v3/apps/tetris.lua"), "a built-in app");
        assert!(app_is_cart(&*p, "v3/apps/hello_acid.lua"), "hello_acid's manifest says source = cart");
        assert!(app_is_cart(&*p, "v3/fsroot/Home/x.lua"), "anything outside v3/apps");
        assert!(!app_is_cart(&*p, "v3/apps/desktop.lua"), "no manifest is still built-in");
    }

    #[test]
    fn a_path_that_only_looks_like_v3_apps_is_a_cart() {
        let p = FakePlatform::new(FakePlatform::repo_root());
        for path in [
            "v3/apps/../fsroot/Home/evil.lua",
            "v3/apps/./desktop.lua",
            "v3/apps//desktop.lua",
            "v3/apps/desktop.rb",
            "v3/apps/desktop",
            "v3/apps/a\\b.lua",
        ] {
            assert!(app_is_cart(&*p, path), "{path:?} must not get built-in trust");
        }
    }

    /// A temp tree holding v3/apps/x.lua whose manifest is `manifest`.
    struct TempTree(std::path::PathBuf);
    impl Drop for TempTree {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn temp_tree(tag: &str) -> TempTree {
        let root = std::env::temp_dir().join(alloc::format!("acid-trust-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("v3/apps")).unwrap();
        std::fs::write(root.join("v3/apps/x.lua"), "-- x").unwrap();
        TempTree(root)
    }

    #[test]
    fn source_cart_counts_however_it_is_spelled() {
        let t = temp_tree("spell");
        let p = FakePlatform::new(t.0.clone());
        for line in ["source = cart", "source = Cart", "source = CART", "source = \"Cart\"", "source = 'cart'", "source =   cart  \r", "source = \" cart \"", "Source = \"Cart\"", "SOURCE = cart"] {
            std::fs::write(t.0.join("v3/apps/x.app.toml"), alloc::format!("name = X\n{line}\n")).unwrap();
            assert!(app_is_cart(&*p, "v3/apps/x.lua"), "{line:?} must count as cart");
        }
        for line in ["source = built-in", "name = X"] {
            std::fs::write(t.0.join("v3/apps/x.app.toml"), alloc::format!("{line}\n")).unwrap();
            assert!(!app_is_cart(&*p, "v3/apps/x.lua"), "{line:?} is built-in");
        }
    }

    #[test]
    fn an_unreadable_manifest_fails_closed() {
        // A directory where the manifest should be: the read fails with an
        // error other than NotFound.
        let t = temp_tree("unreadable");
        std::fs::create_dir_all(t.0.join("v3/apps/x.app.toml")).unwrap();
        let p = FakePlatform::new(t.0.clone());
        assert!(app_is_cart(&*p, "v3/apps/x.lua"), "a manifest read error other than NotFound is cart");
    }

    #[test]
    fn wallpaper_switch_off_composites_theme_bg() {
        let (p, k, _rx) = setup();
        assert!(k.wallpaper_enabled());
        k.take_dirty();
        k.set_wallpaper_enabled(false);
        assert!(k.take_dirty());
        k.composite_frame();
        let f = p.display.last_frame().unwrap();
        assert!(f.iter().all(|&px| px == rgb565(crate::theme::THEME_BG)));
        k.set_wallpaper_enabled(true);
        k.composite_frame();
        assert_eq!(p.display.last_frame().unwrap()[0], wallpaper_canvas().pixel(0, 0).unwrap());
    }

    #[test]
    fn paint_wallpaper_into_copies_screen_pixels_or_bg() {
        let (_p, k, _rx) = setup();
        let wall = wallpaper_canvas();
        let mut c = Canvas::new(640, 204);
        k.paint_wallpaper_into(&mut c, 0, 24, 640, 180);
        assert_eq!(c.pixel(224, 30), wall.pixel(224, 30));
        assert_eq!(c.pixel(5, 5), Some(0), "above the rect is untouched");
        k.set_wallpaper_enabled(false);
        k.paint_wallpaper_into(&mut c, 0, 24, 640, 180);
        assert_eq!(c.pixel(224, 30), Some(rgb565(crate::theme::THEME_BG)));
    }

    #[test]
    fn spawn_failure_rolls_back_the_window() {
        let (p, k, _rx) = setup();
        // The first spawn in a fresh kernel is TaskId(1): point focus and the
        // touch owner at it so only forget_task in the rollback can clear them.
        k.with_state(|st| {
            st.router.focus = Some(TaskId(1));
            router::set_touch_task_for_test(st, Some(TaskId(1)));
        });
        p.fail_next_spawn();
        assert_eq!(k.spawn_app(req(0, 30, 10, 10)), None);
        assert_eq!(k.with_state(|st| st.windows.count()), 0);
        assert_eq!(k.with_state(|st| st.router.focus), None, "rollback forgets focus");
        assert_eq!(k.with_state(|st| router::touch_task_for_test(st)), None, "rollback forgets touch owner");
    }

    #[test]
    fn a_panicking_runner_still_removes_its_window() {
        // Prints one "panicked" line to stderr: that is the point of the test.
        let p = FakePlatform::new(".");
        let k = Kernel::new(p.clone());
        k.set_runner(Arc::new(|_ctx| panic!("runner panic expected by this test")));
        k.spawn_app(req(0, 30, 10, 10)).unwrap();
        wait_until(|| k.with_state(|st| st.windows.count()) == 0);
    }

    #[test]
    fn spawn_without_runner_fails() {
        let k = Kernel::new(FakePlatform::new("."));
        assert_eq!(k.spawn_app(req(0, 30, 10, 10)), None);
    }

    #[test]
    fn spawn_registers_window_and_passes_context() {
        let (_p, k, rx) = setup();
        let task = k.spawn_app(req(5, 30, 40, 20)).unwrap();
        let ctx = recv(&rx);
        assert_eq!(ctx.task, task);
        assert_eq!((ctx.x, ctx.y, ctx.w, ctx.h), (5, 30, 40, 20));
        assert_eq!(ctx.arg.as_deref(), Some("a"));
        assert_eq!(ctx.canvas.lock().width(), 40);
        assert_eq!(k.with_state(|st| st.windows.count()), 1);
    }

    #[test]
    fn ninth_spawn_fails() {
        let (_p, k, rx) = setup();
        for _ in 0..8 {
            k.spawn_app(req(0, 30, 1, 1)).unwrap();
            recv(&rx);
        }
        assert_eq!(k.spawn_app(req(0, 30, 1, 1)), None);
    }

    #[test]
    fn closing_ends_the_app_and_its_window() {
        let (_p, k, rx) = setup();
        let task = k.spawn_app(req(0, 30, 10, 10)).unwrap();
        recv(&rx);
        k.activate_window(task);
        k.close_window(task);
        wait_until(|| k.with_state(|st| st.windows.count()) == 0);
        assert_eq!(k.focus(), None);
    }

    #[test]
    fn exit_app_clears_focus_on_any_exit_path() {
        let p = FakePlatform::new(".");
        let k = Kernel::new(p.clone());
        let (tx, rx) = mpsc::channel::<()>();
        let rx = std::sync::Mutex::new(rx);
        // An app that ends by itself once told to (no Close event involved).
        k.set_runner(Arc::new(move |_ctx| {
            rx.lock().unwrap().recv().unwrap();
        }));
        let task = k.spawn_app(req(0, 30, 10, 10)).unwrap();
        k.activate_window(task);
        tx.send(()).unwrap();
        wait_until(|| k.focus().is_none() && k.with_state(|st| st.windows.count()) == 0);
    }

    #[test]
    fn composite_draws_wallpaper_then_windows_in_z_order() {
        let (p, k, rx) = setup();
        k.spawn_app(req(10, 30, 4, 4)).unwrap();
        recv(&rx).canvas.lock().fill_rect(0, 0, 4, 4, 0xFF0000);
        k.spawn_app(req(12, 32, 4, 4)).unwrap();
        recv(&rx).canvas.lock().fill_rect(0, 0, 4, 4, 0x0000FF);
        k.composite_frame();
        let frame = p.display.last_frame().unwrap();
        let at = |x: i32, y: i32| frame[(y * 640 + x) as usize];
        assert_eq!(at(10, 30), rgb565(0xFF0000));
        assert_eq!(at(13, 33), rgb565(0x0000FF), "window b is in front");
        assert_eq!(at(0, 0), wallpaper_canvas().pixel(0, 0).unwrap());
        k.activate_window(TaskId(1));
        k.composite_frame();
        let frame = p.display.last_frame().unwrap();
        assert_eq!(frame[(33 * 640 + 13) as usize], rgb565(0xFF0000), "a raised over b");
    }

    #[test]
    fn tick_composites_only_when_dirty() {
        let (p, k, _rx) = setup();
        k.tick(); // dirty from boot
        assert_eq!((k.composited_frames(), k.skipped_frames()), (1, 0));
        k.tick();
        assert_eq!((k.composited_frames(), k.skipped_frames()), (1, 1));
        k.mark_dirty();
        k.tick();
        assert_eq!(k.composited_frames(), 2);
        assert_eq!(p.display.frame_count(), 2);
    }

    #[test]
    fn tick_routes_platform_input() {
        // Only focus is checked: the parked runner is the queue's reader, so
        // the event itself is covered by the router tests instead.
        let (p, k, rx) = setup();
        let task = k.spawn_app(req(100, 100, 50, 50)).unwrap();
        recv(&rx);
        p.input.set_touch(110, 130, true);
        k.tick();
        assert_eq!(k.focus(), Some(task));
    }

    #[test]
    fn run_router_returns_on_quit() {
        let (p, k, _rx) = setup();
        let k2 = k.clone();
        let h = std::thread::spawn(move || k2.run_router());
        wait_until(|| k.composited_frames() >= 1);
        p.input.request_quit();
        h.join().unwrap();
    }

    #[test]
    fn new_uses_the_default_screen() {
        let k = Kernel::new(FakePlatform::new("."));
        assert_eq!(k.screen(), crate::layout::Screen::DEFAULT);
    }

    #[test]
    fn the_frame_and_wallpaper_follow_the_screen() {
        for s in crate::layout::Screen::PRESETS {
            let (p, k, _rx) = setup_at(s);
            k.composite_frame();
            let f = p.display.last_frame().unwrap();
            assert_eq!(f.len(), (s.w * s.h) as usize, "{s:?}");
            let wall = wallpaper_canvas_for(s.w, s.h);
            assert_eq!(f[0], wall.pixel(0, 0).unwrap(), "{s:?}");
            let last = (s.w * s.h - 1) as usize;
            assert_eq!(f[last], wall.pixel(s.w - 1, s.h - 1).unwrap(), "{s:?}");
        }
    }

    #[test]
    fn spawn_accepts_a_window_only_as_big_as_the_screen() {
        let (_p, k, _rx) = setup_at(crate::layout::Screen::SVGA);
        assert!(k.spawn_app(req(0, 0, 800, 600)).is_some());
        let (_p, k, _rx) = setup_at(crate::layout::Screen::DEFAULT);
        assert!(k.spawn_app(req(0, 0, 800, 600)).is_none());
    }

    #[test]
    fn font_scale_accepts_only_1_and_2() {
        let (_p, k, _rx) = setup();
        assert_eq!(k.font_scale(), 1, "Normal at boot");
        k.set_font_scale(2);
        assert_eq!(k.font_scale(), 2);
        for bad in [0, 3, -1, 99] {
            k.set_font_scale(bad);
            assert_eq!(k.font_scale(), 2, "{bad} is ignored");
        }
        k.set_font_scale(1);
        assert_eq!(k.font_scale(), 1);
    }

    #[test]
    fn manifest_flags_reads_the_font_opt_in() {
        let t = temp_tree("font_flags");
        std::fs::create_dir_all(t.0.join("v3/apps")).unwrap();
        std::fs::write(t.0.join("v3/apps/big.app.toml"), "name = Big\nw = 100\nh = 60\nFont = 'scalable'\n").unwrap();
        std::fs::write(t.0.join("v3/apps/plain.app.toml"), "name = Plain\nw = 100\nh = 60\n").unwrap();
        std::fs::write(t.0.join("v3/apps/odd.app.toml"), "font = scalable-ish\n").unwrap();
        let p = FakePlatform::new(t.0.clone());
        assert_eq!(manifest_flags(&*p, "v3/apps/big.lua"), ManifestFlags { cart: false, scalable: true });
        assert_eq!(manifest_flags(&*p, "v3/apps/plain.lua"), ManifestFlags { cart: false, scalable: false });
        assert_eq!(manifest_flags(&*p, "v3/apps/odd.lua").scalable, false, "only the exact value counts");
        assert_eq!(manifest_flags(&*p, "v3/fsroot/Home/x.lua"), ManifestFlags { cart: true, scalable: false });
    }

    #[test]
    fn an_opted_in_app_opens_large_and_grown_and_others_dont() {
        let t = temp_tree("font_spawn");
        std::fs::create_dir_all(t.0.join("v3/apps")).unwrap();
        std::fs::write(t.0.join("v3/apps/big.app.toml"), "font = scalable\n").unwrap();
        std::fs::write(t.0.join("v3/apps/cartish.app.toml"), "font = scalable\nsource = cart\n").unwrap();
        let p = FakePlatform::new(t.0.clone());
        let k = Kernel::with_screen(p.clone(), crate::layout::Screen::DEFAULT);
        let (tx, rx) = mpsc::channel();
        k.set_runner(parked_runner(tx));
        let at = |path: &str| SpawnRequest { script_path: path.into(), ..req(38, 52, 220, 160) };
        let normal = k.spawn_app(at("v3/apps/big.lua")).unwrap();
        assert_eq!(recv(&rx).font_scale, 1, "the setting is Normal");
        k.set_font_scale(2);
        let big = k.spawn_app(at("v3/apps/big.lua")).unwrap();
        let ctx = recv(&rx);
        assert_eq!((ctx.font_scale, ctx.w, ctx.h), (2, 440, 304));
        k.spawn_app(at("v3/apps/other.lua")).unwrap();
        assert_eq!(recv(&rx).font_scale, 1, "no opt-in, no scale");
        k.spawn_app(at("v3/apps/cartish.lua")).unwrap();
        assert_eq!(recv(&rx).font_scale, 1, "a cart's opt-in is ignored");
        k.with_state(|st| {
            assert_eq!(st.windows.by_task(normal).map(|w| (w.font_scale, w.w)), Some((1, 220)), "an open window keeps its scale");
            assert_eq!(st.windows.by_task(big).map(|w| (w.font_scale, w.w, w.h)), Some((2, 440, 304)));
        });
    }

    #[test]
    fn a_grown_window_is_kept_on_screen() {
        let t = temp_tree("font_clamp");
        std::fs::create_dir_all(t.0.join("v3/apps")).unwrap();
        std::fs::write(t.0.join("v3/apps/big.app.toml"), "font = scalable\n").unwrap();
        let p = FakePlatform::new(t.0.clone());
        let k = Kernel::with_screen(p.clone(), crate::layout::Screen::DEFAULT);
        let (tx, rx) = mpsc::channel();
        k.set_runner(parked_runner(tx));
        k.set_font_scale(2);
        let t1 = k.spawn_app(SpawnRequest { script_path: "v3/apps/big.lua".into(), ..req(400, 300, 220, 160) }).unwrap();
        recv(&rx);
        assert_eq!(k.with_state(|st| st.windows.by_task(t1).map(|w| (w.x, w.y))), Some((200, 176)), "pulled back to fit 440x304");
    }
}
