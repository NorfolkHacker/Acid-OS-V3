//! The input router. Pure logic over KernelState so every rule is
//! unit-testable; Kernel (kernel.rs) feeds it real input once per 16 ms
//! tick.

use core::sync::atomic::{AtomicBool, Ordering};

use acid_platform::TouchState;

use crate::TaskId;
use crate::event::{Event, EventQueue};
use acid_gfx::Canvas;

use crate::layout::{CLOSE_BTN_MARGIN, CLOSE_BTN_R, DESKTOP_STRIP_H, RESIZE_GRIP, Screen, TITLE_BAR_H};
use crate::theme::THEME_BG;
use crate::window::{Window, WindowRegistry};

#[derive(Debug, Clone, Copy)]
struct Drag {
    task: TaskId,
    off_x: i32,
    off_y: i32,
}

/// A grip gesture in progress: where it began, the size it began at, and
/// the clamped size the pointer currently asks for (applied on release).
#[derive(Debug, Clone, Copy)]
struct Resize {
    task: TaskId,
    press_x: i32,
    press_y: i32,
    orig_w: i32,
    orig_h: i32,
    w: i32,
    h: i32,
}

#[derive(Default)]
pub struct RouterState {
    drag: Option<Drag>,
    resize: Option<Resize>,
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
    /// The screen the kernel runs at, for clamping resizes.
    pub screen: Screen,
}

impl Default for KernelState {
    fn default() -> Self {
        Self::new()
    }
}

impl KernelState {
    pub fn new() -> Self {
        Self::with_screen(Screen::DEFAULT)
    }

    pub fn with_screen(screen: Screen) -> Self {
        Self { windows: WindowRegistry::new(), router: RouterState::default(), screen }
    }

    fn send(&self, task: TaskId, ev: Event) {
        if let Some(w) = self.windows.by_task(task) {
            send(&w.queue, ev);
        }
    }
}

/// The size a resize to (w, h) actually gets: at least the window's
/// minimum (and, for large text, one character cell plus chrome), and at
/// most what fits on screen right of and below its top-left corner.
pub fn resize_clamp(win: &Window, screen: Screen, w: i32, h: i32) -> (i32, i32) {
    let s = win.font_scale.max(1);
    let min_w = win.min_w.max(6 * s + 2);
    let min_h = win.min_h.max(TITLE_BAR_H + 8 * s + 2);
    let max_w = (screen.w - win.x.max(0)).min(screen.w).max(min_w);
    let max_h = (screen.h - win.y.max(0)).min(screen.h).max(min_h);
    (w.clamp(min_w, max_w), h.clamp(min_h, max_h))
}

impl KernelState {
    /// The outline of an in-progress grip gesture: the window's top-left
    /// and the size it would get on release.
    pub fn resize_outline(&self) -> Option<(i32, i32, i32, i32)> {
        let rs = self.router.resize?;
        let win = self.windows.by_task(rs.task)?;
        Some((win.x, win.y, rs.w, rs.h))
    }

