# 2. Apps and manifests

[← Getting started](01-getting-started.md) · [Contents](README.md) · [Next: The app lifecycle →](03-app-lifecycle.md)

## 2.1 The two files

Every app in `v3/apps/` is a pair sharing a base name:

```text
v3/apps/tetris.lua         the Lua source
v3/apps/tetris.app.toml    the manifest
```

The manifest is what makes the app *exist* as far as the OS is concerned. A
`.lua` with no manifest beside it is never registered and never launchable
from the Menu. A manifest with no matching `.lua` registers a path that fails to
load when picked. (`--app` and Terminal's `run` go through the manifest as well.)

A WASM cart is the same pair with `.wasm` in place of `.lua`; see
[§2.7](#27-wasm-carts).

## 2.2 The manifest format

Despite the extension, this is **not** real TOML. The kernel (and `desktop.lua`,
which reads the same format) trims each line, skips blank lines and lines
starting with `#`, splits at the **first** `=`, trims both sides, and keeps the
rest as a raw string. A line with no `=` is ignored. There are no quotes, no
arrays, no tables, and a `#` comment must be on a line of its own. If a key
appears twice, the **later** one wins.

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
| `name` | **yes** | Display name in the Menu, the taskbar and window lists. Keep it short: the Menu truncates at 22 characters, and `AcidApp`'s default window title is cut at 16. |
| `w` | **yes** | Window width in pixels, including the 1px border. 1 to 640. |
| `h` | **yes** | Window height in pixels, including the 16px title bar. 1 to 360. |
| `desc` | no | One line shown alongside the app. Has no effect on behaviour. |
| `menu` | no | `menu = false` hides the app from the Menu dropdown. It stays launchable by path (File Manager, `acid_spawn_app`, `--app`, Terminal's `run`). Anything other than the exact string `false`, including omitting the key, means visible. |
| `multi` | no | `multi = true` allows several windows of this app at once. Default is **singleton**: launching an already-open app raises and focuses the existing window instead of opening a second (a cart that launches it gets `false`, and nothing is raised). A built-in caller never raises a copy that a cart started: it opens a trusted window of its own instead. |
| `libs` | no | Comma-separated module paths to load into this app's VM before its own script. Relative to `v3/apps/`. |
| `source` | no | `source = cart` marks an app installed by Load Cart, and makes it run with cart-level trust ([§2.6](#26-built-in-and-cart-level-apps)). Only Load Cart should write this. |
| `runtime` | no | `runtime = wasm` says the app is `<name>.wasm` rather than `<name>.lua` ([§2.7](#27-wasm-carts)). Any other value, or no key, means Lua. |

If `name`, `w` or `h` is missing, or `w` or `h` is outside the screen, the
manifest is **silently skipped** and the app never appears in the Menu. A
manifest that fails to register does not stop the scan: the other apps still
register. So a missing app usually means a typo in its own manifest, not a
broken system.

### Sizing a window

The screen is **640×360**. `w`/`h` are the whole window:

```text
+--------------------------------------+  <- y = 0, 1px THEME_HARD border
| Title                             ●  |  <- title bar, 16px tall
+--------------------------------------+  <- y = 16, your area starts here
|                                      |
|          your drawing area           |     w - 2 usable width
|                                      |     h - 17 usable height
+--------------------------------------+
```

Usable content therefore runs from `(1, 16)` to `(w - 2, h - 2)`. In practice
apps draw from `(0, 16)` to `(w, h)` and let the clipping and the border
overdraw sort out the edges; see [§4.2](04-graphics.md#42-coordinates-and-clipping).

New windows cascade from the top left as more open, and the kernel clamps the
cascade so a window always lands fully on screen below the desktop strip. A
window bigger than the screen is not clamped: it is **refused**, and so is a
manifest that asks for one. At most **eight** windows can be open at once,
including the desktop's own; once they are all taken, a launch quietly fails.

### Keeping the constants in sync

The kernel gets `w`/`h` from the manifest; your Lua needs them too, for layout.
Nothing links the two, so every app in the tree declares them again and keeps
them matched by convention:

```lua snippet
local CounterApp = AcidApp:extend("CounterApp")
CounterApp.WINDOW_W = 180   -- must match `w =` in counter.app.toml
CounterApp.WINDOW_H = 120   -- must match `h =` in counter.app.toml
```

Get them out of step and your app draws to the wrong size. The canvas is the
manifest's size, and whatever your code draws outside it is clipped away, so the
symptom is content mysteriously cut off or a stripe of unpainted background, not
a crash.

## 2.3 Loading modules

Lua here has **no `require`**, and no `io`, `os`, `package` or `dofile`: the VM
is built with only the `string`, `table`, `math`, `utf8` and `coroutine`
libraries, and everything else an app can reach is an `acid_*` function. The VM
host loads files for you, in this exact order, before your script runs:

1. `v3/apps/lib/acid_keys.lua`: `AcidKeys` key codes
2. `v3/apps/lib/acid_palette.lua`: `AcidPalette.hue`
3. `v3/apps/lib/acid_waveform.lua`: `AcidWaveform` constants
4. `v3/apps/lib/acid_app.lua`: `AcidApp`
5. `v3/apps/lib/acid_game.lua`: `AcidGame`
6. everything in your manifest's `libs`, left to right
7. your own `<name>.lua`

**Those first five are always available.** You never list them in `libs`, and
listing one anyway is harmless but redundant (it is simply run a second time).
`libs` is for *extra* modules:

```toml
libs = lib/acid_sprite.lua, lib/acid_eggs.lua
```

Everything is loaded into one flat global namespace: a module defines a global
table (`AcidSprite = {}`) and your script uses it. A module you write for your
own app goes in `v3/apps/lib/` (or a subdirectory of `v3/apps/`, as Editor does
with `editor/buffer.lua`) and gets named in `libs`. A module that fails to
load, or does not exist, is logged to the terminal and skipped; the next module
and your own script still run, and your script then fails on the first use of
what was missing.

### What `libs` will not accept

Each entry is validated before loading. Rejected entries are skipped with
`Acid OS v3: rejected unsafe lib path <entry>` on stderr and the app still
starts:

- must end in `.lua`
- must be relative: a leading `/` is rejected
- no `\` anywhere
- no empty path components (`a//b.lua`, or a trailing `/`)
- no `..` as a whole component (`a/../../x.lua` is rejected; a file genuinely
  named `weird..name.lua` is fine)

## 2.4 How the launcher finds your app

At boot, `desktop.lua`:

1. lists `v3/apps/`, keeps every entry ending in `.app.toml`, and **sorts them**
   by file name;
2. parses each one, and for each manifest with `name`, `w` and `h` calls
   `acid_launcher_register(path, name, w, h, multi, libs)`, where `path` is the
   `.lua` (or, for `runtime = wasm`, the `.wasm`) beside the manifest;
3. records whether `menu ~= "false"`, which is what the dropdown filters on.

The registry is a fixed-size table of **48** entries. Registration is refused
once it is full, and it refuses the manifests that sort *last*, so if apps start
disappearing from the Menu, check the count before assuming your manifest is
wrong. The dropdown itself lists the first **ten** visible apps in registry
order; any further ones are registered, and launchable by path, but not shown.

`v3/fsroot/App` is a symlink to `v3/apps`. The registry matches launch paths by
exact string, so a path that arrives through the symlink matches nothing and
spawns a VM with none of its modules. Worse, a script started from
`v3/fsroot/...` is cart-level ([§2.6](#26-built-in-and-cart-level-apps)).
`AcidApp:canonical_app_path` converts one form to the other; use it whenever you
take a path from the filesystem and pass it to `acid_spawn_app`.

## 2.5 Carts

A **cart** is a whole app in one file, written outside the OS and carried in.
It is ordinary app Lua with a header comment block on top:

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

The **Load Cart** app (Menu → Load Cart) browses `v3/carts/`, `~/carts`, and the
host's `/media`, `/mnt` and `/run/media` mount points, so a cart on a USB stick
or an SD card appears in the same list (only the folders that exist are
offered). Installing writes `v3/apps/<slug>.lua` plus a generated
`v3/apps/<slug>.app.toml` carrying `source = cart`. `v3/carts/hello_acid.cart`
is the sample, and `v3/carts/README.txt` describes the format.

### The header

All five keys are optional. Only the **first comment block** in the file is
scanned: it ends at the first line that is neither blank nor a `--` comment, so
a `-- name:` further down, inside a comment among real code, is ignored. Keys
are matched in lower case, a line without a `:` is just a comment, and the
first value for a key wins.

| Key | Default if absent, and limits |
|---|---|
| `name` | derived from the filename (`my_game.cart` → "My Game"). At most 40 characters. |
| `w` | 220, clamped to 80–640 |
| `h` | 160, clamped to 48–336 |
| `desc` | empty. At most 40 characters. |
| `libs` | none. Only entries naming modules that exist in `v3/apps/lib/` (written `lib/<file>.lua`) are kept, and at most eight. |

Every value is cleaned: anything outside printable ASCII becomes a space, so a
header can never smuggle a second line into the generated manifest. A cart file
must be 1 byte to 256 KB. The slug (the installed file's base name) comes from
the filename: only lower-case letters and digits survive, runs of anything else
become a single `_`, it is at most 24 characters, and it can never contain a
path separator or a traversal component.

### What a cart may and may not overwrite

A cart can replace an app that was *itself* installed from a cart, which is what
`source = cart` in the generated manifest marks. It can never overwrite a
hand-written app in `v3/apps/`, including ones with no manifest at all (such as
`desktop.lua`): Load Cart's confirm screen reads "REFUSED: <slug> is a built-in
app", and if the install is attempted anyway it reports "that slot belongs to a
built-in app".

If the cart's *display name* matches an app that is already registered, Load
Cart warns before you install, because Terminal's `run` takes the first
case-insensitive match in registry order.

An installed cart joins the Menu at the **next boot**; Load Cart's own **RUN**
button starts it immediately in the meantime.

### Carts are sandboxed, but only lightly

Once installed, a cart runs at **cart level** ([§2.6](#26-built-in-and-cart-level-apps)):
smaller limits, no writes outside `Home`, and no say over other apps' windows.
That is a real fence, but a fence around a cart's *reach*, not around what it
may draw or play on its own window. The header validation stops a *malformed*
cart; the cart-level rules limit a hostile one. Read a cart before you install
it all the same.

An installed cart also shows up as a new untracked file in `git status`. Commit
it or delete it like any other file.

## 2.6 Built-in and cart-level apps

The kernel decides an app's **trust level** at the moment it starts it, and
the app cannot change it. There are two:

- **Built-in:** the script path starts with `v3/apps/` and ends in `.lua`, and
  the app's own manifest (the script path with `.lua` replaced by `.app.toml`)
  does not say `source = cart`. A missing manifest is built-in; that is why
  Load Cart writes the manifest *before* the script when it installs.
- **Cart-level:** everything else. That covers installed carts (their generated
  manifests say `source = cart`), any script started from `v3/fsroot/...`, any
  `.wasm` module (always, with no manifest lookup), and a script whose manifest
  exists but cannot be read. The key and value are matched leniently
  (`Source = "Cart"` counts), so a spelling trick does not buy trust back.
  Any app a cart starts, with `acid_spawn_app` or `acid_launcher_spawn`, is
  cart-level too, even `v3/apps/editor.lua`: a cart cannot borrow a built-in
  app's rights by launching it. A built-in caller never raises a copy of a singleton that a cart started: it opens a trusted window of its own instead.

The manifest is read when the app starts, not taken from the launcher registry,
so the level holds however the app was started: Menu, File Manager, Terminal's
`run`, Load Cart's RUN, `--app`.

### Limits

| | Lua memory | "Stopped responding" |
|---|---|---|
| Built-in | 64 MB | 2 s |
| Cart-level | 16 MB | 1 s |

The second limit measures the time since your app last returned from
`acid_poll_event` (or since it started, before its first poll). `AcidApp`'s loop
polls constantly, so it only matters if one callback computes for longer than
the limit. Either limit ends the app, and the terminal gets
`Acid OS v3: <path>: out of memory` or `Acid OS v3: <path>: stopped responding`.
WASM carts are limited differently ([chapter 10](10-wasm-carts.md)).

### What a cart is refused

Every refusal fails cleanly. The call returns the failure value and your app
carries on:

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

The ordinary path guard ([§7](07-system-apis.md)) still applies first. Reading
files, drawing, sound, your own window, launching installed apps that are not
already open with `acid_spawn_app` (they run cart-level, and only while fewer
than 4 cart-level windows are open), and `acid_set_volume` and `acid_set_wallpaper_enabled` all work
exactly as they do for a built-in app.

An app cannot close its own window with `acid_close_window` anyway: it ends its
run loop (`self:quit()`, [§3.4](03-app-lifecycle.md#34-focus-titles-and-quitting)).
So for a cart that call is simply always `false`.

## 2.7 WASM carts

A cart can also be a **WebAssembly module**, written in Rust (or anything that
targets WebAssembly) and run by the same kernel behind a stricter sandbox. It
installs exactly like a `.cart`, with these differences:

- The file is `.wasm`, and installs as `v3/apps/<slug>.wasm`. Its generated
  manifest carries **`runtime = wasm`** (before the final `source = cart` line),
  which is what tells the Menu, File Manager, Terminal's `run` and `--app` to
  run `<slug>.wasm` rather than `<slug>.lua`.
- It is **always cart-level**.
- Its header is not a comment block but a **custom section named `acid`** inside
  the module. The section's text is the same `key: value` lines as a `.cart`
  header, without the `-- ` prefix, and the same keys apply except that `libs`
  is ignored (a WASM module loads no Lua). A module with no `acid` section, or
  with a malformed one, installs under its filename at the default size
  (220×160) with no description: a hostile module can only lose its own
  metadata.
- The 256 KB size cap is the same.

The callback ABI, the import table, the fuel and memory limits and the
`acid-cart` crate are all in [chapter 10](10-wasm-carts.md); `v3/carts/hello_wasm.wasm`
is the sample. Here is only what the manifest side needs to know: a WASM app
is launched, sized and shown in the Menu from its `.app.toml`, exactly like a
Lua one.

## 2.8 Where an installed cart lands

An install writes a **slot**: the pair `<slug>.app.toml` plus one script,
`<slug>.lua` or `<slug>.wasm`. The rules:

- A slot is **occupied** if *either* `<slug>.lua` *or* `<slug>.wasm` already
  exists. This closes a hole: a `desktop.wasm` cart must not find the slot free
  just because only `desktop.lua` is there.
- An occupied slot with a manifest saying `source = cart` is a **replace**. The
  install writes the new manifest, then the new script, and then **removes the
  other runtime's script**, if there is one. Installing `snake.wasm` over an
  installed `snake.lua` cart leaves `snake.wasm` and deletes `snake.lua`, so
  the manifest's `runtime` line and the file that is there always agree.
- Anything else is **protected**, and Load Cart refuses it: an occupied slot
  whose manifest does not say `source = cart`, an occupied slot with no manifest
  at all (any `.lua` the loader did not install), and a leftover manifest with
  no script.
- A slot with neither a script nor a manifest is a **fresh** install.

The order (manifest first, script second) is deliberate. A `.lua` in `v3/apps/`
with no manifest runs as built-in, so the cart's code must never land before the
line that makes it cart-level. If the script write fails after the manifest
landed, Load Cart says "wrote manifest but not source -- not installed".

---

[← Getting started](01-getting-started.md) · [Contents](README.md) · [Next: The app lifecycle →](03-app-lifecycle.md)
