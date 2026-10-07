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

/// The folder after `--data` on the command line, if any: where the user's
/// own files live instead of the default (acid_platform's userdata).
pub fn data_arg(args: impl IntoIterator<Item = String>) -> Result<Option<String>, String> {
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        if a == "--data" {
            return it.next().filter(|v| !v.is_empty()).map(Some).ok_or_else(|| String::from("--data needs a folder"));
        }
        if let Some(v) = a.strip_prefix("--data=") {
            return if v.is_empty() { Err(String::from("--data needs a folder")) } else { Ok(Some(v.to_string())) };
        }
    }
    Ok(None)
}

/// The size after `--screen` on the command line, if any. Only the presets
/// are accepted; anything else is an error naming them.
pub fn screen_arg(args: impl IntoIterator<Item = String>) -> Result<Option<Screen>, String> {
    let valid = Screen::PRESETS.iter().map(|s| format!("{}x{}", s.w, s.h)).collect::<Vec<_>>().join(", ");
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        let v = if a == "--screen" {
            let Some(v) = it.next() else {
                return Err(format!("--screen needs a size: {valid}"));
            };
            v
        } else if let Some(v) = a.strip_prefix("--screen=") {
            v.to_string()
        } else {
            continue;
        };
        return Screen::parse(&v).map(Some).ok_or_else(|| format!("--screen {v}: not a supported size; use one of {valid}"));
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::{app_arg, data_arg, screen_arg};
    use acid_kernel::layout::Screen;

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn screen_arg_reads_a_preset() {
        assert_eq!(screen_arg(args(&[])), Ok(None));
        assert_eq!(screen_arg(args(&["--screen", "800x600"])), Ok(Some(Screen::SVGA)));
        assert_eq!(screen_arg(args(&["--app", "tetris", "--screen", "640x360"])), Ok(Some(Screen::WIDE)));
        assert_eq!(screen_arg(args(&["--screen=800x600"])), Ok(Some(Screen::SVGA)));
    }

    #[test]
    fn screen_arg_rejects_other_sizes_and_lists_the_valid_ones() {
        for bad in [&["--screen", "1024x768"][..], &["--screen"][..], &["--screen=1x1"][..]] {
            let e = screen_arg(args(bad)).unwrap_err();
            assert!(e.contains("640x480, 640x360, 800x600"), "{e}");
        }
    }

    #[test]
    fn app_arg_finds_the_name_after_the_flag() {
        assert_eq!(app_arg(args(&["--app", "tetris"])), Some("tetris".into()));
        assert_eq!(app_arg(args(&["-x", "--app", "piano"])), Some("piano".into()));
        assert_eq!(app_arg(args(&[])), None);
        assert_eq!(app_arg(args(&["--app"])), None, "a flag with no name is ignored");
    }

    #[test]
    fn data_arg_names_the_user_folder() {
        assert_eq!(data_arg(args(&["--data", "/tmp/a"])), Ok(Some("/tmp/a".into())));
        assert_eq!(data_arg(args(&["--screen", "640x480", "--data=b"])), Ok(Some("b".into())));
        assert_eq!(data_arg(args(&["--app", "tetris"])), Ok(None));
        assert!(data_arg(args(&["--data"])).is_err());
        assert!(data_arg(args(&["--data="])).is_err());
    }
}
