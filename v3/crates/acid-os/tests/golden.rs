use std::path::Path;
use std::time::{Duration, Instant};

use acid_gfx::{rgb565, rgb565_to_888};
use acid_kernel::theme::THEME_HARD;
use acid_kernel::{Kernel, SpawnRequest, TaskId};
use acid_kernel::layout::Screen;
use acid_os::{APPS_DIR, DESKTOP_PATH, HELLO_PATH, boot, boot_with};
use acid_testkit::FakePlatform;

fn wait_for_border(k: &Kernel, task: TaskId, w: i32, h: i32) {
    // The border is drawn last in redraw, so once its bottom edge is
    // there the window's first frame is complete.
    let t = Instant::now();
    loop {
        let px = k.with_state(|st| st.windows.by_task(task).and_then(|win| win.canvas.lock().pixel(w / 2, h - 1)));
        if px == Some(rgb565(THEME_HARD)) {
            return;
        }
        assert!(t.elapsed() < Duration::from_secs(10), "app never finished its first redraw");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn boot_starts_only_the_desktop() {
    let p = FakePlatform::new(FakePlatform::repo_root());
    let k = boot(p.clone());
    let wins = k.with_state(|st| {
        st.windows.in_z_order().iter().map(|w| (w.app_name.clone(), w.x, w.y, w.w, w.h, w.closable)).collect::<Vec<_>>()
    });
    assert_eq!(wins, [("v3/apps/desktop.lua".to_string(), 0, 0, 640, 204, false)]);
}

fn load_ppm_565(path: &Path) -> (usize, usize, Vec<u16>) {
    let data = std::fs::read(path).unwrap_or_else(|e| panic!("read {}: {e} -- missing committed golden frame", path.display()));
    let mut fields = Vec::new();
    let mut i = 0;
    while fields.len() < 4 {
        while data[i].is_ascii_whitespace() {
            i += 1;
        }
        let start = i;
        while !data[i].is_ascii_whitespace() {
            i += 1;
        }
        fields.push(String::from_utf8_lossy(&data[start..i]).into_owned());
    }
    i += 1; // the single whitespace byte after maxval
    assert_eq!((fields[0].as_str(), fields[3].as_str()), ("P6", "255"));
    let (w, h): (usize, usize) = (fields[1].parse().unwrap(), fields[2].parse().unwrap());
    let px = data[i..]
        .chunks_exact(3)
        .map(|c| rgb565(((c[0] as u32) << 16) | ((c[1] as u32) << 8) | c[2] as u32))
        .collect();
    (w, h, px)
}

fn write_ppm(path: &Path, w: usize, h: usize, px: &[u16]) {
    let mut out = format!("P6\n{w} {h}\n255\n").into_bytes();
    for &p in px {
        let c = rgb565_to_888(p);
        out.extend_from_slice(&[(c >> 16) as u8, (c >> 8) as u8, c as u8]);
    }
    std::fs::write(path, out).unwrap();
}

/// Compares a frame with a committed golden frame, pixel for pixel. On a
/// mismatch it writes the actual frame next to the target dir for
/// inspection. Never regenerate a golden to make a test pass.
fn assert_matches_golden(actual: &[u16], golden_name: &str) {
    assert_matches_golden_masked(actual, golden_name, None);
}

/// As `assert_matches_golden`, skipping pixels inside `mask` = (x, y, w, h).
fn assert_matches_golden_masked(actual: &[u16], golden_name: &str, mask: Option<(usize, usize, usize, usize)>) {
    let golden = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden").join(golden_name);
    let (w, h, expected) = load_ppm_565(&golden);
    assert_eq!(actual.len(), expected.len());
    let masked = |i: usize| {
        mask.is_some_and(|(mx, my, mw, mh)| {
            let (x, y) = (i % w, i / w);
            x >= mx && x < mx + mw && y >= my && y < my + mh
        })
    };
    let diffs: Vec<usize> = (0..expected.len()).filter(|&i| !masked(i) && expected[i] != actual[i]).collect();
    if !diffs.is_empty() {
        let out = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../target/actual-{golden_name}"));
        write_ppm(&out, w, h, actual);
        let first = diffs[0];
        panic!(
            "{} pixels differ from golden {golden_name}; first at ({}, {}): expected {:#06x}, got {:#06x}. Actual frame: {}",
            diffs.len(), first % w, first / w, expected[first], actual[first], out.display()
        );
    }
}

#[test]
fn hello_acid_matches_golden_pixel_for_pixel() {
    let p = FakePlatform::new(FakePlatform::repo_root());
    let k = Kernel::with_screen(p.clone(), Screen::WIDE);
    k.set_runner(acid_lua::lua_runner(APPS_DIR));
    // Same window, same place, as in the golden frame.
    let task = k
        .spawn_app(SpawnRequest {
            script_path: HELLO_PATH.into(),
            x: 38, y: 52, w: 200, h: 150, closable: true, arg: None,
            libs: Some("lib/acid_palette.lua".into()),
            force_cart: false,
        })
        .unwrap();
    wait_for_border(&k, task, 200, 150);
    k.composite_frame();
    let actual = p.display.last_frame().unwrap();

    assert_matches_golden(&actual, "hello_acid.ppm");
}

#[test]
fn overlapping_windows_and_overlay_match_golden() {
    // The golden frame's scene: same windows, same positions, same spawn
    // (z) order. None of them is focused, so hello_acid doesn't animate
    // and the frame is deterministic.
    let p = FakePlatform::new(FakePlatform::repo_root());
    let k = Kernel::with_screen(p.clone(), Screen::WIDE);
    k.set_runner(acid_lua::lua_runner_trusting(APPS_DIR, "v3/crates/acid-os/tests/fixtures/"));
    let hello = |x, y| SpawnRequest {
        script_path: HELLO_PATH.into(), x, y, w: 200, h: 150, closable: true, arg: None,
        libs: Some("lib/acid_palette.lua".into()),
        force_cart: false,
    };
    let a = k.spawn_app(hello(38, 52)).unwrap();
    let b = k.spawn_app(hello(120, 100)).unwrap();
    let probe = k
        .spawn_app(SpawnRequest {
            script_path: "v3/crates/acid-os/tests/fixtures/overlay_probe.lua".into(),
            x: 420, y: 240, w: 160, h: 60, closable: true, arg: None, libs: None, force_cart: false,
        })
        .unwrap();
    wait_for_border(&k, a, 200, 150);
    wait_for_border(&k, b, 200, 150);
    wait_for_border(&k, probe, 160, 60);
    // The probe is cart-level and may not open the overlay itself (§16.2),
    // so the test draws its two rectangles as the kernel.
    assert!(k.overlay_open(probe));
    k.overlay_fill_rect(probe, 200, 230, 80, 40, 0xFF2D78);
    k.overlay_fill_rect(probe, -10, 330, 40, 40, 0x00E5FF);
    assert!(k.overlay_is_open());
    k.composite_frame();
    assert_matches_golden(&p.display.last_frame().unwrap(), "overlap_overlay.ppm");
}

#[test]
fn desktop_strip_matches_golden_pixel_for_pixel() {
    let p = FakePlatform::new(FakePlatform::repo_root());
    let k = boot_with(p.clone(), Screen::WIDE);
    // Wait for the clock text and the Menu label (TEXT color, outside the
    // masked clock area), so the whole strip is in without a fixed sleep.
    let start = std::time::Instant::now();
    loop {
        let clock_drawn = k.with_state(|st| {
            st.windows.in_z_order().iter().find(|w| w.app_name == DESKTOP_PATH).is_some_and(|w| {
                let c = w.canvas.lock();
                let text = Some(rgb565(0xD4E6DB));
                (570..636).any(|x| (8..16).any(|y| c.pixel(x, y) == text))
                    && (5..30).any(|x| (7..16).any(|y| c.pixel(x, y) == text))
            })
        });
        if clock_drawn { break; }
        assert!(start.elapsed() < Duration::from_secs(10), "desktop never drew its clock");
        std::thread::sleep(Duration::from_millis(10));
    }
    k.composite_frame();
    let actual = p.display.last_frame().unwrap();
    // The clock shows the time of capture, so its reserved CLOCK_W area is masked.
    assert_matches_golden_masked(&actual, "desktop.ppm", Some((550, 0, 90, 24)));
}

#[test]
fn menu_dropdown_matches_golden_pixel_for_pixel() {
    let p = FakePlatform::new(FakePlatform::repo_root());
    let k = boot_with(p.clone(), Screen::WIDE);
    let manifests = std::fs::read_dir(FakePlatform::repo_root().join("v3/apps"))
        .unwrap()
        .filter(|e| e.as_ref().unwrap().file_name().to_string_lossy().ends_with(".app.toml"))
        .count();
    let text = Some(rgb565(0xD4E6DB));
    let start = std::time::Instant::now();
    // The Menu label is drawn and every app is registered.
    loop {
        let label = k.with_state(|st| {
            st.windows.in_z_order().iter().find(|w| w.app_name == DESKTOP_PATH).is_some_and(|w| {
                let c = w.canvas.lock();
                (5..30).any(|x| (7..16).any(|y| c.pixel(x, y) == text))
            })
        });
        if label && k.launcher_count() >= manifests { break; }
        assert!(start.elapsed() < Duration::from_secs(10), "desktop never drew its Menu / registered its apps");
        std::thread::sleep(Duration::from_millis(10));
    }
    p.input.set_touch(10, 10, true);
    k.poll_input();
    std::thread::sleep(Duration::from_millis(120));
    p.input.set_touch(10, 10, false);
    k.poll_input();
    // A dropdown row's text pixel appears.
    let start = std::time::Instant::now();
    loop {
        let open = k.with_state(|st| {
            st.windows.in_z_order().iter().find(|w| w.app_name == DESKTOP_PATH).is_some_and(|w| {
                let c = w.canvas.lock();
                (6..60).any(|x| (28..36).any(|y| c.pixel(x, y) == text))
            })
        });
        if open { break; }
        assert!(start.elapsed() < Duration::from_secs(10), "the Menu never opened");
        std::thread::sleep(Duration::from_millis(10));
    }
    std::thread::sleep(Duration::from_millis(200));
    k.composite_frame();
    let actual = p.display.last_frame().unwrap();
    assert_matches_golden_masked(&actual, "menu.ppm", Some((550, 0, 90, 24)));
}

#[test]
fn boot_with_spawns_the_desktop_screen_wide() {
    let p = FakePlatform::new(FakePlatform::repo_root());
    let k = boot_with(p.clone(), Screen::SVGA);
    assert_eq!(k.screen(), Screen::SVGA);
    let wins = k.with_state(|st| st.windows.in_z_order().iter().map(|w| (w.x, w.y, w.w, w.h)).collect::<Vec<_>>());
    assert_eq!(wins, [(0, 0, 800, 204)]);
}
