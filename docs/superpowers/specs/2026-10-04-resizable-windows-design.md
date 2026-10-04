# Resizable Windows — Design

**Date:** 2026-10-04
**Status:** approved in brainstorming, awaiting spec review

## Goal

Windows that opt in get a grip in their bottom-right corner. Dragging the
grip shows an outline of the new size. Releasing it resizes the window
once, and the app re-lays out to fit, keeping its state.

## Decisions

| Question | Choice |
|---|---|
| Where to grab | An 8×8 grip in the bottom-right corner only. No edges and no maximize button. |
| While dragging | A 1 px outline of the target size. The resize is applied once, on release. |
| Which windows | Opt-in through the manifest (`resizable = true`, optional `min_w`/`min_h`). Carts may opt in, because it only affects their own window. |
| How apps learn | A `Resized { w, h }` event. `AcidApp` calls `on_resize(w, h)` and then redraws. Apps recompute their layout in a `layout()` method. |

## Out of scope

- Resizing from edges or other corners, maximize/restore, and keyboard resizing.
- Live resize while dragging.
- Moving the window as part of a resize. The top-left corner stays put.
- Remembering window sizes between runs.
- Opting in the desktop, Config, games, Load Cart or hello_acid.

## 1. Kernel (`acid-kernel`)

### 1.1 Opt-in

`ManifestFlags` gains `resizable: bool`, `min_w: i32` and `min_h: i32`,
read by the existing single manifest read at spawn:

- `resizable`: the key `resizable` matches case-insensitively, and its
  value, after unquoting, must be exactly `true`. Unlike `font`, it is
  honoured for carts too.
- `min_w` / `min_h`: positive integers. If absent or invalid they default
  to `RESIZE_MIN_W` = 80 and `RESIZE_MIN_H` = 48. They are clamped up to at
  least `RESIZE_MIN_W` × (`TITLE_BAR_H` + 8), and down to at most the
  window's size at spawn. A window never starts below its own minimum.

`Window` gains `resizable`, `min_w` and `min_h`, set at spawn. For
non-resizable windows the minimums are unused.

### 1.2 The grip

`RESIZE_GRIP` = 8. The grip is the square `(w − 8 .. w, h − 8 .. h)` in
window coordinates.

During `composite_frame`, after blitting each resizable window, the kernel
draws the grip on the framebuffer at the window's bottom-right corner. It
is three diagonal strokes in `THEME_HARD`: the pixels where
`(gx + gy) ∈ {8, 10, 12}` for `gx, gy ∈ 1..=6`, measured from the grip's
top-left. That keeps the strokes inside the 1 px border. Apps don't draw it and can't overwrite it.

### 1.3 Routing a resize

`KernelState` gains `pub screen: Screen`, set by `Kernel::with_screen`;
`KernelState::new()` uses `Screen::DEFAULT`. The router can then clamp to
the screen without a new parameter. The router gains a `resize: Option<Resize>` alongside `drag`, holding the
task, the press point, the original `w`/`h`, and the current target
`w`/`h`.

- **Fresh press** in a resizable window's grip: this check comes before
  the title-bar and app-touch checks. The router activates the window and
  starts a `Resize` whose target is the original size. The press is not
  sent to the app.
- **Held:** the target becomes original + (pointer − press point), clamped
  to `min_w ..= screen.w − x` and `min_h ..= screen.h − y`. For a scale-2
  window the floor is also at least one character cell plus chrome. If the
  target changed, the screen is marked dirty.
- **Release:** if the target differs from the current size, the kernel
  applies it (§1.4). Either way the gesture ends.
- **Lost task:** if the window is closed mid-gesture, `forget_task` clears
  `resize`, the same way it clears `drag`.

### 1.4 Applying a resize

`Kernel::resize_window(task, w, h)` is also used by tests. It:

1. Builds `Canvas::new(w, h)`, filled with `THEME_BG`, and copies the old
   canvas into its top-left with the clip it needs.
2. Replaces the contents of the shared canvas: `*win.canvas.lock() = new`.
   The app's `Arc` handle then sees the new size immediately.
3. Sets `win.w` and `win.h`.
4. Sends `Event::Resized { w, h }` to the app (best effort, as with every
   router event).
5. Marks the screen dirty.

### 1.5 The outline

While `router.resize` is active, `composite_frame` draws a 1 px rectangle in
`THEME_VIOLET` around the target size, at the window's position, above
everything including the overlay.

## 2. API (`acid-api`, `acid-lua`, `acid-wasm`)

- `Event::Resized { w, h }` becomes `PolledEvent::Resized { w, h }`.
- `KernelApi::window_size()` and the chrome calls (`draw_window_frame`,
  `draw_window_border`, `clear_user_area`) use the live canvas size,
  `canvas.width()`/`height()`, instead of `ctx.w`/`ctx.h`.
