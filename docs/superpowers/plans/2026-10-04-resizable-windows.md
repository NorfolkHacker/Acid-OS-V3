# Resizable Windows Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A window that opts in gets a bottom-right grip. Dragging the grip shows an outline of the new size; releasing it resizes the window once, and the app re-lays out with its state kept.

**Architecture:**
- A `Resized { w, h }` event flows from the kernel through the API to Lua (`"resized", w, h`) and to wasm carts (`EV_RESIZED` = 5). `AcidApp` handles it by calling `on_resize` and then redrawing.
- The kernel reads `resizable`/`min_w`/`min_h` from the app's manifest in its existing single read, draws a grip on resizable windows, and resizes a window by swapping in a new canvas.
- The router runs the grip gesture (`KernelState` now knows the screen size), and the compositor draws the outline.
- The six text apps move their load-time layout into a `layout()` method that runs again on resize.

**Tech Stack:** Rust 2024 workspace (`v3/`), mlua 0.10 (Lua 5.4), wasmi 2.

**Spec:** `docs/superpowers/specs/2026-10-04-resizable-windows-design.md`

## Global Constraints

- **Repo and branch:** `/home/norfolkh/acid-os-v3`, branch `resizable-windows` (it already has the spec commit). Run commands from the repo root.
- **Full suite:** `cargo test --manifest-path v3/Cargo.toml --workspace -q`. Baseline is 430 passing. Every task ends with the whole suite green and zero warnings (`cargo build --manifest-path v3/Cargo.toml --workspace 2>&1 | grep -c "^warning"` prints 0).
- **Constants** (in `acid-kernel` `layout.rs`): `RESIZE_GRIP = 8`, `RESIZE_MIN_W = 80`, `RESIZE_MIN_H = 48`. A window's minimum height is never below `TITLE_BAR_H + 8` = 24.
- **Grip pixels:** `THEME_HARD`, at grip-relative `(gx, gy)` for `gx, gy ∈ 1..=6` where `gx + gy ∈ {8, 10, 12}`. The grip's top-left is `(w − 8, h − 8)` in window coordinates.
- **Outline:** 1 px, `THEME_VIOLET`, drawn above everything while a resize gesture is held.
- **Wasm event kinds:** touch 1, key 2, moved 3, close 4, **resized 5**. The cart gets `acid_on_event(5, w, h, 0)` followed by `acid_redraw()`.
- **Opted-in apps** (`resizable = true`, `min_w`, `min_h`):
  - Terminal 160×80;
  - Editor 200×100;
  - File Manager 120×80;
  - System Monitor 160×120;
  - About 120×60;
  - Network 140×80.
- **Never opted in:** desktop, Config, games, Load Cart, hello_acid.
- **Unchanged at the opening size:** every app's layout values, every existing test's expected values, and the existing golden frames.
- **Lock order:** kernel state lock first, then a window's canvas lock. Never the reverse.
- **Commits:** path-only `git add`. Messages end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- **No-std crates:** `acid-kernel` and `acid-api` are no_std + alloc. Signed `i32::div_ceil` is unstable.
- **Test discipline:** each new Lua suite must fail against the app's pre-change code. Prove it with `git show <base>:<path>` and record that in the report.

---

### Task 1: The Resized event, end to end

**Files:**
- Modify: `v3/crates/acid-kernel/src/event.rs`
- Modify: `v3/crates/acid-api/src/lib.rs`: `PolledEvent`, `map`, and live sizes for `window_size` and the chrome calls
- Modify: `v3/crates/acid-lua/src/lib.rs`: the poll mapping
- Modify: `v3/crates/acid-wasm/src/runner.rs`: `EV_RESIZED`
- Modify: `v3/carts-src/acid-cart/src/lib.rs`: guest `Event::Resized`
- Modify: `v3/apps/lib/acid_app.lua`, `v3/apps/lib/acid_game.lua`, `v3/tools/game_test_env.lua` (the stub `AcidApp:on_resize`)
- Modify: `docs/manual-v3/03-app-lifecycle.md`, `docs/manual-v3/10-wasm-carts.md`
- Test: `acid-api` tests, `v3/crates/acid-lua/tests/lua_app.rs`, `v3/crates/acid-wasm/tests/abi.rs`

**Interfaces:**
- Produces:
  - `Event::Resized { w: i32, h: i32 }` and `PolledEvent::Resized { w: i32, h: i32 }`;
  - Lua `acid_poll_event` returns `"resized", w, h`;
  - `AcidApp:on_resize(w, h)`, empty by default; the loop calls it, then `redraw`, then `acid_notify_redraw_done`;
  - `AcidGame` calls `on_resize` but doesn't redraw;
  - wasm: `on_event(5, w, h, 0)`, then `redraw`;
  - `KernelApi::window_size` and the chrome calls read the canvas's live size.

