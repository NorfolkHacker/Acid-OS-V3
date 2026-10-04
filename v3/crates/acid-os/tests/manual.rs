//! The v3 manual's examples run (spec §16.4): every ```lua app fence runs as
//! a built-in app under the real kernel, every ```lua snippet parses, every
//! acid_* name in the Lua chapters is a real global, every ```wat fence
//! assembles (and, if it is a cart, runs as one), and chapter 10's import
//! table matches acid-wasm, names and signatures.
//!
//! Every ```lua app must leave its window open with the border drawn:
//! `AcidApp`'s default `redraw` does this, and a custom `redraw` must call
//! `acid_draw_window_border`.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use acid_gfx::rgb565;
use acid_kernel::theme::THEME_HARD;
use acid_kernel::{Kernel, SpawnRequest, TaskId};
use acid_os::{APPS_DIR, runner};
use acid_testkit::FakePlatform;

/// A throwaway copy of v3/apps (plus an empty Home), removed on drop.
struct TempTree(PathBuf);

impl TempTree {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("acid-os-manual-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        copy_dir(&FakePlatform::repo_root().join("v3/apps"), &dir.join("v3/apps"));
        std::fs::create_dir_all(dir.join("v3/fsroot/Home")).unwrap();
        TempTree(dir)
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let ty = e.file_type().unwrap();
        if ty.is_dir() {
            copy_dir(&e.path(), &to.join(e.file_name()));
        } else if ty.is_file() {
            std::fs::copy(e.path(), to.join(e.file_name())).unwrap();
        }
    }
}

fn manual_dir() -> PathBuf {
    FakePlatform::repo_root().join("docs/manual-v3")
}

/// Every .md in the manual, sorted by name.
fn chapters() -> Vec<(String, String)> {
    let mut v: Vec<_> = std::fs::read_dir(manual_dir())
        .expect("docs/manual-v3 exists")
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "md"))
        .map(|p| (p.file_name().unwrap().to_string_lossy().into_owned(), std::fs::read_to_string(&p).unwrap()))
        .collect();
    v.sort();
    v
}

struct Fence {
    file: String,
    line: usize,
    /// Line of the closing fence.
    end: usize,
    info: String,
    body: String,
}

/// Strips up to `max` blockquote markers (up to 3 spaces, `>`, one optional
/// space) from the start of a line; returns how many it stripped and the rest.
fn unquote(line: &str, max: usize) -> (usize, &str) {
    let mut rest = line;
    let mut depth = 0;
    while depth < max {
        let lead = rest.bytes().take(3).take_while(|&b| b == b' ').count();
        let Some(r) = rest[lead..].strip_prefix('>') else { break };
        rest = r.strip_prefix(' ').unwrap_or(r);
        depth += 1;
    }
    (depth, rest)
}

/// Every fenced code block (a fence line may be indented, or sit inside a
/// blockquote, nested or not) with its info string. A fence closes on a
/// fence line of at least the opening length with no info string, so a
/// longer fence can contain shorter ones. Body lines lose the opening
/// fence's blockquote markers, then are dedented by its indentation (at most
/// that many spaces).
fn fences(file: &str, text: &str) -> Vec<Fence> {
    let mut out = Vec::new();
    // (line, quote depth, indent, ticks, info, body)
    let mut open: Option<(usize, usize, usize, usize, String, String)> = None;
    for (n, raw) in text.lines().enumerate() {
        let depth = open.as_ref().map_or(usize::MAX, |o| o.1);
        let (quotes, line) = unquote(raw, depth);
        let t = line.trim_start();
        let ticks = t.chars().take_while(|&c| c == '`').count();
        let rest = t[ticks..].trim();
        let closes = matches!(&open, Some((_, _, _, len, _, _)) if ticks >= *len && rest.is_empty());
        if closes {
            let (at, _, _, _, info, body) = open.take().unwrap();
            out.push(Fence { file: file.into(), line: at, end: n + 1, info, body });
        } else if let Some((_, _, indent, _, _, body)) = open.as_mut() {
            let strip = line.bytes().take(*indent).take_while(|&b| b == b' ').count();
            body.push_str(&line[strip..]);
            body.push('\n');
        } else if ticks >= 3 {
            open = Some((n + 1, quotes, line.len() - t.len(), ticks, rest.to_string(), String::new()));
        }
    }
    assert!(open.is_none(), "{file}: unclosed code fence");
    out
}

