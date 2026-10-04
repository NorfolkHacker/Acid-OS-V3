# Selectable Screen Size Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Acid OS v3 runs at 640×480 (default), 640×360 or 800×600. The size is picked in a startup picker, or with `--screen WxH`.

**Architecture:** A `Screen { w, h }` value replaces the `SCREEN_W`/`SCREEN_H` constants. The kernel receives it at construction and owns it, and everything reads it from there:

- the framebuffer, overlay, placement and size checks;
- wasm fuel pricing, through `AcidApi::screen_size`;
- Lua apps, through `acid_screen_size()`, and wasm carts, through the `screen_w`/`screen_h` imports.

The wallpaper art stays 640×360 and is scaled to cover the screen. The host window shows a pure-logic picker before anything kernel-side exists, then boots the chosen size.

**Tech Stack:** Rust 2024 workspace (`v3/`), mlua 0.10 (Lua 5.4), wasmi 2, winit 0.30 + softbuffer 0.4.

**Spec:** `docs/superpowers/specs/2026-10-04-screen-size-design.md`

## Global Constraints

- Repo: `/home/norfolkh/acid-os-v3`, branch `screen-size`. Run every command from the repo root.
- Full test command: `cargo test --manifest-path v3/Cargo.toml --workspace -q`. The baseline is 380 tests passing, and every task ends with the whole suite green.
- Presets: `Screen::DEFAULT` = 640×480, `Screen::WIDE` = 640×360, `Screen::SVGA` = 800×600. `Screen::PRESETS` = `[DEFAULT, WIDE, SVGA]`, in that order. No other sizes.
- Golden frames that prove v2 parity (`hello_acid.ppm`, `overlap_overlay.ppm`, `desktop.ppm`, `menu.ppm`) are never regenerated. Their tests run at `Screen::WIDE`.
- **Rule for existing tests:** a test that fails only because it hardcodes 640×360 geometry switches its kernel to `Kernel::with_screen(.., Screen::WIDE)` (or `boot_with(.., Screen::WIDE)`). Never change its expected values.
- Test fakes of `AcidApi` return `(640, 360)` from `screen_size` and **do not log the call**: fuel charging calls it on every draw, and the tests compare call logs.
- The kernel crates are `no_std` + `alloc` (`acid-kernel`, `acid-api`): no `std::`, no `format!` without `alloc::format`.
- Commits use path-only `git add <paths>` and end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. Git identity is already set repo-locally (NorfolkHacker); don't change it.
- Comment style: match the surrounding code. Doc comments say why. Don't leave "was 640x360" history notes.

---

### Task 1: Wallpaper scaled to any screen

**Files:**
- Modify: `v3/crates/acid-gfx/src/wallpaper.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `acid_gfx::wallpaper::wallpaper_canvas_for(w: i32, h: i32) -> Canvas`. `wallpaper_canvas()` stays as it is and returns the 640×360 source art.

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module in `v3/crates/acid-gfx/src/wallpaper.rs`:

```rust
    #[test]
    fn wallpaper_for_its_own_size_is_the_art() {
        assert_eq!(wallpaper_canvas_for(640, 360).pixels(), wallpaper_canvas().pixels());
    }

    #[test]
    fn scaled_wallpaper_has_the_screen_size_and_only_art_colours() {
        let palette: Vec<u16> = WALLPAPER_PALETTE.iter().map(|&c| rgb565(c)).collect();
        for (w, h) in [(640, 480), (800, 600)] {
            let c = wallpaper_canvas_for(w, h);
            assert_eq!((c.width(), c.height()), (w, h));
            assert!(c.pixels().iter().all(|p| palette.contains(p)), "{w}x{h} has a pixel not from the art");
        }
    }

    #[test]
    fn scaled_wallpaper_is_centre_cropped() {
        let art = wallpaper_canvas();
        // 640x480: scale 480/360, 854 px wide scaled, 107 px cropped each side.
        let c = wallpaper_canvas_for(640, 480);
        assert_eq!(c.pixel(0, 0), art.pixel(80, 0));
        assert_eq!(c.pixel(639, 479), art.pixel(559, 359));
        // 800x600: scale 600/360, 1067 px wide scaled, 133 px cropped each side.
        let c = wallpaper_canvas_for(800, 600);
        assert_eq!(c.pixel(0, 0), art.pixel(79, 0));
        assert_eq!(c.pixel(400, 300), art.pixel(319, 180));
        assert_eq!(c.pixel(799, 599), art.pixel(559, 359));
    }
```

Also add `use crate::wallpaper_data::WALLPAPER_PALETTE;` next to the existing `use crate::wallpaper_data::WALLPAPER_RUNS;` in the tests module.

- [ ] **Step 2: Run them and confirm they fail**

Run: `cargo test --manifest-path v3/Cargo.toml -p acid-gfx wallpaper -q`
Expected: compile error, `cannot find function wallpaper_canvas_for`.

- [ ] **Step 3: Implement**

In `v3/crates/acid-gfx/src/wallpaper.rs`, keep `wallpaper_canvas()` and add below it:

```rust
/// The wallpaper for a `w × h` screen: the art scaled (nearest neighbour)
/// until it covers the screen, with the overflow cropped evenly from both
/// sides, so the middle of the picture stays in the middle. At the art's
/// own 640×360 it is the art itself. Integer maths only, so every platform
/// draws the same pixels.
pub fn wallpaper_canvas_for(w: i32, h: i32) -> Canvas {
    let art = wallpaper_canvas();
    if (w, h) == (WALLPAPER_W, WALLPAPER_H) {
        return art;
    }
    // scale = num / den = max(w / WALLPAPER_W, h / WALLPAPER_H).
    let (num, den) = if w * WALLPAPER_H >= h * WALLPAPER_W { (w, WALLPAPER_W) } else { (h, WALLPAPER_H) };
    let scaled_w = (WALLPAPER_W * num).div_ceil(den);
    let scaled_h = (WALLPAPER_H * num).div_ceil(den);
    let (off_x, off_y) = ((scaled_w - w) / 2, (scaled_h - h) / 2);
    let mut c = Canvas::new(w, h);
    for y in 0..h {
        let sy = ((y + off_y) * den / num).min(WALLPAPER_H - 1);
        for x in 0..w {
            let sx = ((x + off_x) * den / num).min(WALLPAPER_W - 1);
            if let Some(p) = art.pixel(sx, sy) {
                c.fill_rect565(x, y, 1, 1, p);
            }
        }
    }
    c
}
```

- [ ] **Step 4: Run them and confirm they pass**

Run: `cargo test --manifest-path v3/Cargo.toml -p acid-gfx -q`
Expected: all pass, including the three new tests.

- [ ] **Step 5: Commit**

```bash
git add v3/crates/acid-gfx/src/wallpaper.rs
git commit -m "Scale the wallpaper to cover any screen size

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 2: The kernel owns the screen size

This replaces the constants everywhere in Rust. The workspace doesn't compile until every user is switched, so this task covers the kernel, `acid-api`, wasm fuel and `acid-os` boot together.

**Files:**
- Modify: `v3/crates/acid-kernel/src/layout.rs` (`Screen`, `window_size_ok`)
- Modify: `v3/crates/acid-kernel/src/kernel.rs` (field, `with_screen`, `screen()`, framebuffer, wallpaper, composite, `spawn_app`)
- Modify: `v3/crates/acid-kernel/src/overlay.rs` (screen-sized canvas)
- Modify: `v3/crates/acid-kernel/src/placement.rs` (`cascade_position` takes a `Screen`)
- Modify: `v3/crates/acid-kernel/src/windows_api.rs` (`launcher_register`, launch cascade)
- Modify: `v3/crates/acid-kernel/src/test_support.rs` (`setup()` at `Screen::WIDE`)
- Modify: `v3/crates/acid-api/src/lib.rs` (`AcidApi::screen_size` and the `KernelApi` impl)
- Modify: `v3/crates/acid-lua/tests/lua_app.rs` and `v3/crates/acid-wasm/tests/abi.rs` (fakes implement `screen_size`)
- Modify: `v3/crates/acid-wasm/src/abi.rs` (fuel reads the screen)
- Modify: `v3/crates/acid-os/src/lib.rs` (`boot_with`, desktop width, manifest cascade)
- Modify: `v3/crates/acid-os/tests/golden.rs` (pin `WIDE`, generalise the golden size check)

