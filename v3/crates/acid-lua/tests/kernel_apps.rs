use std::time::{Duration, Instant};

use acid_gfx::rgb565;
use acid_kernel::{Kernel, SpawnRequest};
use acid_lua::{lua_runner, lua_runner_trusting};
use acid_testkit::FakePlatform;

fn kernel() -> std::sync::Arc<Kernel> {
    let k = Kernel::new(FakePlatform::new(FakePlatform::repo_root()));
    k.set_runner(lua_runner_trusting("v3/apps", "v3/crates/acid-lua/tests/fixtures/"));
    k
}

fn spawn(k: &std::sync::Arc<Kernel>, script: &str) -> acid_kernel::TaskId {
    k.spawn_app(SpawnRequest {
        script_path: format!("v3/crates/acid-lua/tests/fixtures/{script}"),
        x: 0, y: 30, w: 20, h: 20, closable: true, arg: None, libs: None, force_cart: false,
    })
    .expect("spawn")
}

fn wait_until(mut f: impl FnMut() -> bool) {
    let t = Instant::now();
    while !f() {
        assert!(t.elapsed() < Duration::from_secs(5), "timed out");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn a_lua_app_draws_into_its_canvas_and_ends_on_close() {
    let k = kernel();
    let task = spawn(&k, "draw_and_wait.lua");
    wait_until(|| {
        k.with_state(|st| st.windows.by_task(task).map(|w| w.canvas.lock().pixel(5, 5)))
            == Some(Some(rgb565(0xFF0000)))
    });
    k.close_window(task);
    wait_until(|| k.with_state(|st| st.windows.count()) == 0);
}

#[test]
fn a_crashing_app_is_contained() {
    let k = kernel();
    spawn(&k, "boom.lua");
    wait_until(|| k.with_state(|st| st.windows.count()) == 0);
    // The kernel is fine: another app still runs.
    let task = spawn(&k, "draw_and_wait.lua");
    wait_until(|| k.with_state(|st| st.windows.by_task(task).is_some()));
    k.close_window(task);
}

/// Removes a fixture file even when an assertion fails.
struct Cleanup(std::path::PathBuf);
impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn assert_window_goes_away(k: &std::sync::Arc<Kernel>, path: &str) {
    let task = k
        .spawn_app(SpawnRequest { script_path: path.into(), x: 0, y: 30, w: 40, h: 40, closable: true, arg: None, libs: None, force_cart: false })
        .unwrap();
    let start = Instant::now();
    while k.with_state(|st| st.windows.by_task(task).is_some()) {
        assert!(start.elapsed() < Duration::from_secs(5), "{path}: still running");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn a_script_outside_the_roots_never_runs() {
    let repo = FakePlatform::repo_root();
    let body = std::fs::read_to_string(repo.join("v3/crates/acid-lua/tests/fixtures/draw_and_wait.lua")).unwrap();
    let pid = std::process::id();
    // Valid Lua that would stay open, so only the check keeps it from running.
    let txt = format!("v3/fsroot/Tmp/stay-open-{pid}.txt");
    let _txt = Cleanup(repo.join(&txt));
    std::fs::write(repo.join(&txt), &body).unwrap();
    let stray = format!("v3/crates/acid-lua/tests/stay-open-{pid}.lua");
    let _stray = Cleanup(repo.join(&stray));
    std::fs::write(repo.join(&stray), &body).unwrap();

    let k = Kernel::new(FakePlatform::new(&repo));
    k.set_runner(lua_runner("v3/apps"));
    for path in ["v3/crates/acid-lua/tests/fixtures/draw_and_wait.lua", "v3/fsroot/Home/notes.txt", txt.as_str()] {
        assert_window_goes_away(&k, path);
    }

    // The trusting runner still refuses paths outside both roots and its extra root.
    let k = Kernel::new(FakePlatform::new(&repo));
    k.set_runner(lua_runner_trusting("v3/apps", "v3/crates/acid-lua/tests/fixtures/"));
    assert_window_goes_away(&k, &stray);
}

fn run_with_tiny_limits(script: &str) -> (std::sync::Arc<acid_kernel::Kernel>, acid_kernel::TaskId) {
    run_with_limits(script, acid_lua::VmLimits { built_in_mem: 4 << 20, built_in_ms: 300, cart_mem: 4 << 20, cart_ms: 300 })
}

fn run_with_limits(script: &str, tiny: acid_lua::VmLimits) -> (std::sync::Arc<acid_kernel::Kernel>, acid_kernel::TaskId) {
    let p = FakePlatform::new(FakePlatform::repo_root());
    let k = Kernel::new(p);
    k.set_runner(acid_lua::lua_runner_with("v3/apps", Some("v3/crates/acid-lua/tests/fixtures/"), tiny));
    let task = k
        .spawn_app(SpawnRequest {
            script_path: format!("v3/crates/acid-lua/tests/fixtures/{script}"),
            x: 0, y: 30, w: 40, h: 40, closable: true, arg: None, libs: None, force_cart: false,
        })
        .unwrap();
    (k, task)
}

fn ends_within(k: &acid_kernel::Kernel, task: acid_kernel::TaskId, secs: u64) -> bool {
    let start = std::time::Instant::now();
    while k.with_state(|st| st.windows.by_task(task).is_some()) {
        if start.elapsed() > std::time::Duration::from_secs(secs) { return false; }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    true
}

#[test]
fn an_endless_loop_is_stopped() {
    let (k, t) = run_with_tiny_limits("spin.lua");
    assert!(ends_within(&k, t, 5), "spin.lua was never stopped");
}

#[test]
fn catching_the_watchdog_error_does_not_keep_an_app_alive() {
    let (k, t) = run_with_tiny_limits("swallow.lua");
    assert!(ends_within(&k, t, 5), "swallow.lua dodged the watchdog");
}

#[test]
fn endless_allocation_is_stopped_at_the_cap() {
    let (k, t) = run_with_tiny_limits("hog.lua");
    assert!(ends_within(&k, t, 5), "hog.lua was never stopped");
}

#[test]
fn an_app_that_polls_is_never_stopped() {
    let (k, t) = run_with_tiny_limits("draw_and_wait.lua");
    std::thread::sleep(std::time::Duration::from_millis(1200));
    assert!(k.with_state(|st| st.windows.by_task(t).is_some()), "a polling app must keep running");
    k.close_window(t);
}

fn must_end(script: &str) {
    let (k, t) = run_with_tiny_limits(script);
    assert!(ends_within(&k, t, 5), "{script} was never stopped");
}

#[test]
fn a_spin_inside_coroutine_wrap_is_stopped() { must_end("co_wrap.lua"); }

#[test]
fn a_spin_in_a_resumed_coroutine_is_stopped() { must_end("co_resume.lua"); }

#[test]
fn a_spin_in_a_nested_coroutine_is_stopped() { must_end("co_nested.lua"); }

#[test]
fn an_xpcall_message_handler_cannot_strip_the_stop() { must_end("xpcall_handler.lua"); }

#[test]
fn overriding_global_pcall_does_not_disable_the_stop() { must_end("pcall_override.lua"); }

#[test]
fn a_close_metamethod_cannot_replace_the_stop() { must_end("close_replace.lua"); }

#[test]
fn a_gc_finalizer_is_refused_so_the_app_still_ends() { must_end("gc_loop.lua"); }

#[test]
fn string_rep_of_nothing_does_not_hang() { must_end("rep_empty.lua"); }

#[test]
fn a_faked_marker_error_does_not_stop_a_polling_app() {
    let (k, t) = run_with_tiny_limits("fake_marker.lua");
    std::thread::sleep(std::time::Duration::from_millis(1200));
    assert!(k.with_state(|st| st.windows.by_task(t).is_some()), "a faked marker must not end an app");
    k.close_window(t);
}

#[test]
fn string_rep_with_a_numeric_string_count_does_not_hang() { must_end("rep_numstr.lua"); }

#[test]
fn a_looping_xpcall_handler_is_stopped() { must_end("xpcall_loop_handler.lua"); }

#[test]
fn a_looping_close_in_a_wrapped_coroutine_is_stopped() { must_end("co_close_wrap.lua"); }

#[test]
fn a_looping_close_in_a_resumed_coroutine_is_stopped() { must_end("co_close_resume.lua"); }

#[test]
fn a_huge_table_move_is_refused() { must_end("table_move.lua"); }

#[test]
fn a_callable_table_close_cannot_loop_forever() { must_end("co_close_callable.lua"); }

#[test]
fn table_insert_and_remove_refuse_a_huge_fake_length() { must_end("table_len_loop.lua"); }

#[test]
fn table_insert_and_remove_refuse_a_huge_numeric_string_length() { must_end("table_len_string.lua"); }

#[test]
fn normal_table_ops_and_callable_close_keep_an_app_alive() {
    let (k, t) = run_with_tiny_limits("table_ops_ok.lua");
    std::thread::sleep(std::time::Duration::from_millis(1200));
    assert!(k.with_state(|st| st.windows.by_task(t).is_some()), "a well-behaved app must stay alive");
    k.close_window(t);
}

#[test]
fn a_spinning_load_reader_in_a_loop_is_stopped() { must_end("load_reader.lua"); }

#[test]
fn a_spinning_load_reader_cannot_be_survived_by_polling_after() { must_end("load_reader_poll.lua"); }

#[test]
fn a_cart_level_app_gets_the_cart_limit_not_the_built_in_one() {
    // Fixtures live outside v3/apps, so they run cart-level: with a 300 ms
    // cart limit and a 5000 ms built-in one, a spin must end well before
    // the built-in limit could have fired.
    let start = Instant::now();
    let (k, t) = run_with_limits(
        "spin.lua",
        acid_lua::VmLimits { built_in_mem: 4 << 20, built_in_ms: 5000, cart_mem: 4 << 20, cart_ms: 300 },
    );
    assert!(ends_within(&k, t, 3), "a cart-level spin outlived its 300 ms cart limit");
    assert!(start.elapsed() < Duration::from_millis(3000), "ended only after {:?}", start.elapsed());
}

#[test]
fn a_loop_of_heavy_mesh_draws_is_stopped_soon_after_the_limit() {
    // Each draw is slow but only a few instructions, so the count hook alone
    // would check the clock only every few hundred draws. With a 100 ms
    // limit the app must end within 20x that.
    let k = Kernel::new(FakePlatform::new(FakePlatform::repo_root()));
    k.set_runner(acid_lua::lua_runner_with(
        "v3/apps",
        Some("v3/crates/acid-lua/tests/fixtures/"),
        acid_lua::VmLimits { built_in_mem: 4 << 20, built_in_ms: 100, cart_mem: 4 << 20, cart_ms: 100 },
    ));
    let start = Instant::now();
    let t = k
        .spawn_app(SpawnRequest {
            script_path: "v3/crates/acid-lua/tests/fixtures/mesh_heavy.lua".into(),
            x: 0, y: 30, w: 160, h: 120, closable: true, arg: None, libs: None, force_cart: false,
        })
        .unwrap();
    assert!(ends_within(&k, t, 2), "mesh_heavy.lua still running after {:?}", start.elapsed());
}

#[test]
fn an_apps_mesh_store_is_dropped_when_it_exits() {
    // The runner run_app uses, but it hands the test a Weak to the app's
    // KernelApi, which owns the mesh store.
    use acid_api::{AcidApi, KernelApi};
    let (tx, rx) = std::sync::mpsc::channel::<std::sync::Weak<dyn AcidApi>>();
    let tx = std::sync::Mutex::new(tx);
    let k = Kernel::new(FakePlatform::new(FakePlatform::repo_root()));
    k.set_runner(std::sync::Arc::new(move |ctx: acid_kernel::AppContext| {
        let kernel = ctx.kernel.clone();
        let path = ctx.script_path.clone();
        let api: std::sync::Arc<dyn AcidApi> = std::sync::Arc::new(KernelApi::new(ctx));
        tx.lock().unwrap().send(std::sync::Arc::downgrade(&api)).unwrap();
        let fs = kernel.platform().fs();
        let lua = acid_lua::new_app_state_limited(api, fs, "v3/apps", None, Some((4 << 20, 5000))).unwrap();
        acid_lua::load_file(&lua, fs, &path);
    }));
    let task = spawn(&k, "meshes_then_wait.lua");
    let weak = rx.recv_timeout(Duration::from_secs(5)).unwrap();
    // Both meshes exist while it runs.
    wait_until(|| weak.upgrade().is_some_and(|a| a.mesh_draw_cost(2, 10, 10, 64, 0, 0, 0, 0) > 0));
    assert!(weak.upgrade().unwrap().mesh_draw_cost(1, 10, 10, 64, 0, 0, 0, 0) > 0);
    k.close_window(task);
    wait_until(|| weak.upgrade().is_none());
}
