//! The window registry. At most WINDOW_MAX windows; a higher z is nearer
//! the front.

use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;

use acid_gfx::Canvas;
use acid_platform::sync::Mutex;

use crate::TaskId;
use crate::event::EventQueue;
use crate::layout::WINDOW_MAX;

pub struct Window {
    pub task: TaskId,
    pub queue: Arc<EventQueue>,
    /// The app draws into this with window-relative coordinates; only the
    /// compositor ever puts it on screen. Owned jointly with the app's
    /// thread, so unregistering never frees it out from under a draw.
    pub canvas: Arc<Mutex<Canvas>>,
    pub app_name: String,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub z: i32,
    pub closable: bool,
    /// The app's trust level (spec §14.2), set by Kernel::spawn_app. Counted
    /// against CART_WINDOW_MAX.
    pub cart: bool,
    /// The app's text scale (1 or 2), fixed at spawn.
    pub font_scale: i32,
    /// Opted in to being resized by dragging its grip.
    pub resizable: bool,
    /// Smallest size a resize may give it (see `router::resize_clamp`).
    pub min_w: i32,
    pub min_h: i32,
}

impl Window {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        task: TaskId,
        queue: Arc<EventQueue>,
        canvas: Arc<Mutex<Canvas>>,
        app_name: String,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        closable: bool,
    ) -> Self {
        Self { task, queue, canvas, app_name, x, y, w, h, z: 0, closable, cart: false, font_scale: 1, resizable: false, min_w: 0, min_h: 0 }
    }

    fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }
}

pub struct WindowRegistry {
    slots: Vec<Option<Window>>,
    next_z: i32,
}

impl Default for WindowRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl WindowRegistry {
    pub fn new() -> Self {
        Self { slots: (0..WINDOW_MAX).map(|_| None).collect(), next_z: 1 }
    }

    /// False when all WINDOW_MAX slots are taken. A new window goes in front.
    pub fn register(&mut self, mut win: Window) -> bool {
        let Some(slot) = self.slots.iter_mut().find(|s| s.is_none()) else {
            return false;
        };
        win.z = self.next_z;
        self.next_z += 1;
        *slot = Some(win);
        true
    }

    /// Open windows whose app is cart-level.
    pub fn cart_count(&self) -> usize {
        self.slots.iter().flatten().filter(|w| w.cart).count()
    }

    pub fn unregister(&mut self, task: TaskId) -> Option<Window> {
        self.slots.iter_mut().find(|s| s.as_ref().is_some_and(|w| w.task == task))?.take()
    }

    pub fn by_task(&self, task: TaskId) -> Option<&Window> {
        self.slots.iter().flatten().find(|w| w.task == task)
    }

    pub fn by_task_mut(&mut self, task: TaskId) -> Option<&mut Window> {
        self.slots.iter_mut().flatten().find(|w| w.task == task)
    }

    /// The topmost window containing (x, y).
    pub fn find_at(&self, x: i32, y: i32) -> Option<TaskId> {
        self.slots.iter().flatten().filter(|w| w.contains(x, y)).max_by_key(|w| w.z).map(|w| w.task)
    }

    pub fn bring_to_front(&mut self, task: TaskId) -> bool {
        let z = self.next_z;
        match self.by_task_mut(task) {
            Some(w) => {
                w.z = z;
                self.next_z += 1;
                true
            }
            None => false,
        }
    }

    /// The window in slot `i` (0..WINDOW_MAX). Slot numbers are the window
    /// ids apps see.
    pub fn at_index(&self, i: usize) -> Option<&Window> {
        self.slots.get(i)?.as_ref()
    }

    pub fn topmost(&self) -> Option<TaskId> {
        self.slots.iter().flatten().max_by_key(|w| w.z).map(|w| w.task)
    }

    /// Puts `task` below every other window: z = (lowest z, its own
    /// included) - 1.
    pub fn send_to_back(&mut self, task: TaskId) -> bool {
        let Some(own) = self.by_task(task).map(|w| w.z) else { return false };
        let min = self.slots.iter().flatten().map(|w| w.z).min().unwrap_or(own);
        self.by_task_mut(task).expect("checked above").z = min - 1;
        true
    }

    /// First window (in slot order) whose app_name is exactly `name` and
    /// whose app is not cart-level -- the singleton a built-in caller may
    /// raise (spec §16.2: a cart-started copy is skipped, not raised).
    pub fn find_trusted_by_app_name(&self, name: &str) -> Option<TaskId> {
        self.slots.iter().flatten().find(|w| w.app_name == name && !w.cart).map(|w| w.task)
    }

    /// First window (in slot order) whose app_name is exactly `name`.
    pub fn find_by_app_name(&self, name: &str) -> Option<TaskId> {
        self.slots.iter().flatten().find(|w| w.app_name == name).map(|w| w.task)
    }

