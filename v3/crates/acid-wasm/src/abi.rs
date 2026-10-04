//! The `"acid"` imports a cart may call: the whole ABI v1 table (spec §15.3;
//! the binding table is in the Phase 6 plan's global constraints). Also the
//! helpers that move strings and bytes across the linear-memory boundary,
//! and the fuel charges that make host work inside an import count against
//! the cart's per-callback budget (§15.2).

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Write as _;
use core::ops::Range;

use acid_kernel::layout::TITLE_BAR_H;
use wasmi::errors::LinkerError;
use wasmi::{Caller, Error, Extern, Linker, Memory, TrapCode};

use acid_api::{MESH_FACES_MAX, MESH_POINTS_MAX, NO_INDEX};

use crate::runner::Host;

/// The module every import lives in (§15.3).
const MODULE: &str = "acid";

/// Maps an `AcidApi` error string to the ABI's negative code (§15.3).
pub fn err_code(e: &str) -> i32 {
    match e {
        "not found" => -1,
        "bad path" => -2,
        "read only" => -3,
        "not allowed" => -4,
        "too big" => -5,
        _ => -6,
    }
}

/// The cart's linear memory, found through its required `memory` export.
fn memory(caller: &Caller<'_, Host>) -> Result<Memory, Error> {
    caller.get_export("memory").and_then(Extern::into_memory).ok_or_else(|| Error::new("acid: cart exports no memory"))
}

/// `(ptr, len)` as a byte range inside a memory of `size` bytes. Wasm
/// pointers are unsigned, so the i32s are reinterpreted, not sign-extended.
fn range(ptr: i32, len: i32, size: usize) -> Option<Range<usize>> {
    let start = ptr as u32 as usize;
    let end = start.checked_add(len as u32 as usize)?;
    (end <= size).then_some(start..end)
}

/// Host-side work done for a cart is paid for in fuel, at 1 unit per 8 bytes
/// (rounded up). Without this, one import call costs the same flat fuel
/// whether it moves 1 byte or 16 MB, so a cart could keep the host busy far
/// longer than its per-callback budget allows (§15.2).
const BYTES_PER_FUEL: u64 = 8;

/// Fuel charged for moving `bytes` across the boundary.
fn fuel_cost(bytes: u64) -> u64 {
    bytes.div_ceil(BYTES_PER_FUEL)
}

/// Pixels are charged as bytes moved: 2 per RGB565 pixel, so a full-screen
/// fill costs w × h × 2 / 8 fuel (76,800 at 640×480). A draw call moves no cart
/// bytes, but the host may fill the whole window for it; without this a
/// cart could make millions of full-window fills in one callback (§15.2).
const BYTES_PER_PX: u64 = 2;

/// One glyph cell of the system font (acid-gfx's 6 × 8 font).
const GLYPH_PX: u64 = 6 * 8;

/// The flat charge for each file-system or spawn call, on top of the bytes
/// it moves: a syscall's host work (a disk round trip, a new task) does not
/// show in its byte count. 1 MiB-equivalent is 131,072 fuel, so about 1,500
/// such calls fit in one callback, far more than a real cart makes (§15.2).
pub(crate) const FS_CALL_BYTES: u64 = 1 << 20;

/// The screen as `(w, h)`: no draw does more work than covering it.
type ScreenWh = (i32, i32);

fn screen(c: &Caller<'_, Host>) -> ScreenWh {
    c.data().api.screen_size()
}

/// Every pixel on the screen.
fn screen_px(s: ScreenWh) -> u64 {
    s.0 as u64 * s.1 as u64
}

/// Bytes charged for a `w × h` fill, each side clamped to the screen (the
/// host clips to it), so hostile sizes neither overflow nor overcharge.
fn rect_bytes(s: ScreenWh, w: i32, h: i32) -> u64 {
    let w = w.clamp(0, s.0) as u64;
    let h = h.clamp(0, s.1) as u64;
    w * h * BYTES_PER_PX
}

/// Bytes charged for a line: its longer axis in pixels, at most the screen.
fn line_bytes(s: ScreenWh, x1: i32, y1: i32, x2: i32, y2: i32) -> u64 {
    let dx = (i64::from(x2) - i64::from(x1)).unsigned_abs();
    let dy = (i64::from(y2) - i64::from(y1)).unsigned_abs();
    (dx.max(dy) + 1).min(screen_px(s)) * BYTES_PER_PX
}

