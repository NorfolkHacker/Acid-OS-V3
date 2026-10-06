# 1. Getting started

[← Contents](README.md) · [Next: Apps and manifests →](02-apps-and-manifests.md)

You write and test apps on the **hosted build**: the whole OS as one ordinary
program that runs in a window on your desktop. It isn't a simulator. The
kernel, the window system, the synthesiser and the Lua VMs are all the real
thing. Only the screen, the mouse and keyboard, and the sound device are
borrowed from your computer.

All you need is a Rust toolchain and a Linux desktop.

## 1.1 Build and run

### Dependencies

You need:

- a current stable Rust (1.85 or newer, because the code uses the 2024 edition)
- `pkg-config` and the ALSA development package, because the sound output is
  built on ALSA (through the `cpal` and `alsa-sys` crates)

```sh
# Debian / Ubuntu
sudo apt install build-essential pkg-config libasound2-dev
# Arch
sudo pacman -S base-devel alsa-lib
```

That's it. There are no submodules and no C toolchain to set up. The Lua VM
and the WebAssembly runtime are both Rust crates, so Cargo fetches and builds
them for you.

### Build and run

From the top folder of the repository:

```sh
cargo run --manifest-path v3/Cargo.toml -p acid-os
```

A window titled "Acid OS v3" opens on the screen-size picker: 640×480 (the
default), 640×360 or 800×600. Choose with the arrow keys and Enter or a click,
or wait three seconds for the default. `-- --screen 800x600` skips the picker.
Then the desktop appears. **Menu** is at the top left. Config's FONT setting
switches the text apps (Terminal, Editor, File Manager, System Monitor, About
and Network) between Normal and Large; it applies to apps you open afterwards.
Config's RESTART (press it twice) brings the OS back to the size picker; every open app closes without saving.

Try **Acid Spin**: spinning 3D shapes. It isn't in the Menu. Open File
Manager, go into `App` and click Acid Spin's `.app.toml` file. Left and Right
change the shape, Up and Down the speed, Space cycles wire, solid and both, and
you can resize its window.

**AcidStorm** opens the same way: an arena shooter over an acid plasma
background. Hold W, A, S and D to move and the arrows to fire; hold two for a
diagonal. Or hold the pointer down to fire at it. P pauses. Rescue the humans
and clear the robots to finish a wave.

**Acid Snake** is in the same folder. The arrows or W, A, S and D turn (a tap
turns towards it), P pauses. Every fifth pellet brings a rainbow pellet for a
few seconds: it's worth five times as much and sets the snake's colours
spinning.

**Acid Invaders** is there too. Hold Left/A or Right/D to move. Space, Up or W
fires, and holding it keeps firing. Holding the pointer slides the cannon
under it, and a tap fires. Shoot the saucer for a
few seconds of rainbow triple shot.

**Acid Rocks** is a vector rock shooter. Hold Left/A or Right/D to turn and
Up/W to thrust. Space fires (hold it to keep firing) and Down/S jumps through
hyperspace. Hold the pointer to turn towards it and fire.

> **Run it from the top folder of the repository.** Acid OS finds app scripts
> by a path relative to that folder (`v3/apps/<name>.lua`). Run it from
> anywhere else and no app will launch.

### Opening one app directly: `--app`

There is one command-line flag. `--app <name>` boots the desktop as usual and
then opens `v3/apps/<name>.lua` as well, raised and focused. Its size and
settings come from `v3/apps/<name>.app.toml`.

```sh
cargo run --manifest-path v3/Cargo.toml -p acid-os -- --app tetris
```

A few things to know:

- Use the file's base name, not the name in the Menu: `file_manager`, not
  "File Manager".
- It works for apps whose manifest says `menu = false` too. That makes it the
  quickest way to run a game you're working on.
- If it can't open the app, it prints
  `Acid OS v3: --app <name>: no such app` to the terminal and boots normally.
  That happens when the name is unknown, when the manifest has no `w` or `h`,
  or when the window can't be opened (a size that isn't 1 to the screen's size in each direction, or
  all eight window slots already in use).
- `--app` takes exactly one name, and there are no other flags.

### Driving the window

| Input | Effect |
|---|---|
| Left mouse button | Touch: press, drag, release. Other buttons do nothing |
| Dragging a title bar | Moves a window |
| The green dot at a title bar's right | Closes that window |
| Keyboard | Delivered to the focused window as key events: a press, then a release (no auto-repeat) |
| Closing the host window | Ends the process |

