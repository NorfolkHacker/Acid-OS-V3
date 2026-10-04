//! The Lua VM host, for Lua 5.4. One fresh
//! Lua state per app, on that app's own thread: the core libs, then the
//! manifest's libs, then the app script. Whatever happens inside, the app
//! ends cleanly and the kernel removes its window (Kernel::spawn_app).

pub mod lib_paths;

pub use mlua;

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use acid_api::{AcidApi, KernelApi, MESH_FACES_MAX, MESH_POINTS_MAX, NO_INDEX, PolledEvent};
use acid_kernel::{AppContext, AppRunner};
use acid_platform::Fs;
use mlua::{ChunkMode, IntoLuaMulti, Lua, LuaOptions, MultiValue, StdLib, Value, ffi};

use crate::lib_paths::lib_path_is_safe;

/// Lua's flat 1-based mesh tables as the API's points and 0-based faces. The
/// structural problems the API cannot see ("bad mesh") are caught here.
/// Tables are read raw (no `__index`) and their length is checked against the
/// caps first, so no script can make the host build an unbounded Vec.
fn parse_mesh(points: &mlua::Table, faces: &mlua::Table) -> Result<(Vec<(i32, i32, i32)>, Vec<[u16; 4]>), String> {
    let bad = || String::from("bad mesh");
    let too_big = || String::from("too big");
    let n = points.raw_len();
    if n > 3 * MESH_POINTS_MAX {
        return Err(too_big());
    }
    if n % 3 != 0 {
        return Err(bad());
    }
    let mut pts = Vec::with_capacity(n / 3);
    for k in 0..n / 3 {
        let c = |j: usize| points.raw_get::<i32>(3 * k + j + 1).map_err(|_| bad());
        pts.push((c(0)?, c(1)?, c(2)?));
    }
    let nf = faces.raw_len();
    if nf > MESH_FACES_MAX {
        return Err(too_big());
    }
    let mut out = Vec::with_capacity(nf);
    for k in 1..=nf {
        let face: mlua::Table = faces.raw_get(k).map_err(|_| bad())?;
        let len = face.raw_len();
        if !(3..=4).contains(&len) {
            return Err(bad());
        }
        let mut f = [NO_INDEX; 4];
        for (j, slot) in f.iter_mut().enumerate().take(len) {
            let i: i64 = face.raw_get(j + 1).map_err(|_| bad())?;
            // Lua is 1-based; an index that cannot be a u16 (or would be
            // NO_INDEX) is out of range for any mesh.
            *slot = u16::try_from(i).ok().and_then(|v| v.checked_sub(1)).filter(|v| *v != NO_INDEX).ok_or_else(bad)?;
        }
        out.push(f);
    }
    Ok((pts, out))
}


/// Loaded into every app, in this order, before its own libs: keys,
/// palette, waveform, app, game.
pub const CORE_LIBS: &[&str] = &[
    "lib/acid_keys.lua",
    "lib/acid_palette.lua",
    "lib/acid_waveform.lua",
    "lib/acid_app.lua",
    "lib/acid_game.lua",
];

/// Per-app resource limits (spec 14.3): a Lua memory cap and the "stopped
/// responding" watchdog (ms an app may go without calling acid_poll_event).
#[derive(Clone, Copy, Debug)]
pub struct VmLimits {
    pub built_in_mem: usize,
    pub built_in_ms: u64,
    pub cart_mem: usize,
    pub cart_ms: u64,
}

impl Default for VmLimits {
    fn default() -> Self {
        VmLimits { built_in_mem: 64 << 20, built_in_ms: 2000, cart_mem: 16 << 20, cart_ms: 1000 }
    }
}

/// The production runner: refuses any script outside `v3/apps` / `v3/fsroot`.
pub fn lua_runner(apps_dir: &str) -> AppRunner {
    lua_runner_with(apps_dir, None, VmLimits::default())
}

/// Like `lua_runner`, but scripts under `extra_root` (a directory prefix
/// ending in `/`) are also accepted. Exists for tests whose fixture apps
/// live under `v3/crates/.../tests/fixtures/`; production code uses
/// `lua_runner`.
pub fn lua_runner_trusting(apps_dir: &str, extra_root: &str) -> AppRunner {
    lua_runner_with(apps_dir, Some(extra_root), VmLimits::default())
}

