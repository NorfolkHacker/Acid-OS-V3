# Driving Acid OS: keyboard and mouse mechanics

Reference for `running-acid-os`. Read this before sending the app any input.

Keyboard is easy and mouse is not, for one reason: the host hands the kernel a
**queue** for keys and a **state snapshot** for the pointer.

```rust
fn poll_key(&self) -> Option<KeyEvent> { self.keys.lock().unwrap().pop_front() }  // queue
fn poll_touch(&self) -> TouchState     { *self.touch.lock().unwrap() }            // snapshot
```

Every key event is buffered and eventually seen. The pointer is only ever
sampled, once per ~16ms kernel tick, so anything that happens entirely between
two ticks never happened at all. Most mouse trouble here is a consequence.

## Keyboard

```sh
wid=$(xdotool search --name "Acid OS" | head -1)
xdotool windowactivate --sync "$wid"
xdotool key --delay 120 Up Left Left Down Down     # queued, always arrives
xdotool type --delay 70 "typed into the Editor"    # for text apps
```

Keys go to the focused window, so activate first.

Only some keys reach an app at all: printable characters, the arrows, Enter,
Backspace, Escape, Tab and Delete. `Home`, `End`, the function keys, Ctrl and
Alt are dropped before an app sees them (manual chapter 1, "Driving the
window"). `End` appearing to do nothing is the design, not a failed send —
steer with the arrows.

## Never use `xdotool click` — hold the button

`click` sends press and release back to back, which is exactly the sub-tick
case the pointer snapshot cannot see. Measured on this machine, clicking `Menu`
5 times each way:

| Method | Menu opened |
|---|---|
| `xdotool ... click 1` | **0 / 5** |
| `mousedown 1`, hold 150ms, `mouseup 1` | **5 / 5** |

So every mouse action takes this form:

```sh
eval "$(xdotool getwindowgeometry --shell "$wid")"   # re-read: KWin moves it
xdotool mousemove $((X+20)) $((Y+11)) sleep 0.4 \
        mousedown 1 sleep 0.15 mouseup 1 sleep 0.4
```

150ms is comfortably more than one tick; don't trim it toward 16ms. Keep the
`sleep 0.4` after the move too, so the pointer has settled before the button
goes down. Use xdotool's internal `sleep`, never the shell's — foreground
`sleep` is blocked in this harness; to wait on a condition, background an
`until` loop.

## Early presses get lost — verify and retry, don't count

The first press after launch is normally eaten: the window starts unfocused and
KWin spends it on focusing. Waiting does not help — a press 3s after the window
appeared failed just the same, while the *second* press succeeded either way.

But **it is not reliably exactly one**. On a freshly launched instance a press
was still lost after a throwaway had already been spent, and the one after that
worked. A measured 5/5 for single presses was taken on an instance already
warmed up by many interactions, so don't read determinism into it.

So never assume a press landed. Press, check, press again — and make the check
the thing you actually want, not a press count:

```sh
open_menu() {                 # press Menu until the dropdown is actually open
  local wid i
  wid=$(xdotool search --name "Acid OS" | head -1)
  for i in 1 2 3 4 5; do
    eval "$(xdotool getwindowgeometry --shell "$wid")"
    xdotool mousemove $((X+20)) $((Y+11)) sleep 0.4 mousedown 1 sleep 0.15 mouseup 1 sleep 0.5
    menu_is_open && return 0
  done
  echo "menu never opened after 5 presses" >&2; return 1
}
```

`menu_is_open` is yours to define from a capture — the simplest reliable test
is that the `Menu` button has become `Back` (see below), or that the region
below the strip stopped matching a known closed-state baseline. Make that test
able to fail loudly: if it cannot tell "open" from "the capture is misaligned",
a retry loop will happily press empty space (see the limitation above).

The retry is safe here only because the menu *toggles*: an extra press that did
land just closes it again, which `menu_is_open` then catches. Don't blind-retry
an action that is not idempotent or reversible — re-sending a row selection
could spawn a second app.

Keyboard input needs none of this — `poll_key` is queued.

## Known limitation: X11 coordinates can diverge from the painted window

**Unresolved. Check for it before trusting any click.** Under XWayland, the
geometry X11 reports is not guaranteed to be where KWin actually paints the
window. Observed on a freshly launched instance: `xdotool getwindowgeometry`
and `xwininfo` both agreed the window was at `+1013,147` and both were wrong —
a crop at those coordinates showed a *different application's* content in its
left third. Click coordinates computed the same way miss the app entirely,
which looks exactly like input being ignored.

It behaves while the window is left where it spawned, and it is reliably
reproducible after the host window has been moved — including moved
accidentally, by a press that landed in KWin's titlebar.

Guard against it, cheaply: a correct capture of this app always has the dark
top strip with `Menu` at its left edge. If a capture shows anything else —
another window's content, a shifted image, an improbable colour count for a
limited palette — the coordinates have diverged. Do not retry the click;
nothing is wrong with your press. **Relaunch the app** and avoid dragging the
host window.

Do not use a press-retry loop as a workaround for this. A lost press and a
mis-aimed press look identical from outside, so a retry loop on diverged
coordinates just presses empty space five times and reports failure, which is
how this was found.

## Coordinates

`X`/`Y` from `xdotool getwindowgeometry` are the **client** origin, so
`X + rel`, `Y + rel` maps straight from what you measure in a capture (confirm
with `xwininfo -id $wid`: `Relative upper-left` is `0,0`). KWin's titlebar sits
*above* that origin — `_NET_FRAME_EXTENTS = 1,1,28,1` — so never use a negative
relative y, or you grab the decoration and drag the whole host window.

Re-read the geometry immediately before **every** click. KWin repositions the
host window on its own, and stale coordinates are a second, independent way to
lose a click — one that looks identical to the app ignoring you.

Always confirm a mouse action by capture. A mis-aimed click is not inert here:
the dot at a window's top-right closes it, so a stray click can shut the very
app you meant to drive.

## The Menu dropdown

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
`Back` — a cheap way to confirm the open state from a capture. Selecting a row
closes the menu.

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
exactly the one that doesn't. Re-derive rather than guess if an app is added or
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

`--app <name>` is the quicker way to get a specific app up at launch; use the
menu when the menu itself is what you're testing.