fn all_fences() -> Vec<Fence> {
    chapters().iter().flat_map(|(f, t)| fences(f, t)).collect()
}

/// `-- key: value` from the example's leading comment lines.
fn header(body: &str, key: &str) -> Option<String> {
    body.lines()
        .take_while(|l| l.starts_with("--"))
        .find_map(|l| l.strip_prefix("--")?.trim().strip_prefix(key)?.strip_prefix(':').map(|v| v.trim().to_string()))
}

const TAGS: &[&str] = &["lua app", "lua snippet", "rust cart", "rust", "toml", "text", "sh", "wat"];

/// Panics on the first fence whose info string is not allowlisted.
fn check_tags(fences: &[Fence]) {
    for f in fences {
        assert!(TAGS.contains(&f.info.as_str()), "{}:{}: fence tag ```{} is not one of {TAGS:?}", f.file, f.line, f.info);
    }
}

#[test]
fn every_fence_is_tagged() {
    check_tags(&all_fences());
}

#[test]
fn fences_handles_indentation_and_long_fences() {
    let f = fences("t.md", "- item\n\n  ```lua app\n  x = 1\n  ```\n");
    assert_eq!((f.len(), f[0].info.as_str(), f[0].line), (1, "lua app", 3));
    assert_eq!(f[0].body, "x = 1\n", "the body is dedented by the fence's indentation");
    let f = fences("t.md", "````text\na\n```\nb\n````\n```sh\nls\n```\n");
    assert_eq!(f.len(), 2);
    assert_eq!(f[0].body, "a\n```\nb\n");
    assert_eq!(f[1].info, "sh");
}

#[test]
fn fences_inside_blockquotes_are_seen() {
    let f = fences("t.md", "> note\n>\n> ```lua snippet\n> x = 1\n>\n>   y = 2\n> ```\n");
    assert_eq!((f.len(), f[0].info.as_str(), f[0].line, f[0].end), (1, "lua snippet", 3, 7));
    assert_eq!(f[0].body, "x = 1\n\n  y = 2\n");
    // Nested quotes, with and without the optional space.
    let f = fences("t.md", "> > ```text\n>> a\n> >```\n");
    assert_eq!((f.len(), f[0].body.as_str()), (1, "a\n"));
    // A `>` line inside an unquoted fence is body, not a quote marker.
    let f = fences("t.md", "```text\n> prompt\n```\n");
    assert_eq!(f[0].body, "> prompt\n");
}

#[test]
fn header_reads_from_an_indented_fence() {
    let f = fences("t.md", "  ```lua app\n  -- w: 300\n  -- libs: a.lua\n  x = 1\n  ```\n");
    assert_eq!(header(&f[0].body, "w").as_deref(), Some("300"));
    assert_eq!(header(&f[0].body, "libs").as_deref(), Some("a.lua"));
}

#[test]
fn mentioned_skips_other_fences_by_the_same_parser() {
    let text = "````text\nacid_fake_call\n```\nacid_fake_two\n````\n```lua snippet\nacid_real_x()\n```\nprose acid_real_y\n";
    let m = mentioned(text);
    assert_eq!(m.into_iter().collect::<Vec<_>>(), ["acid_real_x", "acid_real_y"]);
}

#[test]
#[should_panic(expected = "t.md:1: fence tag")]
fn an_untagged_fence_is_rejected() {
    check_tags(&fences("t.md", "```\nplain\n```\n"));
}

