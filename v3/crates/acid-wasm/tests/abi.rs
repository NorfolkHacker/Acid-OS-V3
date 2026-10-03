//! ABI v1 tests (spec 15.6): WAT fixtures run against a recording fake API.
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use acid_api::{AcidApi, LocalTime, NetworkInfo, PolledEvent, TaskInfo, WindowInfo};
use acid_wasm::{CartEnd, WasmLimits, run_cart};

/// Records every call as a line; serves scripted events, then Close forever.
struct Rec {
    log: Mutex<Vec<String>>,
    events: Mutex<VecDeque<Option<PolledEvent>>>,
    clock: Mutex<i64>,
}

impl Rec {
    fn new(events: Vec<Option<PolledEvent>>) -> Arc<Self> {
        Arc::new(Self { log: Mutex::default(), events: Mutex::new(events.into()), clock: Mutex::new(0) })
    }
    fn log(&self, s: String) {
        self.log.lock().unwrap().push(s);
    }
}

impl AcidApi for Rec {
    fn poll_event(&self, ms: i64) -> Option<PolledEvent> {
        self.log(format!("poll {ms}"));
        self.events.lock().unwrap().pop_front().unwrap_or(Some(PolledEvent::Close))
    }
    fn now_ms(&self) -> i64 {
        let mut c = self.clock.lock().unwrap();
        *c += 1;
        *c
    }
    fn notify_redraw_done(&self) { self.log("notify".into()) }
    fn fill_rect(&self, x: i32, y: i32, w: i32, h: i32, c: u32) { self.log(format!("fill_rect {x} {y} {w} {h} {c:#08x}")) }
    fn fill_circle(&self, x: i32, y: i32, r: i32, c: u32) { self.log(format!("fill_circle {x} {y} {r} {c:#08x}")) }
    fn draw_text(&self, t: &str, x: i32, y: i32, fg: u32, bg: u32) {
        // Long strings are summarised so the fuel-per-byte test doesn't log megabytes.
        if t.len() > 64 {
            self.log(format!("text <{} bytes>", t.len()))
        } else {
            self.log(format!("text {t} {x} {y} {fg:#08x} {bg:#08x}"))
        }
    }
    fn draw_window_frame(&self, t: &str) { self.log(format!("frame {t}")) }
    fn draw_window_border(&self) { self.log("border".into()) }
    fn clear_user_area(&self) { self.log("clear".into()) }
    fn am_i_focused(&self) -> bool { true }
    fn launch_arg(&self) -> String { "arg!".into() }
    fn play_note(&self, v: i32, o: i32, vol: i32) { self.log(format!("play {v} {o} {vol}")) }
    fn stop_note(&self, v: i32) { self.log(format!("stop {v}")) }
    fn configure_voice(&self, v: i32, r: i32, a: i32, d: i32, s: i32, rel: i32) { self.log(format!("voice {v} {r} {a} {d} {s} {rel}")) }
    fn configure_filter(&self, c: i32, r: i32, m: i32) { self.log(format!("filter {c} {r} {m}")) }
    fn trigger_arp(&self, v: i32, n: [i32; 4], c: i32, ms: i32) { self.log(format!("arp {v} {n:?} {c} {ms}")) }
    fn configure_osc(&self, v: i32, w: i32, d: i32) { self.log(format!("osc {v} {w} {d}")) }
    fn set_ring_partner(&self, v: i32, p: i32) { self.log(format!("ring {v} {p}")) }
    fn set_volume(&self, p: i32) { self.log(format!("volume {p}")) }
    fn volume(&self) -> i32 { 65 }
    fn active_voice_count(&self) -> i32 { 3 }
    // Mirrors KernelApi: a cart never gets the overlay.
    fn overlay_open(&self) -> bool { false }
    fn is_cart(&self) -> bool { true }
    fn overlay_clear(&self) { self.log("overlay_clear".into()) }
    fn overlay_fill_rect(&self, x: i32, y: i32, w: i32, h: i32, c: u32) { self.log(format!("overlay_fill_rect {x} {y} {w} {h} {c:#08x}")) }
    fn overlay_close(&self) { self.log("overlay_close".into()) }
    fn repaint_region(&self, x: i32, y: i32, w: i32, h: i32) { self.log(format!("repaint {x} {y} {w} {h}")) }
    fn set_wallpaper_enabled(&self, on: bool) { self.log(format!("wallpaper {on}")) }
    fn wallpaper_enabled(&self) -> bool { true }
    fn window_max(&self) -> i32 { 8 }
    fn window_info(&self, i: i64) -> Option<WindowInfo> {
        (i == 1).then(|| WindowInfo { app_name: "v3/apps/x.lua".into(), x: 1, y: 2, w: 3, h: 4, focused: true })
    }
    fn activate_window(&self, i: i64) { self.log(format!("activate {i}")) }
    fn close_window(&self, i: i64) -> bool { self.log(format!("close {i}")); false }
    fn send_self_to_back(&self) { self.log("to_back".into()) }
    fn launcher_register(&self, path: &str, name: &str, w: i32, h: i32, multi: bool, libs: &str) -> bool {
        self.log(format!("register {path} {name} {w} {h} {multi} [{libs}]"));
        false
    }
    fn launcher_count(&self) -> i32 { 1 }
    fn launcher_path(&self, i: i64) -> Option<String> { (i == 0).then(|| "v3/apps/x.lua".into()) }
    fn launcher_name(&self, i: i64) -> Option<String> { (i == 0).then(|| "X".into()) }
    fn launcher_spawn(&self, i: i64) -> bool { self.log(format!("launch {i}")); false }
    fn spawn_app(&self, path: &str, w: i32, h: i32, arg: &str) -> bool {
        self.log(format!("spawn {path} {w} {h} [{arg}]"));
        false
    }
    fn composited_frames(&self) -> u32 { 7 }
    fn skipped_frames(&self) -> u32 { 9 }
    fn local_time(&self) -> LocalTime { LocalTime { year: 2026, month: 10, day: 2, hour: 9, min: 5, sec: 7 } }
    fn mem_used_kb(&self) -> i64 { 4321 }
    fn network_info(&self) -> NetworkInfo { NetworkInfo { host: "box".into(), ip: "10.1.2.3".into(), connected: true } }
    fn refresh_tasks(&self) -> i32 { 1 }
    fn task_count(&self) -> i32 { 1 }
    fn task_info(&self, i: i64) -> Option<TaskInfo> { (i == 0).then(|| TaskInfo { name: "router".into(), state: "blocked", cpu_percent: 7 }) }
    fn fs_list(&self, dir: &str) -> Result<Vec<String>, String> {
        if dir == "v3/apps" { Ok(vec!["a".into(), "b".into()]) } else { Err("bad path".into()) }
    }
    fn fs_read(&self, path: &str) -> Result<Vec<u8>, String> {
        match path {
            "v3/apps/a" => Ok(b"A\0B".to_vec()),
            // Large enough that producing it must cost fuel, even if the cart asks for 0 bytes.
            "v3/big" => Ok(vec![b'x'; 1 << 20]),
            _ => Err("not found".into()),
        }
    }
    fn fs_size(&self, path: &str) -> Result<u64, String> { if path == "v3/apps/a" { Ok(3) } else { Err("not found".into()) } }
    fn fs_write(&self, path: &str, data: &[u8]) -> Result<(), String> {
        self.log(format!("write {path} {data:?}"));
        if path.starts_with("v3/fsroot/Home/") { Ok(()) } else { Err("read only".into()) }
    }
    fn fs_rename(&self, from: &str, to: &str) -> Result<(), String> {
        self.log(format!("rename {from} {to}"));
        if from.starts_with("v3/fsroot/Home/") { Ok(()) } else { Err("read only".into()) }
    }
    fn fs_delete(&self, path: &str) -> Result<(), String> {
        self.log(format!("delete {path}"));
        if path.starts_with("v3/fsroot/Home/") { Ok(()) } else { Err("not found".into()) }
    }
    fn cart_roots(&self) -> Result<Vec<String>, String> { Err("not allowed".into()) }
    // The real api refuses every cart_* call for a cart; the fake answers
    // paths under "ok" so the record and listing formats can be checked.
    fn cart_list(&self, dir: &str) -> Result<Vec<String>, String> { if dir == "ok" { Ok(vec!["p".into(), "q".into()]) } else { Err("not allowed".into()) } }
    fn cart_stat(&self, path: &str) -> Result<(bool, u64), String> {
        match path {
            "ok/d" => Ok((true, 0)),
            "ok/f" => Ok((false, 12)),
            _ => Err("not allowed".into()),
        }
    }
    fn cart_read(&self, path: &str) -> Result<Vec<u8>, String> { if path == "ok/f" { Ok(b"hello".to_vec()) } else { Err("not allowed".into()) } }
}

