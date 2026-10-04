# Font Size Setting — Design

**Date:** 2026-10-04
**Status:** approved in brainstorming, awaiting spec review

## Goal

Config gains a FONT setting with two sizes: **Normal** (the 6×8 system font,
as today) and **Large** (the same font doubled, 12×16). Text apps opened
after the change draw their text at that size and open in a window grown to
fit. Apps already open keep the size they opened with.

## Roadmap context

The pending roadmap has three items: a 3D library, resizable app windows,
and this one. Resizable windows will reuse `acid_window_size()` from this
spec.

## Decisions

| Question | Choice |
|---|---|
| What changes | Text in opted-in apps, which re-lay out from the font's metrics. Not a zoom of the whole OS, and not just a drawing API. |
| Sizes | Normal 6×8 and Large 12×16, with each font pixel drawn as a 2×2 block. |
| Window sizes | Opted-in windows opening at Large grow to fit, clamped to the screen. |
| When it applies | Apps opened after the change. Open windows keep their scale, and there is no live re-layout. |
| Mechanism | A per-window scale fixed at spawn from the setting and a manifest opt-in. `acid_draw_text` uses it automatically. |
| Persistence | Kept in memory, like the wallpaper switch, so it is Normal at every boot. |

## Out of scope

- Live re-layout of open windows.
- Fonts other than the system font, and in-between sizes.
- Scaling the window chrome (title bar, border, close button) or the desktop strip.
- Saving settings to disk.
- Opting in the desktop, Config, games, Load Cart or carts (any of them can opt in later).

## 1. Kernel (`acid-kernel`)

### 1.1 The setting

The kernel holds `font_scale: AtomicU8`, either 1 or 2, starting at 1.
`Kernel::font_scale()` reads it. `Kernel::set_font_scale(s)` stores `s` only
when it is 1 or 2 and ignores anything else. Changing the setting never
touches an existing window and does not mark the screen dirty.

### 1.2 Opt-in and per-window scale

The manifest key is `font = scalable`. The key name is matched
case-insensitively, the value exactly after trimming and quote-stripping,
the same handling `source = cart` gets.

At spawn, `spawn_app` reads the app's manifest. That happens in the same
place, and through the same path, as `app_is_cart`, so both now come from a
single manifest read. The window's scale is set as follows:

- `scale = kernel.font_scale()` if the manifest opts in and the app is
  built-in (not cart-level);
- otherwise `scale = 1`.

A cart's `font = scalable` is ignored this round, so cart window sizes stay
exactly what the cart asked for.

Every launch path (the Menu, File Manager, Terminal `run`, `--app`,
`acid_spawn_app`) goes through `spawn_app`, so all of them are covered.

The scale is stored on the window (`Window::font_scale`) and passed to the
app in `AppContext` (`ctx.font_scale`). It never changes afterwards.

### 1.3 Window growth

When `scale == 2`, `spawn_app` replaces the requested size before checking
it and allocating the canvas:

```
w' = min(screen.w, w * 2)
h' = min(screen.h - DESKTOP_STRIP_H, TITLE_BAR_H + (h - TITLE_BAR_H) * 2)
```

At 640×480, File Manager goes from 220×160 to 440×304 and Editor from
420×280 to 640×456. The cascade position is computed by the callers before
`spawn_app`, so a grown window can end up partly off-screen at the
bottom-right. To prevent that, `spawn_app` clamps `x`/`y` so the grown
window stays on screen and below the strip, using the same rules as
`cascade_position`.

## 2. Drawing and API (`acid-gfx`, `acid-api`)

### 2.1 Scaled text

`Canvas::draw_text_scaled(x, y, text, fg, bg, scale)` draws each glyph with
every font pixel as a `scale × scale` block. A glyph cell is
`6·scale × 8·scale`. The background rule is the same as `draw_text`: the
background is painted only when `fg != bg`. `draw_text` becomes
`draw_text_scaled(.., 1)`, and its output is byte-identical to today.

`KernelApi::draw_text` draws at `ctx.font_scale`. The window chrome
(`draw_window_frame`, `draw_window_border`, `clear_user_area`) always draws
at scale 1.

### 2.2 New calls

On the `AcidApi` trait, with the `KernelApi` implementation and the test
fakes:

| Call | Returns |
|---|---|
| `font_size()` | `(6·scale, 8·scale)` for this window |
| `window_size()` | `(ctx.w, ctx.h)`, this window's real size |
| `font_scale()` | the setting (1 or 2) |
| `set_font_scale(s)` | sets the setting. Open to carts, like volume and wallpaper. |

The test fakes return `(6, 8)`, the fake window size, and 1, and do not log.

The Lua bindings are `acid_font_size()` → `w, h`, `acid_window_size()` →
`w, h`, `acid_get_font_scale()` → `n`, and `acid_set_font_scale(n)`.

The wasm imports are `font_w`, `font_h`, `window_w`, `window_h` and
`get_font_scale`, each returning `i32`, and `set_font_scale(i32)`. They go
in `IMPORT_NAMES`, and the guest crate gets bindings `font_size()`,
`window_size()`, `font_scale()` and `set_font_scale()`.

