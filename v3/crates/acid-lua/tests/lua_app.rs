use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{Arc, Mutex};

use acid_api::{AcidApi, LocalTime, NetworkInfo, PolledEvent, TaskInfo, WindowInfo};
use acid_lua::{new_app_state, mlua::Lua};
use acid_testkit::{FakePlatform, StdFs};

/// Records every call; serves scripted events, then Close forever.
struct FakeApi {
    wallpaper: AtomicBool,
    events: Mutex<VecDeque<Option<PolledEvent>>>,
    calls: Mutex<Vec<String>>,
    /// Extra milliseconds the clock jumps after each poll (a late wake-up),
    /// one entry per poll; polls past the end jump 0.
    jumps: Mutex<VecDeque<i64>>,
    clock: Mutex<i64>,
    next_mesh: AtomicI32,
}

impl FakeApi {
    fn with_events(evs: Vec<Option<PolledEvent>>) -> Arc<Self> {
        Arc::new(Self { wallpaper: AtomicBool::new(true), events: Mutex::new(evs.into()), calls: Mutex::default(), jumps: Mutex::default(), clock: Mutex::new(1000), next_mesh: AtomicI32::new(1) })
    }
    fn with_jumps(evs: Vec<Option<PolledEvent>>, jumps: Vec<i64>) -> Arc<Self> {
        let api = Self::with_events(evs);
        *api.jumps.lock().unwrap() = jumps.into();
        api
    }
    fn log(&self, s: String) {
        self.calls.lock().unwrap().push(s);
    }
    fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
    fn texts(&self) -> Vec<String> {
        self.calls().into_iter().filter_map(|c| c.strip_prefix("text ").map(String::from)).collect()
    }
}

impl AcidApi for FakeApi {
    fn sound_load(&self, src: &str) -> Result<i32, String> {
        if src == "bad" { Err("1:1 unknown command 'bad'".into()) } else { Ok(3) }
    }
    fn sound_play(&self, prog: i32, name: &str, note: i32) -> Option<i32> {
        self.log(format!("sound_play {prog} {name} {note}"));
        Some(9)
    }
    fn song_parse(&self, _text: &str) -> Result<(i32, Vec<String>), String> {
        Ok((2, vec!["instrument 01: x: not found".into()]))
    }
    fn song_play(&self, song: i32, order: i32, row: i32) {
        self.log(format!("song_play {song} {order} {row}"));
    }
    fn song_position(&self) -> Option<(i32, i32, i32)> {
        Some((1, 2, 3))
    }

