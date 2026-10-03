//! `.wasm` scripts run through the combined runner (spec §15, Phase 6):
//! always cart-level, and every way a cart ends removes its window.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use acid_gfx::rgb565;
use acid_kernel::theme::THEME_HARD;
use acid_kernel::{Kernel, SpawnRequest, TaskId};
use acid_os::{APPS_DIR, runner, runner_with};
use acid_testkit::FakePlatform;
use acid_wasm::{WasmLimits, wasm_runner};

/// A throwaway directory tree, removed on drop even if the test panics.
struct TempTree(PathBuf);

impl TempTree {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("acid-os-wasm-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        TempTree(dir)
    }
    fn put(&self, rel: &str, bytes: &[u8]) {
        let p = self.0.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, bytes).unwrap();
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Writes `v3/apps/x` (a read-only root for a cart), shows the result as a
/// colour at (0, 16). The border goes first: it covers column 0, so drawn
/// last it would hide the result.
const WRITER: &str = r#"(module
  (import "acid" "fs_write" (func $fw (param i32 i32 i32 i32) (result i32)))
  (import "acid" "fill_rect" (func $fr (param i32 i32 i32 i32 i32)))
  (import "acid" "draw_window_border" (func $db))
  (memory (export "memory") 1)
  (data (i32.const 0) "v3/apps/x")
  (data (i32.const 16) "y")
  (func (export "acid_abi_version") (result i32) i32.const 1)
  (func (export "acid_on_create")
    call $db
    i32.const 0 i32.const 16 i32.const 1 i32.const 1
    i32.const 0 i32.const 9 i32.const 16 i32.const 1 call $fw
    i32.const 0xffffff i32.and
    call $fr)
  (func (export "acid_on_event") (param i32 i32 i32 i32))
  (func (export "acid_on_idle"))
  (func (export "acid_redraw")))"#;

const SPINNER: &str = r#"(module
  (memory (export "memory") 1)
  (func (export "acid_abi_version") (result i32) i32.const 1)
  (func (export "acid_on_create"))
  (func (export "acid_on_event") (param i32 i32 i32 i32))
  (func (export "acid_on_idle") (loop $l br $l))
  (func (export "acid_redraw")))"#;

fn spawn(k: &std::sync::Arc<Kernel>, path: &str) -> TaskId {
    k.spawn_app(SpawnRequest {
        script_path: path.into(),
        x: 0,
        y: 0,
        w: 100,
        h: 80,
        closable: true,
        arg: None,
        libs: None,
        force_cart: false,
    })
    .expect("spawn")
}

fn window_exists(k: &Kernel, task: TaskId) -> bool {
    k.with_state(|st| st.windows.by_task(task).is_some())
}

