//! A headless Platform for tests and benchmarks: real threads and real
//! files (so Lua apps run for real), but the display just keeps the last
//! frame and input is scripted by the test.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub use acid_platform::std_impl::{StdFs, StdSignal};
use acid_platform::{CartStat, FsError, LocalTime, NetworkInfo, ThreadSample, Display, Fs, Input, Platform, Signal, SpawnError, TaskFn, TouchState, std_impl::carts::HostCarts, std_impl::std_spawn};

#[derive(Default)]
pub struct FakeDisplay {
    // Only the last frame: a benchmark presents hundreds of frames, and
    // keeping all of them would distort its memory numbers.
    last: Mutex<Option<Vec<u16>>>,
    count: AtomicUsize,
}

impl FakeDisplay {
    pub fn last_frame(&self) -> Option<Vec<u16>> {
        self.last.lock().unwrap().clone()
    }

    pub fn frame_count(&self) -> usize {
        self.count.load(Ordering::SeqCst)
    }
}

impl Display for FakeDisplay {
    fn present(&self, pixels: &[u16], _width: usize, _height: usize) {
        // Reuse the stored buffer: a fresh ~460 KB allocation per frame would
        // dominate (and add noise to) the benchmark's composite timing.
        let mut last = self.last.lock().unwrap();
        match last.as_mut() {
            Some(buf) => {
                buf.clear();
                buf.extend_from_slice(pixels);
            }
            None => *last = Some(pixels.to_vec()),
        }
        self.count.fetch_add(1, Ordering::SeqCst);
    }
}

#[derive(Default)]
pub struct FakeInput {
    touch: Mutex<TouchState>,
    keys: Mutex<VecDeque<i32>>,
    quit: AtomicBool,
}

impl FakeInput {
    pub fn set_touch(&self, x: i32, y: i32, pressed: bool) {
        *self.touch.lock().unwrap() = TouchState { x, y, pressed };
    }

    pub fn push_key(&self, code: i32) {
        self.keys.lock().unwrap().push_back(code);
    }

    pub fn request_quit(&self) {
        self.quit.store(true, Ordering::SeqCst);
    }
}

impl Input for FakeInput {
    fn poll_touch(&self) -> TouchState {
        *self.touch.lock().unwrap()
    }

    fn poll_key(&self) -> Option<i32> {
        self.keys.lock().unwrap().pop_front()
    }

    fn should_quit(&self) -> bool {
        self.quit.load(Ordering::SeqCst)
    }
}

pub struct FakePlatform {
    start: Instant,
    pub display: FakeDisplay,
    pub input: FakeInput,
    fs: StdFs,
    fail_spawns: AtomicBool,
    local_time: Mutex<LocalTime>,
    mem_kb: AtomicI64,
    network: Mutex<NetworkInfo>,
    threads: Mutex<Vec<ThreadSample>>,
    carts: Mutex<HostCarts>,
    fs_root: PathBuf,
    restarts: AtomicUsize,
}

impl FakePlatform {
    pub fn new(root: impl Into<PathBuf>) -> Arc<Self> {
        let root = root.into();
        Arc::new(Self {
            start: Instant::now(),
            display: FakeDisplay::default(),
            input: FakeInput::default(),
            fs: StdFs::new(root.clone()),
            fail_spawns: AtomicBool::new(false),
            local_time: Mutex::new(LocalTime { year: 2026, month: 10, day: 2, hour: 9, min: 5, sec: 0 }),
            mem_kb: AtomicI64::new(-1),
            network: Mutex::new(NetworkInfo::unknown()),
            threads: Mutex::new(Vec::new()),
            carts: Mutex::new(HostCarts::with_roots(root.clone(), Vec::new())),
            fs_root: root,
            restarts: AtomicUsize::new(0),
        })
    }

    /// Makes the next spawn() fail, to exercise spawn rollback paths.
    pub fn fail_next_spawn(&self) {
        self.fail_spawns.store(true, Ordering::SeqCst);
    }

    pub fn set_local_time(&self, t: LocalTime) {
        *self.local_time.lock().unwrap() = t;
    }

    pub fn set_mem_used_kb(&self, kb: i64) {
        self.mem_kb.store(kb, Ordering::SeqCst);
    }

    pub fn set_network_info(&self, n: NetworkInfo) {
        *self.network.lock().unwrap() = n;
    }

    /// Which host folders count as cart folders (default: none).
    pub fn set_cart_roots(&self, roots: Vec<String>) {
        let mut c = self.carts.lock().unwrap();
        *c = HostCarts::with_roots(self.fs_root.clone(), roots);
    }

    /// How many times restart() has been asked for.
    pub fn restart_count(&self) -> usize {
        self.restarts.load(Ordering::SeqCst)
    }

    pub fn set_thread_samples(&self, s: Vec<ThreadSample>) {
        *self.threads.lock().unwrap() = s;
    }