- Lua: `acid_poll_event` returns `"resized", w, h`.
- `AcidApp` (`apps/lib/acid_app.lua`):
  - on `"resized"` it calls `self:on_resize(w, h)`, then `self:redraw()`,
    then `acid_notify_redraw_done()`;
  - the default `on_resize` is empty.
- `AcidGame` calls `self:on_resize(w, h)` on `"resized"` and doesn't redraw,
  as with `"moved"`.
- Wasm:
  - `EV_RESIZED` is the next free event-kind number after the existing
    kinds;
  - the runner calls `acid_on_event(EV_RESIZED, w, h, 0)`, then
    `acid_redraw()`;
  - the guest crate exports the constant.

## 3. Apps

### 3.1 Opted in

These six apps get `resizable = true` and minimums:

| App | Minimum |
|---|---|
| Terminal | 160 × 80 |
| Editor | 200 × 100 |
| File Manager | 120 × 80 |
| System Monitor | 160 × 120 |
| About | 120 × 60 |
| Network | 140 × 80 |

In each app, the layout values currently computed at load from
`acid_font_size()`/`acid_window_size()` move into a `layout()` method:

- `on_create` calls `layout()` with the opening size;
- `on_resize(w, h)` calls `layout()` again, then fixes up state.

At the opening size every value equals today's value.

`EditorLayout`'s module-level assignments become
`EditorLayout.compute(w, h)`, which fills in the same fields. Editor's
other modules keep reading `EditorLayout.X` unchanged.

State after a resize:

- **File Manager:** the selected row stays visible (`ensure_listing_scroll`)
  and the preview scroll is clamped. The scroll bar appears or disappears
  by itself.
- **Editor:** the cursor stays visible, using its existing
  scroll-to-cursor logic.
- **Terminal:** keeps its scrollback and shows the newest lines.
- **System Monitor:** stays on its current page.
- **About, Network:** only re-lay out.

### 3.2 Unchanged

Desktop, Config, Tetris, Breakout, Acid Blaster, Piano, Load Cart,
hello_acid and carts are not opted in.

## 4. Testing

### 4.1 Rust

- **Manifest:** `resizable = true` sets the flag, and anything else, or no
  key, clears it. `min_w`/`min_h` parse, default and clamp. A cart's
  manifest is honoured.
- **Grip:**
  - a press in a resizable window's grip starts a resize and sends nothing
    to the app;
  - the same press on a non-resizable window is an ordinary touch;
  - title-bar dragging and the close button still work.
- **Drag:** the target follows the pointer and clamps at the minimum and at
  the screen edges. The outline pixels are present in the composited frame
  while the press is held.
- **Release:**
  - the window and canvas have the new size, and the old pixels sit
    top-left;
  - the app's queue receives `Resized { w, h }`;
  - a release at the original size sends nothing;
  - `acid_window_size` and the chrome calls report the new size.
- **Grip drawing:** the grip pixels appear at the bottom-right of resizable
  windows only.
- **API:** Lua receives `"resized", w, h`. A wasm cart receives
  `on_event(EV_RESIZED, w, h, 0)` and then `redraw`.

### 4.2 Lua

Each opted-in app gets a resize suite. It opens the app at its manifest
size, then calls `on_resize` twice: once larger, and once at the app's
minimum. Each time it checks:

- the recomputed layout values;
- `drawn_inside_window()` and `drawn_text_clear()` hold;
- the app's state still makes sense: File Manager's selection stays
  visible, Editor's cursor stays visible, and Terminal keeps its
  scrollback.

Each suite must fail against the app's current, pre-change code.

### 4.3 Golden frame

`file_manager_resized.ppm` shows File Manager after
`resize_window(.., 400, 300)` on a 640×480 desktop, with the grip visible.
The user approves it as a PNG before it is committed.

### 4.4 Manual check by the user

- Drag the grip on File Manager, Terminal and Editor. An outline should
  follow the pointer.
- On release, the window resizes and its contents re-lay out without losing
  state.
- It can't go below the minimum or off the screen.
- Games show no grip.

## 5. Docs

- `02-apps-and-manifests.md`: the `resizable`, `min_w` and `min_h` keys.
- `03-app-lifecycle.md`: the `"resized"` event and `on_resize(w, h)`.
- `04-graphics.md`: lay out in a `layout()` method so it can run again, and
  read `acid_window_size()` there.
- `09-api-reference.md`: `on_resize` in the `AcidApp` table, and
  `acid_window_size` reporting the live size.
- `10-wasm-carts.md`: `EV_RESIZED`.
- `01-getting-started.md`: one line about the corner grip.

## Risks

- **A dropped `Resized` event.** The queue holds 8 events. The app then
  still draws into the new canvas, and reads the live size the next time it
  asks. The kernel test checks delivery in normal use.
- **A canvas swap during a draw.** The swap happens under the canvas mutex,
  so a draw in progress either finishes on the old canvas or starts on the
  new one. Draws are clipped to the canvas, so neither case overruns.
