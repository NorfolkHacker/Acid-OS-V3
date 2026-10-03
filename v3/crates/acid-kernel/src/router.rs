//! The input router. Pure logic over KernelState so every rule is
//! unit-testable; Kernel (kernel.rs) feeds it real input once per 16 ms
//! tick.

use core::sync::atomic::{AtomicBool, Ordering};

use acid_platform::TouchState;

use crate::TaskId;
use crate::event::{Event, EventQueue};
use crate::layout::{CLOSE_BTN_MARGIN, CLOSE_BTN_R, DESKTOP_STRIP_H, TITLE_BAR_H};
use crate::window::WindowRegistry;

#[derive(Debug, Clone, Copy)]
struct Drag {
    task: TaskId,
    off_x: i32,
    off_y: i32,
}

#[derive(Default)]
pub struct RouterState {
    drag: Option<Drag>,
    was_pressed: bool,
    /// Owns the top strip unconditionally.
    pub desktop: Option<TaskId>,
    /// The window a gesture started on keeps it until release.
    touch_task: Option<TaskId>,
    last_x: i32,
    last_y: i32,
    /// Keyboard focus. Only activate_window gains it, and that also raises
    /// the window; a newly registered window is in front but unfocused.
    pub focus: Option<TaskId>,
}

/// Everything behind the one kernel lock.
pub struct KernelState {
    pub windows: WindowRegistry,
    pub router: RouterState,
}

impl Default for KernelState {
    fn default() -> Self {
        Self::new()
    }
}

impl KernelState {
    pub fn new() -> Self {
        Self { windows: WindowRegistry::new(), router: RouterState::default() }
    }

    fn send(&self, task: TaskId, ev: Event) {
        if let Some(w) = self.windows.by_task(task) {
            send(&w.queue, ev);
        }
    }
}

fn send(q: &EventQueue, ev: Event) {
    // Best effort: a full queue drops the event.
    let _ = q.send(ev);
}

fn mark(dirty: &AtomicBool) {
    dirty.store(true, Ordering::SeqCst);
}

/// Raise to front AND take keyboard focus, in one step -- the single rule
/// for how a window gains focus. Sets focus even if `task` has no window.
pub fn activate_window(st: &mut KernelState, task: TaskId, dirty: &AtomicBool) {
    if st.windows.bring_to_front(task) {
        mark(dirty);
    }
    st.router.focus = Some(task);
}

/// Sends Close, unregisters, and drops focus/touch/drag state pointing at it.
pub fn close_window(st: &mut KernelState, task: TaskId, dirty: &AtomicBool) {
    let Some(win) = st.windows.unregister(task) else { return };
    send(&win.queue, Event::Close);
    mark(dirty);
    forget_task(st, task);
}

/// Test seam: seeds the touch owner, which has no public setter.
#[cfg(test)]
pub(crate) fn set_touch_task_for_test(st: &mut KernelState, task: Option<TaskId>) {
    st.router.touch_task = task;
}

/// Test seam: reads the touch owner.
#[cfg(test)]
pub(crate) fn touch_task_for_test(st: &KernelState) -> Option<TaskId> {
    st.router.touch_task
}

/// Drops every router reference to `task`: focus, the touch owner and a
/// drag in progress. For a task whose window is already gone (spawn
/// rollback, app exit), so no state keeps pointing at a dead id. Clearing
/// the touch owner and drag here, rather than leaving a later poll to
/// notice the missing window, makes the desktop strip usable again at once
/// instead of after the dead gesture's release.
pub fn forget_task(st: &mut KernelState, task: TaskId) {
    let r = &mut st.router;
    if r.focus == Some(task) {
        r.focus = None;
    }
    if r.touch_task == Some(task) {
        r.touch_task = None;
    }
    if r.drag.is_some_and(|d| d.task == task) {
        r.drag = None;
    }
}

