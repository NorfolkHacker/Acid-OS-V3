# 1. Getting started

[← Contents](README.md) · [Next: Apps and manifests →](02-apps-and-manifests.md)

You develop against the **hosted build**: the whole OS as one native program
that runs in a window on your desktop. It is the real kernel, the real
compositor, the real synthesiser and the real Lua VMs; only the display, the
mouse and keyboard, and the sound device are your computer's. Nothing in this
manual needs anything but a Rust toolchain and a Linux desktop.

## 1.1 Build and run

### Dependencies

You need a Rust toolchain (the workspace uses edition 2024, so a current stable release) and, on Linux, the ALSA
development package and `pkg-config`, because the sound output uses `cpal`,
which builds against `alsa-sys`.

```sh
# Debian / Ubuntu
sudo apt install build-essential pkg-config libasound2-dev
# Arch
sudo pacman -S base-devel alsa-lib
```

There are no submodules and no C toolchain to set up: the Lua VM and the
WebAssembly runtime are both Rust crates that Cargo fetches and builds.

### Build and run

From the repository root:

```sh
cargo run --manifest-path v3/Cargo.toml -p acid-os
```

A 640×360 window titled "Acid OS v3" opens onto the desktop. **Menu** is at the
top left.

> **Run it from the repository root.** The kernel loads app scripts by
> root-relative path (`v3/apps/<name>.lua`), so the working directory has to be
> the repo root or nothing launches.

### Opening one app directly: `--app`

The binary takes one flag. `--app <name>` boots the desktop and then also opens
`v3/apps/<name>.lua`, sized and configured from `v3/apps/<name>.app.toml`,
raised and focused:

```sh
cargo run --manifest-path v3/Cargo.toml -p acid-os -- --app tetris
```

The name is the file's base name, not the Menu name, so it is `file_manager`,
not "File Manager". It reaches apps whose manifest says `menu = false` too,
which is the quickest way to run a game while you work on it. An unknown name,
one whose manifest lacks `w` or `h`, or one whose window cannot be opened (a
size outside 1 to 640 by 1 to 360, or all eight window slots taken) prints
`Acid OS v3: --app <name>: no such app` to the terminal and boots normally.
There are no other flags, and `--app` takes exactly one name.

### Driving the window

| Input | Effect |
|---|---|
| Left mouse button | Touch: press, drag, release. Other buttons do nothing |
| Dragging a title bar | Moves a window |
| The green dot at a title bar's right | Closes that window |
| Keyboard | Delivered to the focused window as key events (presses only, no auto-repeat) |
| Closing the host window | Ends the process |

