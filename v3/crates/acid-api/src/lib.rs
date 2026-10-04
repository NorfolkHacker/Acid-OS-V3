//! The acid_* API, defined once and independent of any VM. acid-lua (and
//! later acid-wasm) are thin adapters over this trait, so the two runtimes
//! can't drift apart.
#![cfg_attr(not(test), no_std)]

extern crate alloc;

mod chrome;

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicI32, Ordering};

use acid_kernel::AppContext;
use acid_kernel::event::Event;
use acid_kernel::launcher::LaunchableApp;
use acid_kernel::layout::WINDOW_MAX;
pub use acid_gfx::three_d::{MESH_FACES_MAX, MESH_POINTS_MAX, NO_INDEX, builtin_counts};
pub use acid_kernel::WindowInfo;
pub use acid_kernel::tasks::TaskInfo;
pub use acid_platform::{LocalTime, NetworkInfo};
use acid_platform::FsError;
use acid_platform::sync::Mutex;
use acid_gfx::three_d::{self, Mesh, MeshError};

/// Meshes alive per app. The per-mesh limits (`MESH_POINTS_MAX`,
/// `MESH_FACES_MAX`) live in `acid_gfx::three_d`; the per-app ones live here.
pub const MESH_MAX: usize = 16;
/// Points across all of one app's live meshes.
pub const MESH_TOTAL_POINTS_MAX: usize = 4096;

#[cfg(test)]
std::thread_local! {
    /// How many times a mesh was actually built (test seam).
    static MESH_BUILDS: core::cell::Cell<usize> = const { core::cell::Cell::new(0) };
}

/// One app's meshes. Ids start at 1 and are never reused.
struct MeshStore {
    map: BTreeMap<i32, Mesh>,
    next_id: i32,
    total_points: usize,
}

impl MeshStore {
    fn new() -> Self {
        Self { map: BTreeMap::new(), next_id: 1, total_points: 0 }
    }

    /// "too big" if a new mesh of `points` points would not fit. Checked
    /// before anything is built, so a full store costs the host nothing.
    fn room_for(&self, points: usize) -> Result<(), String> {
        if self.map.len() >= MESH_MAX || self.total_points + points > MESH_TOTAL_POINTS_MAX {
            return Err(String::from("too big"));
        }
        Ok(())
    }

    /// Stores a mesh, or "too big" at the mesh-count or total-points limit,
    /// or once the ids run out (they are never reused).
    fn add(&mut self, m: Mesh) -> Result<i32, String> {
        if self.map.len() >= MESH_MAX || self.total_points + m.points().len() > MESH_TOTAL_POINTS_MAX {
            return Err(String::from("too big"));
        }
        let id = self.next_id;
        self.next_id = id.checked_add(1).ok_or_else(|| String::from("too big"))?;
        self.total_points += m.points().len();
        self.map.insert(id, m);
        Ok(id)
    }
}

/// Lua passes ids as integers; a negative one is simply no such slot.
fn index(i: i64) -> Option<usize> {
    usize::try_from(i).ok()
}