/// The general form: optional extra root and explicit limits.
pub fn lua_runner_with(apps_dir: &str, extra_root: Option<&str>, limits: VmLimits) -> AppRunner {
    let apps_dir = apps_dir.to_string();
    let extra_root = extra_root.map(str::to_string);
    Arc::new(move |ctx: AppContext| run_app(ctx, &apps_dir, extra_root.clone(), limits))
}

fn script_is_allowed(path: &str, extra_root: Option<&str>) -> bool {
    path.ends_with(".lua")
        && (acid_kernel::fs_path::fs_path_is_allowed(path)
            || extra_root.is_some_and(|r| path.starts_with(r) && lib_path_is_safe(&path[r.len()..])))
}

pub fn run_app(ctx: AppContext, apps_dir: &str, extra_root: Option<String>, limits: VmLimits) {
    let path = ctx.script_path.clone();
    let result = catch_unwind(AssertUnwindSafe(|| run_app_inner(ctx, apps_dir, extra_root, limits)));
    if let Err(panic) = result {
        let msg = panic
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| panic.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "unknown panic".into());
        eprintln!("Acid OS v3: {path}: panic: {msg}");
    }
}

fn run_app_inner(ctx: AppContext, apps_dir: &str, extra_root: Option<String>, limits: VmLimits) {
    let kernel = ctx.kernel.clone();
    let script_path = ctx.script_path.clone();
    if !script_is_allowed(&script_path, extra_root.as_deref()) {
        eprintln!("Acid OS v3: refused unsafe script path {script_path}");
        return;
    }
    let libs = ctx.libs.clone();
    let (mem, ms) = if ctx.cart { (limits.cart_mem, limits.cart_ms) } else { (limits.built_in_mem, limits.built_in_ms) };
    let api: Arc<dyn AcidApi> = Arc::new(KernelApi::new(ctx));
    let fs = kernel.platform().fs();
    let lua = match new_app_state_limited(api, fs, apps_dir, libs.as_deref(), Some((mem, ms))) {
        Ok(lua) => lua,
        Err(e) => {
            eprintln!("Acid OS v3: {script_path}: could not create VM: {e}");
            return;
        }
    };
    load_file(&lua, fs, &script_path);
}

/// A sandboxed Lua state with the acid_* API registered and the core libs
/// plus `libs` (a manifest's comma-separated list) loaded.
pub fn new_app_state(api: Arc<dyn AcidApi>, fs: &dyn Fs, apps_dir: &str, libs: Option<&str>) -> mlua::Result<Lua> {
    new_app_state_limited(api, fs, apps_dir, libs, None)
}

/// `new_app_state` with optional `(memory cap bytes, watchdog ms)`. The
/// watchdog covers the libs too; the memory cap counts the loaded libs.
pub fn new_app_state_limited(
    api: Arc<dyn AcidApi>,
    fs: &dyn Fs,
    apps_dir: &str,
    libs: Option<&str>,
    limits: Option<(usize, u64)>,
) -> mlua::Result<Lua> {
    // No io/os/package: file and system access only ever goes through the
    // acid_* API. dofile/loadfile are base-library file loaders, removed
    // for the same reason.
    let lua = Lua::new_with(
        StdLib::STRING | StdLib::TABLE | StdLib::MATH | StdLib::UTF8 | StdLib::COROUTINE,
        LuaOptions::default(),
    )?;
    lua.globals().set("dofile", Value::Nil)?;
    lua.globals().set("loadfile", Value::Nil)?;
    let last_poll = Arc::new(AtomicU64::new(api.now_ms() as u64));
    let tripped = Arc::new(AtomicBool::new(false));
    register_api(&lua, api.clone(), last_poll.clone())?;
    if let Some((_, ms)) = limits {
        install_watchdog(&lua, Watchdog { api: api.clone(), last_poll, tripped: tripped.clone(), ms })?;
    }
    // The prelude captures this into a local upvalue and removes the global,
    // so scripts can't reach it.
    let t = tripped.clone();
    lua.globals().set("__acid_tripped", lua.create_function(move |_, ()| Ok(t.load(Ordering::SeqCst)))?)?;
    harden_chunk_loading(&lua)?;
    for lib in CORE_LIBS {
        load_file(&lua, fs, &format!("{apps_dir}/{lib}"));
    }
    load_libs(&lua, fs, apps_dir, libs);
    if let Some((mem, _)) = limits {
        lua.set_memory_limit(mem)?;
    }
    Ok(lua)
}

