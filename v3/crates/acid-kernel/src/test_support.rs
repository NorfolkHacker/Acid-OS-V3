//! Helpers shared by the kernel's own tests (kernel.rs, overlay.rs,
//! windows_api.rs). Test-only.

use std::sync::mpsc;
use std::time::{Duration, Instant};

use alloc::sync::Arc;

use acid_testkit::FakePlatform;

use crate::event::Event;
use crate::{AppContext, AppRunner, Kernel, SpawnRequest};

/// A runner that hands its AppContext to the test, then parks until the
/// window is closed -- a stand-in for a real app.
pub(crate) fn parked_runner(tx: mpsc::Sender<AppContext>) -> AppRunner {
    let tx = std::sync::Mutex::new(tx);
    Arc::new(move |ctx: AppContext| {
        let (queue, kernel) = (ctx.queue.clone(), ctx.kernel.clone());
        let _ = tx.lock().unwrap().send(ctx);
        while queue.recv_timeout(kernel.platform(), 20) != Some(Event::Close) {}
    })
}

pub(crate) fn req(x: i32, y: i32, w: i32, h: i32) -> SpawnRequest {
    SpawnRequest { script_path: "test".into(), x, y, w, h, closable: true, arg: Some("a".into()), libs: None, force_cart: false }
}

pub(crate) fn setup() -> (Arc<FakePlatform>, Arc<Kernel>, mpsc::Receiver<AppContext>) {
    let p = FakePlatform::new(".");
    let k = Kernel::new(p.clone());
    let (tx, rx) = mpsc::channel();
    k.set_runner(parked_runner(tx));
    (p, k, rx)
}

pub(crate) fn recv(rx: &mpsc::Receiver<AppContext>) -> AppContext {
    rx.recv_timeout(Duration::from_secs(5)).expect("app thread started")
}

pub(crate) fn wait_until(mut f: impl FnMut() -> bool) {
    let t = Instant::now();
    while !f() {
        assert!(t.elapsed() < Duration::from_secs(5), "timed out");
        std::thread::sleep(Duration::from_millis(5));
    }
}
