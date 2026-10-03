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
fn acid_sprite() {
    run_suite(&["v3/tools/game_test_env.lua", "v3/apps/lib/acid_sprite.lua", "v3/tools/test_acid_sprite.lua"], 13);
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
    run_suite(&["v3/tools/game_test_env.lua", "v3/apps/about.lua", "v3/tools/test_about.lua"], 4);
}

#[test]
fn config() {
    run_suite(&["v3/tools/game_test_env.lua", "v3/apps/config.lua", "v3/tools/test_config.lua"], 11);
}

#[test]
fn network() {
    run_suite(&["v3/tools/game_test_env.lua", "v3/apps/network.lua", "v3/tools/test_network.lua"], 5);
}

#[test]
fn sysmon() {
    run_suite(&["v3/tools/game_test_env.lua", "v3/apps/sysmon.lua", "v3/tools/test_sysmon.lua"], 23);
}

#[test]
fn desktop() {
    run_suite(&["v3/tools/game_test_env.lua", "v3/apps/desktop.lua", "v3/tools/test_desktop.lua"], 27);
}

#[test]
fn acid_eggs() {
    run_suite(&["v3/tools/game_test_env.lua", "v3/apps/lib/acid_sprite.lua", "v3/apps/lib/acid_eggs.lua", "v3/tools/test_acid_eggs.lua"], 194);
}

#[test]
fn terminal() {
    run_suite(&with_libs(&["v3/tools/game_test_env.lua", "v3/apps/lib/acid_sprite.lua", "v3/apps/lib/acid_eggs.lua", "v3/apps/terminal.lua", "v3/tools/test_terminal.lua"]), 26);
}

#[test]
fn file_manager() {
    run_suite(&with_libs(&["v3/tools/game_test_env.lua", "v3/apps/file_manager.lua", "v3/tools/test_file_manager.lua"]), 21);
}

#[test]
fn editor_modules() {
    run_suite(&["v3/tools/game_test_env.lua", "v3/apps/editor/buffer.lua", "v3/apps/editor/hl.lua", "v3/tools/test_editor.lua"], 97);
}

#[test]
fn editor_app() {
    run_suite(&with_libs(&[
        "v3/tools/game_test_env.lua", "v3/apps/editor/buffer.lua", "v3/apps/editor/hl.lua",
        "v3/apps/editor/layout.lua", "v3/apps/editor/cmdbar.lua", "v3/apps/editor/touch.lua",
        "v3/apps/editor.lua", "v3/tools/test_editor_app.lua",
    ]), 26);
}

#[test]
fn cartfile() {
    run_suite(&["v3/tools/game_test_env.lua", "v3/apps/cart/cartfile.lua", "v3/tools/test_cart.lua"], 133);
}

#[test]
fn cart_install() {
    run_suite(&with_libs(&["v3/tools/game_test_env.lua", "v3/apps/cart/cartfile.lua", "v3/apps/cart.lua", "v3/tools/test_cart_install.lua"]), 76);
}