/// Text chunks only. Lua 5.4 does not verify bytecode, so a crafted binary
/// chunk can corrupt host memory; scripts must not be able to load one,
/// whether via string.dump + load or a file that starts with "\x1bLua".
/// (load_file forces text mode for the same reason.)
struct Watchdog {
    api: Arc<dyn AcidApi>,
    last_poll: Arc<AtomicU64>,
    tripped: Arc<AtomicBool>,
    ms: u64,
}

thread_local! {
    // The VM runs on its app's own thread, so its watchdog lives here for
    // the raw C hook below to find.
    static WATCHDOG: RefCell<Option<Watchdog>> = const { RefCell::new(None) };
}

/// A raw count hook, not mlua's: mlua 0.10 keeps a single hook thread and
/// strips the hook from every coroutine. Lua 5.4 copies the hook to each new
/// coroutine (lua_newthread), so setting it on the main state covers them all.
/// Raises the error only after every Rust value is dropped.
unsafe extern "C-unwind" fn watchdog_hook(l: *mut ffi::lua_State, _ar: *mut ffi::lua_Debug) {
    let trip = WATCHDOG.with(|w| match &*w.borrow() {
        Some(wd) => {
            let idle = (wd.api.now_ms() as u64).saturating_sub(wd.last_poll.load(Ordering::Relaxed));
            if idle > wd.ms {
                wd.tripped.store(true, Ordering::SeqCst);
                true
            } else {
                false
            }
        }
        None => false,
    });
    if trip {
        unsafe {
            ffi::lua_pushstring(l, c"acid: stopped responding".as_ptr());
            ffi::lua_error(l)
        }
    }
}

fn install_watchdog(lua: &Lua, wd: Watchdog) -> mlua::Result<()> {
    WATCHDOG.with(|w| *w.borrow_mut() = Some(wd));
    unsafe {
        lua.exec_raw::<()>((), |st| ffi::lua_sethook(st, Some(watchdog_hook), ffi::LUA_MASKCOUNT, 10_000))
    }
}

const WATCHDOG_MSG: &str = "acid: stopped responding";

