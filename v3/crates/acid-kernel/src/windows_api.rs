//! Window queries and control, and launching -- the kernel side of the
//! window bindings. A separate `impl Kernel` block, so kernel.rs keeps to
//! the core spawn/composite/tick loop.

use alloc::string::String;
use alloc::sync::Arc;

use crate::kernel::{Kernel, SpawnRequest};
use crate::launcher::LaunchableApp;
use crate::placement::cascade_position;
use crate::TaskId;

/// One window as acid_window_info reports it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WindowInfo {
    pub app_name: String,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub focused: bool,
}

impl Kernel {
    pub fn window_info(&self, index: usize) -> Option<WindowInfo> {
        self.with_state(|st| {
            let w = st.windows.at_index(index)?;
            Some(WindowInfo {
                app_name: w.app_name.clone(),
                x: w.x,
                y: w.y,
                w: w.w,
                h: w.h,
                focused: st.router.focus == Some(w.task),
            })
        })
    }

    fn task_at(&self, index: usize) -> Option<TaskId> {
        self.with_state(|st| st.windows.at_index(index).map(|w| w.task))
    }

    /// Whether `task` still has a registered window. Closing a window
    /// unregisters it at once, even if its app keeps running.
    pub fn has_window(&self, task: TaskId) -> bool {
        self.with_state(|st| st.windows.by_task(task).is_some())
    }

    /// The task whose window sits at `index`.
    pub fn task_at_index(&self, index: usize) -> Option<TaskId> {
        self.task_at(index)
    }

    /// The taskbar's tap-to-focus.
    pub fn activate_index(&self, index: usize) {
        if let Some(t) = self.task_at(index) {
            self.activate_window(t);
        }
    }

    /// Closes another app's window. An app can't
    /// close itself this way; it ends its own run loop instead.
    pub fn close_index(&self, index: usize, caller: TaskId) -> bool {
        match self.task_at(index) {
            Some(t) if t != caller => {
                self.close_window(t);
                true
            }
            _ => false,
        }
    }

    pub fn send_to_back(&self, task: TaskId) {
        if self.with_state(|st| st.windows.send_to_back(task)) {
            self.mark_dirty();
        }
    }

    pub fn launcher_register(&self, app: LaunchableApp) -> bool {
        if !crate::layout::window_size_ok(app.w, app.h) {
            return false;
        }
        self.launcher.lock().register(app)
    }

    pub fn launcher_count(&self) -> usize {
        self.launcher.lock().count()
    }

    pub fn launcher_entry(&self, index: usize) -> Option<LaunchableApp> {
        self.launcher.lock().get(index).cloned()
    }

    /// Menu launch. `by_cart` is true when a
    /// cart-level app asks (spec §16.2): "raise the open singleton" becomes
    /// a refusal, the new app runs cart-level, and the spawn is refused once
    /// CART_WINDOW_MAX cart-level windows are open.
    pub fn launch(self: &Arc<Self>, index: usize, by_cart: bool) -> bool {
        let Some(app) = self.launcher_entry(index) else { return false };
        self.spawn_or_activate(&app.path, app.w, app.h, None, app.libs, app.multi, by_cart)
    }

    /// Launch by path, e.g. File Manager opening Editor on a file. multi
    /// and libs come from the launcher entry with this path. An
    /// unregistered path is single-instance with no libs.
    /// `by_cart` as for [`Kernel::launch`].
    pub fn spawn_by_path(
        self: &Arc<Self>,
        path: &str,
        w: i32,
        h: i32,
        arg: Option<String>,
        by_cart: bool,
    ) -> bool {
        let (multi, libs) = self
            .launcher
            .lock()
            .by_path(path)
            .map(|a| (a.multi, a.libs.clone()))
            .unwrap_or((false, None));
        self.spawn_or_activate(path, w, h, arg, libs, multi, by_cart)
    }