    pub fn count(&self) -> usize {
        self.slots.iter().flatten().count()
    }

    pub fn in_z_order(&self) -> Vec<&Window> {
        let mut v: Vec<&Window> = self.slots.iter().flatten().collect();
        v.sort_by_key(|w| w.z);
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use acid_testkit::StdSignal;

    fn win(task: u32, x: i32, y: i32, w: i32, h: i32) -> Window {
        Window::new(
            TaskId(task),
            Arc::new(EventQueue::new(Arc::new(StdSignal::new()))),
            Arc::new(Mutex::new(Canvas::new(w, h))),
            "test".into(),
            x, y, w, h,
            true,
        )
    }

    #[test]
    fn register_hit_test_raise_and_fill() {
        let mut r = WindowRegistry::new();
        assert_eq!(r.count(), 0);
        assert!(r.register(win(1, 0, 0, 100, 100)));
        assert!(r.register(win(2, 50, 50, 100, 100)));
        assert_eq!(r.count(), 2);
        assert_eq!(r.find_at(60, 60), Some(TaskId(2)));
        assert_eq!(r.find_at(10, 10), Some(TaskId(1)));
        assert_eq!(r.find_at(500, 500), None);
        assert!(r.bring_to_front(TaskId(1)));
        assert_eq!(r.find_at(60, 60), Some(TaskId(1)));
        assert!(r.unregister(TaskId(1)).is_some());
        assert_eq!(r.count(), 1);
        assert_eq!(r.find_at(10, 10), None);
        let mut registered = 1;
        for i in 0..WINDOW_MAX as u32 {
            if r.register(win(1000 + i, 0, 0, 1, 1)) {
                registered += 1;
            }
        }
        assert_eq!(registered, WINDOW_MAX);
        assert!(!r.register(win(3, 0, 0, 1, 1)));
    }

    #[test]
    fn hit_test_is_half_open() {
        let mut r = WindowRegistry::new();
        r.register(win(1, 10, 10, 5, 5));
        assert_eq!(r.find_at(10, 10), Some(TaskId(1)));
        assert_eq!(r.find_at(14, 14), Some(TaskId(1)));
        assert_eq!(r.find_at(15, 14), None);
        assert_eq!(r.find_at(14, 15), None);
    }

    #[test]
    fn in_z_order_is_back_to_front() {
        let mut r = WindowRegistry::new();
        r.register(win(1, 0, 0, 1, 1));
        r.register(win(2, 0, 0, 1, 1));
        r.register(win(3, 0, 0, 1, 1));
        r.bring_to_front(TaskId(1));
        let order: Vec<TaskId> = r.in_z_order().iter().map(|w| w.task).collect();
        assert_eq!(order, [TaskId(2), TaskId(3), TaskId(1)]);
    }

    #[test]
    fn missing_task_operations_are_no_ops() {
        let mut r = WindowRegistry::new();
        assert!(!r.bring_to_front(TaskId(9)));
        assert!(r.unregister(TaskId(9)).is_none());
        assert!(r.by_task(TaskId(9)).is_none());
    }

    #[test]
    fn at_index_is_the_slot_number() {
        let mut r = WindowRegistry::new();
        r.register(win(1, 0, 0, 1, 1));
        r.register(win(2, 0, 0, 1, 1));
        r.unregister(TaskId(1));
        assert!(r.at_index(0).is_none(), "slot 0 is free again");
        assert_eq!(r.at_index(1).map(|w| w.task), Some(TaskId(2)));
        assert!(r.at_index(8).is_none());
    }

    #[test]
    fn send_to_back_goes_below_everything() {
        let mut r = WindowRegistry::new();
        r.register(win(1, 0, 0, 10, 10));
        r.register(win(2, 0, 0, 10, 10));
        r.register(win(3, 0, 0, 10, 10));
        assert_eq!(r.topmost(), Some(TaskId(3)));
        assert!(r.send_to_back(TaskId(3)));
        assert_eq!(r.find_at(5, 5), Some(TaskId(2)));
        assert_eq!(r.in_z_order()[0].task, TaskId(3));
        assert!(!r.send_to_back(TaskId(9)));
    }

    #[test]
    fn find_by_app_name_matches_exactly() {
        let mut r = WindowRegistry::new();
        let mut w = win(1, 0, 0, 1, 1);
        w.app_name = "v3/apps/editor.lua".into();
        r.register(w);
        assert_eq!(r.find_by_app_name("v3/apps/editor.lua"), Some(TaskId(1)));
        assert_eq!(r.find_by_app_name("v3/fsroot/Source/editor.lua"), None);
    }
}
