//! The hosted x64 backend: std threads, a winit window drawn through
//! softbuffer, and files under the working directory. winit must own the
//! main thread, so the router (on its own thread) and the window talk
//! through the shared input state and frame buffer here.

pub mod audio;
pub mod keymap;
pub mod picker;
pub mod restart;
pub mod window;

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use acid_gfx::rgb565_to_888;
use acid_platform::std_impl::carts::HostCarts;
use acid_platform::std_impl::{StdFs, StdSignal, std_spawn};
use acid_platform::{CartStat, Display, FsError, Fs, Input, KeyEvent, Platform, Signal, SpawnError, TaskFn, TouchState};
use winit::event_loop::EventLoopProxy;

/// Key queue capacity: a full queue drops the newest press.
pub const KEY_QUEUE_CAP: usize = 16;

#[derive(Debug, Clone, Copy)]
pub enum UserEvent {
    /// A new frame is in HostedDisplay's buffer.
    Frame,
}

#[derive(Default)]
pub struct HostedInput {
    touch: Mutex<TouchState>,
    keys: Mutex<VecDeque<KeyEvent>>,
    quit: AtomicBool,
}

impl HostedInput {
    pub fn set_position(&self, x: i32, y: i32) {
        let mut t = self.touch.lock().unwrap();
        t.x = x;
        t.y = y;
    }

    pub fn set_pressed(&self, pressed: bool) {
        self.touch.lock().unwrap().pressed = pressed;
    }

    /// Queues a press or release. A full queue drops presses, but keeps
    /// room for releases (up to twice the cap): a lost release would leave
    /// an app thinking the key is still held.
    pub fn push_key(&self, code: i32, pressed: bool) {
        let mut q = self.keys.lock().unwrap();
        let cap = if pressed { KEY_QUEUE_CAP } else { 2 * KEY_QUEUE_CAP };
        if q.len() < cap {
            q.push_back(KeyEvent { code, pressed });
        }
    }

    pub fn request_quit(&self) {
        self.quit.store(true, Ordering::SeqCst);
    }
}

impl Input for HostedInput {
    fn poll_touch(&self) -> TouchState {
        *self.touch.lock().unwrap()
    }

    fn poll_key(&self) -> Option<KeyEvent> {
        self.keys.lock().unwrap().pop_front()
    }

    fn should_quit(&self) -> bool {
        self.quit.load(Ordering::SeqCst)
    }
}

#[derive(Default)]
pub struct HostedDisplay {
    frame: Mutex<Vec<u32>>,
    proxy: Mutex<Option<EventLoopProxy<UserEvent>>>,
}

impl Display for HostedDisplay {
    fn present(&self, pixels: &[u16], _width: usize, _height: usize) {
        {
            let mut frame = self.frame.lock().unwrap();
            frame.clear();
            frame.extend(pixels.iter().map(|&p| rgb565_to_888(p)));
        }
        if let Some(proxy) = self.proxy.lock().unwrap().as_ref() {
            let _ = proxy.send_event(UserEvent::Frame);
        }
    }
}

pub struct HostedPlatform {
    start: Instant,
    pub input: HostedInput,
    display: HostedDisplay,
    fs: StdFs,
    carts: HostCarts,
}

impl HostedPlatform {
    pub fn new(root: &str) -> Arc<Self> {
        Arc::new(Self {
            start: Instant::now(),
            input: HostedInput::default(),
            display: HostedDisplay::default(),
            fs: StdFs::new(root),
            carts: HostCarts::from_env(root),
        })
    }

    pub fn set_proxy(&self, proxy: EventLoopProxy<UserEvent>) {
        *self.display.proxy.lock().unwrap() = Some(proxy);
    }

    /// The last presented frame as 0x00RRGGBB, softbuffer's pixel format.
    pub fn frame_snapshot(&self) -> Vec<u32> {
        self.display.frame.lock().unwrap().clone()
    }
}

impl Platform for HostedPlatform {
    fn spawn(&self, name: &str, f: TaskFn) -> Result<(), SpawnError> {
        std_spawn(name, f)
    }

    fn now_ms(&self) -> u64 {
        self.start.elapsed().as_millis() as u64
    }

    fn sleep_ms(&self, ms: u32) {
        std::thread::sleep(Duration::from_millis(ms as u64));
    }

    fn cart_roots(&self) -> Vec<String> {
        self.carts.roots()
    }

    fn cart_list(&self, dir: &str) -> Result<Vec<String>, FsError> {
        self.carts.list(dir)
    }

    fn cart_stat(&self, path: &str) -> Result<CartStat, FsError> {
        self.carts.stat(path)
    }

    fn cart_read(&self, path: &str) -> Result<Vec<u8>, FsError> {
        self.carts.read(path)
    }

    fn local_time(&self) -> acid_platform::LocalTime {
        acid_platform::std_impl::sysinfo::local_time()
    }

    fn mem_used_kb(&self) -> i64 {
        acid_platform::std_impl::sysinfo::mem_used_kb()
    }

    fn network_info(&self) -> acid_platform::NetworkInfo {
        acid_platform::std_impl::sysinfo::network_info()
    }

    fn thread_samples(&self) -> Vec<acid_platform::ThreadSample> {
        acid_platform::std_impl::sysinfo::thread_samples()
    }

    fn new_signal(&self) -> Arc<dyn Signal> {
        Arc::new(StdSignal::new())
    }

    fn display(&self) -> &dyn Display {
        &self.display
    }

    fn input(&self) -> &dyn Input {
        &self.input
    }

    fn fs(&self) -> &dyn Fs {
        &self.fs
    }

    fn restart(&self) -> bool {
        restart::restart_process()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_queue_drops_past_sixteen() {
        let input = HostedInput::default();
        for i in 0..20 {
            input.push_key(i, true);
        }
        let mut got = Vec::new();
        while let Some(k) = input.poll_key() {
            got.push(k.code);
        }
        assert_eq!(got, (0..16).collect::<Vec<_>>());
    }

    #[test]
    fn a_full_queue_still_takes_releases() {
        let input = HostedInput::default();
        for i in 0..20 {
            input.push_key(i, true);
        }
        input.push_key(3, false);
        let mut got = Vec::new();
        while let Some(k) = input.poll_key() {
            got.push(k);
        }
        assert_eq!(got.len(), 17);
        assert_eq!(got[16], KeyEvent { code: 3, pressed: false });
    }

    #[test]
    fn touch_combines_position_and_button() {
        let input = HostedInput::default();
        input.set_position(10, 20);
        input.set_pressed(true);
        assert_eq!(input.poll_touch(), TouchState { x: 10, y: 20, pressed: true });
    }

    #[test]
    fn present_converts_to_xrgb_without_an_event_loop() {
        let p = HostedPlatform::new(".");
        p.display().present(&[0xFFFF, 0x0000], 2, 1);
        assert_eq!(p.frame_snapshot()[..2], [0x00FF_FFFF, 0]);
    }
}