**Interfaces:**
- Consumes: `acid_gfx::wallpaper::wallpaper_canvas_for(w, h)` (Task 1).
- Produces:
  - `acid_kernel::layout::Screen { pub w: i32, pub h: i32 }` with `DEFAULT`, `WIDE`, `SVGA`, `PRESETS: [Screen; 3]` and `fn parse(&str) -> Option<Screen>`
  - `layout::window_size_ok(screen: Screen, w: i32, h: i32) -> bool`
  - `placement::cascade_position(screen: Screen, existing: usize, w: i32, h: i32) -> (i32, i32)`
  - `Kernel::with_screen(platform: Arc<dyn Platform>, screen: Screen) -> Arc<Kernel>`
  - `Kernel::screen(&self) -> Screen`
  - `AcidApi::screen_size(&self) -> (i32, i32)`
  - `acid_os::boot_with(platform: Arc<dyn Platform>, screen: Screen) -> Arc<Kernel>`
  - `SCREEN_W` and `SCREEN_H` no longer exist.

- [ ] **Step 1: Write the failing layout tests**

Add a tests module at the end of `v3/crates/acid-kernel/src/layout.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_presets_parse() {
        assert_eq!(Screen::parse("640x480"), Some(Screen::DEFAULT));
        assert_eq!(Screen::parse("640x360"), Some(Screen::WIDE));
        assert_eq!(Screen::parse("800x600"), Some(Screen::SVGA));
        for bad in ["1024x768", "640", "abc", "", "640X480", "+640x480", "640x480x1", "x"] {
            assert_eq!(Screen::parse(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn the_default_is_first_and_640x480() {
        assert_eq!(Screen::PRESETS[0], Screen::DEFAULT);
        assert_eq!((Screen::DEFAULT.w, Screen::DEFAULT.h), (640, 480));
    }

    #[test]
    fn window_size_ok_follows_the_screen() {
        assert!(!window_size_ok(Screen::DEFAULT, 800, 100), "too wide at 640x480");
        assert!(window_size_ok(Screen::SVGA, 800, 100));
        assert!(window_size_ok(Screen::DEFAULT, 640, 480));
        assert!(!window_size_ok(Screen::WIDE, 640, 480), "too tall at 640x360");
        assert!(!window_size_ok(Screen::SVGA, 0, 10));
    }
}
```

- [ ] **Step 2: Implement `Screen` and `window_size_ok`**

In `layout.rs`, replace the `SCREEN_W`/`SCREEN_H` constants and the `window_size_ok` function with:

```rust
/// A screen size. The kernel gets one when it's built and keeps it for
/// its whole life (`Kernel::screen`), so a click always lands where
/// chrome is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Screen {
    pub w: i32,
    pub h: i32,
}

impl Screen {
    pub const DEFAULT: Screen = Screen { w: 640, h: 480 };
    /// v2's size: the golden frames that prove parity with v2 use it.
    pub const WIDE: Screen = Screen { w: 640, h: 360 };
    pub const SVGA: Screen = Screen { w: 800, h: 600 };
    /// Every size the OS runs at, in the startup picker's order.
    pub const PRESETS: [Screen; 3] = [Screen::DEFAULT, Screen::WIDE, Screen::SVGA];

    /// "640x480" -> `DEFAULT`. Only the presets parse.
    pub fn parse(s: &str) -> Option<Screen> {
        fn num(s: &str) -> Option<i32> {
            if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            s.parse().ok()
        }
        let (w, h) = s.split_once('x')?;
        let (w, h) = (num(w)?, num(h)?);
        Self::PRESETS.into_iter().find(|p| p.w == w && p.h == h)
    }
}

/// A window must fit the screen; the check runs before anything is allocated.
pub fn window_size_ok(screen: Screen, w: i32, h: i32) -> bool {
    (1..=screen.w).contains(&w) && (1..=screen.h).contains(&h)
}
```

`"640x480x1"` fails because `num("480x1")` rejects the `x`.

- [ ] **Step 3: `cascade_position` takes the screen**

In `v3/crates/acid-kernel/src/placement.rs`:
- change `use crate::layout::{DESKTOP_STRIP_H, SCREEN_H, SCREEN_W};` to `use crate::layout::{DESKTOP_STRIP_H, Screen};`;
- change the signature to `pub fn cascade_position(screen: Screen, existing: usize, w: i32, h: i32) -> (i32, i32)`;
- change the body to `let max_x = screen.w - w;` and `let max_y = screen.h - h;`;
- in the tests, change every `cascade_position(` to `cascade_position(Screen::WIDE, `;
- add this test:

```rust
    #[test]
    fn a_bigger_screen_clamps_later() {
        assert_eq!(cascade_position(Screen::WIDE, 0, 700, 150), (0, 34), "wider than 640");
        assert_eq!(cascade_position(Screen::SVGA, 0, 700, 150), (20, 34), "fits at 800");
        assert_eq!(cascade_position(Screen::DEFAULT, 0, 200, 400), (20, 34), "fits at 480 tall");
    }
```

- [ ] **Step 4: The kernel holds the screen**

In `v3/crates/acid-kernel/src/kernel.rs`:
- import `use acid_gfx::{Canvas, wallpaper::wallpaper_canvas_for};` (drop `wallpaper_canvas` from the non-test import) and `use crate::layout::{CART_WINDOW_MAX, Screen, window_size_ok};`;
- add a field `screen: Screen,` to `Kernel` (put it first, after `platform`), with the doc comment `/// Fixed for the kernel's life (spec: screen size is chosen once at startup).`;
- replace `pub fn new` with:

```rust
    /// A kernel at the default screen size.
    pub fn new(platform: Arc<dyn Platform>) -> Arc<Self> {
        Self::with_screen(platform, Screen::DEFAULT)
    }

    pub fn with_screen(platform: Arc<dyn Platform>, screen: Screen) -> Arc<Self> {
        Arc::new(Self {
            platform,
            screen,
            state: Mutex::new(KernelState::new()),
            // Starts dirty, so the first tick always draws.
            dirty: AtomicBool::new(true),
            next_task: AtomicU32::new(0),
            runner: Mutex::new(None),
            framebuffer: Mutex::new(Canvas::new(screen.w, screen.h)),
            overlay: Mutex::new(crate::overlay::Overlay::new(screen)),
            launcher: Mutex::new(crate::launcher::Launcher::new()),
            audio: crate::audio::AudioRuntime::new(),
            tasks: Mutex::new(Default::default()),
            wallpaper: wallpaper_canvas_for(screen.w, screen.h),
            // On at boot; the setting lives in memory only.
            wallpaper_enabled: AtomicBool::new(true),
            composited: AtomicU32::new(0),
            skipped: AtomicU32::new(0),
        })
    }

    pub fn screen(&self) -> Screen {
        self.screen
    }
```

- in `spawn_app`: `if !window_size_ok(self.screen, req.w, req.h) {`;
- in `composite_frame`: `fb.fill_rect(0, 0, self.screen.w, self.screen.h, THEME_BG);` and `self.platform.display().present(fb.pixels(), self.screen.w as usize, self.screen.h as usize);`;
- in the `tests` module, keep `use acid_gfx::{rgb565, wallpaper::wallpaper_canvas};` (the existing tests run at `WIDE`, where the wallpaper is the art itself), and add `wallpaper_canvas_for` to that import.

- [ ] **Step 5: The overlay is sized from the screen**

In `v3/crates/acid-kernel/src/overlay.rs`:
- change the import to `use crate::layout::Screen;`;
- drop `#[derive(Default)]` from `Overlay`, add a `screen: Screen,` field, and replace `new` with:

```rust
    pub fn new(screen: Screen) -> Self {
        Self { canvas: None, owner: None, screen }
    }
```

- in `open`, change the comment to `// Allocated on first use: nobody pays for a screen-sized canvas until an app wants it.`, and set the body to:

```rust
        let s = self.screen;
        let c = self.canvas.get_or_insert_with(|| Canvas::new(s.w, s.h));
        c.fill_rect(0, 0, s.w, s.h, ACID_OVERLAY_KEY);
```

- in `Kernel::overlay_clear`: `c.fill_rect(0, 0, self.screen.w, self.screen.h, ACID_OVERLAY_KEY);`. The `screen` field is private to `kernel.rs`; read it through `self.screen()`.

- [ ] **Step 6: `windows_api.rs`**

- `launcher_register`: `if !crate::layout::window_size_ok(self.screen(), app.w, app.h) {`
- the launch path: `let (x, y) = cascade_position(self.screen(), self.with_state(|st| st.windows.count()), w, h);`

- [ ] **Step 7: Kernel tests run at v2's size; add tests for the new sizes**