/// "" means "none" (libs, launch arg).
fn non_empty(s: &str) -> Option<String> {
    if s.is_empty() { None } else { Some(String::from(s)) }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolledEvent {
    Close,
    Moved,
    /// The window was resized to `w` x `h`; the app re-lays out.
    Resized { w: i32, h: i32 },
    /// `pressed` is always true: only presses are generated.
    Key { code: i32, pressed: bool },
    Touch { x: i32, y: i32, pressed: bool },
}

pub trait AcidApi: Send + Sync {
    /// Blocks up to `timeout_ms` (negative counts as 0) for the next event.
    fn poll_event(&self, timeout_ms: i64) -> Option<PolledEvent>;
    /// Milliseconds since boot; the games' clock (spec 10.2).
    fn now_ms(&self) -> i64;
    /// A harmless no-op: the compositor doesn't wait on apps.
    fn notify_redraw_done(&self);
    fn fill_rect(&self, x: i32, y: i32, w: i32, h: i32, color: u32);
    fn fill_circle(&self, x: i32, y: i32, r: i32, color: u32);
    fn draw_text(&self, text: &str, x: i32, y: i32, fg: u32, bg: u32);
    fn draw_window_frame(&self, title: &str);
    fn draw_window_border(&self);
    fn clear_user_area(&self);
    fn am_i_focused(&self) -> bool;
    fn launch_arg(&self) -> String;
    /// Start a note on a voice, owned by the caller.
    fn play_note(&self, voice: i32, ona: i32, volume: i32);
    /// Release a voice.
    fn stop_note(&self, voice: i32);
    /// Filter route and ADSR envelope.
    fn configure_voice(&self, voice: i32, filter_route: i32, attack_ms: i32, decay_ms: i32, sustain_percent: i32, release_ms: i32);
    /// The shared filter's cutoff, resonance and mode.
    fn configure_filter(&self, cutoff: i32, resonance: i32, mode: i32);
    /// Arpeggiate up to four notes on a voice.
    fn trigger_arp(&self, voice: i32, notes: [i32; 4], count: i32, rate_ms: i32);
    /// Waveform and pulse duty for a voice.
    fn configure_osc(&self, voice: i32, waveform: i32, duty_percent: i32);
    /// Ring-modulate a voice with another (-1 clears).
    fn set_ring_partner(&self, voice: i32, partner: i32);
    /// Set the master volume, percent.
    fn set_volume(&self, percent: i32);
    /// The master volume, percent.
    fn volume(&self) -> i32;
    /// Voices currently sounding.
    fn active_voice_count(&self) -> i32;
    /// Claim the single kernel overlay.
    /// Always false for a cart (spec §16.2).
    fn overlay_open(&self) -> bool;
    /// Wipe the overlay to the colour key.
    fn overlay_clear(&self);
    /// Fill a rect on the overlay.
    fn overlay_fill_rect(&self, x: i32, y: i32, w: i32, h: i32, color: u32);
    /// Release the overlay if this app owns it.
    fn overlay_close(&self);
    /// Repaint wallpaper into a rect of this window.
    fn repaint_region(&self, x: i32, y: i32, w: i32, h: i32);
    /// Turn the wallpaper on or off.
    fn set_wallpaper_enabled(&self, on: bool);
    /// Whether the wallpaper is on.
    fn wallpaper_enabled(&self) -> bool;
    /// Number of window slots.
    fn window_max(&self) -> i32;
    /// The screen's size in pixels, `(w, h)`; fixed for the whole run.
    fn screen_size(&self) -> (i32, i32);
    /// A 1-pixel line in window coordinates.
    fn draw_line(&self, x1: i32, y1: i32, x2: i32, y2: i32, color: u32);
    /// A filled triangle in window coordinates.
    fn fill_triangle(&self, x1: i32, y1: i32, x2: i32, y2: i32, x3: i32, y3: i32, color: u32);
    /// A built-in shape's mesh id; Err("unknown") for an unknown name, Err("too big") over the limits.
    fn mesh_builtin(&self, name: &str) -> Result<i32, String>;
    /// A new mesh from points and 0-based faces (NO_INDEX = triangle); "bad mesh" / "too big".
    fn mesh_new(&self, points: Vec<(i32, i32, i32)>, faces: Vec<[u16; 4]>) -> Result<i32, String>;
    /// Draws a mesh; an unknown id draws nothing.
    #[allow(clippy::too_many_arguments)]
    fn mesh_draw(&self, id: i32, x: i32, y: i32, size: i32, rx: i32, ry: i32, rz: i32, mode: i32, color: u32);
    /// The pixels mesh_draw would touch (for fuel); 0 for an unknown id.
    fn mesh_draw_cost(&self, id: i32, x: i32, y: i32, size: i32, rx: i32, ry: i32, rz: i32, mode: i32) -> u64;
    /// Frees a mesh; an unknown id does nothing.
    fn mesh_free(&self, id: i32);
    /// This window's character cell in pixels, (w, h): (6, 8) at Normal, (12, 16) at Large.
    fn font_size(&self) -> (i32, i32);
    /// This window's size in pixels, (w, h).
    fn window_size(&self) -> (i32, i32);
    /// Config's font setting (1 Normal, 2 Large).
    fn font_scale(&self) -> i32;
    /// Sets it (only 1 or 2); applies to apps opened afterwards.
    fn set_font_scale(&self, scale: i32);
    /// The window in a slot; None for an empty or invalid slot.
    fn window_info(&self, index: i64) -> Option<WindowInfo>;
    /// Raise a window. A cart may raise only its own window; for any other
    /// index it does nothing (spec §16.2).
    fn activate_window(&self, index: i64);
    /// Close a window; false for the caller's own window.
    /// Always false for a cart (spec §16.2).
    fn close_window(&self, index: i64) -> bool;
    /// Whether the caller is a cart (untrusted, cart-level). The WASM host
    /// uses it to answer −4 where the api answers false (spec §16.2).
    /// Required, so every implementor decides.
    fn is_cart(&self) -> bool;
    /// Restart the OS from its boot screen. Always false for a cart
    /// (spec §16.2); otherwise false only if the platform can't.
    /// Required, so every implementor decides.
    fn restart(&self) -> bool;
    /// Send the caller's window to the back.
    fn send_self_to_back(&self);
    /// Add a launcher entry; empty `libs` means none.
    fn launcher_register(&self, path: &str, name: &str, w: i32, h: i32, multi: bool, libs: &str) -> bool;
    /// Number of launcher entries.
    fn launcher_count(&self) -> i32;
    /// A launcher entry's app path.
    fn launcher_path(&self, index: i64) -> Option<String>;
    /// A launcher entry's display name.
    fn launcher_name(&self, index: i64) -> Option<String>;
    /// Launch a launcher entry. For a cart, an entry whose single-instance
    /// app is already open gives false and raises nothing (spec §16.2).
    fn launcher_spawn(&self, index: i64) -> bool;
    /// Launch the app at `path`; empty `arg` means none. For a
    /// cart, a path outside `v3/apps/` or an already-open single-instance
    /// app gives false and raises nothing (spec §16.2).
    fn spawn_app(&self, path: &str, w: i32, h: i32, arg: &str) -> bool;
    /// Compositor frames drawn so far.
    fn composited_frames(&self) -> u32;
    /// Compositor ticks skipped as clean.
    fn skipped_frames(&self) -> u32;
    /// Wall-clock time.
    fn local_time(&self) -> LocalTime;
    /// Memory in use, KiB.
    fn mem_used_kb(&self) -> i64;
    /// Host, IP and link state.
    fn network_info(&self) -> NetworkInfo;
    /// Snapshot tasks, returns the count.
    fn refresh_tasks(&self) -> i32;
    /// Tasks in the last snapshot.
    fn task_count(&self) -> i32;
    /// A task from the last snapshot; None for no such task.
    fn task_info(&self, index: i64) -> Option<TaskInfo>;
    /// Entry names in `dir`; Err is "bad path", "not found" or the platform's message.
    fn fs_list(&self, dir: &str) -> Result<Vec<String>, String>;
    /// File bytes; same errors as `fs_list`.
    fn fs_read(&self, path: &str) -> Result<Vec<u8>, String>;
    /// File size in bytes; same errors as fs_list.
    fn fs_size(&self, path: &str) -> Result<u64, String>;
    /// Create or replace a file; Err is "bad path", "not found" or the platform's message.
    fn fs_write(&self, path: &str, data: &[u8]) -> Result<(), String>;
    /// Move `from` to `to`; same errors as `fs_write`.
    fn fs_rename(&self, from: &str, to: &str) -> Result<(), String>;
    /// Delete a file; also "is a directory"; same errors as `fs_write`.
    fn fs_delete(&self, path: &str) -> Result<(), String>;
    /// Host cart folders (spec §14.4), read only, built-in apps only: "not allowed" for carts.
    fn cart_roots(&self) -> Result<Vec<String>, String>;
    fn cart_list(&self, dir: &str) -> Result<Vec<String>, String>;
    /// `(is_dir, size)`.
    fn cart_stat(&self, path: &str) -> Result<(bool, u64), String>;
    fn cart_read(&self, path: &str) -> Result<Vec<u8>, String>;
}

fn fs_error(e: FsError) -> String {
    match e {
        FsError::NotFound => String::from("not found"),
        FsError::Other(m) => m,
    }
}

/// The real implementation, bound to one app's window.
pub struct KernelApi {
    ctx: AppContext,
    window_x: AtomicI32,
    window_y: AtomicI32,
    /// Lock order: this store, then the canvas, never the reverse
    /// (`mesh_draw` holds this while `draw` takes the canvas).
    meshes: Mutex<MeshStore>,
}

impl KernelApi {
    /// Spec §16.2: closing a window unregisters it at once, so a cart that
    /// ignores "close" runs on windowless and uncounted by CART_WINDOW_MAX.
    /// Such a cart may not start apps.
    fn windowless_cart(&self) -> bool {
        self.ctx.cart && !self.ctx.kernel.has_window(self.ctx.task)
    }

    pub fn new(ctx: AppContext) -> Self {
        let (x, y) = (ctx.x, ctx.y);
        Self { ctx, window_x: AtomicI32::new(x), window_y: AtomicI32::new(y), meshes: Mutex::new(MeshStore::new()) }
    }

    /// Spec §14.2: cart-level apps change files only under Home.
    fn cart_may_change(&self, path: &str) -> bool {
        !self.ctx.cart || path.starts_with("v3/fsroot/Home/")
    }

    /// Spec §14.2: host cart folders are for built-in apps only.
    fn cart_allowed(&self) -> Result<(), String> {
        if self.ctx.cart { Err(String::from("not allowed")) } else { Ok(()) }
    }

    pub fn context(&self) -> &AppContext {
        &self.ctx
    }

    /// Where this app last heard its window is (updated by Moved).
    pub fn window_pos(&self) -> (i32, i32) {
        (self.window_x.load(Ordering::SeqCst), self.window_y.load(Ordering::SeqCst))
    }

    fn map(&self, ev: Event) -> PolledEvent {
        match ev {
            Event::Close => PolledEvent::Close,
            Event::Moved { x, y } => {
                self.window_x.store(x, Ordering::SeqCst);
                self.window_y.store(y, Ordering::SeqCst);
                PolledEvent::Moved
            }
            Event::Resized { w, h } => PolledEvent::Resized { w, h },
            Event::Key { code } => PolledEvent::Key { code, pressed: true },
            Event::Touch { x, y, pressed } => PolledEvent::Touch { x, y, pressed },
        }
    }

    /// The canvas's current size. Read it before `draw`, which holds the
    /// (non-re-entrant) canvas lock.
    fn live_size(&self) -> (i32, i32) {
        let c = self.ctx.canvas.lock();
        (c.width(), c.height())
    }

    fn draw(&self, f: impl FnOnce(&mut acid_gfx::Canvas)) {
        f(&mut self.ctx.canvas.lock());
        self.ctx.kernel.mark_dirty();
    }
}

impl AcidApi for KernelApi {
    fn poll_event(&self, timeout_ms: i64) -> Option<PolledEvent> {
        let ms = timeout_ms.clamp(0, u32::MAX as i64) as u32;
        let ev = self.ctx.queue.recv_timeout(self.ctx.kernel.platform(), ms)?;
        Some(self.map(ev))
    }

    fn now_ms(&self) -> i64 {
        self.ctx.kernel.platform().now_ms() as i64
    }

    fn notify_redraw_done(&self) {}

    fn fill_rect(&self, x: i32, y: i32, w: i32, h: i32, color: u32) {
        self.draw(|c| c.fill_rect(x, y, w, h, color));
    }

    fn fill_circle(&self, x: i32, y: i32, r: i32, color: u32) {
        self.draw(|c| c.fill_circle(x, y, r, color));
    }

    fn draw_line(&self, x1: i32, y1: i32, x2: i32, y2: i32, color: u32) {
        self.draw(|c| c.draw_line(x1, y1, x2, y2, color));
    }

    fn fill_triangle(&self, x1: i32, y1: i32, x2: i32, y2: i32, x3: i32, y3: i32, color: u32) {
        self.draw(|c| c.fill_triangle(x1, y1, x2, y2, x3, y3, color));
    }

    fn mesh_builtin(&self, name: &str) -> Result<i32, String> {
        // An unknown name is "unknown" before any limit; then the limits are
        // checked against the built-in's known size, before building it.
        let (points, _) = three_d::builtin_counts(name).ok_or_else(|| String::from("unknown"))?;
        self.meshes.lock().room_for(points)?;
        #[cfg(test)]
        MESH_BUILDS.with(|c| c.set(c.get() + 1));
        let m = three_d::builtin(name).ok_or_else(|| String::from("unknown"))?;
        self.meshes.lock().add(m)
    }

    fn mesh_new(&self, points: Vec<(i32, i32, i32)>, faces: Vec<[u16; 4]>) -> Result<i32, String> {
        self.meshes.lock().room_for(points.len())?;
        #[cfg(test)]
        MESH_BUILDS.with(|c| c.set(c.get() + 1));
        let m = Mesh::new(points, faces).map_err(|e| {
            String::from(match e {
                MeshError::Bad => "bad mesh",
                MeshError::TooBig => "too big",
            })
        })?;
        self.meshes.lock().add(m)
    }

    fn mesh_draw(&self, id: i32, x: i32, y: i32, size: i32, rx: i32, ry: i32, rz: i32, mode: i32, color: u32) {
        let store = self.meshes.lock();
        if let Some(m) = store.map.get(&id) {
            self.draw(|c| three_d::draw_mesh(c, m, x, y, size, rx, ry, rz, mode, color));
        }
    }

    fn mesh_draw_cost(&self, id: i32, x: i32, y: i32, size: i32, rx: i32, ry: i32, rz: i32, mode: i32) -> u64 {
        let (sw, sh) = self.screen_size();
        match self.meshes.lock().map.get(&id) {
            Some(m) => three_d::mesh_cost(m, x, y, size, rx, ry, rz, mode, sw, sh),
            None => 0,
        }
    }

    fn mesh_free(&self, id: i32) {
        let mut store = self.meshes.lock();
        if let Some(m) = store.map.remove(&id) {
            store.total_points -= m.points().len();
        }
    }

    fn draw_text(&self, text: &str, x: i32, y: i32, fg: u32, bg: u32) {
        let scale = self.ctx.font_scale;
        self.draw(|c| c.draw_text_scaled(x, y, text, fg, bg, scale));
    }

    fn draw_window_frame(&self, title: &str) {
        let (w, _) = self.live_size();
        self.draw(|c| chrome::draw_window_frame(c, w, title));
    }

    fn draw_window_border(&self) {
        let (w, h) = self.live_size();
        self.draw(|c| chrome::draw_window_border(c, w, h));
    }

    fn clear_user_area(&self) {
        let (w, h) = self.live_size();
        self.draw(|c| chrome::clear_user_area(c, w, h));
    }

    fn am_i_focused(&self) -> bool {
        self.ctx.kernel.focus() == Some(self.ctx.task)
    }

    fn launch_arg(&self) -> String {
        self.ctx.arg.clone().unwrap_or_default()
    }

    fn play_note(&self, voice: i32, ona: i32, volume: i32) {
        self.ctx.kernel.audio_note_on(self.ctx.task, voice, ona, volume)
    }
    fn stop_note(&self, voice: i32) { self.ctx.kernel.audio_note_off(voice) }
    fn configure_voice(&self, voice: i32, filter_route: i32, attack_ms: i32, decay_ms: i32, sustain_percent: i32, release_ms: i32) {
        self.ctx.kernel.audio_configure_voice(voice, filter_route, attack_ms, decay_ms, sustain_percent, release_ms)
    }
    fn configure_filter(&self, cutoff: i32, resonance: i32, mode: i32) {
        self.ctx.kernel.audio_configure_filter(cutoff, resonance, mode)
    }
    fn trigger_arp(&self, voice: i32, notes: [i32; 4], count: i32, rate_ms: i32) {
        self.ctx.kernel.audio_trigger_arp(voice, notes, count, rate_ms)
    }
    fn configure_osc(&self, voice: i32, waveform: i32, duty_percent: i32) {
        self.ctx.kernel.audio_configure_osc(voice, waveform, duty_percent)
    }
    fn set_ring_partner(&self, voice: i32, partner: i32) { self.ctx.kernel.audio_set_ring_partner(voice, partner) }
    fn set_volume(&self, percent: i32) { self.ctx.kernel.set_master_volume(percent) }
    fn volume(&self) -> i32 { self.ctx.kernel.master_volume() }
    fn active_voice_count(&self) -> i32 { self.ctx.kernel.active_voice_count() as i32 }

    fn overlay_open(&self) -> bool {
        // Spec §16.2: a cart never takes the whole screen.
        if self.ctx.cart {
            return false;
        }
        self.ctx.kernel.overlay_open(self.ctx.task)
    }

    fn overlay_clear(&self) {
        self.ctx.kernel.overlay_clear(self.ctx.task)
    }

    fn overlay_fill_rect(&self, x: i32, y: i32, w: i32, h: i32, color: u32) {
        self.ctx.kernel.overlay_fill_rect(self.ctx.task, x, y, w, h, color)
    }

    fn overlay_close(&self) {
        self.ctx.kernel.overlay_close(self.ctx.task)
    }

    fn repaint_region(&self, x: i32, y: i32, w: i32, h: i32) {
        // Holds the canvas guard across a Kernel call: allowed only because
        // paint_wallpaper_into takes no lock.
        self.draw(|c| self.ctx.kernel.paint_wallpaper_into(c, x, y, w, h));
    }

    fn set_wallpaper_enabled(&self, on: bool) {
        self.ctx.kernel.set_wallpaper_enabled(on)
    }

    fn wallpaper_enabled(&self) -> bool {
        self.ctx.kernel.wallpaper_enabled()
    }

    fn window_max(&self) -> i32 {
        WINDOW_MAX as i32
    }

    fn screen_size(&self) -> (i32, i32) {
        let s = self.ctx.kernel.screen();
        (s.w, s.h)
    }

    fn font_size(&self) -> (i32, i32) {
        let s = self.ctx.font_scale;
        (6 * s, 8 * s)
    }

    fn window_size(&self) -> (i32, i32) {
        self.live_size()
    }

    fn font_scale(&self) -> i32 {
        self.ctx.kernel.font_scale()
    }

    fn set_font_scale(&self, scale: i32) {
        self.ctx.kernel.set_font_scale(scale)
    }

    fn window_info(&self, i: i64) -> Option<WindowInfo> {
        self.ctx.kernel.window_info(index(i)?)
    }

    fn activate_window(&self, i: i64) {
        let Some(i) = index(i) else { return };
        if !self.ctx.cart {
            self.ctx.kernel.activate_index(i);
        } else if self.ctx.kernel.task_at_index(i) == Some(self.ctx.task) {
            // Spec §16.2: a cart may raise only its own window. Activate the
            // cart's own task directly, so the index is looked up only once.
            self.ctx.kernel.activate_window(self.ctx.task);
        }
    }

    fn is_cart(&self) -> bool {
        self.ctx.cart
    }

    fn restart(&self) -> bool {
        // Ending every app is not a cart's call to make (spec §16.2).
        if self.ctx.cart {
            return false;
        }
        self.ctx.kernel.restart()
    }

    fn close_window(&self, i: i64) -> bool {
        // Spec §16.2: a cart closes no windows (its own ends with its run loop).
        if self.ctx.cart {
            return false;
        }
        index(i).is_some_and(|i| self.ctx.kernel.close_index(i, self.ctx.task))
    }

    fn send_self_to_back(&self) {
        self.ctx.kernel.send_to_back(self.ctx.task)
    }

    fn launcher_register(&self, path: &str, name: &str, w: i32, h: i32, multi: bool, libs: &str) -> bool {
        if self.ctx.cart {
            return false;
        }
        self.ctx.kernel.launcher_register(LaunchableApp {
            path: path.into(),
            name: name.into(),
            w,
            h,
            multi,
            libs: non_empty(libs),
        })
    }

    fn launcher_count(&self) -> i32 {
        self.ctx.kernel.launcher_count() as i32
    }

    fn launcher_path(&self, i: i64) -> Option<String> {
        Some(self.ctx.kernel.launcher_entry(index(i)?)?.path)
    }

    fn launcher_name(&self, i: i64) -> Option<String> {
        Some(self.ctx.kernel.launcher_entry(index(i)?)?.name)
    }

    fn launcher_spawn(&self, i: i64) -> bool {
        if self.windowless_cart() {
            return false;
        }
        index(i).is_some_and(|i| self.ctx.kernel.launch(i, self.ctx.cart))
    }

    fn spawn_app(&self, path: &str, w: i32, h: i32, arg: &str) -> bool {
        if (self.ctx.cart && !path.starts_with("v3/apps/")) || self.windowless_cart() {
            return false;
        }
        self.ctx.kernel.spawn_by_path(path, w, h, non_empty(arg), self.ctx.cart)
    }

    fn composited_frames(&self) -> u32 {
        self.ctx.kernel.composited_frames()
    }

    fn skipped_frames(&self) -> u32 {
        self.ctx.kernel.skipped_frames()
    }

    fn local_time(&self) -> LocalTime {
        self.ctx.kernel.platform().local_time()
    }

    fn mem_used_kb(&self) -> i64 {
        self.ctx.kernel.platform().mem_used_kb()
    }

    fn network_info(&self) -> NetworkInfo {
        self.ctx.kernel.platform().network_info()
    }

    fn refresh_tasks(&self) -> i32 {
        self.ctx.kernel.refresh_tasks() as i32
    }

    fn task_count(&self) -> i32 {
        self.ctx.kernel.task_count() as i32
    }

    fn task_info(&self, i: i64) -> Option<TaskInfo> {
        self.ctx.kernel.task_info(index(i)?)
    }

    fn fs_list(&self, dir: &str) -> Result<Vec<String>, String> {
        if !acid_kernel::fs_path::fs_path_is_allowed(dir) {
            return Err(String::from("bad path"));
        }
        self.ctx.kernel.platform().fs().list(dir).map_err(fs_error)
    }

    fn fs_read(&self, path: &str) -> Result<Vec<u8>, String> {
        if !acid_kernel::fs_path::fs_path_is_allowed(path) {
            return Err(String::from("bad path"));
        }
        self.ctx.kernel.platform().fs().read(path).map_err(fs_error)
    }

    fn fs_size(&self, path: &str) -> Result<u64, String> {
        if !acid_kernel::fs_path::fs_path_is_allowed(path) {
            return Err(String::from("bad path"));
        }
        self.ctx.kernel.platform().fs().size(path).map_err(fs_error)
    }

    fn fs_write(&self, path: &str, data: &[u8]) -> Result<(), String> {
        if !acid_kernel::fs_path::fs_path_is_allowed(path) {
            return Err(String::from("bad path"));
        }
        if !self.cart_may_change(path) {
            return Err(String::from("read only"));
        }
        self.ctx.kernel.platform().fs().write(path, data).map_err(fs_error)
    }

    fn fs_rename(&self, from: &str, to: &str) -> Result<(), String> {
        if !acid_kernel::fs_path::fs_path_is_allowed(from) || !acid_kernel::fs_path::fs_path_is_allowed(to) {
            return Err(String::from("bad path"));
        }
        if !self.cart_may_change(from) || !self.cart_may_change(to) {
            return Err(String::from("read only"));
        }
        self.ctx.kernel.platform().fs().rename(from, to).map_err(fs_error)
    }

    fn fs_delete(&self, path: &str) -> Result<(), String> {
        if !acid_kernel::fs_path::fs_path_is_allowed(path) {
            return Err(String::from("bad path"));
        }
        if !self.cart_may_change(path) {
            return Err(String::from("read only"));
        }
        self.ctx.kernel.platform().fs().delete(path).map_err(fs_error)
    }

    fn cart_roots(&self) -> Result<Vec<String>, String> {
        self.cart_allowed()?;
        Ok(self.ctx.kernel.platform().cart_roots())
    }

    fn cart_list(&self, dir: &str) -> Result<Vec<String>, String> {
        self.cart_allowed()?;
        self.ctx.kernel.platform().cart_list(dir).map_err(fs_error)
    }

    fn cart_stat(&self, path: &str) -> Result<(bool, u64), String> {
        self.cart_allowed()?;
        self.ctx.kernel.platform().cart_stat(path).map(|s| (s.is_dir, s.size)).map_err(fs_error)
    }

    fn cart_read(&self, path: &str) -> Result<Vec<u8>, String> {
        self.cart_allowed()?;
        self.ctx.kernel.platform().cart_read(path).map_err(fs_error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use acid_gfx::rgb565;
    use acid_kernel::event::Event;
    use acid_kernel::theme::*;
    use acid_kernel::{AppRunner, Kernel, SpawnRequest};
    use acid_testkit::FakePlatform;
    use std::sync::{Arc, mpsc};
    use std::time::Duration;

    fn spawn(w: i32, h: i32) -> (Arc<Kernel>, KernelApi) {
        spawn_on(FakePlatform::new("."), w, h)
    }

    fn spawn_on(p: Arc<FakePlatform>, w: i32, h: i32) -> (Arc<Kernel>, KernelApi) {
        spawn_path_on(p, "v3/apps/t.lua", w, h)
    }

    fn spawn_cart_on(p: Arc<FakePlatform>, w: i32, h: i32) -> (Arc<Kernel>, KernelApi) {
        spawn_path_on(p, "v3/fsroot/Home/t.lua", w, h)
    }

    fn spawn_path_on(p: Arc<FakePlatform>, path: &str, w: i32, h: i32) -> (Arc<Kernel>, KernelApi) {
        spawn_scaled_on(p, path, w, h, 1)
    }

    /// Like `spawn_path_on`, with Config's font setting at `scale` first.
    fn spawn_scaled_on(p: Arc<FakePlatform>, path: &str, w: i32, h: i32, scale: i32) -> (Arc<Kernel>, KernelApi) {
        let k = Kernel::new(p);
        k.set_font_scale(scale);
        let (tx, rx) = mpsc::channel();
        let tx = std::sync::Mutex::new(tx);
        // Hands the context over and parks without touching the event
        // queue, so the test is the queue's only reader.
        let runner: AppRunner = Arc::new(move |ctx: AppContext| {
            // A later spawn (a test calling spawn_app) finds the receiver
            // gone; it just parks too.
            let _ = tx.lock().unwrap().send(ctx);
            loop {
                std::thread::park();
            }
        });
        k.set_runner(runner);
        k.spawn_app(SpawnRequest { script_path: path.into(), x: 0, y: 30, w, h, closable: true, arg: None, libs: None, force_cart: false }).unwrap();
        let ctx = rx.recv_timeout(Duration::from_secs(5)).unwrap();
        (k, KernelApi::new(ctx))
    }

    #[test]
    fn cart_level_apps_write_only_under_home_and_cannot_register_or_spawn_outside_apps() {
        let p = FakePlatform::new(FakePlatform::repo_root());
        let (_k, api) = spawn_cart_on(p.clone(), 10, 10);
        assert!(api.context().cart);
        assert_eq!(api.fs_write("v3/fsroot/Tmp/c.txt", b"x"), Err("read only".into()));
        assert_eq!(api.fs_write("v3/apps/x.lua", b"x"), Err("read only".into()));
        assert_eq!(api.fs_delete("v3/apps/tetris.lua"), Err("read only".into()));
        assert_eq!(api.fs_rename("v3/fsroot/Home/notes.txt", "v3/apps/n.txt"), Err("read only".into()));
        let home = format!("v3/fsroot/Home/cart-{}.txt", std::process::id());
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup { fn drop(&mut self) { let _ = std::fs::remove_file(&self.0); } }
        let _c = Cleanup(FakePlatform::repo_root().join(&home));
        assert_eq!(api.fs_write(&home, b"hi"), Ok(()), "Home is writable");
        assert!(!api.launcher_register("v3/apps/x.lua", "X", 100, 80, false, ""));
        assert!(!api.spawn_app("v3/fsroot/Home/x.lua", 100, 80, ""));
        let (_k2, built) = spawn_on(p, 10, 10);
        assert!(!built.context().cart);
        assert!(built.launcher_register("v3/apps/x.lua", "X", 100, 80, false, ""), "built-ins unchanged");
    }

    #[test]
    fn only_built_in_apps_may_restart() {
        let p = FakePlatform::new(FakePlatform::repo_root());
        let (_k, cart) = spawn_cart_on(p.clone(), 10, 10);
        assert!(!cart.restart(), "a cart is refused");
        assert_eq!(p.restart_count(), 0, "and the platform never hears of it");
        let (_k2, built) = spawn_on(p.clone(), 10, 10);
        assert!(built.restart(), "a built-in app's restart reaches the platform");
        assert_eq!(p.restart_count(), 1);
    }

    #[test]
    fn cart_rule_edges() {
        let p = FakePlatform::new(FakePlatform::repo_root());
        let (k, api) = spawn_cart_on(p.clone(), 10, 10);
        assert!(api.context().cart);
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup { fn drop(&mut self) { let _ = std::fs::remove_file(&self.0); } }
        let id = std::process::id();
        // Renaming from outside Home into Home is refused at the source end.
        let tmp = format!("v3/fsroot/Tmp/edge-{id}.txt");
        let _c1 = Cleanup(FakePlatform::repo_root().join(&tmp));
        std::fs::write(FakePlatform::repo_root().join(&tmp), b"x").unwrap();
        let dest = format!("v3/fsroot/Home/edge-{id}.txt");
        let _c3 = Cleanup(FakePlatform::repo_root().join(&dest));
        assert_eq!(api.fs_rename(&tmp, &dest), Err("read only".into()));
        assert!(FakePlatform::repo_root().join(&tmp).exists(), "the source is untouched");
        // Home itself, and a sibling whose name only starts with Home.
        assert_eq!(api.fs_write("v3/fsroot/Home", b"x"), Err("read only".into()));
        assert_eq!(api.fs_delete("v3/fsroot/Home"), Err("read only".into()));
        assert_eq!(api.fs_write("v3/fsroot/HomeX/x", b"x"), Err("read only".into()));
        assert_eq!(api.fs_rename("v3/fsroot/Home/a", "v3/fsroot/HomeX/a"), Err("read only".into()));
        // A delete inside Home works.
        let home = format!("v3/fsroot/Home/edge-del-{id}.txt");
        let _c2 = Cleanup(FakePlatform::repo_root().join(&home));
        std::fs::write(FakePlatform::repo_root().join(&home), b"x").unwrap();
        assert_eq!(api.fs_delete(&home), Ok(()));
        assert!(!FakePlatform::repo_root().join(&home).exists());
        // A cart may still spawn an app from v3/apps.
        let before = k.with_state(|st| st.windows.count());
        assert!(api.spawn_app("v3/apps/t2.lua", 10, 10, ""), "spawning a v3/apps app is allowed");
        assert_eq!(k.with_state(|st| st.windows.count()), before + 1);
    }

    fn px(api: &KernelApi, x: i32, y: i32) -> u16 {
        api.context().canvas.lock().pixel(x, y).unwrap()
    }

    #[test]
    fn fs_calls_are_path_guarded_and_read_the_platform_fs() {
        let (_k, api) = spawn_on(FakePlatform::new(FakePlatform::repo_root()), 10, 10);
        assert_eq!(api.fs_read("../etc/passwd"), Err("bad path".into()));
        assert_eq!(api.fs_list("v3/crates"), Err("bad path".into()), "only v3/apps and v3/fsroot are visible");
        assert!(api.fs_list("v3/apps").unwrap().iter().any(|n| n == "desktop.lua"));
        let hello = api.fs_read("v3/apps/hello_acid.app.toml").unwrap();
        assert!(hello.starts_with(b"name = Hello Acid"));
        assert_eq!(api.fs_read("v3/apps/no_such_file"), Err("not found".into()));
    }

    #[test]
    fn write_calls_are_path_guarded() {
        let (_k, api) = spawn_on(FakePlatform::new(FakePlatform::repo_root()), 10, 10);
        assert_eq!(api.fs_write("v3/crates/x.txt", b"x"), Err("bad path".into()));
        assert_eq!(api.fs_rename("v3/fsroot/Home/notes.txt", "/tmp/x"), Err("bad path".into()));
        assert_eq!(api.fs_delete("../x"), Err("bad path".into()));
        let p = format!("v3/fsroot/Tmp/api-write-{}.txt", std::process::id());
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(&self.0);
            }
        }
        let _cleanup = Cleanup(FakePlatform::repo_root().join(&p));
        assert_eq!(api.fs_write(&p, b"abc"), Ok(()));
        assert_eq!(api.fs_read(&p), Ok(b"abc".to_vec()));
        assert_eq!(api.fs_delete(&p), Ok(()));
        assert_eq!(api.fs_delete(&p), Err("not found".into()));
    }

    #[test]
    fn cart_calls_are_built_in_only() {
        let p = FakePlatform::new(FakePlatform::repo_root());
        // Any existing directory works as a root here; v3/carts only arrives in Task 4.
        p.set_cart_roots(vec!["v3/apps".into()]);
        let (_k, api) = spawn_on(p.clone(), 10, 10);
        assert_eq!(api.cart_roots(), Ok(vec!["v3/apps".to_string()]));
        let (_k2, cart) = spawn_cart_on(p, 10, 10);
        assert_eq!(cart.cart_roots(), Err("not allowed".into()));
        assert_eq!(cart.cart_read("v3/carts/hello_acid.cart"), Err("not allowed".into()));
        assert_eq!(cart.cart_list("v3/apps"), Err("not allowed".into()));
        assert_eq!(cart.cart_stat("v3/apps"), Err("not allowed".into()));
    }

    #[test]
    fn fs_size_is_path_guarded_and_matches_the_file() {
        let (_k, api) = spawn_on(FakePlatform::new(FakePlatform::repo_root()), 10, 10);
        let real = std::fs::metadata(FakePlatform::repo_root().join("v3/apps/hello_acid.app.toml")).unwrap().len();
        assert_eq!(api.fs_size("v3/apps/hello_acid.app.toml"), Ok(real));
        assert_eq!(api.fs_size("v3/crates/acid-os/Cargo.toml"), Err("bad path".into()));
        assert_eq!(api.fs_size("v3/apps/no_such_file"), Err("not found".into()));
    }

    #[test]
    fn system_info_comes_from_the_platform_and_kernel() {
        let p = FakePlatform::new(".");
        let (_k, api) = spawn_on(p.clone(), 10, 10);
        let t = LocalTime { year: 2027, month: 1, day: 31, hour: 23, min: 59, sec: 58 };
        p.set_local_time(t);
        p.set_mem_used_kb(77);
        p.set_network_info(NetworkInfo { host: "h".into(), ip: "10.0.0.9".into(), connected: true });
        p.set_thread_samples(vec![acid_platform::ThreadSample { id: 1, name: "router".into(), state: 'S', cpu_ms: 0 }]);
        assert_eq!(api.local_time(), t);
        assert_eq!(api.mem_used_kb(), 77);
        assert_eq!(api.network_info().ip, "10.0.0.9");
        assert_eq!(api.refresh_tasks(), 1);
        assert_eq!(api.task_count(), 1);
        assert_eq!(api.task_info(0).unwrap().name, "router");
        assert!(api.task_info(1).is_none() && api.task_info(-1).is_none());
    }

    #[test]
    fn poll_event_maps_kernel_events() {
        let (_k, api) = spawn(10, 10);
        let q = api.context().queue.clone();
        q.send(Event::Key { code: 65 });
        q.send(Event::Touch { x: 1, y: 2, pressed: false });
        q.send(Event::Moved { x: 7, y: 8 });
        q.send(Event::Close);
        assert_eq!(api.poll_event(100), Some(PolledEvent::Key { code: 65, pressed: true }));
        assert_eq!(api.poll_event(100), Some(PolledEvent::Touch { x: 1, y: 2, pressed: false }));
        assert_eq!(api.poll_event(100), Some(PolledEvent::Moved));
        assert_eq!(api.window_pos(), (7, 8), "Moved updates the app's idea of its position");
        assert_eq!(api.poll_event(100), Some(PolledEvent::Close));
    }

    #[test]
    fn poll_event_maps_resized() {
        let (_k, api) = spawn(10, 10);
        api.context().queue.send(Event::Resized { w: 300, h: 200 });
        assert_eq!(api.poll_event(100), Some(PolledEvent::Resized { w: 300, h: 200 }));
    }

    #[test]
    fn window_size_and_chrome_follow_the_live_canvas() {
        let (_k, api) = spawn(100, 80);
        *api.context().canvas.lock() = acid_gfx::Canvas::new(150, 90);
        assert_eq!(api.window_size(), (150, 90));
        api.draw_window_border();
        assert_eq!(px(&api, 149, 45), rgb565(THEME_HARD));
    }

    #[test]
    fn poll_event_times_out_and_treats_negative_as_zero() {
        let (_k, api) = spawn(10, 10);
        let t = std::time::Instant::now();
        assert_eq!(api.poll_event(-5), None);
        assert!(t.elapsed() < Duration::from_millis(40));
    }

    #[test]
    fn now_ms_is_the_platform_clock() {
        let (_k, a) = spawn(10, 10);
        let t0 = a.now_ms();
        std::thread::sleep(Duration::from_millis(20));
        let t1 = a.now_ms();
        assert!(t1 >= t0 + 15, "{t0} -> {t1}");
    }

    #[test]
    fn drawing_marks_the_kernel_dirty() {
        let (k, api) = spawn(10, 10);
        k.take_dirty();
        api.fill_rect(0, 0, 2, 2, 0xFF0000);
        assert!(k.take_dirty());
        assert_eq!(px(&api, 1, 1), rgb565(0xFF0000));
        api.fill_circle(5, 5, 1, 0x00FF00);
        assert!(k.take_dirty());
        api.draw_text("A", 0, 0, 0xFFFFFF, 0x000000);
        assert!(k.take_dirty());
    }

    #[test]
    fn window_frame_title_bar_and_close_dot() {
        let (_k, api) = spawn(100, 50);
        api.draw_window_frame("Hi");
        assert_eq!(px(&api, 50, 0), rgb565(THEME_PANEL));
        assert_eq!(px(&api, 50, 15), rgb565(THEME_PANEL));
        assert_eq!(px(&api, 92, 8), rgb565(THEME_HARD), "close dot centre (w-8, 8)");
        assert_eq!(px(&api, 4, 6), rgb565(THEME_TEXT), "'H' column 0 at (4, 4)+row2");
        assert_eq!(px(&api, 50, 16), 0, "frame stops at the title bar");
    }

    #[test]
    fn window_border_and_rounded_corners() {
        let (_k, api) = spawn(100, 50);
        api.draw_window_border();
        assert_eq!(px(&api, 0, 25), rgb565(THEME_HARD));
        assert_eq!(px(&api, 99, 25), rgb565(THEME_HARD));
        assert_eq!(px(&api, 50, 49), rgb565(THEME_HARD));
        // Top-left corner, r = 3, background THEME_PANEL.
        assert_eq!(px(&api, 0, 0), rgb565(THEME_PANEL));
        assert_eq!(px(&api, 1, 0), rgb565(THEME_PANEL));
        assert_eq!(px(&api, 2, 0), rgb565(THEME_HARD));
        assert_eq!(px(&api, 0, 1), rgb565(THEME_PANEL));
        assert_eq!(px(&api, 1, 1), rgb565(THEME_HARD));
        assert_eq!(px(&api, 0, 2), rgb565(THEME_HARD));
        // Bottom-left uses THEME_BG.
        assert_eq!(px(&api, 0, 49), rgb565(THEME_BG));
        assert_eq!(px(&api, 1, 49), rgb565(THEME_BG));
        assert_eq!(px(&api, 2, 49), rgb565(THEME_HARD));
        // Top-right mirrors.
        assert_eq!(px(&api, 99, 0), rgb565(THEME_PANEL));
        assert_eq!(px(&api, 97, 0), rgb565(THEME_HARD));
    }

    #[test]
    fn clear_user_area_spares_the_title_bar() {
        let (_k, api) = spawn(20, 30);
        api.fill_rect(0, 0, 20, 30, 0xFFFFFF);
        api.clear_user_area();
        assert_eq!(px(&api, 5, 15), 0xFFFF);
        assert_eq!(px(&api, 5, 16), rgb565(THEME_BG));
        assert_eq!(px(&api, 19, 29), rgb565(THEME_BG));
    }

    #[test]
    fn focus_and_launch_arg() {
        let (k, api) = spawn(10, 10);
        assert!(!api.am_i_focused());
        k.activate_window(api.context().task);
        assert!(api.am_i_focused());
        assert_eq!(api.launch_arg(), "");
    }

    /// A kernel whose apps hand over their context and park forever without
    /// reading their queue, plus the receiver for every spawned app.
    fn kernel_with_parked_apps() -> (Arc<Kernel>, mpsc::Receiver<AppContext>) {
        let k = Kernel::new(FakePlatform::new("."));
        let (tx, rx) = mpsc::channel();
        let tx = std::sync::Mutex::new(tx);
        k.set_runner(Arc::new(move |ctx: AppContext| {
            let _ = tx.lock().unwrap().send(ctx);
            loop {
                std::thread::park();
            }
        }));
        (k, rx)
    }

    fn api_at(k: &Arc<Kernel>, rx: &mpsc::Receiver<AppContext>, x: i32, y: i32, w: i32, h: i32) -> KernelApi {
        k.spawn_app(SpawnRequest {
            script_path: alloc::format!("v3/apps/app{x}.lua"), x, y, w, h, closable: true, arg: None, libs: None, force_cart: false,
        })
        .unwrap();
        KernelApi::new(rx.recv_timeout(Duration::from_secs(5)).unwrap())
    }

    #[test]
    fn overlay_goes_through_the_callers_task() {
        let (k, rx) = kernel_with_parked_apps();
        let a = api_at(&k, &rx, 0, 30, 10, 10);
        let b = api_at(&k, &rx, 20, 30, 10, 10);
        assert!(a.overlay_open());
        assert!(!b.overlay_open());
        a.overlay_fill_rect(0, 0, 2, 2, 0xFFFFFF);
        a.overlay_close();
        assert!(!k.overlay_is_open());
        a.overlay_clear(); // closed: silently nothing
    }

    #[test]
    fn repaint_region_paints_wallpaper_and_marks_dirty() {
        let (k, rx) = kernel_with_parked_apps();
        let a = api_at(&k, &rx, 0, 0, 640, 60);
        k.take_dirty();
        a.repaint_region(0, 24, 640, 36);
        assert!(k.take_dirty());
        let wall = acid_gfx::wallpaper::wallpaper_canvas();
        assert_eq!(a.context().canvas.lock().pixel(224, 30), wall.pixel(224, 30));
        a.set_wallpaper_enabled(false);
        assert!(!a.wallpaper_enabled());
        a.repaint_region(0, 24, 640, 36);
        assert_eq!(a.context().canvas.lock().pixel(224, 30), Some(rgb565(THEME_BG)));
    }

    #[test]
    fn carts_cannot_close_or_raise_other_windows_or_open_the_overlay() {
        let (k, rx) = kernel_with_parked_apps();
        let built = api_at(&k, &rx, 20, 30, 10, 10); // index 0
        k.spawn_app(SpawnRequest {
            script_path: "v3/fsroot/Home/c.lua".into(), x: 0, y: 30, w: 10, h: 10,
            closable: true, arg: None, libs: None, force_cart: false,
        })
        .unwrap();
        let cart = KernelApi::new(rx.recv_timeout(Duration::from_secs(5)).unwrap());
        assert!(cart.context().cart);
        let cart_index = (0..8).find(|&i| k.task_at_index(i) == Some(cart.context().task)).unwrap() as i64;
        let other_index = 1 - cart_index;
        // Close: refused, the window stays.
        assert!(!cart.close_window(other_index));
        assert_eq!(k.with_state(|st| st.windows.count()), 2);
        // Raise another window: nothing; raise its own: allowed.
        built.activate_window(cart_index);
        assert!(cart.window_info(cart_index).unwrap().focused);
        cart.activate_window(other_index);
        assert!(cart.window_info(cart_index).unwrap().focused, "focus unchanged");
        built.activate_window(other_index);
        cart.activate_window(cart_index);
        assert!(cart.window_info(cart_index).unwrap().focused, "own window allowed");
        // Edge indices: nothing happens, nothing is closed.
        for i in [-1, 7] {
            cart.activate_window(i);
            assert!(cart.window_info(cart_index).unwrap().focused, "activate_window({i})");
        }
        assert!(!cart.close_window(-1));
        assert!(!cart.close_window(7));
        assert_eq!(k.with_state(|st| st.windows.count()), 2);
        // Launching an open singleton would raise it: refused for the cart,
        // focus and z-order unchanged (spec §16.2).
        assert!(built.launcher_register("v3/apps/app20.lua", "Built", 10, 10, false, ""));
        let z = || k.with_state(|st| st.windows.in_z_order().iter().map(|w| w.task).collect::<Vec<_>>());
        let before = z();
        assert!(!cart.launcher_spawn(0));
        assert!(!cart.spawn_app("v3/apps/app20.lua", 10, 10, ""));
        assert!(cart.window_info(cart_index).unwrap().focused, "focus unchanged");
        assert_eq!(z(), before, "z-order unchanged");
        assert_eq!(k.with_state(|st| st.windows.count()), 2, "nothing spawned");
        // A built-in still raises it.
        assert!(built.spawn_app("v3/apps/app20.lua", 10, 10, ""));
        assert!(built.window_info(other_index).unwrap().focused, "built-in raises");
        // Overlay: refused for the cart, still fine for a built-in.
        assert!(!cart.overlay_open());
        assert!(!k.overlay_is_open());
        assert!(built.overlay_open());
        built.overlay_close();
        // Built-ins unchanged.
        assert!(built.close_window(cart_index));
    }

    /// A cart-level app spawned straight by the kernel (as Load Cart does).
    fn cart_api(k: &Arc<Kernel>, rx: &mpsc::Receiver<AppContext>, path: &str) -> KernelApi {
        k.spawn_app(SpawnRequest {
            script_path: path.into(), x: 0, y: 30, w: 10, h: 10,
            closable: true, arg: None, libs: None, force_cart: false,
        })
        .unwrap();
        let api = KernelApi::new(rx.recv_timeout(Duration::from_secs(5)).unwrap());
        assert!(api.context().cart, "{path} is cart-level");
        api
    }

    fn next_api(rx: &mpsc::Receiver<AppContext>) -> KernelApi {
        KernelApi::new(rx.recv_timeout(Duration::from_secs(5)).unwrap())
    }

    #[test]
    fn an_app_a_cart_starts_runs_cart_level() {
        // A Lua cart and a WASM cart, by spawn_app and by launcher_spawn,
        // starting built-in apps (by path and manifest). A fresh kernel each,
        // so the cart window cap stays out of the way.
        for cart_path in ["v3/fsroot/Home/c.lua", "v3/fsroot/Home/c.wasm"] {
            for by_launcher in [false, true] {
                let (k, rx) = kernel_with_parked_apps();
                let built = api_at(&k, &rx, 20, 30, 10, 10);
                assert!(built.launcher_register("v3/apps/file_manager.lua", "Files", 100, 80, false, ""));
                let cart = cart_api(&k, &rx, cart_path);
                let how = if by_launcher { "launcher_spawn" } else { "spawn_app" };
                assert!(if by_launcher { cart.launcher_spawn(0) } else { cart.spawn_app("v3/apps/editor.lua", 100, 80, "") });
                let child = next_api(&rx);
                assert!(child.context().cart, "{cart_path} {how}: child is cart-level");
                assert_eq!(child.fs_write("v3/fsroot/Tmp/c.txt", b"x"), Err("read only".into()));
                assert!(!child.launcher_register("v3/apps/x.lua", "X", 100, 80, false, ""));
                assert!(!child.close_window(0));
                // The taint carries on down: the child's own spawn is a cart's.
                assert!(child.spawn_app("v3/apps/grandchild.lua", 100, 80, ""));
                assert!(next_api(&rx).context().cart, "{cart_path} {how}: grandchild is cart-level");
                // A built-in starting a built-in app gets a built-in.
                assert!(built.spawn_app("v3/apps/sysmon.lua", 100, 80, ""));
                assert!(!next_api(&rx).context().cart, "built-in caller: built-in app");
            }
        }
    }

    #[test]
    fn a_cart_cannot_spawn_while_cart_window_max_cart_windows_are_open() {
        use acid_kernel::layout::CART_WINDOW_MAX;
        let (k, rx) = kernel_with_parked_apps();
        let built = api_at(&k, &rx, 20, 30, 10, 10);
        let count = || k.with_state(|st| st.windows.count());
        let cart = cart_api(&k, &rx, "v3/fsroot/Home/c.wasm");
        // The cart is one cart-level window; it may start CART_WINDOW_MAX - 1.
        for i in 1..CART_WINDOW_MAX {
            assert!(cart.spawn_app(&alloc::format!("v3/apps/k{i}.lua"), 50, 50, ""), "{i} cart windows open");
            assert!(next_api(&rx).context().cart);
        }
        let before = count();
        assert_eq!(before, 1 + CART_WINDOW_MAX);
        assert!(!cart.spawn_app("v3/apps/k9.lua", 50, 50, ""), "the cap is reached");
        assert!(built.launcher_register("v3/apps/m.lua", "M", 50, 50, true, ""));
        assert!(!cart.launcher_spawn(0), "launcher_spawn: same");
        assert_eq!(count(), before, "window count unchanged");
        // A built-in caller is not refused.
        assert!(built.spawn_app("v3/apps/k9.lua", 50, 50, ""));
        assert!(!next_api(&rx).context().cart);
        assert!(built.launcher_spawn(0));
        assert!(!next_api(&rx).context().cart);
        assert_eq!(count(), before + 2);
    }

    #[test]
    fn a_cart_whose_window_is_closed_cannot_spawn() {
        let (k, rx) = kernel_with_parked_apps();
        let built = api_at(&k, &rx, 20, 30, 10, 10);
        assert!(built.launcher_register("v3/apps/m.lua", "M", 50, 50, true, ""));
        let cart = cart_api(&k, &rx, "v3/fsroot/Home/c.lua");
        assert!(cart.spawn_app("v3/apps/a.lua", 50, 50, ""), "with its window, the cart may spawn");
        next_api(&rx);
        // Closing unregisters the window at once; a cart that ignores "close"
        // keeps running windowless, and must not keep spawning uncounted.
        k.close_window(cart.context().task);
        let count = || k.with_state(|st| st.windows.count());
        let before = count();
        assert!(!cart.spawn_app("v3/apps/b.lua", 50, 50, ""), "spawn_app: no window");
        assert!(!cart.launcher_spawn(0), "launcher_spawn: no window");
        assert_eq!(count(), before, "nothing spawned");
        // A windowless built-in is not refused by this rule.
        k.close_window(built.context().task);
        assert!(built.spawn_app("v3/apps/b.lua", 50, 50, ""));
        assert!(!next_api(&rx).context().cart);
    }

    #[test]
    fn window_queries_and_control() {
        let (k, rx) = kernel_with_parked_apps();
        let a = api_at(&k, &rx, 0, 30, 10, 10);
        let _b = api_at(&k, &rx, 20, 30, 10, 10);
        assert_eq!(a.window_max(), 8);
        assert_eq!(a.window_info(1).unwrap().x, 20);
        assert_eq!(a.window_info(-1), None);
        assert_eq!(a.window_info(7), None);
        a.activate_window(1);
        assert!(a.window_info(1).unwrap().focused);
        assert!(!a.close_window(0), "own window");
        assert!(a.close_window(1));
        assert!(!a.close_window(-3));
        a.send_self_to_back();
        assert_eq!(k.with_state(|st| st.windows.count()), 1);
    }

    #[test]
    fn launcher_and_spawn_app() {
        let (k, rx) = kernel_with_parked_apps();
        let a = api_at(&k, &rx, 0, 30, 10, 10);
        assert!(a.launcher_register("v3/apps/x.lua", "X", 100, 80, true, ""));
        assert_eq!(a.launcher_count(), 1);
        assert_eq!(a.launcher_path(0).as_deref(), Some("v3/apps/x.lua"));
        assert_eq!(a.launcher_name(0).as_deref(), Some("X"));
        assert_eq!(a.launcher_path(1), None);
        assert_eq!(a.launcher_path(-1), None);
        assert_eq!(k.launcher_entry(0).unwrap().libs, None, "empty libs string means none");
        assert!(a.launcher_spawn(0));
        assert_eq!(rx.recv_timeout(Duration::from_secs(5)).unwrap().script_path, "v3/apps/x.lua");
        assert!(!a.launcher_spawn(5));
        assert!(a.spawn_app("v3/apps/e.lua", 100, 80, "file.txt"));
        assert_eq!(rx.recv_timeout(Duration::from_secs(5)).unwrap().arg.as_deref(), Some("file.txt"));
        assert!(a.spawn_app("v3/apps/f.lua", 100, 80, ""));
        assert_eq!(rx.recv_timeout(Duration::from_secs(5)).unwrap().arg, None, "empty arg means none");
    }

    #[test]
    fn frame_counters_come_from_the_kernel() {
        let (k, rx) = kernel_with_parked_apps();
        let a = api_at(&k, &rx, 0, 30, 10, 10);
        k.tick();
        k.tick();
        assert_eq!((a.composited_frames(), a.skipped_frames()), (1, 1));
    }

    /// Drive the API and a bare Synth through the same call sequence and
    /// require identical bytes (the technique of acid-kernel's
    /// commands_match_the_bare_synth), so a mis-wired or swapped argument in
    /// KernelApi shows up as different audio. Rendered in two halves so a
    /// stop_note in `api_tail` / `synth_tail` (release stage) is covered too.
    fn assert_matches_bare_synth(
        api_head: impl Fn(&KernelApi),
        synth_head: impl Fn(&mut acid_synth::Synth),
        api_tail: impl Fn(&KernelApi),
        synth_tail: impl Fn(&mut acid_synth::Synth),
    ) {
        let (k, rx) = kernel_with_parked_apps();
        let a = api_at(&k, &rx, 0, 30, 10, 10);
        let mut s = acid_synth::Synth::new();
        api_head(&a);
        synth_head(&mut s);
        let (mut got, mut want) = (vec![0u8; 4096], vec![0u8; 4096]);
        k.render_audio(&mut got);
        s.render(&mut want);
        assert!(got.iter().any(|&b| b != 128), "the scenario must be audible");
        assert_eq!(got, want, "head");
        api_tail(&a);
        synth_tail(&mut s);
        k.render_audio(&mut got);
        s.render(&mut want);
        assert_eq!(got, want, "tail");
    }

    /// Note-on as the kernel applies it: ona, sustain level, gate.
    fn synth_note_on(s: &mut acid_synth::Synth, voice: i32, ona: i32, volume: i32) {
        s.set_ona(voice, ona);
        s.voice_mut(voice as usize).sustain_level = (volume * acid_synth::ENV_FULL / 100) << 8;
        s.gate_on(voice);
    }

    #[test]
    fn configure_osc_forwards_waveform_and_duty() {
        assert_matches_bare_synth(
            |a| { a.configure_osc(2, 1, 23); a.play_note(2, 45, 90); },
            |s| { s.set_voice_waveform(2, 1); s.set_duty(2, 23); synth_note_on(s, 2, 45, 90); },
            |_| {}, |_| {},
        );
    }

    #[test]
    fn configure_voice_forwards_route_and_adsr_in_order() {
        // Distinct a/d/s/r, and a filter that audibly colours the routed voice.
        // The attack/decay/release rates are read live by the envelope, so
        // setting them after the gate is still audible.
        assert_matches_bare_synth(
            |a| {
                a.configure_filter(70, 9, 1);
                // Configured after the gate so sustain_percent (55), not
                // play_note's volume, sets the sustain level the decay falls to.
                a.play_note(3, 47, 100);
                a.configure_voice(3, 1, 7, 31, 55, 113);
            },
            |s| {
                s.set_filter_cutoff(70); s.set_filter_resonance(9); s.set_filter_mode(1);
                s.set_voice_filter_route(3, 1);
                synth_note_on(s, 3, 47, 100);
                s.set_adsr(3, 7, 31, 55, 113);
            },
            |a| a.stop_note(3),
            |s| s.gate_off(3),
        );
    }

    #[test]
    fn configure_filter_forwards_cutoff_resonance_and_mode() {
        assert_matches_bare_synth(
            |a| {
                a.configure_osc(0, 2, 50);
                a.configure_voice(0, 1, 0, 0, 100, 0);
                a.configure_filter(45, 14, 2);
                a.play_note(0, 52, 100);
            },
            |s| {
                s.set_voice_waveform(0, 2); s.set_duty(0, 50);
                s.set_voice_filter_route(0, 1); s.set_adsr(0, 0, 0, 100, 0);
                s.set_filter_cutoff(45); s.set_filter_resonance(14); s.set_filter_mode(2);
                synth_note_on(s, 0, 52, 100);
            },
            |_| {}, |_| {},
        );
    }

    #[test]
    fn trigger_arp_forwards_notes_count_and_rate() {
        // Arp set up, then the gate; 3 of 4 notes at 17 ms. (Apps usually call
        // play_note first, then trigger_arp; the kernel test covers that
        // order.)
        assert_matches_bare_synth(
            |a| { a.trigger_arp(1, [40, 44, 47, 52], 3, 17); a.play_note(1, 40, 80); },
            |s| {
                for (i, n) in [40, 44, 47, 52].iter().enumerate() { s.set_arp_note(1, i as i32, *n); }
                s.set_arp_rate(1, 17);
                s.voice_mut(1).arp_step = 0;
                s.voice_mut(1).arp_step_counter = 0;
                s.arp_on(1, 3);
                synth_note_on(s, 1, 40, 80);
            },
            |_| {}, |_| {},
        );
    }

    #[test]
    fn set_ring_partner_forwards_voice_and_partner_and_clears() {
        // Triangle voices so the XOR modulation is audible. Voice 2 is
        // modulated by voice 5's phase (XOR with a silent partner's 0 phase
        // would be a no-op, so 5 plays too); swapping (voice, partner) would
        // modulate 5 by 2 instead and the bytes differ.
        assert_matches_bare_synth(
            |a| {
                a.configure_osc(2, 2, 50); a.configure_osc(5, 2, 50);
                a.set_ring_partner(2, 5);
                a.play_note(2, 45, 100); a.play_note(5, 52, 100);
            },
            |s| {
                s.set_voice_waveform(2, 2); s.set_duty(2, 50);
                s.set_voice_waveform(5, 2); s.set_duty(5, 50);
                s.set_ring_partner(2, 5);
                synth_note_on(s, 2, 45, 100); synth_note_on(s, 5, 52, 100);
            },
            |a| a.set_ring_partner(2, -1),
            |s| s.clear_ring_partner(2),
        );
    }

    #[test]
    fn audio_calls_drive_the_kernel_synth() {
        let (k, rx) = kernel_with_parked_apps();
        let a = api_at(&k, &rx, 0, 30, 10, 10);
        a.configure_osc(0, 1, 50);
        a.configure_voice(0, 0, 0, 0, 100, 0);
        a.play_note(0, 49, 100);
        let mut buf = [0u8; 512];
        k.render_audio(&mut buf);
        assert!(buf.iter().any(|&b| b != 128));
        assert_eq!(a.active_voice_count(), 1);
        a.stop_note(0);
        k.render_audio(&mut buf);
        assert_eq!(a.active_voice_count(), 0);
    }

    #[test]
    fn volume_and_filter_and_arp_and_ring() {
        let (k, rx) = kernel_with_parked_apps();
        let a = api_at(&k, &rx, 0, 30, 10, 10);
        a.set_volume(40);
        assert_eq!(a.volume(), 40);
        a.configure_filter(60, 12, 7);
        a.trigger_arp(1, [40, 44, 47, 52], 4, 30);
        a.set_ring_partner(2, 3);
        a.set_ring_partner(2, -1);
        a.play_note(1, 40, 80);
        let mut buf = [0u8; 256];
        k.render_audio(&mut buf);
        assert_eq!(a.active_voice_count(), 1);
    }

    #[test]
    fn notes_belong_to_the_calling_app() {
        let (k, rx) = kernel_with_parked_apps();
        let a = api_at(&k, &rx, 0, 30, 10, 10);
        a.play_note(0, 49, 100);
        k.audio_release_owner(a.context().task);
        let mut buf = [0u8; 64];
        k.render_audio(&mut buf);
        assert_eq!(a.active_voice_count(), 0, "release by owner stopped it");
    }

    #[test]
    fn text_draws_at_the_windows_scale_and_chrome_stays_normal() {
        let (_k, a) = spawn(200, 100);
        assert_eq!((a.font_size(), a.window_size(), a.font_scale()), ((6, 8), (200, 100), 1));
        a.set_font_scale(2);
        assert_eq!(a.font_scale(), 2, "the setting changed");
        assert_eq!(a.font_size(), (6, 8), "but this window keeps the scale it opened with");

        let root = std::env::temp_dir().join(format!("acid-api-font-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("v3/apps")).unwrap();
        std::fs::write(root.join("v3/apps/t.lua"), "-- t").unwrap();
        std::fs::write(root.join("v3/apps/t.app.toml"), "font = scalable\n").unwrap();
        let (_k2, b) = spawn_scaled_on(FakePlatform::new(root.clone()), "v3/apps/t.lua", 200, 100, 2);
        let _ = std::fs::remove_dir_all(&root);
        assert_eq!(b.font_size(), (12, 16));
        let (_k1, one) = spawn(200, 100);
        let (fg, bg) = (0xFFFFFF_u32, 0x000000_u32);
        one.draw_text("A", 0, 0, fg, bg);
        b.draw_text("A", 0, 20, fg, bg);
        let mut lit_seen = false;
        for gy in 0..8 {
            for gx in 0..6 {
                let want = px(&one, gx, gy);
                lit_seen |= want == rgb565(fg);
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    assert_eq!(px(&b, gx * 2 + dx, 20 + gy * 2 + dy), want, "glyph pixel ({gx}, {gy}) is a 2x2 block");
                }
            }
        }
        assert!(lit_seen);
        b.draw_window_frame("T");
        let (w, _) = b.window_size();
        let text_rows: Vec<i32> = (0..16).filter(|&y| (0..w - 20).any(|x| px(&b, x, y) == rgb565(THEME_TEXT))).collect();
        assert!(!text_rows.is_empty() && text_rows.iter().all(|y| (4..12).contains(y)), "title at scale 1: {text_rows:?}");
    }

    #[test]
    fn screen_size_is_the_kernels() {
        let (k, a) = spawn(100, 100);
        assert_eq!(a.screen_size(), (k.screen().w, k.screen().h));
    }

    fn dot(api: &KernelApi, x: i32, y: i32) -> bool {
        px(api, x, y) != rgb565(0)
    }

    #[test]
    fn line_and_triangle_paint_the_window() {
        let (_k, api) = spawn(20, 20);
        api.draw_line(0, 5, 9, 5, 0xFF0000);
        assert_eq!(px(&api, 4, 5), rgb565(0xFF0000));
        api.fill_triangle(2, 10, 12, 10, 2, 19, 0x00FF00);
        assert_eq!(px(&api, 3, 12), rgb565(0x00FF00));
    }

    #[test]
    fn every_builtin_gets_a_distinct_increasing_id() {
        let (_k, api) = spawn(10, 10);
        let ids: Vec<i32> = three_d::BUILTIN_NAMES.iter().map(|n| api.mesh_builtin(n).unwrap()).collect();
        assert_eq!(ids, [1, 2, 3, 4, 5]);
        assert_eq!(api.mesh_builtin("nope"), Err("unknown".into()));
    }

    #[test]
    fn the_seventeenth_mesh_is_too_big() {
        let (_k, api) = spawn(10, 10);
        for _ in 0..MESH_MAX {
            api.mesh_builtin("cube").unwrap();
        }
        assert_eq!(api.mesh_builtin("cube"), Err("too big".into()));
        api.mesh_free(3);
        assert_eq!(api.mesh_builtin("cube"), Ok(17), "a freed slot is usable but its id is not reused");
    }

    #[test]
    fn the_last_id_is_too_big_and_ids_are_never_reused() {
        let mut store = MeshStore::new();
        store.next_id = i32::MAX - 1;
        let cube = || three_d::builtin("cube").unwrap();
        assert_eq!(store.add(cube()), Ok(i32::MAX - 1));
        assert_eq!(store.add(cube()), Err("too big".into()), "no id after i32::MAX - 1");
        store.map.clear();
        store.total_points = 0;
        assert_eq!(store.add(cube()), Err("too big".into()), "freeing never brings an id back");
    }

    #[test]
    fn a_full_store_builds_nothing() {
        let builds = || MESH_BUILDS.with(|c| c.get());
        let (_k, api) = spawn(10, 10);
        for _ in 0..MESH_MAX {
            api.mesh_builtin("cube").unwrap();
        }
        let before = builds();
        let (p, f) = big(512);
        assert_eq!(api.mesh_new(p, f), Err("too big".into()));
        assert_eq!(api.mesh_builtin("cube"), Err("too big".into()));
        assert_eq!(builds(), before, "no mesh is built once the count limit is hit");
        // The same for the points limit: 8 x 512 fills it.
        let (_k2, api) = spawn(10, 10);
        for _ in 0..8 {
            let (p, f) = big(512);
            api.mesh_new(p, f).unwrap();
        }
        let before = builds();
        let (p, f) = big(3);
        assert_eq!(api.mesh_new(p, f), Err("too big".into()));
        assert_eq!(builds(), before, "no mesh is built over the points limit");
    }

    #[test]
    fn a_near_full_store_rejects_a_builtin_without_building_it() {
        let builds = || MESH_BUILDS.with(|c| c.get());
        let (_k, api) = spawn(10, 10);
        for _ in 0..7 {
            let (p, f) = big(512);
            api.mesh_new(p, f).unwrap();
        }
        let (p, f) = big(500);
        api.mesh_new(p, f).unwrap(); // 4084 points: 12 free
        let before = builds();
        assert_eq!(api.mesh_builtin("torus"), Err("too big".into()), "72 points do not fit");
        assert_eq!(builds(), before);
        assert!(api.mesh_builtin("cube").is_ok(), "8 points do");
    }

    #[test]
    fn an_unknown_builtin_is_unknown_even_on_a_full_store() {
        let (_k, api) = spawn(10, 10);
        for _ in 0..MESH_MAX {
            api.mesh_builtin("cube").unwrap();
        }
        assert_eq!(api.mesh_builtin("nope"), Err("unknown".into()));
    }

    fn big(n: usize) -> (Vec<(i32, i32, i32)>, Vec<[u16; 4]>) {
        ((0..n).map(|i| (i as i32, 0, 0)).collect(), vec![[0, 1, 2, three_d::NO_INDEX]])
    }

    #[test]
    fn total_points_are_capped_at_4096_and_free_gives_them_back() {
        let (_k, api) = spawn(10, 10);
        let mut ids = Vec::new();
        for _ in 0..8 {
            let (p, f) = big(512);
            ids.push(api.mesh_new(p, f).unwrap());
        }
        let (p, f) = big(3);
        assert_eq!(api.mesh_new(p, f), Err("too big".into()), "4096 + 3 > 4096");
        api.mesh_free(ids[0]);
        let (p, f) = big(512);
        assert_eq!(api.mesh_new(p, f), Ok(9), "the freed 512 points fit again");
        let (p, f) = big(1);
        assert_eq!(api.mesh_new(p, f), Err("too big".into()), "the store is full, so the limits are checked first");
        api.mesh_free(ids[1]);
        api.mesh_free(ids[1]);
        let (p, f) = big(512);
        assert!(api.mesh_new(p, f).is_ok(), "a double free does not double-credit");
        let (p, f) = big(3);
        assert_eq!(api.mesh_new(p, f), Err("too big".into()));
    }

    #[test]
    fn mesh_draw_paints_and_an_unknown_id_draws_nothing() {
        let (_k, api) = spawn(40, 40);
        api.mesh_draw(99, 20, 20, 64, 0, 0, 0, 1, 0xFFFFFF);
        assert!((0..40).all(|y| (0..40).all(|x| !dot(&api, x, y))), "unknown id");
        let id = api.mesh_builtin("cube").unwrap();
        // At size 640 the cube's near face fills the whole 40x40 canvas.
        api.mesh_draw(id, 20, 20, 640, 10, 20, 0, 1, 0xFFFFFF);
        assert!((0..40).all(|y| (0..40).all(|x| dot(&api, x, y))), "a big cube covers the canvas");
        api.mesh_free(id);
        api.mesh_free(id);
        api.mesh_draw(id, 20, 20, 8, 10, 20, 0, 1, 0x0000FF);
        assert!((0..40).all(|y| (0..40).all(|x| px(&api, x, y) != rgb565(0x0000FF))), "freed id");
    }

    #[test]
    fn mesh_draw_cost_is_zero_for_an_unknown_id_and_positive_for_a_mesh() {
        let (_k, api) = spawn(10, 10);
        assert_eq!(api.mesh_draw_cost(7, 5, 5, 64, 0, 0, 0, 2), 0);
        let id = api.mesh_builtin("cube").unwrap();
        assert!(api.mesh_draw_cost(id, 5, 5, 64, 0, 0, 0, 2) >= 64 * 8);
    }
}