fn run(src: &str, events: Vec<Option<PolledEvent>>, limits: WasmLimits) -> (CartEnd, Vec<String>) {
    let bytes = wat::parse_str(src).expect("fixture parses");
    let rec = Rec::new(events);
    let end = run_cart(rec.clone(), &bytes, limits);
    let log = rec.log.lock().unwrap().clone();
    (end, log)
}

/// The log without the poll lines, for tests that only care about drawing.
fn draws(log: &[String]) -> Vec<String> {
    log.iter().filter(|l| !l.starts_with("poll ")).cloned().collect()
}

const MIN: &str = r#"(module
  (import "acid" "fill_rect" (func $fr (param i32 i32 i32 i32 i32)))
  (memory (export "memory") 1)
  (func (export "acid_abi_version") (result i32) i32.const 1)
  (func (export "acid_on_create") i32.const 1 i32.const 2 i32.const 3 i32.const 4 i32.const 0xff0000 call $fr)
  (func (export "acid_on_event") (param i32 i32 i32 i32) local.get 0 local.get 1 local.get 2 local.get 3 i32.const 7 call $fr)
  (func (export "acid_on_idle") i32.const 9 i32.const 9 i32.const 9 i32.const 9 i32.const 9 call $fr)
  (func (export "acid_redraw") i32.const 0 i32.const 0 i32.const 0 i32.const 0 i32.const 0 call $fr))"#;

/// MIN with `extra` spliced in before the closing paren.
fn min_plus(extra: &str) -> String {
    format!("{}\n  {extra})", &MIN[..MIN.len() - 1])
}

/// A cart whose create body is `body`, with the drawing imports it needs.
fn create_cart(body: &str) -> String {
    format!(
        r#"(module
  (import "acid" "draw_text" (func $dt (param i32 i32 i32 i32 i32 i32)))
  (import "acid" "launch_arg" (func $la (param i32 i32) (result i32)))
  (memory (export "memory") 1)
  (data (i32.const 16) "hi")
  (func (export "acid_abi_version") (result i32) i32.const 1)
  (func (export "acid_on_create") {body})
  (func (export "acid_on_event") (param i32 i32 i32 i32))
  (func (export "acid_on_idle"))
  (func (export "acid_redraw")))"#
    )
}

#[test]
fn callbacks_run_in_order() {
    let events = vec![Some(PolledEvent::Touch { x: 5, y: 6, pressed: true }), None, Some(PolledEvent::Moved), Some(PolledEvent::Close)];
    let (end, log) = run(MIN, events, WasmLimits::default());
    assert_eq!(end, CartEnd::Closed);
    assert_eq!(
        draws(&log),
        vec![
            "fill_rect 1 2 3 4 0xff0000",
            "fill_rect 0 0 0 0 0x000000",
            "fill_rect 1 5 6 1 0x000007",
            "fill_rect 9 9 9 9 0x000009",
            "fill_rect 3 0 0 0 0x000007",
            "fill_rect 0 0 0 0 0x000000",
            "fill_rect 4 0 0 0 0x000007",
        ]
    );
    // Without acid_poll_timeout_ms, every poll waits the default 200 ms.
    assert!(log.iter().filter(|l| l.starts_with("poll ")).all(|l| l == "poll 200"));
}

#[test]
fn key_event_passes_code_and_pressed() {
    let (end, log) = run(MIN, vec![Some(PolledEvent::Key { code: 65, pressed: true })], WasmLimits::default());
    assert_eq!(end, CartEnd::Closed);
    assert!(draws(&log).contains(&"fill_rect 2 65 1 0 0x000007".to_string()), "{log:?}");
}

#[test]
fn version_other_than_1_is_refused() {
    let src = MIN.replace("(result i32) i32.const 1)", "(result i32) i32.const 2)");
    let (end, log) = run(&src, vec![], WasmLimits::default());
    assert!(matches!(end, CartEnd::Refused(_)), "{end:?}");
    assert!(log.is_empty(), "{log:?}");
}

#[test]
fn missing_export_is_refused() {
    let src = MIN.replace("(export \"acid_on_idle\")", "");
    let (end, log) = run(&src, vec![], WasmLimits::default());
    assert!(matches!(end, CartEnd::Refused(_)), "{end:?}");
    assert!(log.is_empty(), "{log:?}");
}

#[test]
fn wrong_export_signature_is_refused() {
    let src = MIN.replace("(func (export \"acid_on_idle\")", "(func (export \"acid_on_idle\") (param i32)");
    let (end, log) = run(&src, vec![], WasmLimits::default());
    assert!(matches!(end, CartEnd::Refused(_)), "{end:?}");
    assert!(log.is_empty(), "{log:?}");
}

