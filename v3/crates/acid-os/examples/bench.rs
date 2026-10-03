//! The Phase 1 benchmark gate: spawn 8 hello_acid windows, time until
//! every one has finished its first redraw, measure resident memory per
//! app, then time 1000 composites individually (median per frame).
//! Headless (FakePlatform), release mode:
//!   cargo run --release --manifest-path v3/Cargo.toml -p acid-os --example bench

use std::time::{Duration, Instant};

use acid_kernel::{Kernel, SpawnRequest};
use acid_os::{APPS_DIR, HELLO_PATH};
use acid_testkit::FakePlatform;

fn rss_kb() -> i64 {
    let statm = std::fs::read_to_string("/proc/self/statm").unwrap_or_default();
    let pages: i64 = statm.split_whitespace().nth(1).and_then(|s| s.parse().ok()).unwrap_or(0);
    pages * 4 // x86_64 Linux pages are 4 KiB
}

fn busy_spin_ms(ms: u64) {
    let t = Instant::now();
    while t.elapsed() < Duration::from_millis(ms) {
        std::hint::spin_loop();
    }
}

fn main() {
    let platform = FakePlatform::new(FakePlatform::repo_root());
    let kernel = Kernel::new(platform.clone());
    kernel.set_runner(acid_lua::lua_runner(APPS_DIR));
    // The CPU clock ramps up after idle (powersave governor): the first ~20
    // composites run ~7x slower than steady state. Busy-spin ~100 ms first so
    // both timed phases start at full clock.
    std::thread::sleep(Duration::from_millis(500));
    busy_spin_ms(100);

    let rss0 = rss_kb();
    let t0 = Instant::now();
    let tasks: Vec<_> = (0..8)
        .map(|i| {
            kernel
                .spawn_app(SpawnRequest {
                    script_path: HELLO_PATH.into(),
                    x: 20 + i * 18, y: 34 + i * 18, w: 200, h: 150, closable: true, arg: None,
                    libs: Some("lib/acid_palette.lua".into()),
                    force_cart: false,
                })
                .expect("spawn")
        })
        .collect();
    loop {
        let ready = kernel.with_state(|st| {
            tasks
                .iter()
                .filter(|t| st.windows.by_task(**t).is_some_and(|w| w.canvas.lock().pixel(100, 149).unwrap_or(0) != 0))
                .count()
        });
        if ready == 8 {
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    let spawn_ms = t0.elapsed().as_secs_f64() * 1000.0;
    std::thread::sleep(Duration::from_millis(500));
    let rss1 = rss_kb();

    // Warm-up: busy-spin with discarded composites, then time each of 1000
    // composites on its own and report the median.
    let w0 = Instant::now();
    while w0.elapsed() < Duration::from_millis(100) {
        kernel.composite_frame();
    }
    let mut us: Vec<f64> = (0..1000)
        .map(|_| {
            let c0 = Instant::now();
            kernel.composite_frame();
            c0.elapsed().as_secs_f64() * 1e6
        })
        .collect();
    us.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let composite_us = (us[499] + us[500]) / 2.0;
    println!("BENCH spawn8_ms={spawn_ms:.2} composite_us={composite_us:.1} rss_per_app_kb={}", (rss1 - rss0) / 8);
    std::process::exit(0);
}