In `v3/crates/acid-kernel/src/test_support.rs`, change `setup()` to build `Kernel::with_screen(p.clone(), crate::layout::Screen::WIDE)` and add a doc comment: `/// At v2's 640x360: the kernel tests' coordinates were written for it.` Then add:

```rust
/// As `setup`, at `screen`.
pub(crate) fn setup_at(screen: crate::layout::Screen) -> (Arc<FakePlatform>, Arc<Kernel>, mpsc::Receiver<AppContext>) {
    let p = FakePlatform::new(".");
    let k = Kernel::with_screen(p.clone(), screen);
    let (tx, rx) = mpsc::channel();
    k.set_runner(parked_runner(tx));
    (p, k, rx)
}
```

Make `setup()` call `setup_at(Screen::WIDE)`.

Add to the `tests` module in `kernel.rs`:

```rust
    #[test]
    fn new_uses_the_default_screen() {
        let k = Kernel::new(FakePlatform::new("."));
        assert_eq!(k.screen(), crate::layout::Screen::DEFAULT);
    }

    #[test]
    fn the_frame_and_wallpaper_follow_the_screen() {
        for s in crate::layout::Screen::PRESETS {
            let (p, k, _rx) = setup_at(s);
            k.composite_frame();
            let f = p.display.last_frame().unwrap();
            assert_eq!(f.len(), (s.w * s.h) as usize, "{s:?}");
            let wall = wallpaper_canvas_for(s.w, s.h);
            assert_eq!(f[0], wall.pixel(0, 0).unwrap(), "{s:?}");
            let last = (s.w * s.h - 1) as usize;
            assert_eq!(f[last], wall.pixel(s.w - 1, s.h - 1).unwrap(), "{s:?}");
        }
    }

    #[test]
    fn spawn_accepts_a_window_only_as_big_as_the_screen() {
        let (_p, k, _rx) = setup_at(crate::layout::Screen::SVGA);
        assert!(k.spawn_app(req(0, 0, 800, 600)).is_some());
        let (_p, k, _rx) = setup_at(crate::layout::Screen::DEFAULT);
        assert!(k.spawn_app(req(0, 0, 800, 600)).is_none());
    }
```

Add to the `tests` module in `overlay.rs`:

```rust
    #[test]
    fn the_overlay_covers_a_bigger_screen() {
        let (p, k, rx) = setup_at(crate::layout::Screen::SVGA);
        let t = k.spawn_app(req(0, 30, 10, 10)).unwrap();
        recv(&rx);
        assert!(k.overlay_open(t));
        k.overlay_fill_rect(t, 790, 590, 20, 20, 0x00E5FF);
        k.composite_frame();
        let f = p.display.last_frame().unwrap();
        assert_eq!(f[(599 * 800 + 799) as usize], rgb565(0x00E5FF));
    }
```

(The overlay tests already import from `crate::test_support::*`; `setup_at` comes with it.)

- [ ] **Step 8: `AcidApi::screen_size`**

In `v3/crates/acid-api/src/lib.rs`, add to the trait, after `fn window_max(&self) -> i32;` and its doc comment:

```rust
    /// The screen's size in pixels, `(w, h)`; fixed for the whole run.
    fn screen_size(&self) -> (i32, i32);
```

In `impl AcidApi for KernelApi`, after `window_max`:

```rust
    fn screen_size(&self) -> (i32, i32) {
        let s = self.ctx.kernel.screen();
        (s.w, s.h)
    }
```

Add a test to the `tests` module in `acid-api/src/lib.rs`:

```rust
    #[test]
    fn screen_size_is_the_kernels() {
        let (k, a) = spawn(100, 100);
        assert_eq!(a.screen_size(), (k.screen().w, k.screen().h));
    }
```

In `v3/crates/acid-lua/tests/lua_app.rs` (`impl AcidApi for FakeApi`) and `v3/crates/acid-wasm/tests/abi.rs` (`impl AcidApi for Rec`), add, **without logging**:

```rust
    fn screen_size(&self) -> (i32, i32) { (640, 360) }
```

- [ ] **Step 9: Wasm fuel reads the screen**

In `v3/crates/acid-wasm/src/abi.rs`:
- change the import to `use acid_kernel::layout::TITLE_BAR_H;`;
- delete `const SCREEN_PX`, and update the `BYTES_PER_PX` doc comment's example to: `so a full-screen fill costs w × h × 2 / 8 fuel (76,800 at 640×480)`;
- replace `rect_bytes`, `circle_bytes` and `text_bytes` with:

```rust
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
/// byte count never undercounts the glyphs), at most the screen.
fn text_bytes(s: ScreenWh, len: i32) -> u64 {
    (len as u32 as u64).saturating_mul(GLYPH_PX).min(screen_px(s)) * BYTES_PER_PX
}
```

- At every `charge(&mut c, ...)` call site, read the screen into a local **first**. `charge(&mut c, rect_bytes(screen(&c), ..))` does not borrow-check. For example:

```rust
        let s = screen(&c);
        charge(&mut c, rect_bytes(s, w, h))?;
```

  The call sites are `fill_rect`, `fill_circle`, `draw_text`, `draw_window_frame` (`rect_bytes(s, s.0, TITLE_BAR_H)`), `draw_window_border` (`2 * (s.0 as u64 + s.1 as u64) * BYTES_PER_PX`), `clear_user_area` (`rect_bytes(s, s.0, s.1)`), `overlay_clear` (`rect_bytes(s, s.0, s.1)`), `overlay_fill_rect` and `repaint_region`. `grep -n "charge(&mut c" v3/crates/acid-wasm/src/abi.rs` lists them.
- In the internal test at the bottom (`use super::{circle_bytes, ...}`), pass `W` as the first argument everywhere, with `const W: (i32, i32) = (640, 360);` at the top of that test, and add:

```rust
        let svga = (800, 600);
        assert_eq!(rect_bytes(svga, i32::MAX, i32::MAX), 800 * 600 * 2, "a full fill costs more on a bigger screen");
        assert_eq!(circle_bytes(svga, i32::MAX), 800 * 600 * 2);
        assert_eq!(text_bytes(svga, i32::MAX), 800 * 600 * 2);
```

- [ ] **Step 10: `acid-os` boots at a given size**

In `v3/crates/acid-os/src/lib.rs`:
- change `use acid_kernel::layout::SCREEN_W;` to `use acid_kernel::layout::Screen;`;
- change the `DESKTOP_H` doc comment to `/// The desktop window's height: it spawns screen-wide and 204 tall, which is desktop.lua's TOTAL_H (strip plus the open dropdown's room).`;
- replace `pub fn boot` with:

```rust
/// Boots at the default screen size.
pub fn boot(platform: Arc<dyn Platform>) -> Arc<Kernel> {
    boot_with(platform, Screen::DEFAULT)
}

pub fn boot_with(platform: Arc<dyn Platform>, screen: Screen) -> Arc<Kernel> {
    let kernel = Kernel::with_screen(platform, screen);
```

  The old body continues unchanged, except `w: SCREEN_W,` becomes `w: screen.w,`;
- in `spawn_from_manifest`: `let (x, y) = cascade_position(kernel.screen(), kernel.with_state(|st| st.windows.count()), w, h);`.

- [ ] **Step 11: Goldens keep v2's size**

In `v3/crates/acid-os/tests/golden.rs`:
- import `use acid_kernel::layout::Screen;` and `boot_with` from `acid_os`;
- `hello_acid_matches_golden_pixel_for_pixel` and `overlapping_windows_and_overlay_match_golden`: `Kernel::with_screen(p.clone(), Screen::WIDE)`;
- `desktop_strip_matches_golden_pixel_for_pixel` and `menu_dropdown_matches_golden_pixel_for_pixel`: `boot_with(p.clone(), Screen::WIDE)`;
- in `assert_matches_golden_masked`, drop `assert_eq!((w, h), (640, 360));`. The existing `actual.len()` check covers the size;
- leave `boot_starts_only_the_desktop` on `boot()`; it still expects 640 wide, which is the default width. Add:

```rust
#[test]
fn boot_with_spawns_the_desktop_screen_wide() {
    let p = FakePlatform::new(FakePlatform::repo_root());
    let k = boot_with(p.clone(), Screen::SVGA);
    assert_eq!(k.screen(), Screen::SVGA);
    let wins = k.with_state(|st| st.windows.in_z_order().iter().map(|w| (w.x, w.y, w.w, w.h)).collect::<Vec<_>>());
    assert_eq!(wins, [(0, 0, 800, 204)]);
}
```

