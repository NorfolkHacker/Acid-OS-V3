# 9. API reference

[← Cookbook](08-cookbook.md) · [Contents](README.md) · [Next: WASM carts →](10-wasm-carts.md)

This chapter lists every function Acid OS gives your app. There are **63** of
them. Each one is a plain global Lua function, so you can call it from anywhere
by name, with no object in front.

You pass in plain integers, numbers, strings and booleans, and you get plain
values back. Some calls return **several values at once**.

A few rules hold for every call:

- **Out-of-range values are clamped or ignored, not treated as errors.** A voice
  of 9 does nothing, a volume of 150 becomes 100, and a rectangle hanging off
  the edge is trimmed.
- **A value of the wrong type is an error.** If you pass `nil` where a number
  belongs, you get, for example:
  `bad argument #3: error converting Lua nil to i32 (expected number or string coercible to number)`.
  A table, a boolean or a non-numeric string gives the same message, with
  `Lua table`, `Lua boolean` or `Lua string` in place of `Lua nil`. An integer
  too big for 32 bits gives
  `bad argument #3: error converting Lua integer to i32 (out of range)`.
  If you don't catch the error, it ends your app.
- **Floats and numeric strings are accepted.** A float is **truncated** towards
  zero, and a string that holds a number is converted.
- **Calls that can fail for reasons outside your control return
  `nil, message`** instead of raising an error. These are the file system
  calls and the host cart folder calls.
- **"Or nothing" means no values at all.** So
  `local name = acid_window_info(i)` gives `nil` for an empty slot.
