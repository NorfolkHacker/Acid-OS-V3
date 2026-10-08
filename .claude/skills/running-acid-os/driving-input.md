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

## Activate the window once before driving it

KDE consumes a press that lands on an **unfocused** window to focus it, so
early presses appear to vanish. Raising is not enough — raising changes stacking,
not focus. Activate once, before the first press:

```sh
xdotool windowactivate --sync "$wid"
```

Measured on one instance: a single hold-press on `Menu` opened the dropdown
**0/3** and then **2/3** without this, and **4/4** straight after activating.
Do it once up front, not inside a capture helper — see the warning in
`capture.sh` about why activating during a capture is harmful.

Activating is necessary but, on the evidence here, **not always sufficient**:
see the next section before concluding your press was wrong.

## If no press lands at all, check the display layout

Pointer injection on this setup is sensitive to the monitor configuration, and
this is **not understood** — only correlated, cleanly, across one long session:

- With an external monitor attached (`DP-1 1920x1080` plus the internal panel),
  every hold-press landed. The Menu dropdown and row selection worked
  repeatedly, on many fresh instances.
- After that monitor was disconnected, leaving only `eDP-1 1280x800`, no press
  landed again — on fresh instances, on warm ones, with and without
  `windowactivate`, at the spawn position and after moving the window.

Before blaming your press, rule out the app and the capture:

```sh
xrandr --listmonitors          # did the layout change under you?
# app alive? capture twice a minute apart -- the clock in the strip must differ
```

In the failing state the app was provably live (clock advancing, process up)
and captures were correct — the presses simply never arrived. Keyboard input
is a separate path (`poll_key` is queued) and is the reliable fallback, along
with `--app <name>` to open an app without touching the menu.

If you can explain this, replace this section. Until then, treat "no press ever
lands" as an environment question, not a coordinates question — hunting
coordinates here cost a long debugging detour and produced three wrong
explanations before the display layout was even checked.

## When a capture isn't the app at all

Two ways to get a convincing 800x600 PNG that is not a picture of the app.
Both were originally misread as the window having "moved" or X11 geometry
diverging from where KWin paints. Neither is true: geometry is honest, and
`xdotool windowmove 900 400` on a 1280x800 screen reports back `+479,149`
because KWin clamped it and said so.

**The window was underneath something.** The grab reads whatever is painted at
those screen coordinates, so an obscured window yields the window on top of
it. `capture.sh` now raises first. If you grab the screen yourself, raise it
yourself.

**The crop ran off the edge of the screen.** `ffmpeg`'s `crop` filter **clamps
out-of-range rectangles instead of erroring**: asking for 800x600 at x=1013 of
a 1280-wide grab silently returns 800x600 taken from x=480 — verified
byte-identical to an explicit crop at 480. `capture.sh` now bounds-checks and
refuses.

A window ends up off-screen most easily when the **display layout changes under
it** — unplug a monitor mid-session and the X11 screen shrinks (here
1920x1880 to 1280x800) while existing windows stay where they were. Re-check
`xrandr --listmonitors` if captures start looking wrong for no reason.

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