pub fn poll(st: &mut KernelState, key: Option<i32>, touch: TouchState, dirty: &AtomicBool) {
    if let (Some(code), Some(focus)) = (key, st.router.focus) {
        st.send(focus, Event::Key { code });
    }

    let TouchState { x, y, pressed } = touch;
    if pressed {
        st.router.last_x = x;
        st.router.last_y = y;
    }
    let fresh_press = pressed && !st.router.was_pressed;
    let fresh_release = !pressed && st.router.was_pressed;
    st.router.was_pressed = pressed;
    let (last_x, last_y) = (st.router.last_x, st.router.last_y);

    // A gesture that started in a window keeps going to that window even over the strip (touch_task check); otherwise the strip always belongs to the desktop, unconditionally.
    if let Some(desktop) = st.router.desktop {
        if st.router.drag.is_none() && st.router.touch_task.is_none() && last_y < DESKTOP_STRIP_H {
            if fresh_press || pressed || fresh_release {
                if let Some(w) = st.windows.by_task(desktop) {
                    send(&w.queue, Event::Touch { x: last_x - w.x, y: last_y - w.y, pressed });
                }
            }
            return;
        }
    }

    if let Some(drag) = st.router.drag {
        match st.windows.by_task_mut(drag.task) {
            Some(w) if pressed => {
                w.x = x - drag.off_x;
                // Never let a window's title bar go under the strip: the strip claims every press there, so a window parked under it could never be dragged back.
                w.y = (y - drag.off_y).max(DESKTOP_STRIP_H);
                mark(dirty);
            }
            _ => st.router.drag = None,
        }
        return;
    }

    if fresh_press {
        let Some(task) = st.windows.find_at(x, y) else { return };
        let (rel_x, rel_y, win_w, closable) = {
            let w = st.windows.by_task(task).expect("find_at returned a live window");
            (x - w.x, y - w.y, w.w, w.closable)
        };
        if rel_y < TITLE_BAR_H {
            if closable {
                let dx = rel_x - (win_w - CLOSE_BTN_MARGIN);
                let dy = rel_y - TITLE_BAR_H / 2;
                let hit_r = CLOSE_BTN_R + 3; // a little forgiveness for touch
                if dx * dx + dy * dy <= hit_r * hit_r {
                    close_window(st, task, dirty);
                    return;
                }
            }
            activate_window(st, task, dirty);
            st.router.drag = Some(Drag { task, off_x: rel_x, off_y: rel_y });
            return;
        }
        activate_window(st, task, dirty);
        st.router.touch_task = Some(task);
        st.send(task, Event::Touch { x: rel_x, y: rel_y, pressed: true });
        return;
    }

    // Deliberately not find_at: a held touch goes to the window the gesture started on, wherever it is now.
    if pressed {
        if let Some(task) = st.router.touch_task {
            if let Some(w) = st.windows.by_task(task) {
                send(&w.queue, Event::Touch { x: x - w.x, y: y - w.y, pressed: true });
            }
        }
        return;
    }

    if fresh_release {
        if let Some(task) = st.router.touch_task {
            if let Some(w) = st.windows.by_task(task) {
                send(&w.queue, Event::Touch { x: last_x - w.x, y: last_y - w.y, pressed: false });
            }
        }
        st.router.touch_task = None;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::window::Window;
    use acid_gfx::Canvas;
    use acid_platform::sync::Mutex;
    use acid_testkit::StdSignal;
    use alloc::sync::Arc;

    fn add(st: &mut KernelState, task: u32, x: i32, y: i32, w: i32, h: i32, closable: bool) -> Arc<EventQueue> {
        let q = Arc::new(EventQueue::new(Arc::new(StdSignal::new())));
        let win = Window::new(TaskId(task), q.clone(), Arc::new(Mutex::new(Canvas::new(w, h))), "t".into(), x, y, w, h, closable);
        assert!(st.windows.register(win));
        q
    }

    fn touch(x: i32, y: i32, pressed: bool) -> TouchState {
        TouchState { x, y, pressed }
    }

    fn drain(q: &EventQueue) -> Vec<Event> {
        let mut v = Vec::new();
        while let Some(e) = q.try_recv() {
            v.push(e);
        }
        v
    }

    const UP: TouchState = TouchState { x: 0, y: 0, pressed: false };

    #[test]
    fn desktop_strip_is_relative_to_a_desktop_not_at_origin() {
        let mut st = KernelState::new();
        let d = AtomicBool::new(false);
        let qd = add(&mut st, 1, 10, 0, 600, 24, false);
        st.router.desktop = Some(TaskId(1));
        poll(&mut st, None, touch(100, 10, true), &d);
        assert_eq!(drain(&qd), [Event::Touch { x: 90, y: 10, pressed: true }]);
    }

    #[test]
    fn desktop_set_without_a_window_swallows_strip_touches() {
        // The strip branch returns even when the desktop task has no window,
        // so a window under the strip never sees those touches.
        let mut st = KernelState::new();
        let d = AtomicBool::new(false);
        st.router.desktop = Some(TaskId(9));
        let qa = add(&mut st, 1, 0, 0, 100, 100, true);
        poll(&mut st, None, touch(50, 10, true), &d);
        assert!(drain(&qa).is_empty());
        assert_eq!(st.router.focus, None);
    }

    #[test]
    fn window_vanishing_mid_drag_ends_the_drag() {
        let mut st = KernelState::new();
        let d = AtomicBool::new(false);
        add(&mut st, 1, 100, 100, 200, 150, true);
        let qb = add(&mut st, 2, 100, 300, 50, 50, true);
        poll(&mut st, None, touch(110, 105, true), &d); // title bar: drag
        st.windows.unregister(TaskId(1));
        poll(&mut st, None, touch(120, 310, true), &d); // drag branch ends it
        poll(&mut st, None, touch(120, 320, true), &d); // held, no owner
        poll(&mut st, None, UP, &d);
        assert!(drain(&qb).is_empty());
    }

    #[test]
    fn closing_the_touch_owner_stops_its_gesture() {
        let mut st = KernelState::new();
        let d = AtomicBool::new(false);
        let qa = add(&mut st, 1, 100, 100, 200, 150, true);
        let qb = add(&mut st, 2, 400, 100, 100, 100, true);
        poll(&mut st, None, touch(150, 140, true), &d);
        close_window(&mut st, TaskId(1), &d);
        poll(&mut st, None, touch(450, 140, true), &d);
        poll(&mut st, None, UP, &d);
        assert_eq!(drain(&qa), [Event::Touch { x: 50, y: 40, pressed: true }, Event::Close]);
        assert!(drain(&qb).is_empty());
    }

    #[test]
    fn activate_and_close_mark_dirty_only_when_a_window_changes() {
        let mut st = KernelState::new();
        let d = AtomicBool::new(false);
        add(&mut st, 1, 0, 30, 10, 10, true);
        activate_window(&mut st, TaskId(1), &d);
        assert!(d.swap(false, Ordering::SeqCst));
        close_window(&mut st, TaskId(1), &d);
        assert!(d.swap(false, Ordering::SeqCst));
        activate_window(&mut st, TaskId(9), &d);
        assert!(!d.load(Ordering::SeqCst), "no window raised, nothing to redraw");
    }

    #[test]
    fn forget_task_clears_focus_touch_and_drag() {
        let mut st = KernelState::new();
        let d = AtomicBool::new(false);
        add(&mut st, 1, 100, 100, 200, 150, true);
        let qb = add(&mut st, 2, 400, 100, 100, 100, true);
        poll(&mut st, None, touch(110, 105, true), &d); // drag on 1, focus 1
        forget_task(&mut st, TaskId(1));
        assert_eq!(st.router.focus, None);
        poll(&mut st, None, touch(200, 200, true), &d);
        assert_eq!(st.windows.by_task(TaskId(1)).unwrap().x, 100, "drag is gone");
        poll(&mut st, None, UP, &d);
        poll(&mut st, None, touch(450, 140, true), &d); // owner 2
        forget_task(&mut st, TaskId(2));
        poll(&mut st, None, UP, &d);
        assert_eq!(drain(&qb), [Event::Touch { x: 50, y: 40, pressed: true }], "no release for a forgotten owner");
    }

    #[test]
    fn key_goes_to_focused_window_only() {
        let mut st = KernelState::new();
        let d = AtomicBool::new(false);
        let qa = add(&mut st, 1, 0, 30, 100, 100, true);
        let qb = add(&mut st, 2, 200, 30, 100, 100, true);
        poll(&mut st, Some(65), UP, &d);
        assert!(drain(&qa).is_empty() && drain(&qb).is_empty(), "no focus yet: key dropped");
        activate_window(&mut st, TaskId(1), &d);
        poll(&mut st, Some(65), UP, &d);
        assert_eq!(drain(&qa), [Event::Key { code: 65 }]);
        assert!(drain(&qb).is_empty());
    }

    #[test]
    fn strip_touches_go_to_desktop_relative_to_its_window() {
        let mut st = KernelState::new();
        let d = AtomicBool::new(false);
        let qd = add(&mut st, 1, 0, 0, 640, 24, false);
        st.router.desktop = Some(TaskId(1));
        poll(&mut st, None, touch(100, 10, true), &d);
        poll(&mut st, None, touch(101, 11, true), &d);
        poll(&mut st, None, touch(0, 0, false), &d);
        poll(&mut st, None, touch(0, 0, false), &d);
        assert_eq!(
            drain(&qd),
            [
                Event::Touch { x: 100, y: 10, pressed: true },
                Event::Touch { x: 101, y: 11, pressed: true },
                Event::Touch { x: 101, y: 11, pressed: false },
            ]
        );
        assert_eq!(st.router.focus, None, "strip touches never take focus");
    }

    #[test]
    fn body_press_activates_owns_and_tracks_the_gesture() {
        let mut st = KernelState::new();
        let d = AtomicBool::new(false);
        let qa = add(&mut st, 1, 100, 100, 200, 150, true);
        let qb = add(&mut st, 2, 400, 100, 100, 100, true);
        poll(&mut st, None, touch(150, 140, true), &d);
        assert_eq!(st.router.focus, Some(TaskId(1)));
        assert_eq!(st.windows.find_at(150, 140), Some(TaskId(1)));
        // Held and moved: still window 1's, even outside it and over the strip.
        poll(&mut st, None, touch(450, 10, true), &d);
        poll(&mut st, None, touch(0, 0, false), &d);
        assert_eq!(
            drain(&qa),
            [
                Event::Touch { x: 50, y: 40, pressed: true },
                Event::Touch { x: 350, y: -90, pressed: true },
                Event::Touch { x: 350, y: -90, pressed: false },
            ]
        );
        assert!(drain(&qb).is_empty());
    }

    #[test]
    fn a_gesture_started_in_a_window_ignores_the_desktop_strip() {
        let mut st = KernelState::new();
        let d = AtomicBool::new(false);
        let qd = add(&mut st, 1, 0, 0, 640, 24, false);
        st.router.desktop = Some(TaskId(1));
        let qa = add(&mut st, 2, 100, 100, 200, 150, true);
        poll(&mut st, None, touch(150, 140, true), &d);
        poll(&mut st, None, touch(150, 5, true), &d);
        assert!(drain(&qd).is_empty());
        assert_eq!(drain(&qa).len(), 2);
    }

    #[test]
    fn title_bar_drag_moves_window_and_clamps_below_strip() {
        let mut st = KernelState::new();
        let d = AtomicBool::new(false);
        let qa = add(&mut st, 1, 100, 100, 200, 150, true);
        poll(&mut st, None, touch(110, 105, true), &d);
        assert_eq!(st.router.focus, Some(TaskId(1)));
        d.store(false, Ordering::SeqCst);
        poll(&mut st, None, touch(160, 135, true), &d);
        let w = st.windows.by_task(TaskId(1)).unwrap();
        assert_eq!((w.x, w.y), (150, 130));
        assert!(d.load(Ordering::SeqCst));
        poll(&mut st, None, touch(160, 0, true), &d);
        assert_eq!(st.windows.by_task(TaskId(1)).unwrap().y, DESKTOP_STRIP_H);
        poll(&mut st, None, UP, &d);
        // Drag over: the next press is an ordinary body press again.
        poll(&mut st, None, touch(200, 100, true), &d);
        assert_eq!(drain(&qa), [Event::Touch { x: 50, y: 76, pressed: true }]);
    }

    #[test]
    fn close_dot_closes_within_its_forgiving_radius() {
        let mut st = KernelState::new();
        let d = AtomicBool::new(false);
        let qa = add(&mut st, 1, 100, 100, 200, 150, true);
        activate_window(&mut st, TaskId(1), &d);
        // Close dot centre: (w - 8, 8) = window-relative (192, 8). Hit
        // radius CLOSE_BTN_R + 3 = 8: (292 - 8, 108) is exactly 8 away.
        poll(&mut st, None, touch(284, 108, true), &d);
        assert_eq!(drain(&qa), [Event::Close]);
        assert_eq!(st.windows.count(), 0);
        assert_eq!(st.router.focus, None);
    }

    #[test]
    fn just_outside_the_close_radius_starts_a_drag() {
        let mut st = KernelState::new();
        let d = AtomicBool::new(false);
        let qa = add(&mut st, 1, 100, 100, 200, 150, true);
        poll(&mut st, None, touch(283, 108, true), &d);
        poll(&mut st, None, touch(293, 118, true), &d);
        assert!(drain(&qa).is_empty());
        assert_eq!(st.windows.by_task(TaskId(1)).unwrap().x, 110);
    }

    #[test]
    fn non_closable_window_drags_from_its_dot() {
        let mut st = KernelState::new();
        let d = AtomicBool::new(false);
        add(&mut st, 1, 100, 100, 200, 150, false);
        poll(&mut st, None, touch(292, 108, true), &d);
        assert_eq!(st.windows.count(), 1);
        poll(&mut st, None, touch(302, 108, true), &d);
        assert_eq!(st.windows.by_task(TaskId(1)).unwrap().x, 110);
    }

    #[test]
    fn press_on_empty_screen_does_nothing() {
        let mut st = KernelState::new();
        let d = AtomicBool::new(false);
        let qa = add(&mut st, 1, 100, 100, 50, 50, true);
        poll(&mut st, None, touch(500, 300, true), &d);
        poll(&mut st, None, UP, &d);
        assert!(drain(&qa).is_empty());
        assert_eq!(st.router.focus, None);
    }
}
