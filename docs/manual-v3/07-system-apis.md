# 7. System APIs

[← Games](06-games.md) · [Contents](README.md) · [Next: Cookbook →](08-cookbook.md)

Your app can do more than draw and play sound. It can also look at, and change,
the system around it:

- the window list
- the launcher registry (the list of installed apps)
- task and memory statistics
- the clock
- the network
- the master volume
- the file system

The Config, System Monitor, Network, File Manager, Terminal and desktop apps
are all built from these calls. Any app can use them, though carts get a few
extra rules, listed in [§7.11](#711-what-a-cart-is-refused).

Your app's Lua has no `io`, `os` or `package` library, and `dofile` and
`loadfile` are removed. Anything that reaches the outside world has to go
through an `acid_*` call. That's what lets the kernel check it.

## 7.1 Launching other apps

### By registry index

The launcher registry is built when the system boots, from the
`v3/apps/*.app.toml` files (see
[§2.4](02-apps-and-manifests.md#24-how-the-launcher-finds-your-app)). You can
walk through it like this:

```lua snippet
local count = acid_launcher_count()       -- number of registered apps
local name  = acid_launcher_name(index)   -- "System Monitor", or nil
local path  = acid_launcher_path(index)   -- "v3/apps/sysmon.lua", or nil
local ok    = acid_launcher_spawn(index)  -- true / false
```

Indexes are **zero-based**, so you walk from `0` to `count - 1`.

`acid_launcher_spawn` uses the width, height, `multi` flag and `libs` from the
app's manifest, so the app opens exactly as it would from the Menu.

It returns `true` when it works. That includes the case where the app is a
singleton that's already open: then the existing window is raised and focused
instead.

It returns `false` when:

- there's no app at that index, or
- the window couldn't be made, for example because all eight window slots are
  taken.

Carts get a few more `false` cases. A cart gets `false` for a singleton that's
already open (and nothing is raised), and while 4 or more cart-level windows
are open. Any app a cart starts also runs cart-level. See
[§7.11](#711-what-a-cart-is-refused).

Here's roughly how the Terminal's `run` command finds an app by name:

```lua snippet
function TerminalApp:cmd_run(args)
  local query = args[1]:lower()
  for i = 0, acid_launcher_count() - 1 do
    if acid_launcher_name(i):lower() == query then
      acid_launcher_spawn(i)
      return
    end
  end
  self.lines[#self.lines + 1] = "run: no app named " .. args[1]
end
```

The registry also includes apps hidden from the Menu with `menu = false`. So a
launcher you write yourself can reach every app on the system, not just the
visible ones.

The registry holds at most **48** apps. Once it's full,
`acid_launcher_register` returns `false`.

This complete app is a simple launcher. It lists every registered app and
starts the one you tap:

```lua app
-- w: 200
-- h: 150
local LauncherApp = AcidApp:extend("LauncherApp")

local ROW_H = 12
local MAX_ROWS = 9

function LauncherApp:on_create()
  self.names = {}
  for i = 0, acid_launcher_count() - 1 do
    self.names[#self.names + 1] = acid_launcher_name(i)
  end
  self.was_down = false
end

function LauncherApp:on_touch(x, y, pressed)
  -- Act once per press, not once per poll while the finger is held.
  if not pressed then self.was_down = false; return end
  if self.was_down then return end
  self.was_down = true
  local row = (y - 18) // ROW_H            -- zero-based, which is the registry index
  if row >= 0 and row < #self.names then
    acid_launcher_spawn(row)
  end
end

function LauncherApp:redraw()
  acid_clear_user_area()
  acid_draw_window_frame(self:window_title())
  if #self.names == 0 then
    acid_draw_text("NO APPS REGISTERED", 6, 22, 0x9DAAA3, 0x050607)
  end
  for i = 1, math.min(#self.names, MAX_ROWS) do
    acid_draw_text(self.names[i]:sub(1, 30), 6, 18 + (i - 1) * ROW_H + 2, 0xD4E6DB, 0x050607)
  end
  acid_draw_window_border()
end

LauncherApp:new():start()
```

### By path

```lua snippet
local ok = acid_spawn_app(path, w, h, arg)    -- true / false
```

This launches an app by its script path instead of its registry index.

The `multi` flag and `libs` still come from the registry, which is searched for
that exact path. So an app launched this way gets the same modules the Menu
would give it. If the path isn't in the registry, the app is treated as a
singleton with no modules.

`arg` is an optional startup string. For none, leave it out or pass `nil` or
`""`.

```lua snippet
-- Open the Editor on a specific file
acid_spawn_app("v3/apps/editor.lua", 420, 280, "v3/fsroot/Home/notes.txt")
```

It returns `false` when:

- the size is bigger than the screen
- all eight window slots are in use
- you're a cart and the path doesn't start with `v3/apps/`
  ([§7.11](#711-what-a-cart-is-refused))
- you're a cart and the app is a singleton that's already open. A built-in app
  would raise its window here, but a cart isn't allowed to
  ([§7.11](#711-what-a-cart-is-refused))
- you're a cart and 4 or more cart-level windows are open, counting your own
  ([§7.11](#711-what-a-cart-is-refused))
- you're a cart and your own window has been closed, even if you're still
  running ([§7.11](#711-what-a-cart-is-refused))

Any app a cart starts runs cart-level, even one under `v3/apps/` that would
normally be built-in.

A **bad path is not on that list.** The window is made first and the app is
started second. So if the path isn't a `.lua` or `.wasm` file under `v3/apps`
or `v3/fsroot`, you still get `true`. The new app then ends straight away,
taking its window with it, and logs:

`Acid OS v3: refused unsafe script path <path>`

If you're launching something a user typed, check the path with `acid_fs_size`
first.

> **Canonicalise paths that came from the file system.** `v3/fsroot/App` is a
> symlink to `v3/apps`. The registry and the singleton check both match paths
> as exact strings, so a path that came in through the symlink matches
> nothing. The app then opens with none of its modules, and a singleton can
> open a second time. Use the helper on `AcidApp`:
>
> ```lua snippet
> acid_spawn_app(self:canonical_app_path(path), w, h, "")
> ```
>
> It turns `v3/fsroot/App/x.lua` back into `v3/apps/x.lua` and leaves every
> other path alone.

### Reading your own launch argument

```lua snippet
function MyApp:on_create()
  local target = acid_launch_arg()          -- "" if launched without one
  if target ~= "" then self:load_file(target) end
end
```

You can read it as many times as you like. Reading it doesn't use it up.

### Singletons

By default an app is a **singleton**. If it's already open, launching it again
raises and focuses the existing window instead of opening a second one. The
match is made on the window's script path. Put `multi = true` in the manifest
to turn this off.

There are two exceptions:

- A cart can't raise another app's window this way. For a cart, launching an
  open singleton returns `false` and changes nothing.
- A built-in app never raises a copy of a singleton that a cart started.
  Instead it opens a trusted window of its own.

Editor, File Manager and Terminal are the apps that allow more than one window.

Remember that two windows of a `multi` app are **two separate Lua VMs**, each
on its own OS thread. They share no globals, tables or variables, and neither
can see the other. So if something must hold system-wide, like "only one of
these animations at a time", the kernel has to enforce it. Your Lua can't. See
[§4.5](04-graphics.md#45-the-overlay).

## 7.2 The window list

```lua snippet
acid_window_max()                -- capacity of the window table (8)
acid_window_info(index)          -- name, x, y, w, h, focused  (or nothing)
acid_activate_window(index)      -- raise and focus that window
acid_close_window(index)         -- true / false
```

`acid_window_info` returns **six values**, or nothing for an empty slot. So walk
the whole range and skip the gaps. Indexes are zero-based, from `0` to
`acid_window_max() - 1`:

```lua snippet
local function active_windows()
  local out = {}
  for i = 0, acid_window_max() - 1 do
    local name, x, y, w, h, focused = acid_window_info(i)
    if name then
      out[#out + 1] = { index = i, name = name, x = x, y = y, w = w, h = h, focused = focused }
    end
  end
  return out
end
```

The six values are:

- the window's **name**, which is the script path the app was started from
  (`"v3/apps/sysmon.lua"`). The singleton check compares this too.
- its screen position, `x` and `y`
- its size, `w` and `h`
- whether it has focus right now

A negative or out-of-range index returns nothing. It never raises an error.

`acid_close_window` closes **another** app's window and returns `true`. It
won't close your own window: it returns `false` for that, and for an empty or
out-of-range slot. A monitor that closes itself from its own window list is a
confusing way to quit. Use the title-bar close button or `self:quit()`
instead. System Monitor's windows page uses this call, behind a two-tap
confirmation.

`acid_activate_window` raises and focuses the window at that index. An empty or
out-of-range index does nothing.

`acid_send_self_to_back` drops your own window behind all the others. It
only ever moves your window, so you can't send someone else's window back. The
desktop uses it to tidy up after closing its dropdown, because it raises itself
for a moment to show the dropdown.

A **cart** can't close any window, and can only raise its own. See
[§7.11](#711-what-a-cart-is-refused).

This complete app is a small window list. It shows every window and raises the
one you tap:

```lua app
-- w: 220
-- h: 150
local WindowListApp = AcidApp:extend("WindowListApp")

function WindowListApp:poll_timeout_ms()
  return 500
end

function WindowListApp:on_create()
  self.rows = {}      -- window index for each row on screen
  self.was_down = false
end

function WindowListApp:on_idle()
  self:redraw()
end

function WindowListApp:on_touch(x, y, pressed)
  if not pressed then self.was_down = false; return end
  if self.was_down then return end
  self.was_down = true
  local idx = self.rows[(y - 18) // 12 + 1]
  if idx then acid_activate_window(idx) end
end

function WindowListApp:redraw()
  acid_clear_user_area()
  acid_draw_window_frame(self:window_title())
  self.rows = {}
  local y = 20
  for i = 0, acid_window_max() - 1 do
    local name, wx, wy, w, h, focused = acid_window_info(i)
    if name then
      self.rows[#self.rows + 1] = i
      local label = (focused and "> " or "  ") .. name:gsub("^v3/apps/", "")
      acid_draw_text(label:sub(1, 34), 4, y, focused and 0x00FF66 or 0xD4E6DB, 0x050607)
      y = y + 12
    end
  end
  acid_draw_window_border()
end

WindowListApp:new():start()
```

## 7.3 Focus

```lua snippet
local focused = acid_am_i_focused()    -- true / false
```

`AcidApp:focused()` wraps this call. In Acid OS, having focus and being the top
window always go together. So this also tells you "is my window the one
actually showing on top?" See
[§6.3](06-games.md#63-focus-and-the-z-order-trap) for why a game needs to check
it.

## 7.4 Tasks and memory

```lua snippet
local n = acid_refresh_tasks()     -- sample the task table; returns the task count
local n = acid_task_count()        -- tasks in the last sample
local name, state, cpu = acid_task_info(index)   -- or nothing
local kb = acid_mem_used_kb()      -- kilobytes in use, or -1
```

`acid_refresh_tasks` takes a snapshot, and `acid_task_info` reads from that
snapshot using a zero-based index. So refresh first, then walk:

```lua snippet
local function sample()
  local rows = {}
  for i = 0, acid_refresh_tasks() - 1 do
    local name, state, cpu = acid_task_info(i)
    if name then rows[#rows + 1] = { name = name, state = state, cpu = cpu } end
  end
  return rows
end
```

When Acid OS runs on Linux (the "hosted" build), a task is an **OS thread of
the Acid OS process**. That means the kernel's own threads, plus one for each
running app. Worth knowing:

- `name` is the thread name. Linux **cuts thread names to 15 characters**, so
  `v3/apps/sysmon.lua` arrives as `v3/apps/sysmon.`. System Monitor's
  `short_name` strips that trailing dot.
- `state` is one of these strings: `"running"`, `"blocked"`, `"suspended"`,
  `"deleted"` or `"?"`.
- `cpu` is a whole-number percentage: the thread's CPU time since the last
  refresh, divided by the real time since the last refresh. It's **0** on the
  first sample, and for any task that wasn't there last time. So sample at
  least twice, a second or so apart.
- Only the **first 16** threads are tracked.
- `acid_mem_used_kb` is the memory used by the **whole Acid OS process**, not
  just your app. Every app shares that number. It returns `-1` when the
  platform can't tell, so any check you write should handle `-1` too.

Reading these once a second is cheap. Reading them every frame isn't.

## 7.5 Compositor and audio statistics

```lua snippet
acid_composited_frames()    -- frames the compositor has painted
acid_skipped_frames()       -- frames it skipped because nothing was dirty
acid_active_voice_count()   -- voices with a sounding envelope, 0-8
```

These are here so a system monitor can show off what makes Acid OS different:
its own compositor (the part that paints windows to the screen) and its own
synthesiser.

They're also the quickest way to answer two common questions:

- "Is my app repainting far more than it needs to?" Watch
  `acid_composited_frames` climb while nothing on screen moves.
- "Did I leave a note playing?" Watch whether `acid_active_voice_count` goes
  back to zero.

## 7.6 Network

```lua snippet
local hostname, ip, connected = acid_network_info()
```

This returns **three values**: the host name, an IP address string and a
boolean.

On the hosted build:

- Normally you get the machine's host name, the **first non-loopback IPv4
  address** on any network interface as a dotted string (`"192.168.1.20"`),
  and `true`.
- If there's no such address, you get the host name, the string `"none"` and
  `false`.
- If the platform can't tell at all, you get `"unknown"`, `"none"` and
  `false`.

`connected` only means **an address was found**. It doesn't mean anything is
reachable. There's no live connection check, no sockets and no HTTP client.
This is information for a status display, not a networking API.

## 7.7 Master volume and the wallpaper

```lua snippet
acid_set_volume(percent)           -- 0-100, clamped
local v = acid_get_volume()        -- 0-100
acid_set_wallpaper_enabled(true)
local on = acid_get_wallpaper_enabled()   -- true / false
```

Both of these are **system-wide settings that belong to the Config app**. Read
them as much as you like, but think twice before changing them.

An app that turns the user's volume down because it's loud, or turns their
wallpaper off because it wants a plain background, is misbehaving. Scale your
own note volumes and draw your own background instead.

Carts can still use both calls. These are the user's settings to protect, so
the kernel doesn't block them.

## 7.8 The file system

There's no `io` library here, so apps use the calls below instead.

Any call that can fail reports it by returning **`nil, message`**. It doesn't
raise an error. So always check the first value:

```lua snippet
local names = acid_fs_list(dir)           -- a sequence of names, or nil, err
local text  = acid_fs_read(path)          -- the file's bytes as a string, or nil, err
local size  = acid_fs_size(path)          -- bytes, or nil, err
local ok    = acid_fs_write(path, data)   -- true, or nil, err
local ok    = acid_fs_rename(from, to)    -- true, or nil, err
local ok    = acid_fs_delete(path)        -- true, or nil, err
```

- `acid_fs_list` returns the names in the folder, **sorted**, without `.` and
  `..`. It doesn't say which are folders. `acid_fs_size` returns a number for
  a folder too, so it can't tell you either. To find out, list the path and
  see whether it answers.
- `acid_fs_read` returns the whole file as one Lua string. The string can hold
  any bytes, not just text.
- `acid_fs_write` creates the file, or replaces it completely. There's no
  append. To add to a file, read it, join the new part on, and write it back.
- `acid_fs_rename` moves a file or folder. If a file is already at the target,
  it's replaced.
- `acid_fs_delete` deletes **files only**.
- To ask "does it exist?", use `acid_fs_size(path) ~= nil`. That's true for a
  folder as well.

### The path guard

Every path goes through one check **before** anything touches the disk. A path
is only accepted if it:

- is under one of the two roots, **`v3/apps`** or **`v3/fsroot`**. It must be
  exactly that root or `root/...`, so `v3/appsx` doesn't count.
- is relative, with no leading `/`
- has no **empty parts** (`v3/apps//x`, or a trailing `/`), no **`..` parts**
  and no **`.` parts** (`v3/apps/./x.lua`, `./v3/apps`)
- has no NUL characters and no backslashes

Anything else gets `nil, "bad path"`. That includes the Rust source in
`v3/crates`, `v3/carts`, absolute paths and anything with `..` in it.

Paths are relative to the top folder of the repository. So Acid OS has to be
started from there, just like the simulator
([§1.1](01-getting-started.md#11-build-and-run)). The guard only looks at the
text of the path, so it works the same on every platform.

### Error messages

The messages are plain strings that you can show to the user or compare
against:

| Message | Meaning |
|---|---|
| `bad path` | The path guard refused it, **or** (for a write, rename or delete) the real location is not inside a root, or its parent directory does not exist |
| `not found` | Nothing at that path |
| `read only` | A cart tried to change something outside `v3/fsroot/Home/` ([§7.11](#711-what-a-cart-is-refused)) |
| `not allowed` | A cart called one of the host cart folder calls (below) |
| `is a directory` | `acid_fs_delete` on a directory |
| `too big` | A host cart file over 256 KB |
| anything else | The operating system's own text, such as `Not a directory (os error 20)` from listing a file, or `Is a directory (os error 21)` from reading one |

In short:

- `not found` just means nothing is there.
- `bad path` and `read only` mean you broke a rule.
- Anything else is worth showing to the user.

### Writes check where a path really points

Reads only get the text check above. A **write, rename or delete** also works
out where the path *really* leads, following any symlinks. It looks at the file
itself if it exists, or at its parent folder if it doesn't. The change is
refused unless that real location is inside the real `v3/apps` or the real
`v3/fsroot`.

What this means in practice:

- `v3/fsroot/App` is a symlink to `v3/apps`. Writing to `v3/fsroot/App/x.lua`
  **works** and lands in `v3/apps/x.lua`, because it really points into the
  apps folder.
- A symlink to anywhere outside the two roots is refused with `bad path`. So
  writes can't escape through a symlink.
- The parent folder has to exist to be checked. So **writing into a folder that
  doesn't exist fails** with `bad path`. There's no call to make a folder.
- `acid_fs_rename` won't move a root itself. It also refuses if either end is a
  symlink, because it would move the link, not the thing it points at.
- Reads follow symlinks freely, so `acid_fs_list("v3/fsroot/App")` lists the
  apps.

So a built-in app can write anywhere under the two roots, `v3/apps` included.
Please don't. Keep your own data under **`v3/fsroot/Home`**. A cart can't
write anywhere else anyway.

### The layout

`v3/fsroot/` is Acid OS's own file system root. You can browse it in File
Manager:

| Path | Holds |
|---|---|
| `v3/fsroot/Home` | The user's files |
| `v3/fsroot/App` | Symlink to `v3/apps` |
| `v3/fsroot/Lib` | Symlink to `v3/apps/lib`, the shared Lua libraries |
| `v3/fsroot/Help` | Help text |
| `v3/fsroot/Tmp` | Temporary files; anything here can vanish |

On the hosted build these are real folders, relative to the top of the
repository.

This complete app counts how many times it's been launched, keeping the count
in a file under Home. That's how an app should store a setting. If the write is
refused, it shows the error:

```lua app
-- w: 200
-- h: 100
local CounterApp = AcidApp:extend("CounterApp")

local FILE = "v3/fsroot/Home/launch_count.txt"

function CounterApp:on_create()
  local text = acid_fs_read(FILE)             -- nil, "not found" the first time
  self.count = (tonumber(text) or 0) + 1
  local ok, err = acid_fs_write(FILE, tostring(self.count))
  self.status = ok and "SAVED" or ("NOT SAVED: " .. tostring(err))
end

function CounterApp:redraw()
  acid_clear_user_area()
  acid_draw_window_frame(self:window_title())
  acid_draw_text("LAUNCHES: " .. self.count, 10, 30, 0xD4E6DB, 0x050607)
  acid_draw_text(self.status:sub(1, 30), 10, 46, 0x9DAAA3, 0x050607)
  acid_draw_window_border()
end

CounterApp:new():start()
```

### Host cart folders

There are four more calls, made for the **Load Cart** app. Only built-in apps
can use them. A cart that calls them gets `nil, "not allowed"`.

```lua snippet
local roots = acid_cart_roots()            -- the cart folders that exist, or nil, err
local names = acid_cart_list(dir)          -- sorted names (files and directories), or nil, err
local kind, size = acid_cart_stat(path)    -- "dir" or "file", and bytes; or nil, err
local text = acid_cart_read(path)          -- up to 256 KB, or nil, err
```

They're read-only, and they look **outside** Acid OS's two roots, in these
folders:

- `v3/carts`
- `$HOME/carts`
- `/media`, `/mnt` and `/run/media`

They never follow a symlink out of those folders, and they leave symlinks out
of listings. Reading a file over 256 KB gives `nil, "too big"`.

You'll only need these if you're writing a replacement for Load Cart
([§2.5](02-apps-and-manifests.md#25-carts)).

## 7.9 The clock

```lua snippet
local year, month, day, hour, min, sec = acid_local_time()
local ms = acid_now_ms()
```

There are two clocks, for two different jobs.

`acid_local_time()` is the **local time of day**, as six whole numbers: year,
month (1 to 12), day, hour (0 to 23), minute and second. The desktop clock uses
it. On a platform with no clock, you get `1970, 1, 1, 0, 0, 0`.

`acid_now_ms()` is **milliseconds since the platform started**. It only ever
goes forward. Use it for anything you measure: animation timing, doing
something "once a second", how long a button was held. `AcidGame` uses it to
pace its ticks.

Don't use `acid_local_time` for measuring. It can jump when the user changes
the time.

This complete app shows both:

```lua app
-- w: 200
-- h: 100
local ClockApp = AcidApp:extend("ClockApp")

function ClockApp:poll_timeout_ms()
  return 250
end

function ClockApp:on_idle()
  self:redraw()
end

function ClockApp:redraw()
  acid_clear_user_area()
  acid_draw_window_frame(self:window_title())
  local _, month, day, hour, min, sec = acid_local_time()
  acid_draw_text(string.format("%02d:%02d:%02d", hour, min, sec), 10, 30, 0x00FF66, 0x050607)
  acid_draw_text(string.format("%02d/%02d", day, month), 10, 44, 0xD4E6DB, 0x050607)
  acid_draw_text("UP " .. acid_now_ms() // 1000 .. " S", 10, 60, 0x9DAAA3, 0x050607)
  acid_draw_window_border()
end

ClockApp:new():start()
```

## 7.10 Limits

Every app has a memory limit and a "stopped responding" limit. Both depend on
whether the app is built-in or cart-level
([§2.6](02-apps-and-manifests.md#26-built-in-and-cart-level-apps)):

| | Lua memory | "Stopped responding" |
|---|---|---|
| Built-in | 64 MB | 2 s |
| Cart-level | 16 MB | 1 s |

**Memory.** The limit covers everything your app's Lua VM allocates. That
**includes the core libraries and the `libs` from your manifest**, which are
loaded before your code. If an allocation would go over the limit, it fails and
the app ends.

**Stopped responding.** Every 10,000 Lua instructions, the kernel checks how
long it's been since your app last came back from `acid_poll_event` (or since
it started, if it hasn't polled yet). If that's longer than the limit, a Lua
error is raised.

`AcidApp` and `AcidGame` poll all the time, so this only bites when a single
callback keeps working for longer than the limit. A long poll *timeout* never
trips it, because the clock restarts when the poll returns. If your app has
something slow to do, split it into slices across several callbacks.

**What happens at a limit.** The app ends, its window is removed and its
resources are freed. The terminal gets one line:

```text
Acid OS v3: <path>: out of memory
Acid OS v3: <path>: stopped responding
```

**Your app can't catch either one.** `pcall`, `xpcall` and `coroutine.resume`
all pass a timeout or out-of-memory error straight back up instead of handing
it to you. Once a limit has tripped, the error handler of an `xpcall` is
skipped too. So a script can't swallow its own watchdog. Other apps aren't
affected, because each one is its own VM on its own thread.

A few other doors a runaway script might try are also closed:

- `string.dump` is gone.
- `load` only reads text.
- `setmetatable` refuses a metatable with `__gc`.
- `table.insert`, `table.remove` and `table.move` refuse a table or range of
  more than 16,777,216 entries.
- Repeating an empty string costs nothing.

A **WASM** cart has different limits: fuel and linear-memory pages, rather than
Lua memory and a clock. See [chapter 10](10-wasm-carts.md).

## 7.11 What a cart is refused

Everything above applies to cart-level apps too, apart from the exceptions
below. Each refusal fails cleanly: the call returns its failure value and the
app carries on.

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

The path guard runs first. So a badly formed path gets `bad path` even from a
cart, and `read only` is what a *well-formed* path outside Home gets.

The `Home/` rule matches on what the path starts with. That means a cart can
change what's inside `v3/fsroot/Home`, but not the `v3/fsroot/Home` folder
itself.

Everything else works for a cart exactly as it does for a built-in app:

- reading files
- drawing on its own window
- sound
- the clock
- task and network information
- `acid_launcher_spawn` and `acid_spawn_app`, for installed apps that aren't
  already open. The new app runs cart-level, and only while fewer than 4
  cart-level windows are open.
- `acid_set_volume` and `acid_set_wallpaper_enabled`

The overlay drawing calls and `acid_repaint_region` can still be called too.
They only act for the task that holds the overlay, or only on the caller's own
window, and a cart never holds the overlay.

A cart's own window closes when its run loop ends, so a cart never needs
`acid_close_window`. That's why it simply always returns `false`.

---

[← Games](06-games.md) · [Contents](README.md) · [Next: Cookbook →](08-cookbook.md)