    /// Resizes a resizable window, clamped by `resize_clamp`: a new canvas
    /// with the old picture top-left and THEME_BG elsewhere is swapped in
    /// under the canvas lock (so the app's handle sees it), then the app
    /// gets Resized. None if the window isn't resizable or the size doesn't
    /// change.
    pub fn resize_window(&mut self, task: TaskId, w: i32, h: i32, dirty: &AtomicBool) -> Option<(i32, i32)> {
        let screen = self.screen;
        let win = self.windows.by_task_mut(task)?;
        if !win.resizable {
            return None;
        }
        let (w, h) = resize_clamp(win, screen, w, h);
        if (w, h) == (win.w, win.h) {
            return None;
        }
        {
            let mut canvas = win.canvas.lock();
            let mut next = Canvas::new(w, h);
            next.fill_rect(0, 0, w, h, THEME_BG);
            next.copy_rect_from(&canvas, 0, 0, w.min(canvas.width()), h.min(canvas.height()));
            *canvas = next;
        }
        win.w = w;
        win.h = h;
        send(&win.queue, Event::Resized { w, h });
        mark(dirty);
        Some((w, h))
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
/// drag or resize in progress. For a task whose window is already gone (spawn
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
    if r.resize.is_some_and(|g| g.task == task) {
        r.resize = None;
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
        if st.router.drag.is_none() && st.router.resize.is_none() && st.router.touch_task.is_none() && last_y < DESKTOP_STRIP_H {
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

    if let Some(rs) = st.router.resize {
        let screen = st.screen;
        let Some(w) = st.windows.by_task(rs.task) else {
            st.router.resize = None;
            return;
        };
        let (tw, th) = resize_clamp(w, screen, rs.orig_w + (x - rs.press_x), rs.orig_h + (y - rs.press_y));
        if pressed {
            if (tw, th) != (rs.w, rs.h) {
                if let Some(g) = st.router.resize.as_mut() {
                    g.w = tw;
                    g.h = th;
                }
                mark(dirty);
            }
        } else {
            // The last held position decides; release coordinates are not trusted.
            st.router.resize = None;
            if (rs.w, rs.h) != (w.w, w.h) {
                st.resize_window(rs.task, rs.w, rs.h, dirty);
            }
            mark(dirty); // the outline goes away
        }
        return;
    }

    if fresh_press {
        let Some(task) = st.windows.find_at(x, y) else { return };
        let (rel_x, rel_y, win_w, win_h, closable, resizable) = {
            let w = st.windows.by_task(task).expect("find_at returned a live window");
            (x - w.x, y - w.y, w.w, w.h, w.closable, w.resizable)
        };
        if resizable && rel_x >= win_w - RESIZE_GRIP && rel_y >= win_h - RESIZE_GRIP {
            activate_window(st, task, dirty);
            st.router.resize = Some(Resize { task, press_x: x, press_y: y, orig_w: win_w, orig_h: win_h, w: win_w, h: win_h });
            return;
        }
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
        add_resizable(&mut st, 3, 300, 300, 100, 100);
        poll(&mut st, None, touch(396, 396, true), &d); // grip on 3
        assert!(st.resize_outline().is_some());
        forget_task(&mut st, TaskId(3));
        assert_eq!(st.resize_outline(), None, "resize is gone");
        poll(&mut st, None, UP, &d);
        poll(&mut st, None, touch(450, 140, true), &d); // owner 2
        forget_task(&mut st, TaskId(2));
        poll(&mut st, None, UP, &d);
        assert_eq!(drain(&qb), [Event::Touch { x: 50, y: 40, pressed: true }], "no release for a forgotten owner");
    }

    fn add_resizable(st: &mut KernelState, task: u32, x: i32, y: i32, w: i32, h: i32) -> Arc<EventQueue> {
        let q = add(st, task, x, y, w, h, true);
        let win = st.windows.by_task_mut(TaskId(task)).unwrap();
        win.resizable = true;
        win.min_w = 80;
        win.min_h = 48;
        q
    }

    #[test]
    fn dragging_the_grip_resizes_on_release_and_only_then() {
        let mut st = KernelState::new(); // 640x480
        let d = AtomicBool::new(false);
        let q = add_resizable(&mut st, 1, 100, 100, 200, 150);
        // grip spans x 292..300, y 242..250 on screen
        poll(&mut st, None, touch(296, 246, true), &d);
        assert!(drain(&q).is_empty(), "a grip press is not a touch for the app");
        poll(&mut st, None, touch(346, 296, true), &d);
        assert_eq!(st.resize_outline(), Some((100, 100, 250, 200)), "the outline follows the pointer");
        assert_eq!(st.windows.by_task(TaskId(1)).map(|w| (w.w, w.h)), Some((200, 150)), "nothing applied yet");
        poll(&mut st, None, UP, &d);
        assert_eq!(st.windows.by_task(TaskId(1)).map(|w| (w.w, w.h)), Some((250, 200)));
        assert_eq!(drain(&q), [Event::Resized { w: 250, h: 200 }]);
        assert_eq!(st.resize_outline(), None);
    }

    #[test]
    fn the_resize_target_clamps_to_the_minimum_and_the_screen() {
        let mut st = KernelState::new();
        let d = AtomicBool::new(false);
        add_resizable(&mut st, 1, 100, 100, 200, 150);
        poll(&mut st, None, touch(296, 246, true), &d);
        poll(&mut st, None, touch(0, 0, true), &d);
        assert_eq!(st.resize_outline(), Some((100, 100, 80, 48)));
        poll(&mut st, None, touch(2000, 2000, true), &d);
        assert_eq!(st.resize_outline(), Some((100, 100, 540, 380)));
    }

    #[test]
    fn a_window_left_of_the_screen_cannot_grow_without_bound() {
        let mut st = KernelState::new(); // 640x480
        let d = AtomicBool::new(false);
        add_resizable(&mut st, 1, -300, 100, 400, 150);
        for _ in 0..2 {
            let (w, h) = st.windows.by_task(TaskId(1)).map(|w| (w.w, w.h)).unwrap();
            // grip at the window's bottom-right, on screen
            poll(&mut st, None, touch(-300 + w - 4, 100 + h - 4, true), &d);
            poll(&mut st, None, touch(5000, 100 + h - 4, true), &d);
            poll(&mut st, None, UP, &d);
            let w = st.windows.by_task(TaskId(1)).map(|w| w.w).unwrap();
            assert!(w <= 640, "width {w}");
        }
    }

    #[test]
    fn a_held_resize_survives_the_pointer_crossing_the_desktop_strip() {
        let mut st = KernelState::new();
        let d = AtomicBool::new(false);
        let qd = add(&mut st, 9, 0, 0, 640, 24, false);
        st.router.desktop = Some(TaskId(9));
        let q = add_resizable(&mut st, 1, 100, 100, 200, 150);
        poll(&mut st, None, touch(296, 246, true), &d);
        poll(&mut st, None, touch(350, 10, true), &d);
        assert!(drain(&qd).is_empty(), "the desktop gets no touch");
        assert_eq!(st.resize_outline(), Some((100, 100, 254, 48)), "the outline still follows");
        poll(&mut st, None, UP, &d);
        assert!(drain(&qd).is_empty());
        assert_eq!(drain(&q), [Event::Resized { w: 254, h: 48 }]);
    }

    #[test]
    fn a_grip_press_on_a_fixed_window_is_an_ordinary_touch() {
        let mut st = KernelState::new();
        let d = AtomicBool::new(false);
        let q = add(&mut st, 1, 100, 100, 200, 150, true);
        poll(&mut st, None, touch(296, 246, true), &d);
        assert_eq!(drain(&q), [Event::Touch { x: 196, y: 146, pressed: true }]);
    }

    #[test]
    fn releasing_where_it_started_sends_nothing() {
        let mut st = KernelState::new();
        let d = AtomicBool::new(false);
        let q = add_resizable(&mut st, 1, 100, 100, 200, 150);
        poll(&mut st, None, touch(296, 246, true), &d);
        poll(&mut st, None, UP, &d);
        assert!(drain(&q).is_empty());
    }

    #[test]
    fn forget_task_ends_a_resize() {
        let mut st = KernelState::new();
        let d = AtomicBool::new(false);
        add_resizable(&mut st, 1, 100, 100, 200, 150);
        poll(&mut st, None, touch(296, 246, true), &d);
        forget_task(&mut st, TaskId(1));
        assert_eq!(st.resize_outline(), None);
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