- [ ] **Step 1: Failing tests**
  - **acid-api:** in the existing test module (reuse the `spawn` helper), send `Event::Resized { w: 300, h: 200 }` on the app's queue, then assert `api.poll_event(100) == Some(PolledEvent::Resized { w: 300, h: 200 })`. Mirror the existing `Moved` test there.
  - **acid-api, live size:** spawn at 100×80. Replace the canvas contents with `*ctx.canvas.lock() = Canvas::new(150, 90)`; get `ctx` through the existing `api.context()`. Then assert:
    - `api.window_size() == (150, 90)`;
    - after `api.draw_window_border()`, the canvas pixel at (149, 45) is `rgb565(THEME_HARD)`.
  - **lua_app.rs:** use `FakeApi::with_events(vec![Some(PolledEvent::Resized { w: 300, h: 200 })])` (check the fake's events type), then run:

    ```lua
    local k, w, h = acid_poll_event(0)
    acid_draw_text(k .. " " .. w .. "x" .. h, 0, 0, 0, 0)
    ```

    Expect `api.texts() == ["resized 300x200"]`.
  - **lua_app.rs, the AcidApp loop:** if lua_app.rs already has a test that runs a real `AcidApp` subclass through its loop (look for one that delivers `"moved"`), add the same kind of test for `"resized"`. It should check that `on_resize(300, 200)` is called before `redraw`. If no such test exists, add one: load `v3/apps/lib/acid_app.lua` into the state the way the real runner does, define an app whose `on_resize` and `redraw` record calls, and feed it Resized then Close.
  - **abi.rs (wasm):** mirror the existing moved-event test. Feed `Some(PolledEvent::Resized { w: 300, h: 200 })` and assert that the cart's `on_event` sees kind 5 with `(300, 200, 0)`, then that `redraw` is called. Use the same logging the moved test uses.

- [ ] **Step 2: Run them and confirm they fail (compile errors for the new variant)**

- [ ] **Step 3: Implement**
  - `event.rs`: add `/// The window was resized; the app re-lays out (resizable windows).` above `Resized { w: i32, h: i32 },`.
  - `acid-api`:
    - add `Resized { w: i32, h: i32 }` to `PolledEvent`, with a doc comment;
    - `map`: `Event::Resized { w, h } => PolledEvent::Resized { w, h }`;
    - add a private `fn live_size(&self) -> (i32, i32) { let c = self.ctx.canvas.lock(); (c.width(), c.height()) }`. Read it **before** `self.draw(..)` takes the lock again, because the mutex isn't re-entrant;
    - `draw_window_frame`, `draw_window_border`, `clear_user_area` and `window_size` use `live_size()`.
  - `acid-lua`: `Some(PolledEvent::Resized { w, h }) => ("resized", w, h).into_lua_multi(lua)`.
  - `acid-wasm` runner: `const EV_RESIZED: i32 = 5;` and the arm `Some(PolledEvent::Resized { w, h }) => { call(on_event, (EV_RESIZED, w, h, 0))?; call(redraw, ())?; }`.
  - Guest crate:
    - add `Event::Resized { w: i32, h: i32 }`, with a doc comment;
    - map kind `5 => Some(Event::Resized { w: a, h: b })`, using the decoder's own names for the first two event arguments;
    - read the `__ACID_MOVED` handling in the event macro. The host already calls `acid_redraw` after a resize, the same as after `moved`. If the macro does something special for `Moved`, such as suppressing a duplicate redraw, apply the same handling to `Resized`.
  - `acid_app.lua`:
    - add `function AcidApp:on_resize(w, h) end`, with a one-line comment;
    - in `start`, add `elseif kind == "resized" then self:on_resize(a, b); self:redraw(); acid_notify_redraw_done()`.
  - `acid_game.lua`: add `elseif kind == "resized" then self:on_resize(a, b)`.
  - `game_test_env.lua`: add `function AcidApp:on_resize(w, h) end` to the stub `AcidApp`.
  - Docs:
    - `10-wasm-carts.md`: add an event-kinds row `| 5 | resized | w, h, 0 | the host calls acid_redraw |`;
    - `03-app-lifecycle.md`: add an `### on_resize(w, h)` callback entry in §3.1, in the shape of its neighbours. It says: called when the user resized a resizable window, with the new size; re-run your layout here; `redraw` follows. Add `"resized"` to the event-loop description where `"moved"` is described.

- [ ] **Step 4: Run them and confirm they pass, then build the guest crate**

Run: `cargo build --manifest-path v3/carts-src/Cargo.toml --release --target wasm32-unknown-unknown -q`

- [ ] **Step 5: Run the full suite, check warnings, then commit**

```bash
git add v3/crates/acid-kernel/src/event.rs v3/crates/acid-api/src/lib.rs v3/crates/acid-lua/src/lib.rs v3/crates/acid-lua/tests/lua_app.rs v3/crates/acid-wasm/src/runner.rs v3/crates/acid-wasm/tests/abi.rs v3/carts-src/acid-cart/src/lib.rs v3/apps/lib/acid_app.lua v3/apps/lib/acid_game.lua v3/tools/game_test_env.lua docs/manual-v3/03-app-lifecycle.md docs/manual-v3/10-wasm-carts.md
git commit -m "A Resized event for apps and carts, and live window sizes

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 2: Kernel: opt-in, resizing a window, and the grip

**Files:**
- Modify: `v3/crates/acid-kernel/src/layout.rs` (constants)
- Modify: `v3/crates/acid-kernel/src/window.rs` (`resizable`, `min_w`, `min_h`)
- Modify: `v3/crates/acid-kernel/src/router.rs`: `KernelState::screen`, `KernelState::resize_window`, `resize_clamp`
- Modify: `v3/crates/acid-kernel/src/kernel.rs`: `ManifestFlags`, `manifest_flags`, `spawn_app`, `Kernel::resize_window`, grip drawing in `composite_frame`, `Kernel::with_screen` passing the screen to `KernelState`

**Interfaces:**
- Consumes: `Event::Resized` (Task 1).
- Produces:
  - `ManifestFlags { cart, scalable, resizable: bool, min_w: i32, min_h: i32 }`. `min_w`/`min_h` are 0 when absent;
  - `Window::{resizable, min_w, min_h}`;
  - `KernelState::with_screen(screen)` and `pub screen: Screen`;
  - `KernelState::resize_window(&mut self, task, w, h, dirty: &AtomicBool) -> Option<(i32, i32)>`, which clamps, applies, sends `Resized`, and returns the applied size, or `None` if nothing changed or the window isn't resizable;
  - `pub fn resize_clamp(win: &Window, screen: Screen, w: i32, h: i32) -> (i32, i32)`;
  - `Kernel::resize_window(&self, task, w, h) -> Option<(i32, i32)>`.

- [ ] **Step 1: Failing tests** (in `kernel.rs`'s test module, using `temp_tree` and the existing helpers)

```rust
    #[test]
    fn manifest_flags_reads_resizable_and_its_minimums() {
        let t = temp_tree("resize_flags");
        std::fs::create_dir_all(t.0.join("v3/apps")).unwrap();
        std::fs::write(t.0.join("v3/apps/r.app.toml"), "Resizable = 'true'\nmin_w = 120\nmin_h = 70\n").unwrap();
        std::fs::write(t.0.join("v3/apps/n.app.toml"), "resizable = yes\n").unwrap();
        std::fs::write(t.0.join("v3/apps/c.app.toml"), "resizable = true\nsource = cart\n").unwrap();
        let p = FakePlatform::new(t.0.clone());
        let r = manifest_flags(&*p, "v3/apps/r.lua");
        assert_eq!((r.resizable, r.min_w, r.min_h), (true, 120, 70));
        assert!(!manifest_flags(&*p, "v3/apps/n.lua").resizable, "only the exact value `true` counts");
        let c = manifest_flags(&*p, "v3/apps/c.lua");
        assert!(c.cart && c.resizable, "a cart may opt in");
    }

    #[test]
    fn a_resizable_window_resizes_keeping_its_pixels_and_tells_the_app() {
        let t = temp_tree("resize_apply");
        std::fs::create_dir_all(t.0.join("v3/apps")).unwrap();
        std::fs::write(t.0.join("v3/apps/r.app.toml"), "resizable = true\nmin_w = 100\nmin_h = 60\n").unwrap();
        let p = FakePlatform::new(t.0.clone());
        let k = Kernel::with_screen(p.clone(), crate::layout::Screen::DEFAULT);
        let (tx, rx) = mpsc::channel();
        k.set_runner(parked_runner(tx));
        let task = k.spawn_app(SpawnRequest { script_path: "v3/apps/r.lua".into(), ..req(40, 40, 200, 150) }).unwrap();
        let ctx = recv(&rx);
        ctx.canvas.lock().fill_rect(0, 0, 10, 10, 0xFF0000);
        assert_eq!(k.resize_window(task, 300, 200), Some((300, 200)));
        {
            let c = ctx.canvas.lock();
            assert_eq!((c.width(), c.height()), (300, 200), "the app's canvas handle sees the new size");
            assert_eq!(c.pixel(5, 5), Some(acid_gfx::rgb565(0xFF0000)), "old pixels kept top-left");
            assert_eq!(c.pixel(250, 180), Some(acid_gfx::rgb565(crate::theme::THEME_BG)), "the new area is background");
        }
        assert_eq!(k.with_state(|st| st.windows.by_task(task).map(|w| (w.w, w.h))), Some((300, 200)));
        assert_eq!(ctx.queue.try_recv(), Some(Event::Resized { w: 300, h: 200 }));
        assert_eq!(k.resize_window(task, 300, 200), None, "the same size changes nothing");
        assert_eq!(k.resize_window(task, 10, 10), Some((100, 60)), "clamped up to the minimum");
        assert_eq!(k.resize_window(task, 5000, 5000), Some((600, 440)), "clamped to the screen right and below the window");
    }

    #[test]
    fn a_window_without_the_opt_in_never_resizes_and_has_no_grip() {
        let (p, k, rx) = setup_at(crate::layout::Screen::DEFAULT);
        let task = k.spawn_app(req(40, 40, 200, 150)).unwrap();
        recv(&rx);
        assert_eq!(k.resize_window(task, 300, 200), None);
        k.composite_frame();
        let f = p.display.last_frame().unwrap();
        let grip = |gx: i32, gy: i32| f[((40 + 150 - 8 + gy) * 640 + (40 + 200 - 8 + gx)) as usize];
        assert_ne!(grip(6, 6), acid_gfx::rgb565(crate::theme::THEME_HARD));
    }

    #[test]
    fn a_resizable_window_shows_its_grip() {
        // Spawn a resizable app as in the test above (temp_tree with resizable = true),
        // composite, and check the grip pixels:
        // (6,6), (5,5), (4,6), (6,4), (2,6), (6,2) are THEME_HARD;
        // (1,1) and (3,3) are not.
    }
```

(Fill in `a_resizable_window_shows_its_grip` concretely, following the comments.) Check what `ctx.queue` and `try_recv` are called in this codebase (the router tests use `q.try_recv()`). Check the clamp: a window at (40, 40) on 640×480 can grow to 600×440.

- [ ] **Step 2: Run them and confirm they fail**

- [ ] **Step 3: Implement**
  - `layout.rs`, with doc comments:

```rust
/// The resize grip: a square this size in a resizable window's bottom-right corner.
pub const RESIZE_GRIP: i32 = 8;
/// A resizable window's minimum size when its manifest gives none.
pub const RESIZE_MIN_W: i32 = 80;
pub const RESIZE_MIN_H: i32 = 48;
```

  - `window.rs`: add `pub resizable: bool, pub min_w: i32, pub min_h: i32` to `Window`, default `false, 0, 0` in `new`.
  - `kernel.rs` `manifest_flags`:
    - `resizable: has("resizable", "true")`;
    - `min_w`/`min_h`: the value of a key matching `min_w`/`min_h` case-insensitively, `unquote`d, parsed as a positive `i32`, else 0;
    - `outside` and NotFound give `resizable: false, min_w: 0, min_h: 0`;
    - update the existing tests' `ManifestFlags { .. }` literals with the new fields.
  - `spawn_app`, after computing `w`/`h` and building `win`, when `flags.resizable`:
    - `win.resizable = true`;
    - `win.min_w = (if flags.min_w > 0 { flags.min_w } else { RESIZE_MIN_W }).max(RESIZE_MIN_W).min(w)`;
    - `win.min_h = (if flags.min_h > 0 { flags.min_h } else { RESIZE_MIN_H }).max(TITLE_BAR_H + 8).min(h)`.
  - `router.rs`: `KernelState` gains `pub screen: Screen`, with `with_screen(screen)`; `new()` = `with_screen(Screen::DEFAULT)`. In `kernel.rs`, `Kernel::with_screen` builds `KernelState::with_screen(screen)`. Then add:

```rust
/// The size a resize to (w, h) actually gets: at least the window's
/// minimum (and, for large text, one character cell plus chrome), and at
/// most what fits on screen right of and below its top-left corner.
pub fn resize_clamp(win: &Window, screen: Screen, w: i32, h: i32) -> (i32, i32) {
    let s = win.font_scale.max(1);
    let min_w = win.min_w.max(6 * s + 2);
    let min_h = win.min_h.max(TITLE_BAR_H + 8 * s + 2);
    let max_w = (screen.w - win.x).max(min_w);
    let max_h = (screen.h - win.y).max(min_h);
    (w.clamp(min_w, max_w), h.clamp(min_h, max_h))
}

impl KernelState {
    /// Resizes a resizable window, clamped by `resize_clamp`: a new canvas
    /// with the old picture top-left and THEME_BG elsewhere is swapped in
    /// under the canvas lock (so the app's handle sees it), then the app
    /// gets Resized. None if the window isn't resizable or the size doesn't
    /// change.
    pub fn resize_window(&mut self, task: TaskId, w: i32, h: i32, dirty: &AtomicBool) -> Option<(i32, i32)> {
        let screen = self.screen;
        let win = self.windows.by_task_mut(task)?;
        if !win.resizable {
            return None;
        }
        let (w, h) = resize_clamp(win, screen, w, h);
        if (w, h) == (win.w, win.h) {
            return None;
        }
        {
            let mut canvas = win.canvas.lock();
            let mut next = Canvas::new(w, h);
            next.fill_rect(0, 0, w, h, THEME_BG);
            next.copy_rect_from(&canvas, 0, 0, w.min(canvas.width()), h.min(canvas.height()));
            *canvas = next;
        }
        win.w = w;
        win.h = h;
        send(&win.queue, Event::Resized { w, h });
        mark(dirty);
        Some((w, h))
    }
}
```

  Check `Canvas::copy_rect_from`'s argument meaning in `acid-gfx/src/lib.rs` before relying on it. It copies `src` pixels at the same coordinates (x, y, w, h) into self. If its meaning differs, copy the overlap with `blit` instead.
  - `Kernel::resize_window(&self, task, w, h) -> Option<(i32, i32)>` locks state and calls `st.resize_window(task, w, h, &self.dirty)`.
  - `composite_frame`:
    - the copied layer tuple gains `(resizable, w, h)`;
    - after blitting a resizable window at `(x, y)`, draw the grip on `fb`: for `gx, gy in 1..=6` with `gx + gy` in `{8, 10, 12}`, `fb.fill_rect(x + w - RESIZE_GRIP + gx, y + h - RESIZE_GRIP + gy, 1, 1, THEME_HARD)`.

- [ ] **Step 4: Run them and confirm they pass, then the full suite and warnings, then commit**

```bash
git add v3/crates/acid-kernel/src
git commit -m "Kernel: resizable windows: opt-in, minimums, resize_window and the grip

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 3: The grip gesture and the outline

**Files:**
- Modify: `v3/crates/acid-kernel/src/router.rs`: `Resize` state, `poll`, `forget_task`, and an outline accessor
- Modify: `v3/crates/acid-kernel/src/kernel.rs`: the outline in `composite_frame`

**Interfaces:**
- Consumes: `resize_clamp` and `KernelState::resize_window` (Task 2).
- Produces: `RouterState::resize_outline(&self, windows: &WindowRegistry) -> Option<(i32, i32, i32, i32)>`, the screen rectangle of the target size, or an equivalent method on `KernelState`.

- [ ] **Step 1: Failing tests** (`router.rs` test module)

The existing `add` helper registers plain windows. Add a variant that makes a window resizable:

```rust
    fn add_resizable(st: &mut KernelState, task: u32, x: i32, y: i32, w: i32, h: i32) -> Arc<EventQueue> {
        let q = add(st, task, x, y, w, h, true);
        let win = st.windows.by_task_mut(TaskId(task)).unwrap();
        win.resizable = true;
        win.min_w = 80;
        win.min_h = 48;
        q
    }

    #[test]
    fn dragging_the_grip_resizes_on_release_and_only_then() {
        let mut st = KernelState::new(); // 640x480
        let d = AtomicBool::new(false);
        let q = add_resizable(&mut st, 1, 100, 100, 200, 150);
        // grip spans x 292..300, y 242..250 on screen
        poll(&mut st, None, touch(296, 246, true), &d);
        assert!(drain(&q).is_empty(), "a grip press is not a touch for the app");
        poll(&mut st, None, touch(346, 296, true), &d);
        assert_eq!(st.resize_outline(), Some((100, 100, 250, 200)), "the outline follows the pointer");
        assert_eq!(st.windows.by_task(TaskId(1)).map(|w| (w.w, w.h)), Some((200, 150)), "nothing applied yet");
        poll(&mut st, None, UP, &d);
        assert_eq!(st.windows.by_task(TaskId(1)).map(|w| (w.w, w.h)), Some((250, 200)));
        assert_eq!(drain(&q), [Event::Resized { w: 250, h: 200 }]);
        assert_eq!(st.resize_outline(), None);
    }

    #[test]
    fn the_resize_target_clamps_to_the_minimum_and_the_screen() {
        let mut st = KernelState::new();
        let d = AtomicBool::new(false);
        add_resizable(&mut st, 1, 100, 100, 200, 150);
        poll(&mut st, None, touch(296, 246, true), &d);
        poll(&mut st, None, touch(0, 0, true), &d);
        assert_eq!(st.resize_outline(), Some((100, 100, 80, 48)));
        poll(&mut st, None, touch(2000, 2000, true), &d);
        assert_eq!(st.resize_outline(), Some((100, 100, 540, 380)));
    }

    #[test]
    fn a_grip_press_on_a_fixed_window_is_an_ordinary_touch() {
        let mut st = KernelState::new();
        let d = AtomicBool::new(false);
        let q = add(&mut st, 1, 100, 100, 200, 150, true);
        poll(&mut st, None, touch(296, 246, true), &d);
        assert_eq!(drain(&q), [Event::Touch { x: 196, y: 146, pressed: true }]);
    }

    #[test]
    fn releasing_where_it_started_sends_nothing() {
        let mut st = KernelState::new();
        let d = AtomicBool::new(false);
        let q = add_resizable(&mut st, 1, 100, 100, 200, 150);
        poll(&mut st, None, touch(296, 246, true), &d);
        poll(&mut st, None, UP, &d);
        assert!(drain(&q).is_empty());
    }

    #[test]
    fn forget_task_ends_a_resize() {
        let mut st = KernelState::new();
        let d = AtomicBool::new(false);
        add_resizable(&mut st, 1, 100, 100, 200, 150);
        poll(&mut st, None, touch(296, 246, true), &d);
        forget_task(&mut st, TaskId(1));
        assert_eq!(st.resize_outline(), None);
    }
```

Also:
- extend `forget_task_clears_focus_touch_and_drag` so it covers `resize` too;
- add a kernel test that composites while a resize gesture is held and finds `THEME_VIOLET` on the outline's top edge;
- keep the title-bar drag and close-dot tests passing unchanged.

- [ ] **Step 2: Run them and confirm they fail**

- [ ] **Step 3: Implement**
  - `struct Resize { task: TaskId, press_x: i32, press_y: i32, orig_w: i32, orig_h: i32, w: i32, h: i32 }`, and `resize: Option<Resize>` in `RouterState`.
  - `forget_task` clears `resize` for the task.
  - In `poll`:
    1. Add `&& st.router.resize.is_none()` to the desktop-strip condition.
    2. After the `drag` block, add a `resize` block. While `pressed`, look up the window; if it's gone, clear `resize` and return. Otherwise set the target to `resize_clamp(win, st.screen, orig_w + (x − press_x), orig_h + (y − press_y))`, and mark the screen dirty if it changed. On release, take the gesture; if the target differs from the window's size, call `st.resize_window(task, w, h, dirty)`. Return either way.
    3. In the fresh-press branch, **before** the title-bar check: if the window is resizable and `rel_x >= win_w − RESIZE_GRIP && rel_y >= win_h − RESIZE_GRIP`, activate it, start `Resize` with the target equal to the current size, and return.
  - `resize_outline()` returns `(win.x, win.y, r.w, r.h)` for the active gesture. Put it on `KernelState`, since it needs the window's position.
  - `kernel.rs` `composite_frame`: read the outline under the state lock, together with the layers. After the overlay blit, draw four 1 px `THEME_VIOLET` rects (top, bottom, left, right) when the outline is `Some`.

- [ ] **Step 4: Run them and confirm they pass, then the full suite and warnings, then commit**

```bash
git add v3/crates/acid-kernel/src
git commit -m "Router: drag a window's grip to resize it, with an outline

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 4: About, Network and System Monitor re-lay out on resize

**Files:**
- Modify: `v3/apps/about.lua`, `network.lua`, `sysmon.lua`, and their `.app.toml`s
- Modify: `v3/tools/game_test_env.lua` (a `resize_app` helper)
- Create: `v3/tools/test_resize_about.lua`, `test_resize_network.lua`, `test_resize_sysmon.lua`
- Modify: `v3/crates/acid-lua/tests/game_tests.rs`

**Interfaces:**
- Consumes: `AcidApp:on_resize(w, h)` (Task 1), and `acid_window_size` reporting the live size (in tests, `WIN_W`/`WIN_H`).
- Produces, for Tasks 5–7: a test helper `resize_app(w, h)` that sets `WIN_W, WIN_H = w, h`, calls `GAME:on_resize(w, h)`, empties `TEXT_AT` and `RECTS`, and calls `GAME:redraw()`.

The rule for every app:
- Module-level `local CW, CH = acid_font_size()` stays, because the font never changes for a window.
- Everything derived from the **window size** moves into `function App:layout()`. It reads `local WW, WH = acid_window_size()` and sets the fields on `self`, or on the class if the code reads class fields; follow each app's style.
- Every function that used the module-level `WW`/`WH` locals reads the stored fields instead.
- `on_create` calls `self:layout()` first.
- `function App:on_resize(w, h) self:layout() end`, plus state fix-ups where an app needs them.

- [ ] **Step 1: Test helper and failing suites**

`game_test_env.lua`:

```lua
-- Resizes the app under test the way the kernel would: the live window
-- size changes, then on_resize, then a redraw, with TEXT_AT/RECTS emptied
-- first so a fit check sees only the new frame.
function resize_app(w, h)
  WIN_W, WIN_H = w, h
  GAME:on_resize(w, h)
  TEXT_AT, RECTS = {}, {}
  GAME:redraw()
end
```

`test_resize_about.lua` seeds about.txt the way `test_about_fits.lua` does, then:

```lua
GAME.lines = GAME:read_lines()
resize_app(300, 200)
local fits, what = drawn_inside_window()
local clear, w2 = drawn_text_clear()
ok(fits and clear, "fits at 300x200" .. (what and (": " .. what) or "") .. (w2 and (": " .. w2) or ""))
local longest = 0
for _, t in ipairs(TEXT_AT) do longest = math.max(longest, #t[1]) end
eq(longest, (300 - 24) // FONT_W, "lines re-clip to the new width")
resize_app(120, 60)
fits, what = drawn_inside_window()
ok(fits, "fits at its minimum" .. (what and (": " .. what) or ""))
```

`test_resize_network.lua` follows the same pattern: resize to 320×160 and then to 140×80, checking the fit and no overlap after each.

`test_resize_sysmon.lua` seeds windows and tasks like `test_sysmon_large.lua`. Resize to 360×260 and then to 160×120; at each size, for each of the 4 pages, check the fit and no overlap. Also check that `G.page` is unchanged by a resize.

`game_tests.rs`: add three suites with the Normal prelude for each app's opening size (`WIN_W, WIN_H = 180, 150` / `200, 110` / `200, 160`), files env + app + test, and pinned counts.

Prove each suite fails against the pre-change app (`git show <BASE>:v3/apps/<app>.lua`).

- [ ] **Step 2: Implement** `layout()` and `on_resize` in the three apps, following the rule above. Add `resizable = true` and the minimums to each `.app.toml`: About `min_w = 120`, `min_h = 60`; Network `140`/`80`; System Monitor `160`/`120`.

- [ ] **Step 3: Run the suites (old Normal and Large suites unchanged and green), then the full suite and warnings, then commit**

```bash
git add v3/apps/about.lua v3/apps/about.app.toml v3/apps/network.lua v3/apps/network.app.toml v3/apps/sysmon.lua v3/apps/sysmon.app.toml v3/tools/game_test_env.lua v3/tools/test_resize_about.lua v3/tools/test_resize_network.lua v3/tools/test_resize_sysmon.lua v3/crates/acid-lua/tests/game_tests.rs
git commit -m "About, Network and System Monitor re-lay out when resized

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 5: Terminal re-lays out on resize

**Files:** `v3/apps/terminal.lua`, `terminal.app.toml` (`resizable = true`, `min_w = 160`, `min_h = 80`), `v3/tools/test_resize_terminal.lua`, `game_tests.rs`

- [ ] **Step 1: Failing suite.** Use the same file list as the `terminal` suite, with prelude `"WIN_W, WIN_H = 260, 160"` and the new test last. Fill `G.lines` with 40 lines of 60 characters, and set `G.input` to 50 characters.
  - `resize_app(400, 300)`: assert `G:visible_lines() == (300 - 16) // (FONT_H + 2) - 1`, the fit and no overlap, and longest drawn line == `(400 - 8) // FONT_W`.
  - `resize_app(160, 80)`: assert the fit, and that the **last** scrollback line is still drawn. Find `G.lines[#G.lines]:sub(1, cols)` in `TEXT_AT`.
  - Prove it fails against the pre-change `terminal.lua`.
- [ ] **Step 2: Implement** `TerminalApp:layout()`, which recomputes `WINDOW_W/H`, `COLS` and anything else derived from the window. `on_resize` calls `layout()`; the scrollback is already drawn from the end, so nothing else should be needed (check `draw_scrollback`).
- [ ] **Step 3: Run the suites, then the full suite and warnings, then commit** with the message "Terminal re-lays out when resized" plus the trailer.

---

### Task 6: File Manager re-lays out on resize

**Files:** `v3/apps/file_manager.lua`, `file_manager.app.toml` (keep `libs` and `font`; add `resizable = true`, `min_w = 120`, `min_h = 80`), `v3/tools/test_resize_file_manager.lua`, `game_tests.rs`

- [ ] **Step 1: Failing suite.** Prelude `"WIN_W, WIN_H = 220, 160"`; files env, `lib/acid_scrollbar.lua`, `file_manager.lua`, then the new test. Seed `v3/fsroot/Many` with 30 files (as the Large suite does), scan it, and select row 25 (`G.selected = 25; G:ensure_listing_scroll()`).
  - `resize_app(400, 300)`:
    - `G:visible_listing_rows() == (300 - 16) // (FONT_H + 4) - 1`;
    - `{ G:bar_geometry() } == { 400 - 7, 16 + FONT_H + 4, 300 - 1 - (16 + FONT_H + 4) }`;
    - the selected row is still visible (`G.selected >= G.scroll and G.selected < G.scroll + G:visible_listing_rows()`);
    - the fit and no overlap.
  - `resize_app(120, 80)`: the selected row is still visible, and the fit holds.
  - Open a preview of a 50-line file, scroll it to the end, then `resize_app(400, 300)`: `G.preview_scroll` is clamped to the new maximum (`G:max_preview_scroll(lines)`).
  - Prove it fails against the pre-change code.
- [ ] **Step 2: Implement** `FileManagerApp:layout()` (window size, `CLIP_COLS`). `on_resize` calls `layout()`, then `ensure_listing_scroll()`, then clamps `preview_scroll` when previewing.
- [ ] **Step 3: Run the suites, then the full suite and warnings, then commit** with the message "File Manager re-lays out when resized" plus the trailer.

---

### Task 7: Editor re-lays out on resize

**Files:** `v3/apps/editor/layout.lua`, `v3/apps/editor.lua`, `editor.app.toml` (add `resizable = true`, `min_w = 200`, `min_h = 100`), `v3/tools/test_resize_editor.lua`, `game_tests.rs`

- [ ] **Step 1: Failing suite.** Use the same file list as the `editor_app` suite, with prelude `"WIN_W, WIN_H = 420, 280"` and the new test last. Load 100 lines of 80 characters into the buffer (use that suite's way of doing it) and put the cursor on line 90, column 70.
  - `resize_app(600, 400)`:
    - `{ EditorLayout.WINDOW_W, EditorLayout.WINDOW_H, EditorLayout.STATUS_Y } == { 600, 400, 400 - (FONT_H + 2) }`;
    - the fit;
    - the cursor's line and column are within the visible rows and columns. Use the editor's own visible-lines and visible-columns helpers and its scroll fields.
  - `resize_app(200, 100)`: the same cursor-visible check, and the fit.
  - Prove it fails against the pre-change code.
- [ ] **Step 2: Implement.**
  - Turn `EditorLayout`'s window-derived assignments into `function EditorLayout.compute(w, h)`, which sets `WINDOW_W`, `WINDOW_H`, `STATUS_Y` and anything else derived from the window. Font-derived fields such as `CHAR_W`, `LINE_H`, `GUTTER_W` and `TEXT_X` can stay at module level.
  - Call `EditorLayout.compute(acid_window_size())` once at the bottom of the module, so the existing load order still works.
  - `EditorApp:on_resize(w, h)` calls `EditorLayout.compute(w, h)`, then the editor's existing scroll-to-cursor routine.
  - `cmdbar`'s `CMD_CELL_CHARS` depends on `WINDOW_W`. Recompute it inside `compute`, or make it a function. Check the cmdbar and touch modules read it at use time, not at load time.
- [ ] **Step 3: Run the suites, then the full suite and warnings, then commit** with the message "Editor re-lays out when resized" plus the trailer.

---

### Task 8: Golden frame and the manual

**Files:**
- Modify: `v3/crates/acid-os/tests/golden.rs`
- Create (after user approval only): `v3/crates/acid-os/tests/golden/file_manager_resized.ppm`
- Modify: `docs/manual-v3/01-getting-started.md`, `02-apps-and-manifests.md`, `04-graphics.md`, `09-api-reference.md`

- [ ] **Step 1: Golden test.** Use the real helper names and signatures already in `golden.rs` (`boot_with`, `wait_for_desktop`, `wait_for_border`, `assert_matches_golden_masked`, `clock_mask`).
  1. Boot at `Screen::DEFAULT`, wait for the desktop, open File Manager with `spawn_from_manifest`, and wait for its border.
  2. Call `k.resize_window(fm, 400, 300)` and assert it returns `Some((400, 300))`.
  3. Wait until File Manager has redrawn at the new size: its canvas's bottom border row at y = 299 is `THEME_HARD`, using `wait_for_border(&k, fm, 400, 300)`.
  4. Composite and compare with `file_manager_resized.ppm`, masking the clock.

  Run it, expect the "no golden yet" panic, and convert the candidate to PNG with `magick`.
- [ ] **Step 2: USER GATE.** Stop. The controller shows the PNG to the user. The golden is copied only after explicit approval; until then, nothing is committed.
- [ ] **Step 3: Manual.**
  - `02-apps-and-manifests.md`: add `resizable`, `min_w` and `min_h` rows to the manifest key table. `resizable = true` gives the window a corner grip; the minimums default to 80×48; carts may opt in.
  - `04-graphics.md`: in the Text size subsection, or a new short "Window size" subsection next to it, say to compute layout in a `layout()` method that runs in `on_create` and again in `on_resize`.
  - `09-api-reference.md`:
    - in the `AcidApp` table, `on_resize(w, h)`: after the user resizes a resizable window; `redraw` follows;
    - in the `acid_window_size` entry, it reports the current size, which changes when the user resizes a resizable window.
  - `01-getting-started.md`: one sentence. Windows with a small grip in their bottom-right corner can be resized by dragging it; an outline shows the new size until you let go.
- [ ] **Step 4: After approval.** Copy the golden, run the full suite and check warnings, then commit golden.rs, the .ppm and the four chapters with the message "Golden frame of a resized File Manager; document resizable windows" plus the trailer.
- [ ] **Step 5: USER GATE, interactive check.** The user runs the OS and confirms:
  - dragging the grip on File Manager, Terminal and Editor shows an outline;
  - on release, each resizes and re-lays out, keeping its state;
  - a window can't go below its minimum or off the screen;
  - games show no grip.