#[test]
fn bad_bytes_and_unknown_imports_are_refused() {
    let rec = Rec::new(vec![]);
    assert!(matches!(run_cart(rec, b"not wasm", WasmLimits::default()), CartEnd::Refused(_)));
    let src = MIN.replace("(memory", "(import \"acid\" \"no_such_call\" (func))\n  (memory");
    assert_ne!(src, MIN);
    let (end, log) = run(&src, vec![], WasmLimits::default());
    assert!(matches!(end, CartEnd::Refused(_)), "{end:?}");
    assert!(log.is_empty(), "{log:?}");
}

#[test]
fn poll_timeout_export_is_used() {
    let src = min_plus(r#"(func (export "acid_poll_timeout_ms") (result i32) i32.const 40)"#);
    let (end, log) = run(&src, vec![None], WasmLimits::default());
    assert_eq!(end, CartEnd::Closed);
    let polls: Vec<_> = log.iter().filter(|l| l.starts_with("poll ")).collect();
    assert_eq!(polls, vec!["poll 40", "poll 40"]);
}

#[test]
fn poll_timeout_below_1_counts_as_1() {
    let src = min_plus(r#"(func (export "acid_poll_timeout_ms") (result i32) i32.const -5)"#);
    let (_, log) = run(&src, vec![], WasmLimits::default());
    assert!(log.contains(&"poll 1".to_string()), "{log:?}");
}

#[test]
fn destroy_runs_after_close() {
    let src = min_plus(r#"(func (export "acid_on_destroy") i32.const 8 i32.const 8 i32.const 8 i32.const 8 i32.const 0 call $fr)"#);
    let (end, log) = run(&src, vec![None], WasmLimits::default());
    assert_eq!(end, CartEnd::Closed);
    assert_eq!(log.last().map(String::as_str), Some("fill_rect 8 8 8 8 0x000000"));
    let n = log.len();
    assert_eq!(log[n - 2], "fill_rect 4 0 0 0 0x000007", "close event comes just before destroy");
}

#[test]
fn endless_loop_runs_out_of_fuel() {
    let src = MIN.replace(
        "(func (export \"acid_on_idle\") i32.const 9 i32.const 9 i32.const 9 i32.const 9 i32.const 9 call $fr)",
        "(func (export \"acid_on_idle\") (loop br 0))",
    );
    assert_ne!(src, MIN);
    let start = Instant::now();
    let (end, _) = run(&src, vec![None], WasmLimits { fuel: 1_000_000, ..WasmLimits::default() });
    assert_eq!(end, CartEnd::StoppedResponding);
    assert!(start.elapsed() < Duration::from_secs(10), "took {:?}", start.elapsed());
}

#[test]
fn fuel_is_refilled_per_callback() {
    // Each idle burns well under the budget, but many idles together exceed
    // it: a per-callback budget must not run out.
    let src = MIN.replace(
        "(func (export \"acid_on_idle\") i32.const 9 i32.const 9 i32.const 9 i32.const 9 i32.const 9 call $fr)",
        "(func (export \"acid_on_idle\") (local i32) i32.const 2000 local.set 0 (loop local.get 0 i32.const 1 i32.sub local.tee 0 br_if 0))",
    );
    let (end, _) = run(&src, vec![None; 20], WasmLimits { fuel: 20_000, ..WasmLimits::default() });
    assert_eq!(end, CartEnd::Closed);
}

#[test]
fn memory_grow_past_cap_ends_the_cart_out_of_memory() {
    // The cart ignores grow's result: the host itself must end it (§15.2).
    let src = create_cart(
        "i32.const 300 memory.grow drop \
         i32.const 16 i32.const 2 i32.const 0 i32.const 0 i32.const 0 i32.const 0 call $dt",
    );
    let (end, log) = run(&src, vec![], WasmLimits { max_pages: 256, ..WasmLimits::default() });
    assert_eq!(end, CartEnd::OutOfMemory);
    // The grow ended the cart before it drew anything.
    assert!(log.is_empty(), "{log:?}");
}

#[test]
fn memory_grow_within_cap_succeeds() {
    let src = create_cart(
        "i32.const 3 memory.grow i32.const -1 i32.eq if unreachable end \
         i32.const 16 i32.const 2 i32.const 0 i32.const 0 i32.const 0 i32.const 0 call $dt",
    );
    let (end, log) = run(&src, vec![], WasmLimits { max_pages: 4, ..WasmLimits::default() });
    assert_eq!(end, CartEnd::Closed);
    assert_eq!(log[0], "text hi 0 0 0x000000 0x000000");
}

#[test]
fn initial_memory_past_cap_is_out_of_memory() {
    let src = MIN.replace("(memory (export \"memory\") 1)", "(memory (export \"memory\") 300)");
    let (end, log) = run(&src, vec![], WasmLimits { max_pages: 256, ..WasmLimits::default() });
    assert_eq!(end, CartEnd::OutOfMemory);
    assert!(log.is_empty(), "{log:?}");
}

#[test]
fn string_args_cross_the_boundary() {
    let src = create_cart("i32.const 16 i32.const 2 i32.const 1 i32.const 2 i32.const 3 i32.const 4 call $dt");
    let (end, log) = run(&src, vec![], WasmLimits::default());
    assert_eq!(end, CartEnd::Closed);
    assert_eq!(log[0], "text hi 1 2 0x000003 0x000004");

    let src = create_cart("i32.const 65530 i32.const 100 i32.const 1 i32.const 2 i32.const 3 i32.const 4 call $dt");
    let (end, log) = run(&src, vec![], WasmLimits::default());
    assert!(matches!(end, CartEnd::Trap(_)), "{end:?}");
    assert!(log.is_empty(), "{log:?}");
}

#[test]
fn non_utf8_string_traps() {
    let src = create_cart("i32.const 16 i32.const 0xff i32.store8 i32.const 16 i32.const 2 i32.const 0 i32.const 0 i32.const 0 i32.const 0 call $dt");
    let (end, log) = run(&src, vec![], WasmLimits::default());
    assert!(matches!(end, CartEnd::Trap(_)), "{end:?}");
    assert!(log.is_empty(), "{log:?}");
}

#[test]
fn launch_arg_writes_and_truncates() {
    // ret = launch_arg(32, cap); draw_text(32, min(ret, cap)) shows the bytes
    // written; draw_text(ret as a digit) shows the returned length.
    let body = |cap: i32| {
        format!(
            "(local i32) i32.const 32 i32.const {cap} call $la local.set 0 \
             i32.const 32 local.get 0 i32.const {cap} local.get 0 i32.const {cap} i32.lt_s select \
             local.get 0 i32.const 0 i32.const 0 i32.const 0 call $dt"
        )
    };
    let (end, log) = run(&create_cart(&body(64)), vec![], WasmLimits::default());
    assert_eq!(end, CartEnd::Closed);
    assert_eq!(log[0], "text arg! 4 0 0x000000 0x000000");

    let (end, log) = run(&create_cart(&body(2)), vec![], WasmLimits::default());
    assert_eq!(end, CartEnd::Closed);
    assert_eq!(log[0], "text ar 4 0 0x000000 0x000000");
}

#[test]
fn launch_arg_out_of_bounds_buffer_traps() {
    let (end, _) = run(&create_cart("i32.const 65535 i32.const 16 call $la drop"), vec![], WasmLimits::default());
    assert!(matches!(end, CartEnd::Trap(_)), "{end:?}");
}

#[test]
fn core_imports_reach_the_api() {
    let src = r#"(module
  (import "acid" "now_ms" (func $now (result i64)))
  (import "acid" "notify_redraw_done" (func $nrd))
  (import "acid" "fill_circle" (func $fc (param i32 i32 i32 i32)))
  (import "acid" "draw_window_frame" (func $dwf (param i32 i32)))
  (import "acid" "draw_window_border" (func $dwb))
  (import "acid" "clear_user_area" (func $cua))
  (import "acid" "am_i_focused" (func $foc (result i32)))
  (memory (export "memory") 1)
  (data (i32.const 0) "Title")
  (func (export "acid_abi_version") (result i32) i32.const 1)
  (func (export "acid_on_create")
    call $now drop
    call $now i32.wrap_i64 call $foc i32.const 10 i32.const 0x123456 call $fc
    i32.const 0 i32.const 5 call $dwf
    call $dwb call $cua call $nrd)
  (func (export "acid_on_event") (param i32 i32 i32 i32))
  (func (export "acid_on_idle"))
  (func (export "acid_redraw")))"#;
    let (end, log) = run(src, vec![], WasmLimits::default());
    assert_eq!(end, CartEnd::Closed);
    assert_eq!(draws(&log), vec!["fill_circle 2 1 10 0x123456", "frame Title", "border", "clear", "notify"]);
}

#[test]
fn trap_carries_its_message() {
    let src = MIN.replace(
        "(func (export \"acid_on_idle\") i32.const 9 i32.const 9 i32.const 9 i32.const 9 i32.const 9 call $fr)",
        "(func (export \"acid_on_idle\") unreachable)",
    );
    let (end, _) = run(&src, vec![None], WasmLimits::default());
    match end {
        CartEnd::Trap(msg) => assert!(msg.contains("unreachable"), "{msg}"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn limits_default_to_the_constants() {
    let l = WasmLimits::default();
    assert_eq!((l.fuel, l.max_pages), (acid_wasm::WASM_FUEL, acid_wasm::WASM_MAX_PAGES));
    assert_eq!((acid_wasm::WASM_FUEL, acid_wasm::WASM_MAX_PAGES, acid_wasm::ABI_VERSION), (200_000_000, 256, 1));
}

#[test]
fn error_strings_map_to_codes() {
    use acid_wasm::err_code;
    let got: Vec<i32> = ["not found", "bad path", "read only", "not allowed", "too big", "disk on fire"].iter().map(|s| err_code(s)).collect();
    assert_eq!(got, vec![-1, -2, -3, -4, -5, -6]);
}

/// MIN with the export `name`'s body replaced by an endless loop.
fn spin(name: &str, result: bool) -> String {
    let start = MIN.find(&format!("(func (export \"{name}\")")).expect("export in MIN");
    let end = start + MIN[start..].find('\n').unwrap_or(MIN.len() - 1 - start);
    let body = if result { format!("(func (export \"{name}\") (result i32) (loop br 0) i32.const 1)") } else { format!("(func (export \"{name}\") (loop br 0))") };
    let mut s = MIN.to_string();
    s.replace_range(start..end, &body);
    s
}

const SMALL_FUEL: WasmLimits = WasmLimits { fuel: 1_000_000, max_pages: 256 };

#[test]
fn endless_start_function_runs_out_of_fuel() {
    let src = min_plus("(func $spin (loop br 0)) (start $spin)");
    assert_eq!(run(&src, vec![], SMALL_FUEL).0, CartEnd::StoppedResponding);
}

#[test]
fn endless_version_check_runs_out_of_fuel() {
    let (end, log) = run(&spin("acid_abi_version", true), vec![], SMALL_FUEL);
    assert_eq!(end, CartEnd::StoppedResponding);
    assert!(log.is_empty(), "{log:?}");
}

#[test]
fn endless_poll_timeout_runs_out_of_fuel() {
    let src = min_plus(r#"(func (export "acid_poll_timeout_ms") (result i32) (loop br 0) i32.const 1)"#);
    assert_eq!(run(&src, vec![], SMALL_FUEL).0, CartEnd::StoppedResponding);
}

#[test]
fn endless_destroy_runs_out_of_fuel() {
    let src = min_plus(r#"(func (export "acid_on_destroy") (loop br 0))"#);
    assert_eq!(run(&src, vec![], SMALL_FUEL).0, CartEnd::StoppedResponding);
}

#[test]
fn poll_timeout_is_clamped_to_a_minute() {
    let src = min_plus(r#"(func (export "acid_poll_timeout_ms") (result i32) i32.const 0x7fffffff)"#);
    let (_, log) = run(&src, vec![], WasmLimits::default());
    assert!(log.contains(&"poll 60000".to_string()), "{log:?}");
}

#[test]
fn memory_grows_exactly_to_the_cap() {
    // 1 + 255 = 256 pages is allowed; one page more ends the cart.
    let src = create_cart(
        "i32.const 255 memory.grow i32.const 1 i32.ne if unreachable end \
         i32.const 16 i32.const 2 i32.const 0 i32.const 0 i32.const 0 i32.const 0 call $dt \
         i32.const 1 memory.grow drop",
    );
    let (end, log) = run(&src, vec![], WasmLimits { max_pages: 256, ..WasmLimits::default() });
    assert_eq!(end, CartEnd::OutOfMemory);
    assert_eq!(log, vec!["text hi 0 0 0x000000 0x000000"], "the grow to the cap succeeded");
}

#[test]
fn huge_initial_table_is_out_of_memory() {
    let src = min_plus("(table 100000000 funcref)");
    let start = Instant::now();
    let (end, log) = run(&src, vec![], WasmLimits::default());
    assert_eq!(end, CartEnd::OutOfMemory);
    assert!(log.is_empty(), "{log:?}");
    assert!(start.elapsed() < Duration::from_secs(2), "took {:?}", start.elapsed());
}

#[test]
fn table_grow_past_cap_is_out_of_memory() {
    // The cart ignores grow's result: the host itself must end it (§15.2).
    let src = create_cart(&format!("ref.null func i32.const {} table.grow 0 drop", acid_wasm::WASM_MAX_TABLE_ELEMENTS))
        .replace("(memory", "(table 1 funcref)\n  (memory");
    let (end, log) = run(&src, vec![], WasmLimits::default());
    assert_eq!(end, CartEnd::OutOfMemory);
    assert!(log.is_empty(), "{log:?}");
}

#[test]
fn table_grows_exactly_to_the_cap() {
    let src = create_cart(&format!(
        "ref.null func i32.const {} table.grow 0 i32.const 1 i32.ne if unreachable end \
         i32.const 16 i32.const 2 i32.const 0 i32.const 0 i32.const 0 i32.const 0 call $dt \
         ref.null func i32.const 1 table.grow 0 drop",
        acid_wasm::WASM_MAX_TABLE_ELEMENTS - 1
    ))
    .replace("(memory", "(table 1 funcref)\n  (memory");
    let (end, log) = run(&src, vec![], WasmLimits::default());
    assert_eq!(end, CartEnd::OutOfMemory, "one element past the cap ends the cart");
    assert_eq!(log, vec!["text hi 0 0 0x000000 0x000000"], "the grow to the cap succeeded");
}

#[test]
fn second_table_is_refused() {
    // wasmi reports a second table as InstantiationError::TooManyTables,
    // which is a refusal (the module asks for more than a cart may have),
    // not an allocation failure.
    let src = min_plus("(table 1 funcref) (table 1 funcref)");
    let (end, log) = run(&src, vec![], WasmLimits::default());
    match &end {
        CartEnd::Refused(why) => assert!(why.contains("too many tables"), "{why}"),
        other => panic!("{other:?}"),
    }
    assert!(log.is_empty(), "{log:?}");
}

#[test]
fn big_strings_cost_fuel() {
    // 1 MiB of zero bytes is valid UTF-8; drawing it over and over must run
    // out of fuel, because every byte crossing the boundary is charged.
    let src = r#"(module
  (import "acid" "draw_text" (func $dt (param i32 i32 i32 i32 i32 i32)))
  (memory (export "memory") 17)
  (func (export "acid_abi_version") (result i32) i32.const 1)
  (func (export "acid_on_create")
    (loop i32.const 0 i32.const 1048576 i32.const 0 i32.const 0 i32.const 0 i32.const 0 call $dt br 0))
  (func (export "acid_on_event") (param i32 i32 i32 i32))
  (func (export "acid_on_idle"))
  (func (export "acid_redraw")))"#;
    let start = Instant::now();
    let (end, log) = run(src, vec![], WasmLimits { fuel: 10_000_000, ..WasmLimits::default() });
    assert_eq!(end, CartEnd::StoppedResponding);
    assert!(start.elapsed() < Duration::from_secs(5), "took {:?}", start.elapsed());
    // 10M fuel at 1 per 8 bytes covers about 76 MiB: at most 76 draws.
    assert!(log.len() <= 76, "{} draws", log.len());
}

#[test]
fn full_window_fills_cost_fuel() {
    // Each fill moves no cart bytes, but the host fills a whole window for
    // it: charged by area, an endless fill loop must stop within the real
    // budget after a few thousand fills, not tens of millions (§15.2).
    let src = r#"(module
  (import "acid" "fill_rect" (func $fr (param i32 i32 i32 i32 i32)))
  (memory (export "memory") 1)
  (func (export "acid_abi_version") (result i32) i32.const 1)
  (func (export "acid_on_create")
    (loop i32.const 0 i32.const 0 i32.const 640 i32.const 336 i32.const 0 call $fr br 0))
  (func (export "acid_on_event") (param i32 i32 i32 i32))
  (func (export "acid_on_idle"))
  (func (export "acid_redraw")))"#;
    let start = Instant::now();
    let (end, log) = run(src, vec![], WasmLimits::default());
    assert_eq!(end, CartEnd::StoppedResponding);
    // 200M fuel at 640 * 336 * 2 / 8 = 53,760 fuel a fill: about 3,700 fills.
    assert!(log.len() < 10_000, "{} fills", log.len());
    assert!(log.len() > 1_000, "{} fills: the charge is far above the area", log.len());
    assert!(start.elapsed() < Duration::from_secs(10), "took {:?}", start.elapsed());
}

#[test]
fn file_calls_cost_a_flat_fee() {
    // Deleting an empty-named file moves 0 bytes; the flat 1 MiB-equivalent
    // per call must still stop the loop within the real budget (§15.2).
    let src = r#"(module
  (import "acid" "fs_delete" (func $fd (param i32 i32) (result i32)))
  (memory (export "memory") 1)
  (func (export "acid_abi_version") (result i32) i32.const 1)
  (func (export "acid_on_create")
    (loop i32.const 0 i32.const 0 call $fd drop br 0))
  (func (export "acid_on_event") (param i32 i32 i32 i32))
  (func (export "acid_on_idle"))
  (func (export "acid_redraw")))"#;
    let start = Instant::now();
    let (end, log) = run(src, vec![], WasmLimits::default());
    assert_eq!(end, CartEnd::StoppedResponding);
    // 200M fuel at 131,072 fuel a call: about 1,500 calls.
    assert!(log.len() < 2_000, "{} deletes", log.len());
    assert!(log.len() > 1_000, "{} deletes", log.len());
    assert!(start.elapsed() < Duration::from_secs(10), "took {:?}", start.elapsed());
}

// ---- The rest of the import table (Task 2) ----

/// A cart for the import groups: the shared helpers `$say` (an i32 as a
/// fill_rect log line), `$say64` (an i64 as lo, hi), `$txt` (ptr, len as a
/// draw_text line) and `$rec` (calls a `(buf, cap) -> len` result already
/// on the stack, then shows the bytes) sit around `imports` and `body`.
fn api_cart(imports: &str, data: &str, body: &str) -> String {
    format!(
        r#"(module
  (import "acid" "fill_rect" (func $fr (param i32 i32 i32 i32 i32)))
  (import "acid" "draw_text" (func $dt (param i32 i32 i32 i32 i32 i32)))
  {imports}
  (memory (export "memory") 2)
  {data}
  (func $say (param i32) local.get 0 i32.const 0 i32.const 0 i32.const 0 i32.const 0 call $fr)
  (func $say64 (param i64) local.get 0 i32.wrap_i64 local.get 0 i64.const 32 i64.shr_s i32.wrap_i64 i32.const 0 i32.const 0 i32.const 0 call $fr)
  (func $txt (param i32 i32) local.get 0 local.get 1 i32.const 0 i32.const 0 i32.const 0 i32.const 0 call $dt)
  (func (export "acid_abi_version") (result i32) i32.const 1)
  (func (export "acid_on_create") {body})
  (func (export "acid_on_event") (param i32 i32 i32 i32))
  (func (export "acid_on_idle"))
  (func (export "acid_redraw")))"#
    )
}

/// Runs `api_cart` and requires a clean close.
fn run_api(imports: &str, data: &str, body: &str) -> Vec<String> {
    let (end, log) = run(&api_cart(imports, data, body), vec![], WasmLimits::default());
    assert_eq!(end, CartEnd::Closed, "{log:?}");
    draws(&log)
}

/// "fill_rect v 0 0 0 0x000000", what `$say v` logs.
fn said(v: i32) -> String {
    format!("fill_rect {v} 0 0 0 0x000000")
}

/// Expected log lines from a mix of &str and String.
macro_rules! exp {
    ($($x:expr),* $(,)?) => { vec![$(String::from($x)),*] };
}

/// What `$txt` logs for `s`.
fn texted(s: &str) -> String {
    format!("text {s} 0 0 0x000000 0x000000")
}

#[test]
fn audio_calls_log_exact_arguments() {
    let imports = r#"
  (import "acid" "play_note" (func $pn (param i32 i32 i32)))
  (import "acid" "stop_note" (func $sn (param i32)))
  (import "acid" "configure_voice" (func $cv (param i32 i32 i32 i32 i32 i32)))
  (import "acid" "configure_filter" (func $cf (param i32 i32 i32)))
  (import "acid" "trigger_arp" (func $ta (param i32 i32 i32 i32 i32 i32 i32)))
  (import "acid" "configure_osc" (func $co (param i32 i32 i32)))
  (import "acid" "set_ring_partner" (func $rp (param i32 i32)))
  (import "acid" "set_volume" (func $sv (param i32)))
  (import "acid" "get_volume" (func $gv (result i32)))
  (import "acid" "active_voice_count" (func $avc (result i32)))"#;
    let body = "i32.const 1 i32.const 60 i32.const 90 call $pn \
        i32.const 2 call $sn \
        i32.const 3 i32.const 1 i32.const 10 i32.const 20 i32.const 30 i32.const 40 call $cv \
        i32.const 500 i32.const 8 i32.const 2 call $cf \
        i32.const 1 i32.const 60 i32.const 64 i32.const 67 i32.const 72 i32.const 4 i32.const 125 call $ta \
        i32.const 2 i32.const 3 i32.const 25 call $co \
        i32.const 1 i32.const -1 call $rp \
        i32.const 80 call $sv \
        call $gv call $say call $avc call $say";
    let log = run_api(imports, "", body);
    assert_eq!(
        log,
        exp![
            "play 1 60 90",
            "stop 2",
            "voice 3 1 10 20 30 40",
            "filter 500 8 2",
            "arp 1 [60, 64, 67, 72] 4 125",
            "osc 2 3 25",
            "ring 1 -1",
            "volume 80",
            said(65),
            said(3),
        ]
    );
}

#[test]
fn overlay_and_wallpaper_booleans() {
    let imports = r#"
  (import "acid" "overlay_open" (func $oo (result i32)))
  (import "acid" "overlay_clear" (func $oc))
  (import "acid" "overlay_fill_rect" (func $ofr (param i32 i32 i32 i32 i32)))
  (import "acid" "overlay_close" (func $ocl))
  (import "acid" "repaint_region" (func $rr (param i32 i32 i32 i32)))
  (import "acid" "set_wallpaper_enabled" (func $swe (param i32)))
  (import "acid" "get_wallpaper_enabled" (func $gwe (result i32)))
  (import "acid" "window_max" (func $wm (result i32)))"#;
    let body = "call $oo call $say call $oc i32.const 1 i32.const 2 i32.const 3 i32.const 4 i32.const 0xff00ff call $ofr call $ocl \
        i32.const 5 i32.const 6 i32.const 7 i32.const 8 call $rr \
        i32.const 1 call $swe i32.const 0 call $swe i32.const 7 call $swe \
        call $gwe call $say call $wm call $say";
    let log = run_api(imports, "", body);
    assert_eq!(
        log,
        exp![
            said(0),
            "overlay_clear",
            "overlay_fill_rect 1 2 3 4 0xff00ff",
            "overlay_close",
            "repaint 5 6 7 8",
            "wallpaper true",
            "wallpaper false",
            // Any non-zero value is "on".
            "wallpaper true",
            said(1),
            said(8),
        ]
    );
}

#[test]
fn window_info_record_and_missing_slot() {
    let imports = r#"
  (import "acid" "window_info" (func $wi (param i32 i32 i32) (result i32)))
  (import "acid" "activate_window" (func $aw (param i32)))
  (import "acid" "close_window" (func $cw (param i32) (result i32)))
  (import "acid" "send_self_to_back" (func $stb))"#;
    let body = "(local i32) i32.const 1 i32.const 256 i32.const 64 call $wi local.set 0 \
        i32.const 256 local.get 0 call $txt \
        local.get 0 call $say \
        i32.const 2 i32.const 256 i32.const 64 call $wi call $say \
        i32.const -1 i32.const 256 i32.const 64 call $wi call $say \
        i32.const 3 call $aw i32.const 4 call $cw call $say call $stb";
    let log = run_api(imports, "", body);
    assert_eq!(
        log,
        exp![
            texted("v3/apps/x.lua\t1\t2\t3\t4\t1"),
            said(23),
            said(-1),
            said(-1),
            "activate 3",
            said(-4),
            "to_back",
        ]
    );
}

#[test]
fn launcher_calls() {
    let imports = r#"
  (import "acid" "launcher_register" (func $lr (param i32 i32 i32 i32 i32 i32 i32 i32 i32) (result i32)))
  (import "acid" "launcher_count" (func $lc (result i32)))
  (import "acid" "launcher_path" (func $lp (param i32 i32 i32) (result i32)))
  (import "acid" "launcher_name" (func $ln (param i32 i32 i32) (result i32)))
  (import "acid" "launcher_spawn" (func $ls (param i32) (result i32)))"#;
    let data = r#"(data (i32.const 0) "pathnamelibs")"#;
    let body = "(local i32) call $lc call $say \
        i32.const 0 i32.const 256 i32.const 64 call $lp local.set 0 i32.const 256 local.get 0 call $txt \
        i32.const 0 i32.const 256 i32.const 64 call $ln local.set 0 i32.const 256 local.get 0 call $txt \
        i32.const 1 i32.const 256 i32.const 64 call $lp call $say \
        i32.const 1 i32.const 256 i32.const 64 call $ln call $say \
        i32.const 0 i32.const 4 i32.const 4 i32.const 4 i32.const 320 i32.const 200 i32.const 1 i32.const 8 i32.const 4 call $lr call $say \
        i32.const 2 call $ls call $say";
    let log = run_api(imports, data, body);
    assert_eq!(
        log,
        exp![
            said(1),
            texted("v3/apps/x.lua"),
            texted("X"),
            said(-1),
            said(-1),
            "register path name 320 200 true [libs]",
            said(0),
            "launch 2",
            said(0),
        ]
    );
}

#[test]
fn spawn_app_passes_its_strings() {
    let imports = r#"(import "acid" "spawn_app" (func $sa (param i32 i32 i32 i32 i32 i32) (result i32)))"#;
    let data = r#"(data (i32.const 0) "v3/apps/y.luahi")"#;
    let body = "i32.const 0 i32.const 13 i32.const 100 i32.const 50 i32.const 13 i32.const 2 call $sa call $say \
        i32.const 0 i32.const 13 i32.const 1 i32.const 2 i32.const 0 i32.const 0 call $sa call $say";
    let log = run_api(imports, data, body);
    assert_eq!(log, exp!["spawn v3/apps/y.lua 100 50 [hi]", said(0), "spawn v3/apps/y.lua 1 2 []", said(0)]);
}

#[test]
fn time_network_and_task_records() {
    let imports = r#"
  (import "acid" "local_time" (func $lt (param i32 i32) (result i32)))
  (import "acid" "network_info" (func $ni (param i32 i32) (result i32)))
  (import "acid" "refresh_tasks" (func $rt (result i32)))
  (import "acid" "task_count" (func $tc (result i32)))
  (import "acid" "task_info" (func $ti (param i32 i32 i32) (result i32)))
  (import "acid" "composited_frames" (func $cf (result i32)))
  (import "acid" "skipped_frames" (func $sf (result i32)))"#;
    let body = "(local i32) i32.const 256 i32.const 64 call $lt local.set 0 i32.const 256 local.get 0 call $txt \
        i32.const 256 i32.const 64 call $ni local.set 0 i32.const 256 local.get 0 call $txt \
        call $rt call $say call $tc call $say \
        i32.const 0 i32.const 256 i32.const 64 call $ti local.set 0 i32.const 256 local.get 0 call $txt \
        i32.const 1 i32.const 256 i32.const 64 call $ti call $say \
        call $cf call $say call $sf call $say";
    let log = run_api(imports, "", body);
    assert_eq!(
        log,
        exp![
            texted("2026\t10\t2\t9\t5\t7"),
            texted("box\t10.1.2.3\t1"),
            said(1),
            said(1),
            texted("router\tblocked\t7"),
            said(-1),
            said(7),
            said(9),
        ]
    );
}

#[test]
fn mem_used_and_now_are_i64() {
    let imports = r#"
  (import "acid" "mem_used_kb" (func $mu (result i64)))
  (import "acid" "now_ms" (func $now (result i64)))"#;
    // 4321 in the low word; the fake clock's first reading is 1. A value
    // above 2^32 would show in the high word, so also shift one up.
    let body = "call $mu call $say64 call $now call $say64 call $mu i64.const 32 i64.shl call $say64";
    let log = run_api(imports, "", body);
    assert_eq!(
        log,
        exp!["fill_rect 4321 0 0 0 0x000000", "fill_rect 1 0 0 0 0x000000", "fill_rect 0 4321 0 0 0x000000"]
    );
}

#[test]
fn fs_list_read_and_size() {
    let imports = r#"
  (import "acid" "fs_list" (func $fl (param i32 i32 i32 i32) (result i32)))
  (import "acid" "fs_read" (func $frd (param i32 i32 i32 i32) (result i32)))
  (import "acid" "fs_size" (func $fsz (param i32 i32) (result i64)))"#;
    let data = r#"(data (i32.const 0) "v3/appsv3/apps/anopeNOPE")"#;
    let body = "(local i32) \
        i32.const 0 i32.const 7 i32.const 256 i32.const 64 call $fl local.set 0 i32.const 256 local.get 0 call $txt \
        i32.const 12 i32.const 5 i32.const 256 i32.const 64 call $fl call $say \
        i32.const 7 i32.const 9 i32.const 256 i32.const 64 call $frd local.set 0 local.get 0 call $say i32.const 256 local.get 0 call $txt \
        i32.const 16 i32.const 4 i32.const 256 i32.const 64 call $frd call $say \
        i32.const 7 i32.const 9 call $fsz call $say64 \
        i32.const 16 i32.const 4 call $fsz call $say64";
    let log = run_api(imports, data, body);
    assert_eq!(
        log,
        exp![
            texted("a\nb"),
            // Not a directory the fake knows: "bad path" is -2.
            said(-2),
            said(3),
            // The NUL byte in the file survives; the UTF-8 rule is for strings only.
            texted("A\0B"),
            said(-1),
            // fs_size: 3 in the low word; "not found" -1 as an i64 is lo -1, hi -1.
            "fill_rect 3 0 0 0 0x000000",
            "fill_rect -1 -1 0 0 0x000000",
        ]
    );
}

#[test]
fn fs_write_raw_data_and_codes() {
    let imports = r#"(import "acid" "fs_write" (func $fw (param i32 i32 i32 i32) (result i32)))"#;
    // Paths at 0 and 16; the data at 64 is not UTF-8 and has a NUL.
    let data = r#"(data (i32.const 0) "v3/apps/x") (data (i32.const 16) "v3/fsroot/Home/f") (data (i32.const 64) "\ff\00\01")"#;
    let body = "i32.const 0 i32.const 9 i32.const 64 i32.const 3 call $fw call $say \
        i32.const 16 i32.const 16 i32.const 64 i32.const 3 call $fw call $say \
        i32.const 16 i32.const 16 i32.const 0 i32.const 0 call $fw call $say";
    let log = run_api(imports, data, body);
    assert_eq!(
        log,
        exp![
            "write v3/apps/x [255, 0, 1]".to_string(),
            said(-3),
            "write v3/fsroot/Home/f [255, 0, 1]".to_string(),
            said(0),
            "write v3/fsroot/Home/f []".to_string(),
            said(0),
        ]
    );
}

#[test]
fn fs_write_data_out_of_bounds_traps() {
    let imports = r#"(import "acid" "fs_write" (func $fw (param i32 i32 i32 i32) (result i32)))"#;
    let (end, log) = run(&api_cart(imports, r#"(data (i32.const 0) "v3/apps/x")"#, "i32.const 0 i32.const 9 i32.const 131000 i32.const 1000 call $fw drop"), vec![], WasmLimits::default());
    assert!(matches!(end, CartEnd::Trap(_)), "{end:?}");
    assert!(log.is_empty(), "{log:?}");
}

#[test]
fn fs_rename_and_delete_codes() {
    let imports = r#"
  (import "acid" "fs_rename" (func $frn (param i32 i32 i32 i32) (result i32)))
  (import "acid" "fs_delete" (func $fd (param i32 i32) (result i32)))"#;
    let data = r#"(data (i32.const 0) "v3/fsroot/Home/a") (data (i32.const 32) "v3/apps/b") (data (i32.const 64) "v3/fsroot/Home/c")"#;
    let body = "i32.const 0 i32.const 16 i32.const 64 i32.const 16 call $frn call $say \
        i32.const 32 i32.const 9 i32.const 64 i32.const 16 call $frn call $say \
        i32.const 0 i32.const 16 call $fd call $say \
        i32.const 32 i32.const 9 call $fd call $say";
    let log = run_api(imports, data, body);
    assert_eq!(
        log,
        exp![
            "rename v3/fsroot/Home/a v3/fsroot/Home/c".to_string(),
            said(0),
            "rename v3/apps/b v3/fsroot/Home/c".to_string(),
            said(-3),
            "delete v3/fsroot/Home/a".to_string(),
            said(0),
            "delete v3/apps/b".to_string(),
            said(-1),
        ]
    );
}

#[test]
fn cart_calls_are_not_allowed_but_formats_hold() {
    let imports = r#"
  (import "acid" "cart_roots" (func $cr (param i32 i32) (result i32)))
  (import "acid" "cart_list" (func $cl (param i32 i32 i32 i32) (result i32)))
  (import "acid" "cart_stat" (func $cs (param i32 i32 i32 i32) (result i32)))
  (import "acid" "cart_read" (func $crd (param i32 i32 i32 i32) (result i32)))"#;
    let data = r#"(data (i32.const 0) "nope") (data (i32.const 8) "ok") (data (i32.const 16) "ok/d") (data (i32.const 24) "ok/f")"#;
    let body = "(local i32) i32.const 256 i32.const 64 call $cr call $say \
        i32.const 0 i32.const 4 i32.const 256 i32.const 64 call $cl call $say \
        i32.const 0 i32.const 4 i32.const 256 i32.const 64 call $cs call $say \
        i32.const 0 i32.const 4 i32.const 256 i32.const 64 call $crd call $say \
        i32.const 8 i32.const 2 i32.const 256 i32.const 64 call $cl local.set 0 i32.const 256 local.get 0 call $txt \
        i32.const 16 i32.const 4 i32.const 256 i32.const 64 call $cs local.set 0 i32.const 256 local.get 0 call $txt \
        i32.const 24 i32.const 4 i32.const 256 i32.const 64 call $cs local.set 0 i32.const 256 local.get 0 call $txt \
        i32.const 24 i32.const 4 i32.const 256 i32.const 64 call $crd local.set 0 i32.const 256 local.get 0 call $txt";
    let log = run_api(imports, data, body);
    assert_eq!(
        log,
        exp![said(-4), said(-4), said(-4), said(-4), texted("p\nq"), texted("dir\t0"), texted("file\t12"), texted("hello")]
    );
}

#[test]
fn results_larger_than_cap_return_full_length_and_write_cap_bytes() {
    let imports = r#"
  (import "acid" "window_info" (func $wi (param i32 i32 i32) (result i32)))
  (import "acid" "fs_list" (func $fl (param i32 i32 i32 i32) (result i32)))
  (import "acid" "fs_read" (func $frd (param i32 i32 i32 i32) (result i32)))"#;
    // The buffer is pre-filled with Z so the bytes beyond cap show as untouched.
    let data = r#"(data (i32.const 0) "v3/appsv3/apps/a") (data (i32.const 256) "ZZZZZZZZ")"#;
    let body = "i32.const 1 i32.const 256 i32.const 4 call $wi call $say i32.const 256 i32.const 8 call $txt \
        i32.const 0 i32.const 7 i32.const 256 i32.const 2 call $fl call $say i32.const 256 i32.const 8 call $txt \
        i32.const 7 i32.const 9 i32.const 256 i32.const 0 call $frd call $say i32.const 256 i32.const 8 call $txt";
    let log = run_api(imports, data, body);
    assert_eq!(
        log,
        exp![
            said(23),
            texted("v3/aZZZZ"),
            said(3),
            // "a\nb" cut to its first 2 bytes over the Z fill, and the file with cap 0 leaves it alone.
            texted("a\n/aZZZZ"),
            said(3),
            texted("a\n/aZZZZ"),
        ]
    );
}

#[test]
fn out_buffer_out_of_bounds_traps() {
    let imports = r#"(import "acid" "local_time" (func $lt (param i32 i32) (result i32)))"#;
    let (end, _) = run(&api_cart(imports, "", "i32.const 131000 i32.const 1000 call $lt drop"), vec![], WasmLimits::default());
    assert!(matches!(end, CartEnd::Trap(_)), "{end:?}");
}

#[test]
fn reading_a_big_file_into_nothing_still_costs_fuel() {
    // The host builds a 1 MiB result on every call; with cap 0 nothing is
    // copied, but the production must still be paid for, or this loop would
    // never end (§15.2).
    let imports = r#"(import "acid" "fs_read" (func $frd (param i32 i32 i32 i32) (result i32)))"#;
    let data = r#"(data (i32.const 0) "v3/big")"#;
    let (end, _) = run(&api_cart(imports, data, "(loop i32.const 0 i32.const 6 i32.const 0 i32.const 0 call $frd drop br 0)"), vec![], WasmLimits { fuel: 10_000_000, ..WasmLimits::default() });
    assert_eq!(end, CartEnd::StoppedResponding);
}

#[test]
fn cart_gets_minus_4_closing_and_0_opening_overlay() {
    let imports = r#"
  (import "acid" "overlay_open" (func $oo (result i32)))
  (import "acid" "close_window" (func $cw (param i32) (result i32)))"#;
    let log = run_api(imports, "", "i32.const 1 call $cw call $say call $oo call $say");
    assert_eq!(log, exp![said(-4), said(0)]);
}

#[test]
fn import_names_match_the_linker() {
    use std::collections::BTreeSet;
    use acid_wasm::IMPORT_NAMES;
    assert_eq!(IMPORT_NAMES.len(), 56);
    assert_eq!(IMPORT_NAMES.iter().collect::<BTreeSet<_>>().len(), 56, "duplicate import name");
    // wasmi's Linker cannot list or `get` host functions, so probe each name:
    // import it with a signature no real import has. A defined name fails on
    // the signature; an undefined one fails on the missing definition.
    let refusal = |name: &str| {
        let wat = format!(r#"(module (import "acid" "{name}" (func (param f64 f64 f64 f64 f64 f64 f64 f64 f64 f64 f64))))"#);
        match run_cart(Rec::new(vec![]), &wat::parse_str(wat).unwrap(), WasmLimits::default()) {
            CartEnd::Refused(m) => m,
            other => panic!("{name}: expected a refusal, got {other:?}"),
        }
    };
    let missing = refusal("no_such_import");
    assert!(missing.contains("no_such_import"), "{missing}");
    for name in IMPORT_NAMES {
        let m = refusal(name);
        assert_ne!(m, missing.replace("no_such_import", name), "{name} is not defined by the linker: {m}");
    }
    // Fragile by nature: this counts `func_wrap(` in the source text, so an
    // import defined through a helper or macro would not be counted, and a
    // `func_wrap(` in a comment would be. It only backs up the probe above.
    // The set is also complete: the linker defines exactly as many functions as listed.
    let src = include_str!("../src/abi.rs");
    let link = src.split("pub(crate) fn link").nth(1).unwrap().split("#[cfg(test)]").next().unwrap();
    assert_eq!(link.matches("func_wrap(").count(), IMPORT_NAMES.len(), "link defines a different number of imports");
}
