//! Runs the headless game tests and the behaviour tests for the system
//! apps (v3/tools/test_*.lua).
//! Each suite gets a fresh Lua state and its files in concatenation
//! order (libs, then game_test_env.lua, then the app, then the test).
//! The assertion count is pinned so a suite can't pass by asserting less.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use acid_lua::mlua::{Lua, LuaOptions, StdLib, Table};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

const GAME_LIBS: [&str; 3] = [
    "v3/apps/lib/acid_keys.lua",
    "v3/apps/lib/acid_palette.lua",
    "v3/apps/lib/acid_waveform.lua",
];

fn run_suite(files: &[&str], assertions: usize) {
    run_suite_with("", files, assertions);
}

fn run_suite_with(prelude: &str, files: &[&str], assertions: usize) {
    let lua = Lua::new_with(StdLib::STRING | StdLib::TABLE | StdLib::MATH, LuaOptions::default()).unwrap();
    // Lua's print goes to a buffer so passing runs are silent; a failure
    // includes it.
    let out = Arc::new(Mutex::new(String::new()));
    let sink = out.clone();
    let print = lua
        .create_function(move |_, args: acid_lua::mlua::Variadic<acid_lua::mlua::Value>| {
            let line: Vec<String> = args.iter().map(|v| v.to_string().unwrap_or_default()).collect();
            let mut o = sink.lock().unwrap();
            o.push_str(&line.join("\t"));
            o.push('\n');
            Ok(())
        })
        .unwrap();
    lua.globals().set("print", print).unwrap();
    let printed = || out.lock().unwrap().clone();
    if !prelude.is_empty() {
        lua.load(prelude).set_name("@prelude").exec().unwrap_or_else(|e| panic!("prelude: {e}"));
    }
    for f in files {
        let src = std::fs::read(repo_root().join(f)).unwrap_or_else(|e| panic!("{f}: {e}"));
        lua.load(&src[..]).set_name(format!("@{f}")).exec().unwrap_or_else(|e| panic!("{f}: {e}\nLua output:\n{}", printed()));
    }
    let fails: Table = lua.globals().get("FAILS").unwrap();
    let msgs: Vec<String> = fails.sequence_values::<String>().map(|m| m.unwrap()).collect();
    assert!(msgs.is_empty(), "{} failure(s):\n{}\nLua output:\n{}", msgs.len(), msgs.join("\n"), printed());
    let passes: usize = lua.globals().get("PASSES").unwrap();
    assert_eq!(passes, assertions, "assertion count");
}