    fn is_cart(&self) -> bool { false }
    fn restart(&self) -> bool { self.log("restart".into()); true }
    fn local_time(&self) -> LocalTime { LocalTime { year: 2026, month: 10, day: 2, hour: 9, min: 5, sec: 7 } }
    fn mem_used_kb(&self) -> i64 { 4321 }
    fn network_info(&self) -> NetworkInfo { NetworkInfo { host: "box".into(), ip: "10.1.2.3".into(), connected: true } }
    fn refresh_tasks(&self) -> i32 { 2 }
    fn task_count(&self) -> i32 { 2 }
    fn task_info(&self, i: i64) -> Option<TaskInfo> {
        match i {
            0 => Some(TaskInfo { name: "main".into(), state: "running", cpu_percent: 1 }),
            1 => Some(TaskInfo { name: "router".into(), state: "blocked", cpu_percent: 7 }),
            _ => None,
        }
    }
    fn fs_list(&self, dir: &str) -> Result<Vec<String>, String> {
        if dir == "v3/apps" { Ok(vec!["a.app.toml".into(), "b.lua".into()]) } else { Err("bad path".into()) }
    }
    fn fs_read(&self, path: &str) -> Result<Vec<u8>, String> {
        if path == "v3/apps/a.app.toml" { Ok(b"name = A\n".to_vec()) } else { Err("not found".into()) }
    }
    fn fs_size(&self, path: &str) -> Result<u64, String> {
        if path == "v3/apps/a.app.toml" { Ok(9) } else { Err("not found".into()) }
    }
    fn fs_write(&self, path: &str, data: &[u8]) -> Result<(), String> {
        self.log(format!("write {path} {}", String::from_utf8_lossy(data)));
        if path == "v3/fsroot/Home/ok.txt" { Ok(()) } else { Err("bad path".into()) }
    }
    fn fs_rename(&self, from: &str, to: &str) -> Result<(), String> {
        self.log(format!("rename {from} {to}"));
        Ok(())
    }
    fn cart_roots(&self) -> Result<Vec<String>, String> { Ok(vec!["v3/carts".into()]) }
    fn cart_list(&self, dir: &str) -> Result<Vec<String>, String> {
        if dir == "v3/carts" { Ok(vec!["a.cart".into()]) } else { Err("bad path".into()) }
    }
    fn cart_stat(&self, path: &str) -> Result<(bool, u64), String> {
        if path == "v3/carts/a.cart" { Ok((false, 11)) } else { Err("bad path".into()) }
    }
    fn cart_read(&self, path: &str) -> Result<Vec<u8>, String> {
        if path == "v3/carts/a.cart" { Ok(b"-- name: A\n".to_vec()) } else { Err("bad path".into()) }
    }
    fn fs_delete(&self, path: &str) -> Result<(), String> {
        self.log(format!("delete {path}"));
        Err("not found".into())
    }
    fn poll_event(&self, ms: i64) -> Option<PolledEvent> {
        self.log(format!("poll {ms}"));
        let ev = self.events.lock().unwrap().pop_front().unwrap_or(Some(PolledEvent::Close));
        if ev.is_none() {
            // A timeout: the call blocked for its full duration.
            *self.clock.lock().unwrap() += ms;
        }
        *self.clock.lock().unwrap() += self.jumps.lock().unwrap().pop_front().unwrap_or(0);
        ev
    }
    fn now_ms(&self) -> i64 { *self.clock.lock().unwrap() }
    fn notify_redraw_done(&self) { self.log("notify".into()) }
    fn fill_rect(&self, x: i32, y: i32, w: i32, h: i32, c: u32) { self.log(format!("fill_rect {x} {y} {w} {h} {c:#08x}")) }
    fn draw_line(&self, x1: i32, y1: i32, x2: i32, y2: i32, c: u32) { self.log(format!("line {x1} {y1} {x2} {y2} {c:#08x}")) }
    fn fill_triangle(&self, x1: i32, y1: i32, x2: i32, y2: i32, x3: i32, y3: i32, c: u32) { self.log(format!("tri {x1} {y1} {x2} {y2} {x3} {y3} {c:#08x}")) }
    fn mesh_builtin(&self, name: &str) -> Result<i32, String> {
        if acid_gfx::three_d::BUILTIN_NAMES.contains(&name) { Ok(self.next_mesh.fetch_add(1, Ordering::SeqCst)) } else { Err("unknown".into()) }
    }
    fn mesh_new(&self, points: Vec<(i32, i32, i32)>, faces: Vec<[u16; 4]>) -> Result<i32, String> {
        self.log(format!("mesh_new {points:?} {faces:?}"));
        Ok(self.next_mesh.fetch_add(1, Ordering::SeqCst))
    }
    fn mesh_draw(&self, id: i32, x: i32, y: i32, size: i32, rx: i32, ry: i32, rz: i32, mode: i32, c: u32) {
        self.log(format!("mesh_draw {id} {x} {y} {size} {rx} {ry} {rz} {mode} {c:#08x}"))
    }
    fn mesh_draw_cost(&self, _: i32, _: i32, _: i32, _: i32, _: i32, _: i32, _: i32, _: i32) -> u64 { 0 }
    fn mesh_free(&self, id: i32) { self.log(format!("mesh_free {id}")) }
    fn fill_circle(&self, x: i32, y: i32, r: i32, c: u32) { self.log(format!("fill_circle {x} {y} {r} {c:#08x}")) }
    fn draw_text(&self, t: &str, _x: i32, _y: i32, _fg: u32, _bg: u32) { self.log(format!("text {t}")) }
    fn draw_window_frame(&self, t: &str) { self.log(format!("frame {t}")) }
    fn draw_window_border(&self) { self.log("border".into()) }
    fn clear_user_area(&self) { self.log("clear".into()) }
    fn am_i_focused(&self) -> bool { false }
    fn launch_arg(&self) -> String { "arg!".into() }
    fn play_note(&self, v: i32, o: i32, vol: i32) { self.log(format!("play {v} {o} {vol}")) }
    fn stop_note(&self, v: i32) { self.log(format!("stop {v}")) }
    fn configure_voice(&self, v: i32, r: i32, a: i32, d: i32, s: i32, rel: i32) { self.log(format!("voice {v} {r} {a} {d} {s} {rel}")) }
    fn configure_filter(&self, c: i32, r: i32, m: i32) { self.log(format!("filter {c} {r} {m}")) }
    fn trigger_arp(&self, v: i32, n: [i32; 4], c: i32, ms: i32) { self.log(format!("arp {v} {:?} {c} {ms}", n)) }
    fn configure_osc(&self, v: i32, w: i32, d: i32) { self.log(format!("osc {v} {w} {d}")) }
    fn set_ring_partner(&self, v: i32, p: i32) { self.log(format!("ring {v} {p}")) }
    fn set_volume(&self, p: i32) { self.log(format!("volume {p}")) }
    fn volume(&self) -> i32 { 65 }
    fn active_voice_count(&self) -> i32 { 3 }
    fn overlay_open(&self) -> bool { self.log("overlay_open".into()); true }
    fn overlay_clear(&self) { self.log("overlay_clear".into()) }
    fn overlay_fill_rect(&self, x: i32, y: i32, w: i32, h: i32, c: u32) { self.log(format!("overlay_fill_rect {x} {y} {w} {h} {c:#08x}")) }
    fn overlay_close(&self) { self.log("overlay_close".into()) }
    fn repaint_region(&self, x: i32, y: i32, w: i32, h: i32) { self.log(format!("repaint {x} {y} {w} {h}")) }
    fn set_wallpaper_enabled(&self, on: bool) {
        self.wallpaper.store(on, Ordering::SeqCst);
        self.log(format!("wallpaper {on}"))
    }
    fn wallpaper_enabled(&self) -> bool { self.wallpaper.load(Ordering::SeqCst) }
    fn window_max(&self) -> i32 { 8 }
    fn screen_size(&self) -> (i32, i32) { (640, 360) }
    fn font_size(&self) -> (i32, i32) { (6, 8) }
    fn window_size(&self) -> (i32, i32) { (200, 150) }
    fn font_scale(&self) -> i32 { 1 }
    fn set_font_scale(&self, n: i32) { self.log(format!("font_scale {n}")) }
    fn window_info(&self, i: i64) -> Option<WindowInfo> {
        (i == 2).then(|| WindowInfo { app_name: "v3/apps/x.lua".into(), x: 1, y: 2, w: 3, h: 4, focused: true })
    }
    fn activate_window(&self, i: i64) { self.log(format!("activate {i}")) }
    fn close_window(&self, i: i64) -> bool { self.log(format!("close {i}")); i == 1 }
    fn send_self_to_back(&self) { self.log("to_back".into()) }
    fn launcher_register(&self, path: &str, name: &str, w: i32, h: i32, multi: bool, libs: &str) -> bool {
        self.log(format!("register {path} {name} {w} {h} {multi} [{libs}]"));
        true
    }
    fn launcher_count(&self) -> i32 { 1 }
    fn launcher_path(&self, i: i64) -> Option<String> { (i == 0).then(|| "v3/apps/x.lua".into()) }
    fn launcher_name(&self, i: i64) -> Option<String> { (i == 0).then(|| "X".into()) }
    fn launcher_spawn(&self, i: i64) -> bool { self.log(format!("launch {i}")); i == 0 }
    fn spawn_app(&self, path: &str, w: i32, h: i32, arg: &str) -> bool {
        self.log(format!("spawn {path} {w} {h} [{arg}]"));
        true
    }
    fn composited_frames(&self) -> u32 { 7 }
    fn skipped_frames(&self) -> u32 { 9 }
}

