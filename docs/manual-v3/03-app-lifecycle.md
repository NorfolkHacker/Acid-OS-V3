# 3. The app lifecycle

[← Apps and manifests](02-apps-and-manifests.md) · [Contents](README.md) · [Next: Graphics →](04-graphics.md)

Your app is a subclass of `AcidApp` (`v3/apps/lib/acid_app.lua`), and the last
line of your file starts it. Lua has no classes, so a "class" is a table:
`extend` makes a new one that inherits from `AcidApp`, the name you pass is
the class name, and `new` makes an instance.

```lua snippet
local MyApp = AcidApp:extend("MyApp")
-- override what you need

MyApp:new():start()
```

Per-app state lives in fields on `self` (`self.count`), and methods are defined
with a colon (`function MyApp:on_touch(x, y, pressed)`) so they receive `self`.
Constants are fields on the class table or plain `local`s at the top of the file.

`start` calls `on_create`, paints once with `redraw`, then loops until the
window closes, dispatching events to your callbacks. When the loop ends it calls
`on_destroy`.

> **Keep off these field names.** `AcidApp` itself keeps `self.running` (the
> loop flag, see [§3.2](#32-the-event-loop)) and `self.class_name` (the title
> source). A field of yours with either name breaks the loop or the title. A
> field named like a method (`self.redraw = 5`) hides that method on your
> instance, so give state and behaviour different names.

## 3.1 The callbacks

Every one of these has an empty default (except `redraw`). Override only what
you need.

### `on_create`

Called once, before the first paint. Set up your fields, configure synth voices,
read `acid_launch_arg`.

```lua snippet
function MyApp:on_create()
  self.items = {}
  self.selected = 0
  acid_configure_voice(5, 1, 5, 80, 60, 120)
end
```

`acid_launch_arg()` returns the string another app passed when it launched
yours, or `""` if there was none (File Manager opens Editor on a file this way).

### `on_touch(x, y, pressed)`

`x` and `y` are **window-relative**: `(0, 0)` is your window's own top-left
corner, not the screen's. `pressed` is `true` for a press or a drag, `false` for
a release.

Three things the kernel does before you see a touch:

- **The title bar is never yours.** A press at `y < 16` is consumed by the
  router as a drag or a close. You never receive a touch that *starts* above
  `y = 16`.
- **A gesture belongs to the window it started in.** Once a press lands in your
  window, every drag event and the final release come to you, *even after the
  pointer leaves your window*. So `x` and `y` can be **negative or larger than
  your window**. Clamp before you use them for a hit test.
- **A gesture that started elsewhere never leaks in.** A drag that began on a
  close button, in the desktop strip, or over nothing delivers nothing to
  whichever window happens to be under the pointer now.

Lua's integer division `//` floors, so a touch above your first row gives a
negative index, never row 0. Test the range explicitly, as this complete app
does (it also takes keys; see the next section):

```lua app
-- w: 170
-- h: 120
local PickerApp = AcidApp:extend("PickerApp")

local TITLE_BAR_H = 16
local ROW_H = 14
local ITEMS = { "Alpha", "Bravo", "Charlie", "Delta" }

function PickerApp:on_create()
  self.selected = 0
  self.line = ""
end

function PickerApp:on_touch(x, y, pressed)
  if not pressed then return end
  local row = (y - TITLE_BAR_H) // ROW_H
  if row >= 0 and row < #ITEMS then
    self.selected = row
    self:redraw()
  end
end

function PickerApp:on_key(code, pressed)
  if not pressed then return end
  if code == AcidKeys.UP then
    self.selected = math.max(self.selected - 1, 0)
  elseif code == AcidKeys.DOWN then
    self.selected = math.min(self.selected + 1, #ITEMS - 1)
  elseif code == AcidKeys.ESCAPE then
    self:quit()
  elseif code == AcidKeys.BACKSPACE then
    self.line = self.line:sub(1, -2)
  elseif code >= 32 and code < 127 then
    self.line = self.line .. string.char(code)
  end
  self:redraw()
end

function PickerApp:redraw()
  acid_clear_user_area()
  acid_draw_window_frame(self:window_title())
  for i = 0, #ITEMS - 1 do
    local y = TITLE_BAR_H + i * ROW_H
    local on = (i == self.selected)
    local fg = on and 0x050607 or 0xD4E6DB
    local bg = on and 0x00FF66 or 0x050607
    acid_fill_rect(2, y, 166, ROW_H, bg)
    acid_draw_text(ITEMS[i + 1], 6, y + 3, fg, bg)
  end
  acid_draw_text("> " .. self.line, 6, 16 + 4 * ROW_H + 8, 0xD4E6DB, 0x050607)
  acid_draw_window_border()
end

PickerApp:new():start()
```

Note the `+ 1` at the one place the 0-based `selected` meets a Lua sequence.
Keeping row numbers 0-based and shifting only where you index (`ITEMS[i + 1]`)
is the convention throughout v3's apps.

> **Presses repeat while held.** A mouse button held down delivers
> `pressed == true` repeatedly (once per router tick, about every 16 ms), not
> once. Anything that *acts* on a press (firing a shot, flipping a mode,
> advancing a counter) needs a guard, or it fires on every tick the touch is
> held. `PickerApp` gets away without one because setting `selected` is
> idempotent. See [§3.5](#35-touch-debouncing).

### `on_key(code, pressed)`

Only the focused window receives key events. `code` is the character's byte for
printable ASCII (Shift already applied: `A`, not `a` plus a modifier), or one of
the `AcidKeys` constants:

```lua snippet
AcidKeys = {
  ENTER = 257, BACKSPACE = 258, ESCAPE = 259, TAB = 260,
  DELETE = 261, UP = 262, DOWN = 263, LEFT = 264, RIGHT = 265,
}
```

(That table is already defined for you by `lib/acid_keys.lua`; it is shown here
so you can see the values.) On this hosted build `pressed` is always `true`: the
host delivers key presses only, with no releases and no auto-repeat, so holding
a key sends one event. Test it anyway, as `PickerApp` does, so your app keeps
working if releases are ever added. Turn a code into a character with
`string.char(code)`.

Events wait in a small queue, 8 deep per window. If your app is slow to poll,
the router drops the extra events rather than blocking, so a callback that
takes long enough can lose keys and touches. Keep callbacks short.

### `on_idle`

Fires whenever a poll times out with no event waiting, so its rate is set by
`poll_timeout_ms`, and it fires *at most* that often. This is where an
animating app advances a frame.

```lua snippet
function MyApp:on_idle()
  if not self:focused() then return end
  self.phase = self.phase + 1
  self:redraw()
end
```

### `on_destroy`

Called once, after the loop ends, whichever way it ended. Use it to stop
sounding notes, close an overlay, or flush state to disk.

```lua snippet
function MyApp:on_destroy()
  acid_stop_note(MyApp.VOICE)
  acid_overlay_close()
end
```

> Voices your app gated on are released by the kernel when your app ends, on
> every exit path, so a forgotten `acid_stop_note` here will not leave a note
> droning after your app is gone. Do it anyway: it is the difference between a
> clean fade and an abrupt cut, and it is the habit that keeps your *running*
> app's sound correct.

`on_destroy` does not run if your app dies of a Lua error, or is ended for
exceeding a limit ([§2.6](02-apps-and-manifests.md#26-built-in-and-cart-level-apps)):
those leave the loop by an error, not by an event. The kernel's own cleanup
(window, overlay, voices) still happens.

### `redraw`

Repaints the whole window. The default implementation draws bare chrome:

```lua snippet
function AcidApp:redraw()
  acid_clear_user_area()
  acid_draw_window_frame(self:window_title())
  acid_draw_window_border()
end
```

Yours should follow the same shape, **clear, frame, your content, border**, and
the border genuinely must come last. See [§4.3](04-graphics.md#43-window-chrome).

## 3.2 The event loop

This is the real loop, from `acid_app.lua`:

```lua snippet
function AcidApp:start()
  self:on_create()
  self:redraw()
  self.running = true
  while self.running do
    local kind, a, b, c = acid_poll_event(math.max(self:poll_timeout_ms(), 1))
    if kind == "close" then
      self.running = false
    elseif kind == "moved" then
      self:redraw()
      acid_notify_redraw_done()
    elseif kind == "key" then
      self:on_key(a, b)
    elseif kind == "touch" then
      self:on_touch(a, b, c)
    else
      self:on_idle()
    end
  end
  self:on_destroy()
end
```

`acid_poll_event(timeout_ms)` blocks on your window's event queue and returns
**several values**, the first of which names the event:

| Returns | Meaning |
|---|---|
| nothing (`nil`) | The timeout expired with nothing waiting → `on_idle` |
| `"close"` | The close button, or the kernel ending your app → loop ends |
| `"moved"` | Your window was dragged; repaint and then acknowledge |
| `"key", code, pressed` | A key event |
| `"touch", x, y, pressed` | A touch event |

Because the first value is always the kind, `AcidApp` can read all four values
into locals (`kind, a, b, c`) and dispatch on `kind`. You normally never call
`acid_poll_event` yourself; `AcidApp` does it. You only reach for it when
writing a loop of your own, as `AcidGame` does ([chapter 6](06-games.md)).

### `"moved"` and `acid_notify_redraw_done`

`acid_notify_redraw_done` tells the compositor a moved window has repainted.
**Today it is a harmless no-op**: the compositor does not wait on apps, and the
router does not send `"moved"` at all. `AcidApp` and `AcidGame` still handle the
event and call the function, so an app is ready if `"moved"` is sent in future. If you write your own loop, handle
the event the way `AcidApp` does and call `acid_notify_redraw_done()` after any
repaint; `AcidGame` calls it without repainting, because it repaints everything
on its next tick anyway.

### Errors and the loop

An error in any callback leaves `start`, which ends your script: the window
closes, the kernel frees the app's resources, and the message goes to the
terminal as `Acid OS v3: <path>: <message>`. Nothing in `start` catches errors
for you. If a failure in one step should not end the whole app, wrap that step
in `pcall` yourself.

## 3.3 `poll_timeout_ms`

How long `acid_poll_event` blocks when nothing is waiting, and therefore how
often `on_idle` fires. The default is **200 ms**.

```lua snippet
function MyApp:poll_timeout_ms()
  return 40      -- ~25 fps
end
```

It is a **method, not a constant**, because the right answer changes while the
app runs. The Terminal returns a 33 ms frame interval while one of its
easter-egg animations is in flight and drops back to 200 afterwards:

```lua snippet
function TerminalApp:poll_timeout_ms()
  return AcidEggs.active() and AcidEggs.TICK_MS or 200
end
```

This complete app animates at about 25 frames a second, and only while it is
the focused window:

```lua app
-- w: 160
-- h: 90
local PulseApp = AcidApp:extend("PulseApp")

function PulseApp:on_create()
  self.phase = 0
end

function PulseApp:poll_timeout_ms()
  return 40
end

function PulseApp:on_idle()
  if not self:focused() then return end
  self.phase = self.phase + 3
  self:redraw()
end

function PulseApp:redraw()
  acid_clear_user_area()
  acid_draw_window_frame(self:window_title())
  local colour = AcidPalette.hue(self.phase)
  acid_fill_rect(20, 30, 120, 40, colour)
  acid_draw_text("PHASE " .. self.phase % 256, 40, 46, 0x050607, colour)
  acid_draw_window_border()
end

PulseApp:new():start()
```

The loop clamps the wait to a minimum of 1 ms with `math.max(..., 1)`. The host
treats a negative timeout as 0, and `0` does not block at all, so an unclamped
`return 0` would spin a CPU core at 100% and fire `on_idle` as fast as the
thread can go. The clamp prevents the spin; don't rely on it.

**Pick the largest number that still looks right.** Every app is an OS thread
with its own VM, and a 200 ms poll that wakes five times a second costs almost
nothing.

The "stopped responding" limit ([§2.6](02-apps-and-manifests.md#26-built-in-and-cart-level-apps))
is measured from the last time your app *returned* from `acid_poll_event`, so a
long poll timeout never trips it. Only a single callback that computes for more
than 2 seconds (1 second for a cart) does.

## 3.4 Focus, titles and quitting

### `focused()`

True when your window holds keyboard focus, which in this OS also means it is
the topmost visible window, because focus and z-order always change together.
It is a method: `self:focused()`.

Use it to skip work you cannot see:

```lua snippet
function MyApp:on_idle()
  if not self:focused() then return end
  self:advance_animation()
  self:redraw()
end
```

This matters most for apps that paint **outside** the compositor's z-order-aware
repaint; see [§6.3](06-games.md#63-focus-and-the-z-order-trap).

### `window_title`

Derived from your class name automatically: `DemoTouchApp` → `"Demo Touch"`
(trailing `App` dropped, camel case split, capped at 16 characters). The name is
the string you gave `extend`, so pick it with the title in mind. Override the
method for something custom:

```lua snippet
function CounterApp:window_title()
  return "Counter " .. self.count
end
```

16 characters is a real limit, not a style note: narrow windows are around 140px
and the title is **not clipped against the close button**, so an overlong title
runs straight into it.

### `quit`

Ends the loop from inside your app:

```lua snippet
function MyApp:on_key(code, pressed)
  if pressed and code == AcidKeys.ESCAPE then self:quit() end
end
```

The other two ways the loop ends, the title-bar close button and the kernel
ending your app, both arrive as the `"close"` event, not through this method.

> `quit` only sets `self.running`. `AcidGame` runs its own loop with its own
> local flag and deliberately ignores that field, so **`quit` does nothing in a
> game**. See [§6.1](06-games.md#61-acidgame).

## 3.5 Touch debouncing

The single most common bug in an Acid OS app.

A held touch delivers `pressed == true` on **every router tick**, not once per
press. Anything that acts on a press therefore needs to fire once per *hold*, not
once per event:

```lua snippet
function MyApp:on_create()
  self.touch_down = false
end

function MyApp:on_touch(x, y, pressed)
  if not pressed then
    self.touch_down = false      -- release: re-arm
    return
  end
  if self.touch_down then return end   -- still the same hold: ignore
  self.touch_down = true
  self:fire()                    -- runs exactly once per press
end
```

Some interactions genuinely *want* the repeat: dragging a paddle, painting,
scrubbing a slider. Those read `x`/`y` on every event and hold no state:

```lua snippet
function MyApp:on_touch(x, y, pressed)
  if not pressed then return end
  self.paddle_x = clamp(x - PADDLE_W // 2)
end
```

The rule is about **acting**, not about moving. Ask: "if the user rests their
finger here for a second, should this happen sixty times?" If not, guard it.

A second pattern does the same job without a flag, when the action is a state
change: compare against what is already true. This complete app is a four-key
keyboard that sounds one note per key press, even as a pointer slides across the
keys:

```lua app
-- w: 180
-- h: 80
local KeysApp = AcidApp:extend("KeysApp")

local VOICE = 5
local ROOT_ONA = 40
local KEY_W = 40
local TITLE_BAR_H = 16

function KeysApp:on_create()
  self.active_offset = nil
  acid_configure_voice(VOICE, 1, 5, 80, 60, 120)
end

-- Which key (0..3) is under x, or nil outside the keyboard.
function KeysApp:hit_test(x)
  local key = x // KEY_W
  if key < 0 or key > 3 then return nil end
  return key * 2
end

function KeysApp:on_touch(x, y, pressed)
  if not pressed then
    if self.active_offset ~= nil then acid_stop_note(VOICE) end
    self.active_offset = nil
    self:redraw()
    return
  end
  local offset = self:hit_test(x)
  if offset == self.active_offset then return end   -- same key still held
  self.active_offset = offset
  if offset == nil then
    acid_stop_note(VOICE)
  else
    acid_play_note(VOICE, ROOT_ONA + offset, 45)
  end
  self:redraw()
end

function KeysApp:on_destroy()
  acid_stop_note(VOICE)
end

function KeysApp:redraw()
  acid_clear_user_area()
  acid_draw_window_frame(self:window_title())
  for key = 0, 3 do
    local down = (key * 2 == self.active_offset)
    local colour = down and 0x00FF66 or 0xD4E6DB
    acid_fill_rect(key * KEY_W + 2, TITLE_BAR_H + 4, KEY_W - 4, 50, colour)
  end
  acid_draw_window_border()
end

KeysApp:new():start()
```

That is how `piano.lua` gets one note per key press while a pointer slides across
the keyboard, retriggering correctly at each boundary. Note `offset == nil`
compares fine with `self.active_offset == nil` in Lua, so the "no key" state
needs no special case in the guard.

---

[← Apps and manifests](02-apps-and-manifests.md) · [Contents](README.md) · [Next: Graphics →](04-graphics.md)
