//! WebAssembly carts (spec §15): runs a `.wasm` module under `wasmi` with
//! the callback ABI v1 (§15.3). The host owns the event loop and calls the
//! cart's exports, each call on a fresh fuel budget (§15.2) and under a
//! linear-memory cap, with the `acid_*` calls bound as imports from module
//! `"acid"`.
#![cfg_attr(not(any(test, feature = "std")), no_std)]

extern crate alloc;

mod abi;
mod runner;

use alloc::format;
use alloc::string::String;
use alloc::sync::Arc;

use acid_api::{AcidApi, KernelApi};
use acid_kernel::{AppContext, AppRunner};

pub use abi::err_code;
pub use runner::run_cart;

/// The only ABI a cart may declare through `acid_abi_version` (§15.3).
pub const ABI_VERSION: i32 = 1;
/// Fuel per callback: on the order of a second of interpreted code (§15.2).
pub const WASM_FUEL: u64 = 200_000_000;
/// Linear-memory cap in 64 KiB pages: 16 MB (§15.2).
pub const WASM_MAX_PAGES: u32 = 256;
/// Table-element cap. Tables live in host memory outside the linear-memory
/// cap, so without this a cart could allocate gigabytes through them (§15.2).
pub const WASM_MAX_TABLE_ELEMENTS: u32 = 65_536;
/// At most one table: one is all a compiled cart needs (§15.2).
pub const WASM_MAX_TABLES: u32 = 1;

/// Every import in module `"acid"`, in the ABI table's order (§15.3). A test
/// keeps this equal to what `abi::link` defines; chapter 10 of the manual
/// lists exactly these.
pub const IMPORT_NAMES: &[&str] = &[
    "now_ms",
    "notify_redraw_done",
    "fill_rect",
    "fill_circle",
    "draw_text",
    "draw_window_frame",
    "draw_window_border",
    "clear_user_area",
    "am_i_focused",
    "launch_arg",
    "play_note",
    "stop_note",
    "configure_voice",
    "configure_filter",
    "trigger_arp",
    "configure_osc",
    "set_ring_partner",
    "set_volume",
    "get_volume",
    "active_voice_count",
    "overlay_open",
    "overlay_clear",
    "overlay_fill_rect",
    "overlay_close",
    "repaint_region",
    "set_wallpaper_enabled",
    "get_wallpaper_enabled",
    "window_max",
    "screen_w",
    "screen_h",
    "font_w",
    "font_h",
    "window_w",
    "window_h",
    "get_font_scale",
    "set_font_scale",
    "window_info",
    "activate_window",
    "close_window",
    "send_self_to_back",
    "launcher_register",
    "launcher_count",
    "launcher_path",
    "launcher_name",
    "launcher_spawn",
    "spawn_app",
    "composited_frames",
    "skipped_frames",
    "local_time",
    "mem_used_kb",
    "network_info",
    "refresh_tasks",
    "task_count",
    "task_info",
    "fs_list",
    "fs_read",
    "fs_size",
    "fs_write",
    "fs_rename",
    "fs_delete",
    "cart_roots",
    "cart_list",
    "cart_stat",
    "cart_read",
];

/// Per-cart resource limits; tests shrink them, the OS uses the defaults.
#[derive(Clone, Copy, Debug)]
pub struct WasmLimits {
    pub fuel: u64,
    pub max_pages: u32,
}

impl Default for WasmLimits {
    fn default() -> Self {
        Self { fuel: WASM_FUEL, max_pages: WASM_MAX_PAGES }
    }
}

/// How a cart's run ended (§15.2: every case removes the window).
#[derive(Debug, PartialEq)]
pub enum CartEnd {
    /// The close event arrived, and `acid_on_destroy` (if any) returned.
    Closed,
    /// The module never ran: it failed to compile, missed a required export
    /// or import, or declared another ABI version.
    Refused(String),
    /// A callback used up its fuel budget.
    StoppedResponding,
    /// A callback trapped; the message says why.
    Trap(String),
    /// Memory or a table was declared, or grown, past its cap (§15.2).
    OutOfMemory,
}

/// The kernel's log line for a cart that ended badly. Logging needs `std`;
/// without it (firmware) the line is dropped until the platform grows a log.
#[cfg(any(test, feature = "std"))]
fn log(line: &str) {
    extern crate std;
    std::eprintln!("{line}");
}
#[cfg(not(any(test, feature = "std")))]
fn log(_line: &str) {}

/// The `AppRunner` for `.wasm` scripts (§15.2). A WASM cart is always
/// cart-level (`app_is_cart` only trusts `.lua`), so `KernelApi` applies the
/// cart rules to its calls. Every way a cart ends returns from the runner,
/// which is what removes its window.
pub fn wasm_runner(limits: WasmLimits) -> AppRunner {
    Arc::new(move |ctx: AppContext| {
        let path = ctx.script_path.clone();
        if !path.ends_with(".wasm") || !acid_kernel::fs_path::fs_path_is_allowed(&path) {
            log(&format!("Acid OS v3: refused unsafe script path {path}"));
            return;
        }
        let bytes = match ctx.kernel.platform().fs().read(&path) {
            Ok(b) => b,
            Err(e) => {
                log(&format!("Acid OS v3: {path}: refused: could not read it: {e:?}"));
                return;
            }
        };
        let api: Arc<dyn AcidApi> = Arc::new(KernelApi::new(ctx));
        match run_cart(api, &bytes, limits) {
            CartEnd::Closed => {}
            CartEnd::Refused(why) => log(&format!("Acid OS v3: {path}: refused: {why}")),
            CartEnd::StoppedResponding => log(&format!("Acid OS v3: {path}: stopped responding")),
            CartEnd::OutOfMemory => log(&format!("Acid OS v3: {path}: out of memory")),
            CartEnd::Trap(t) => log(&format!("Acid OS v3: {path}: {t}")),
        }
    })
}