### 2.3 Fuel

A wasm cart's `draw_text` is charged one glyph cell per byte at the cart's
own scale: `GLYPH_PX · scale²`, still capped at the screen. Carts don't
opt in this round, so their scale is always 1. The charge is written for
the scaled case anyway.

## 3. Apps

### 3.1 Opted in

These apps get `font = scalable` in their `.app.toml`:

- Terminal;
- Editor;
- File Manager;
- System Monitor;
- About;
- Network.

Each one reads `acid_font_size()` and `acid_window_size()` once in
`on_create`. Every value it currently hard-codes is derived from those two:

- character width;
- row and line height, keeping each app's current padding (for example,
  File Manager's `ROW_H` = char_h + 4);
- visible rows and columns;
- text clipping (for example, File Manager's `sub(1, 34)` becomes
  "columns that fit");
- touch hit-testing (the row or column under the pointer);
- `WINDOW_W`/`WINDOW_H`.

At Normal every derived value equals today's constant. Where an app keeps a
named constant for its Normal value, it stays as documentation, but layout
reads the derived value.

File Manager's scroll bar keeps its 6 px width. Its track starts below the
(now taller) header row.

### 3.2 Unchanged

These apps stay as they are:

- the desktop;
- Config, which gets a new section but draws at Normal;
- Tetris, Breakout, Acid Blaster and Piano;
- Load Cart;
- hello_acid;
- every cart.

### 3.3 Config

A FONT section goes below Wallpaper. It has two buttons, **NORMAL** and
**LARGE**. The current one is highlighted the way the wallpaper toggle is:
accent background with background-coloured text. Under the buttons, a muted
note reads "applies to newly opened apps". `on_create` reads
`acid_get_font_scale()`, and a tap calls `acid_set_font_scale`.

The window grows from 180×140 to 180×190, so `config.app.toml` changes with
it. The existing volume and wallpaper layout keeps its coordinates.

## 4. Testing

### 4.1 Rust

- **Setting:** `set_font_scale` stores 1 and 2 and ignores 0, 3 and -1.
- **Opt-in:** an app with `font = scalable` gets scale 2 when the setting
  is Large. One without the key gets 1. An already-open window keeps scale
  1 after the setting changes.
- **Growth:** sizes for File Manager and Editor at 640×360, 640×480 and
  800×600. The clamp at each screen is checked, and a grown window is
  clamped on screen.
- **Drawing:** `draw_text_scaled(.., 2)` gives each pixel of the scale-1
  glyph as a 2×2 block, compared pixel for pixel. `draw_text` is unchanged.
  The chrome title text stays scale 1 in a scale-2 window.
- **API:** `acid_font_size`, `acid_window_size`, `acid_get_font_scale` and
  `acid_set_font_scale` work from Lua. The wasm imports return the same
  values. The import table, its count, and manual chapter 10 stay in sync.
- **Fuel:** the text charge at scale 2 is four times the scale-1 charge.

### 4.2 Lua (headless suites)

`game_test_env.lua` gains `acid_font_size`/`acid_window_size` stubs driven by
`FONT_W`/`FONT_H`/`WIN_W`/`WIN_H` globals. They default to 6×8 and to each
app's current constant size, so the existing suites are unchanged. New
preload files switch to Large: `font_large.lua` sets 12×16, and the window
size is set per suite. Each opted-in app gets a Large suite:

- **File Manager:** visible rows, clipping, row taps, and the scroll bar
  geometry.
- **Terminal:** columns and rows, and that a wrapped line wraps at the new
  column count.
- **Editor:** columns and lines, and touch-to-cursor at the new cell size.
- **System Monitor, About, Network:** every text rectangle drawn lies inside
  the window.

### 4.3 Golden frame

`file_manager_large.ppm` shows File Manager opened at Large over the desktop
at 640×480. The user approves it as a PNG before it is committed, as was done
for the screen-size goldens.

### 4.4 Manual check by the user

Change FONT to LARGE in Config, then open File Manager, Terminal and Editor.
The text should be doubled, the windows bigger, and everything readable,
with nothing cut off mid-character. Windows that were already open stay
Normal.

## 5. Docs

- `02-apps-and-manifests.md`: the `font = scalable` key.
- `04-graphics.md`: a "Text size" subsection on scale, and laying out from
  `acid_font_size`/`acid_window_size` rather than constants.
- `09-api-reference.md`: the four new calls, with index entries.
- `10-wasm-carts.md`: the six new imports.
- `01-getting-started.md`: one line about Config's FONT setting.

## Risks

- **Hard-coded layout numbers missed in an app.** At Large this shows up as
  overlap or clipped text. The per-app Large suites and the "inside the
  window" checks are there to catch it. Apps that don't opt in are
  untouched by construction.
- **Grown windows crowd the screen.** That is the expected trade-off at
  Large. Resizable windows (roadmap) will address it later.