Keys arrive already resolved: Shift gives you the character that would be typed
(`A`, `!`), and the arrows, Enter, Backspace, Escape, Tab and Delete become the
`AcidKeys` constants ([chapter 3](03-app-lifecycle.md#31-the-callbacks)). Ctrl, Alt
and the function keys are ignored.

### Running the tests

```sh
cargo test --manifest-path v3/Cargo.toml --workspace
```

The suite builds the example WASM cart from source, so it needs the
`wasm32-unknown-unknown` standard library (on Arch, `sudo pacman -S rust-wasm`);
without it that one test fails and says why. The manual you are reading is part
of the suite: `cargo test --manifest-path v3/Cargo.toml -p acid-os --test
manual` runs every complete example in these chapters and parses every
fragment.

## 1.2 There is no hardware build

This is the hosted x64 build only, and nothing in the manual depends on a
device. The system is still shaped for small hardware (a 640×360 screen, an
8-window limit, one task per app), and that gives you one rule to keep in mind:
an app should be small and polite. See [§8](08-cookbook.md) for the conventions.

## 1.3 Your first app

An app is **two files** in `v3/apps/`, sharing a base name.

### The script

`v3/apps/counter.lua`:

```lua app
-- w: 180
-- h: 120
local CounterApp = AcidApp:extend("CounterApp")

local WINDOW_W = 180
local WINDOW_H = 120

local BG = 0x050607
local TEXT = 0xD4E6DB
local ACCENT = 0x00FF66

function CounterApp:on_create()
  self.count = 0
  self.touch_down = false
end

function CounterApp:on_touch(x, y, pressed)
  if not pressed then
    self.touch_down = false
    return
  end
  if self.touch_down then return end
  self.touch_down = true
  self.count = self.count + 1
  self:redraw()
end

function CounterApp:redraw()
  acid_clear_user_area()
  acid_draw_window_frame(self:window_title())
  acid_fill_rect(20, 40, WINDOW_W - 40, 30, ACCENT)
  acid_draw_text("TAPS: " .. self.count, 34, 52, BG, ACCENT)
  acid_draw_text("tap anywhere", 40, 88, TEXT, BG)
  acid_draw_window_border()
end

CounterApp:new():start()
```

`WINDOW_H` is declared to match the manifest below even though this app only
lays out against the width; [§2.2](02-apps-and-manifests.md#keeping-the-constants-in-sync)
explains why you keep the pair. The two leading `-- w:` and `-- h:` comment
lines are not part of the app. They are how this manual's test tells its runner
what size window to open for the example; delete them from your own copy.

### The manifest

`v3/apps/counter.app.toml`:

```toml
name = Counter
w = 180
h = 120
desc = Counts taps
```

### Run it

```sh
cargo run --manifest-path v3/Cargo.toml -p acid-os -- --app counter
```

or restart the OS and pick **Menu → Counter**. The launcher rescans
`v3/apps/` for `*.app.toml` files at every boot, so a new app needs a restart (or
`--app`) but never a rebuild: Cargo has nothing to recompile, because the app
is not part of the Rust program.

## 1.4 What just happened

1. At boot, `desktop.lua` scanned `v3/apps/` for `*.app.toml` manifests and
   registered each one it could parse as launchable.
2. Picking **Counter** made the kernel start a **new OS thread** with its
   **own Lua VM** and its own 180×120 drawing canvas.
3. That VM loaded the framework libraries (`acid_keys.lua`, `acid_palette.lua`,
   `acid_waveform.lua`, `acid_app.lua`, `acid_game.lua`), then any modules your
   manifest asked for, then `counter.lua`.
4. The last line, `CounterApp:new():start()`, entered `AcidApp`'s event loop,
   which blocks on the window's event queue and dispatches to your callbacks.
5. Your drawing went to a private canvas. The compositor blits every window's
   canvas to the screen in z-order.

Because each app is its own VM on its own thread, a Lua error in your app takes
down your app and nothing else: its window disappears and the kernel carries on.
The message goes to the terminal you launched the OS from, as
`Acid OS v3: v3/apps/counter.lua: <message>`. **Keep that terminal visible while
developing**; it is your only error console.

Two limits are watched for you, and each ends the app with a line in the same
terminal: more than 64 MB of Lua memory (`out of memory`), or a callback that
runs for more than 2 seconds without returning to the event loop
(`stopped responding`). Apps installed as carts get 16 MB and 1 second. See
[§2.6](02-apps-and-manifests.md#26-built-in-and-cart-level-apps).

## 1.5 The development loop

Each start loads your script **from disk**, so:

- **Changing an existing app's Lua**: close its window and launch it again
  (Menu, or `--app`). No restart, no rebuild.
- **Adding a new app, or changing a `.app.toml`**: restart the OS. The launcher
  registry is built once, at boot, by `desktop.lua`'s manifest scan. (`--app`
  reads the manifest fresh, so it sees changes at once.)
- **Changing Rust**: `cargo run` rebuilds what changed and restarts.

There is also an **Editor** app inside the OS (Menu → Editor) that opens app
source from the running system, and a **File Manager** that launches an app by
clicking its `.app.toml`. Editing an app from inside the OS it runs in is a
perfectly good way to work.

---

[← Contents](README.md) · [Next: Apps and manifests →](02-apps-and-manifests.md)
