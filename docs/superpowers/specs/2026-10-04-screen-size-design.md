# Selectable Screen Size — Design

**Date:** 2026-10-04
**Status:** approved in brainstorming, awaiting spec review

## Goal

Acid OS v3 runs at one of three screen sizes, chosen at startup:
**640×480 (the new default)**, 640×360 (today's size) and 800×600.

Today the size is a compile-time constant pair (`SCREEN_W`/`SCREEN_H` in
`crates/acid-kernel/src/layout.rs`, 640×360). The host window
(`crates/acid-hosted/src/window.rs`), the wallpaper run table, three Lua
apps, the wasm fuel pricing and the golden tests all assume it.

## Roadmap context

This is the first of three planned features. Each gets its own spec:

1. **Selectable screen size** (this spec).
2. **Resizable app windows**: drag a corner or edge, opt in through the
   manifest, apps get a "resized" event. It depends on (1), because the
   window limits come from the screen size.
3. **Window scroll bars**: a right-hand scroll bar for windows such as
   the File Manager. It pairs with (2).

## Decisions

| Question | Choice |
|---|---|
| Who owns the size | The kernel. It's set at construction and read through `kernel.screen()`, not a process global (that would stop tests from booting different sizes in one process). |
| Sizes offered | 640×480 (default), 640×360, 800×600. No other sizes. |
| How it's chosen | A startup picker in the host window with a ~3 s countdown to the default. `--screen WxH` skips it. |
| Wallpaper at non-16:9 sizes | Nearest-neighbour scale to cover, with the overflow cropped evenly. |
| Wasm cart fuel | Pricing stays per pixel, so a full-screen fill costs more at larger sizes. The fuel budget does not scale. |

## Out of scope

- Arbitrary sizes and live resizing of the host window.
- Resizable app windows and scroll bars (roadmap items 2 and 3).
- Changing size without a restart.
- HiDPI or integer pixel scaling of the host window. Pixels stay 1:1.

## 1. Kernel (`acid-kernel`)

### 1.1 `Screen`

`layout.rs` gains:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Screen { pub w: i32, pub h: i32 }

impl Screen {
    pub const DEFAULT: Screen = Screen { w: 640, h: 480 };
    pub const WIDE: Screen = Screen { w: 640, h: 360 };
    pub const SVGA: Screen = Screen { w: 800, h: 600 };
    /// Picker order; DEFAULT first.
    pub const PRESETS: [Screen; 3] = [Screen::DEFAULT, Screen::WIDE, Screen::SVGA];
    /// "640x480" -> Some(DEFAULT); only the presets parse.
    pub fn parse(s: &str) -> Option<Screen>;
}
```

`SCREEN_W` and `SCREEN_H` are removed. The chrome constants
(`TITLE_BAR_H`, `DESKTOP_STRIP_H`, `WINDOW_MAX`, `CART_WINDOW_MAX`, close
button geometry) don't change.

### 1.2 Kernel construction

- `Kernel::new(platform)` keeps its signature and uses `Screen::DEFAULT`.
- `Kernel::with_screen(platform, screen)` is new. `Kernel::new` calls it.
- `Kernel::screen(&self) -> Screen` returns the size; it never changes
  after construction.

### 1.3 Uses that switch to the kernel's size

- `kernel.rs`: the framebuffer allocation, the full-screen background
  fill and the `present()` dimensions.
- `overlay.rs`: the overlay canvas and its clear fills.
- `layout::window_size_ok(w, h)` becomes `window_size_ok(screen, w, h)`.
- `placement::cascade_position` takes the `Screen`.
- The wallpaper canvas is built for the kernel's size (§3).

## 2. App API

### 2.1 Lua

`acid_screen_size()` returns two integers, `w, h`. It is registered in
`crates/acid-lua/src/lib.rs` next to the other `acid_*` globals and backed
by a new `Api::screen_size()` in `acid-api`.

### 2.2 Wasm carts

Two imports in the `acid` module: `screen_w() -> i32` and
`screen_h() -> i32`. They are scalar imports, matching the rest of the
ABI.

### 2.3 Fuel

`crates/acid-wasm/src/abi.rs` currently prices against the constant
`SCREEN_PX` and clamps rectangles to `SCREEN_W`/`SCREEN_H`. Both read the
kernel's `Screen` instead. A full-screen fill therefore costs
`w * h * BYTES_PER_PX`. The per-callback fuel limits stay the same.

### 2.4 Built-in Lua apps

These stop hardcoding the size:

- `apps/desktop.lua`: `SCREEN_W` comes from `acid_screen_size()`. The
  taskbar slot count (`MAX_TASKBAR_SLOTS`) follows the width. The
  "must match the boot code" comments are removed.
- `apps/lib/acid_eggs.lua`: `SCREEN_W`/`SCREEN_H` come from
  `acid_screen_size()`.
- `apps/cart/cartfile.lua`: `MAX_W`/`MAX_H` come from
  `acid_screen_size()`, less the desktop strip as before.
- `apps/editor/layout.lua`: only the comment mentions 640×360. It is
  reworded.

### 2.5 Boot

- `acid_os::boot_with(platform, screen)` is new. `boot(platform)` calls
  it with `Screen::DEFAULT`.
- The desktop spawns at `kernel.screen().w` wide. `DESKTOP_H` (204)
  doesn't change.
- `acid_os` gains `screen_arg(args) -> Result<Option<Screen>, String>`
  next to `app_arg`, for `--screen WxH`. An unknown size returns an
  error message that lists the valid sizes.

## 3. Wallpaper (`acid-gfx`)

`wallpaper_canvas()` becomes `wallpaper_canvas_for(w, h)`:

1. Replay the 640×360 run table into a source canvas, as today.
2. If `(w, h) == (640, 360)`, return it unchanged.
3. Otherwise `scale = max(w / 640, h / 360)` (as `f32`). The scaled
   image is `ceil(640 * scale)` × `ceil(360 * scale)`. Crop offsets are
   `(scaled_w - w) / 2` and `(scaled_h - h) / 2`. Destination pixel
   `(x, y)` samples source
   `(floor((x + off_x) / scale), floor((y + off_y) / scale))`, clamped
   to the source bounds.

At 640×480 that gives scale 1.333 and about 107 px cropped from each
side. At 800×600 it gives scale 1.667 and about 133 px cropped from each
side. The canvas is built once at kernel construction, so compositing
costs the same as today.

## 4. Startup picker (`acid-hosted`)

### 4.1 Startup flow

`run_window` changes to:

```rust
pub fn run_window(
    event_loop: EventLoop<UserEvent>,
    platform: Arc<HostedPlatform>,
    preselected: Option<Screen>,
    boot: Box<dyn FnOnce(Screen)>,
)
```

`main.rs` parses `--screen` first. On an error it prints the message and
exits with code 2. Kernel boot, the `--app` launch, audio start and the
router thread all move into the `boot` closure. Nothing kernel-side
exists until a size is chosen.

- With `preselected`, the window opens at that size and `boot` runs at
  once. There is no picker.
- Without it, the window opens at 640×480 and shows the picker.

### 4.2 Picker state machine

`crates/acid-hosted/src/picker.rs` holds the picker as pure logic, with
no winit:

```rust
pub struct Picker { /* selected index, countdown remaining, counting flag */ }
impl Picker {
    pub fn new() -> Picker;                       // DEFAULT selected, 3000 ms
    pub fn tick(&mut self, elapsed_ms: u32) -> Option<Screen>; // Some on timeout
    pub fn key(&mut self, k: PickerKey) -> Option<Screen>;     // Up/Down/Enter
    pub fn click(&mut self, x: i32, y: i32) -> Option<Screen>; // row hit-test
    pub fn hover(&mut self, x: i32, y: i32);                    // moves highlight
    pub fn draw(&self, c: &mut Canvas);                         // 640x480 canvas
}
```

- Rows list the three presets in `Screen::PRESETS` order. The default
  row is labelled "640x480 (default)".
- Any key, click or mouse movement over a row stops the countdown.
- Enter picks the highlighted row, and a click picks the row under the
  pointer. When the countdown reaches zero, it picks `Screen::DEFAULT`.
- `draw` uses the acid-gfx font and the kernel theme colours. The title
  reads "Acid OS v3 — screen size", and the line under the rows reads
  "starting 640x480 in N" while counting or "Enter to start" once
  stopped.

### 4.3 Window integration

While the picker is up, `App` draws the picker's canvas into the surface
and drives `tick` from a ~100 ms timer
(`ControlFlow::WaitUntil`). Keyboard and mouse events go to the picker
instead of `platform.input`.

When the picker returns a size:

1. Call `window.request_inner_size(screen)` and resize the softbuffer
   surface to the same size.
2. Call `boot(screen)`.
3. Hand events to `platform.input` from then on.

The existing redraw guard (`buf.len() == frame.len()`) already drops any
frame whose size doesn't match the surface, so a mid-resize frame can't
corrupt the display.

## 5. Testing

### 5.1 v2-parity goldens

The existing goldens (`hello_acid`, `overlap_overlay`, `desktop`, `menu`)
prove pixel parity with v2 at 640×360. Those tests switch to
`Kernel::with_screen(.., Screen::WIDE)` and `boot_with(.., Screen::WIDE)`
so the committed frames still apply. `boot_starts_only_the_desktop`
asserts the desktop at the default size (640 wide).

### 5.2 New goldens

`desktop_640x480.ppm` and `desktop_800x600.ppm` capture the desktop
strip over the scaled wallpaper. They come from v3 itself, so the user
reviews them as PNGs before they're committed.

### 5.3 Unit and integration tests

- **Screen:** `parse` accepts the three presets and rejects
  `"1024x768"`, `"640"`, `"abc"` and `""`.
- **Wallpaper:** `wallpaper_canvas_for(640, 360)` is byte-identical to
  today's canvas. At 640×480 and 800×600 the canvas has the right size,
  every pixel holds a palette colour, and the crop is centred (spot-check
  pixels against their computed source pixels).
- **Layout:** `window_size_ok` refuses an 800-wide window at 640×480 and
  allows it at 800×600. `cascade_position` keeps windows on-screen at
  every preset.
- **Kernel:** the framebuffer and overlay are sized from the `Screen`,
  and `present()` gets `w × h` pixels.
- **API:** Lua `acid_screen_size()` reports each preset. A wasm test
  cart reports `screen_w`/`screen_h`.
- **Fuel:** a full-screen fill is charged `w * h * BYTES_PER_PX` at each
  preset.
- **Desktop:** it spawns at full width at each preset, and
  `MAX_TASKBAR_SLOTS` follows the width.
- **Picker:** the countdown times out to DEFAULT; a key stops the
  countdown; Up/Down clamp at the first and last rows; Enter picks the highlight; a
  click picks the row under the pointer and a click outside the rows
  picks nothing.
- **`screen_arg`:** absent → `Ok(None)`, valid → `Ok(Some)`,
  invalid → `Err` listing the valid sizes.
- The full existing suite (380 tests) stays green.

### 5.4 Manual check by the user

Run `cargo run --release --manifest-path v3/Cargo.toml -p acid-os` and
confirm:

- the picker appears and counts down;
- each size boots with the desktop spanning the width;
- the wallpaper fills the screen;
- `-- --screen 800x600` skips the picker.

## 6. Docs

- `docs/manual-v3`: each "640×360" becomes the current screen size
  ("640×480 by default; ask with `acid_screen_size`").
- `09-api-reference.md` gains `acid_screen_size`.
- `10-wasm-carts.md` gains `screen_w`/`screen_h` and notes that fuel
  scales with screen size.
- `01-getting-started.md` documents the picker and `--screen`.
- The doc comments in `window.rs`, `layout.rs` and `acid-os/src/lib.rs`
  are updated.
- The top-level `README.md` run instructions mention `--screen`.

## Risks

- **`request_inner_size` can be ignored by some window managers** (tiling
  WMs on Linux). If the window stays 640×480 after 800×600 is picked, the
  redraw guard drops every frame and the screen goes blank. Mitigation:
  after the request, read `window.inner_size()` on the next `Resized`
  event. If it doesn't match, log a warning to stderr. The window keeps
  `with_resizable(false)`, and `with_min_inner_size`/`with_max_inner_size`
  are set to the chosen size as a hint.
- **Lua apps that cache the size at load.** `acid_screen_size()` never
  changes during a run, so caching it is safe.