#[test]
#[should_panic(expected = "t.md:2: fence tag ```lua is not")]
fn a_bare_lua_fence_is_rejected() {
    check_tags(&fences("t.md", "x\n```lua\nx = 1\n```\n"));
}

#[test]
fn every_snippet_parses() {
    let lua = mlua::Lua::new();
    for f in all_fences().into_iter().filter(|f| f.info == "lua snippet") {
        if let Err(e) = lua.load(&f.body).set_name(format!("{}:{}", f.file, f.line)).into_function() {
            panic!("{}:{}: snippet does not parse: {e}", f.file, f.line);
        }
    }
}

fn border_drawn(k: &Kernel, task: TaskId) -> Option<bool> {
    k.with_state(|st| st.windows.by_task(task).map(|w| w.canvas.lock().pixel(w.w / 2, w.h - 1) == Some(rgb565(THEME_HARD))))
}

#[test]
fn every_app_example_runs() {
    let apps: Vec<Fence> = all_fences().into_iter().filter(|f| f.info == "lua app").collect();
    assert!(!apps.is_empty(), "the manual has no ```lua app examples");
    let tree = TempTree::new("apps");
    let p = FakePlatform::new(&tree.0);
    let k = Kernel::new(p);
    k.set_runner(runner(APPS_DIR));
    for (i, f) in apps.iter().enumerate() {
        let where_ = format!("{}:{}", f.file, f.line);
        let rel = format!("v3/apps/manual_example_{i}.lua");
        std::fs::write(tree.0.join(&rel), &f.body).unwrap();
        let w = header(&f.body, "w").map_or(200, |v| v.parse().expect("-- w: number"));
        let h = header(&f.body, "h").map_or(150, |v| v.parse().expect("-- h: number"));
        let task = k
            .spawn_app(SpawnRequest { script_path: rel, x: 0, y: 0, w, h, closable: true, arg: None, libs: header(&f.body, "libs"), force_cart: false })
            .unwrap_or_else(|| panic!("{where_}: spawn failed"));
        k.activate_window(task);
        let start = Instant::now();
        loop {
            match border_drawn(&k, task) {
                Some(true) => break,
                Some(false) => {}
                None => panic!("{where_}: the example ended (an error?) before drawing its border"),
            }
            assert!(start.elapsed() < Duration::from_secs(5), "{where_}: never drew its window border within 5 s");
            std::thread::sleep(Duration::from_millis(10));
        }
        // Still alive a moment later: idle and redraw paths ran without error.
        std::thread::sleep(Duration::from_millis(500));
        assert!(border_drawn(&k, task).is_some(), "{where_}: the example ended after drawing (an error?)");
        k.close_window(task);
    }
}

/// Names of the custom sections in a wasm binary, in order.
fn custom_sections(wasm: &[u8]) -> Vec<String> {
    fn leb(b: &[u8], i: &mut usize) -> usize {
        let (mut v, mut shift) = (0usize, 0);
        loop {
            let byte = b[*i];
            *i += 1;
            v |= ((byte & 0x7f) as usize) << shift;
            if byte & 0x80 == 0 {
                return v;
            }
            shift += 7;
        }
    }
    assert_eq!(&wasm[..8], b"\0asm\x01\0\0\0", "not a wasm module");
    let (mut i, mut out) = (8, Vec::new());
    while i < wasm.len() {
        let id = wasm[i];
        i += 1;
        let size = leb(wasm, &mut i);
        let end = i + size;
        if id == 0 {
            let mut j = i;
            let n = leb(wasm, &mut j);
            out.push(String::from_utf8_lossy(&wasm[j..j + n]).into_owned());
        }
        i = end;
    }
    out
}

