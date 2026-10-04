//! Boot: start the kernel, install the Lua VM host and spawn the desktop
//! alone, full height; every other app opens from the desktop's Menu.

use std::sync::Arc;

use acid_kernel::layout::Screen;
use acid_kernel::manifest::parse_manifest;
use acid_kernel::placement::cascade_position;
use acid_kernel::{AppContext, AppRunner, Kernel, SpawnRequest, TaskId};
use acid_platform::Platform;

pub const APPS_DIR: &str = "v3/apps";
/// The desktop window's height: it spawns screen-wide and 204 tall, which is
/// desktop.lua's TOTAL_H (strip plus the open dropdown's room).
pub const DESKTOP_H: i32 = 204;
pub const DESKTOP_PATH: &str = "v3/apps/desktop.lua";
pub const HELLO_PATH: &str = "v3/apps/hello_acid.lua";

/// The OS's app runner: `.wasm` scripts run under acid-wasm (always
/// cart-level, spec 15), everything else under the Lua VM.
pub fn runner(apps_dir: &str) -> AppRunner {
    runner_with(acid_lua::lua_runner(apps_dir), acid_wasm::wasm_runner(acid_wasm::WasmLimits::default()))
}

/// `runner` with both halves supplied, for tests that shrink the limits.
pub fn runner_with(lua: AppRunner, wasm: AppRunner) -> AppRunner {
    Arc::new(move |ctx: AppContext| {
        if ctx.script_path.ends_with(".wasm") {
            wasm(ctx)
        } else {
            lua(ctx)
        }
    })
}

/// Boots at the default screen size.
pub fn boot(platform: Arc<dyn Platform>) -> Arc<Kernel> {
    boot_with(platform, Screen::DEFAULT)
}

pub fn boot_with(platform: Arc<dyn Platform>, screen: Screen) -> Arc<Kernel> {
    let kernel = Kernel::with_screen(platform, screen);
    kernel.set_runner(runner(APPS_DIR));
    // The full 204 px window, so the dropdown has room (spec 11.4).
    let desktop = kernel.spawn_app(SpawnRequest {
        script_path: DESKTOP_PATH.into(),
        x: 0,
        y: 0,
        w: screen.w,
        h: DESKTOP_H,
        closable: false,
        arg: None,
        libs: None,
        force_cart: false,
    });
    if desktop.is_none() {
        eprintln!("Acid OS v3: could not start {DESKTOP_PATH}");
    }
    kernel.set_desktop_task(desktop);
    kernel
}

/// Launches `<APPS_DIR>/<name>.lua` (`.wasm` when the manifest says
/// `runtime = wasm`, spec 15.4) using its `.app.toml` for size and
/// libs, at the cascade position for the next window.
pub fn spawn_from_manifest(kernel: &Arc<Kernel>, name: &str) -> Option<TaskId> {
    let text = kernel.platform().fs().read(&format!("{APPS_DIR}/{name}.app.toml")).ok()?;
    let fields = parse_manifest(&String::from_utf8_lossy(&text));
    let w: i32 = fields.get("w")?.parse().ok()?;
    let h: i32 = fields.get("h")?.parse().ok()?;
    let (x, y) = cascade_position(kernel.screen(), kernel.with_state(|st| st.windows.count()), w, h);
    let ext = if fields.get("runtime").map(String::as_str) == Some("wasm") { "wasm" } else { "lua" };
    kernel.spawn_app(SpawnRequest {
        script_path: format!("{APPS_DIR}/{name}.{ext}"),
        x,
        y,
        w,
        h,
        closable: true,
        arg: None,
        libs: fields.get("libs").cloned(),
        force_cart: false,
    })
}

/// The name after `--app` on the command line, if any (spec 10.5).
pub fn app_arg(args: impl IntoIterator<Item = String>) -> Option<String> {
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        if a == "--app" {
            return it.next();
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::app_arg;

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn app_arg_finds_the_name_after_the_flag() {
        assert_eq!(app_arg(args(&["--app", "tetris"])), Some("tetris".into()));
        assert_eq!(app_arg(args(&["-x", "--app", "piano"])), Some("piano".into()));
        assert_eq!(app_arg(args(&[])), None);
        assert_eq!(app_arg(args(&["--app"])), None, "a flag with no name is ignored");
    }
}
