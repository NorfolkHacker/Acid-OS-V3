---
name: running-acid-os
description: Use when launching, screenshotting, or driving the Acid OS v3 app to confirm a change works in the real app rather than in tests. Covers this machine's SteamOS/KDE-Wayland traps: screenshots that come back pure black, xdotool finding no window, clicks that seem ignored, a tool call that exits 144, and a build that dies on limits.h.
---

# Running Acid OS v3

## Overview

Acid OS is a native winit + softbuffer window with a Lua/WASM desktop inside
it. Launching it is one command; **seeing** and **driving** it on this machine
is not, because the session is KDE Wayland and the obvious tools fail silently
rather than erroring.

Core principle: **a black screenshot is a broken capture, not a broken app.**
Verify the capture has colour before concluding anything about the app.

## Quick reference

| Need | Command |
|---|---|
| Run it | `cargo run --release --manifest-path v3/Cargo.toml -p acid-os` |
| Skip the size picker | `-- --screen 800x600` |
| Open an app on boot | `-- --app tetris` (see README for names) |
| Keep test files out of your real ones | `-- --data /tmp/acid-test` |
| Screenshot | `.claude/skills/running-acid-os/capture.sh out.png` |
| Window id / geometry | `xdotool search --name "Acid OS"` |

**Always run from the repository root**, never from inside `v3/`. The desktop
resolves apps by paths relative to the root, so from elsewhere no app launches.

## Launching so you can see and drive it

Use the release binary and hide `WAYLAND_DISPLAY`:

```sh
env -u WAYLAND_DISPLAY DISPLAY=:0 \
  ./v3/target/release/acid-os --screen 800x600 --app tetris
```

Hiding `WAYLAND_DISPLAY` pushes winit onto XWayland, which is what makes the
window visible to `xdotool` and capturable at all. Without it the app runs
correctly on native Wayland and is completely invisible to every tool here.

`WINIT_UNIX_BACKEND=x11` does **nothing** — winit 0.30 dropped it. If you find
yourself with a live process and zero matching windows, this is why.

**Launch it as a background task's own foreground process** — no trailing `&`,
no wrapper. The app then stays up across tool calls and its stdout lands in the
task's output file.

**The first mouse press after launch is swallowed.** The window starts
unfocused, so KWin spends that press on focusing it and the app never sees it.
Waiting longer does not help — a press 3s after the window appeared failed just
the same, while the *second* press succeeded either way. Spend one throwaway
press on empty desktop background, then drive normally:

```sh
until wid=$(xdotool search --name "Acid OS" 2>/dev/null | head -1); [ -n "$wid" ]; do :; done
eval "$(xdotool getwindowgeometry --shell "$wid")"
xdotool mousemove $((X+600)) $((Y+300)) sleep 0.3 mousedown 1 sleep 0.15 mouseup 1 sleep 0.4
```

Aim it at bare background, away from any window or the top strip, so absorbing
it changes nothing. Keyboard input does not need this — `poll_key` is queued.

## Driving it

Keys go to the focused window, so activate first:

```sh
wid=$(xdotool search --name "Acid OS" | head -1)
xdotool windowactivate --sync "$wid"
xdotool key --delay 120 Up Left Left Down Down     # queued, always arrives
xdotool type --delay 70 "typed into the Editor"    # for text apps
```

Only some keys reach an app at all: printable characters, the arrows, Enter,
Backspace, Escape, Tab and Delete. `Home`, `End`, the function keys, Ctrl and
Alt are dropped before an app sees them (chapter 1, "Driving the window"), so
`End` appearing to do nothing is the design, not a broken send. Steer with the
arrows.

### Never use `xdotool click` here — hold the button instead

`click` sends press and release back to back, and this app will miss it. The
host exposes the pointer as a **state snapshot** (`poll_touch` returns
`*self.touch.lock()`), which the kernel samples once per ~16ms tick, so a press
and release that both land inside one tick are never observed at all. Keys do
not have this problem because `poll_key` pops from a queue, which is why
keyboard driving is reliable and clicking appears to be flaky.

Measured on this machine, clicking `Menu` 5 times each way:

| Method | Menu opened |
|---|---|
| `xdotool ... click 1` | **0 / 5** |
| `mousedown 1`, hold 150ms, `mouseup 1` | **5 / 5** |

So every mouse action takes the form:

```sh
eval "$(xdotool getwindowgeometry --shell "$wid")"   # re-read: KWin moves it
xdotool mousemove $((X+20)) $((Y+11)) sleep 0.4 \
        mousedown 1 sleep 0.15 mouseup 1 sleep 0.4
```

150ms is comfortably more than one tick; don't trim it to the 16ms minimum.
Re-read the geometry too — KWin repositions the host window on its own, and
stale coordinates are a second, independent way to lose a click.

`X`/`Y` are the **client** origin, so `X + rel`, `Y + rel` maps straight from
what you see in a capture (confirm with `xwininfo -id $wid`: `Relative
upper-left` is `0,0`). KWin's titlebar sits *above* it —
`_NET_FRAME_EXTENTS = 1,1,28,1` — so never use a negative relative y, or you
grab the decoration and drag the whole host window instead.

Always confirm a mouse action by capture. A mis-aimed click is not inert here:
the dot at a window's top-right closes it, so a stray click can shut the very
app you meant to drive.

### Driving the Menu dropdown

With the hold method the menu works fully, including launching apps. Both
presses re-read the geometry, so each block stands on its own:

```sh
wid=$(xdotool search --name "Acid OS" | head -1)

# open it: the Menu slot is x < 60, y < 24
eval "$(xdotool getwindowgeometry --shell "$wid")"
xdotool mousemove $((X+20)) $((Y+11)) sleep 0.4 mousedown 1 sleep 0.15 mouseup 1 sleep 0.5

# pick row N from the table below; x must be < 150 (DROPDOWN_W)
N=6
eval "$(xdotool getwindowgeometry --shell "$wid")"
xdotool mousemove $((X+40)) $((Y+24+N*18+9)) sleep 0.4 mousedown 1 sleep 0.15 mouseup 1 sleep 0.8
```

While the dropdown is open the `Menu` button itself reads a highlighted
`Back` — a cheap way to confirm the open state from a capture.

| N | Entry | N | Entry |
|---|---|---|---|
| 0 | About | 4 | File Manager |
| 1 | Load Cart | 5 | Network |
| 2 | Config | 6 | System Monitor |
| 3 | Editor | 7 | Terminal |

**That order is not alphabetical — it is `v3/apps/*.app.toml` sorted by
filename**, with `menu = false` manifests dropped. `Load Cart` comes second
because its manifest is `cart.app.toml`. Most display names happen to match
their filenames, which makes the list look alphabetical and will mislead you on
exactly the two that don't. Re-derive rather than guess if an app is added or
renamed:

```sh
cd v3/apps && for f in $(ls *.app.toml | sort); do
  grep -qE '^\s*menu\s*=\s*false' "$f" || { printf '%s -> ' "$f"; sed -n 's/^\s*name\s*=\s*//p' "$f" | head -1; }
done | cat -n
```

`cat -n` counts from 1 and `N` is 0-based, so `N` is the printed number minus
one — System Monitor prints as 7 and is row 6.

`v3/apps/desktop.lua` (`scan_launchable_apps`, `STRIP_H`, `ITEM_H`,
`DROPDOWN_W`) is the authority for all of this.

`--app <name>` is still the quicker way to get a specific app up at launch;
use the menu when the menu itself is what you're testing.

Keep the `sleep 0.4` between the move and the press as well, so the pointer
position is settled before the button goes down.

Use xdotool's internal `sleep`, never the shell's — foreground `sleep` is
blocked in this harness. To wait on a condition, background an `until` loop.

## Confirming it actually ran

`capture.sh` crops to the window and counts distinct colours, failing if the
result is blank. **Then look at the PNG.** A healthy desktop shows the `Menu`
bar, a clock, a taskbar entry per open app, and the synthwave background.

Capture twice, before and after driving, and compare: pieces stacking where you
steered them, a new taskbar entry, a window appearing. Any of those proves
input reached a **Lua** app, which is the part worth proving — that path runs
through `mlua`, the vendored-C crate most likely to be broken. Don't key the
check to one counter: Tetris's `SCORE` only moves when a line clears, so it can
sit at 0 through a run that worked perfectly.

An in-OS window is only a couple of hundred pixels across inside an 800x600
screen, which is too small to read reliably. Crop to it and upscale with
nearest-neighbour, so the pixel art stays sharp instead of being blurred into
guesswork:

```sh
ffmpeg -i after.png -vf "crop=430:290:35:45,scale=1290:870:flags=neighbor" -y zoom.png
```

Pick the strongest possible evidence: steer to an outcome **gravity alone could
not produce**. A piece flush against the left wall, then the next one flush
right, in the order you sent the commands, cannot be coincidence — whereas
"blocks moved" can be the game playing itself.

## Cleaning up

```sh
pkill -f "[r]elease/acid-os"
```

The brackets are load-bearing. `pkill -f "release/acid-os"` matches the command
line of the very shell running it and kills *that* — your tool call dies with
exit 144 and the app is left running. The bracket makes the pattern not match
itself while still matching the app.

**Give the cleanup its own tool call.** The bracket only stops the *pattern*
from matching itself; it cannot help if the same command line also contains the
real binary path. Combining cleanup with a relaunch —
`pkill -f "[r]elease/acid-os"; ... ./v3/target/release/acid-os ...` — matches
on the launch half, so the shell kills itself before the app ever starts, and
any later wait loop in that command spins until it times out.

## Common mistakes

| Symptom | Cause |
|---|---|
| Screenshot is pure black, ~1.5 KB | `ffmpeg -f x11grab` under KWin. Use `capture.sh`. |
| `xdotool search` finds nothing, process alive | Launched without `env -u WAYLAND_DISPLAY`; app is on native Wayland. |
| Tool call exits 144, app still running | `pkill -f` matched its own shell. Use `pkill -f "[r]elease/acid-os"`. |
| Cleanup+relaunch in one call dies, then a wait loop times out | `pkill` matched the launch path in the same command. Separate calls. |
| App task reports exit 144 | Signal-killed. Expected after your own cleanup `pkill`. |
| `Menu` click does nothing | `xdotool click` is sub-tick and gets missed. Hold the button 150ms. |
| First click after launch does nothing | KWin spent it focusing the window. Send one throwaway press first. |
| An app you were driving vanished | Stray click hit a window's close dot. |
| `End`/`Home`/function keys do nothing | Never delivered to apps by design. Use the arrows. |
| Before/after shots look identical | Window is too small at native size. Crop and upscale with `flags=neighbor`. |
| Build fails at `fatal error: limits.h` | SteamOS ships no libc headers. Run `scripts/refresh-sysroot.sh`. |
| `xdotool` lists no windows at all | You passed `--onlyvisible`; it filters out everything here. |
| Desktop opens but no app does | Launched from `v3/` instead of the repository root. |
| Clicks ignored while keys work | `click` is too fast for the pointer snapshot; also re-read geometry. |
| A drag moved the whole app window | Clicked into KWin's 28px titlebar above the client origin. |
| Capture misaligned, shows the desktop behind | Window moved between the geometry read and the grab. Capture again. |