#[test]
fn custom_sections_are_listed() {
    let wasm = wat::parse_str(r#"(module (@custom "acid" "x") (@custom "other" ""))"#).unwrap();
    assert_eq!(custom_sections(&wasm), ["acid", "other"]);
}

/// Every ```wat fence assembles with the `wat` crate. One that declares the
/// `acid` header keeps it, and one that exports `acid_on_create` is a cart:
/// it runs under the real kernel and must draw its window border.
#[test]
fn every_wat_example_runs() {
    let wats: Vec<Fence> = all_fences().into_iter().filter(|f| f.info == "wat").collect();
    assert!(!wats.is_empty(), "the manual has no ```wat examples");
    let tree = TempTree::new("wat");
    let k = Kernel::new(FakePlatform::new(&tree.0));
    k.set_runner(runner(APPS_DIR));
    let mut carts = 0;
    for (i, f) in wats.iter().enumerate() {
        let where_ = format!("{}:{}", f.file, f.line);
        let wasm = wat::parse_str(&f.body).unwrap_or_else(|e| panic!("{where_}: does not assemble: {e}"));
        if f.body.contains(r#"(@custom "acid""#) {
            assert!(custom_sections(&wasm).iter().any(|n| n == "acid"), "{where_}: the `acid` header was dropped");
        }
        if !f.body.contains(r#"(export "acid_on_create")"#) {
            continue;
        }
        carts += 1;
        let rel = format!("v3/apps/manual_wat_{i}.wasm");
        std::fs::write(tree.0.join(&rel), &wasm).unwrap();
        let task = k
            .spawn_app(SpawnRequest { script_path: rel, x: 0, y: 0, w: 200, h: 150, closable: true, arg: None, libs: None, force_cart: false })
            .unwrap_or_else(|| panic!("{where_}: spawn failed"));
        k.activate_window(task);
        let start = Instant::now();
        loop {
            match border_drawn(&k, task) {
                Some(true) => break,
                Some(false) => {}
                None => panic!("{where_}: the cart ended (refused or trapped?) before drawing its border"),
            }
            assert!(start.elapsed() < Duration::from_secs(5), "{where_}: never drew its window border within 5 s");
            std::thread::sleep(Duration::from_millis(10));
        }
        std::thread::sleep(Duration::from_millis(500));
        assert!(border_drawn(&k, task).is_some(), "{where_}: the cart ended after drawing (a trap?)");
        k.close_window(task);
    }
    assert!(carts > 0, "no ```wat fence is a runnable cart");
}

/// Words that look like calls but are file or crate names.
const NOT_CALLS: &[&str] = &["acid_app", "acid_game", "acid_keys", "acid_palette", "acid_sprite", "acid_waveform", "acid_eggs", "acid_blaster", "acid_cart", "acid_wasm", "acid_os", "acid_lua", "acid_api", "acid_kernel", "acid_ship"];

/// The acid_* globals a fresh built-in app sees (core libs loaded),
/// collected by an app that writes them to Home and quits.
fn live_lua_names() -> BTreeSet<String> {
    let tree = TempTree::new("names");
    let script = r#"
local names = {}
for k in pairs(_G) do
  if type(k) == "string" and k:match("^acid_") then names[#names + 1] = k end
end
table.sort(names)
acid_fs_write("v3/fsroot/Home/names.txt", table.concat(names, "\n"))
"#;
    std::fs::write(tree.0.join("v3/apps/names_probe.lua"), script).unwrap();
    let k = Kernel::new(FakePlatform::new(&tree.0));
    k.set_runner(runner(APPS_DIR));
    k.spawn_app(SpawnRequest { script_path: "v3/apps/names_probe.lua".into(), x: 0, y: 0, w: 50, h: 50, closable: true, arg: None, libs: None, force_cart: false })
        .expect("spawn");
    let out = tree.0.join("v3/fsroot/Home/names.txt");
    let start = Instant::now();
    while !out.exists() {
        assert!(start.elapsed() < Duration::from_secs(5), "names probe never wrote");
        std::thread::sleep(Duration::from_millis(10));
    }
    std::thread::sleep(Duration::from_millis(50));
    std::fs::read_to_string(out).unwrap().lines().map(String::from).collect()
}

/// acid_* words in the text, skipping file names (`acid_app.lua`) and
/// fenced Rust/WAT code.
fn mentioned(text: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let fs = fences("", text);
    for (n, line) in text.lines().enumerate() {
        let no = n + 1;
        // Fence lines themselves, and the bodies of non-lua fences, are skipped.
        if fs.iter().any(|f| no == f.line || no == f.end || (no > f.line && no < f.end && !f.info.starts_with("lua"))) {
            continue;
        }
        let bytes = line.as_bytes();
        let mut i = 0;
        while let Some(off) = line[i..].find("acid_") {
            let s = i + off;
            let before_ok = s == 0 || !(bytes[s - 1].is_ascii_alphanumeric() || bytes[s - 1] == b'_');
            let mut e = s + 5;
            while e < bytes.len() && (bytes[e].is_ascii_lowercase() || bytes[e].is_ascii_digit() || bytes[e] == b'_') {
                e += 1;
            }
            let word = &line[s..e];
            let file_name = line[e..].starts_with(".lua") || line[e..].starts_with(".wasm");
            if before_ok && !file_name && word.len() > 5 && !NOT_CALLS.contains(&word) {
                out.insert(word.to_string());
            }
            i = e;
        }
    }
    out
}

#[test]
fn every_acid_name_in_the_lua_chapters_exists() {
    let live = live_lua_names();
    assert!(live.contains("acid_fill_rect"), "probe sanity: {live:?}");
    let mut missing = Vec::new();
    for (file, text) in chapters() {
        if file.starts_with("10-") {
            continue; // the WASM chapter names ABI imports, checked below
        }
        for name in mentioned(&text) {
            if !live.contains(&name) {
                missing.push(format!("{file}: {name}"));
            }
        }
    }
    assert!(missing.is_empty(), "the manual mentions calls v3 does not have:\n{}", missing.join("\n"));
}

/// One import-table row: name, parameter count, and result type ("" for none).
fn import_rows() -> Vec<(String, usize, &'static str)> {
    let path = manual_dir().join("10-wasm-carts.md");
    let text = std::fs::read_to_string(&path).expect("10-wasm-carts.md");
    // Rows of the import table: | `name` | signature | ... under "## The import table".
    let section = text.split("## The import table").nth(1).expect("10-wasm-carts.md has '## The import table'");
    let section = section.split("\n## ").next().unwrap();
    section
        .lines()
        .filter(|l| l.starts_with("| `"))
        .map(|l| {
            let cells: Vec<&str> = l.split(" | ").collect();
            let name = cells[0].trim_start_matches("| `").trim_end_matches('`').to_string();
            parse_signature(&name, cells[1])
        })
        .collect()
}

/// `params → result` as the table writes it: params are comma-separated
/// (all i32), the result is i64 if it says so, i32 if present, else none.
fn parse_signature(name: &str, sig: &str) -> (String, usize, &'static str) {
    let (params, result) = sig.split_once('→').unwrap_or((sig, ""));
    let params = params.split(',').filter(|p| !p.trim().is_empty()).count();
    let result = match result.trim() {
        "" => "",
        r if r.contains("i64") => "i64",
        _ => "i32",
    };
    (name.to_string(), params, result)
}

#[test]
fn parse_signature_reads_the_table_forms() {
    assert_eq!(parse_signature("a", "→ i64"), ("a".into(), 0, "i64"));
    assert_eq!(parse_signature("b", "→"), ("b".into(), 0, ""));
    assert_eq!(parse_signature("c", "x, y, w, h, color"), ("c".into(), 5, ""));
    assert_eq!(parse_signature("d", "index, buf, cap → len \\| −1"), ("d".into(), 3, "i32"));
    assert_eq!(parse_signature("e", "ptr, len → i64 size \\| err"), ("e".into(), 2, "i64"));
}

#[test]
fn the_wasm_import_table_matches_acid_wasm() {
    let rows = import_rows();
    let listed: BTreeSet<String> = rows.iter().map(|r| r.0.clone()).collect();
    assert_eq!(listed.len(), rows.len(), "an import is listed twice");
    let real: BTreeSet<String> = acid_wasm::IMPORT_NAMES.iter().map(|s| s.to_string()).collect();
    assert_eq!(listed, real, "chapter 10's import table must list exactly acid-wasm's imports");

    // Signatures: a module importing each name with the documented signature
    // must instantiate under acid-wasm's linker. It declares ABI version 2,
    // so a module that got that far is refused with exactly that message;
    // a wrong signature is refused at instantiation instead.
    let k = Kernel::new(FakePlatform::new("."));
    let (tx, rx) = std::sync::mpsc::channel();
    let tx = std::sync::Mutex::new(tx);
    k.set_runner(std::sync::Arc::new(move |ctx: acid_kernel::AppContext| {
        let _ = tx.lock().unwrap().send(ctx);
        loop {
            std::thread::park();
        }
    }));
    k.spawn_app(SpawnRequest { script_path: "v3/apps/probe.lua".into(), x: 0, y: 0, w: 50, h: 50, closable: true, arg: None, libs: None, force_cart: false })
        .expect("spawn");
    let api: std::sync::Arc<dyn acid_api::AcidApi> =
        std::sync::Arc::new(acid_api::KernelApi::new(rx.recv_timeout(Duration::from_secs(5)).unwrap()));
    let mut wrong = Vec::new();
    for (name, params, result) in &rows {
        let params = " i32".repeat(*params);
        let result = if result.is_empty() { String::new() } else { format!("(result {result})") };
        let wat = format!(
            r#"(module (import "acid" "{name}" (func (param{params}) {result})) (memory (export "memory") 1)
              (func (export "acid_abi_version") (result i32) (i32.const 2))
              (func (export "acid_on_create")) (func (export "acid_on_idle")) (func (export "acid_redraw"))
              (func (export "acid_on_event") (param i32 i32 i32 i32)))"#
        );
        match acid_wasm::run_cart(api.clone(), &wat::parse_str(&wat).unwrap(), acid_wasm::WasmLimits::default()) {
            acid_wasm::CartEnd::Refused(m) if m.starts_with("ABI version 2") => {}
            other => wrong.push(format!("{name} (param{params}) {result}: {other:?}")),
        }
    }
    assert!(wrong.is_empty(), "chapter 10's import table has wrong signatures:\n{}", wrong.join("\n"));
}

#[test]
fn the_manual_is_complete() {
    let want = [
        "README.md", "01-getting-started.md", "02-apps-and-manifests.md", "03-app-lifecycle.md",
        "04-graphics.md", "05-sound.md", "06-games.md", "07-system-apis.md", "08-cookbook.md",
        "09-api-reference.md", "10-wasm-carts.md",
    ];
    let have: Vec<String> = chapters().into_iter().map(|(f, _)| f).collect();
    assert_eq!(have, { let mut w: Vec<String> = want.iter().map(|s| s.to_string()).collect(); w.sort(); w });
    // Every relative .md link in the README points at a real file.
    let readme = std::fs::read_to_string(manual_dir().join("README.md")).unwrap();
    for part in readme.split("](").skip(1) {
        let target = part.split(')').next().unwrap();
        if target.ends_with(".md") && !target.contains("://") {
            assert!(manual_dir().join(target).exists(), "README links a missing chapter: {target}");
        }
    }
}

/// GitHub's heading anchor: lowercase, punctuation other than `-` and `_`
/// dropped, each space a `-`. (Duplicates get `-1`, `-2`, ...: see `anchors`.)
fn slug(heading: &str) -> String {
    heading
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | ' '))
        .map(|c| if c == ' ' { '-' } else { c })
        .collect()
}

