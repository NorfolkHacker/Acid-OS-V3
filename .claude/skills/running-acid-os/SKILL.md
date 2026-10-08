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

## Driving it

Keys go to the focused window, so activate first:

```sh
wid=$(xdotool search --name "Acid OS" | head -1)
xdotool windowactivate --sync "$wid"
xdotool key --delay 120 Up Left Left Down Down     # XTEST, reaches the app
xdotool type --delay 70 "typed into the Editor"    # for text apps
```

Only some keys reach an app at all: printable characters, the arrows, Enter,
Backspace, Escape, Tab and Delete. `Home`, `End`, the function keys, Ctrl and
Alt are dropped before an app sees them (chapter 1, "Driving the window"), so
`End` appearing to do nothing is the design, not a broken send. Steer with the
arrows.

**Prefer `--app <name>` over clicking the `Menu` button.** A synthetic single
click on `Menu` did not open the menu in testing (it opened once during a
multi-step press-drag-release sequence and not on two clean retries), so don't
build a check on it. The flag gets an app window up every time.

If you do need the mouse, **re-read the geometry immediately before every
click.** KWin repositions the host window on its own, so coordinates computed a
few actions ago land somewhere else — which looks exactly like the app ignoring
input:

```sh
eval "$(xdotool getwindowgeometry --shell "$wid")"   # now, not earlier
xdotool mousemove $((X+40)) $((Y+200)) sleep 0.5 click 1
```

Always confirm a mouse action by capture. A mis-aimed click is not inert here:
the dot at a window's top-right closes it, so a stray click can shut the very
app you meant to drive.

`X`/`Y` are the **client** origin, so `X + rel`, `Y + rel` maps straight from
what you see in a capture (confirm with `xwininfo -id $wid`: `Relative
upper-left` is `0,0`). KWin's titlebar sits *above* it —
`_NET_FRAME_EXTENTS = 1,1,28,1` — so never use a negative relative y, or you
grab the decoration and drag the whole host window instead.

Keep xdotool's own `sleep` between the move and the click; one combined
`mousemove ... click` can deliver the button before the motion is processed.
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

## Common mistakes

| Symptom | Cause |
|---|---|
| Screenshot is pure black, ~1.5 KB | `ffmpeg -f x11grab` under KWin. Use `capture.sh`. |
| `xdotool search` finds nothing, process alive | Launched without `env -u WAYLAND_DISPLAY`; app is on native Wayland. |
| Tool call exits 144, app still running | `pkill -f` matched its own shell. Use `pkill -f "[r]elease/acid-os"`. |
| App task reports exit 144 | Signal-killed. Expected after your own cleanup `pkill`. |
| `Menu` click does nothing | Known: synthetic clicks on it are unreliable. Use `--app` instead. |
| An app you were driving vanished | Stray click hit a window's close dot. |
| `End`/`Home`/function keys do nothing | Never delivered to apps by design. Use the arrows. |
| Before/after shots look identical | Window is too small at native size. Crop and upscale with `flags=neighbor`. |
| Build fails at `fatal error: limits.h` | SteamOS ships no libc headers. Run `scripts/refresh-sysroot.sh`. |
| `xdotool` lists no windows at all | You passed `--onlyvisible`; it filters out everything here. |
| Desktop opens but no app does | Launched from `v3/` instead of the repository root. |
| Clicks ignored while keys work | Stale geometry — KWin moved the host window. Re-read it before each click. |
| A drag moved the whole app window | Clicked into KWin's 28px titlebar above the client origin. |
| Capture misaligned, shows the desktop behind | Window moved between the geometry read and the grab. Capture again. |
