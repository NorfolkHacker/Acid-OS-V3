//! Every Phase 4 game, Phase 5a system app (About, Config, Network,
//! System Monitor), Phase 5b app (Terminal, File Manager) and Phase 5c app (Editor) boots under the real kernel, from its manifest, and
//! draws its first frame (the THEME_HARD bottom border) without a Lua error (an error ends the app, which
//! removes its window).

use std::time::{Duration, Instant};

use acid_gfx::rgb565;
use acid_kernel::Kernel;
use acid_kernel::theme::THEME_HARD;
use acid_os::{APPS_DIR, spawn_from_manifest};
use acid_testkit::FakePlatform;

fn boots_and_draws(name: &str) {
    let p = FakePlatform::new(FakePlatform::repo_root());
    let k = Kernel::new(p.clone());
    k.set_runner(acid_lua::lua_runner(APPS_DIR));
    let task = spawn_from_manifest(&k, name).unwrap_or_else(|| panic!("{name}: spawn"));
    // Games only draw while focused.
    k.activate_window(task);
    let start = Instant::now();
    loop {
        // The first frame is done once the bottom border pixel is THEME_HARD.
        let drawn = k.with_state(|st| {
            st.windows.by_task(task).map(|w| w.canvas.lock().pixel(w.w / 2, w.h - 1) == Some(rgb565(THEME_HARD)))
        });
        match drawn {
            Some(true) => break,
            Some(false) => {}
            None => panic!("{name}: the app exited (a Lua error?) before drawing"),
        }
        assert!(start.elapsed() < Duration::from_secs(10), "{name}: never drew");
        std::thread::sleep(Duration::from_millis(10));
    }
    std::thread::sleep(Duration::from_millis(500));
    assert!(k.with_state(|st| st.windows.by_task(task).is_some()), "{name}: died after its first frame");
    k.close_window(task);
}

#[test]
fn tetris() { boots_and_draws("tetris"); }
#[test]
fn breakout() { boots_and_draws("breakout"); }
#[test]
fn acid_blaster() { boots_and_draws("acid_blaster"); }
#[test]
fn acidstorm() { boots_and_draws("acidstorm"); }
#[test]
fn acid_snake() { boots_and_draws("acid_snake"); }
#[test]
fn acid_invaders() { boots_and_draws("acid_invaders"); }
#[test]
fn acid_spin() { boots_and_draws("acid_spin"); }
#[test]
fn piano() { boots_and_draws("piano"); }
#[test]
fn sprite_paint() { boots_and_draws("sprite"); }
#[test]
fn about() { boots_and_draws("about"); }
#[test]
fn config() { boots_and_draws("config"); }
#[test]
fn network() { boots_and_draws("network"); }
#[test]
fn sysmon() { boots_and_draws("sysmon"); }
#[test]
fn terminal() { boots_and_draws("terminal"); }
#[test]
fn file_manager() { boots_and_draws("file_manager"); }
#[test]
fn editor() { boots_and_draws("editor"); }

#[test]
fn editor_opens_a_lua_file_from_file_manager() {
    // File Manager's .lua click: acid_spawn_app("v3/apps/editor.lua", 420, 280, path).
    // The libs come from the launcher entry the desktop registered at boot.
    let p = FakePlatform::new(FakePlatform::repo_root());
    let k = acid_os::boot(p.clone());
    let start = Instant::now();
    while !(0..k.launcher_count()).any(|i| k.launcher_entry(i).is_some_and(|e| e.path == "v3/apps/editor.lua")) {
        assert!(start.elapsed() < Duration::from_secs(10), "Editor never registered");
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(k.spawn_by_path("v3/apps/editor.lua", 420, 280, Some("v3/fsroot/App/tetris.lua".into()), false));
    std::thread::sleep(Duration::from_millis(800));
    let open = k.with_state(|st| st.windows.in_z_order().iter().any(|w| w.app_name == "v3/apps/editor.lua"));
    assert!(open, "Editor stayed open on a .lua file");
}

#[test]
fn boot_discovers_every_manifest() {
    let p = FakePlatform::new(FakePlatform::repo_root());
    let k = acid_os::boot(p);
    let mut manifests: Vec<String> = std::fs::read_dir(FakePlatform::repo_root().join("v3/apps"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".app.toml"))
        .collect();
    manifests.sort();
    let start = Instant::now();
    while k.launcher_count() < manifests.len() {
        assert!(start.elapsed() < Duration::from_secs(10), "launcher has {} of {}", k.launcher_count(), manifests.len());
        std::thread::sleep(Duration::from_millis(10));
    }
    let paths: Vec<String> = (0..k.launcher_count()).map(|i| k.launcher_entry(i).unwrap().path).collect();
    let want: Vec<String> = manifests.iter().map(|m| format!("v3/apps/{}.lua", m.trim_end_matches(".app.toml"))).collect();
    assert_eq!(paths, want, "sorted by manifest name, as the desktop registers them");
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(k.with_state(|st| st.windows.count()), 1, "only the desktop is open");
}

#[test]
fn cart() { boots_and_draws("cart"); }
#[test]
fn hello_acid_runs_cart_level() {
    boots_and_draws("hello_acid");
    // The kernel doesn't expose a window's app context, so assert through the
    // trust decision: the manifest says `source = cart`, and that is what
    // spawn_from_manifest hands the runner.
    let p = FakePlatform::new(FakePlatform::repo_root());
    let k = Kernel::new(p.clone());
    k.set_runner(acid_lua::lua_runner(APPS_DIR));
    assert!(spawn_from_manifest(&k, "hello_acid").is_some());
    assert!(acid_kernel::app_is_cart(&*p, "v3/apps/hello_acid.lua"));
}

fn copy_dir(from: &std::path::Path, to: &std::path::Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let (src, dst) = (e.path(), to.join(e.file_name()));
        let ty = e.file_type().unwrap();
        if ty.is_symlink() {
            std::os::unix::fs::symlink(std::fs::read_link(&src).unwrap(), &dst).unwrap();
        } else if ty.is_dir() {
            copy_dir(&src, &dst);
        } else {
            std::fs::copy(&src, &dst).unwrap();
        }
    }
}