fn state(api: Arc<FakeApi>) -> Lua {
    let fs = StdFs::new(FakePlatform::repo_root());
    new_app_state(api, &fs, "v3/apps", None).expect("lua state")
}

fn run(lua: &Lua, src: &str) {
    lua.load(src).exec().expect("script ran");
}

#[test]
fn window_title_splits_camel_case_and_truncates() {
    let api = FakeApi::with_events(vec![]);
    let lua = state(api.clone());
    run(&lua, r#"
        acid_draw_text(AcidApp:extend("DemoTouchApp"):new():window_title(), 0, 0, 0, 0)
        acid_draw_text(AcidApp:extend("VeryLongNameForAWindowApp"):new():window_title(), 0, 0, 0, 0)
        acid_draw_text(AcidApp:extend("Terminal"):new():window_title(), 0, 0, 0, 0)
    "#);
    assert_eq!(api.texts(), ["Demo Touch", "Very Long Name F", "Terminal"]);
}

#[test]
fn run_loop_dispatches_each_event_kind() {
    let api = FakeApi::with_events(vec![
        Some(PolledEvent::Touch { x: 1, y: 2, pressed: true }),
        Some(PolledEvent::Key { code: 65, pressed: true }),
        None,
        Some(PolledEvent::Moved),
        Some(PolledEvent::Close),
    ]);
    let lua = state(api.clone());
    run(&lua, r#"
        local T = AcidApp:extend("TApp")
        function T:on_create() acid_draw_text("create", 0, 0, 0, 0) end
        function T:redraw() acid_draw_text("redraw", 0, 0, 0, 0) end
        function T:on_touch(x, y, p) acid_draw_text("touch " .. x .. " " .. y .. " " .. tostring(p), 0, 0, 0, 0) end
        function T:on_key(c, p) acid_draw_text("key " .. c .. " " .. tostring(p), 0, 0, 0, 0) end
        function T:on_idle() acid_draw_text("idle", 0, 0, 0, 0) end
        function T:on_destroy() acid_draw_text("destroy", 0, 0, 0, 0) end
        T:new():start()
    "#);
    assert_eq!(api.texts(), ["create", "redraw", "touch 1 2 true", "key 65 true", "idle", "redraw", "destroy"]);
    assert!(api.calls().contains(&"notify".to_string()), "moved -> redraw + notify");
}

#[test]
fn resized_polls_with_its_size() {
    let api = FakeApi::with_events(vec![Some(PolledEvent::Resized { w: 300, h: 200 })]);
    let lua = state(api.clone());
    run(&lua, r#"
        local k, w, h = acid_poll_event(0)
        acid_draw_text(k .. " " .. w .. "x" .. h, 0, 0, 0, 0)
    "#);
    assert_eq!(api.texts(), ["resized 300x200"]);
}

#[test]
fn run_loop_calls_on_resize_before_redraw() {
    let api = FakeApi::with_events(vec![Some(PolledEvent::Resized { w: 300, h: 200 }), Some(PolledEvent::Close)]);
    let lua = state(api.clone());
    run(&lua, r#"
        local T = AcidApp:extend("TApp")
        function T:redraw() acid_draw_text("redraw", 0, 0, 0, 0) end
        function T:on_resize(w, h) acid_draw_text("resize " .. w .. " " .. h, 0, 0, 0, 0) end
        T:new():start()
    "#);
    assert_eq!(api.texts(), ["redraw", "resize 300 200", "redraw"]);
    assert!(api.calls().contains(&"notify".to_string()));
}

#[test]
fn quit_ends_the_loop_from_inside() {
    let api = FakeApi::with_events(vec![None, None, None]);
    let lua = state(api.clone());
    run(&lua, r#"
        local T = AcidApp:extend("TApp")
        function T:redraw() end
        function T:on_idle() self:quit() end
        function T:on_destroy() acid_draw_text("destroy", 0, 0, 0, 0) end
        T:new():start()
    "#);
    let polls = api.calls().iter().filter(|c| c.starts_with("poll")).count();
    assert_eq!(polls, 1);
    assert_eq!(api.texts(), ["destroy"]);
}

#[test]
fn poll_timeout_is_clamped_to_at_least_one() {
    for (ret, expect) in [("0", "poll 1"), ("-5", "poll 1"), ("40", "poll 40")] {
        let api = FakeApi::with_events(vec![]);
        let lua = state(api.clone());
        run(&lua, &format!(r#"
            local T = AcidApp:extend("TApp")
            function T:redraw() end
            function T:poll_timeout_ms() return {ret} end
            T:new():start()
        "#));
        assert_eq!(api.calls().iter().find(|c| c.starts_with("poll")).unwrap(), expect);
    }
}

#[test]
fn default_redraw_draws_chrome() {
    let api = FakeApi::with_events(vec![]);
    let lua = state(api.clone());
    run(&lua, r#"AcidApp:extend("HelloAcidApp"):new():start()"#);
    let calls = api.calls();
    assert_eq!(&calls[..3], ["clear", "frame Hello Acid", "border"]);
}

#[test]
fn palette_hue_matches_spot_values() {
    let api = FakeApi::with_events(vec![]);
    let lua = state(api.clone());
    run(&lua, r#"
        acid_draw_text(tostring(AcidPalette.hue(0)), 0, 0, 0, 0)
        acid_draw_text(tostring(AcidPalette.hue(64)), 0, 0, 0, 0)
        acid_draw_text(tostring(AcidPalette.hue(-1)), 0, 0, 0, 0)
        acid_draw_text(tostring(AcidPalette.hue(30, 60)), 0, 0, 0, 0)
    "#);
    // hue(0)=0xFF0000; hue(64): h=90 -> (128,255,0); hue(-1): step 255,
    // h=358 -> (255,0,9); hue(30,60): h=180 -> sector 3, f=0 -> (0,255,255).
    assert_eq!(api.texts(), ["16711680", "8453888", "16711689", "65535"]);
}

#[test]
fn sandbox_has_no_io_os_package_or_file_loaders() {
    let api = FakeApi::with_events(vec![]);
    let lua = state(api.clone());
    run(&lua, r#"
        acid_draw_text(tostring(io) .. tostring(os) .. tostring(package) .. tostring(dofile) .. tostring(loadfile), 0, 0, 0, 0)
        acid_draw_text(type(string) .. type(table) .. type(math) .. type(utf8) .. type(coroutine), 0, 0, 0, 0)
    "#);
    assert_eq!(api.texts(), ["nilnilnilnilnil", "tabletabletabletabletable"]);
}

#[test]
fn sandbox_refuses_binary_chunks() {
    let api = FakeApi::with_events(vec![]);
    let lua = state(api.clone());
    // REAL bytecode, built host-side (string.dump is gone in the sandbox).
    // A truncated header would be rejected by Lua itself in any mode, so
    // only a valid chunk shows that the mode-"t" wrapper is doing the work.
    let bc = lua.load("return 42").into_function().unwrap().dump(true);
    lua.globals().set("BC", lua.create_string(&bc).unwrap()).unwrap();
    run(&lua, r#"
        assert(require == nil and string.dump == nil)
        local f, err = load(BC)
        assert(f == nil and err:find("attempt to load a binary chunk", 1, true), tostring(err))
        -- an explicit mode argument can't re-enable binary either
        local g, err2 = load(BC, "x", "b")
        assert(g == nil and err2:find("attempt to load a binary chunk", 1, true), tostring(err2))
        assert(load("return 1")() == 1)
        -- the env argument still works, and stays absent when not given
        assert(load("return x", "c", "t", { x = 7 })() == 7)
        acid_draw_text("ok", 0, 0, 0, 0)
    "#);
    assert_eq!(api.texts(), ["ok"]);
}

/// Serves one file whose contents are real, valid Lua bytecode.
struct BytecodeFs(Vec<u8>);

impl acid_platform::Fs for BytecodeFs {
    fn read(&self, _: &str) -> Result<Vec<u8>, acid_platform::FsError> {
        Ok(self.0.clone())
    }
    fn list(&self, _: &str) -> Result<Vec<String>, acid_platform::FsError> {
        Ok(vec![])
    }
    fn size(&self, _: &str) -> Result<u64, acid_platform::FsError> {
        Ok(self.0.len() as u64)
    }
    fn write(&self, _: &str, _: &[u8]) -> Result<(), acid_platform::FsError> { Ok(()) }
    fn rename(&self, _: &str, _: &str) -> Result<(), acid_platform::FsError> { Ok(()) }
    fn delete(&self, _: &str) -> Result<(), acid_platform::FsError> { Ok(()) }
}

#[test]
fn load_file_refuses_a_binary_script() {
    let api = FakeApi::with_events(vec![]);
    let lua = state(api.clone());
    let bytecode = lua.load("acid_draw_text('ran', 0, 0, 0, 0)").into_function().unwrap().dump(true);
    assert!(bytecode.starts_with(b"\x1bLua"));
    assert!(!acid_lua::load_file(&lua, &BytecodeFs(bytecode), "evil.lua"));
    assert!(api.texts().is_empty(), "bytecode must not run");
}

#[test]
fn launch_arg_and_event_values_reach_lua() {
    let api = FakeApi::with_events(vec![None]);
    let lua = state(api.clone());
    run(&lua, r#"
        acid_draw_text(acid_launch_arg(), 0, 0, 0, 0)
        local timed_out = acid_poll_event(5)   -- no values at all -> nil
        acid_draw_text(tostring(timed_out), 0, 0, 0, 0)
        acid_draw_text(acid_poll_event(5), 0, 0, 0, 0)
    "#);
    assert_eq!(api.texts(), ["arg!", "nil", "close"]);
}

#[test]
fn manifest_libs_load_and_unsafe_ones_are_refused() {
    let api = FakeApi::with_events(vec![]);
    let fs = StdFs::new(FakePlatform::repo_root());
    let lua = new_app_state(api.clone(), &fs, "v3/crates/acid-lua/tests/fixtures/apps", Some(" extra.lua , ../escape.lua,, ")).unwrap();
    run(&lua, r#"acid_draw_text(tostring(EXTRA) .. tostring(ESCAPED), 0, 0, 0, 0)"#);
    assert_eq!(api.texts(), ["1nil"]);
}

#[test]
fn overlay_and_wallpaper_bindings() {
    let api = FakeApi::with_events(vec![]);
    let lua = state(api.clone());
    run(&lua, r#"
        acid_draw_text(tostring(acid_overlay_open()), 0, 0, 0, 0)
        acid_overlay_fill_rect(-1, 2, 3, 4, 0xFF00FF)
        acid_overlay_clear()
        acid_overlay_close()
        acid_repaint_region(0, 24, 640, 180)
        acid_draw_text(tostring(acid_get_wallpaper_enabled()), 0, 0, 0, 0)
        acid_set_wallpaper_enabled(false)
        acid_draw_text(tostring(acid_get_wallpaper_enabled()), 0, 0, 0, 0)
    "#);
    let calls = api.calls();
    for want in ["overlay_open", "overlay_fill_rect -1 2 3 4 0xff00ff", "overlay_clear", "overlay_close",
                 "repaint 0 24 640 180", "wallpaper false"] {
        assert!(calls.contains(&want.to_string()), "missing {want:?} in {calls:?}");
    }
    assert_eq!(api.texts(), ["true", "true", "false"]);
}

#[test]
fn window_bindings_use_ids_and_multi_return() {
    let api = FakeApi::with_events(vec![]);
    let lua = state(api.clone());
    run(&lua, r#"
        acid_draw_text(tostring(acid_window_max()), 0, 0, 0, 0)
        local n, x, y, w, h, f = acid_window_info(2)
        acid_draw_text(table.concat({ n, x, y, w, h, tostring(f) }, ","), 0, 0, 0, 0)
        acid_draw_text(tostring(select('#', acid_window_info(0))), 0, 0, 0, 0)
        acid_activate_window(3)
        acid_draw_text(tostring(acid_close_window(1)) .. tostring(acid_close_window(0)), 0, 0, 0, 0)
        acid_send_self_to_back()
        acid_draw_text(acid_composited_frames() .. "/" .. acid_skipped_frames(), 0, 0, 0, 0)
    "#);
    assert_eq!(api.texts(), ["8", "v3/apps/x.lua,1,2,3,4,true", "0", "truefalse", "7/9"]);
    assert!(api.calls().contains(&"activate 3".to_string()));
    assert!(api.calls().contains(&"to_back".to_string()));
}

#[test]
fn restart_binding_returns_the_api_result() {
    let api = FakeApi::with_events(vec![]);
    let lua = state(api.clone());
    run(&lua, r#"acid_draw_text(tostring(acid_restart()), 0, 0, 0, 0)"#);
    assert_eq!(api.texts(), ["true"]);
    assert_eq!(api.calls().iter().filter(|c| *c == "restart").count(), 1);
}

#[test]
fn launcher_and_spawn_bindings() {
    let api = FakeApi::with_events(vec![]);
    let lua = state(api.clone());
    run(&lua, r#"
        acid_draw_text(tostring(acid_launcher_register("v3/apps/x.lua", "X", 100, 80, true, "lib/a.lua")), 0, 0, 0, 0)
        acid_launcher_register("v3/apps/y.lua", "Y", 10, 20, false, nil)
        acid_draw_text(acid_launcher_count() .. " " .. acid_launcher_path(0) .. " " .. acid_launcher_name(0), 0, 0, 0, 0)
        acid_draw_text(tostring(acid_launcher_path(1)), 0, 0, 0, 0)
        acid_draw_text(tostring(acid_launcher_spawn(0)) .. tostring(acid_launcher_spawn(4)), 0, 0, 0, 0)
        acid_draw_text(tostring(acid_spawn_app("v3/apps/e.lua", 100, 80, "notes.txt")), 0, 0, 0, 0)
        acid_spawn_app("v3/apps/f.lua", 100, 80, nil)
    "#);
    assert_eq!(api.texts(), ["true", "1 v3/apps/x.lua X", "nil", "truefalse", "true"]);
    let calls = api.calls();
    for want in ["register v3/apps/x.lua X 100 80 true [lib/a.lua]", "register v3/apps/y.lua Y 10 20 false []",
                 "spawn v3/apps/e.lua 100 80 [notes.txt]", "spawn v3/apps/f.lua 100 80 []"] {
        assert!(calls.contains(&want.to_string()), "missing {want:?} in {calls:?}");
    }
}

#[test]
fn audio_bindings_pass_their_arguments_through() {
    let api = FakeApi::with_events(vec![]);
    let lua = state(api.clone());
    run(&lua, r#"
        acid_play_note(0, 49, 100)
        acid_stop_note(0)
        acid_configure_voice(1, 2, 5, 40, 60, 80)
        acid_configure_filter(60, 12, 7)
        acid_trigger_arp(2, 40, 44, 47, 52, 4, 30)
        acid_configure_osc(3, 2, 50)
        acid_set_ring_partner(3, -1)
        acid_set_volume(40)
        acid_draw_text(acid_get_volume() .. " " .. acid_active_voice_count(), 0, 0, 0, 0)
    "#);
    let calls = api.calls();
    // Exact sequence, and within each call every argument is distinct, so a
    // swapped or mis-wired argument changes the log.
    assert_eq!(calls, [
        "play 0 49 100", "stop 0", "voice 1 2 5 40 60 80", "filter 60 12 7",
        "arp 2 [40, 44, 47, 52] 4 30", "osc 3 2 50", "ring 3 -1", "volume 40", "text 65 3",
    ]);
    assert_eq!(api.texts(), ["65 3"]);
}

#[test]
fn acid_game_ticks_every_tick_ms_on_the_clock() {
    // AcidGame paces on_tick by the clock, not by poll results.
    let api = FakeApi::with_events(vec![None, None, None]);
    let lua = state(api.clone());
    run(&lua, r#"
        local T = AcidGame:extend("TGame")
        function T:on_create() self.ticks = 0 end
        function T:on_tick()
          self.ticks = self.ticks + 1
          acid_draw_text("tick " .. self.ticks .. " at " .. acid_now_ms(), 0, 0, 0, 0)
        end
        T:new():start()
    "#);
    assert_eq!(api.texts(), ["tick 1 at 1050", "tick 2 at 1100", "tick 3 at 1150"]);
    let polls: Vec<String> = api.calls().into_iter().filter(|c| c.starts_with("poll")).collect();
    assert_eq!(polls, ["poll 50", "poll 50", "poll 50", "poll 50"]);
}

#[test]
fn acid_game_resyncs_instead_of_bursting_after_a_late_poll() {
    // The catch-up rule: a tick more than a full tick late fires once and
    // reschedules at now + TICK_MS, not a burst of missed ticks.
    let api = FakeApi::with_jumps(vec![None, None], vec![0, 300]);
    let lua = state(api.clone());
    run(&lua, r#"
        local T = AcidGame:extend("TGame")
        function T:on_tick() acid_draw_text("tick at " .. acid_now_ms(), 0, 0, 0, 0) end
        T:new():start()
    "#);
    // Second poll times out at 1100, then the clock jumps 300 to 1400: one
    // tick (not 6), and the next is due at 1450, so the third poll waits 50.
    assert_eq!(api.texts(), ["tick at 1050", "tick at 1400"]);
    let polls: Vec<String> = api.calls().into_iter().filter(|c| c.starts_with("poll")).collect();
    assert_eq!(polls, ["poll 50", "poll 50", "poll 50"]);
}

#[test]
fn acid_game_ignores_quit_and_runs_until_close() {
    // AcidGame:start keeps its own `running`, so quit() has no effect.
    let api = FakeApi::with_events(vec![None, None, None]);
    let lua = state(api.clone());
    run(&lua, r#"
        local T = AcidGame:extend("TGame")
        function T:on_tick() self:quit(); acid_draw_text("tick", 0, 0, 0, 0) end
        function T:on_destroy() acid_draw_text("destroy", 0, 0, 0, 0) end
        T:new():start()
    "#);
    assert_eq!(api.texts(), ["tick", "tick", "tick", "destroy"]);
}

#[test]
fn core_libs_define_keys_and_waveforms() {
    let api = FakeApi::with_events(vec![]);
    let lua = state(api.clone());
    run(&lua, r#"
        acid_draw_text(AcidKeys.ENTER .. " " .. AcidKeys.RIGHT .. " " .. AcidWaveform.NOISE .. " " .. AcidGame.TICK_MS, 0, 0, 0, 0)
    "#);
    assert_eq!(api.texts(), ["257 265 3 50"]);
}

#[test]
fn system_info_and_fs_bindings_use_multi_return_and_nil_err() {
    let lua = state(FakeApi::with_events(vec![]));
    run(&lua, r#"
      local y, mo, d, h, mi, s = acid_local_time()
      assert(y == 2026 and mo == 10 and d == 2 and h == 9 and mi == 5 and s == 7, "local time")
      assert(acid_mem_used_kb() == 4321, "mem")
      local host, ip, up = acid_network_info()
      assert(host == "box" and ip == "10.1.2.3" and up == true, "network")
      assert(acid_refresh_tasks() == 2 and acid_task_count() == 2, "task counts")
      local n, st, cpu = acid_task_info(1)
      assert(n == "router" and st == "blocked" and cpu == 7, "task info is 0-based")
      assert(select('#', acid_task_info(2)) == 0, "no such task returns nothing")
      local names = acid_fs_list("v3/apps")
      assert(#names == 2 and names[1] == "a.app.toml" and names[2] == "b.lua", "fs_list")
      local none, err = acid_fs_list("/etc")
      assert(none == nil and err == "bad path", "fs_list error")
      assert(acid_fs_read("v3/apps/a.app.toml") == "name = A\n", "fs_read")
      local gone, why = acid_fs_read("v3/apps/missing")
      assert(gone == nil and why == "not found", "fs_read error")
    "#);
}

#[test]
fn write_bindings_return_true_or_nil_err() {
    let api = FakeApi::with_events(vec![]);
    let lua = state(api.clone());
    run(&lua, r#"
      assert(acid_fs_write("v3/fsroot/Home/ok.txt", "a\0b") == true, "write ok")
      local n, e = acid_fs_write("v3/x", "y")
      assert(n == nil and e == "bad path", "write error")
      assert(acid_fs_rename("v3/fsroot/a", "v3/fsroot/b") == true, "rename ok")
      local d, de = acid_fs_delete("v3/fsroot/gone")
      assert(d == nil and de == "not found", "delete error")
    "#);
    assert!(api.calls().iter().any(|c| c == "write v3/fsroot/Home/ok.txt a\u{0}b"), "write is binary-safe: {:?}", api.calls());
}

#[test]
fn cart_bindings_shapes() {
    let lua = state(FakeApi::with_events(vec![]));
    run(&lua, r#"
      local roots = acid_cart_roots()
      assert(#roots == 1 and roots[1] == "v3/carts", "roots")
      assert(acid_cart_list("v3/carts")[1] == "a.cart", "list")
      local kind, size = acid_cart_stat("v3/carts/a.cart")
      assert(kind == "file" and size == 11 and math.type(size) == "integer", "stat")
      assert(acid_cart_read("v3/carts/a.cart") == "-- name: A\n", "read")
      local n, e = acid_cart_read("/etc/passwd")
      assert(n == nil and e == "bad path", "error")
    "#);
}

#[test]
fn fs_size_binding_returns_bytes_or_nil_err() {
    let lua = state(FakeApi::with_events(vec![]));
    run(&lua, r#"
      assert(acid_fs_size("v3/apps/a.app.toml") == 9, "size")
      assert(math.type(acid_fs_size("v3/apps/a.app.toml")) == "integer", "an integer")
      local none, err = acid_fs_size("v3/apps/missing")
      assert(none == nil and err == "not found", "error")
    "#);
}

#[test]
fn canonical_app_path_maps_the_fsroot_app_symlink_back() {
    let lua = state(FakeApi::with_events(vec![]));
    run(&lua, r#"
      assert(AcidApp:canonical_app_path("v3/fsroot/Source/x.lua") == "v3/apps/x.lua", "mapped")
      assert(AcidApp:canonical_app_path("v3/apps/y.lua") == "v3/apps/y.lua", "canonical unchanged")
      assert(AcidApp:canonical_app_path("v3/fsroot/Home/n.txt") == "v3/fsroot/Home/n.txt", "other fsroot unchanged")
    "#);
}

#[test]
fn font_and_window_size_bindings() {
    let api = FakeApi::with_events(vec![]);
    let lua = state(api.clone());
    run(&lua, r#"
        local fw, fh = acid_font_size()
        local ww, wh = acid_window_size()
        acid_draw_text(fw .. "x" .. fh .. " " .. ww .. "x" .. wh .. " " .. acid_get_font_scale(), 0, 0, 0, 0)
        acid_set_font_scale(2)
    "#);
    assert_eq!(api.texts(), ["6x8 200x150 1"]);
    assert!(api.calls().contains(&"font_scale 2".to_string()));
}

#[test]
fn screen_size_returns_width_and_height() {
    let api = FakeApi::with_events(vec![]);
    let lua = state(api.clone());
    run(&lua, r#"
        local w, h = acid_screen_size()
        acid_draw_text(w .. "x" .. h, 0, 0, 0, 0)
    "#);
    assert_eq!(api.texts(), ["640x360"]);
}

#[test]
fn line_and_triangle_reach_the_api() {
    let api = FakeApi::with_events(vec![]);
    let lua = state(api.clone());
    run(&lua, "acid_draw_line(1, 2, 3, 4, 0xFF0000)  acid_fill_triangle(1, 2, 3, 4, 5, 6, 0x00FF00)");
    let calls = api.calls();
    assert!(calls.contains(&"line 1 2 3 4 0xff0000".to_string()), "{calls:?}");
    assert!(calls.contains(&"tri 1 2 3 4 5 6 0x00ff00".to_string()), "{calls:?}");
}

#[test]
fn mesh_new_converts_one_based_faces_to_zero_based() {
    let api = FakeApi::with_events(vec![]);
    let lua = state(api.clone());
    run(&lua, r#"
        local id = acid_mesh_new({0,0,0, 10,0,0, 0,10,0}, {{1,2,3}})
        local q = acid_mesh_new({0,0,0, 10,0,0, 0,10,0, 5,5,5}, {{1,2,3,4}})
        acid_draw_text(id .. "," .. q, 0, 0, 0, 0)
        acid_mesh_draw(id, 10, 20, 64, 1, 2, 3, 2, 0xABCDEF)
        acid_mesh_free(id)
    "#);
    let calls = api.calls();
    assert!(calls.contains(&"mesh_new [(0, 0, 0), (10, 0, 0), (0, 10, 0)] [[0, 1, 2, 65535]]".to_string()), "{calls:?}");
    assert!(calls.contains(&"mesh_new [(0, 0, 0), (10, 0, 0), (0, 10, 0), (5, 5, 5)] [[0, 1, 2, 3]]".to_string()), "{calls:?}");
    assert_eq!(api.texts(), ["1,2"]);
    assert!(calls.contains(&"mesh_draw 1 10 20 64 1 2 3 2 0xabcdef".to_string()), "{calls:?}");
    assert!(calls.contains(&"mesh_free 1".to_string()), "{calls:?}");
}

#[test]
fn mesh_new_rejects_bad_shapes_before_the_api() {
    let api = FakeApi::with_events(vec![]);
    let lua = state(api.clone());
    run(&lua, r#"
        local cases = {
            {{0,0,0, 1,0}, {{1,2,3}}},
            {{0,0,0, 1,0,0, 0,1,0}, {{1,2}}},
            {{0,0,0, 1,0,0, 0,1,0}, {{1,2,3,1,2}}},
            {{0,0,0, 1,0,0, 0,1,0}, {{0,1,2}}},
            {{0,0,0, 1,0,0, 0,1,0}, {{-1,1,2}}},
        }
        for _, c in ipairs(cases) do
            local id, err = acid_mesh_new(c[1], c[2])
            acid_draw_text(tostring(id) .. ":" .. tostring(err), 0, 0, 0, 0)
        end
    "#);
    assert_eq!(api.texts(), vec!["nil:bad mesh"; 5]);
    assert!(!api.calls().iter().any(|c| c.starts_with("mesh_new")), "{:?}", api.calls());
}

#[test]
fn mesh_builtin_returns_an_id_or_nil() {
    let api = FakeApi::with_events(vec![]);
    let lua = state(api.clone());
    run(&lua, r#"
        acid_draw_text(tostring(acid_mesh_builtin("cube")) .. " " .. tostring(acid_mesh_builtin("nope")), 0, 0, 0, 0)
    "#);
    assert_eq!(api.texts(), ["1 nil"]);
}

/// Runs a script on a thread so a hang fails the test instead of stalling it.
fn texts_within(src: &'static str) -> Vec<String> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let api = FakeApi::with_events(vec![]);
        let lua = state(api.clone());
        run(&lua, src);
        let _ = tx.send(api.texts());
    });
    rx.recv_timeout(std::time::Duration::from_secs(10)).expect("returned promptly")
}

#[test]
fn mesh_new_survives_an_extreme_index() {
    let t = texts_within(r#"
        for _, bad in ipairs({math.mininteger, math.maxinteger, 0, 65536}) do
            local id, err = acid_mesh_new({0,0,0, 1,0,0, 0,1,0}, {{bad, 2, 3}})
            acid_draw_text(tostring(id) .. ":" .. tostring(err), 0, 0, 0, 0)
        end
    "#);
    assert_eq!(t, vec!["nil:bad mesh"; 4]);
}

#[test]
fn mesh_new_never_follows_index_metamethods() {
    // Raw access: the trick table has no real entries, so points has fewer
    // than 3 (bad mesh) and faces is simply empty (the points decide).
    let t = texts_within(r#"
        local trick = setmetatable({}, {__index = function() return 0 end})
        local id, err = acid_mesh_new(trick, {{1,2,3}})
        acid_draw_text(tostring(id) .. ":" .. tostring(err), 0, 0, 0, 0)
        local id2, err2 = acid_mesh_new({0,0,0, 1,0,0, 0,1,0}, trick)
        acid_draw_text(tostring(id2) .. ":" .. tostring(err2), 0, 0, 0, 0)
        local id3, err3 = acid_mesh_new({0,0,0, 1,0,0, 0,1,0}, {trick})
        acid_draw_text(tostring(id3) .. ":" .. tostring(err3), 0, 0, 0, 0)
    "#);
    // (The fake API accepts the empty point list; the real one says bad mesh.)
    assert_eq!(t, ["1:nil", "2:nil", "nil:bad mesh"]);
}

#[test]
fn mesh_new_caps_lengths_before_building_vecs() {
    let t = texts_within(r#"
        local function pts(n) local t = {} for i = 1, n do t[i] = 0 end return t end
        local function faces(n) local t = {} for i = 1, n do t[i] = {1,2,3} end return t end
        local a, ea = acid_mesh_new(pts(3 * 512 + 1), {})
        local b, eb = acid_mesh_new(pts(3 * 512), {})
        local c, ec = acid_mesh_new(pts(3), faces(1025))
        local d, ed = acid_mesh_new(pts(3), faces(1024))
        acid_draw_text(tostring(a) .. ":" .. tostring(ea) .. " " .. tostring(c) .. ":" .. tostring(ec), 0, 0, 0, 0)
        acid_draw_text(tostring(b ~= nil) .. " " .. tostring(d ~= nil), 0, 0, 0, 0)
    "#);
    assert_eq!(t, ["nil:too big nil:too big", "true true"]);
}

#[test]
fn sound_and_song_calls_reach_lua() {
    let api = FakeApi::with_events(vec![]);
    let lua = state(api.clone());
    run(&lua, r#"
        local id, err = acid_sound_load("bad")
        acid_draw_text(tostring(id) .. " " .. err, 0, 0, 0, 0)
        local p = acid_sound_load("gate on")
        acid_draw_text(tostring(acid_sound_play(p)), 0, 0, 0, 0)
        acid_sound_play(p, "zap", 52)
        local s, w = acid_song_parse("x")
        acid_draw_text(s .. " " .. #w .. " " .. w[1], 0, 0, 0, 0)
        acid_song_play(s)
        acid_song_play(s, 4, 8)
        local o, r, t = acid_song_position()
        acid_draw_text(o .. r .. t, 0, 0, 0, 0)
    "#);
    assert_eq!(api.texts(), ["nil 1:1 unknown command 'bad'", "9", "2 1 instrument 01: x: not found", "123"]);
    let calls = api.calls();
    for want in ["sound_play 3  40", "sound_play 3 zap 52", "song_play 2 0 0", "song_play 2 4 8"] {
        assert!(calls.contains(&want.to_string()), "missing {want:?} in {calls:?}");
    }
}