fn with_libs(rest: &[&'static str]) -> Vec<&'static str> {
    GAME_LIBS.iter().copied().chain(rest.iter().copied()).collect()
}

#[test]
fn acid_scrollbar() {
    run_suite(&["v3/tools/game_test_env.lua", "v3/apps/lib/acid_scrollbar.lua", "v3/tools/test_acid_scrollbar.lua"], 23);
}

#[test]
fn acid_sprite() {
    run_suite(&["v3/tools/game_test_env.lua", "v3/apps/lib/acid_sprite.lua", "v3/tools/test_acid_sprite.lua"], 13);
}

#[test]
fn acid_sprite_format() {
    run_suite(&["v3/tools/game_test_env.lua", "v3/apps/lib/acid_sprite.lua", "v3/tools/test_acid_sprite_format.lua"], 40);
}

#[test]
fn shipped_sprite_files_parse() {
    // Each file goes to Lua as a long-bracket string; the first newline
    // after [==[ is dropped by Lua, so the text arrives unchanged.
    let mut prelude = String::from("SPR = {}\n");
    let mut count = 0;
    for entry in std::fs::read_dir(repo_root().join("v3/fsroot/Home")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "spr") {
            let text = std::fs::read_to_string(&path).unwrap();
            assert!(!text.contains("]==]"), "{} can't be quoted", path.display());
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            prelude.push_str(&format!("SPR[#SPR + 1] = {{ {name:?}, [==[\n{text}]==] }}\n"));
            count += 1;
        }
    }
    assert!(count >= 1, "acid_ship.spr is shipped");
    run_suite_with(&prelude, &["v3/tools/game_test_env.lua", "v3/apps/lib/acid_sprite.lua", "v3/tools/test_sprite_files.lua"], count);
}

#[test]
fn sprite_doc() {
    run_suite(&with_libs(&["v3/tools/game_test_env.lua", "v3/apps/lib/acid_sprite.lua", "v3/apps/sprite/doc.lua", "v3/tools/test_sprite_doc.lua"]), 43);
}

#[test]
fn sprite_tools() {
    run_suite(&with_libs(&["v3/tools/game_test_env.lua", "v3/apps/lib/acid_sprite.lua", "v3/apps/sprite/doc.lua", "v3/apps/sprite/tools.lua", "v3/tools/test_sprite_tools.lua"]), 21);
}

#[test]
fn sprite_layout() {
    run_suite(&with_libs(&["v3/tools/game_test_env.lua", "v3/apps/lib/acid_sprite.lua", "v3/apps/sprite/layout.lua", "v3/apps/sprite/picker.lua", "v3/tools/test_sprite_layout.lua"]), 29);
}

const SPRITE_APP: [&str; 7] = [
    "v3/tools/game_test_env.lua",
    "v3/apps/lib/acid_sprite.lua",
    "v3/apps/sprite/doc.lua",
    "v3/apps/sprite/tools.lua",
    "v3/apps/sprite/layout.lua",
    "v3/apps/sprite/picker.lua",
    "v3/apps/sprite.lua",
];

fn sprite_app_suite(test: &'static str, assertions: usize) {
    let files: Vec<&str> = SPRITE_APP.iter().copied().chain(["v3/tools/sprite_test_helpers.lua", test]).collect();
    run_suite_with("WIN_W, WIN_H = 360, 260", &with_libs(&files), assertions);
}

#[test]
fn sprite_app() {
    sprite_app_suite("v3/tools/test_sprite_app.lua", 29);
}

#[test]
fn sprite_app_commands() {
    sprite_app_suite("v3/tools/test_sprite_app_cmds.lua", 36);
}

#[test]
fn breakout() {
    run_suite(&with_libs(&["v3/tools/game_test_env.lua", "v3/apps/breakout.lua", "v3/tools/test_breakout.lua"]), 11);
}

#[test]
fn acid_blaster() {
    run_suite(&with_libs(&["v3/tools/game_test_env.lua", "v3/apps/acid_blaster.lua", "v3/tools/test_acid_blaster.lua"]), 27);
}

#[test]
fn tetris() {
    run_suite(&with_libs(&["v3/tools/game_test_env.lua", "v3/apps/tetris.lua", "v3/tools/test_tetris.lua"]), 24);
}

#[test]
fn piano() {
    run_suite(&with_libs(&["v3/tools/game_test_env.lua", "v3/apps/piano.lua", "v3/tools/test_piano.lua"]), 7);
}

#[test]
fn about() {
    run_suite_with("WIN_W, WIN_H = 180, 150", &["v3/tools/game_test_env.lua", "v3/apps/about.lua", "v3/tools/test_about.lua"], 4);
}

#[test]
fn config() {
    run_suite(&["v3/tools/game_test_env.lua", "v3/apps/config.lua", "v3/tools/test_config.lua"], 18);
}

#[test]
fn network() {
    run_suite_with("WIN_W, WIN_H = 200, 110", &["v3/tools/game_test_env.lua", "v3/apps/network.lua", "v3/tools/test_network.lua"], 5);
}

#[test]
fn sysmon() {
    run_suite_with("WIN_W, WIN_H = 200, 160", &["v3/tools/game_test_env.lua", "v3/apps/sysmon.lua", "v3/tools/test_sysmon.lua"], 25);
}

#[test]
fn desktop() {
    run_suite(&["v3/tools/game_test_env.lua", "v3/apps/desktop.lua", "v3/tools/test_desktop.lua"], 27);
}

#[test]
fn acid_spin() {
    run_suite_with("WIN_W, WIN_H = 240, 200", &with_libs(&["v3/tools/game_test_env.lua", "v3/apps/acid_spin.lua", "v3/tools/test_acid_spin.lua"]), 58);
}

#[test]
fn acid_eggs() {
    run_suite(&["v3/tools/game_test_env.lua", "v3/apps/lib/acid_sprite.lua", "v3/apps/lib/acid_eggs.lua", "v3/tools/test_acid_eggs.lua"], 194);
}

#[test]
fn terminal() {
    run_suite_with("WIN_W, WIN_H = 260, 160", &with_libs(&["v3/tools/game_test_env.lua", "v3/apps/lib/acid_sprite.lua", "v3/apps/lib/acid_eggs.lua", "v3/apps/terminal.lua", "v3/tools/test_terminal.lua"]), 26);
}

#[test]
fn terminal_large() {
    run_suite_with("FONT_W, FONT_H, WIN_W, WIN_H = 12, 16, 520, 304", &with_libs(&["v3/tools/game_test_env.lua", "v3/apps/lib/acid_sprite.lua", "v3/apps/lib/acid_eggs.lua", "v3/apps/terminal.lua", "v3/tools/test_terminal_large.lua"]), 3);
}

#[test]
fn file_manager() {
    run_suite_with("WIN_W, WIN_H = 220, 160", &with_libs(&["v3/tools/game_test_env.lua", "v3/apps/lib/acid_scrollbar.lua", "v3/apps/file_manager.lua", "v3/tools/test_file_manager.lua"]), 33);
}

#[test]
fn file_manager_large() {
    run_suite_with("FONT_W, FONT_H, WIN_W, WIN_H = 12, 16, 440, 304", &with_libs(&["v3/tools/game_test_env.lua", "v3/apps/lib/acid_scrollbar.lua", "v3/apps/file_manager.lua", "v3/tools/test_file_manager_large.lua"]), 4);
}

#[test]
fn editor_modules() {
    run_suite(&["v3/tools/game_test_env.lua", "v3/apps/editor/buffer.lua", "v3/apps/editor/hl.lua", "v3/tools/test_editor.lua"], 97);
}

#[test]
fn editor_app() {
    run_suite_with("WIN_W, WIN_H = 420, 280", &with_libs(&[
        "v3/tools/game_test_env.lua", "v3/apps/editor/buffer.lua", "v3/apps/editor/hl.lua",
        "v3/apps/editor/layout.lua", "v3/apps/editor/cmdbar.lua", "v3/apps/editor/touch.lua",
        "v3/apps/editor.lua", "v3/tools/test_editor_app.lua",
    ]), 26);
}

#[test]
fn editor_resize() {
    run_suite_with("WIN_W, WIN_H = 420, 280", &with_libs(&[
        "v3/tools/game_test_env.lua", "v3/apps/editor/buffer.lua", "v3/apps/editor/hl.lua",
        "v3/apps/editor/layout.lua", "v3/apps/editor/cmdbar.lua", "v3/apps/editor/touch.lua",
        "v3/apps/editor.lua", "v3/tools/test_resize_editor.lua",
    ]), 8);
}

#[test]
fn editor_large() {
    run_suite_with("FONT_W, FONT_H, WIN_W, WIN_H = 12, 16, 640, 456", &with_libs(&[
        "v3/tools/game_test_env.lua", "v3/apps/editor/buffer.lua", "v3/apps/editor/hl.lua",
        "v3/apps/editor/layout.lua", "v3/apps/editor/cmdbar.lua", "v3/apps/editor/touch.lua",
        "v3/apps/editor.lua", "v3/tools/test_editor_large.lua",
    ]), 6);
}

#[test]
fn cartfile() {
    run_suite(&["v3/tools/game_test_env.lua", "v3/apps/cart/cartfile.lua", "v3/tools/test_cart.lua"], 143);
}

#[test]
fn cart_install() {
    run_suite(&with_libs(&["v3/tools/game_test_env.lua", "v3/apps/cart/cartfile.lua", "v3/apps/cart.lua", "v3/tools/test_cart_install.lua"]), 76);
}

#[test]
fn apps_follow_the_screen_size() {
    run_suite(&[
        "v3/tools/screen_800x600.lua",
        "v3/tools/game_test_env.lua",
        "v3/apps/desktop.lua",
        "v3/apps/cart/cartfile.lua",
        "v3/apps/lib/acid_sprite.lua",
        "v3/apps/lib/acid_eggs.lua",
        "v3/tools/test_screen_svga.lua",
    ], 5);
}

#[test]
fn about_large() {
    run_suite_with("FONT_W, FONT_H, WIN_W, WIN_H = 12, 16, 360, 284",
        &["v3/tools/game_test_env.lua", "v3/apps/about.lua", "v3/tools/test_about_fits.lua"], 3);
}

#[test]
fn network_large() {
    run_suite_with("FONT_W, FONT_H, WIN_W, WIN_H = 12, 16, 400, 204",
        &["v3/tools/game_test_env.lua", "v3/apps/network.lua", "v3/tools/test_fits_window.lua"], 1);
}

#[test]
fn sysmon_large() {
    run_suite_with("FONT_W, FONT_H, WIN_W, WIN_H = 12, 16, 400, 304",
        &["v3/tools/game_test_env.lua", "v3/apps/sysmon.lua", "v3/tools/test_sysmon_large.lua"], 4);
}

#[test]
fn terminal_resize() {
    run_suite_with("WIN_W, WIN_H = 260, 160", &with_libs(&["v3/tools/game_test_env.lua", "v3/apps/lib/acid_sprite.lua", "v3/apps/lib/acid_eggs.lua", "v3/apps/terminal.lua", "v3/tools/test_resize_terminal.lua"]), 6);
}

#[test]
fn file_manager_resize() {
    run_suite_with("WIN_W, WIN_H = 220, 160", &with_libs(&["v3/tools/game_test_env.lua", "v3/apps/lib/acid_scrollbar.lua", "v3/apps/file_manager.lua", "v3/tools/test_resize_file_manager.lua"]), 9);
}

#[test]
fn about_network_fit_at_normal() {
    run_suite_with("WIN_W, WIN_H = 180, 150", &["v3/tools/game_test_env.lua", "v3/apps/about.lua", "v3/tools/test_about_fits.lua"], 3);
    run_suite_with("WIN_W, WIN_H = 200, 110", &["v3/tools/game_test_env.lua", "v3/apps/network.lua", "v3/tools/test_fits_window.lua"], 1);
}

#[test]
fn about_resize() {
    run_suite_with("WIN_W, WIN_H = 180, 150", &["v3/tools/game_test_env.lua", "v3/apps/about.lua", "v3/tools/test_resize_about.lua"], 3);
}

#[test]
fn network_resize() {
    run_suite_with("WIN_W, WIN_H = 200, 110", &["v3/tools/game_test_env.lua", "v3/apps/network.lua", "v3/tools/test_resize_network.lua"], 2);
}

#[test]
fn sysmon_resize() {
    run_suite_with("WIN_W, WIN_H = 200, 160", &["v3/tools/game_test_env.lua", "v3/apps/sysmon.lua", "v3/tools/test_resize_sysmon.lua"], 16);
}
