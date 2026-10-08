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
| Send it keyboard or mouse input | see [driving-input.md](driving-input.md) |

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
task's output file. Run it from the repository root: a relative
`./v3/target/release/...` fails with exit 127 from anywhere else, and the
session's working directory is not always where you left it.

Then wait for the window with a **bounded** loop, so a launch that failed
reports it instead of hanging:

```sh
for _ in $(seq 100); do
  wid=$(xdotool search --name "Acid OS" 2>/dev/null | head -1)
  [ -n "$wid" ] && break
  xdotool sleep 0.1
done
[ -n "$wid" ] || { echo "no window — check the launch task's output" >&2; exit 1; }
```

An unbounded `until` loop here turns a failed launch into a tool-call timeout
several minutes later, with nothing explaining why.

## Driving it

```sh
wid=$(xdotool search --name "Acid OS" | head -1)
xdotool windowactivate --sync "$wid"
xdotool key --delay 120 Up Left Left Down Down     # queued, always arrives
```

Keyboard is the easy half: key events are queued, so they always arrive. Only
printable characters, the arrows, Enter, Backspace, Escape, Tab and Delete are
delivered at all.

**The mouse is the hard half, and has its own reference:
[driving-input.md](driving-input.md).** Read it before sending any click. The
pointer is exposed to the kernel as a state snapshot rather than a queue, so
`xdotool click` is literally too short to be seen (measured 0/5 against 5/5 for
a held press), early presses after launch get eaten by the window manager, and
coordinates go stale when KWin moves the window. The rule that falls out of all
three: press, verify from a capture, retry — never assume a click landed. It
also carries the Menu dropdown row table and geometry.

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
| Clicks silently do nothing early on | Early presses get eaten. Retry until a capture shows the change. |
| An app you were driving vanished | Stray click hit a window's close dot. |
| `End`/`Home`/function keys do nothing | Never delivered to apps by design. Use the arrows. |
| Before/after shots look identical | Window is too small at native size. Crop and upscale with `flags=neighbor`. |
| Build fails at `fatal error: limits.h` | SteamOS ships no libc headers. Run `scripts/refresh-sysroot.sh`. |
| `xdotool` lists no windows at all | You passed `--onlyvisible`; it filters out everything here. |
| Desktop opens but no app does | Launched from `v3/` instead of the repository root. |
| Clicks ignored while keys work | `click` is too fast for the pointer snapshot; also re-read geometry. |
| A drag moved the whole app window | Clicked into KWin's 28px titlebar above the client origin. |
| Capture shows another window's content | X11 geometry has diverged from the painted window. Relaunch; see driving-input.md. |
