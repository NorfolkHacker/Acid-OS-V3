//! Per-app event queues: an EVENT_QUEUE_CAP-slot ring per app plus the
//! signal an app's acid_poll_event blocks on.

use alloc::collections::VecDeque;
use alloc::sync::Arc;

use acid_platform::sync::Mutex;
use acid_platform::{Platform, Signal};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    Touch { x: i32, y: i32, pressed: bool },
    Close,
    /// Reserved for window moves; the router never sends it yet.
    Moved { x: i32, y: i32 },
    Key { code: i32 },
    /// The window was resized; the app re-lays out (resizable windows).
    Resized { w: i32, h: i32 },
}

pub const EVENT_QUEUE_CAP: usize = 8;

pub struct EventQueue {
    ring: Mutex<VecDeque<Event>>,
    signal: Arc<dyn Signal>,
}

impl EventQueue {
    pub fn new(signal: Arc<dyn Signal>) -> Self {
        Self { ring: Mutex::new(VecDeque::with_capacity(EVENT_QUEUE_CAP)), signal }
    }

    /// Never blocks. A full queue drops the event and returns false --
    /// the router's best-effort contract.
    pub fn send(&self, ev: Event) -> bool {
        {
            let mut ring = self.ring.lock();
            if ring.len() >= EVENT_QUEUE_CAP {
                return false;
            }
            ring.push_back(ev);
        }
        self.signal.notify();
        true
    }

    pub fn try_recv(&self) -> Option<Event> {
        self.ring.lock().pop_front()
    }

    pub fn recv_timeout(&self, platform: &dyn Platform, ms: u32) -> Option<Event> {
        let deadline = platform.now_ms() + ms as u64;
        loop {
            if let Some(ev) = self.try_recv() {
                return Some(ev);
            }
            let now = platform.now_ms();
            if now >= deadline {
                return None;
            }
            if !self.signal.wait_timeout((deadline - now) as u32) {
                return self.try_recv();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use acid_testkit::{FakePlatform, StdSignal};
    use alloc::sync::Arc;

    fn queue() -> EventQueue {
        EventQueue::new(Arc::new(StdSignal::new()))
    }

    #[test]
    fn delivers_in_order() {
        let q = queue();
        assert!(q.send(Event::Key { code: 1 }));
        assert!(q.send(Event::Close));
        assert_eq!(q.try_recv(), Some(Event::Key { code: 1 }));
        assert_eq!(q.try_recv(), Some(Event::Close));
        assert_eq!(q.try_recv(), None);
    }

    #[test]
    fn drops_when_full_instead_of_blocking() {
        let q = queue();
        for i in 0..EVENT_QUEUE_CAP as i32 {
            assert!(q.send(Event::Key { code: i }));
        }
        assert!(!q.send(Event::Key { code: 99 }));
        assert_eq!(q.try_recv(), Some(Event::Key { code: 0 }));
    }

    #[test]
    fn recv_timeout_waits_for_a_sender() {
        let p = FakePlatform::new(".");
        let q = Arc::new(queue());
        let q2 = q.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(20));
            q2.send(Event::Close);
        });
        assert_eq!(q.recv_timeout(&*p, 2000), Some(Event::Close));
    }

    #[test]
    fn recv_timeout_returns_none_on_timeout() {
        let p = FakePlatform::new(".");
        let q = queue();
        let t = std::time::Instant::now();
        assert_eq!(q.recv_timeout(&*p, 30), None);
        assert!(t.elapsed().as_millis() >= 25);
    }
}