Keys arrive already worked out for you. Shift gives you the character that
would be typed (`A`, `!`). The arrows, Enter, Backspace, Escape, Tab and Delete
arrive as `AcidKeys` constants ([chapter 3](03-app-lifecycle.md#31-the-callbacks)).
Ctrl, Alt and the function keys are ignored.

Windows with a small grip in their bottom-right corner can be resized by dragging it; an outline shows the new size until you let go.

### Running the tests

```sh
cargo test --manifest-path v3/Cargo.toml --workspace
```

The tests build the example WASM cart from source, so you also need the
`wasm32-unknown-unknown` target installed (on Arch, `sudo pacman -S rust-wasm`).
Without it, that one test fails and tells you why.

This manual is tested too. `cargo test --manifest-path v3/Cargo.toml -p acid-os
--test manual` runs every complete example in these chapters and checks that
every shorter fragment parses.

## 1.2 There is no hardware build

This build only runs hosted, on an x64 desktop, and nothing in the manual needs
a device. But Acid OS is still designed for small hardware: a 640×480 screen (640×360 at the smallest),
at most 8 windows, one task per app. So keep one rule in mind: **an app should
be small and polite.** [§8](08-cookbook.md) covers the conventions.

## 1.3 Your first app

An app is **two files** in `v3/apps/` with the same base name: a Lua script and
a manifest.

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

Two notes on this script:

- `WINDOW_H` matches the manifest below, even though this app only uses the
  width for its layout. [§2.2](02-apps-and-manifests.md#keeping-the-constants-in-sync)
  explains why you keep both.
- The `-- w:` and `-- h:` comments at the top aren't part of the app. They tell
  this manual's test what size window to open. Leave them out of your own copy.

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

Or restart the OS and pick **Menu → Counter**.

The Menu looks in `v3/apps/` for `*.app.toml` files each time Acid OS starts.
So a new app needs a restart (or `--app`), but never a rebuild. Your app isn't
part of the Rust program, so Cargo has nothing to recompile.

## 1.4 What just happened

1. When Acid OS started, `desktop.lua` looked in `v3/apps/` for `*.app.toml`
   manifests and registered each one it could read as launchable.
2. When you picked **Counter**, the kernel started a **new OS thread** with its
   **own Lua VM** and its own 180×120 canvas to draw on.
3. That VM loaded the framework libraries (`acid_keys.lua`, `acid_palette.lua`,
   `acid_waveform.lua`, `acid_app.lua`, `acid_game.lua`), then any modules your
   manifest asked for, then `counter.lua`.
4. The last line, `CounterApp:new():start()`, started `AcidApp`'s event loop.
   It waits for events on your window and calls your callbacks.
5. Your drawing went to your own private canvas. The compositor (the part that
   builds the screen) then copies every window's canvas to the screen, back to
   front.

Because each app has its own VM on its own thread, a Lua error in your app
stops your app and nothing else. Its window disappears and the rest of the
system carries on.

The error message goes to the terminal you started Acid OS from, like this:
`Acid OS v3: v3/apps/counter.lua: <message>`. **Keep that terminal visible
while you work**: it's the only place errors show up.

Two limits are also watched for you. Going over either one ends the app, with
a line in the same terminal:

- more than 64 MB of Lua memory (`out of memory`)
- a callback that runs for more than 2 seconds without returning to the event
  loop (`stopped responding`)

Apps installed as carts get tighter limits: 16 MB and 1 second. See
[§2.6](02-apps-and-manifests.md#26-built-in-and-cart-level-apps).

## 1.5 The development loop

Your script is loaded **from disk** every time the app starts. So:

- **Changed an existing app's Lua?** Close its window and launch it again
  (from the Menu, or with `--app`). No restart, no rebuild.
- **Added a new app, or changed a `.app.toml`?** Restart the OS. The Menu's
  list of apps is only built once, when `desktop.lua` reads the manifests at
  startup. (`--app` reads the manifest fresh, so it sees changes straight
  away.)
- **Changed Rust?** `cargo run` rebuilds what changed and starts it again.

You can also work from inside Acid OS itself. The **Editor** app (Menu →
Editor) opens app source from the running system, and the **File Manager**
launches an app when you click its `.app.toml`. Editing an app inside the OS
it runs in is a perfectly good way to work.

File Manager, Editor, Terminal and Load Cart show a scroll bar at their
right edge when there's more than fits. Drag its thumb, or click above or
below the thumb to move a screenful. In Terminal it scrolls back through
older output, and typing brings you back to the bottom.

**Sprite Paint** draws pixel-art sprites, with animation frames, and saves
them as `.spr` files in `Home`. Like the games, it isn't in the Menu: open
`App/sprite.app.toml` in the File Manager, or open any `.spr` file, such as
`Home/acid_ship.spr`, to edit it. Your apps can load sprites with
`AcidSprite.load` ([§4.6](04-graphics.md#sprite-files)).

**Acid Tracker** writes music: four channels of two voices, saved as `.trk`
files. Open `App/tracker.app.toml` or any `.trk` file in the File Manager,
from the `Home/music` folder, and press F1 to play it. You can also
play a song or a `.snd` sound from the Terminal with `play` and the file's path, such as a `.snd` file from `Home/sounds`.
See [chapter 11](11-music.md).

---

[← Contents](README.md) · [Next: Apps and manifests →](02-apps-and-manifests.md)
