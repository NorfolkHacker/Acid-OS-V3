# 2. Apps and manifests

[← Getting started](01-getting-started.md) · [Contents](README.md) · [Next: The app lifecycle →](03-app-lifecycle.md)

## 2.1 The two files

Every app in `v3/apps/` is a pair of files with the same base name:

```text
v3/apps/tetris.lua         the Lua source
v3/apps/tetris.app.toml    the manifest
```

**The manifest is what makes an app exist** as far as the OS is concerned:

- A `.lua` file with no manifest beside it is never registered, so you can't
  launch it from the Menu.
- A manifest with no matching `.lua` still registers, but the app fails to
  load when you pick it.
- `--app` and Terminal's `run` go through the manifest too.

A WASM cart is the same pair, with a `.wasm` file in place of the `.lua`. See
[§2.7](#27-wasm-carts).

## 2.2 The manifest format

Despite the `.toml` extension, a manifest is **not** real TOML. It's a much
simpler format. Acid OS reads it line by line:

- Each line is trimmed. Blank lines, and lines starting with `#`, are skipped.
- The line is split at the **first** `=`. Both sides are trimmed, and the value
  is kept exactly as written, as a plain string.
- A line with no `=` is ignored.
- There are no quotes, arrays or tables. A `#` comment must be on a line of its
  own.
- If a key appears twice, the **later** one wins.

The kernel and `desktop.lua` both read manifests this way.

```toml
name = System Monitor
w = 200
h = 160
desc = Live kernel/task/audio stats
menu = true
multi = false
libs = lib/acid_sprite.lua, lib/acid_eggs.lua
```

| Key | Required | Meaning |
|---|---|---|
| `name` | **yes** | The name shown in the Menu, the taskbar and window lists. Keep it short: the Menu cuts it off at 22 characters, and `AcidApp`'s default window title at 16. |
| `w` | **yes** | Window width in pixels, including the 1px border. 1 to the screen width (640 by default). |
| `h` | **yes** | Window height in pixels, including the 16px title bar. 1 to the screen height (480 by default). |
| `desc` | no | A one-line description shown alongside the app. It doesn't change how the app behaves. |
| `menu` | no | `menu = false` hides the app from the Menu. You can still launch it by path (File Manager, `acid_spawn_app`, `--app`, Terminal's `run`). Any other value, or leaving the key out, means the app is shown. Only the exact string `false` hides it. |
| `multi` | no | `multi = true` lets several windows of this app be open at once. By default an app is **single-instance**: launching it again raises and focuses the window that's already open instead of opening a second one. If a cart tries to launch it, the cart gets `false` and nothing is raised. And if a cart started the open copy, a built-in app launching it gets a new, trusted window of its own instead of raising the cart's copy. |
| `resizable` | no | `resizable = true` gives the window a small grip in its bottom-right corner; dragging it resizes the window and the app gets `on_resize(w, h)` ([§3.1](03-app-lifecycle.md#31-the-callbacks)). Any other value, or leaving the key out, means a fixed size. Carts may opt in too. |
| `min_w`, `min_h` | no | The smallest size a resizable window can be dragged to. Both default to 80 and 48, and are capped at the window's opening size. `min_w` is raised to at least 80, and `min_h` to at least the title bar plus one text line. Both are doubled for a Large-text window. Ignored without `resizable`. Carts may opt in too, through Load Cart's header (`-- resizable: true`, `-- min_w:`, `-- min_h:`) or their manifest. |
| `libs` | no | A comma-separated list of extra modules to load before the app's own script, relative to `v3/apps/` ([§2.3](#23-loading-modules)). |
| `source` | no | `source = cart` marks an app installed by Load Cart, and makes it run with cart-level trust ([§2.6](#26-built-in-and-cart-level-apps)). Only Load Cart should write this. |
| `runtime` | no | `runtime = wasm` means the app is `<name>.wasm` rather than `<name>.lua` ([§2.7](#27-wasm-carts)). Any other value, or leaving the key out, means Lua. |
| `font` | no | `font = scalable` opts the app into Config's FONT setting: at Large its text is drawn at 12×16 and its window opens bigger ([Text size](04-graphics.md#text-size)). Built-in apps only; ignored for carts. |

**A broken manifest is skipped without a word.** If `name`, `w` or `h` is
missing, or `w` or `h` isn't 1 to the screen's size in each direction, the app simply never appears
in the Menu. The other apps still load fine. So if your app is missing, look
for a typo in its own manifest first; the rest of the system is probably fine.

### Sizing a window

The screen is **640×480** by default (640×360 and 800×600 are the other sizes;
ask with [`acid_screen_size`](09-api-reference.md#acid_screen_size)). To fit
every size, keep a window within 640×336 below the strip. A cart installed on a bigger
screen can be too big to open on a smaller one, so keep carts within 640×336 if they
should open at every size. `w` and `h` are the size of the whole window,
frame included:

```text
+--------------------------------------+  <- y = 0, 1px THEME_HARD border
| Title                             ●  |  <- title bar, 16px tall
+--------------------------------------+  <- y = 16, your area starts here
|                                      |
|          your drawing area           |     w - 2 usable width
|                                      |     h - 17 usable height
+--------------------------------------+
```

So the space you can use runs from `(1, 16)` to `(w - 2, h - 2)`. In practice,
apps draw from `(0, 16)` to `(w, h)` and let clipping and the border tidy up
the edges. See [§4.2](04-graphics.md#42-coordinates-and-clipping).

Where windows go:

- New windows cascade down from the top left as more of them open. The cascade
  is kept in bounds, so a window always lands fully on screen, below the
  desktop strip.
- A window whose size isn't 1 to the screen's size in each direction isn't shrunk to fit. It's **refused**, and so
  is a manifest that asks for one.
- At most **eight** windows can be open at once, and that includes the
  desktop's own. Once they're all in use, launching another app quietly fails.

### Keeping the constants in sync

The kernel gets `w` and `h` from the manifest, but your Lua needs them too, for
layout. Nothing connects the two, so every app declares them again in its own
code and keeps them matching by hand:

```lua snippet
local CounterApp = AcidApp:extend("CounterApp")
CounterApp.WINDOW_W = 180   -- must match `w =` in counter.app.toml
CounterApp.WINDOW_H = 120   -- must match `h =` in counter.app.toml
```

If they get out of step, your app lays itself out for the wrong size. It won't
crash. Your canvas is always the manifest's size, and anything drawn outside it
is clipped away. So what you'll see is content that's mysteriously cut off, or
a stripe of unpainted background.

## 2.3 Loading modules

Lua in Acid OS has **no `require`**. It also has no `io`, `os`, `package` or
`dofile`. Each VM only has the `string`, `table`, `math`, `utf8` and
`coroutine` libraries. Everything else your app can reach is an `acid_*`
function.

Instead of `require`, Acid OS loads files for you before your script runs, in
exactly this order:

1. `v3/apps/lib/acid_keys.lua`: `AcidKeys` key codes
2. `v3/apps/lib/acid_palette.lua`: `AcidPalette.hue`
3. `v3/apps/lib/acid_waveform.lua`: `AcidWaveform` constants
4. `v3/apps/lib/acid_app.lua`: `AcidApp`
5. `v3/apps/lib/acid_game.lua`: `AcidGame`
6. everything in your manifest's `libs`, left to right
7. your own `<name>.lua`

**The first five are always there.** You never need to list them in `libs`. If
you do, no harm is done; they just run a second time. `libs` is for *extra*
modules:

```toml
libs = lib/acid_sprite.lua, lib/acid_eggs.lua
```

Everything shares one global namespace. A module defines a global table (for
example `AcidSprite = {}`), and your script uses it.

If you write a module for your own app, put it in `v3/apps/lib/` and name it in
`libs`. A subfolder of `v3/apps/` works too; Editor does this with
`editor/buffer.lua`.

If a module fails to load, or doesn't exist, the error goes to the terminal and
that module is skipped. The next module and your own script still run. Your
script will then fail the first time it uses whatever was missing.

### What `libs` will not accept

Each entry in `libs` is checked before it's loaded. A rejected entry is
skipped, the app still starts, and this goes to the terminal (stderr):
`Acid OS v3: rejected unsafe lib path <entry>`.

An entry is rejected unless it follows all of these rules:

- it must end in `.lua`
- it must be relative: a leading `/` is rejected
- no `\` anywhere
- no empty path parts (`a//b.lua`, or a trailing `/`)
- no `..` as a whole path part (`a/../../x.lua` is rejected, but a file
  really named `weird..name.lua` is fine)

## 2.4 How the launcher finds your app

When Acid OS starts, `desktop.lua`:

1. lists `v3/apps/`, keeps every file ending in `.app.toml`, and **sorts them**
   by file name;
2. reads each one, and for every manifest that has `name`, `w` and `h`, calls
   `acid_launcher_register(path, name, w, h, multi, libs)`, where `path` is the
   `.lua` beside the manifest (or the `.wasm`, for `runtime = wasm`);
3. notes whether `menu ~= "false"`, which is what the Menu uses to decide
   what to show.

There are two limits to know about:

- **The registry holds 48 apps.** Once it's full, further apps are refused, and
  it's the manifests that sort *last* that miss out. So if apps start vanishing
  from the Menu, count them before you blame your manifest.
- **The Menu shows the first ten** visible apps, in registry order. Any more are
  still registered, and you can launch them by path, but they don't appear in
  the Menu.

One trap: `v3/fsroot/App` is a symlink to `v3/apps`. The registry matches
launch paths as exact strings, so a path that goes through the symlink matches
nothing. The app starts in a VM with none of its modules loaded. Worse, a
script started from `v3/fsroot/...` runs at cart level
([§2.6](#26-built-in-and-cart-level-apps)). `AcidApp:canonical_app_path`
converts one form of path to the other. Use it whenever you take a path from
the filesystem and pass it to `acid_spawn_app`.

## 2.5 Carts

A **cart** is a whole app in one file, written outside the OS and brought in.
It's ordinary app Lua with a block of header comments at the top:

```lua snippet
-- name: Hello Acid
-- w: 200
-- h: 150
-- desc: Example cart -- colour-cycling bars
-- libs: lib/acid_palette.lua

local HelloAcidApp = AcidApp:extend("HelloAcidApp")

-- ... ordinary app code ...

HelloAcidApp:new():start()
```

You install carts with the **Load Cart** app (Menu → Load Cart). It looks for
carts in:

- `v3/carts/`
- `~/carts`
- your computer's `/media`, `/mnt` and `/run/media` mount points, so a cart on
  a USB stick or SD card shows up in the same list

Only the folders that exist are offered.

Installing a cart writes `v3/apps/<slug>.lua`, plus a generated
`v3/apps/<slug>.app.toml` that includes `source = cart`. (The slug is the
installed file's base name; see below.) There's a sample at
`v3/carts/hello_acid.cart`, and `v3/carts/README.txt` describes the format.
The sample ships already installed, as Hello Acid. Its manifest says
`menu = false`, so open it from File Manager (`App` → `hello_acid.app.toml`).
Installing the sample again writes a fresh manifest without that line, and it
then joins the Menu at the next boot.

### The header

All five keys are optional. The rules for reading them:

- Only the **first comment block** in the file is read. It ends at the first
  line that is neither blank nor a `--` comment. So a `-- name:` further down,
  in a comment among real code, is ignored.
- Keys are matched in lower case.
- A line without a `:` is just a comment.
- If a key appears twice, the first value wins.

| Key | Default if absent, and limits |
|---|---|
| `name` | Made from the filename (`my_game.cart` → "My Game"). At most 40 characters. |
| `w` | 220, clamped to 80 to the screen width |
| `h` | 160, clamped to 48 to the screen height less the 24 px strip (456 at the default 640×480) |
| `desc` | Empty. At most 40 characters. |
| `libs` | None. Only entries naming modules that exist in `v3/apps/lib/` (written `lib/<file>.lua`) are kept, and at most eight. |

A few more rules keep installs safe:

- Every value is cleaned: anything that isn't printable ASCII becomes a space.
  That way a header can never sneak an extra line into the generated manifest.
- A cart file must be between 1 byte and 256 KB.
- The slug comes from the filename. Only lower-case letters and digits are
  kept, and any run of other characters becomes a single `_`. It's at most 24
  characters, and it can never contain a path separator or a `..`-style
  traversal part.

### What a cart may and may not overwrite

A cart can replace an app that was *itself* installed from a cart. That's what
the `source = cart` line in the generated manifest marks.

A cart can never overwrite a hand-written app in `v3/apps/`, including ones
with no manifest at all, such as `desktop.lua`. Load Cart's confirm screen says
"REFUSED: <slug> is a built-in app". If the install is attempted anyway, it
reports "that slot belongs to a built-in app".

If the cart's *display name* matches an app that's already registered, Load
Cart warns you before installing. That's because Terminal's `run` launches the
first case-insensitive match in registry order, which might not be the one you
expect.

An installed cart joins the Menu at the **next boot**. Until then, Load Cart's
own **RUN** button starts it straight away.

### Carts are sandboxed, but only lightly

Once installed, a cart runs at **cart level**
([§2.6](#26-built-in-and-cart-level-apps)). That means smaller limits, no
writing outside `Home`, and no control over other apps' windows.

That's a real fence, but it limits what a cart can *reach*, not what it can
draw or play in its own window. The header checks stop a *badly formed* cart.
The cart-level rules limit a *hostile* one. Even so, read a cart before you
install it.

An installed cart also shows up as a new untracked file in `git status`.
Commit it or delete it like any other file.

## 2.6 Built-in and cart-level apps

Every app runs at one of two **trust levels**. The kernel decides which when it
starts the app, and the app can't change it.

**Built-in** apps meet all of these:

- the script path starts with `v3/apps/` and ends in `.lua`
- the app's own manifest (the script path with `.lua` replaced by `.app.toml`)
  doesn't say `source = cart`

A script with no manifest at all counts as built-in. That's why Load Cart
writes the manifest *before* the script when it installs.

**Cart-level** is everything else. That includes:

- installed carts (their generated manifests say `source = cart`)
- any script started from `v3/fsroot/...`
- any `.wasm` module, always (no manifest is even checked)
- a script whose manifest exists but can't be read

The `source` key and its value are matched loosely (`Source = "Cart"` counts),
so you can't get trust back with a spelling trick.

Any app that a cart starts, with `acid_spawn_app` or `acid_launcher_spawn`, is
cart-level too. That's true even for `v3/apps/editor.lua`: a cart can't borrow
a built-in app's rights by launching it. And if a cart started an open copy of
a single-instance app, a built-in app launching it never raises the cart's
copy. It gets a new, trusted window of its own instead.

The manifest is read at the moment the app starts, not taken from the Menu's
registry. So the level is the same however the app was started: from the Menu,
File Manager, Terminal's `run`, Load Cart's RUN, or `--app`.

### Limits

| | Lua memory | "Stopped responding" |
|---|---|---|
| Built-in | 64 MB | 2 s |
| Cart-level | 16 MB | 1 s |

The "stopped responding" limit is the time since your app last returned from
`acid_poll_event` (or since it started, before its first poll). `AcidApp`'s
event loop polls all the time, so this only matters if one of your callbacks
keeps computing for longer than the limit.

Going over either limit ends the app, and the terminal shows
`Acid OS v3: <path>: out of memory` or `Acid OS v3: <path>: stopped responding`.

WASM carts have different limits ([chapter 10](10-wasm-carts.md)).

### What a cart is refused

Every refusal fails cleanly: the call returns its failure value and your app
carries on.

| Call | Cart-level behaviour |
|---|---|
| `acid_fs_write`, `acid_fs_delete` | allowed only under `v3/fsroot/Home/`; otherwise `nil, "read only"` |
| `acid_fs_rename` | both ends must be under `v3/fsroot/Home/`; otherwise `nil, "read only"` |
| `acid_launcher_register` | returns `false` |
| `acid_spawn_app` | returns `false` unless the path starts with `v3/apps/` |
| `acid_launcher_spawn`, `acid_spawn_app` | return `false` for a single-instance app that is already open; its window is not raised or focused |
| `acid_launcher_spawn`, `acid_spawn_app` | the app they start runs cart-level, even one that would otherwise be built-in |
| `acid_launcher_spawn`, `acid_spawn_app` | return `false` while 4 or more cart-level windows are open (the cart's own included); nothing is started |
| `acid_launcher_spawn`, `acid_spawn_app` | return `false` once the cart's own window has been closed, even if the cart is still running |
| `acid_cart_roots`, `acid_cart_list`, `acid_cart_stat`, `acid_cart_read` | `nil, "not allowed"` |
| `acid_close_window` | returns `false` |
| `acid_activate_window` | does nothing unless the index is the cart's own window |
| `acid_overlay_open` | returns `false`, so a cart can never hold the full-screen overlay |

The ordinary path checks ([§7](07-system-apis.md)) still apply first.

Everything else works for a cart exactly as it does for a built-in app,
including:

- reading files
- drawing and sound
- its own window
- launching installed apps that aren't already open with `acid_spawn_app`
  (they run cart-level, and only while fewer than 4 cart-level windows are
  open)
- `acid_set_volume` and `acid_set_wallpaper_enabled`

`acid_close_window` always returning `false` costs a cart nothing. No app can
close its own window with it anyway: an app closes its window by ending its
run loop (`self:quit()`, [§3.4](03-app-lifecycle.md#34-focus-titles-and-quitting)).

## 2.7 WASM carts

A cart can also be a **WebAssembly module**, written in Rust or anything else
that targets WebAssembly. It runs on the same kernel, in a stricter sandbox. It
installs just like a `.cart`, with these differences:

- The file ends in `.wasm`, and installs as `v3/apps/<slug>.wasm`. Its
  generated manifest includes **`runtime = wasm`** (just before the final
  `source = cart` line). That line is what tells the Menu, File Manager,
  Terminal's `run` and `--app` to run `<slug>.wasm` instead of `<slug>.lua`.
- It always runs at **cart level**.
- Its header isn't a comment block. It's a **custom section named `acid`**
  inside the module. The section holds the same `key: value` lines as a
  `.cart` header, without the `-- ` at the start. The same keys apply, except
  `libs`, which is ignored because a WASM module loads no Lua.
- A module with no `acid` section, or a broken one, still installs: under its
  filename, at the default size (220×160), with no description. A hostile
  module can only lose its own details.
- The 256 KB size limit is the same.

[Chapter 10](10-wasm-carts.md) covers the rest: the callback ABI, the import
table, the fuel and memory limits, and the `acid-cart` crate. The sample is
`v3/carts/hello_wasm.wasm`. All the manifest side needs to know is that a WASM
app is launched, sized and shown in the Menu from its `.app.toml`, exactly like
a Lua one.

## 2.8 Where an installed cart lands

An install fills a **slot**: the manifest `<slug>.app.toml` plus one script,
either `<slug>.lua` or `<slug>.wasm`. Load Cart sorts every slot into one of
three cases:

- **Fresh install:** there's no script and no manifest yet.
- **Replace:** the slot is occupied and its manifest says `source = cart`. The
  install writes the new manifest, then the new script, and then **deletes the
  other runtime's script** if there is one. For example, installing
  `snake.wasm` over an installed `snake.lua` cart leaves `snake.wasm` and
  deletes `snake.lua`. That way the manifest's `runtime` line always matches
  the file that's there.
- **Protected:** anything else, and Load Cart refuses it. That covers an
  occupied slot whose manifest doesn't say `source = cart`, an occupied slot
  with no manifest at all (any `.lua` that Load Cart didn't install), and a
  leftover manifest with no script.

A slot counts as **occupied** if *either* `<slug>.lua` *or* `<slug>.wasm`
already exists. This matters: a `desktop.wasm` cart mustn't find the slot free
just because only `desktop.lua` is there.

**The manifest is always written first, and the script second.** That's on
purpose. A `.lua` in `v3/apps/` with no manifest runs as built-in, so the
cart's code must never land before the line that makes it cart-level. If the
script fails to write after the manifest is in place, Load Cart says
"wrote manifest but not source -- not installed".

---

[← Getting started](01-getting-started.md) · [Contents](README.md) · [Next: The app lifecycle →](03-app-lifecycle.md)