    /// The shared rule: a running single-instance app is activated instead
    /// of spawned again; otherwise spawn at the next cascade slot, closable,
    /// and activate the new window. When `by_cart` is true, a running
    /// single-instance app (any copy) is left alone and the call returns
    /// false. A built-in caller looks only at copies that are not
    /// cart-level, so it never raises a cart-started copy: it gets a
    /// trusted window of its own instead (spec §16.2).
    #[allow(clippy::too_many_arguments)]
    fn spawn_or_activate(
        self: &Arc<Self>,
        path: &str,
        w: i32,
        h: i32,
        arg: Option<String>,
        libs: Option<String>,
        multi: bool,
        by_cart: bool,
    ) -> bool {
        let open = self.with_state(|st| {
            if by_cart { st.windows.find_by_app_name(path) } else { st.windows.find_trusted_by_app_name(path) }
        });
        if !multi && let Some(t) = open {
            if by_cart {
                return false;
            }
            self.activate_window(t);
            return true;
        }
        let (x, y) = cascade_position(self.with_state(|st| st.windows.count()), w, h);
        match self.spawn_app(SpawnRequest { script_path: path.into(), x, y, w, h, closable: true, arg, libs, force_cart: by_cart }) {
            Some(t) => {
                self.activate_window(t);
                true
            }
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::launcher::LaunchableApp;
    use crate::test_support::*;

    fn entry(path: &str, multi: bool, libs: Option<&str>) -> LaunchableApp {
        LaunchableApp { path: path.into(), name: "App".into(), w: 200, h: 150, multi, libs: libs.map(Into::into) }
    }

    #[test]
    fn bad_window_sizes_are_refused_before_anything_is_allocated() {
        let (_p, k, _rx) = setup();
        for (w, h) in [(0, 10), (10, 0), (-5, 10), (641, 10), (10, 361)] {
            assert!(k.spawn_app(req(0, 30, w, h)).is_none(), "{w}x{h} spawn");
            let mut e = entry("v3/apps/a.lua", false, None);
            (e.w, e.h) = (w, h);
            assert!(!k.launcher_register(e), "{w}x{h} register");
        }
        assert_eq!(k.with_state(|st| st.windows.count()), 0);
        assert_eq!(k.launcher_count(), 0);
        assert!(k.spawn_app(req(0, 0, 640, 360)).is_some(), "the full screen is a valid size");
    }

    #[test]
    fn kernel_tasks_come_from_the_platform() {
        let (p, k, _rx) = setup();
        p.set_thread_samples(vec![acid_platform::ThreadSample { id: 9, name: "router".into(), state: 'S', cpu_ms: 0 }]);
        assert_eq!(k.refresh_tasks(), 1);
        assert_eq!(k.task_count(), 1);
        assert_eq!(k.task_info(0).unwrap().state, "blocked");
        assert!(k.task_info(1).is_none());
    }

    #[test]
    fn window_info_reports_slots_and_focus() {
        let (_p, k, rx) = setup();
        let t = k.spawn_app(req(5, 30, 40, 20)).unwrap();
        recv(&rx);
        assert_eq!(
            k.window_info(0),
            Some(WindowInfo { app_name: "test".into(), x: 5, y: 30, w: 40, h: 20, focused: false })
        );
        k.activate_window(t);
        assert!(k.window_info(0).unwrap().focused);
        assert_eq!(k.window_info(1), None);
        assert_eq!(k.window_info(99), None);
    }

    #[test]
    fn activate_and_close_by_index() {
        let (_p, k, rx) = setup();
        let a = k.spawn_app(req(0, 30, 10, 10)).unwrap();
        recv(&rx);
        let b = k.spawn_app(req(20, 30, 10, 10)).unwrap();
        recv(&rx);
        k.activate_index(0);
        assert_eq!(k.focus(), Some(a));
        assert!(!k.close_index(1, b), "an app can't close itself this way");
        assert!(!k.close_index(5, a), "empty slot");
        assert!(k.close_index(1, a));
        assert_eq!(k.with_state(|st| st.windows.count()), 1);
    }

    #[test]
    fn send_to_back_marks_dirty() {
        let (_p, k, rx) = setup();
        let a = k.spawn_app(req(0, 30, 10, 10)).unwrap();
        recv(&rx);
        k.take_dirty();
        k.send_to_back(a);
        assert!(k.take_dirty());
    }

    #[test]
    fn launcher_round_trip() {
        let (_p, k, _rx) = setup();
        assert!(k.launcher_register(entry("v3/apps/a.lua", false, Some("lib/x.lua"))));
        assert_eq!(k.launcher_count(), 1);
        assert_eq!(k.launcher_entry(0).unwrap().libs.as_deref(), Some("lib/x.lua"));
        assert!(k.launcher_entry(1).is_none());
    }

    #[test]
    fn launch_spawns_at_cascade_with_libs_and_activates() {
        let (_p, k, rx) = setup();
        k.launcher_register(entry("v3/apps/a.lua", false, Some("lib/x.lua")));
        assert!(k.launch(0, false));
        let ctx = recv(&rx);
        assert_eq!((ctx.x, ctx.y), (20, 34), "first window's cascade slot");
        assert_eq!(ctx.libs.as_deref(), Some("lib/x.lua"));
        assert_eq!(ctx.arg, None);
        assert_eq!(k.focus(), Some(ctx.task));
        assert!(!k.launch(7, false), "no such entry");
    }

    #[test]
    fn a_running_singleton_is_activated_not_respawned() {
        let (_p, k, rx) = setup();
        k.launcher_register(entry("v3/apps/a.lua", false, None));
        k.launch(0, false);
        let first = recv(&rx).task;
        let other = k.spawn_app(req(300, 30, 10, 10)).unwrap();
        recv(&rx);
        k.activate_window(other);
        assert!(k.launch(0, false));
        assert_eq!(k.with_state(|st| st.windows.count()), 2);
        assert_eq!(k.focus(), Some(first));
    }

    #[test]
    fn an_open_singleton_is_not_raised_when_raising_is_forbidden() {
        let (_p, k, rx) = setup();
        k.launcher_register(entry("v3/apps/a.lua", false, None));
        assert!(k.launch(0, false));
        let first = recv(&rx).task;
        let other = k.spawn_app(req(300, 30, 10, 10)).unwrap();
        recv(&rx);
        k.activate_window(other);
        let z = |k: &Kernel| k.with_state(|st| st.windows.in_z_order().iter().map(|w| w.task).collect::<alloc::vec::Vec<_>>());
        let before = z(&k);
        assert!(!k.launch(0, true), "launch: open singleton, a cart may not raise it");
        assert!(!k.spawn_by_path("v3/apps/a.lua", 200, 150, None, true), "by path: same");
        assert_eq!(k.focus(), Some(other), "focus unchanged");
        assert_eq!(z(&k), before, "z-order unchanged");
        assert_eq!(k.with_state(|st| st.windows.count()), 2, "nothing spawned");
        // A cart still gets a new window (spawned and focused).
        assert!(k.spawn_by_path("v3/apps/b.lua", 100, 80, None, true));
        let b = recv(&rx).task;
        assert_eq!(k.focus(), Some(b));
        // A built-in caller still raises it.
        assert!(k.spawn_by_path("v3/apps/a.lua", 200, 150, None, false));
        assert_eq!(k.focus(), Some(first));
    }

    #[test]
    fn an_app_a_cart_starts_runs_cart_level() {
        let (_p, k, rx) = setup();
        assert!(k.spawn_by_path("v3/apps/editor.lua", 100, 80, None, true));
        assert!(recv(&rx).cart, "spawn_by_path from a cart");
        k.launcher_register(entry("v3/apps/file_manager.lua", false, None));
        assert!(k.launch(0, true));
        assert!(recv(&rx).cart, "launch from a cart");
        assert!(k.spawn_by_path("v3/apps/viewer.lua", 100, 80, None, false));
        assert!(!recv(&rx).cart, "a built-in caller's app stays built-in");
    }

    #[test]
    fn a_cart_may_not_spawn_once_cart_window_max_cart_windows_are_open() {
        use crate::layout::CART_WINDOW_MAX;
        let (_p, k, rx) = setup();
        let count = |k: &Kernel| k.with_state(|st| st.windows.count());
        k.launcher_register(entry("v3/apps/m.lua", true, None));
        let mut carts = alloc::vec::Vec::new();
        for i in 0..CART_WINDOW_MAX {
            assert!(k.launch(0, true), "cart window {i} of {CART_WINDOW_MAX}");
            carts.push(recv(&rx).task);
        }
        assert_eq!(count(&k), CART_WINDOW_MAX);
        assert!(!k.launch(0, true), "launch: the cap is reached");
        assert!(!k.spawn_by_path("v3/apps/x.lua", 100, 80, None, true), "by path: same");
        assert_eq!(count(&k), CART_WINDOW_MAX, "nothing spawned");
        // Built-in callers are never refused by the cap, even for a cart.
        assert!(k.spawn_by_path("v3/fsroot/Home/c.lua", 100, 80, None, false));
        assert!(recv(&rx).cart);
        assert!(k.launch(0, false));
        assert!(!recv(&rx).cart);
        assert_eq!(count(&k), CART_WINDOW_MAX + 2);
        assert!(!k.launch(0, true), "five cart windows: still refused");
        // Built-in windows don't count: closing one cart window and the
        // Load-Cart-style one leaves CART_WINDOW_MAX - 1 cart windows.
        let home = k.with_state(|st| st.windows.find_by_app_name("v3/fsroot/Home/c.lua")).unwrap();
        for t in [carts[0], home] {
            k.close_window(t);
        }
        wait_until(|| count(&k) == CART_WINDOW_MAX);
        assert!(k.launch(0, true), "below the cap again");
        assert!(recv(&rx).cart);
        assert!(!k.launch(0, true), "the new app counts as cart-level");
    }

    #[test]
    fn a_trusted_launch_skips_a_cart_started_copy_of_a_singleton() {
        let (_p, k, rx) = setup();
        let count = |k: &Kernel| k.with_state(|st| st.windows.count());
        k.launcher_register(entry("v3/apps/config.lua", false, None));
        assert!(k.launch(0, true));
        let tainted = recv(&rx);
        assert!(tainted.cart);
        // A built-in launch opens a trusted window instead of raising the
        // tainted one.
        assert!(k.launch(0, false));
        let trusted = recv(&rx);
        assert!(!trusted.cart);
        assert_eq!(count(&k), 2);
        assert_eq!(k.focus(), Some(trusted.task));
        // Now there is a trusted one: built-in launches raise it, by index and by path.
        k.activate_window(tainted.task);
        assert!(k.launch(0, false));
        assert_eq!(k.focus(), Some(trusted.task));
        k.activate_window(tainted.task);
        assert!(k.spawn_by_path("v3/apps/config.lua", 200, 150, None, false));
        assert_eq!(k.focus(), Some(trusted.task));
        assert_eq!(count(&k), 2, "nothing more spawned");
        // A cart still finds the singleton open (either copy) and is refused.
        assert!(!k.launch(0, true));
        assert_eq!(count(&k), 2);
    }

    #[test]
    fn a_multi_app_spawns_again() {
        let (_p, k, rx) = setup();
        k.launcher_register(entry("v3/apps/t.lua", true, None));
        k.launch(0, false);
        recv(&rx);
        k.launch(0, false);
        let second = recv(&rx);
        assert_eq!((second.x, second.y), (38, 52));
        assert_eq!(k.with_state(|st| st.windows.count()), 2);
    }

    #[test]
    fn spawn_by_path_passes_arg_and_treats_unregistered_as_singleton() {
        let (_p, k, rx) = setup();
        assert!(k.spawn_by_path("v3/apps/e.lua", 100, 80, Some("v3/fsroot/Home/notes.txt".into()), false));
        let ctx = recv(&rx);
        assert_eq!(ctx.arg.as_deref(), Some("v3/fsroot/Home/notes.txt"));
        assert_eq!(ctx.libs, None);
        assert!(k.spawn_by_path("v3/apps/e.lua", 100, 80, None, false));
        assert_eq!(k.with_state(|st| st.windows.count()), 1);
    }

    #[test]
    fn spawn_by_path_honours_a_registered_entry() {
        let (_p, k, rx) = setup();
        k.launcher_register(entry("v3/apps/m.lua", true, Some("lib/x.lua")));
        assert!(k.spawn_by_path("v3/apps/m.lua", 100, 80, None, false));
        let first = recv(&rx);
        assert!(k.spawn_by_path("v3/apps/m.lua", 100, 80, None, false));
        assert_eq!(k.with_state(|st| st.windows.count()), 2, "multi: spawned again");
        assert_eq!(first.libs.as_deref(), Some("lib/x.lua"));
    }
}
