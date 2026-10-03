# 9. API reference

[← Cookbook](08-cookbook.md) · [Contents](README.md) · [Next: WASM carts →](10-wasm-carts.md)

Every `acid_*` function is a **global Lua function**, callable from anywhere with
no receiver. There are **57** of them. Arguments are plain integers, numbers,
strings and booleans; results are plain values, **several at a time** where
a call has more than one.

How the calls treat their arguments and failures:

- **Out-of-range values are clamped or ignored**, never raised: a voice of 9 does
  nothing, a volume of 150 becomes 100, a rectangle off the edge is trimmed.
- **A value of the wrong type does raise.** A number argument given `nil`
  raises, for example,
  `bad argument #3: error converting Lua nil to i32 (expected number or string coercible to number)`.
  A table, a boolean or a string that is not a number raises the same message
  with `Lua table`, `Lua boolean` or `Lua string` in place of `Lua nil`. An
  integer too big for 32 bits raises
  `bad argument #3: error converting Lua integer to i32 (out of range)`. An uncaught error ends the app. A float is
  accepted and **truncated** towards zero; a string that holds a number is
  converted.
- **A call that can fail for a reason outside your control returns
  `nil, message`**, not an error: the file system calls, and the host cart folder
  calls. Where a call says "or nothing", it returns no values at all, so
  `local name = acid_window_info(i)` is `nil` for an empty slot.