#[test]
fn slug_follows_github() {
    assert_eq!(slug("4.2 Coordinates and clipping"), "42-coordinates-and-clipping");
    assert_eq!(slug("6.1 `AcidGame`"), "61-acidgame");
    assert_eq!(slug("7.11 What a cart is refused"), "711-what-a-cart-is-refused");
    assert_eq!(slug("Fuel: host work -- and you"), "fuel-host-work----and-you");
}

/// Every heading anchor in a markdown file, outside code fences, with
/// GitHub's `-N` suffix on repeats.
fn anchors(text: &str) -> BTreeSet<String> {
    let fs = fences("", text);
    let mut out = BTreeSet::new();
    let mut seen: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for (n, line) in text.lines().enumerate() {
        let no = n + 1;
        if fs.iter().any(|f| no >= f.line && no <= f.end) {
            continue;
        }
        let hashes = line.chars().take_while(|&c| c == '#').count();
        if !(1..=6).contains(&hashes) || !line[hashes..].starts_with(' ') {
            continue;
        }
        let base = slug(line[hashes..].trim());
        let count = seen.entry(base.clone()).or_insert(0);
        out.insert(if *count == 0 { base.clone() } else { format!("{base}-{count}") });
        *count += 1;
    }
    out
}

/// `(line, target)` for every inline link `[text](target)` outside code
/// fences and inline code spans.
fn links(text: &str) -> Vec<(usize, String)> {
    let fs = fences("", text);
    let mut out = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let no = n + 1;
        if fs.iter().any(|f| no >= f.line && no <= f.end) {
            continue;
        }
        // Drop inline code: odd-numbered pieces between backticks.
        let prose: String = line.split('`').step_by(2).collect::<Vec<_>>().join(" ");
        for part in prose.split("](").skip(1) {
            if let Some(target) = part.split(')').next() {
                out.push((no, target.trim().to_string()));
            }
        }
    }
    out
}