fn wait_gone(k: &Kernel, task: TaskId, what: &str) {
    let start = Instant::now();
    while window_exists(k, task) {
        assert!(start.elapsed() < Duration::from_secs(5), "{what}: window never went away");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn a_wasm_cart_runs_cart_level_and_its_window_goes_when_it_ends() {
    let tree = TempTree::new("run");
    tree.put("v3/apps/t.wasm", &wat::parse_str(WRITER).unwrap());
    let k = Kernel::new(FakePlatform::new(&tree.0));
    k.set_runner(runner(APPS_DIR));
    let task = spawn(&k, "v3/apps/t.wasm");
    let start = Instant::now();
    let pixel = loop {
        let got = k.with_state(|st| {
            let w = st.windows.by_task(task)?;
            let c = w.canvas.lock();
            (c.pixel(w.w / 2, w.h - 1) == Some(rgb565(THEME_HARD))).then(|| c.pixel(0, 16))
        });
        // The border is down once the bottom edge is; the result lands just
        // after it, so wait for the colour too.
        if let Some(p) = got.filter(|p| *p == Some(rgb565(0xfffffd))) {
            break p;
        }
        assert!(window_exists(&k, task), "the cart ended before drawing");
        assert!(start.elapsed() < Duration::from_secs(10), "never drew its border");
        std::thread::sleep(Duration::from_millis(10));
    };
    // -3 ("read only": a cart may not write under v3/apps) as 0xfffffd.
    assert_eq!(pixel, Some(rgb565(0xfffffd)));
    k.close_window(task);
    wait_gone(&k, task, "closed cart");
}

#[test]
fn a_spinning_wasm_cart_is_stopped_and_removed() {
    let tree = TempTree::new("spin");
    tree.put("v3/apps/s.wasm", &wat::parse_str(SPINNER).unwrap());
    let k = Kernel::new(FakePlatform::new(&tree.0));
    let lua = acid_lua::lua_runner(APPS_DIR);
    k.set_runner(runner_with(lua, wasm_runner(WasmLimits { fuel: 1_000_000, max_pages: 256 })));
    let task = spawn(&k, "v3/apps/s.wasm");
    wait_gone(&k, task, "spinning cart");
}

#[test]
fn lua_apps_still_run_through_the_combined_runner() {
    let k = Kernel::new(FakePlatform::new(FakePlatform::repo_root()));
    k.set_runner(runner(APPS_DIR));
    let task = acid_os::spawn_from_manifest(&k, "hello_acid").expect("spawn");
    k.activate_window(task);
    let start = Instant::now();
    loop {
        let drawn = k.with_state(|st| {
            st.windows.by_task(task).map(|w| w.canvas.lock().pixel(w.w / 2, w.h - 1) == Some(rgb565(THEME_HARD)))
        });
        match drawn {
            Some(true) => break,
            Some(false) => {}
            None => panic!("hello_acid exited before drawing"),
        }
        assert!(start.elapsed() < Duration::from_secs(10), "hello_acid never drew");
        std::thread::sleep(Duration::from_millis(10));
    }
    k.close_window(task);
}

#[test]
fn a_wasm_path_outside_the_roots_is_refused() {
    let tree = TempTree::new("refuse");
    tree.put("elsewhere/t.wasm", &wat::parse_str(WRITER).unwrap());
    let k = Kernel::new(FakePlatform::new(&tree.0));
    k.set_runner(runner(APPS_DIR));
    let task = spawn(&k, "elsewhere/t.wasm");
    wait_gone(&k, task, "refused path");
}

#[test]
fn spawn_from_manifest_runs_a_runtime_wasm_manifest_as_dot_wasm() {
    // Spec 15.4: `runtime = wasm` in the manifest means <name>.wasm.
    let tree = TempTree::new("manifest");
    tree.put("v3/apps/m.app.toml", b"name = M\nw = 100\nh = 80\nruntime = wasm\nsource = cart\n");
    tree.put("v3/apps/m.wasm", &wat::parse_str(SPINNER).unwrap());
    let k = Kernel::new(FakePlatform::new(&tree.0));
    k.set_runner(runner(APPS_DIR));
    let task = acid_os::spawn_from_manifest(&k, "m").expect("spawn");
    let name = k.with_state(|st| st.windows.by_task(task).map(|w| w.app_name.clone()));
    assert_eq!(name.as_deref(), Some("v3/apps/m.wasm"));
    k.close_window(task);
    wait_gone(&k, task, "manifest-launched cart");
}

/// A cart that only draws its border and then idles: proof the kernel still
/// runs carts after another one ended badly.
const BORDER: &str = r#"(module
  (import "acid" "draw_window_border" (func $db))
  (memory (export "memory") 1)
  (func (export "acid_abi_version") (result i32) i32.const 1)
  (func (export "acid_on_create") call $db)
  (func (export "acid_on_event") (param i32 i32 i32 i32))
  (func (export "acid_on_idle"))
  (func (export "acid_redraw")))"#;

/// Runs `bad` (a cart that must end on its own) under the real runner,
/// waits for its window to go, then checks a healthy cart still runs.
fn bad_cart_is_removed_and_the_kernel_runs_on(tag: &str, bad: &str) {
    let tree = TempTree::new(tag);
    tree.put("v3/apps/bad.wasm", &wat::parse_str(bad).unwrap());
    tree.put("v3/apps/ok.wasm", &wat::parse_str(BORDER).unwrap());
    let k = Kernel::new(FakePlatform::new(&tree.0));
    k.set_runner(runner(APPS_DIR));
    let task = spawn(&k, "v3/apps/bad.wasm");
    wait_gone(&k, task, tag);
    let ok = spawn(&k, "v3/apps/ok.wasm");
    wait_border(&k, ok, "the cart after it");
    k.close_window(ok);
    wait_gone(&k, ok, "the cart after it");
}

#[test]
fn a_trapping_wasm_cart_is_removed() {
    let trap = SPINNER.replace("(func (export \"acid_on_create\"))", "(func (export \"acid_on_create\") unreachable)");
    assert_ne!(trap, SPINNER);
    bad_cart_is_removed_and_the_kernel_runs_on("trap", &trap);
}

#[test]
fn a_wasm_cart_over_the_memory_cap_is_removed() {
    // 300 pages is past the 256-page (16 MB) cap (§15.2).
    let big = SPINNER.replace("(memory (export \"memory\") 1)", "(memory (export \"memory\") 300)");
    assert_ne!(big, SPINNER);
    bad_cart_is_removed_and_the_kernel_runs_on("oom", &big);
}

// ---- The example cart (spec 15.5) ------------------------------------------

/// The committed example cart, built from `v3/carts-src/hello-wasm`.
const HELLO_WASM: &str = "v3/carts/hello_wasm.wasm";

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

fn wait_border(k: &Kernel, task: TaskId, what: &str) {
    let start = Instant::now();
    loop {
        let drawn = k.with_state(|st| {
            st.windows.by_task(task).map(|w| w.canvas.lock().pixel(w.w / 2, w.h - 1) == Some(rgb565(THEME_HARD)))
        });
        match drawn {
            Some(true) => return,
            Some(false) => {}
            None => panic!("{what}: the cart ended before drawing"),
        }
        assert!(start.elapsed() < Duration::from_secs(10), "{what}: never drew its border");
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Runs `module` as `v3/apps/hello_wasm.wasm`, focused, and checks the
/// first bar (at (100, 20)) changes colour within 2 s: the animation runs.
fn hello_wasm_animates(module: &[u8], tag: &str) {
    let tree = TempTree::new(tag);
    tree.put("v3/apps/hello_wasm.wasm", module);
    let k = Kernel::new(FakePlatform::new(&tree.0));
    k.set_runner(runner(APPS_DIR));
    let task = k
        .spawn_app(SpawnRequest {
            script_path: "v3/apps/hello_wasm.wasm".into(),
            x: 0,
            y: 0,
            w: 200,
            h: 150,
            closable: true,
            arg: None,
            libs: None,
            force_cart: false,
        })
        .expect("spawn");
    // It steps only while focused, as hello_acid does.
    k.activate_window(task);
    wait_border(&k, task, tag);
    let bar = |k: &Kernel| k.with_state(|st| st.windows.by_task(task).and_then(|w| w.canvas.lock().pixel(100, 20)));
    let first = bar(&k);
    assert!(first.is_some(), "{tag}: no bar pixel");
    let start = Instant::now();
    while bar(&k) == first {
        assert!(window_exists(&k, task), "{tag}: the cart ended");
        assert!(start.elapsed() < Duration::from_secs(2), "{tag}: the bars never changed colour");
        std::thread::sleep(Duration::from_millis(10));
    }
    k.close_window(task);
    wait_gone(&k, task, tag);
}

#[test]
fn the_committed_example_cart_runs() {
    let module = std::fs::read(FakePlatform::repo_root().join(HELLO_WASM)).expect("v3/carts/hello_wasm.wasm");
    hello_wasm_animates(&module, "example");
}

#[test]
fn installing_the_wasm_cart_end_to_end() {
    // A throwaway copy of v3/apps + v3/carts, so the real tree is never written.
    let tree = TempTree::new("install");
    let tmp = tree.0.clone();
    copy_dir(&FakePlatform::repo_root().join("v3/apps"), &tmp.join("v3/apps"));
    copy_dir(&FakePlatform::repo_root().join("v3/carts"), &tmp.join("v3/carts"));
    std::fs::create_dir_all(tmp.join("v3/fsroot/Home")).unwrap();
    assert!(!tmp.join("v3/apps/hello_wasm.wasm").exists());
    let p = FakePlatform::new(&tmp);
    p.set_cart_roots(vec!["v3/carts".into()]);
    let k = Kernel::new(p.clone());
    k.set_runner(runner(APPS_DIR));
    let cart = acid_os::spawn_from_manifest(&k, "cart").unwrap();
    k.activate_window(cart);
    std::thread::sleep(Duration::from_millis(300));
    // roots -> v3/carts, whose listing is hello_acid.cart then
    // hello_wasm.wasm (README.txt is not a cart), so Down once -> select
    // hello_wasm.wasm -> INSTALL -> RUN.
    use acid_platform::keys::{KEY_DOWN, KEY_ENTER};
    for key in [KEY_ENTER, KEY_DOWN, KEY_ENTER, KEY_ENTER, KEY_ENTER] {
        p.input.push_key(key);
        k.poll_input();
        std::thread::sleep(Duration::from_millis(300));
    }
    let toml = std::fs::read_to_string(tmp.join("v3/apps/hello_wasm.app.toml")).unwrap();
    assert!(toml.contains("runtime = wasm"), "{toml}");
    assert!(toml.contains("source = cart"), "{toml}");
    assert_eq!(
        std::fs::read(tmp.join("v3/apps/hello_wasm.wasm")).unwrap(),
        std::fs::read(tmp.join(HELLO_WASM)).unwrap(),
        "installed byte for byte"
    );
    let start = Instant::now();
    let task = loop {
        let found = k.with_state(|st| {
            st.windows.in_z_order().iter().find(|w| w.app_name == "v3/apps/hello_wasm.wasm").map(|w| w.task)
        });
        if let Some(t) = found {
            break t;
        }
        assert!(start.elapsed() < Duration::from_secs(10), "the installed cart never started");
        std::thread::sleep(Duration::from_millis(20));
    };
    assert!(acid_kernel::app_is_cart(&*p, "v3/apps/hello_wasm.wasm"), "the installed cart must be cart-level");
    k.close_window(task);
    wait_gone(&k, task, "installed cart");
    k.close_window(cart);
    wait_gone(&k, cart, "Load Cart");
}

#[test]
fn menu_discovers_a_wasm_manifest() {
    let tree = TempTree::new("menu");
    copy_dir(&FakePlatform::repo_root().join("v3/apps"), &tree.0.join("v3/apps"));
    std::fs::create_dir_all(tree.0.join("v3/fsroot/Home")).unwrap();
    tree.put("v3/apps/zzz.app.toml", b"name = Zzz\nw = 100\nh = 80\nruntime = wasm\nsource = cart\n");
    tree.put("v3/apps/zzz.wasm", &wat::parse_str(SPINNER).unwrap());
    let k = acid_os::boot(FakePlatform::new(&tree.0));
    let start = Instant::now();
    while !(0..k.launcher_count()).any(|i| k.launcher_entry(i).is_some_and(|e| e.path == "v3/apps/zzz.wasm")) {
        assert!(start.elapsed() < Duration::from_secs(10), "the desktop never registered v3/apps/zzz.wasm");
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Builds `hello-wasm` from source and runs the fresh module. It is not
/// compared byte for byte with the committed one: a rolling toolchain
/// update changes the bytes (a ruling against spec 15.6's "byte-identical").
#[test]
fn the_example_cart_builds_from_source() {
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into());
    let sysroot = std::process::Command::new(&rustc).args(["--print", "sysroot"]).output().expect("rustc --print sysroot");
    let sysroot = PathBuf::from(String::from_utf8(sysroot.stdout).unwrap().trim());
    assert!(
        sysroot.join("lib/rustlib/wasm32-unknown-unknown").is_dir(),
        "install rust-wasm: sudo pacman -S rust-wasm"
    );
    let target = TempTree::new("build");
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let out = std::process::Command::new(cargo)
        .args(["build", "--release", "--target", "wasm32-unknown-unknown", "-p", "hello-wasm", "--manifest-path"])
        .arg(FakePlatform::repo_root().join("v3/carts-src/Cargo.toml"))
        .env("CARGO_TARGET_DIR", &target.0)
        .output()
        .expect("run cargo");
    assert!(out.status.success(), "cargo build failed:\n{}", String::from_utf8_lossy(&out.stderr));
    let module = std::fs::read(target.0.join("wasm32-unknown-unknown/release/hello_wasm.wasm")).expect("built module");
    hello_wasm_animates(&module, "fresh");
}