fn harden_chunk_loading(lua: &Lua) -> mlua::Result<()> {
    // The wrapper keeps an absent 4th (env) argument absent: Lua treats an
    // explicit nil env differently from a missing one.
    lua.load(
        r#"
        string.dump = nil

        -- The watchdog trips a Rust flag; the wrappers below check that flag
        -- (never the error value, which a script can fake or replace) after
        -- every protected call and re-raise, so nothing can swallow it. They
        -- use only captured upvalues, never globals read at call time.
        local tripped = __acid_tripped
        __acid_tripped = nil
        local rtype, rerror, find = type, error, string.find
        local rpcall, rxpcall, rsetmt, rrawget = pcall, xpcall, setmetatable, rawget
        local resume, cclose, cwrap, rrep = coroutine.resume, coroutine.close, coroutine.wrap, string.rep
        local function after(...)
          if tripped() then rerror("acid: stopped responding", 0) end
          return ...
        end
        local function check(...)
          if tripped() then rerror("acid: stopped responding", 0) end
          local ok, e = ...
          if ok == false and rtype(e) == "string" and find(e, "not enough memory", 1, true) then
            rerror(e, 0)
          end
          return ...
        end
        pcall = function(...) return check(rpcall(...)) end
        -- A reader function runs under load's own protected parser, which
        -- turns the hook's error into `nil, msg`: re-raise once tripped, and
        -- make the reader end the chunk (return nil) once tripped so a
        -- spinning reader is not re-entered.
        local rload, rselect = load, select
        load = function(chunk, name, _, ...)
          if rtype(chunk) == "function" then
            local reader = chunk
            chunk = function()
              if tripped() then return nil end
              return reader()
            end
          end
          if rselect('#', ...) > 0 then return after(rload(chunk, name, "t", (...))) end
          return after(rload(chunk, name, "t"))
        end
        -- A hook-raised error runs its message handler with hooks off, so a
        -- looping handler would never be stopped: once tripped, the handler
        -- is skipped.
        xpcall = function(f, h, ...)
          if rtype(h) == "function" then
            local user = h
            h = function(e)
              if tripped() then return e end
              return user(e)
            end
          end
          return check(rxpcall(f, h, ...))
        end
        coroutine.resume = function(...) return check(resume(...)) end
        coroutine.close = function(...)
          if tripped() then rerror("acid: stopped responding", 0) end
          return check(cclose(...))
        end
        coroutine.wrap = function(f)
          local w = cwrap(f)
          return function(...) return after(w(...)) end
        end
        -- No finalizers: Lua turns hooks off while one runs, so a looping
        -- __gc could never be stopped.
        -- A thread the hook stopped keeps its hooks off while its pending
        -- __close handlers run (also from coroutine.wrap/close), so a looping
        -- __close would hang. Wrap each __close to do nothing once tripped.
        -- The metatable is mutated in place (not copied) so getmetatable(t)
        -- == mt still holds; `wrapped` stops a shared metatable being wrapped
        -- again on every setmetatable call. (A __close added to the table
        -- after setmetatable is not caught.)
        local wrapped = rsetmt({}, {__mode = "k"})
        setmetatable = function(t, mt)
          if rtype(mt) == "table" then
            if rrawget(mt, "__gc") ~= nil then
              rerror("__gc is not allowed in apps", 2)
            end
            local c = rrawget(mt, "__close")
            -- Any callable __close counts (a table with __call too).
            if c ~= nil and not wrapped[c] then
              local w = function(...)
                if tripped() then return end
                return c(...)
              end
              wrapped[w] = true
              mt.__close = w
            end
          end
          return rsetmt(t, mt)
        end
        -- table.move loops over its range inside C, with no allocation.
        -- insert/remove walk the length (which __len can fake) inside C.
        -- The length is coerced the way luaL_len does (a numeric string
        -- counts); a non-integer is left for the raw function to reject.
        local rinsert, rremove = table.insert, table.remove
        local tointeger, tonum = math.tointeger, tonumber
        local function bounded(raw)
          return function(t, ...)
            if rtype(t) == "table" then
              local n = tointeger(tonum(#t))
              if n and n > 16777216 then rerror("table too large", 2) end
            end
            return raw(t, ...)
          end
        end
        table.insert = bounded(rinsert)
        table.remove = bounded(rremove)
        local rmove = table.move
        table.move = function(a1, f, e, t, a2)
          local fi, ei = tointeger(tonum(f)), tointeger(tonum(e))
          if fi and ei and ei >= fi then
            local d = ei - fi
            if d < 0 or d >= 16777216 then rerror("table.move range too large", 2) end
          end
          return rmove(a1, f, e, t, a2)
        end
        -- Repeating an empty string loops n times inside one C call.
        string.rep = function(s, n, sep)
          if rtype(s) == "string" and #s == 0 and tonum(n) ~= nil
             and (sep == nil or (rtype(sep) == "string" and #sep == 0)) then
            return ""
          end
          return rrep(s, n, sep)
        end
        "#,
    )
    .set_mode(ChunkMode::Text)
    .exec()
}

/// Runs one file in `lua`. Failures are logged and reported as false; a
/// failing lib doesn't stop the next one or the app script.
pub fn load_file(lua: &Lua, fs: &dyn Fs, path: &str) -> bool {
    let src = match fs.read(path) {
        Ok(src) => src,
        Err(_) => {
            eprintln!("Acid OS v3: could not open {path}");
            return false;
        }
    };
    match lua.load(&src[..]).set_name(format!("@{path}")).set_mode(ChunkMode::Text).exec() {
        Ok(()) => true,
        Err(e) => {
            let text = e.to_string();
            if matches!(e, mlua::Error::MemoryError(_)) || text.contains("not enough memory") {
                eprintln!("Acid OS v3: {path}: out of memory");
            } else if text.contains(WATCHDOG_MSG) {
                eprintln!("Acid OS v3: {path}: stopped responding");
            } else {
                eprintln!("Acid OS v3: {path}: {e}");
            }
            false
        }
    }
}

fn load_libs(lua: &Lua, fs: &dyn Fs, apps_dir: &str, libs: Option<&str>) {
    let Some(libs) = libs else { return };
    for entry in libs.split(',').map(str::trim).filter(|e| !e.is_empty()) {
        if !lib_path_is_safe(entry) {
            eprintln!("Acid OS v3: rejected unsafe lib path {entry}");
            continue;
        }
        load_file(lua, fs, &format!("{apps_dir}/{entry}"));
    }
}

fn poll_event_values(lua: &Lua, ev: Option<PolledEvent>) -> mlua::Result<MultiValue> {
    match ev {
        None => Ok(MultiValue::new()),
        Some(PolledEvent::Close) => "close".into_lua_multi(lua),
        Some(PolledEvent::Moved) => "moved".into_lua_multi(lua),
        Some(PolledEvent::Resized { w, h }) => ("resized", w, h).into_lua_multi(lua),
        Some(PolledEvent::Key { code, pressed }) => ("key", code, pressed).into_lua_multi(lua),
        Some(PolledEvent::Touch { x, y, pressed }) => ("touch", x, y, pressed).into_lua_multi(lua),
    }
}

/// Optional string argument: nil and "" both mean "none".
fn opt_str(s: Option<mlua::String>) -> String {
    s.map(|s| s.to_string_lossy()).unwrap_or_default()
}

fn register_api(lua: &Lua, api: Arc<dyn AcidApi>, last_poll: Arc<AtomicU64>) -> mlua::Result<()> {
    let g = lua.globals();

    let a = api.clone();
    g.set("acid_poll_event", lua.create_function(move |lua, ms: i64| {
        let ev = a.poll_event(ms);
        last_poll.store(a.now_ms() as u64, Ordering::Relaxed);
        poll_event_values(lua, ev)
    })?)?;

    let a = api.clone();
    g.set("acid_notify_redraw_done", lua.create_function(move |_, ()| {
        a.notify_redraw_done();
        Ok(())
    })?)?;

    let a = api.clone();
    g.set("acid_fill_rect", lua.create_function(move |_, (x, y, w, h, c): (i32, i32, i32, i32, i64)| {
        a.fill_rect(x, y, w, h, c as u32);
        Ok(())
    })?)?;

    let a = api.clone();
    g.set("acid_draw_line", lua.create_function(move |_, (x1, y1, x2, y2, c): (i32, i32, i32, i32, i64)| {
        a.draw_line(x1, y1, x2, y2, c as u32);
        Ok(())
    })?)?;

    let a = api.clone();
    g.set("acid_fill_triangle", lua.create_function(move |_, (x1, y1, x2, y2, x3, y3, c): (i32, i32, i32, i32, i32, i32, i64)| {
        a.fill_triangle(x1, y1, x2, y2, x3, y3, c as u32);
        Ok(())
    })?)?;

    let a = api.clone();
    g.set("acid_mesh_builtin", lua.create_function(move |lua, name: mlua::String| {
        match a.mesh_builtin(&name.to_string_lossy()) {
            Ok(id) => id.into_lua_multi(lua),
            Err(_) => Value::Nil.into_lua_multi(lua),
        }
    })?)?;

    let a = api.clone();
    g.set("acid_mesh_new", lua.create_function(move |lua, (points, faces): (mlua::Table, mlua::Table)| {
        match parse_mesh(&points, &faces).and_then(|(p, f)| a.mesh_new(p, f)) {
            Ok(id) => id.into_lua_multi(lua),
            Err(e) => (Value::Nil, e).into_lua_multi(lua),
        }
    })?)?;

    let a = api.clone();
    g.set("acid_mesh_draw", lua.create_function(
        move |_, (id, x, y, size, rx, ry, rz, mode, c): (i32, i32, i32, i32, i32, i32, i32, i32, i64)| {
            a.mesh_draw(id, x, y, size, rx, ry, rz, mode, c as u32);
            Ok(())
        },
    )?)?;

    let a = api.clone();
    g.set("acid_mesh_free", lua.create_function(move |_, id: i32| {
        a.mesh_free(id);
        Ok(())
    })?)?;

    let a = api.clone();
    g.set("acid_fill_circle", lua.create_function(move |_, (x, y, r, c): (i32, i32, i32, i64)| {
        a.fill_circle(x, y, r, c as u32);
        Ok(())
    })?)?;

    let a = api.clone();
    g.set("acid_draw_text", lua.create_function(
        move |_, (text, x, y, fg, bg): (mlua::String, i32, i32, i64, i64)| {
            a.draw_text(&text.to_string_lossy(), x, y, fg as u32, bg as u32);
            Ok(())
        },
    )?)?;

    let a = api.clone();
    g.set("acid_draw_window_frame", lua.create_function(move |_, title: mlua::String| {
        a.draw_window_frame(&title.to_string_lossy());
        Ok(())
    })?)?;

    let a = api.clone();
    g.set("acid_draw_window_border", lua.create_function(move |_, ()| {
        a.draw_window_border();
        Ok(())
    })?)?;

    let a = api.clone();
    g.set("acid_clear_user_area", lua.create_function(move |_, ()| {
        a.clear_user_area();
        Ok(())
    })?)?;

    let a = api.clone();
    g.set("acid_am_i_focused", lua.create_function(move |_, ()| Ok(a.am_i_focused()))?)?;

    let a = api.clone();
    g.set("acid_overlay_open", lua.create_function(move |_, ()| Ok(a.overlay_open()))?)?;
    let a = api.clone();
    g.set("acid_overlay_clear", lua.create_function(move |_, ()| { a.overlay_clear(); Ok(()) })?)?;
    let a = api.clone();
    g.set("acid_overlay_fill_rect", lua.create_function(move |_, (x, y, w, h, c): (i32, i32, i32, i32, i64)| {
        a.overlay_fill_rect(x, y, w, h, c as u32);
        Ok(())
    })?)?;
    let a = api.clone();
    g.set("acid_overlay_close", lua.create_function(move |_, ()| { a.overlay_close(); Ok(()) })?)?;
    let a = api.clone();
    g.set("acid_repaint_region", lua.create_function(move |_, (x, y, w, h): (i32, i32, i32, i32)| {
        a.repaint_region(x, y, w, h);
        Ok(())
    })?)?;
    let a = api.clone();
    g.set("acid_set_wallpaper_enabled", lua.create_function(move |_, on: bool| { a.set_wallpaper_enabled(on); Ok(()) })?)?;
    let a = api.clone();
    g.set("acid_get_wallpaper_enabled", lua.create_function(move |_, ()| Ok(a.wallpaper_enabled()))?)?;
    let a = api.clone();
    g.set("acid_window_max", lua.create_function(move |_, ()| Ok(a.window_max()))?)?;
    let a = api.clone();
    g.set("acid_screen_size", lua.create_function(move |_, ()| Ok(a.screen_size()))?)?;
    let a = api.clone();
    g.set("acid_font_size", lua.create_function(move |_, ()| Ok(a.font_size()))?)?;
    let a = api.clone();
    g.set("acid_window_size", lua.create_function(move |_, ()| Ok(a.window_size()))?)?;
    let a = api.clone();
    g.set("acid_get_font_scale", lua.create_function(move |_, ()| Ok(a.font_scale()))?)?;
    let a = api.clone();
    g.set("acid_set_font_scale", lua.create_function(move |_, n: i32| { a.set_font_scale(n); Ok(()) })?)?;
    let a = api.clone();
    g.set("acid_window_info", lua.create_function(move |lua, i: i64| match a.window_info(i) {
        None => Ok(MultiValue::new()),
        Some(w) => (w.app_name, w.x, w.y, w.w, w.h, w.focused).into_lua_multi(lua),
    })?)?;
    let a = api.clone();
    g.set("acid_local_time", lua.create_function(move |lua, ()| {
        let t = a.local_time();
        (t.year, t.month, t.day, t.hour, t.min, t.sec).into_lua_multi(lua)
    })?)?;
    let a = api.clone();
    g.set("acid_mem_used_kb", lua.create_function(move |_, ()| Ok(a.mem_used_kb()))?)?;
    let a = api.clone();
    g.set("acid_network_info", lua.create_function(move |lua, ()| {
        let n = a.network_info();
        (n.host, n.ip, n.connected).into_lua_multi(lua)
    })?)?;
    let a = api.clone();
    g.set("acid_refresh_tasks", lua.create_function(move |_, ()| Ok(a.refresh_tasks()))?)?;
    let a = api.clone();
    g.set("acid_task_count", lua.create_function(move |_, ()| Ok(a.task_count()))?)?;
    let a = api.clone();
    g.set("acid_task_info", lua.create_function(move |lua, i: i64| match a.task_info(i) {
        None => Ok(MultiValue::new()),
        Some(t) => (t.name, t.state, t.cpu_percent).into_lua_multi(lua),
    })?)?;
    let a = api.clone();
    g.set("acid_fs_list", lua.create_function(move |lua, dir: mlua::String| {
        match a.fs_list(&dir.to_string_lossy()) {
            Ok(names) => lua.create_sequence_from(names)?.into_lua_multi(lua),
            Err(e) => (Value::Nil, e).into_lua_multi(lua),
        }
    })?)?;
    let a = api.clone();
    g.set("acid_fs_read", lua.create_function(move |lua, path: mlua::String| {
        match a.fs_read(&path.to_string_lossy()) {
            Ok(bytes) => lua.create_string(&bytes)?.into_lua_multi(lua),
            Err(e) => (Value::Nil, e).into_lua_multi(lua),
        }
    })?)?;
    let a = api.clone();
    g.set("acid_fs_size", lua.create_function(move |lua, path: mlua::String| {
        match a.fs_size(&path.to_string_lossy()) {
            Ok(n) => (n as i64).into_lua_multi(lua),
            Err(e) => (Value::Nil, e).into_lua_multi(lua),
        }
    })?)?;
    let a = api.clone();
    g.set("acid_cart_roots", lua.create_function(move |lua, ()| {
        match a.cart_roots() {
            Ok(names) => lua.create_sequence_from(names)?.into_lua_multi(lua),
            Err(e) => (Value::Nil, e).into_lua_multi(lua),
        }
    })?)?;
    let a = api.clone();
    g.set("acid_cart_list", lua.create_function(move |lua, dir: mlua::String| {
        match a.cart_list(&dir.to_string_lossy()) {
            Ok(names) => lua.create_sequence_from(names)?.into_lua_multi(lua),
            Err(e) => (Value::Nil, e).into_lua_multi(lua),
        }
    })?)?;
    let a = api.clone();
    g.set("acid_cart_stat", lua.create_function(move |lua, path: mlua::String| {
        match a.cart_stat(&path.to_string_lossy()) {
            Ok((is_dir, size)) => (if is_dir { "dir" } else { "file" }, size as i64).into_lua_multi(lua),
            Err(e) => (Value::Nil, e).into_lua_multi(lua),
        }
    })?)?;
    let a = api.clone();
    g.set("acid_cart_read", lua.create_function(move |lua, path: mlua::String| {
        match a.cart_read(&path.to_string_lossy()) {
            Ok(bytes) => lua.create_string(&bytes)?.into_lua_multi(lua),
            Err(e) => (Value::Nil, e).into_lua_multi(lua),
        }
    })?)?;
    let a = api.clone();
    g.set("acid_fs_write", lua.create_function(move |lua, (path, data): (mlua::String, mlua::String)| {
        match a.fs_write(&path.to_string_lossy(), &data.as_bytes()) {
            Ok(()) => true.into_lua_multi(lua),
            Err(e) => (Value::Nil, e).into_lua_multi(lua),
        }
    })?)?;
    let a = api.clone();
    g.set("acid_fs_rename", lua.create_function(move |lua, (from, to): (mlua::String, mlua::String)| {
        match a.fs_rename(&from.to_string_lossy(), &to.to_string_lossy()) {
            Ok(()) => true.into_lua_multi(lua),
            Err(e) => (Value::Nil, e).into_lua_multi(lua),
        }
    })?)?;
    let a = api.clone();
    g.set("acid_fs_delete", lua.create_function(move |lua, path: mlua::String| {
        match a.fs_delete(&path.to_string_lossy()) {
            Ok(()) => true.into_lua_multi(lua),
            Err(e) => (Value::Nil, e).into_lua_multi(lua),
        }
    })?)?;
    let a = api.clone();
    g.set("acid_activate_window", lua.create_function(move |_, i: i64| { a.activate_window(i); Ok(()) })?)?;
    let a = api.clone();
    g.set("acid_close_window", lua.create_function(move |_, i: i64| Ok(a.close_window(i)))?)?;
    let a = api.clone();
    g.set("acid_send_self_to_back", lua.create_function(move |_, ()| { a.send_self_to_back(); Ok(()) })?)?;
    let a = api.clone();
    g.set("acid_launcher_register", lua.create_function(
        move |_, (path, name, w, h, multi, libs): (mlua::String, mlua::String, i32, i32, bool, Option<mlua::String>)| {
            Ok(a.launcher_register(&path.to_string_lossy(), &name.to_string_lossy(), w, h, multi, &opt_str(libs)))
        },
    )?)?;
    let a = api.clone();
    g.set("acid_launcher_count", lua.create_function(move |_, ()| Ok(a.launcher_count()))?)?;
    let a = api.clone();
    g.set("acid_launcher_path", lua.create_function(move |_, i: i64| Ok(a.launcher_path(i)))?)?;
    let a = api.clone();
    g.set("acid_launcher_name", lua.create_function(move |_, i: i64| Ok(a.launcher_name(i)))?)?;
    let a = api.clone();
    g.set("acid_launcher_spawn", lua.create_function(move |_, i: i64| Ok(a.launcher_spawn(i)))?)?;
    let a = api.clone();
    g.set("acid_spawn_app", lua.create_function(
        move |_, (path, w, h, arg): (mlua::String, i32, i32, Option<mlua::String>)| {
            Ok(a.spawn_app(&path.to_string_lossy(), w, h, &opt_str(arg)))
        },
    )?)?;
    let a = api.clone();
    g.set("acid_composited_frames", lua.create_function(move |_, ()| Ok(a.composited_frames()))?)?;
    let a = api.clone();
    g.set("acid_skipped_frames", lua.create_function(move |_, ()| Ok(a.skipped_frames()))?)?;

    let a = api.clone();
    g.set("acid_play_note", lua.create_function(move |_, (v, o, vol): (i32, i32, i32)| { a.play_note(v, o, vol); Ok(()) })?)?;
    let a = api.clone();
    g.set("acid_stop_note", lua.create_function(move |_, v: i32| { a.stop_note(v); Ok(()) })?)?;
    let a = api.clone();
    g.set("acid_configure_voice", lua.create_function(
        move |_, (v, r, at, d, s, rel): (i32, i32, i32, i32, i32, i32)| { a.configure_voice(v, r, at, d, s, rel); Ok(()) },
    )?)?;
    let a = api.clone();
    g.set("acid_configure_filter", lua.create_function(move |_, (c, r, m): (i32, i32, i32)| { a.configure_filter(c, r, m); Ok(()) })?)?;
    let a = api.clone();
    g.set("acid_trigger_arp", lua.create_function(
        move |_, (v, n0, n1, n2, n3, c, ms): (i32, i32, i32, i32, i32, i32, i32)| { a.trigger_arp(v, [n0, n1, n2, n3], c, ms); Ok(()) },
    )?)?;
    let a = api.clone();
    g.set("acid_configure_osc", lua.create_function(move |_, (v, w, d): (i32, i32, i32)| { a.configure_osc(v, w, d); Ok(()) })?)?;
    let a = api.clone();
    g.set("acid_set_ring_partner", lua.create_function(move |_, (v, p): (i32, i32)| { a.set_ring_partner(v, p); Ok(()) })?)?;
    let a = api.clone();
    g.set("acid_set_volume", lua.create_function(move |_, p: i32| { a.set_volume(p); Ok(()) })?)?;
    let a = api.clone();
    g.set("acid_get_volume", lua.create_function(move |_, ()| Ok(a.volume()))?)?;
    let a = api.clone();
    g.set("acid_active_voice_count", lua.create_function(move |_, ()| Ok(a.active_voice_count()))?)?;

    let a = api.clone();
    g.set("acid_now_ms", lua.create_function(move |_, ()| Ok(a.now_ms()))?)?;

    let a = api;
    g.set("acid_launch_arg", lua.create_function(move |_, ()| Ok(a.launch_arg()))?)?;

    Ok(())
}