#[test]
fn installing_the_sample_cart_end_to_end() {
    // A throwaway copy of v3/apps + v3/carts, so the real tree is never written.
    let tmp = std::env::temp_dir().join(format!("acid-e2e-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    // Removes the temp tree even when an assertion below fails.
    struct TempTree(std::path::PathBuf);
    impl Drop for TempTree {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _tree = TempTree(tmp.clone());
    copy_dir(&FakePlatform::repo_root().join("v3/apps"), &tmp.join("v3/apps"));
    copy_dir(&FakePlatform::repo_root().join("v3/carts"), &tmp.join("v3/carts"));
    std::fs::create_dir_all(tmp.join("v3/fsroot/Home")).unwrap();
    std::fs::remove_file(tmp.join("v3/apps/hello_acid.lua")).unwrap();
    std::fs::remove_file(tmp.join("v3/apps/hello_acid.app.toml")).unwrap();
    let p = FakePlatform::new(&tmp);
    p.set_cart_roots(vec!["v3/carts".into()]);
    let k = Kernel::new(p.clone());
    k.set_runner(acid_lua::lua_runner(APPS_DIR));
    let cart = spawn_from_manifest(&k, "cart").unwrap();
    k.activate_window(cart);
    std::thread::sleep(Duration::from_millis(300));
    // roots -> v3/carts -> hello_acid.cart -> INSTALL -> RUN, by Enter presses
    for _ in 0..4 {
        p.input.push_key(acid_platform::keys::KEY_ENTER);
        k.poll_input();
        std::thread::sleep(Duration::from_millis(300));
    }
    let toml = std::fs::read_to_string(tmp.join("v3/apps/hello_acid.app.toml")).unwrap();
    assert!(toml.contains("source = cart"), "{toml}");
    let start = Instant::now();
    loop {
        let open = k.with_state(|st| st.windows.in_z_order().iter().any(|w| w.app_name == "v3/apps/hello_acid.lua"));
        if open { break; }
        assert!(start.elapsed() < Duration::from_secs(10), "the installed cart never started");
        std::thread::sleep(Duration::from_millis(20));
    }
    // The trust seam, end to end: what Load Cart installed is cart-level...
    assert!(acid_kernel::app_is_cart(&*p, "v3/apps/hello_acid.lua"), "the installed cart must be cart-level");
    // ...so an API built for it the way the kernel builds one refuses a
    // write outside Home.
    let k2 = Kernel::new(p.clone());
    let (tx, rx) = std::sync::mpsc::channel();
    let tx = std::sync::Mutex::new(tx);
    k2.set_runner(std::sync::Arc::new(move |ctx: acid_kernel::AppContext| {
        let _ = tx.lock().unwrap().send(ctx);
        loop {
            std::thread::park();
        }
    }));
    k2.spawn_app(acid_kernel::SpawnRequest {
        script_path: "v3/apps/hello_acid.lua".into(),
        x: 0, y: 30, w: 10, h: 10, closable: true, arg: None, libs: None, force_cart: false,
    })
    .unwrap();
    let ctx = rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(ctx.cart, "the kernel spawns the installed cart at cart level");
    let api = acid_api::KernelApi::new(ctx);
    use acid_api::AcidApi;
    assert_eq!(api.fs_write("v3/apps/x", b"x"), Err("read only".into()));
    assert!(!tmp.join("v3/apps/x").exists());
}