- [ ] **Step 12: Build and run everything**

Run: `cargo test --manifest-path v3/Cargo.toml --workspace -q 2>&1 | grep -E "^test result|FAILED|panicked|error"`
Expected: every `test result: ok`. Fix any remaining compile errors where a removed constant is still used: `grep -rn "SCREEN_W\|SCREEN_H" v3/crates --include='*.rs'` should show nothing outside comments. For a test that fails only on 640×360 geometry, apply the Global Constraints rule (switch it to `WIDE`). The likely candidates are in `acid-api` (`spawn_path_on`), `acid-os/tests/games.rs` and `acid-os/tests/wasm.rs`.

- [ ] **Step 13: Commit**

```bash
git add v3/crates/acid-kernel/src v3/crates/acid-api/src/lib.rs v3/crates/acid-lua/tests/lua_app.rs v3/crates/acid-wasm/src/abi.rs v3/crates/acid-wasm/tests/abi.rs v3/crates/acid-os/src/lib.rs v3/crates/acid-os/tests
git commit -m "Kernel owns the screen size; 640x480 by default

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 3: Apps can ask the screen size

**Files:**
- Modify: `v3/crates/acid-lua/src/lib.rs` (register `acid_screen_size`)
- Modify: `v3/crates/acid-lua/tests/lua_app.rs` (test)
- Modify: `v3/crates/acid-wasm/src/abi.rs` (`screen_w`, `screen_h` imports)
- Modify: `v3/crates/acid-wasm/src/lib.rs` (`IMPORT_NAMES`)
- Modify: `v3/crates/acid-wasm/tests/abi.rs` (import count 56 → 58, new test)
- Modify: `v3/carts-src/acid-cart/src/lib.rs` (guest bindings)
- Modify: `docs/manual-v3/09-api-reference.md` (entry and index)
- Modify: `docs/manual-v3/10-wasm-carts.md` (import table rows)

**Interfaces:**
- Consumes: `AcidApi::screen_size(&self) -> (i32, i32)` (Task 2).
- Produces:
  - Lua global `acid_screen_size()` → `w, h`;
  - wasm imports `"acid" "screen_w" (result i32)` and `"acid" "screen_h" (result i32)`;
  - guest fn `acid_cart::screen_size() -> (i32, i32)`.

- [ ] **Step 1: Write the failing tests**

`v3/crates/acid-lua/tests/lua_app.rs`:

```rust
#[test]
fn screen_size_returns_width_and_height() {
    let api = FakeApi::with_events(vec![]);
    let lua = state(api.clone());
    run(&lua, r#"
        local w, h = acid_screen_size()
        acid_draw_text(w .. "x" .. h, 0, 0, 0, 0)
    "#);
    assert_eq!(api.texts(), ["640x360"]);
}
```

`v3/crates/acid-wasm/tests/abi.rs`:

```rust
#[test]
fn screen_size_imports() {
    let imports = r#"
  (import "acid" "screen_w" (func $sw (result i32)))
  (import "acid" "screen_h" (func $sh (result i32)))"#;
    let log = run_api(imports, "", "call $sw call $say call $sh call $say");
    assert_eq!(log, exp![said(640), said(360)]);
}
```

In `import_names_match_the_linker`, change both `56`s to `58`.

- [ ] **Step 2: Run them and confirm they fail**

Run: `cargo test --manifest-path v3/Cargo.toml -p acid-lua -p acid-wasm -q 2>&1 | grep -E "FAILED|panicked|test result"`
Expected: `screen_size_returns_width_and_height`, `screen_size_imports` and `import_names_match_the_linker` fail.

- [ ] **Step 3: Implement**

`v3/crates/acid-lua/src/lib.rs`, right after the `acid_window_max` registration:

```rust
    let a = api.clone();
    g.set("acid_screen_size", lua.create_function(move |_, ()| Ok(a.screen_size()))?)?;
```

`v3/crates/acid-wasm/src/abi.rs`, right after the `window_max` `func_wrap`:

```rust
    linker.func_wrap(MODULE, "screen_w", |c: Caller<'_, Host>| c.data().api.screen_size().0)?;
    linker.func_wrap(MODULE, "screen_h", |c: Caller<'_, Host>| c.data().api.screen_size().1)?;
```

`v3/crates/acid-wasm/src/lib.rs`, in `IMPORT_NAMES` right after `"window_max",`:

```rust
    "screen_w",
    "screen_h",
```

`v3/carts-src/acid-cart/src/lib.rs`: in the `sys` extern block after `pub fn window_max() -> i32;`:

```rust
        pub fn screen_w() -> i32;
        pub fn screen_h() -> i32;
```

and after the `pub fn window_max()` wrapper:

```rust
/// The screen's size in pixels, `(w, h)`; fixed for the whole run.
pub fn screen_size() -> (i32, i32) {
    unsafe { (sys::screen_w(), sys::screen_h()) }
}
```

- [ ] **Step 4: Document them**

`docs/manual-v3/09-api-reference.md`: add an entry in alphabetical position (after `acid_repaint_region`, before `acid_send_self_to_back`). It uses the same shape as the other entries:

~~~~markdown
### `acid_screen_size`

```lua snippet
local w, h = acid_screen_size()   -- 640, 480 by default
```

The screen's size in pixels. Acid OS picks it at startup (640×480, 640×360
or 800×600), and it doesn't change while it runs, so it's safe to read once
and keep.

---
~~~~

Also add `acid_screen_size` to the "Index by area" list next to the other window/screen calls (where `acid_window_max` is listed).

`docs/manual-v3/10-wasm-carts.md`: in the import table, right after the `window_max` row:

```markdown
| `screen_w` | → i32 | screen width in pixels | [`acid_screen_size`](09-api-reference.md#acid_screen_size) |
| `screen_h` | → i32 | screen height in pixels | [`acid_screen_size`](09-api-reference.md#acid_screen_size) |
```

- [ ] **Step 5: Run everything**

Run: `cargo test --manifest-path v3/Cargo.toml --workspace -q 2>&1 | grep -E "^test result|FAILED|panicked"`
Expected: all ok. `acid-os/tests/manual.rs` checks that the import table matches `IMPORT_NAMES` and that every `acid_*` name in the manual exists.

Run: `cargo build --manifest-path v3/carts-src/Cargo.toml --release --target wasm32-unknown-unknown -q`
Expected: builds (the guest crate compiles with the new bindings).

- [ ] **Step 6: Commit**

```bash
git add v3/crates/acid-lua/src/lib.rs v3/crates/acid-lua/tests/lua_app.rs v3/crates/acid-wasm/src/abi.rs v3/crates/acid-wasm/src/lib.rs v3/crates/acid-wasm/tests/abi.rs v3/carts-src/acid-cart/src/lib.rs docs/manual-v3/09-api-reference.md docs/manual-v3/10-wasm-carts.md
git commit -m "Add acid_screen_size and the screen_w/screen_h cart imports

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 4: Built-in Lua apps use the real screen size

**Files:**
- Modify: `v3/apps/desktop.lua`, `v3/apps/lib/acid_eggs.lua`, `v3/apps/cart/cartfile.lua`, `v3/apps/editor/layout.lua` (comment only)
- Modify: `v3/tools/game_test_env.lua` (stub)
- Create: `v3/tools/screen_800x600.lua`, `v3/tools/test_screen_svga.lua`
- Modify: `v3/crates/acid-lua/tests/game_tests.rs` (new suite)
- Modify: `v3/crates/acid-os/tests/golden.rs` (desktop clock at 800 wide)

**Interfaces:**
- Consumes: Lua `acid_screen_size()` (Task 3); `acid_os::boot_with` and `Screen` (Task 2).
- Produces: `DesktopApp.SCREEN_W`, `AcidEggs.SCREEN_W/SCREEN_H` and `Cartfile.MAX_W/MAX_H` follow the screen.

- [ ] **Step 1: Give the Lua test env a screen**

In `v3/tools/game_test_env.lua`, after `local function push...`:

```lua
-- The screen size apps see. The suites' expectations were written for
-- v2's 640x360; a suite can load a file setting SCREEN_W/SCREEN_H before
-- this one to run at another size (screen_800x600.lua).
SCREEN_W = SCREEN_W or 640
SCREEN_H = SCREEN_H or 360
function acid_screen_size() return SCREEN_W, SCREEN_H end
```

Create `v3/tools/screen_800x600.lua`:

```lua
-- Loaded before game_test_env.lua: the apps under test see an 800x600 screen.
SCREEN_W, SCREEN_H = 800, 600
```

- [ ] **Step 2: Write the failing suite**

Create `v3/tools/test_screen_svga.lua`:

```lua
-- The built-in apps size themselves from acid_screen_size(), not 640x360.
eq(DesktopApp.SCREEN_W, 800, "the desktop spans the screen")
eq(DesktopApp.MAX_TASKBAR_SLOTS, 10, "(800 - 60 - 90) // 60 window buttons fit")
eq(Cartfile.MAX_W, 800, "a cart window may be as wide as the screen")
eq(Cartfile.MAX_H, 576, "and as tall as the screen less the desktop strip")
eq({ AcidEggs.SCREEN_W, AcidEggs.SCREEN_H }, { 800, 600 }, "the eggs fly across the whole screen")
```

In `v3/crates/acid-lua/tests/game_tests.rs` add:

```rust
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
```

In `v3/crates/acid-os/tests/golden.rs` add:

```rust
#[test]
fn desktop_draws_its_clock_at_the_right_edge_of_a_wider_screen() {
    let p = FakePlatform::new(FakePlatform::repo_root());
    let k = boot_with(p.clone(), Screen::SVGA);
    let text = Some(rgb565(0xD4E6DB));
    let start = std::time::Instant::now();
    loop {
        let clock_at_800 = k.with_state(|st| {
            st.windows.in_z_order().iter().find(|w| w.app_name == DESKTOP_PATH).is_some_and(|w| {
                let c = w.canvas.lock();
                (730..796).any(|x| (8..16).any(|y| c.pixel(x, y) == text))
            })
        });
        if clock_at_800 { break; }
        assert!(start.elapsed() < Duration::from_secs(10), "desktop never drew its clock at the 800 px edge");
        std::thread::sleep(Duration::from_millis(10));
    }
}
```

- [ ] **Step 3: Run them and confirm they fail**

Run: `cargo test --manifest-path v3/Cargo.toml -p acid-lua --test game_tests apps_follow -q; cargo test --manifest-path v3/Cargo.toml -p acid-os --test golden wider_screen -q`
Expected: the Lua suite fails on `SCREEN_W` (640 ~= 800), and the golden test times out ("never drew its clock at the 800 px edge").

- [ ] **Step 4: Implement**

`v3/apps/desktop.lua`: replace the comment block above `DesktopApp.SCREEN_W = 640` and the line itself with:

```lua
-- The screen's width: the boot code spawns this window that wide.
DesktopApp.SCREEN_W = (acid_screen_size())
```

Also reword the two comments that mention 640:
- The dropdown comment ("...never needed the other 480+ px of a 640px-wide screen...") becomes "...never needed most of the screen's width...".
- The `TOTAL_H` comment ending "Must match the boot code's own kernel_spawn_app(MY_APP_NAME, 0, 0, 640, TOTAL_H, 0) call, synced by comment on both sides, same as SCREEN_W above." becomes "Must match DESKTOP_H in the boot code (crates/acid-os/src/lib.rs), synced by comment on both sides."

`v3/apps/lib/acid_eggs.lua`: replace the two `SCREEN_W`/`SCREEN_H` lines with:

```lua
AcidEggs.SCREEN_W, AcidEggs.SCREEN_H = acid_screen_size()
```

`v3/apps/cart/cartfile.lua`: replace the window-bounds comment's first line and the `MAX_W`/`MAX_H` lines:

```lua
-- Window bounds: the screen less the desktop strip. A cart
-- asking for something outside this gets clamped rather than refused -- a
-- bad number is a typo, not an attack, but an unclamped one is a window
-- nobody can reach the title bar of.
local screen_w, screen_h = acid_screen_size()
Cartfile.MIN_W = 80
Cartfile.MAX_W = screen_w
Cartfile.MIN_H = 48
Cartfile.MAX_H = screen_h - 24
```

`v3/apps/editor/layout.lua`: change "The screen is 640x360, so two of these still fit side by side." to "The smallest screen is 640x360, so it fits at every screen size."

- [ ] **Step 5: Run everything**

Run: `cargo test --manifest-path v3/Cargo.toml --workspace -q 2>&1 | grep -E "^test result|FAILED|panicked"`
Expected: all ok. The existing desktop, cart, eggs and terminal suites see 640×360 from the stub, so their pinned counts and values don't change.

- [ ] **Step 6: Commit**

```bash
git add v3/apps/desktop.lua v3/apps/lib/acid_eggs.lua v3/apps/cart/cartfile.lua v3/apps/editor/layout.lua v3/tools/game_test_env.lua v3/tools/screen_800x600.lua v3/tools/test_screen_svga.lua v3/crates/acid-lua/tests/game_tests.rs v3/crates/acid-os/tests/golden.rs
git commit -m "Desktop, eggs and cart limits follow the screen size

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 5: The startup picker (pure logic)

**Files:**
- Modify: `v3/crates/acid-hosted/Cargo.toml` (add `acid-kernel`)
- Create: `v3/crates/acid-hosted/src/picker.rs`
- Modify: `v3/crates/acid-hosted/src/lib.rs` (`pub mod picker;`)

**Interfaces:**
- Consumes: `acid_kernel::layout::Screen` (Task 2); `acid_kernel::theme::*`; `acid_gfx::Canvas` with `draw_text(x, y, text, fg, bg)` and `fill_rect`.
- Produces: `acid_hosted::picker::{Picker, PickerKey, COUNTDOWN_MS, PICKER_SCREEN}`:
  - `Picker::new() -> Picker`
  - `tick(&mut self, elapsed_ms: u32) -> Option<Screen>`
  - `key(&mut self, k: PickerKey) -> Option<Screen>`
  - `click(&mut self, x: i32, y: i32) -> Option<Screen>`
  - `hover(&mut self, x: i32, y: i32)`
  - `draw(&self, c: &mut Canvas)`
  - `PickerKey { Up, Down, Enter, Other }`
  - `PICKER_SCREEN: Screen = Screen::DEFAULT` (the canvas and window size while picking)

- [ ] **Step 1: Add the dependency**

In `v3/crates/acid-hosted/Cargo.toml` `[dependencies]`, add `acid-kernel = { workspace = true }`. Then add `pub mod picker;` to `v3/crates/acid-hosted/src/lib.rs` after `pub mod keymap;`.

- [ ] **Step 2: Write the module with failing tests first**

Create `v3/crates/acid-hosted/src/picker.rs` with the types stubbed (`todo!()` bodies) and these tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn row_centre(i: usize) -> (i32, i32) {
        (ROW_X + ROW_W / 2, ROW_Y + i as i32 * ROW_H + ROW_H / 2)
    }

    #[test]
    fn the_countdown_boots_the_default() {
        let mut p = Picker::new();
        assert_eq!(p.tick(COUNTDOWN_MS - 1), None);
        assert_eq!(p.tick(1), Some(Screen::DEFAULT));
    }

    #[test]
    fn a_key_stops_the_countdown() {
        let mut p = Picker::new();
        assert_eq!(p.key(PickerKey::Other), None);
        assert_eq!(p.tick(COUNTDOWN_MS * 10), None, "no timeout once stopped");
    }

    #[test]
    fn arrows_move_and_clamp_and_enter_picks() {
        let mut p = Picker::new();
        p.key(PickerKey::Up);
        assert_eq!(p.key(PickerKey::Enter), Some(Screen::PRESETS[0]), "Up at the top stays");
        let mut p = Picker::new();
        for _ in 0..5 {
            p.key(PickerKey::Down);
        }
        assert_eq!(p.key(PickerKey::Enter), Some(Screen::PRESETS[2]), "Down at the bottom stays");
    }

    #[test]
    fn a_click_picks_the_row_under_it() {
        let mut p = Picker::new();
        let (x, y) = row_centre(1);
        assert_eq!(p.click(x, y), Some(Screen::WIDE));
        let mut p = Picker::new();
        assert_eq!(p.click(5, 5), None, "outside the rows");
        assert_eq!(p.tick(COUNTDOWN_MS), None, "any click stops the countdown");
    }

    #[test]
    fn hover_highlights_and_stops_the_countdown() {
        let mut p = Picker::new();
        let (x, y) = row_centre(2);
        p.hover(x, y);
        assert_eq!(p.tick(COUNTDOWN_MS), None);
        assert_eq!(p.key(PickerKey::Enter), Some(Screen::SVGA));
        let mut p = Picker::new();
        p.hover(5, 5);
        assert_eq!(p.tick(COUNTDOWN_MS), Some(Screen::DEFAULT), "hovering off the rows changes nothing");
    }

    #[test]
    fn draw_highlights_the_selected_row() {
        let p = Picker::new();
        let mut c = Canvas::new(PICKER_SCREEN.w, PICKER_SCREEN.h);
        p.draw(&mut c);
        let (x, y) = (ROW_X + 1, ROW_Y + 1);
        assert_eq!(c.pixel(x, y), Some(rgb565(THEME_PANEL)), "row 0 is selected");
        assert_eq!(c.pixel(x, y + ROW_H), Some(rgb565(THEME_BG)), "row 1 is not");
    }
}
```

Run: `cargo test --manifest-path v3/Cargo.toml -p acid-hosted picker -q`
Expected: the tests panic on `todo!()`.

- [ ] **Step 3: Implement**

The whole of `v3/crates/acid-hosted/src/picker.rs` above the tests:

```rust
//! The startup screen-size picker: the three preset sizes, a countdown to
//! the default, and keys or the mouse to choose. Pure logic and drawing
//! into a Canvas; window.rs feeds it events and shows its canvas.

use acid_gfx::{Canvas, rgb565};
use acid_kernel::layout::Screen;
use acid_kernel::theme::{THEME_BG, THEME_HARD, THEME_MUTED, THEME_PANEL, THEME_TEXT};

/// How long the picker waits before booting the default.
pub const COUNTDOWN_MS: u32 = 3000;
/// The window's size while picking.
pub const PICKER_SCREEN: Screen = Screen::DEFAULT;

/// The system font's glyph width (acid-gfx's 6 × 8 font).
const CHAR_W: i32 = 6;
const ROW_X: i32 = 220;
const ROW_W: i32 = 200;
const ROW_Y: i32 = 180;
const ROW_H: i32 = 28;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickerKey {
    Up,
    Down,
    Enter,
    /// Any other key: it only stops the countdown.
    Other,
}

pub struct Picker {
    selected: usize,
    remaining_ms: u32,
    counting: bool,
}

impl Default for Picker {
    fn default() -> Self {
        Self::new()
    }
}

impl Picker {
    pub fn new() -> Picker {
        Picker { selected: 0, remaining_ms: COUNTDOWN_MS, counting: true }
    }

    /// Advances the countdown; the default once it runs out.
    pub fn tick(&mut self, elapsed_ms: u32) -> Option<Screen> {
        if !self.counting {
            return None;
        }
        self.remaining_ms = self.remaining_ms.saturating_sub(elapsed_ms);
        (self.remaining_ms == 0).then_some(Screen::DEFAULT)
    }

    pub fn key(&mut self, k: PickerKey) -> Option<Screen> {
        self.counting = false;
        match k {
            PickerKey::Up => self.selected = self.selected.saturating_sub(1),
            PickerKey::Down => self.selected = (self.selected + 1).min(Screen::PRESETS.len() - 1),
            PickerKey::Enter => return Some(Screen::PRESETS[self.selected]),
            PickerKey::Other => {}
        }
        None
    }

    pub fn click(&mut self, x: i32, y: i32) -> Option<Screen> {
        self.counting = false;
        let i = row_at(x, y)?;
        self.selected = i;
        Some(Screen::PRESETS[i])
    }

    pub fn hover(&mut self, x: i32, y: i32) {
        if let Some(i) = row_at(x, y) {
            self.counting = false;
            self.selected = i;
        }
    }

    pub fn draw(&self, c: &mut Canvas) {
        c.fill_rect(0, 0, c.width(), c.height(), THEME_BG);
        centred(c, 140, "Acid OS v3 - screen size", THEME_HARD, THEME_BG);
        for (i, s) in Screen::PRESETS.iter().enumerate() {
            let y = ROW_Y + i as i32 * ROW_H;
            let (fg, bg) = if i == self.selected { (THEME_HARD, THEME_PANEL) } else { (THEME_TEXT, THEME_BG) };
            c.fill_rect(ROW_X, y, ROW_W, ROW_H, bg);
            let label = if *s == Screen::DEFAULT { format!("{}x{} (default)", s.w, s.h) } else { format!("{}x{}", s.w, s.h) };
            centred(c, y + (ROW_H - 8) / 2, &label, fg, bg);
        }
        let footer = if self.counting {
            let d = Screen::DEFAULT;
            format!("starting {}x{} in {}", d.w, d.h, self.remaining_ms.div_ceil(1000))
        } else {
            "Enter to start".to_string()
        };
        centred(c, ROW_Y + 3 * ROW_H + 20, &footer, THEME_MUTED, THEME_BG);
    }
}

/// The preset row under (x, y), if any.
fn row_at(x: i32, y: i32) -> Option<usize> {
    if !(ROW_X..ROW_X + ROW_W).contains(&x) || y < ROW_Y {
        return None;
    }
    let i = ((y - ROW_Y) / ROW_H) as usize;
    (i < Screen::PRESETS.len()).then_some(i)
}

/// Text centred across the canvas at row `y`.
fn centred(c: &mut Canvas, y: i32, text: &str, fg: u32, bg: u32) {
    let x = (c.width() - text.len() as i32 * CHAR_W) / 2;
    c.draw_text(x, y, text, fg, bg);
}
```

`acid-gfx`'s `font` module is private, hence the local `CHAR_W`. The tests get `rgb565`, `THEME_PANEL` and `THEME_BG` through `super::*`.

- [ ] **Step 4: Run them and confirm they pass**

Run: `cargo test --manifest-path v3/Cargo.toml -p acid-hosted -q`
Expected: all pass, including the six picker tests.

- [ ] **Step 5: Commit**

```bash
git add v3/crates/acid-hosted/Cargo.toml v3/crates/acid-hosted/src/lib.rs v3/crates/acid-hosted/src/picker.rs v3/Cargo.lock
git commit -m "Add the startup screen-size picker

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 6: Window integration, `--screen`, and boot after the pick

**Files:**
- Modify: `v3/crates/acid-hosted/src/window.rs`
- Modify: `v3/crates/acid-os/src/main.rs`
- Modify: `v3/crates/acid-os/src/lib.rs` (`screen_arg`)

**Interfaces:**
- Consumes: `Picker`, `PickerKey`, `PICKER_SCREEN` (Task 5); `Screen` (Task 2); `acid_os::boot_with` (Task 2).
- Produces:
  - `acid_hosted::window::run_window(event_loop, platform, preselected: Option<Screen>, boot: BootFn)`, where `pub type BootFn = Box<dyn FnOnce(Screen) -> Box<dyn std::any::Any>>`. The box `boot` returns is kept alive for the rest of the run, because the audio output handle must outlive `main`'s closure.
  - `acid_os::screen_arg(args: impl IntoIterator<Item = String>) -> Result<Option<Screen>, String>`.

- [ ] **Step 1: Write the failing `screen_arg` tests**

In the `tests` module of `v3/crates/acid-os/src/lib.rs` (`use super::{app_arg, screen_arg};` and `use acid_kernel::layout::Screen;`):

```rust
    #[test]
    fn screen_arg_reads_a_preset() {
        assert_eq!(screen_arg(args(&[])), Ok(None));
        assert_eq!(screen_arg(args(&["--screen", "800x600"])), Ok(Some(Screen::SVGA)));
        assert_eq!(screen_arg(args(&["--app", "tetris", "--screen", "640x360"])), Ok(Some(Screen::WIDE)));
    }

    #[test]
    fn screen_arg_rejects_other_sizes_and_lists_the_valid_ones() {
        for bad in [&["--screen", "1024x768"][..], &["--screen"][..]] {
            let e = screen_arg(args(bad)).unwrap_err();
            assert!(e.contains("640x480, 640x360, 800x600"), "{e}");
        }
    }
```

Run: `cargo test --manifest-path v3/Cargo.toml -p acid-os --lib -q`
Expected: compile error, `screen_arg` not found.

- [ ] **Step 2: Implement `screen_arg`**

In `v3/crates/acid-os/src/lib.rs`, after `app_arg`:

```rust
/// The size after `--screen` on the command line, if any. Only the presets
/// are accepted; anything else is an error naming them.
pub fn screen_arg(args: impl IntoIterator<Item = String>) -> Result<Option<Screen>, String> {
    let valid = Screen::PRESETS.iter().map(|s| format!("{}x{}", s.w, s.h)).collect::<Vec<_>>().join(", ");
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        if a == "--screen" {
            let Some(v) = it.next() else {
                return Err(format!("--screen needs a size: {valid}"));
            };
            return Screen::parse(&v).map(Some).ok_or_else(|| format!("--screen {v}: not a supported size; use one of {valid}"));
        }
    }
    Ok(None)
}
```

Run: `cargo test --manifest-path v3/Cargo.toml -p acid-os --lib -q`
Expected: pass.

- [ ] **Step 3: Picker and boot in the window**

Rewrite `v3/crates/acid-hosted/src/window.rs`. Keep `translate_key` handling and the redraw guard exactly as they are. The module doc becomes:

```rust
//! The host window: 1:1 pixels, not resizable by the user. It opens on the
//! screen-size picker (or straight at a size given on the command line),
//! then resizes to the chosen size and boots the OS there. Mouse = touch,
//! keys go through keymap::translate_key.
```

The structure:

```rust
use std::any::Any;
use std::time::{Duration, Instant};

use acid_gfx::{Canvas, rgb565_to_888};
use acid_kernel::layout::Screen;
use winit::event_loop::ControlFlow;
use winit::keyboard::KeyCode;

use crate::picker::{PICKER_SCREEN, Picker, PickerKey};

/// Boots the OS at the chosen size. What it returns is kept for the rest
/// of the run (the audio output must outlive the closure).
pub type BootFn = Box<dyn FnOnce(Screen) -> Box<dyn Any>>;

/// How often the countdown redraws.
const PICKER_TICK: Duration = Duration::from_millis(100);

enum Stage {
    Picking { picker: Picker, canvas: Canvas, last_tick: Instant, cursor: (i32, i32) },
    Running,
}

struct App {
    platform: Arc<HostedPlatform>,
    window: Option<Rc<Window>>,
    surface: Option<softbuffer::Surface<Rc<Window>, Rc<Window>>>,
    shift: bool,
    /// The surface's size: the picker's until a size is chosen.
    size: Screen,
    stage: Stage,
    boot: Option<BootFn>,
    keep_alive: Option<Box<dyn Any>>,
    warned_size: bool,
}
```

Behaviour:

1. **`run_window(event_loop, platform, preselected, boot)`**
   - With `Some(s)`: start with `size: s`, `stage: Stage::Running`, and call `boot(s)` **before** `event_loop.run_app` (same as today, where the kernel boots before the window exists). Store the result in `keep_alive` and set `boot: None`.
   - With `None`: start with `size: PICKER_SCREEN`, `stage: Stage::Picking { picker: Picker::new(), canvas: Canvas::new(PICKER_SCREEN.w, PICKER_SCREEN.h), last_tick: Instant::now(), cursor: (0, 0) }`, and `boot: Some(boot)`.
2. **`resumed`:** as today, but use `self.size` instead of `W`/`H` for `with_inner_size` and `surface.resize`. Delete the `W`/`H` constants.
3. **`RedrawRequested`:**
   - While `Picking`: `picker.draw(canvas)`, then copy `canvas.pixels()` through `rgb565_to_888` into the surface buffer (lengths match: both are `PICKER_SCREEN`) and `present()`.
   - While `Running`: today's code.
4. **Input while `Picking`:** events go to the picker, not `platform.input`.
   - `CursorMoved` stores `cursor` and calls `picker.hover`, then `request_redraw`.
   - A left `MouseInput` press calls `picker.click(cursor)`; `Some(s)` means `self.choose(s)`.
   - `KeyboardInput` press (not repeat) maps `ArrowUp`→`Up`, `ArrowDown`→`Down`, `Enter`/`NumpadEnter`→`Enter`, and any other key→`Other`. On `Some(s)` it calls `choose(s)`, and otherwise `request_redraw`.
   - `ModifiersChanged` and `CloseRequested` behave as today.
5. **`about_to_wait`** (new `ApplicationHandler` method):
   - While `Picking`: `let elapsed = last_tick.elapsed(); last_tick = Instant::now();`, then `picker.tick(elapsed.as_millis() as u32)`. On `Some(s)` call `choose(s)`; otherwise `request_redraw()` and `event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + PICKER_TICK))`.
   - While `Running`: `event_loop.set_control_flow(ControlFlow::Wait)`.
6. **`choose(&mut self, s: Screen)`:**

```rust
    fn choose(&mut self, s: Screen) {
        let size = PhysicalSize::new(s.w as u32, s.h as u32);
        if let Some(w) = &self.window {
            // Min = max = the size: a hint to window managers that would
            // otherwise ignore a resize of a non-resizable window.
            w.set_min_inner_size(Some(size));
            w.set_max_inner_size(Some(size));
            let _ = w.request_inner_size(size);
        }
        if let Some(surface) = &mut self.surface {
            surface
                .resize(NonZeroU32::new(s.w as u32).unwrap(), NonZeroU32::new(s.h as u32).unwrap())
                .expect("size surface");
        }
        self.size = s;
        self.stage = Stage::Running;
        if let Some(boot) = self.boot.take() {
            self.keep_alive = Some(boot(s));
        }
    }
```

7. **`WindowEvent::Resized(new)`** while `Running`: if `(new.width, new.height) != (self.size.w as u32, self.size.h as u32)` and `!self.warned_size`, print `eprintln!("Acid OS v3: the window manager kept the window at {}x{}, not {}x{}; the screen may stay blank", new.width, new.height, self.size.w, self.size.h);` and set `warned_size = true`. Ignore `Resized` while `Picking`.

- [ ] **Step 4: `main.rs` boots inside the callback**

Replace the body of `main` in `v3/crates/acid-os/src/main.rs`:

```rust
fn main() {
    let preselected = match acid_os::screen_arg(std::env::args().skip(1)) {
        Ok(s) => s,
        Err(msg) => {
            eprintln!("Acid OS v3: {msg}");
            std::process::exit(2);
        }
    };
    let event_loop = EventLoop::<UserEvent>::with_user_event().build().expect("event loop");
    let platform = HostedPlatform::new(".");
    platform.set_proxy(event_loop.create_proxy());
    let boot_platform = platform.clone();
    // Nothing kernel-side exists until a screen size is chosen.
    let boot: BootFn = Box::new(move |screen| {
        let kernel = acid_os::boot_with(boot_platform, screen);
        // Development launcher: `-- --app tetris` opens an app from its
        // manifest, the way Terminal's `run` does (spec 10.5).
        if let Some(name) = acid_os::app_arg(std::env::args().skip(1)) {
            match acid_os::spawn_from_manifest(&kernel, &name) {
                Some(task) => kernel.activate_window(task),
                None => eprintln!("Acid OS v3: --app {name}: no such app"),
            }
        }
        // Sound: the device's callback pulls straight from the kernel synth.
        // The handle is !Send; the window keeps it for the rest of the run.
        let audio_kernel = kernel.clone();
        let audio = acid_hosted::audio::start_output(std::sync::Arc::new(move |buf: &mut [u8]| {
            audio_kernel.render_audio(buf)
        }));
        std::thread::Builder::new()
            .name("router".into())
            .spawn(move || kernel.run_router())
            .expect("router thread");
        Box::new(audio)
    });
    run_window(event_loop, platform, preselected, boot);
}
```

Update the imports: `use acid_hosted::window::{BootFn, run_window};`. Update the module doc to mention `-- --screen 800x600` next to the run command.

- [ ] **Step 5: Build, test, smoke-run**

Run: `cargo build --release --manifest-path v3/Cargo.toml -p acid-os 2>&1 | tail -3`
Expected: builds with no warnings from the changed files.

Run: `cargo test --manifest-path v3/Cargo.toml --workspace -q 2>&1 | grep -E "^test result|FAILED|panicked"`
Expected: all ok.

Run: `./v3/target/release/acid-os --screen 1024x768; echo "exit=$?"`
Expected: `Acid OS v3: --screen 1024x768: not a supported size; use one of 640x480, 640x360, 800x600` and `exit=2`.

The interactive check (picker appears, countdown, each size) is the user's, at the end of Task 7. Don't claim it works visually.

- [ ] **Step 6: Commit**

```bash
git add v3/crates/acid-hosted/src/window.rs v3/crates/acid-os/src/main.rs v3/crates/acid-os/src/lib.rs
git commit -m "Pick the screen size at startup, or with --screen

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 7: Goldens at the new sizes, and the docs

**Files:**
- Modify: `v3/crates/acid-os/tests/golden.rs`
- Create (after user approval): `v3/crates/acid-os/tests/golden/desktop_640x480.ppm`, `v3/crates/acid-os/tests/golden/desktop_800x600.ppm`
- Modify: `docs/manual-v3/01-getting-started.md`, `02-apps-and-manifests.md`, `04-graphics.md`, `07-system-apis.md`, `09-api-reference.md`, `10-wasm-carts.md`
- Modify: `README.md` (repo root), `v3/README.md`

**Interfaces:**
- Consumes: `boot_with`, `Screen` (Task 2), the desktop following the width (Task 4).
- Produces: two committed golden frames; docs that describe the selectable size.

- [ ] **Step 1: One desktop-capture helper, three sizes**

In `v3/crates/acid-os/tests/golden.rs`, factor the wait-and-capture out of `desktop_strip_matches_golden_pixel_for_pixel` into:

```rust
/// Boots at `screen`, waits until the desktop has drawn its clock and Menu
/// label, and returns the composited frame.
fn desktop_frame(screen: Screen) -> Vec<u16> {
    let p = FakePlatform::new(FakePlatform::repo_root());
    let k = boot_with(p.clone(), screen);
    let text = Some(rgb565(0xD4E6DB));
    let start = std::time::Instant::now();
    loop {
        let drawn = k.with_state(|st| {
            st.windows.in_z_order().iter().find(|w| w.app_name == DESKTOP_PATH).is_some_and(|w| {
                let c = w.canvas.lock();
                (screen.w - 70..screen.w - 4).any(|x| (8..16).any(|y| c.pixel(x, y) == text))
                    && (5..30).any(|x| (7..16).any(|y| c.pixel(x, y) == text))
            })
        });
        if drawn { break; }
        assert!(start.elapsed() < Duration::from_secs(10), "desktop never drew its clock");
        std::thread::sleep(Duration::from_millis(10));
    }
    k.composite_frame();
    p.display.last_frame().unwrap()
}

/// The clock shows the time of capture, so its reserved CLOCK_W area is masked.
fn clock_mask(screen: Screen) -> Option<(usize, usize, usize, usize)> {
    Some(((screen.w - 90) as usize, 0, 90, 24))
}
```

`desktop_strip_matches_golden_pixel_for_pixel` becomes `assert_matches_golden_masked(&desktop_frame(Screen::WIDE), "desktop.ppm", clock_mask(Screen::WIDE));`. The test from Task 4 (`desktop_draws_its_clock_at_the_right_edge_of_a_wider_screen`) is now covered by `desktop_frame(Screen::SVGA)`, so delete it. Add:

```rust
#[test]
fn desktop_at_640x480_matches_golden() {
    assert_matches_golden_masked(&desktop_frame(Screen::DEFAULT), "desktop_640x480.ppm", clock_mask(Screen::DEFAULT));
}

#[test]
fn desktop_at_800x600_matches_golden() {
    assert_matches_golden_masked(&desktop_frame(Screen::SVGA), "desktop_800x600.ppm", clock_mask(Screen::SVGA));
}
```

In `assert_matches_golden_masked`, handle a missing golden before loading it. It writes the actual frame and fails with instructions, and never creates the golden itself. The helper doesn't know the frame's width, so add a `w: usize` parameter to `assert_matches_golden_masked` and `assert_matches_golden`, and pass `640` from the four v2 goldens and `screen.w as usize` from the new ones. Height is `actual.len() / w`. Then:

```rust
    if !golden.exists() {
        let out = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../target/actual-{golden_name}"));
        write_ppm(&out, w, actual.len() / w, actual);
        panic!("no golden {golden_name} yet: review {} and, once approved, copy it to tests/golden/", out.display());
    }
```

- [ ] **Step 2: Produce the candidate frames**

Run: `cargo test --manifest-path v3/Cargo.toml -p acid-os --test golden desktop_at -q 2>&1 | grep "no golden"`
Expected: two panics naming `v3/target/actual-desktop_640x480.ppm` and `v3/target/actual-desktop_800x600.ppm`.

Run: `for s in 640x480 800x600; do magick v3/target/actual-desktop_$s.ppm v3/target/actual-desktop_$s.png; done`

- [ ] **Step 3: USER GATE: approve the frames**

**Stop here.** The controller (not a subagent) shows both PNGs to the user and asks for approval. They should show the desktop strip spanning the full width, the clock at the right edge, and the wallpaper filling the screen with no bars. Do not copy the goldens without an explicit yes. If the user rejects them, report what they said and stop.

- [ ] **Step 4: Commit the approved goldens**

```bash
cp v3/target/actual-desktop_640x480.ppm v3/crates/acid-os/tests/golden/desktop_640x480.ppm
cp v3/target/actual-desktop_800x600.ppm v3/crates/acid-os/tests/golden/desktop_800x600.ppm
cargo test --manifest-path v3/Cargo.toml -p acid-os --test golden -q
```

Expected: all golden tests pass.

- [ ] **Step 5: Docs**

Make each of these edits; `grep -rn "640\|360" docs/manual-v3 README.md v3/README.md` lists every location.

- `01-getting-started.md:42`: "A 640×360 window titled..." becomes "A window titled "Acid OS v3" opens on the screen-size picker: 640×480 (the default), 640×360 or 800×600. Choose with the arrow keys and Enter or a click, or wait three seconds for the default. `-- --screen 800x600` skips the picker. Then the desktop appears. **Menu** is at the..." (keep the rest of the paragraph).
- `01-getting-started.md:68`: "a size outside 1 to 640 by 1 to 360" becomes "a size bigger than the screen".
- `01-getting-started.md:104`: "a 640×360 screen" becomes "a 640×480 screen (640×360 at the smallest)".
- `02-apps-and-manifests.md:53-54`: "1 to 640." becomes "1 to the screen width (640 by default)."; "1 to 360." becomes "1 to the screen height (480 by default)."
- `02-apps-and-manifests.md:69`: "The screen is **640×360**." becomes "The screen is **640×480** by default (640×360 and 800×600 are the other sizes; ask with [`acid_screen_size`](09-api-reference.md#acid_screen_size))." To fit every size, keep a window within 640×336 below the strip.
- `02-apps-and-manifests.md:242`: "220, clamped to 80–640" becomes "220, clamped to 80 to the screen width".
- `04-graphics.md:18`: as for 02:69.
- `04-graphics.md:359`: "on the 640×360 screen" becomes "on the screen".
- `04-graphics.md:398,498,505`: the example's literal `640` becomes a local `SCREEN_W`, set once with `local SCREEN_W = acid_screen_size()` at the top of that example. Keep the examples runnable: `manual.rs` runs them.
- `07-system-apis.md:150`: "isn't 1 to 640 wide and 1 to 360 high" becomes "is bigger than the screen".
- `09-api-reference.md:463-464, 744-745`: the "outside 1 to 640 / 1 to 360" wording becomes "bigger than the screen".
- `09-api-reference.md:583`: "on a 640×360 screen" is dropped.
- `09-api-reference.md:968`: the "Screen" row becomes `| Screen | 640 × 480 by default; 640 × 360 or 800 × 600 ([`acid_screen_size`](#acid_screen_size)) |`, and the section heading "Fixed geometry and limits" stays.
- `10-wasm-carts.md:243`: "clamped to the 640 × 360 screen" becomes "clamped to the screen". Add one sentence after that list: "Because charges are counted in pixels, a full-screen draw costs more on a bigger screen: 640×480 costs a third more than 640×360, and 800×600 a little over twice as much."
- `10-wasm-carts.md:248`: "a 640 × 16 title bar" becomes "a screen-wide, 16 px title bar".
- `README.md` (root, next to the run command at line 27) and `v3/README.md` (next to line 37): add "Acid OS opens on a screen-size picker; add `-- --screen 640x480` (or `640x360`, `800x600`) to skip it."

- [ ] **Step 6: Run everything**

Run: `cargo test --manifest-path v3/Cargo.toml --workspace -q 2>&1 | grep -E "^test result|FAILED|panicked"`
Expected: all ok (the manual tests re-run the edited examples).

- [ ] **Step 7: Commit**

```bash
git add v3/crates/acid-os/tests/golden.rs v3/crates/acid-os/tests/golden/desktop_640x480.ppm v3/crates/acid-os/tests/golden/desktop_800x600.ppm docs/manual-v3 README.md v3/README.md
git commit -m "Goldens at 640x480 and 800x600; document the screen sizes

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

- [ ] **Step 8: USER GATE: interactive check**

Ask the user to run `cargo run --release --manifest-path v3/Cargo.toml -p acid-os` and confirm:

- the picker appears and counts down;
- each size boots with the desktop spanning the width and the wallpaper filling the screen;
- `-- --screen 800x600` skips the picker.

Report their answer as-is.