/// Bytes charged for a triangle: its bounding box clamped to the screen,
/// like `rect_bytes`.
fn triangle_bytes(s: ScreenWh, p: [(i32, i32); 3]) -> u64 {
    let (minx, maxx) = p.iter().fold((i64::MAX, i64::MIN), |a, q| (a.0.min(i64::from(q.0)), a.1.max(i64::from(q.0))));
    let (miny, maxy) = p.iter().fold((i64::MAX, i64::MIN), |a, q| (a.0.min(i64::from(q.1)), a.1.max(i64::from(q.1))));
    let w = (maxx - minx + 1).min(i64::from(i32::MAX)) as i32;
    let h = (maxy - miny + 1).min(i64::from(i32::MAX)) as i32;
    rect_bytes(s, w, h)
}

/// Bytes charged for a mesh draw that touches `px` pixels: at most 64
/// screens' worth, since overdraw of a solid mesh can exceed the screen.
fn mesh_bytes(s: ScreenWh, px: u64) -> u64 {
    px.min(screen_px(s) * 64).saturating_mul(BYTES_PER_PX)
}

/// Reads `n` little-endian i32s at `ptr`; traps out of bounds, charges the bytes.
fn read_i32s(caller: &mut Caller<'_, Host>, ptr: i32, n: usize) -> Result<Vec<i32>, Error> {
    let len = i32::try_from(n.checked_mul(4).ok_or(Error::from(TrapCode::MemoryOutOfBounds))?).map_err(|_| Error::from(TrapCode::MemoryOutOfBounds))?;
    let b = read_bytes(caller, ptr, len)?;
    Ok(b.chunks_exact(4).map(|c| i32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect())
}

/// Bytes charged for a circle of radius `r`: its bounding square, at most the
/// screen. A negative radius draws nothing and costs nothing.
fn circle_bytes(s: ScreenWh, r: i32) -> u64 {
    if r < 0 {
        return 0;
    }
    let d = (r as u64) * 2 + 1;
    d.saturating_mul(d).min(screen_px(s)) * BYTES_PER_PX
}

/// Bytes charged for drawing a `len`-byte string: one glyph cell per byte (a
/// byte count never undercounts the glyphs), at most the screen. A glyph at
/// `scale` covers `scale²` times the pixels.
fn text_bytes(s: ScreenWh, len: i32, scale: i32) -> u64 {
    let glyph = GLYPH_PX * (scale * scale) as u64;
    (len as u32 as u64).saturating_mul(glyph).min(screen_px(s)) * BYTES_PER_PX
}

/// Charges the fuel for `bytes` of host work (§15.2). If the callback's
/// budget can't cover it, the budget is emptied and the call traps out of
/// fuel, so the cart ends as "stopped responding" like any other overrun.
/// Every import that copies or scans cart memory goes through here.
pub(crate) fn charge(caller: &mut Caller<'_, Host>, bytes: u64) -> Result<(), Error> {
    let cost = fuel_cost(bytes);
    let left = caller.get_fuel()?;
    if left < cost {
        caller.set_fuel(0)?;
        return Err(Error::from(TrapCode::OutOfFuel));
    }
    caller.set_fuel(left - cost)
}

/// The bytes at `(ptr, len)`; an out-of-bounds range traps (§15.3). The
/// bounds are checked before charging, so a bad range is a trap, not an
/// out-of-fuel; then the bytes are paid for, since the caller scans them.
pub(crate) fn read_bytes<'a>(caller: &'a mut Caller<'_, Host>, ptr: i32, len: i32) -> Result<&'a [u8], Error> {
    let mem = memory(caller)?;
    let r = range(ptr, len, mem.data_size(&*caller)).ok_or(Error::from(TrapCode::MemoryOutOfBounds))?;
    charge(caller, r.len() as u64)?;
    Ok(&mem.data(&*caller)[r])
}

/// The string at `(ptr, len)`; out of bounds or non-UTF-8 traps (§15.3).
pub(crate) fn read_str<'a>(caller: &'a mut Caller<'_, Host>, ptr: i32, len: i32) -> Result<&'a str, Error> {
    core::str::from_utf8(read_bytes(caller, ptr, len)?).map_err(|_| Error::new("acid: string argument is not UTF-8"))
}

/// Strings out (§15.3): writes `min(len, cap)` bytes at `buf` and returns the
/// full length, so a cart can retry with a bigger buffer. The whole
/// `(buf, cap)` range must be in bounds, or the call traps, even when the
/// result would fit in less: a cart's bad buffer is caught on every call.
/// The whole result is charged, not just the `min(len, cap)` copied: the host
/// has already produced all of it (read a file, joined a listing), so a cart
/// must not get that work free by passing `cap` 0 (§15.2).
pub(crate) fn write_out(caller: &mut Caller<'_, Host>, buf: i32, cap: i32, bytes: &[u8]) -> Result<i32, Error> {
    let mem = memory(caller)?;
    let r = range(buf, cap, mem.data_size(&*caller)).ok_or(Error::from(TrapCode::MemoryOutOfBounds))?;
    let n = bytes.len().min(r.len());
    charge(caller, bytes.len() as u64)?;
    mem.data_mut(&mut *caller)[r.start..r.start + n].copy_from_slice(&bytes[..n]);
    Ok(i32::try_from(bytes.len()).unwrap_or(i32::MAX))
}

/// A record or listing out (§15.3); `None` is "no such thing", -1.
fn out_opt(c: &mut Caller<'_, Host>, buf: i32, cap: i32, s: Option<String>) -> Result<i32, Error> {
    match s {
        Some(s) => write_out(c, buf, cap, s.as_bytes()),
        None => Ok(-1),
    }
}

/// The bytes of an `AcidApi` result, or its error as a negative code.
fn out_result(c: &mut Caller<'_, Host>, buf: i32, cap: i32, r: Result<Vec<u8>, String>) -> Result<i32, Error> {
    match r {
        Ok(b) => write_out(c, buf, cap, &b),
        Err(e) => Ok(err_code(&e)),
    }
}

/// Names joined by `\n` (§15.3), as a result for `out_result`.
fn joined(r: Result<Vec<String>, String>) -> Result<Vec<u8>, String> {
    r.map(|names| names.join("\n").into_bytes())
}

/// 0 on success, else the error's code.
fn status(r: Result<(), String>) -> i32 {
    r.map_or_else(|e| err_code(&e), |()| 0)
}

/// Registers every import in the ABI v1 table. A module importing anything else fails to
/// instantiate, and the runner refuses it.
pub(crate) fn link(linker: &mut Linker<Host>) -> Result<(), LinkerError> {
    linker.func_wrap(MODULE, "now_ms", |c: Caller<'_, Host>| c.data().api.now_ms())?;
    linker.func_wrap(MODULE, "notify_redraw_done", |c: Caller<'_, Host>| c.data().api.notify_redraw_done())?;
    // Colours are passed as i32 and handed on as u32, as acid-lua does.
    // Draws are charged for the pixels they can touch, before drawing (§15.2).
    linker.func_wrap(MODULE, "fill_rect", |mut c: Caller<'_, Host>, x: i32, y: i32, w: i32, h: i32, color: i32| -> Result<(), Error> {
        let s = screen(&c);
        charge(&mut c, rect_bytes(s, w, h))?;
        c.data().api.fill_rect(x, y, w, h, color as u32);
        Ok(())
    })?;
    linker.func_wrap(MODULE, "fill_circle", |mut c: Caller<'_, Host>, x: i32, y: i32, r: i32, color: i32| -> Result<(), Error> {
        let s = screen(&c);
        charge(&mut c, circle_bytes(s, r))?;
        c.data().api.fill_circle(x, y, r, color as u32);
        Ok(())
    })?;
    linker.func_wrap(
        MODULE,
        "draw_text",
        |mut c: Caller<'_, Host>, ptr: i32, len: i32, x: i32, y: i32, fg: i32, bg: i32| -> Result<(), Error> {
            // The api is cloned first: the string borrows `c` mutably (it
            // charges fuel) for as long as it is in use. The glyphs are
            // charged on top of the string's bytes.
            let api = c.data().api.clone();
            let s = screen(&c);
            let scale = api.font_size().0 / 6;
            charge(&mut c, text_bytes(s, len, scale))?;
            let text = read_str(&mut c, ptr, len)?;
            api.draw_text(text, x, y, fg as u32, bg as u32);
            Ok(())
        },
    )?;
    linker.func_wrap(MODULE, "draw_window_frame", |mut c: Caller<'_, Host>, ptr: i32, len: i32| -> Result<(), Error> {
        // The title bar spans at most the screen's width; its text is clipped
        // to the bar, so it is covered by the same area.
        let api = c.data().api.clone();
        let s = screen(&c);
        charge(&mut c, rect_bytes(s, s.0, TITLE_BAR_H))?;
        let title = read_str(&mut c, ptr, len)?;
        api.draw_window_frame(title);
        Ok(())
    })?;
    // The border is the window's perimeter, at most the screen's.
    linker.func_wrap(MODULE, "draw_window_border", |mut c: Caller<'_, Host>| -> Result<(), Error> {
        let s = screen(&c);
        charge(&mut c, 2 * (s.0 as u64 + s.1 as u64) * BYTES_PER_PX)?;
        c.data().api.draw_window_border();
        Ok(())
    })?;
    // The user area is at most the whole screen.
    linker.func_wrap(MODULE, "clear_user_area", |mut c: Caller<'_, Host>| -> Result<(), Error> {
        let s = screen(&c);
        charge(&mut c, rect_bytes(s, s.0, s.1))?;
        c.data().api.clear_user_area();
        Ok(())
    })?;
    linker.func_wrap(MODULE, "am_i_focused", |c: Caller<'_, Host>| i32::from(c.data().api.am_i_focused()))?;
    linker.func_wrap(MODULE, "launch_arg", |mut c: Caller<'_, Host>, buf: i32, cap: i32| -> Result<i32, Error> {
        let arg = c.data().api.launch_arg();
        write_out(&mut c, buf, cap, arg.as_bytes())
    })?;
    // Audio (§15.3): thin calls, no cart memory involved.
    linker.func_wrap(MODULE, "play_note", |c: Caller<'_, Host>, voice: i32, ona: i32, volume: i32| c.data().api.play_note(voice, ona, volume))?;
    linker.func_wrap(MODULE, "stop_note", |c: Caller<'_, Host>, voice: i32| c.data().api.stop_note(voice))?;
    linker.func_wrap(
        MODULE,
        "configure_voice",
        |c: Caller<'_, Host>, voice: i32, route: i32, attack: i32, decay: i32, sustain: i32, release: i32| {
            c.data().api.configure_voice(voice, route, attack, decay, sustain, release)
        },
    )?;
    linker.func_wrap(MODULE, "configure_filter", |c: Caller<'_, Host>, cutoff: i32, resonance: i32, mode: i32| {
        c.data().api.configure_filter(cutoff, resonance, mode)
    })?;
    // The four notes are separate arguments: a note array would need a
    // pointer, and these are the same four i32s the Lua call takes.
    linker.func_wrap(
        MODULE,
        "trigger_arp",
        |c: Caller<'_, Host>, voice: i32, n0: i32, n1: i32, n2: i32, n3: i32, count: i32, rate_ms: i32| {
            c.data().api.trigger_arp(voice, [n0, n1, n2, n3], count, rate_ms)
        },
    )?;
    linker.func_wrap(MODULE, "configure_osc", |c: Caller<'_, Host>, voice: i32, waveform: i32, duty: i32| {
        c.data().api.configure_osc(voice, waveform, duty)
    })?;
    linker.func_wrap(MODULE, "set_ring_partner", |c: Caller<'_, Host>, voice: i32, partner: i32| c.data().api.set_ring_partner(voice, partner))?;
    linker.func_wrap(MODULE, "set_volume", |c: Caller<'_, Host>, percent: i32| c.data().api.set_volume(percent))?;
    linker.func_wrap(MODULE, "get_volume", |c: Caller<'_, Host>| c.data().api.volume())?;
    linker.func_wrap(MODULE, "active_voice_count", |c: Caller<'_, Host>| c.data().api.active_voice_count())?;

    // Overlay and wallpaper.
    linker.func_wrap(MODULE, "overlay_open", |c: Caller<'_, Host>| i32::from(c.data().api.overlay_open()))?;
    // The overlay is screen-sized: clearing it fills every pixel.
    linker.func_wrap(MODULE, "overlay_clear", |mut c: Caller<'_, Host>| -> Result<(), Error> {
        let s = screen(&c);
        charge(&mut c, rect_bytes(s, s.0, s.1))?;
        c.data().api.overlay_clear();
        Ok(())
    })?;
    linker.func_wrap(MODULE, "overlay_fill_rect", |mut c: Caller<'_, Host>, x: i32, y: i32, w: i32, h: i32, color: i32| -> Result<(), Error> {
        let s = screen(&c);
        charge(&mut c, rect_bytes(s, w, h))?;
        c.data().api.overlay_fill_rect(x, y, w, h, color as u32);
        Ok(())
    })?;
    linker.func_wrap(MODULE, "overlay_close", |c: Caller<'_, Host>| c.data().api.overlay_close())?;
    linker.func_wrap(MODULE, "repaint_region", |mut c: Caller<'_, Host>, x: i32, y: i32, w: i32, h: i32| -> Result<(), Error> {
        let s = screen(&c);
        charge(&mut c, rect_bytes(s, w, h))?;
        c.data().api.repaint_region(x, y, w, h);
        Ok(())
    })?;
    linker.func_wrap(MODULE, "set_wallpaper_enabled", |c: Caller<'_, Host>, on: i32| c.data().api.set_wallpaper_enabled(on != 0))?;
    linker.func_wrap(MODULE, "get_wallpaper_enabled", |c: Caller<'_, Host>| i32::from(c.data().api.wallpaper_enabled()))?;

    // Windows: a record is `name\tx\ty\tw\th\tfocused` (§15.3).
    linker.func_wrap(MODULE, "window_max", |c: Caller<'_, Host>| c.data().api.window_max())?;
    linker.func_wrap(MODULE, "screen_w", |c: Caller<'_, Host>| c.data().api.screen_size().0)?;
    linker.func_wrap(MODULE, "screen_h", |c: Caller<'_, Host>| c.data().api.screen_size().1)?;
    linker.func_wrap(MODULE, "font_w", |c: Caller<'_, Host>| c.data().api.font_size().0)?;
    linker.func_wrap(MODULE, "font_h", |c: Caller<'_, Host>| c.data().api.font_size().1)?;
    linker.func_wrap(MODULE, "window_w", |c: Caller<'_, Host>| c.data().api.window_size().0)?;
    linker.func_wrap(MODULE, "window_h", |c: Caller<'_, Host>| c.data().api.window_size().1)?;
    linker.func_wrap(MODULE, "get_font_scale", |c: Caller<'_, Host>| c.data().api.font_scale())?;
    linker.func_wrap(MODULE, "set_font_scale", |c: Caller<'_, Host>, n: i32| c.data().api.set_font_scale(n))?;
    linker.func_wrap(MODULE, "draw_line", |mut c: Caller<'_, Host>, x1: i32, y1: i32, x2: i32, y2: i32, color: i32| -> Result<(), Error> {
        let s = screen(&c);
        charge(&mut c, line_bytes(s, x1, y1, x2, y2))?;
        c.data().api.draw_line(x1, y1, x2, y2, color as u32);
        Ok(())
    })?;
    linker.func_wrap(
        MODULE,
        "fill_triangle",
        |mut c: Caller<'_, Host>, x1: i32, y1: i32, x2: i32, y2: i32, x3: i32, y3: i32, color: i32| -> Result<(), Error> {
            let s = screen(&c);
            charge(&mut c, triangle_bytes(s, [(x1, y1), (x2, y2), (x3, y3)]))?;
            c.data().api.fill_triangle(x1, y1, x2, y2, x3, y3, color as u32);
            Ok(())
        },
    )?;
    // Meshes: -1 unknown built-in, -2 bad mesh, -5 too big (§15.3).
    linker.func_wrap(MODULE, "mesh_builtin", |mut c: Caller<'_, Host>, p: i32, l: i32| -> Result<i32, Error> {
        let api = c.data().api.clone();
        let name = read_str(&mut c, p, l)?;
        Ok(match api.mesh_builtin(name) {
            Ok(id) => id,
            Err(e) if e == "too big" => -5,
            Err(_) => -1,
        })
    })?;
    linker.func_wrap(MODULE, "mesh_new", |mut c: Caller<'_, Host>, pp: i32, np: i32, fp: i32, nf: i32| -> Result<i32, Error> {
        // The layout and bounds are checked before any memory is read, so a
        // hostile count can neither allocate nor charge past the caps.
        if np < 3 || nf < 0 {
            return Ok(-2);
        }
        if np as usize > MESH_POINTS_MAX || nf as usize > MESH_FACES_MAX {
            return Ok(-5);
        }
        let api = c.data().api.clone();
        let pts: Vec<(i32, i32, i32)> = read_i32s(&mut c, pp, np as usize * 3)?.chunks_exact(3).map(|t| (t[0], t[1], t[2])).collect();
        let raw = read_i32s(&mut c, fp, nf as usize * 4)?;
        let mut faces = Vec::with_capacity(nf as usize);
        for f in raw.chunks_exact(4) {
            let idx = |v: i32, tri_ok: bool| -> Option<u16> {
                if v == -1 && tri_ok {
                    Some(NO_INDEX)
                } else if (0..np).contains(&v) {
                    Some(v as u16)
                } else {
                    None
                }
            };
            match (idx(f[0], false), idx(f[1], false), idx(f[2], false), idx(f[3], true)) {
                (Some(a), Some(b), Some(d), Some(e)) => faces.push([a, b, d, e]),
                _ => return Ok(-2),
            }
        }
        Ok(match api.mesh_new(pts, faces) {
            Ok(id) => id,
            Err(e) if e == "too big" => -5,
            Err(_) => -2,
        })
    })?;
    linker.func_wrap(
        MODULE,
        "mesh_draw",
        |mut c: Caller<'_, Host>, id: i32, x: i32, y: i32, size: i32, rx: i32, ry: i32, rz: i32, mode: i32, color: i32| -> Result<(), Error> {
            let s = screen(&c);
            let px = c.data().api.mesh_draw_cost(id, x, y, size, rx, ry, rz, mode);
            charge(&mut c, mesh_bytes(s, px))?;
            c.data().api.mesh_draw(id, x, y, size, rx, ry, rz, mode, color as u32);
            Ok(())
        },
    )?;
    linker.func_wrap(MODULE, "mesh_free", |c: Caller<'_, Host>, id: i32| c.data().api.mesh_free(id))?;
    linker.func_wrap(MODULE, "window_info", |mut c: Caller<'_, Host>, index: i32, buf: i32, cap: i32| -> Result<i32, Error> {
        let rec = c.data().api.window_info(i64::from(index)).map(|w| {
            let mut s = String::new();
            let _ = write!(s, "{}\t{}\t{}\t{}\t{}\t{}", w.app_name, w.x, w.y, w.w, w.h, u8::from(w.focused));
            s
        });
        out_opt(&mut c, buf, cap, rec)
    })?;
    linker.func_wrap(MODULE, "activate_window", |c: Caller<'_, Host>, index: i32| c.data().api.activate_window(i64::from(index)))?;
    // -4 for a cart (spec §16.2); else 1 closed / 0 nothing closed.
    linker.func_wrap(MODULE, "close_window", |c: Caller<'_, Host>, index: i32| {
        let api = &c.data().api;
        if api.is_cart() { -4 } else { i32::from(api.close_window(i64::from(index))) }
    })?;
    linker.func_wrap(MODULE, "send_self_to_back", |c: Caller<'_, Host>| c.data().api.send_self_to_back())?;

    // Launcher: a WASM cart is cart-level, so the api answers register with
    // false (§14.2); the call is still bound so the table mirrors Lua's.
    linker.func_wrap(
        MODULE,
        "launcher_register",
        |mut c: Caller<'_, Host>, pp: i32, pl: i32, np: i32, nl: i32, w: i32, h: i32, multi: i32, lp: i32, ll: i32| -> Result<i32, Error> {
            let api = c.data().api.clone();
            // Each string is copied out before the next read borrows `c`.
            let path = String::from(read_str(&mut c, pp, pl)?);
            let name = String::from(read_str(&mut c, np, nl)?);
            let libs = read_str(&mut c, lp, ll)?;
            Ok(i32::from(api.launcher_register(&path, &name, w, h, multi != 0, libs)))
        },
    )?;
    linker.func_wrap(MODULE, "launcher_count", |c: Caller<'_, Host>| c.data().api.launcher_count())?;
    linker.func_wrap(MODULE, "launcher_path", |mut c: Caller<'_, Host>, index: i32, buf: i32, cap: i32| -> Result<i32, Error> {
        let s = c.data().api.launcher_path(i64::from(index));
        out_opt(&mut c, buf, cap, s)
    })?;
    linker.func_wrap(MODULE, "launcher_name", |mut c: Caller<'_, Host>, index: i32, buf: i32, cap: i32| -> Result<i32, Error> {
        let s = c.data().api.launcher_name(i64::from(index));
        out_opt(&mut c, buf, cap, s)
    })?;
    // Spawns start a task: a flat charge on top of any bytes (§15.2).
    linker.func_wrap(MODULE, "launcher_spawn", |mut c: Caller<'_, Host>, index: i32| -> Result<i32, Error> {
        charge(&mut c, FS_CALL_BYTES)?;
        Ok(i32::from(c.data().api.launcher_spawn(i64::from(index))))
    })?;
    linker.func_wrap(
        MODULE,
        "spawn_app",
        |mut c: Caller<'_, Host>, pp: i32, pl: i32, w: i32, h: i32, ap: i32, al: i32| -> Result<i32, Error> {
            charge(&mut c, FS_CALL_BYTES)?;
            let api = c.data().api.clone();
            let path = String::from(read_str(&mut c, pp, pl)?);
            let arg = read_str(&mut c, ap, al)?;
            Ok(i32::from(api.spawn_app(&path, w, h, arg)))
        },
    )?;
    // Frame counters saturate at i32::MAX rather than wrap negative.
    linker.func_wrap(MODULE, "composited_frames", |c: Caller<'_, Host>| i32::try_from(c.data().api.composited_frames()).unwrap_or(i32::MAX))?;
    linker.func_wrap(MODULE, "skipped_frames", |c: Caller<'_, Host>| i32::try_from(c.data().api.skipped_frames()).unwrap_or(i32::MAX))?;

    // System records, tab-joined (§15.3).
    linker.func_wrap(MODULE, "local_time", |mut c: Caller<'_, Host>, buf: i32, cap: i32| -> Result<i32, Error> {
        let t = c.data().api.local_time();
        let mut s = String::new();
        let _ = write!(s, "{}\t{}\t{}\t{}\t{}\t{}", t.year, t.month, t.day, t.hour, t.min, t.sec);
        write_out(&mut c, buf, cap, s.as_bytes())
    })?;
    linker.func_wrap(MODULE, "mem_used_kb", |c: Caller<'_, Host>| c.data().api.mem_used_kb())?;
    linker.func_wrap(MODULE, "network_info", |mut c: Caller<'_, Host>, buf: i32, cap: i32| -> Result<i32, Error> {
        let n = c.data().api.network_info();
        let mut s = String::new();
        let _ = write!(s, "{}\t{}\t{}", n.host, n.ip, u8::from(n.connected));
        write_out(&mut c, buf, cap, s.as_bytes())
    })?;
    linker.func_wrap(MODULE, "refresh_tasks", |c: Caller<'_, Host>| c.data().api.refresh_tasks())?;
    linker.func_wrap(MODULE, "task_count", |c: Caller<'_, Host>| c.data().api.task_count())?;
    linker.func_wrap(MODULE, "task_info", |mut c: Caller<'_, Host>, index: i32, buf: i32, cap: i32| -> Result<i32, Error> {
        let rec = c.data().api.task_info(i64::from(index)).map(|t| {
            let mut s = String::new();
            let _ = write!(s, "{}\t{}\t{}", t.name, t.state, t.cpu_percent);
            s
        });
        out_opt(&mut c, buf, cap, rec)
    })?;

    // Files. Paths are UTF-8 strings; file contents are raw bytes (§15.3).
    // Every byte moved either way goes through read_*/write_out, so it is
    // fuelled; each call also pays FS_CALL_BYTES up front for the syscall.
    linker.func_wrap(MODULE, "fs_list", |mut c: Caller<'_, Host>, p: i32, l: i32, buf: i32, cap: i32| -> Result<i32, Error> {
        charge(&mut c, FS_CALL_BYTES)?;
        let api = c.data().api.clone();
        let r = joined(api.fs_list(read_str(&mut c, p, l)?));
        out_result(&mut c, buf, cap, r)
    })?;
    linker.func_wrap(MODULE, "fs_read", |mut c: Caller<'_, Host>, p: i32, l: i32, buf: i32, cap: i32| -> Result<i32, Error> {
        charge(&mut c, FS_CALL_BYTES)?;
        let api = c.data().api.clone();
        let r = api.fs_read(read_str(&mut c, p, l)?);
        out_result(&mut c, buf, cap, r)
    })?;
    // A size past i64::MAX saturates to i64::MAX rather than wrap negative
    // into an error code.
    linker.func_wrap(MODULE, "fs_size", |mut c: Caller<'_, Host>, p: i32, l: i32| -> Result<i64, Error> {
        charge(&mut c, FS_CALL_BYTES)?;
        let api = c.data().api.clone();
        Ok(match api.fs_size(read_str(&mut c, p, l)?) {
            Ok(n) => i64::try_from(n).unwrap_or(i64::MAX),
            Err(e) => i64::from(err_code(&e)),
        })
    })?;
    linker.func_wrap(MODULE, "fs_write", |mut c: Caller<'_, Host>, p: i32, l: i32, dp: i32, dl: i32| -> Result<i32, Error> {
        charge(&mut c, FS_CALL_BYTES)?;
        let api = c.data().api.clone();
        let path = String::from(read_str(&mut c, p, l)?);
        let data = read_bytes(&mut c, dp, dl)?;
        Ok(status(api.fs_write(&path, data)))
    })?;
    linker.func_wrap(MODULE, "fs_rename", |mut c: Caller<'_, Host>, fp: i32, fl: i32, tp: i32, tl: i32| -> Result<i32, Error> {
        charge(&mut c, FS_CALL_BYTES)?;
        let api = c.data().api.clone();
        let from = String::from(read_str(&mut c, fp, fl)?);
        let to = read_str(&mut c, tp, tl)?;
        Ok(status(api.fs_rename(&from, to)))
    })?;
    linker.func_wrap(MODULE, "fs_delete", |mut c: Caller<'_, Host>, p: i32, l: i32| -> Result<i32, Error> {
        charge(&mut c, FS_CALL_BYTES)?;
        let api = c.data().api.clone();
        Ok(status(api.fs_delete(read_str(&mut c, p, l)?)))
    })?;

    // Host cart folders: the api refuses them for carts ("not allowed", -4).
    linker.func_wrap(MODULE, "cart_roots", |mut c: Caller<'_, Host>, buf: i32, cap: i32| -> Result<i32, Error> {
        let r = joined(c.data().api.cart_roots());
        out_result(&mut c, buf, cap, r)
    })?;
    linker.func_wrap(MODULE, "cart_list", |mut c: Caller<'_, Host>, p: i32, l: i32, buf: i32, cap: i32| -> Result<i32, Error> {
        let api = c.data().api.clone();
        let r = joined(api.cart_list(read_str(&mut c, p, l)?));
        out_result(&mut c, buf, cap, r)
    })?;
    linker.func_wrap(MODULE, "cart_stat", |mut c: Caller<'_, Host>, p: i32, l: i32, buf: i32, cap: i32| -> Result<i32, Error> {
        let api = c.data().api.clone();
        let r = api.cart_stat(read_str(&mut c, p, l)?).map(|(is_dir, size)| {
            let mut s = String::new();
            let _ = write!(s, "{}\t{}", if is_dir { "dir" } else { "file" }, size);
            s.into_bytes()
        });
        out_result(&mut c, buf, cap, r)
    })?;
    linker.func_wrap(MODULE, "cart_read", |mut c: Caller<'_, Host>, p: i32, l: i32, buf: i32, cap: i32| -> Result<i32, Error> {
        let api = c.data().api.clone();
        let r = api.cart_read(read_str(&mut c, p, l)?);
        out_result(&mut c, buf, cap, r)
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{circle_bytes, fuel_cost, line_bytes, mesh_bytes, range, rect_bytes, text_bytes, triangle_bytes};

    const W: (i32, i32) = (640, 360);

    #[test]
    fn fuel_is_one_per_8_bytes_rounded_up() {
        assert_eq!([0, 1, 8, 9, 16, 1 << 20].map(fuel_cost), [0, 1, 1, 2, 2, 131_072]);
    }

    #[test]
    fn draw_charges_are_clipped_to_the_screen_and_overflow_safe() {
        assert_eq!(rect_bytes(W, 640, 336), 640 * 336 * 2);
        assert_eq!(rect_bytes(W, i32::MAX, i32::MAX), 640 * 360 * 2);
        assert_eq!(rect_bytes(W, -5, 100), 0);
        assert_eq!(rect_bytes(W, 10, i32::MIN), 0);
        assert_eq!(circle_bytes(W, 0), 2);
        assert_eq!(circle_bytes(W, 10), 21 * 21 * 2);
        assert_eq!(circle_bytes(W, i32::MAX), 640 * 360 * 2);
        assert_eq!(circle_bytes(W, -1), 0);
        assert_eq!(text_bytes(W, 2, 1), 2 * 48 * 2);
        assert_eq!(text_bytes(W, i32::MAX, 1), 640 * 360 * 2);
        assert_eq!(text_bytes(W, -1, 1), 640 * 360 * 2, "a negative length is a huge u32; it traps out of bounds anyway");
        assert_eq!(rect_bytes((640, 480), i32::MAX, i32::MAX), 640 * 480 * 2);
        let svga = (800, 600);
        assert_eq!(rect_bytes(svga, i32::MAX, i32::MAX), 800 * 600 * 2, "a full fill costs more on a bigger screen");
        assert_eq!(circle_bytes(svga, i32::MAX), 800 * 600 * 2);
        assert_eq!(text_bytes(svga, i32::MAX, 1), 800 * 600 * 2);
        assert_eq!(text_bytes(W, 2, 2), 4 * text_bytes(W, 2, 1), "a Large glyph costs four times the pixels");
        assert_eq!(text_bytes(W, i32::MAX, 2), 640 * 360 * 2, "the screen cap still holds at scale 2");
    }

    #[test]
    fn line_triangle_and_mesh_charges() {
        assert_eq!(line_bytes(W, 0, 0, 9, 3), 10 * 2);
        assert_eq!(line_bytes(W, 5, 5, 5, 5), 2);
        assert_eq!(line_bytes(W, 0, 0, -3, 20), 21 * 2);
        assert_eq!(line_bytes(W, i32::MIN, i32::MIN, i32::MAX, i32::MAX), 640 * 360 * 2);
        assert_eq!(triangle_bytes(W, [(0, 0), (9, 0), (0, 4)]), 10 * 5 * 2);
        assert_eq!(triangle_bytes(W, [(i32::MIN, i32::MIN), (i32::MAX, 0), (0, i32::MAX)]), 640 * 360 * 2);
        assert_eq!(mesh_bytes(W, 1000), 2000);
        assert_eq!(mesh_bytes(W, u64::MAX), 640 * 360 * 64 * 2);
    }

    #[test]
    fn ranges_are_unsigned_and_overflow_safe() {
        assert_eq!(range(16, 2, 64), Some(16..18));
        assert_eq!(range(62, 2, 64), Some(62..64));
        assert_eq!(range(63, 2, 64), None);
        assert_eq!(range(-1, 1, 64), None, "ptr -1 is 4 GiB - 1, not before 0");
        assert_eq!(range(0, -1, 64), None);
        assert_eq!(range(i32::MAX, i32::MAX, usize::MAX), Some(0x7fff_ffff..0xffff_fffe));
    }
}