    /// The repository root, so tests can use the same repo-relative script
    /// paths ("v3/apps/...") the real OS uses.
    pub fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
    }
}

impl Platform for FakePlatform {
    /// Counts the call and reports success, without restarting anything.
    fn restart(&self) -> bool {
        self.restarts.fetch_add(1, Ordering::SeqCst);
        true
    }
    fn spawn(&self, name: &str, f: TaskFn) -> Result<(), SpawnError> {
        if self.fail_spawns.swap(false, Ordering::SeqCst) {
            return Err(SpawnError);
        }
        std_spawn(name, f)
    }

    fn now_ms(&self) -> u64 {
        self.start.elapsed().as_millis() as u64
    }

    fn sleep_ms(&self, ms: u32) {
        std::thread::sleep(Duration::from_millis(ms as u64));
    }

    fn local_time(&self) -> LocalTime {
        *self.local_time.lock().unwrap()
    }

    fn mem_used_kb(&self) -> i64 {
        self.mem_kb.load(Ordering::SeqCst)
    }

    fn network_info(&self) -> NetworkInfo {
        self.network.lock().unwrap().clone()
    }

    fn thread_samples(&self) -> Vec<ThreadSample> {
        self.threads.lock().unwrap().clone()
    }

    fn cart_roots(&self) -> Vec<String> {
        self.carts.lock().unwrap().roots()
    }

    fn cart_list(&self, dir: &str) -> Result<Vec<String>, FsError> {
        self.carts.lock().unwrap().list(dir)
    }

    fn cart_stat(&self, path: &str) -> Result<CartStat, FsError> {
        self.carts.lock().unwrap().stat(path)
    }

    fn cart_read(&self, path: &str) -> Result<Vec<u8>, FsError> {
        self.carts.lock().unwrap().read(path)
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
}

#[cfg(test)]
mod tests {
    #[test]
    fn fail_next_spawn_fails_exactly_once() {
        let p = FakePlatform::new(".");
        p.fail_next_spawn();
        assert!(p.spawn("x", Box::new(|| {})).is_err());
        assert!(p.spawn("x", Box::new(|| {})).is_ok());
    }

    use super::*;
    use acid_platform::Signal;

    #[test]
    fn system_info_is_scripted() {
        use acid_platform::{LocalTime, NetworkInfo, Platform, ThreadSample};
        let p = FakePlatform::new(".");
        assert_eq!(p.local_time(), LocalTime { year: 2026, month: 10, day: 2, hour: 9, min: 5, sec: 0 });
        assert_eq!((p.mem_used_kb(), p.network_info(), p.thread_samples()), (-1, NetworkInfo::unknown(), vec![]));
        p.set_mem_used_kb(512);
        p.set_network_info(NetworkInfo { host: "h".into(), ip: "1.2.3.4".into(), connected: true });
        p.set_thread_samples(vec![ThreadSample { id: 1, name: "router".into(), state: 'S', cpu_ms: 10 }]);
        assert_eq!(p.mem_used_kb(), 512);
        assert_eq!(p.network_info().ip, "1.2.3.4");
        assert_eq!(p.thread_samples()[0].name, "router");
    }
    use std::time::Instant;

    #[test]
    fn signal_notified_before_wait_returns_true_immediately() {
        let s = StdSignal::new();
        s.notify();
        let t = Instant::now();
        assert!(s.wait_timeout(1000));
        assert!(t.elapsed().as_millis() < 100);
    }

    #[test]
    fn signal_times_out_and_is_consumed() {
        let s = StdSignal::new();
        s.notify();
        assert!(s.wait_timeout(10));
        assert!(!s.wait_timeout(10), "a notify is consumed by one wait");
    }

    #[test]
    fn fs_reads_relative_to_root() {
        let p = FakePlatform::new(FakePlatform::repo_root());
        let text = p.fs().read("v3/Cargo.toml").expect("workspace manifest");
        assert!(String::from_utf8_lossy(&text).contains("[workspace]"));
        assert_eq!(p.fs().read("v3/no-such-file"), Err(acid_platform::FsError::NotFound));
    }

    #[test]
    fn display_keeps_only_the_last_frame() {
        let p = FakePlatform::new(".");
        p.display().present(&[1, 2], 2, 1);
        p.display().present(&[3, 4], 2, 1);
        assert_eq!(p.display.frame_count(), 2);
        assert_eq!(p.display.last_frame(), Some(vec![3, 4]));
    }

    #[test]
    fn input_queues_keys_and_reports_touch() {
        let p = FakePlatform::new(".");
        p.input.set_touch(5, 6, true);
        p.input.push_key(65);
        assert_eq!(p.input().poll_touch(), TouchState { x: 5, y: 6, pressed: true });
        assert_eq!(p.input().poll_key(), Some(65));
        assert_eq!(p.input().poll_key(), None);
        assert!(!p.input().should_quit());
        p.input.request_quit();
        assert!(p.input().should_quit());
    }
}