- **A cart-level app is refused some calls** ([§2.6](02-apps-and-manifests.md#26-built-in-and-cart-level-apps),
  [§7.11](07-system-apis.md#711-what-a-cart-is-refused)). Each entry says what
  a cart gets. Where it says nothing, a cart behaves exactly like a built-in
  app.

All the calls below are **zero-based** where they take an index (windows, tasks,
the launcher registry), though Lua tables are one-based.

## Index by area

**Graphics** — [`acid_fill_rect`](#acid_fill_rect) · [`acid_fill_circle`](#acid_fill_circle) · [`acid_draw_text`](#acid_draw_text)

**Chrome** — [`acid_clear_user_area`](#acid_clear_user_area) · [`acid_draw_window_frame`](#acid_draw_window_frame) · [`acid_draw_window_border`](#acid_draw_window_border) · [`acid_repaint_region`](#acid_repaint_region)

**Overlay** — [`acid_overlay_open`](#acid_overlay_open) · [`acid_overlay_clear`](#acid_overlay_clear) · [`acid_overlay_fill_rect`](#acid_overlay_fill_rect) · [`acid_overlay_close`](#acid_overlay_close)

**Events and time** — [`acid_poll_event`](#acid_poll_event) · [`acid_notify_redraw_done`](#acid_notify_redraw_done) · [`acid_now_ms`](#acid_now_ms) · [`acid_local_time`](#acid_local_time)

**Audio** — [`acid_play_note`](#acid_play_note) · [`acid_stop_note`](#acid_stop_note) · [`acid_configure_voice`](#acid_configure_voice) · [`acid_configure_osc`](#acid_configure_osc) · [`acid_configure_filter`](#acid_configure_filter) · [`acid_set_ring_partner`](#acid_set_ring_partner) · [`acid_trigger_arp`](#acid_trigger_arp) · [`acid_set_volume`](#acid_set_volume) · [`acid_get_volume`](#acid_get_volume) · [`acid_active_voice_count`](#acid_active_voice_count)

**Windows** — [`acid_window_max`](#acid_window_max) · [`acid_window_info`](#acid_window_info) · [`acid_activate_window`](#acid_activate_window) · [`acid_close_window`](#acid_close_window) · [`acid_send_self_to_back`](#acid_send_self_to_back) · [`acid_am_i_focused`](#acid_am_i_focused)

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

Raises the window at `index` (an [`acid_window_info`](#acid_window_info) index)
to the front and gives it focus. Returns nothing. An out-of-range, negative or
empty index: no-op.

**Cart:** does nothing unless `index` is the cart's own window.

### `acid_active_voice_count`

```lua snippet
local n = acid_active_voice_count()   -- 0 to 8
```

How many synth voices have a sounding envelope, as of the most recent audio
buffer. Diagnostic. See [§5.9](05-sound.md#59-master-volume-and-metering).

### `acid_am_i_focused`

```lua snippet
local focused = acid_am_i_focused()   -- true / false
```

True if the calling app's own window holds keyboard focus, which in this OS also
means it is the topmost visible window. Wrapped by `AcidApp:focused()`.

### `acid_cart_list`

```lua snippet
local names, err = acid_cart_list(dir)
```

The sorted entry names (files and directories) of a host cart folder `dir`, or
`nil, err`. Symlinks and other special files are left out. `dir` must really lie
inside a cart folder: otherwise `nil, "bad path"`; a missing one is
`nil, "not found"`. Read only.

**Cart:** `nil, "not allowed"`. See [§7.8](07-system-apis.md#host-cart-folders).

### `acid_cart_read`

```lua snippet
local bytes, err = acid_cart_read(path)
```

The contents of a file in a host cart folder as a string, or `nil, err`. Files
over **256 KB** give `nil, "too big"`; a symlink, a directory or a path outside
the cart folders gives `nil, "bad path"`; a missing file is
`nil, "not found"`.

**Cart:** `nil, "not allowed"`.

### `acid_cart_roots`

```lua snippet
local roots, err = acid_cart_roots()
```

A sequence of the host cart folders that exist, in order: `v3/carts`,
`$HOME/carts`, then `/media`, `/mnt` and `/run/media`. May be empty.

**Cart:** `nil, "not allowed"`.

### `acid_cart_stat`

```lua snippet
local kind, size = acid_cart_stat(path)    -- "dir" or "file", and the size in bytes
```

What a path in a host cart folder is, and its size in bytes; or `nil, err` with
the same errors as [`acid_cart_read`](#acid_cart_read) (without `too big`).

**Cart:** `nil, "not allowed"`.

### `acid_clear_user_area`

```lua snippet
acid_clear_user_area()
```

Fills everything below the 16px title bar with `THEME_BG` (`0x050607`). Does not
touch the title bar.

### `acid_close_window`

```lua snippet
local closed = acid_close_window(index)   -- true / false
```

Ends the window at `index`: the window is removed and its app is told to close.
Returns `true` when it did. Returns `false` for an out-of-range or empty index,
and **refuses to close the calling app's own window**.

**Cart:** always `false`.

### `acid_composited_frames`

```lua snippet
local n = acid_composited_frames()
```

Frames the compositor has painted since boot.

### `acid_configure_filter`

```lua snippet
acid_configure_filter(cutoff, resonance, mode)
```

Sets the **one shared** filter. Global: affects every routed voice in every app.

| Argument | Range | |
|---|---|---|
| `cutoff` | 0–255 | Corner frequency. Clamped. |
| `resonance` | 0–15 | Q from about 0.707 to 8.0. Clamped. |
| `mode` | bitmask 0–7 | `1` low-pass, `2` band-pass, `4` high-pass. Combinable; `0` mutes routed voices. Clamped to 0–7. |

Only affects voices whose `filter_route` is 1 (see
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

Times are capped at 100,000 ms. Defaults on a fresh voice: pulse wave, 50% duty,
instant attack/decay/release, unfiltered. See
[§5.4](05-sound.md#54-shaping-the-voice-acid_configure_voice).

### `acid_draw_text`

```lua snippet
acid_draw_text(str, x, y, fg, bg)
```

Draws `str` with its top-left corner at `(x, y)` in window-relative coordinates.
Fixed-width bitmap font, **6×8 pixels per glyph**, opaque background: `bg` must
match what is already behind the text. (If `fg` and `bg` are equal, only the lit
pixels are drawn.) A number is converted to a string.

**Clipped to your window glyph by glyph**; it does not clip against your own
border or neighbouring drawing, so a long string runs across them. Truncate with
`s:sub(1, n)`. See [§4.2](04-graphics.md#42-coordinates-and-clipping).

### `acid_draw_window_border`

```lua snippet
acid_draw_window_border()
```

Draws a 1px `THEME_HARD` (`0x00FF66`) outline around the whole window, then cuts
the four rounded corners. **Must be the last call in your redraw**: your content
is drawn over exactly these pixels.

### `acid_draw_window_frame`

```lua snippet
acid_draw_window_frame(title)
```

Fills the 16px title bar with `THEME_PANEL`, draws `title` at `(4, 4)` in
`THEME_TEXT`, and the `THEME_HARD` close dot at its right. The title is **not**
clipped against the close dot: keep it to 16 characters.

### `acid_fill_circle`

```lua snippet
acid_fill_circle(x, y, r, color)
```

Filled circle centred at `(x, y)`, radius `r`, in window-relative coordinates,
clipped to the window. A negative radius draws nothing.

### `acid_fill_rect`

```lua snippet
acid_fill_rect(x, y, w, h, color)
```

Filled rectangle in window-relative coordinates. `color` is 24-bit `0xRRGGBB`.

**Clipped against the window's bounds**: a rectangle running off an edge is
trimmed, one entirely outside draws nothing. You cannot paint outside your own
window.

### `acid_fs_delete`

```lua snippet
local ok, err = acid_fs_delete(path)    -- true, or nil, err
```

Deletes a file. A directory gives `nil, "is a directory"`. Errors: `bad path`
(guard, or the real location is outside the roots), `not found`, `read only`
(cart), or the operating system's own text.

**Cart:** only under `v3/fsroot/Home/`; elsewhere `nil, "read only"`.

### `acid_fs_list`

```lua snippet
local names, err = acid_fs_list(dir)    -- a sequence of names, or nil, err
```

The sorted entry names of directory `dir`, without `.` and `..`. Errors:
`bad path`, `not found`, or the operating system's text (`Not a directory (os
error 20)` for a file). See [§7.8](07-system-apis.md#78-the-file-system).

### `acid_fs_read`

```lua snippet
local bytes, err = acid_fs_read(path)   -- a string, or nil, err
```

The whole file as one string (it may hold any bytes). Errors: `bad path`,
`not found`, or the operating system's text (`Is a directory (os error 21)` for a
directory).

### `acid_fs_rename`

```lua snippet
local ok, err = acid_fs_rename(from, to)    -- true, or nil, err
```

Renames or moves a file or directory; an existing file at `to` is replaced. Both
paths pass the guard and the real-location check. A root itself, and a symlink on
either end, give `nil, "bad path"`. Also `not found` and `read only` (cart).

**Cart:** both ends must be under `v3/fsroot/Home/`; otherwise
`nil, "read only"`.

### `acid_fs_size`

```lua snippet
local bytes, err = acid_fs_size(path)   -- an integer, or nil, err
```

The size in bytes. Also the **existence test**: `acid_fs_size(path) ~= nil`, true
for a directory too (its size is whatever the host reports). Errors: `bad path`,
`not found`.

### `acid_fs_write`

```lua snippet
local ok, err = acid_fs_write(path, data)    -- true, or nil, err
```

Creates the file or replaces it entirely with `data`. There is no append. The
parent directory must exist and the path's real location must lie inside
`v3/apps` or `v3/fsroot`, else `nil, "bad path"`. Writing to a directory gives the
operating system's text.

**Cart:** only under `v3/fsroot/Home/`; elsewhere `nil, "read only"`.

### `acid_get_volume`

```lua snippet
local percent = acid_get_volume()   -- 0 to 100
```

System-wide output gain.

### `acid_get_wallpaper_enabled`

```lua snippet
local on = acid_get_wallpaper_enabled()   -- true / false
```

### `acid_launch_arg`

```lua snippet
local arg = acid_launch_arg()   -- a string
```

The optional startup string this app was spawned with, or `""`. Safe to read more
than once.

### `acid_launcher_count`

```lua snippet
local n = acid_launcher_count()
```

Number of apps in the launcher registry, including those hidden from the Menu
with `menu = false`. At most 48.

### `acid_launcher_name`

```lua snippet
local name = acid_launcher_name(index)   -- a string, or nil
```

The registered display name, or `nil` for an out-of-range index.

### `acid_launcher_path`

```lua snippet
local path = acid_launcher_path(index)   -- a string, or nil
```

The registered script path (`"v3/apps/tetris.lua"`), or `nil`.

### `acid_launcher_register`

```lua snippet
local ok = acid_launcher_register(path, name, w, h, multi, libs)   -- true / false
```

Adds an app to the launcher registry. `multi` is a boolean, `libs` a
comma-separated string (`""` or `nil` for none). Returns `false` if the registry
is full or `w` or `h` is outside 1 to 640 and 1 to 360.

Called by `desktop.lua`'s boot-time manifest scan. You would only call it
yourself when writing a replacement desktop.

**Cart:** always `false`.

### `acid_launcher_spawn`

```lua snippet
local ok = acid_launcher_spawn(index)   -- true / false
```

Launches the registered app at `index` with its recorded size, `multi` flag and
`libs`. Returns `true` on success, **including** when the app was a singleton and
already open, in which case its existing window is raised and focused. `false`
for an unknown index or when no window could be made.

**Cart:** `false` when the app is single-instance and already open; its window
is not raised or focused. Also `false` while 4 or more cart-level windows are
open (the cart's own included), and once the cart's own window has been
closed. Otherwise launching works as for a built-in app, except that the app it
starts runs cart-level. A built-in caller never raises a copy of a singleton that a cart started: it opens a trusted window of its own instead.

### `acid_local_time`

```lua snippet
local year, month, day, hour, min, sec = acid_local_time()
```

Wall-clock local time as six integers (month 1 to 12, hour 0 to 23). A platform
with no clock reports `1970, 1, 1, 0, 0, 0`. See
[§7.9](07-system-apis.md#79-the-clock).

### `acid_mem_used_kb`

```lua snippet
local kb = acid_mem_used_kb()   -- an integer, or -1
```

Kilobytes of memory in use: on the hosted build the resident size of the whole OS
process, shared by every app. `-1` when unknown.

### `acid_network_info`

```lua snippet
local hostname, ip, connected = acid_network_info()
```

The host name, the first non-loopback IPv4 address as a dotted string, and
whether one was found. With no address: the host name, `"none"`, `false`. With
nothing known: `"unknown"`, `"none"`, `false`. `connected` means an address was
found, **not** that anything is reachable. There is no live check and no socket
API.

### `acid_notify_redraw_done`

```lua snippet
acid_notify_redraw_done()
```

Signals that this window has finished the repaint triggered by a moved event.
**Today it is a harmless no-op**: the compositor does not wait for apps.
`AcidApp` and `AcidGame` still call it, and a hand-written loop may. See
[§3.2](03-app-lifecycle.md#moved-and-acid_notify_redraw_done).

### `acid_now_ms`

```lua snippet
local ms = acid_now_ms()
```

Milliseconds since the platform started, as an integer. Monotonic. The clock for
timing, and what `AcidGame` paces its ticks with. See
[§7.9](07-system-apis.md#79-the-clock).

### `acid_overlay_clear`

```lua snippet
acid_overlay_clear()
```

Fills the whole overlay canvas with the transparency key (`0xFF00FF`). No-op if
you do not own the overlay.

### `acid_overlay_close`

```lua snippet
acid_overlay_close()
```

Hides the overlay and releases your claim. No-op unless you are the current
owner, so you can never close someone else's animation. Released automatically if
your app exits.

### `acid_overlay_fill_rect`

```lua snippet
acid_overlay_fill_rect(x, y, w, h, color)
```

Filled rectangle on the overlay, in **screen-absolute** coordinates on a 640×360
screen. Clipped against the screen, so negative coordinates are normal. Silently
draws nothing if you do not own the overlay.

### `acid_overlay_open`

```lua snippet
local ok = acid_overlay_open()   -- true / false
```

Claims the kernel's single full-screen overlay canvas and clears it to the
transparency key. `false` means another task already holds it, which is **not an
error**: an effect that cannot start should simply do nothing. Re-opening from
the owning task succeeds and re-clears. See
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

Sets pitch, sets sustain level from `volume`, and gates the envelope on,
restarting it from zero, so retriggering replays the attack. A voice that has
never had a pitch and is not arpeggiating silently does nothing (this avoids a DC
thump). The voice is recorded as owned by the calling app, and released when that
app ends.

**The note sounds until `acid_stop_note`.** There is no duration argument.

### `acid_poll_event`

```lua snippet
local kind, a, b, c = acid_poll_event(timeout_ms)
```

Blocks on this window's event queue for up to `timeout_ms` milliseconds and
returns the next event as **several values**, the first of which names it:

| Returns | Meaning |
|---|---|
| nothing | Timed out, nothing waiting |
| `"close"` | Close button, or the kernel ending this app |
| `"moved"` | Window was dragged; repaint, then `acid_notify_redraw_done` (the router does not send this today) |
| `"key", code, pressed` | Key event; `code` is ASCII or an `AcidKeys` constant. Only presses are generated, so `pressed` is always `true` |
| `"touch", x, y, pressed` | Touch event, window-relative; may fall outside the window during a drag |

A negative `timeout_ms` counts as `0`, and `0` does not block, so a loop that
passes it spins a CPU core. `AcidApp` clamps to a minimum of 1.

Each return from this call restarts the "stopped responding" clock
([§7.10](07-system-apis.md#710-limits)).

### `acid_refresh_tasks`

```lua snippet
local n = acid_refresh_tasks()
```

Samples the task table (the OS process's threads on the hosted build) and returns
how many it found, at most 16. Call before [`acid_task_info`](#acid_task_info).

### `acid_repaint_region`

```lua snippet
acid_repaint_region(x, y, w, h)
```

Fills that region **of your own canvas** with the wallpaper (or `THEME_BG` when
the wallpaper is off), erasing your claim on it. The next composite shows
whatever is really underneath. Despite the name it asks nothing of any other
window. The wallpaper pixels are those at the *same coordinates*, so it is only
right for a window at the screen's `(0, 0)`.

### `acid_send_self_to_back`

```lua snippet
acid_send_self_to_back()
```

Drops the calling app's own window to the back of the z-order. Always targets the
caller.

### `acid_set_ring_partner`

```lua snippet
acid_set_ring_partner(voice, partner)
```

Pairs `voice` with `partner` (0–7) for ring modulation, or clears it if `partner`
is negative. A partner above 7 is ignored. **Audible only on a `TRIANGLE` voice**:
this matches the real SID, whose ring modulator is wired into the triangle
generator. The partner contributes only its oscillator phase and need not be
gated on.

### `acid_set_volume`

```lua snippet
acid_set_volume(percent)
```

System-wide output gain, 0–100, clamped. A **system setting**: the Config app owns
it. Scale your own note volumes instead. Open to carts.

### `acid_set_wallpaper_enabled`

```lua snippet
acid_set_wallpaper_enabled(enabled)
```

System-wide. Flips the flag and asks the compositor to recomposite. Open to carts.

### `acid_skipped_frames`

```lua snippet
local n = acid_skipped_frames()
```

Frames the compositor skipped because nothing was dirty.

### `acid_spawn_app`

```lua snippet
local ok = acid_spawn_app(path, w, h, arg)   -- true / false; arg optional
```

Launches an app by script path. The `multi` flag and `libs` come from the
registry, looked up by **exact path**: an unregistered path gets singleton
behaviour and no modules. `arg` is the startup string; `""` or `nil` for none.

Returns `false` if the size is outside 1 to 640 by 1 to 360 or no window slot is
free. A path that is neither a `.lua` nor a `.wasm` file under `v3/apps` or
`v3/fsroot` still returns `true`: the new app then ends at once, logging
`Acid OS v3: refused unsafe script path <path>`. Already-open singletons are
raised instead of spawned again.

Canonicalise paths that came from the filesystem with
`AcidApp:canonical_app_path`: `v3/fsroot/App` is a symlink to `v3/apps` and the
registry does not follow it.

**Cart:** `false` unless `path` starts with `v3/apps/`. Also `false` when that
app is single-instance and already open: its window is not raised or focused.
Also `false` while 4 or more cart-level windows are open (the cart's own
included), and once the cart's own window has been closed. The app it starts
runs cart-level, even one that would otherwise be built-in. A built-in caller never raises a copy of a singleton that a cart started: it opens a trusted window of its own instead.

### `acid_stop_note`

```lua snippet
acid_stop_note(voice)
```

Gates the voice off (starting its release) and stops any arpeggio it was running.
**Unconditional**: it silences whichever voice is there regardless of which app
started it.

### `acid_task_count`

```lua snippet
local n = acid_task_count()
```

Tasks in the most recent [`acid_refresh_tasks`](#acid_refresh_tasks) sample.

### `acid_task_info`

```lua snippet
local name, state, cpu_percent = acid_task_info(index)   -- or nothing
```

`state` is a string (`"running"`, `"blocked"`, `"suspended"`, `"deleted"`,
`"?"`). `cpu_percent` is an integer, 0 on the first sample. `name` is the thread
name, truncated to 15 characters by Linux. Nothing is returned for an
out-of-range index.

### `acid_trigger_arp`

```lua snippet
acid_trigger_arp(voice, note0, note1, note2, note3, count, rate_ms)
```

Steps `voice` through `count` of the four **absolute `ona` values**, cycling
upward with wraparound, `rate_ms` per step.

| Argument | Range | |
|---|---|---|
| `voice` | 0–7 | |
| `note0`–`note3` | 1–88 | Absolute pitches, not offsets. Unused slots must still be valid: pass `1`. |
| `count` | 2–4 | How many slots to use. A `count` outside 2–4 (1 included) does not start the arpeggio; the notes and rate are still stored, and an arpeggio already running restarts from slot 0. |
| `rate_ms` | ms | Per step; 0 or less is one audio sample, capped at 10,000. |

Call **immediately after `acid_play_note`** on the same voice: `acid_play_note`
sets the volume, envelope and gate; this drives pitch-stepping on top. The
pattern restarts at slot 0 on gate-on.

**It cycles forever until `acid_stop_note`.** See
[§6.4](06-games.md#64-the-sound-effect-lifecycle).

### `acid_window_info`

```lua snippet
local name, x, y, w, h, focused = acid_window_info(index)   -- or nothing
```

The window's name (its script path), screen coordinates, size and whether it holds
focus. Nothing is returned for an empty, negative or out-of-range slot: walk `0`
to `acid_window_max() - 1` and skip the gaps.

### `acid_window_max`

```lua snippet
local n = acid_window_max()   -- 8
```

Capacity of the window table; the exclusive upper bound for
[`acid_window_info`](#acid_window_info) indices.

---

## Library modules

All in `v3/apps/lib/`. The five core modules are loaded into every app; anything
else goes in the manifest's `libs`
([§2.3](02-apps-and-manifests.md#23-loading-modules)).

### `AcidApp`: always loaded

Base class for event-driven apps. [Chapter 3](03-app-lifecycle.md).

```lua snippet
local MyApp = AcidApp:extend("MyApp")   -- the string is the class name
MyApp:new():start()
```

| Member | |
|---|---|
| `on_create()` | Once, before the first paint |
| `on_touch(x, y, pressed)` | Window-relative touch |
| `on_key(code, pressed)` | Key event; focused window only |
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

`AcidApp` subclass with a fixed-tick loop. [Chapter 6](06-games.md).

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

These are the values of the fields on `AcidKeys`. Printable keys arrive as their
ASCII byte.

### `AcidPalette`: always loaded

```lua snippet
local colour = AcidPalette.hue(step)          -- 256-step HSV hue wheel, 0xRRGGBB
local colour = AcidPalette.hue(step, steps)   -- a wheel of `steps` divisions
```

Integer-only, full saturation and value. `step` wraps. For **content**; use the
theme colours for UI. [§4.1](04-graphics.md#41-colours).

### `AcidWaveform`: always loaded

```lua snippet
local names = { PULSE = 0, SAW = 1, TRIANGLE = 2, NOISE = 3 }
```

The fields of `AcidWaveform`, for [`acid_configure_osc`](#acid_configure_osc).

### `AcidSprite`: add `lib/acid_sprite.lua` to `libs`

Draws character-grid pictures onto the **overlay** (screen-absolute).

```lua snippet
AcidSprite.draw(rows, x, y, scale, palette, flip)   -- flip is optional
local cols = AcidSprite.width(rows)    -- in characters
local lines = AcidSprite.height(rows)  -- in characters
```

`rows` is a sequence of equal-length strings, one character per pixel; `.` is
transparent; `palette` maps character to `0xRRGGBB`, and a character with no
entry is transparent too. Runs of identical colour are merged into single
rectangles. The caller must own the overlay, so a cart draws nothing.
[§4.6](04-graphics.md#46-sprites).

## Theme colours

Defined in the kernel (`acid-kernel`'s `theme.rs`), not exposed to Lua. Declare the
ones you use as your own constants.

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
| Screen | 640 × 360 |
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

A WASM cart calls the same operations through imports, not Lua globals. Where
the Lua call returns `nil, message` (the file system and host cart folder
calls), the import returns a negative error code (`-1` not found, `-2` bad
path, `-3` read only, `-4` not allowed, `-5` too big, `-6` other). Where a cart
is refused with `false` (`acid_overlay_open`, `acid_launcher_register`,
`acid_launcher_spawn`, `acid_spawn_app`), the import returns `0`. The one
exception is `acid_close_window`, whose import returns `-4` to a cart. Fuel and
memory limits are different too. All of that is in
[chapter 10](10-wasm-carts.md).

---

[← Cookbook](08-cookbook.md) · [Contents](README.md) · [Next: WASM carts →](10-wasm-carts.md)