- **Cart-level apps are refused some calls**
  ([§2.6](02-apps-and-manifests.md#26-built-in-and-cart-level-apps),
  [§7.11](07-system-apis.md#711-what-a-cart-is-refused)). Each entry below
  says what a cart gets. If an entry says nothing, a cart behaves exactly like
  a built-in app.
- **Indexes start at 0.** Windows, tasks and the launcher registry are all
  numbered from zero, even though Lua tables start at 1.

Each entry below shows how to call the function, then what it does, then any
limits or errors, then what a cart gets (if that differs).

## Index by area

**Graphics** — [`acid_fill_rect`](#acid_fill_rect) · [`acid_fill_circle`](#acid_fill_circle) · [`acid_draw_text`](#acid_draw_text) · [`acid_draw_line`](#acid_draw_line) · [`acid_fill_triangle`](#acid_fill_triangle) · [`acid_mesh_builtin`](#acid_mesh_builtin) · [`acid_mesh_new`](#acid_mesh_new) · [`acid_mesh_draw`](#acid_mesh_draw) · [`acid_mesh_free`](#acid_mesh_free)

**Chrome** — [`acid_clear_user_area`](#acid_clear_user_area) · [`acid_draw_window_frame`](#acid_draw_window_frame) · [`acid_draw_window_border`](#acid_draw_window_border) · [`acid_repaint_region`](#acid_repaint_region)

**Overlay** — [`acid_overlay_open`](#acid_overlay_open) · [`acid_overlay_clear`](#acid_overlay_clear) · [`acid_overlay_fill_rect`](#acid_overlay_fill_rect) · [`acid_overlay_close`](#acid_overlay_close)

**Events and time** — [`acid_poll_event`](#acid_poll_event) · [`acid_notify_redraw_done`](#acid_notify_redraw_done) · [`acid_now_ms`](#acid_now_ms) · [`acid_local_time`](#acid_local_time)

**Audio** — [`acid_play_note`](#acid_play_note) · [`acid_stop_note`](#acid_stop_note) · [`acid_configure_voice`](#acid_configure_voice) · [`acid_configure_osc`](#acid_configure_osc) · [`acid_configure_filter`](#acid_configure_filter) · [`acid_set_ring_partner`](#acid_set_ring_partner) · [`acid_trigger_arp`](#acid_trigger_arp) · [`acid_set_volume`](#acid_set_volume) · [`acid_get_volume`](#acid_get_volume) · [`acid_active_voice_count`](#acid_active_voice_count)

**Windows** — [`acid_window_max`](#acid_window_max) · [`acid_screen_size`](#acid_screen_size) · [`acid_window_size`](#acid_window_size) · [`acid_font_size`](#acid_font_size) · [`acid_get_font_scale`](#acid_get_font_scale) · [`acid_set_font_scale`](#acid_set_font_scale) · [`acid_window_info`](#acid_window_info) · [`acid_activate_window`](#acid_activate_window) · [`acid_close_window`](#acid_close_window) · [`acid_send_self_to_back`](#acid_send_self_to_back) · [`acid_am_i_focused`](#acid_am_i_focused)

**Launching** — [`acid_launcher_register`](#acid_launcher_register) · [`acid_launcher_count`](#acid_launcher_count) · [`acid_launcher_name`](#acid_launcher_name) · [`acid_launcher_path`](#acid_launcher_path) · [`acid_launcher_spawn`](#acid_launcher_spawn) · [`acid_spawn_app`](#acid_spawn_app) · [`acid_launch_arg`](#acid_launch_arg)

**System** — [`acid_refresh_tasks`](#acid_refresh_tasks) · [`acid_task_count`](#acid_task_count) · [`acid_task_info`](#acid_task_info) · [`acid_mem_used_kb`](#acid_mem_used_kb) · [`acid_composited_frames`](#acid_composited_frames) · [`acid_skipped_frames`](#acid_skipped_frames) · [`acid_network_info`](#acid_network_info) · [`acid_set_wallpaper_enabled`](#acid_set_wallpaper_enabled) · [`acid_get_wallpaper_enabled`](#acid_get_wallpaper_enabled)

**File system** — [`acid_fs_list`](#acid_fs_list) · [`acid_fs_read`](#acid_fs_read) · [`acid_fs_size`](#acid_fs_size) · [`acid_fs_write`](#acid_fs_write) · [`acid_fs_rename`](#acid_fs_rename) · [`acid_fs_delete`](#acid_fs_delete)

**Host cart folders** — [`acid_cart_roots`](#acid_cart_roots) · [`acid_cart_list`](#acid_cart_list) · [`acid_cart_stat`](#acid_cart_stat) · [`acid_cart_read`](#acid_cart_read)

---

## Functions, alphabetically

### `acid_activate_window`

```lua snippet
acid_activate_window(index)
```

Brings the window at `index` to the front and gives it focus. `index` is the
same index [`acid_window_info`](#acid_window_info) uses. Returns nothing.

If the index is out of range, negative or an empty slot, nothing happens.

**Cart:** does nothing unless `index` is the cart's own window.

### `acid_active_voice_count`

```lua snippet
local n = acid_active_voice_count()   -- 0 to 8
```

How many synth voices are making sound right now (strictly, how many had a
sounding envelope in the latest audio buffer). Handy for checking and
debugging. See [§5.9](05-sound.md#59-master-volume-and-metering).

### `acid_am_i_focused`

```lua snippet
local focused = acid_am_i_focused()   -- true / false
```

`true` if your app's window has keyboard focus. In Acid OS that also means
it is the topmost visible window. `AcidApp:focused()` calls this for you.

### `acid_cart_list`

```lua snippet
local names, err = acid_cart_list(dir)
```

Lists what is inside `dir`, a folder within a host cart folder. You get the
names of its files and folders, sorted, or `nil, err`. Symlinks and other
special files are left out. This call only reads; it never changes anything.

Errors:

- `"bad path"`: `dir` is not really inside a cart folder.
- `"not found"`: `dir` does not exist.

**Cart:** `nil, "not allowed"`. See [§7.8](07-system-apis.md#host-cart-folders).

### `acid_cart_read`

```lua snippet
local bytes, err = acid_cart_read(path)
```

Reads a whole file from a host cart folder and returns it as a string, or
`nil, err`.

Errors:

- `"too big"`: the file is over **256 KB**.
- `"bad path"`: the path is a symlink, a directory, or outside the cart
  folders.
- `"not found"`: there is no such file.

**Cart:** `nil, "not allowed"`.

### `acid_cart_roots`

```lua snippet
local roots, err = acid_cart_roots()
```

Returns a list of the host cart folders that exist, in this order:
`v3/carts`, `$HOME/carts`, `/media`, `/mnt`, `/run/media`. The list may be
empty.

**Cart:** `nil, "not allowed"`.

### `acid_cart_stat`

```lua snippet
local kind, size = acid_cart_stat(path)    -- "dir" or "file", and the size in bytes
```

Tells you whether a path in a host cart folder is a folder (`"dir"`) or a
file (`"file"`), and its size in bytes. On failure it returns `nil, err`, with
the same errors as [`acid_cart_read`](#acid_cart_read) except `too big`.

**Cart:** `nil, "not allowed"`.

### `acid_clear_user_area`

```lua snippet
acid_clear_user_area()
```

Fills everything below the 16px title bar with `THEME_BG` (`0x050607`). The
title bar itself is left alone.

### `acid_close_window`

```lua snippet
local closed = acid_close_window(index)   -- true / false
```

Closes the window at `index`: the window is removed and its app is told to
close. Returns `true` if that happened.

Returns `false` for an out-of-range or empty index. It also **refuses to close
your own app's window**, and returns `false`.

**Cart:** always `false`.

### `acid_composited_frames`

```lua snippet
local n = acid_composited_frames()
```

How many frames the compositor (the part of the OS that combines all the
windows into one screen) has painted since boot.

### `acid_configure_filter`

```lua snippet
acid_configure_filter(cutoff, resonance, mode)
```

Sets up the filter. There is **only one filter, shared by every app**, so a
change here affects every routed voice in every app.

| Argument | Range | |
|---|---|---|
| `cutoff` | 0–255 | Corner frequency. Clamped. |
| `resonance` | 0–15 | Q from about 0.707 to 8.0. Clamped. |
| `mode` | bitmask 0–7 | `1` low-pass, `2` band-pass, `4` high-pass. Combinable; `0` mutes routed voices. Clamped to 0–7. |

It only affects voices whose `filter_route` is 1 (see
[`acid_configure_voice`](#acid_configure_voice)).
See [§5.6](05-sound.md#56-the-filter-acid_configure_filter).

### `acid_configure_osc`

```lua snippet
acid_configure_osc(voice, waveform, duty_percent)
```

| Argument | Range | |
|---|---|---|
| `voice` | 0–7 | Out of range ignored. |
| `waveform` | 0–3 | `AcidWaveform.PULSE`, `SAW`, `TRIANGLE`, `NOISE`. |
| `duty_percent` | 1–99 | Clamped. Audible only on `PULSE`. Default 50. |

See [§5.5](05-sound.md#55-the-oscillator-acid_configure_osc).

### `acid_configure_voice`

```lua snippet
acid_configure_voice(voice, filter_route, attack_ms, decay_ms, sustain_percent, release_ms)
```

| Argument | Range | |
|---|---|---|
| `voice` | 0–7 | |
| `filter_route` | 0 or 1 | Non-zero routes this voice through the shared filter. |
| `attack_ms` | ms | Silence to full. 0 or less is instant. |
| `decay_ms` | ms | Full to sustain. |
| `sustain_percent` | 0–100 | Clamped. **Overwritten by every `acid_play_note`'s `volume`.** |
| `release_ms` | ms | Current level to silence, after gate-off. |

Times are capped at 100,000 ms. A fresh voice starts as a pulse wave with 50%
duty, instant attack, decay and release, and no filter. See
[§5.4](05-sound.md#54-shaping-the-voice-acid_configure_voice).

### `acid_draw_line`

```lua snippet
acid_draw_line(x1, y1, x2, y2, color)
```

Draws a 1-pixel line from `(x1, y1)` to `(x2, y2)`, both ends included.
Coordinates are relative to your window, and `color` is a 24-bit `0xRRGGBB`
value.

**You can never paint outside your own window.** The parts of the line that
fall outside are trimmed, and any coordinates at all are safe.

### `acid_draw_text`

```lua snippet
acid_draw_text(str, x, y, fg, bg)
```

Draws `str` with its top-left corner at `(x, y)`, measured from your
window's top-left corner. If you pass a number, it is converted to a string.

The font is a fixed-width bitmap font, **6×8 pixels per character**. The
background is solid, so `bg` should match whatever is already behind the text.
If `fg` and `bg` are the same colour, only the lit pixels are drawn.

**Text is clipped to your window one character at a time.** It is not clipped
to your border or to anything else you have drawn, so a long string runs
straight across them. Shorten it first with `s:sub(1, n)`. See [§4.2](04-graphics.md#42-coordinates-and-clipping).

### `acid_draw_window_border`

```lua snippet
acid_draw_window_border()
```

Draws a 1px `THEME_HARD` (`0x00FF66`) outline around the whole window, then
cuts the four rounded corners.

**Make this the last call in your redraw.** Your content sits on exactly these
pixels, so anything drawn afterwards would cover the border.

### `acid_draw_window_frame`

```lua snippet
acid_draw_window_frame(title)
```

Draws the title bar. It fills the 16px bar with `THEME_PANEL`, draws `title` at
`(4, 4)` in `THEME_TEXT`, and puts the `THEME_HARD` close dot on the right.

The title is **not** clipped at the close dot, so keep it to 16 characters or
fewer.

### `acid_fill_circle`

```lua snippet
acid_fill_circle(x, y, r, color)
```

Draws a filled circle centred at `(x, y)` with radius `r`. Coordinates are
relative to your window, and the circle is clipped to it. A negative radius
draws nothing.

### `acid_fill_rect`

```lua snippet
acid_fill_rect(x, y, w, h, color)
```

Draws a filled rectangle. Coordinates are relative to your window, and
`color` is a 24-bit `0xRRGGBB` value.

**You can never paint outside your own window.** A rectangle that runs off an
edge is trimmed, and one that is entirely outside draws nothing.

### `acid_fill_triangle`

```lua snippet
acid_fill_triangle(x1, y1, x2, y2, x3, y3, color)
```

Draws a filled triangle with corners `(x1, y1)`, `(x2, y2)` and `(x3, y3)`, in
any order. Coordinates are relative to your window, and `color` is a 24-bit
`0xRRGGBB` value. Three corners in a straight line draw nothing wider than that
line.

**You can never paint outside your own window.** The triangle is trimmed to it,
and any coordinates at all are safe.

### `acid_font_size`

```lua snippet
local w, h = acid_font_size()   -- 6, 8 (Large: 12, 16)
```

This window's character cell in pixels. It is fixed for the window's life, so
lay text out from it.

### `acid_fs_delete`

```lua snippet
local ok, err = acid_fs_delete(path)    -- true, or nil, err
```

Deletes a file. It won't delete a folder: that gives `nil, "is a directory"`.

Errors:

- `bad path`: the path guard rejected it, or its real location is outside the
  allowed roots.
- `not found`: there is no such file.
- `read only`: a cart tried to delete outside its area (see below).
- Anything else is the operating system's own message.

**Cart:** only under `v3/fsroot/Home/`; elsewhere `nil, "read only"`.

### `acid_fs_list`

```lua snippet
local names, err = acid_fs_list(dir)    -- a sequence of names, or nil, err
```

Lists the names in folder `dir`, sorted, without `.` and `..`.

Errors: `bad path`, `not found`, or the operating system's own message. For
example, passing a file gives `Not a directory (os error 20)`. See [§7.8](07-system-apis.md#78-the-file-system).

### `acid_fs_read`

```lua snippet
local bytes, err = acid_fs_read(path)   -- a string, or nil, err
```

Reads the whole file and returns it as one string. The string can hold any
bytes, not just text.

Errors: `bad path`, `not found`, or the operating system's own message. For
example, passing a folder gives `Is a directory (os error 21)`.

### `acid_fs_rename`

```lua snippet
local ok, err = acid_fs_rename(from, to)    -- true, or nil, err
```

Renames or moves a file or folder. If a file already exists at `to`, it is
replaced.

Both paths go through the path guard and the real-location check.

Errors:

- `bad path`: either path fails those checks, is a root folder itself, or is a
  symlink. This includes a `from` or `to` whose folder doesn't exist.
- `not found`: there is nothing at `from`.
- `read only`: a cart tried to rename outside its area (see below).

**Cart:** both ends must be under `v3/fsroot/Home/`; otherwise
`nil, "read only"`.

### `acid_fs_size`

```lua snippet
local bytes, err = acid_fs_size(path)   -- an integer, or nil, err
```

Returns the size of a file in bytes.

**This is also how you check whether something exists:**
`acid_fs_size(path) ~= nil`. That works for folders too; a folder's size is
whatever the host reports.

Errors: `bad path`, `not found`.

### `acid_fs_write`

```lua snippet
local ok, err = acid_fs_write(path, data)    -- true, or nil, err
```

Creates the file, or replaces its whole contents with `data`. There is no
append mode.

Errors:

- `bad path`: the parent folder doesn't exist, or the path's real location is
  not inside `v3/apps` or `v3/fsroot`.
- Writing to a folder gives the operating system's own message.

**Cart:** only under `v3/fsroot/Home/`; elsewhere `nil, "read only"`.

### `acid_get_font_scale`

```lua snippet
local n = acid_get_font_scale()   -- 1 Normal, 2 Large
```

Config's font setting.

### `acid_get_volume`

```lua snippet
local percent = acid_get_volume()   -- 0 to 100
```

The system-wide output volume.

### `acid_get_wallpaper_enabled`

```lua snippet
local on = acid_get_wallpaper_enabled()   -- true / false
```

Whether the desktop wallpaper is switched on.

### `acid_launch_arg`

```lua snippet
local arg = acid_launch_arg()   -- a string
```

The startup string your app was launched with, or `""` if there wasn't one.
You can read it as many times as you like.

### `acid_launcher_count`

```lua snippet
local n = acid_launcher_count()
```

How many apps are in the launcher registry (the list of installed apps). This
includes apps hidden from the Menu with `menu = false`. The most it can be is
48.

### `acid_launcher_name`

```lua snippet
local name = acid_launcher_name(index)   -- a string, or nil
```

The app's display name, or `nil` if the index is out of range.

### `acid_launcher_path`

```lua snippet
local path = acid_launcher_path(index)   -- a string, or nil
```

The app's script path, such as `"v3/apps/tetris.lua"`, or `nil` if the index
is out of range.

### `acid_launcher_register`

```lua snippet
local ok = acid_launcher_register(path, name, w, h, multi, libs)   -- true / false
```

Adds an app to the launcher registry.

- `multi` is a boolean: can the app have more than one window open?
- `libs` is a comma-separated string of modules. Use `""` or `nil` for none.

Returns `false` if the registry is full, if `w` or `h` is bigger than the
screen.

The desktop (`desktop.lua`) calls this when it scans the app manifests at
boot. You only need it yourself if you are writing a replacement desktop.

**Cart:** always `false`.

### `acid_launcher_spawn`

```lua snippet
local ok = acid_launcher_spawn(index)   -- true / false
```

Launches the registered app at `index`, using the size, `multi` flag and
`libs` it was registered with.

Returns `true` if it worked. That **includes** the case where the app is a
singleton (only one window allowed) and is already open: its existing window
is brought to the front and focused instead.

Returns `false` for an unknown index, or if no window could be made.

**Cart:** returns `false`:

- when the app is single-instance and already open (its window is not raised
  or focused);
- while 4 or more cart-level windows are open, counting the cart's own;
- once the cart's own window has been closed.

Otherwise it works as it does for a built-in app, except that the app it starts
runs cart-level. A built-in caller never raises a copy of a singleton that a
cart started. It opens a trusted window of its own instead.

### `acid_local_time`

```lua snippet
local year, month, day, hour, min, sec = acid_local_time()
```

The local date and time as six integers. Months run 1 to 12 and hours 0 to
23. On a platform with no clock you get `1970, 1, 1, 0, 0, 0`. See
[§7.9](07-system-apis.md#79-the-clock).

### `acid_mem_used_kb`

```lua snippet
local kb = acid_mem_used_kb()   -- an integer, or -1
```

How much memory is in use, in kilobytes, or `-1` if that isn't known.

When Acid OS runs in a window on your computer, this is the memory used by the
whole OS process, shared by every app, not just yours.

### `acid_mesh_builtin`

```lua snippet
local id = acid_mesh_builtin("cube")   -- or nil
```

Gives you the id of a ready-made shape: `"cube"`, `"pyramid"`, `"octahedron"`,
`"sphere"` or `"torus"`. An unknown name returns `nil`, and so does a request
over the mesh limits (see [`acid_mesh_new`](#acid_mesh_new)).

Ids start at 1 and count up, and an id is never reused, even after
[`acid_mesh_free`](#acid_mesh_free).

### `acid_mesh_draw`

```lua snippet
acid_mesh_draw(id, x, y, size, rx, ry, rz, mode, color)
```

Draws a mesh with its centre at `(x, y)` in your window.

| Argument | Range | |
|---|---|---|
| `id` | | From `acid_mesh_builtin` or `acid_mesh_new`. An unknown or freed id draws nothing. |
| `size` | 1 and up | 64 is one model unit per pixel. Zero or less draws nothing. |
| `rx`, `ry`, `rz` | 0–255 | Rotation around each axis, 256 to a full turn. Any integer is accepted and wraps. |
| `mode` | 0, 1 or 2 | 0 wire, 1 solid, 2 both (solid, with the edges in a brighter colour). Any other value draws nothing. |
| `color` | `0xRRGGBB` | The mesh's colour. |

Solid faces are shaded, and parts behind the camera are not drawn. Like every
drawing call, it is trimmed to your window.

### `acid_mesh_free`

```lua snippet
acid_mesh_free(id)
```

Frees a mesh, which gives its slot and its points back to your limits. An
unknown id does nothing.

### `acid_mesh_new`

```lua snippet
local id, err = acid_mesh_new(points, faces)
```

Makes a mesh of your own. `points` is a flat list of `x, y, z` numbers, three
per point. `faces` is a list of faces, each a list of 3 or 4 point numbers
counting **from 1**. A face with 4 points is split into two triangles, so it
should be flat.

```lua snippet
local id = acid_mesh_new({0,0,0, 100,0,0, 0,100,0}, {{1, 2, 3}})
```

It returns the new id, or `nil, message`:

| Message | When |
|---|---|
| `"bad mesh"` | Fewer than 3 points, a `points` list that isn't a multiple of 3, a face with other than 3 or 4 indices, an index below 1 or past the last point, a face that repeats a point, or a coordinate beyond ±32767. |
| `"too big"` | Over 512 points or 1024 faces in the mesh, 16 meshes alive, or 4096 points across your live meshes. |

The list lengths are checked before the shape, so a mesh that is both too big
and bad gives `"too big"`: 2 points with 2000 faces is `"too big"` here. A wasm
cart's `mesh_new` checks for fewer than 3 points first and returns −2 for the
same mesh ([§10](10-wasm-carts.md#mesh-arrays)).

### `acid_network_info`

```lua snippet
local hostname, ip, connected = acid_network_info()
```

Returns three things: the host name, the first real IPv4 address (not the
loopback `127.x` one) as a dotted string, and whether an address was found.

- With no address: the host name, `"none"`, `false`.
- With nothing known at all: `"unknown"`, `"none"`, `false`.

**`connected` only means an address was found, not that anything is
reachable.** There is no live connection check, and no socket API for network
traffic.

### `acid_notify_redraw_done`

```lua snippet
acid_notify_redraw_done()
```

Tells the OS that your window has finished repainting after a `"moved"`
event. **In this version it does nothing**, because the compositor doesn't
wait for apps. It is harmless to call. `AcidApp` and `AcidGame` still call it,
and a hand-written loop may too. See
[§3.2](03-app-lifecycle.md#moved-and-acid_notify_redraw_done).

### `acid_now_ms`

```lua snippet
local ms = acid_now_ms()
```

Milliseconds since the platform started, as an integer. It only ever goes up,
never backwards, so it is the clock to use for timing. `AcidGame` uses it to
pace its ticks. See
[§7.9](07-system-apis.md#79-the-clock).

### `acid_overlay_clear`

```lua snippet
acid_overlay_clear()
```

Fills the whole overlay with the transparency colour (`0xFF00FF`), which
shows as see-through. Does nothing if you don't own the overlay.

### `acid_overlay_close`

```lua snippet
acid_overlay_close()
```

Hides the overlay and gives up your claim on it. It does nothing unless you are
the current owner, so you can never close someone else's animation. If your
app exits, the overlay is released for you.

### `acid_overlay_fill_rect`

```lua snippet
acid_overlay_fill_rect(x, y, w, h, color)
```

Draws a filled rectangle on the overlay. Coordinates are **measured from the
top-left of the screen**, not your window. The rectangle
is clipped to the screen, so negative coordinates are fine.

If you don't own the overlay, it quietly draws nothing.

### `acid_overlay_open`

```lua snippet
local ok = acid_overlay_open()   -- true / false
```

Claims the overlay, a single full-screen drawing layer that sits above every
window, and clears it to the transparency colour. Returns `true` if you got
it.

`false` means another app already holds it. **That is not an error**: an effect
that can't start should simply not run. If you already own the overlay,
opening it again succeeds and clears it again. See
[§4.5](04-graphics.md#45-the-overlay).

**Cart:** always `false`; the other overlay calls then do nothing.

### `acid_play_note`

```lua snippet
acid_play_note(voice, ona, volume)
```

| Argument | Range | |
|---|---|---|
| `voice` | 0–7 | Out of range ignored. |
| `ona` | 1–88 | 88-key piano numbering; 49 = A4 = 440 Hz. Out of range leaves pitch unchanged. |
| `volume` | 0–100 | Clamped. **Becomes the voice's sustain level.** |

Starts a note. It sets the pitch, sets the sustain level from `volume`, and
starts the envelope from zero, so playing a note again replays the attack.

On a voice that has never had a pitch and isn't running an arpeggio, this call
does nothing. That avoids a thump from the speaker.

The voice is marked as belonging to your app, and it is released when your app
ends.

**The note sounds until `acid_stop_note`.** There is no duration argument.

### `acid_poll_event`

```lua snippet
local kind, a, b, c = acid_poll_event(timeout_ms)
```

Waits up to `timeout_ms` milliseconds for the next event for your window, then
returns it as **several values**. The first value says what kind of event it
is:

| Returns | Meaning |
|---|---|
| nothing | Timed out, nothing waiting |
| `"close"` | Close button, or the kernel ending this app |
| `"moved"` | Window was dragged; repaint, then `acid_notify_redraw_done` (not sent in this version) |
| `"key", code, pressed` | Key event; `code` is ASCII or an `AcidKeys` constant. Only presses are generated, so `pressed` is always `true` |
| `"touch", x, y, pressed` | Touch event, window-relative; may fall outside the window during a drag |

A negative `timeout_ms` counts as `0`. With `0` the call doesn't wait at all,
so a loop that passes `0` keeps a CPU core busy. `AcidApp` never passes less
than 1.

Each time this call returns, the "stopped responding" clock starts again
([§7.10](07-system-apis.md#710-limits)).

### `acid_refresh_tasks`

```lua snippet
local n = acid_refresh_tasks()
```

Takes a fresh snapshot of the running tasks and returns how many it found, at
most 16. When Acid OS runs in a window on your computer, the tasks are the OS
process's threads. Call this before [`acid_task_info`](#acid_task_info).

### `acid_repaint_region`

```lua snippet
acid_repaint_region(x, y, w, h)
```

Fills that region **of your own window** with the wallpaper, or with `THEME_BG`
when the wallpaper is off. This erases what you drew there, so the next frame
shows whatever is really underneath.

Despite the name, it doesn't ask any other window to repaint.

It copies the wallpaper pixels at the *same coordinates*, so it only looks
right for a window sitting at the screen's `(0, 0)`.

### `acid_screen_size`

```lua snippet
local w, h = acid_screen_size()   -- 640, 480 by default
```

The screen's size in pixels. Acid OS picks it at startup (640×480, 640×360
or 800×600), and it doesn't change while it runs, so it's safe to read once
and keep.

### `acid_send_self_to_back`

```lua snippet
acid_send_self_to_back()
```

Sends your app's own window behind all the others. It always acts on your own
window.

### `acid_set_font_scale`

```lua snippet
acid_set_font_scale(2)
```

Sets Config's font setting. Only 1 and 2 are accepted. Apps opened afterwards
that set `font = scalable` use it. This is a system setting that belongs to
the Config app. Carts can call it too.

### `acid_set_ring_partner`

```lua snippet
acid_set_ring_partner(voice, partner)
```

Pairs `voice` with `partner` (0–7) for ring modulation, a metallic,
bell-like effect. A negative `partner` clears the pairing, and a partner above
7 is ignored.

**You only hear it on a `TRIANGLE` voice.** That matches the real SID (the
Commodore 64's sound chip), whose ring modulator is wired into the triangle
wave. The partner only lends its oscillator phase, so it doesn't need to be
playing a note.

### `acid_set_volume`

```lua snippet
acid_set_volume(percent)
```

Sets the system-wide output volume, 0–100, clamped.

**This is a system setting that belongs to the Config app.** To make your app
quieter, turn down your own note volumes instead. Carts can call it too.

### `acid_set_wallpaper_enabled`

```lua snippet
acid_set_wallpaper_enabled(enabled)
```

Turns the desktop wallpaper on or off for the whole system, and redraws the
screen. Carts can call it too.

### `acid_skipped_frames`

```lua snippet
local n = acid_skipped_frames()
```

How many frames the compositor skipped because nothing had changed.

### `acid_spawn_app`

```lua snippet
local ok = acid_spawn_app(path, w, h, arg)   -- true / false; arg optional
```

Launches an app by its script path. `arg` is the startup string; use `""` or
`nil` for none.

The `multi` flag and `libs` come from the launcher registry, matched on the
**exact path**. A path that isn't registered gets singleton behaviour (one
window only) and no modules. If a singleton is already open, its window is
brought to the front instead of opening a second one.

Returns `false` if the size isn't 1 to the screen's size in each direction, or no window slot is
free.

A path that isn't a `.lua` or `.wasm` file under `v3/apps` or `v3/fsroot`
still returns `true`. The new app then ends straight away and logs
`Acid OS v3: refused unsafe script path <path>`.

If a path came from the file system, tidy it with
`AcidApp:canonical_app_path` first. `v3/fsroot/App` is a symlink to `v3/apps`,
and the registry doesn't follow it, so the same app can look unregistered.

**Cart:** returns `false`:

- unless `path` starts with `v3/apps/`;
- when that app is single-instance and already open (its window is not raised
  or focused);
- while 4 or more cart-level windows are open, counting the cart's own;
- once the cart's own window has been closed.

The app it starts runs cart-level, even one that would otherwise be built-in. A
built-in caller never raises a copy of a singleton that a cart started. It
opens a trusted window of its own instead.

### `acid_stop_note`

```lua snippet
acid_stop_note(voice)
```

Stops the note on `voice`: its release phase starts and any arpeggio on it
stops.

**It works on any voice, whichever app started it.** There is no ownership
check.

### `acid_task_count`

```lua snippet
local n = acid_task_count()
```

How many tasks were in the latest [`acid_refresh_tasks`](#acid_refresh_tasks)
snapshot.

### `acid_task_info`

```lua snippet
local name, state, cpu_percent = acid_task_info(index)   -- or nothing
```

Details of one task from the latest snapshot:

- `name` is the thread name. Linux cuts it to 15 characters.
- `state` is one of `"running"`, `"blocked"`, `"suspended"`, `"deleted"` or
  `"?"`.
- `cpu_percent` is an integer. It is 0 on the first snapshot.

Returns nothing for an out-of-range index.

### `acid_trigger_arp`

```lua snippet
acid_trigger_arp(voice, note0, note1, note2, note3, count, rate_ms)
```

Plays an arpeggio: `voice` steps through the first `count` of the four notes,
in slot order and wrapping back to the first, spending `rate_ms` on each. The
notes are **absolute `ona` values** (pitches), not offsets.

| Argument | Range | |
|---|---|---|
| `voice` | 0–7 | |
| `note0`–`note3` | 1–88 | Absolute pitches, not offsets. Unused slots must still be valid: pass `1`. |
| `count` | 2–4 | How many slots to use. A `count` outside 2–4 (1 included) does not start the arpeggio; the notes and rate are still stored, and an arpeggio already running restarts from slot 0. |
| `rate_ms` | ms | Per step; 0 or less is one audio sample, capped at 10,000. |

Call it **straight after `acid_play_note`** on the same voice.
`acid_play_note` sets the volume and starts the envelope; this call then
steps the pitch on top. The pattern starts again from slot 0 each time a note
starts.

**It cycles forever until `acid_stop_note`.** See
[§6.4](06-games.md#64-the-sound-effect-lifecycle).

### `acid_window_info`

```lua snippet
local name, x, y, w, h, focused = acid_window_info(index)   -- or nothing
```

Details of one window: its name (which is its script path), its position on
screen, its size, and whether it has focus.

Returns nothing for an empty, negative or out-of-range slot. To see every
window, loop from `0` to `acid_window_max() - 1` and skip the gaps.

### `acid_window_max`

```lua snippet
local n = acid_window_max()   -- 8
```

How many window slots there are. Valid
[`acid_window_info`](#acid_window_info) indexes run from 0 up to one less
than this.

### `acid_window_size`

```lua snippet
local w, h = acid_window_size()
```

This window's real size in pixels. It can be larger than the manifest's `w`
and `h` when the app opted into the font setting (it is larger at Large,
clamped to the screen). It reports the current size, which changes when the
user resizes a resizable window.

---

## Library modules

These all live in `v3/apps/lib/`. The five core modules are loaded into every
app. To use any other module, list it in your manifest's `libs`
([§2.3](02-apps-and-manifests.md#23-loading-modules)).

### `AcidApp`: always loaded

The base class for apps that react to events. [Chapter 3](03-app-lifecycle.md).

```lua snippet
local MyApp = AcidApp:extend("MyApp")   -- the string is the class name
MyApp:new():start()
```

| Member | |
|---|---|
| `on_create()` | Once, before the first paint |
| `on_touch(x, y, pressed)` | Window-relative touch |
| `on_key(code, pressed)` | Key event; focused window only |
| `on_resize(w, h)` | After the user resizes a resizable window, with the new size; `redraw` follows |
| `on_idle()` | Every `poll_timeout_ms` with no event |
| `on_destroy()` | Once, after the loop ends |
| `redraw()` | Full repaint; default draws bare chrome |
| `poll_timeout_ms()` | Method, default `200`; clamped to at least 1 |
| `window_title()` | Derived from the class name, capped at 16 chars |
| `focused()` | Wraps `acid_am_i_focused` |
| `quit()` | Ends the loop (**no effect in `AcidGame`**) |
| `canonical_app_path(path)` | Maps `v3/fsroot/App/...` to `v3/apps/...` |
| `start()` | Runs the event loop |
| `extend(class_name)` | Makes a subclass |
| `new()` | Makes an instance |

### `AcidGame`: always loaded

A subclass of `AcidApp` with a fixed-tick loop: it runs your code at a steady
rate, whatever events arrive. Made for games. [Chapter 6](06-games.md).

| Member | |
|---|---|
| `TICK_MS` | **Field** on your class; tick interval, default 50 |
| `on_tick()` | Every `TICK_MS`, regardless of events |
| `start()` | Own loop; never calls `redraw`, ignores `quit`, acknowledges `"moved"` without repainting |

### `AcidKeys`: always loaded

```lua snippet
local keys = {
  ENTER = 257, BACKSPACE = 258, ESCAPE = 259, TAB = 260, DELETE = 261,
  UP = 262, DOWN = 263, LEFT = 264, RIGHT = 265,
}
```

These are the fields of `AcidKeys` and their values. Printable keys arrive as
their ASCII code instead.

### `AcidPalette`: always loaded

```lua snippet
local colour = AcidPalette.hue(step)          -- 256-step HSV hue wheel, 0xRRGGBB
local colour = AcidPalette.hue(step, steps)   -- a wheel of `steps` divisions
```

Gives colours at full saturation and full brightness, using whole-number maths
only. `step`
wraps round. Use it for **your content**; use the theme colours for interface
parts like panels and text. [§4.1](04-graphics.md#41-colours).

### `AcidWaveform`: always loaded

```lua snippet
local names = { PULSE = 0, SAW = 1, TRIANGLE = 2, NOISE = 3 }
```

These are the fields of `AcidWaveform` and their values, for
[`acid_configure_osc`](#acid_configure_osc).

### `AcidSprite`: add `lib/acid_sprite.lua` to `libs`

Draws small pictures, made from a grid of characters, onto the **overlay**.
Coordinates are measured from the top-left of the screen.

```lua snippet
AcidSprite.draw(rows, x, y, scale, palette, flip)   -- flip is optional
local cols = AcidSprite.width(rows)    -- in characters
local lines = AcidSprite.height(rows)  -- in characters
local s, err = AcidSprite.load(path)    -- read and parse a .spr file
local s, err = AcidSprite.parse(text)   -- parse .spr text
local text = AcidSprite.serialize(s)    -- .spr text for a sprite table
```

- `rows` is a list of strings, all the same length, with one character per
  pixel. `.` is transparent.
- `palette` maps each character to a `0xRRGGBB` colour. A character with no
  entry is transparent too.
- `load` and `parse` return a sprite table,
  `{ w, h, fps, palette, frames }`, where each of `frames` is a `rows` list
  you can pass to `draw` with `palette`. On a bad file they return `nil` and
  `"line N: reason"`. See [Sprite files](04-graphics.md#sprite-files).

Runs of the same colour are drawn as one rectangle, to save work. You must own
the overlay to draw, so a cart draws nothing.
[§4.6](04-graphics.md#46-sprites).

### `AcidScrollbar`: add `lib/acid_scrollbar.lua` to `libs`

A vertical scroll bar for a list or text view. The module does the maths and
the drawing; your app keeps the scroll offset (the first row on screen) and,
while a drag is held, the grab point. `total` rows of which `visible` fit;
`(x, y, h)` is the track's top-left corner and height in window pixels.

```lua snippet
AcidScrollbar.draw(x, y, h, total, visible, offset)   -- nothing if it all fits
if AcidScrollbar.needed(total, visible) and AcidScrollbar.hit(x, y, h, px, py) then
  offset, grab = AcidScrollbar.press(h, total, visible, offset, py - y)
end
offset = AcidScrollbar.drag(h, total, visible, grab, py - y)   -- while held
```

- `press` pages one screenful when you tap the track above or below the
  thumb, and returns `grab = nil`. On the thumb it returns the offset
  unchanged and a grab point to pass to `drag` for as long as the touch is
  held.
- Offsets are always clamped to `0` to `AcidScrollbar.max_offset(total, visible)`.
- The bar is `AcidScrollbar.WIDTH` (6) pixels wide. File Manager uses it.

## Theme colours

These are built into the system and are not
available to Lua by name. Copy the ones you use into your app as your own
constants.

| Name | Value |
|---|---|
| `THEME_BG` | `0x050607` |
| `THEME_HARD` | `0x00FF66` |
| `THEME_PANEL` | `0x0B1712` |
| `THEME_TEXT` | `0xD4E6DB` |
| `THEME_MUTED` | `0x9DAAA3` |
| `THEME_VIOLET` | `0xB026FF` |
| overlay transparency key | `0xFF00FF` |

## Fixed geometry and limits

| | |
|---|---|
| Screen | 640 × 480 by default; 640 × 360 or 800 × 600 ([`acid_screen_size`](#acid_screen_size)) |
| Window slots | 8 |
| Launcher registry | 48 apps |
| Tracked tasks | 16 |
| Title bar height | 16 |
| Window border | 1px, all round |
| Desktop top strip | 24 |
| Font glyph | 6 × 8 |
| Synth voices | 8 (0–7) |
| Synth sample rate | 22,050 Hz |
| Arpeggio slots | 4 (a `count` of 2–4) |
| `ona` range | 1–88 (49 = A4 = 440 Hz) |
| Built-in app limits | 64 MB Lua memory, 2 s without polling |
| Cart-level limits | 16 MB Lua memory, 1 s without polling |
| Cart file size (host cart folders) | 256 KB |

## Where the WASM carts differ

A WASM cart can do the same things, but it calls imports instead of Lua
globals. The main differences:

- **Errors are numbers.** Where a Lua call returns `nil, message` (the file
  system and host cart folder calls), the import returns a negative code:
  `-1` not found, `-2` bad path, `-3` read only, `-4` not allowed, `-5` too
  big, `-6` other.
- **Refusals are `0`.** Where a cart gets `false` (`acid_overlay_open`,
  `acid_launcher_register`, `acid_launcher_spawn`, `acid_spawn_app`), the
  import returns `0`.
- **Except closing windows.** The `acid_close_window` import returns `-4` to a
  cart.
- **The limits are different.** Carts have their own fuel and memory limits.

[Chapter 10](10-wasm-carts.md) covers all of this.

---

[← Cookbook](08-cookbook.md) · [Contents](README.md) · [Next: WASM carts →](10-wasm-carts.md)