#[test]
fn links_and_anchors_skip_code() {
    let text = "# 1.2 Top\n\n```text\n# not a heading [x](gone.md)\n```\nsee [a](#12-top), `[b](no.md)` and [c](b.md#x)\n## Top\n## 1.2 Top\n";
    assert_eq!(links(text), [(6, "#12-top".to_string()), (6, "b.md#x".to_string())]);
    assert_eq!(anchors(text).into_iter().collect::<Vec<_>>(), ["12-top", "12-top-1", "top"]);
}

#[test]
fn every_manual_link_resolves() {
    let dir = manual_dir();
    let mut broken = Vec::new();
    for (file, text) in chapters() {
        for (line, target) in links(&text) {
            if target.contains("://") || target.starts_with("mailto:") {
                continue;
            }
            let (path, anchor) = match target.split_once('#') {
                Some((p, a)) => (p, Some(a)),
                None => (target.as_str(), None),
            };
            let where_ = format!("{file}:{line}: ({target})");
            if path.is_empty() {
                if !anchors(&text).contains(anchor.unwrap_or_default()) {
                    broken.push(format!("{where_}: no such heading in {file}"));
                }
                continue;
            }
            let full = dir.join(path);
            if !full.exists() {
                broken.push(format!("{where_}: no such file"));
                continue;
            }
            // Only manual chapters are checked for anchors; a file outside
            // the manual only has to exist.
            let inside = !path.contains('/') && path.ends_with(".md");
            if let (true, Some(a)) = (inside, anchor) {
                if !anchors(&std::fs::read_to_string(&full).unwrap()).contains(a) {
                    broken.push(format!("{where_}: no such heading in {path}"));
                }
            }
        }
    }
    assert!(broken.is_empty(), "broken manual links:\n{}", broken.join("\n"));
}
