# 7. System APIs

[← Games](06-games.md) · [Contents](README.md) · [Next: Cookbook →](08-cookbook.md)

Beyond drawing and sound, an app can inspect and manipulate the running system:
the window list, the launcher registry, task and memory statistics, the clock,
the network, the master volume and the file system. These are what the Config,
System Monitor, Network, File Manager, Terminal and desktop apps are built out
of, and they are available to any app, subject to the cart-level rules in
[§7.11](#711-what-a-cart-is-refused).

The Lua standard library an app sees has no `io`, `os` or `package`, and
`dofile` and `loadfile` are removed. Everything that touches the outside world
goes through an `acid_*` call, which is what lets the kernel check it.

## 7.1 Launching other apps

### By registry index

The launcher registry is built at boot from `v3/apps/*.app.toml` (see
[§2.4](02-apps-and-manifests.md#24-how-the-launcher-finds-your-app)). You can
walk it:

```lua snippet
local count = acid_launcher_count()       -- number of registered apps
local name  = acid_launcher_name(index)   -- "System Monitor", or nil
local path  = acid_launcher_path(index)   -- "v3/apps/sysmon.lua", or nil
local ok    = acid_launcher_spawn(index)  -- true / false
```

Indexes are **zero-based**: walk `0` to `count - 1`.
`acid_launcher_spawn` uses the width, height, `multi` flag and `libs` recorded
in the manifest, so the app opens exactly as it would from the Menu. It returns
`true` on success, including when the app was a singleton and already open, in
which case the existing window is raised and focused instead. It returns
`false` for an index with no entry, or when the window could not be made (all
eight window slots are taken, for one). A cart gets `false` for an open
singleton (nothing is raised) and while 4 or more cart-level windows are open,
and an app a cart starts runs cart-level
([§7.11](#711-what-a-cart-is-refused)).

This is the shape of the Terminal's `run` command, which finds an app by name:

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

The registry includes apps hidden from the Menu with `menu = false`, so a
launcher of your own can reach every app on the system, not just the visible
ones. It holds at most **48** apps; `acid_launcher_register` returns `false`
once it is full.

This complete app is a launcher of exactly that kind. It lists every registered
app and starts the one you tap:

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

Launch by script path instead of registry index. The `multi` flag and `libs`
still come from the registry (looked up by exact path), so an app launched this
way gets its modules exactly as the Menu would give them. A path with no
registry entry defaults to singleton and no modules. `arg` is an optional
startup string; leave it out, pass `nil` or pass `""` for none.

```lua snippet
-- Open the Editor on a specific file
acid_spawn_app("v3/apps/editor.lua", 420, 280, "v3/fsroot/Home/notes.txt")
```

What `false` means, exactly:

- The size is not 1 to 640 wide and 1 to 360 high.
- All eight window slots are in use.
- You are a cart and the path does not start with `v3/apps/`
  ([§7.11](#711-what-a-cart-is-refused)).
- You are a cart and the app is a singleton that is already open: a built-in
  app would raise its window, a cart may not ([§7.11](#711-what-a-cart-is-refused)).
- You are a cart and 4 or more cart-level windows are open, your own included
  ([§7.11](#711-what-a-cart-is-refused)).
- You are a cart and your own window has been closed, even if you are still
  running ([§7.11](#711-what-a-cart-is-refused)).

An app a cart starts runs cart-level, even one under `v3/apps/` that would
otherwise be built-in.

A **bad path is not one of them.** The window is created first and the app
started second, so a path that is neither a `.lua` nor a `.wasm` file under
`v3/apps` or `v3/fsroot` returns `true` and the new app then ends at once, logging
`Acid OS v3: refused unsafe script path <path>` and taking its window with it.
Check a path with `acid_fs_size` first if you are launching something a user
typed.

> **Canonicalise paths that came from the filesystem.** `v3/fsroot/App` is a
> symlink to `v3/apps`, and the registry and the singleton check both match by
> exact string. A path that arrived through the symlink matches nothing: the app
> spawns with none of its modules, and a singleton opens a second time. Use the
> helper on `AcidApp`:
>
> ```lua snippet
> acid_spawn_app(self:canonical_app_path(path), w, h, "")
> ```
>
> It maps `v3/fsroot/App/x.lua` back to `v3/apps/x.lua` and leaves every other
> path alone.

### Reading your own launch argument

```lua snippet
function MyApp:on_create()
  local target = acid_launch_arg()          -- "" if launched without one
  if target ~= "" then self:load_file(target) end
end
```

Safe to read more than once; it is plain context state, not consumed.

### Singletons

By default an app is a **singleton**: launching one that is already open raises
and focuses the existing window instead of spawning a second. The match is on
the window's script path. `multi = true` in the manifest opts out. A cart
cannot raise another app's window this way: for a cart, launching an open
singleton returns `false` and changes nothing. A built-in caller never raises a copy of a singleton that a cart started: it opens a trusted window of its own instead. Editor, File
Manager and Terminal are the multi-window apps in the tree.

Remember that two windows of a `multi` app are **two separate Lua VMs**, each on
its own OS thread. They share no globals, no tables and no variables, and
neither can see the other. Anything that must be true system-wide, such as "only
one of these animations at a time", has to be enforced by the kernel, not by
your Lua. See [§4.5](04-graphics.md#45-the-overlay).

## 7.2 The window list

```lua snippet
acid_window_max()                -- capacity of the window table (8)
acid_window_info(index)          -- name, x, y, w, h, focused  (or nothing)
acid_activate_window(index)      -- raise and focus that window
acid_close_window(index)         -- true / false
```

`acid_window_info` returns **six values**, or nothing for an empty slot, so walk
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

The values are the window's **name**, its screen coordinates, its size, and
whether it currently holds focus. The name is the script path the app was
started from (`"v3/apps/sysmon.lua"`), which is also what the singleton check
compares. A negative or out-of-range index returns nothing, never an error.

`acid_close_window` closes **another** app's window and returns `true`. It
refuses to close the calling app's own window and returns `false`, as it does
for an empty or out-of-range slot. A monitor ending itself from its own window
list is a confusing way to quit; the title-bar close button, or `self:quit()`,
is the normal route. System Monitor's windows page uses this call, behind a
two-tap confirmation.

`acid_activate_window` raises and focuses the window at that index. An empty or
out-of-range index does nothing.

`acid_send_self_to_back` drops your own window to the back of the z-order. It
always targets the caller, so there is no way to send someone else's window
back. The desktop uses it when closing its dropdown, having temporarily raised
itself to show it.

A **cart** cannot close any window, and can raise only its own: see
[§7.11](#711-what-a-cart-is-refused).

This complete app is a small window manager's list. It shows every window and
raises the one you tap:

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

`AcidApp:focused()` wraps this. In this OS, focus and being the topmost window
always change together, so it doubles as "am I the window actually visible on
top". See [§6.3](06-games.md#63-focus-and-the-z-order-trap) for why a game must
check it.

## 7.4 Tasks and memory

```lua snippet
local n = acid_refresh_tasks()     -- sample the task table; returns the task count
local n = acid_task_count()        -- tasks in the last sample
local name, state, cpu = acid_task_info(index)   -- or nothing
local kb = acid_mem_used_kb()      -- kilobytes in use, or -1
```

`acid_refresh_tasks` takes a snapshot; `acid_task_info` reads from it, with a
zero-based index. Call refresh first, then walk:

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

On the hosted build a "task" is an **OS thread of the OS process**: the kernel's
own threads and one per running app. The details worth knowing:

- `name` is the thread name, which Linux **truncates to 15 characters**, so
  `v3/apps/sysmon.lua` arrives as `v3/apps/sysmon.`. System Monitor's
  `short_name` strips that trailing dot.
- `state` is a string: `"running"`, `"blocked"`, `"suspended"`, `"deleted"` or
  `"?"`.
- `cpu` is an integer percentage: the thread's CPU time since the previous
  refresh, over the wall time since the previous refresh. It is **0** on the
  first sample and for a task not seen last time, so sample at least twice,
  a second or so apart.
- Only the **first 16** threads are tracked.
- `acid_mem_used_kb` is the resident size of the **whole OS process**, not of
  your app: every app shares it. It returns `-1` when the platform cannot tell,
  and so should any check you write for it.

An app that reads these once per second is cheap; reading them every frame is
not.

## 7.5 Compositor and audio statistics

```lua snippet
acid_composited_frames()    -- frames the compositor has painted
acid_skipped_frames()       -- frames it skipped because nothing was dirty
acid_active_voice_count()   -- voices with a sounding envelope, 0-8
```

Exposed for a system monitor to show what is actually distinctive about this
build: its own compositor, its own synthesiser. They are also the fastest way
to answer "is my app repainting far more than it needs to?" (watch
`acid_composited_frames` climb while nothing moves) and "did I leave a note
gated on?" (watch `acid_active_voice_count` fail to return to zero).

## 7.6 Network

```lua snippet
local hostname, ip, connected = acid_network_info()
```

Returns **three values**: the host name, an IP address string and a boolean.

On the hosted build the host name is the machine's, and the address is the
**first non-loopback IPv4 address** found on any interface, as a dotted string
(`"192.168.1.20"`), with `connected` `true`. If there is none, you get the host
name, the string `"none"` and `false`. If the platform cannot say at all, the
values are `"unknown"`, `"none"` and `false`.

`connected` means **an address was found**, not that anything is reachable.
There is no live reachability check, no sockets, no HTTP client. This is
information for a status display, not a networking API.

## 7.7 Master volume and the wallpaper

```lua snippet
acid_set_volume(percent)           -- 0-100, clamped
local v = acid_get_volume()        -- 0-100
acid_set_wallpaper_enabled(true)
local on = acid_get_wallpaper_enabled()   -- true / false
```

Both are **system-wide settings that the Config app owns**. Read them freely;
think twice before writing them. An app that turns the user's volume down
because it is loud, or switches their wallpaper off because it wants a plain
background, is misbehaving. Scale your own note volumes and draw your own
background instead. (Both calls stay available to carts: the user's settings are
the user's to protect, not the kernel's.)

## 7.8 The file system

Lua has no `io` library here, so apps use the file system calls below. Every call that
can fail reports it by returning **`nil, message`** rather than raising an error, so you check the first value:

```lua snippet
local names = acid_fs_list(dir)           -- a sequence of names, or nil, err
local text  = acid_fs_read(path)          -- the file's bytes as a string, or nil, err
local size  = acid_fs_size(path)          -- bytes, or nil, err
local ok    = acid_fs_write(path, data)   -- true, or nil, err
local ok    = acid_fs_rename(from, to)    -- true, or nil, err
local ok    = acid_fs_delete(path)        -- true, or nil, err
```

- `acid_fs_list` returns the entry names **sorted**, without `.` and `..`, and
  does not say which are directories. `acid_fs_size` returns a number for a
  directory too, so it cannot tell you either; list the path and see whether it
  answers.
- `acid_fs_read` returns the whole file as one Lua string, which may hold any
  bytes.
- `acid_fs_write` creates the file or replaces it entirely. There is no append:
  read, join and write back.
- `acid_fs_rename` moves a file or directory; an existing file at the target is
  replaced.
- `acid_fs_delete` deletes **files only**.
- "Does it exist?" is `acid_fs_size(path) ~= nil`, true for a directory as
  well.

### The path guard

Every path passes through one check **before** anything touches the disk. A path
is accepted only if it is:

- under one of the two roots, **`v3/apps`** or **`v3/fsroot`**: either exactly
  that root or `root/...` (so `v3/appsx` is out);
- relative: no leading `/`;
- free of **empty segments** (`v3/apps//x`, or a trailing `/`), **`..`
  segments** and **`.` segments** (`v3/apps/./x.lua`, `./v3/apps`);
- free of NUL characters and backslashes.

Anything else returns `nil, "bad path"`: the crates, `v3/carts`, an absolute
path, a path with `..` in it. Paths are relative to the repository root, so the
process's working directory has to be the repository root, as for the simulator
([§1.1](01-getting-started.md#11-build-and-run)). The guard is a text check, so
it is the same on every platform.

### Error messages

The messages are plain strings you can show or compare:

| Message | Meaning |
|---|---|
| `bad path` | The path guard refused it, **or** (for a write, rename or delete) the real location is not inside a root, or its parent directory does not exist |
| `not found` | Nothing at that path |
| `read only` | A cart tried to change something outside `v3/fsroot/Home/` ([§7.11](#711-what-a-cart-is-refused)) |
| `not allowed` | A cart called one of the host cart folder calls (below) |
| `is a directory` | `acid_fs_delete` on a directory |
| `too big` | A host cart file over 256 KB |
| anything else | The operating system's own text, such as `Not a directory (os error 20)` from listing a file, or `Is a directory (os error 21)` from reading one |

So the portable reading is: `not found` is an ordinary absence, `bad path` and
`read only` are rules, and anything else is worth showing to the user.

### Writes check where a path really points

Reads only run the text check above. A **write, rename or delete** also resolves
where the path *really* leads (the file itself if it exists, otherwise its parent
directory) and refuses unless that lies inside the real `v3/apps` or the real
`v3/fsroot`:

- `v3/fsroot/App` is a symlink to `v3/apps`. Writing to
  `v3/fsroot/App/x.lua` **works** and lands in `v3/apps/x.lua`, because its real
  target is the apps directory.
- A symlink to anywhere outside the two roots is refused with `bad path`: writes
  can't escape through one.
- Because the parent has to exist to be resolved, **writing into a directory
  that does not exist fails** with `bad path`. There is no call to make a
  directory.
- `acid_fs_rename` refuses to move a root itself, and refuses when either end is
  a symlink (it would move the link, not what it points at).
- Reads follow symlinks freely: `acid_fs_list("v3/fsroot/App")` lists the apps.

A built-in app may therefore write anywhere under the two roots, including
`v3/apps`. Do not: write your own data under **`v3/fsroot/Home`**. A cart can
write nowhere else.

### The layout

`v3/fsroot/` is the OS's own file system root, browsable from File Manager:

| Path | Holds |
|---|---|
| `v3/fsroot/Home` | The user's files |
| `v3/fsroot/App` | Symlink to `v3/apps` |
| `v3/fsroot/Lib` | Symlink to `v3/apps/lib`, the shared Lua libraries |
| `v3/fsroot/Help` | Help text |
| `v3/fsroot/Tmp` | Temporary files; anything here can vanish |

On the hosted build these are real directories relative to the repository root.

This complete app keeps a launch counter in a file under Home, the way an app
should store a setting, and shows the error if the write is refused:

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

Four more calls exist for **Load Cart**, and only for built-in apps. A cart
calling them gets `nil, "not allowed"`:

```lua snippet
local roots = acid_cart_roots()            -- the cart folders that exist, or nil, err
local names = acid_cart_list(dir)          -- sorted names (files and directories), or nil, err
local kind, size = acid_cart_stat(path)    -- "dir" or "file", and bytes; or nil, err
local text = acid_cart_read(path)          -- up to 256 KB, or nil, err
```

They are read only and look **outside** the OS's two roots, at `v3/carts`,
`$HOME/carts`, and `/media`, `/mnt` and `/run/media`. They never follow a
symlink out of those folders, and leave symlinks out of listings. Reading a file
over 256 KB gives `nil, "too big"`. You will only need them if you write a
replacement for Load Cart ([§2.5](02-apps-and-manifests.md#25-carts)).

## 7.9 The clock

```lua snippet
local year, month, day, hour, min, sec = acid_local_time()
local ms = acid_now_ms()
```

Two clocks, for two jobs.

`acid_local_time()` is the **wall-clock local time**, six integers: year, month
(1 to 12), day, hour (0 to 23), minute and second. The desktop's clock reads it.
A platform with no clock reports the epoch, `1970, 1, 1, 0, 0, 0`.

`acid_now_ms()` is **milliseconds since the platform started**. It only moves
forward, and it is the clock to use for anything you measure: animation timing,
"once a second" sampling, how long a button was held. `AcidGame` paces its ticks
with it. Do not use `acid_local_time` for that, since it can jump when the user
changes the time.

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

Each app is held to a memory limit and a responsiveness limit, set by its trust
level ([§2.6](02-apps-and-manifests.md#26-built-in-and-cart-level-apps)):

| | Lua memory | "Stopped responding" |
|---|---|---|
| Built-in | 64 MB | 2 s |
| Cart-level | 16 MB | 1 s |

**Memory.** The cap counts everything the app's Lua VM allocates, **including the
core libraries and the manifest's `libs`**, which are loaded first. An allocation
past the cap fails, and the app ends.

**Stopped responding.** The kernel checks every 10,000 Lua instructions how long
it has been since your app last returned from `acid_poll_event` (or since it
started, before its first poll). If that is more than the limit, a Lua error is
raised. `AcidApp` and `AcidGame` poll constantly, so this only matters when one
callback computes for longer than the limit. A long poll *timeout* never trips
it, because the clock restarts as the poll returns. If your app has to do
something that takes a while, do it in slices across several callbacks.

**What happens at a limit.** The app ends, its window is removed and its
resources are freed, and the terminal gets one line:

```text
Acid OS v3: <path>: out of memory
Acid OS v3: <path>: stopped responding
```

Neither can be caught. `pcall`, `xpcall` and `coroutine.resume` all re-raise a
timeout or an out-of-memory error rather than hand it back, and the error
handler of an `xpcall` is skipped once the limit has tripped, so a script cannot
swallow its own watchdog. Other apps are unaffected: each is its own VM on its
own thread.

The same hardening closes a few other doors a runaway script might try:
`string.dump` is gone, `load` reads only text, `setmetatable` refuses a
metatable with `__gc`, `table.insert`, `table.remove` and `table.move` refuse a
table or range of more than 16,777,216 entries, and repeating an empty string
costs nothing.

A **WASM** cart is limited differently: by fuel and linear-memory pages, not by
Lua memory and a clock. See [chapter 10](10-wasm-carts.md).

## 7.11 What a cart is refused

Everything above applies to a cart-level app, with these exceptions. Each refusal
fails cleanly: the call returns its failure value and the app carries on.

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

The path guard runs first, so a malformed path is `bad path` even for a cart,
and `read only` is what a *well-formed* path outside Home gets. The `Home/`
rule is a prefix match: the directory `v3/fsroot/Home` itself is not a path a
cart may change, only what is inside it.

Reading, drawing on your own window, sound, the clock, task and network
information, `acid_launcher_spawn` and `acid_spawn_app` for installed apps that
are not already open (they run cart-level, and only while fewer than 4
cart-level windows are open), and
`acid_set_volume` and `acid_set_wallpaper_enabled` all work exactly as they do
for a built-in app. The overlay drawing calls and `acid_repaint_region` stay
callable: they act only for the task that holds the overlay or only on the
caller's own canvas, and a cart never holds the overlay.

Because a cart's own window ends when its run loop ends, `acid_close_window` is
simply always `false` for it.

---

[← Games](06-games.md) · [Contents](README.md) · [Next: Cookbook →](08-cookbook.md)
