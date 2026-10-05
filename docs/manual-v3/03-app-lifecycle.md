# 3. The app lifecycle

[← Apps and manifests](02-apps-and-manifests.md) · [Contents](README.md) · [Next: Graphics →](04-graphics.md)

Every app is built on `AcidApp`, a base class you'll find in
`v3/apps/lib/acid_app.lua`. You make your own app from it, override the parts
you care about, and start it on the last line of your file.

Lua doesn't have classes, so a "class" here is just a table:

- `extend` makes a new class that inherits from `AcidApp`. The name you pass
  becomes the class name.
- `new` makes an instance of it.

```lua snippet
local MyApp = AcidApp:extend("MyApp")
-- override what you need

MyApp:new():start()
```

A few conventions:

- Keep your app's state in fields on `self`, such as `self.count`.
- Define methods with a colon, as in `function MyApp:on_touch(x, y, pressed)`,
  so they receive `self`.
- Put constants on the class table, or in plain `local`s at the top of the file.

When you call `start`, it:

1. calls `on_create`,
2. paints the window once with `redraw`,
3. loops until the window closes, passing each event to your callbacks,
4. calls `on_destroy` when the loop ends.

> **Keep off these field names.** `AcidApp` uses two fields of its own:
> `self.running` (the loop flag, see [§3.2](#32-the-event-loop)) and
> `self.class_name` (where the title comes from). If you use either name for
> your own data, you'll break the loop or the title. Also, a field with the
> same name as a method (`self.redraw = 5`) hides that method, so give your
> data and your methods different names.

## 3.1 The callbacks

Each of these does nothing by default, apart from `redraw`. Override only the
ones you need.

### `on_create`

Called once, before the first paint. This is where you set up your fields,
configure synth voices and read `acid_launch_arg`.

```lua snippet
function MyApp:on_create()
  self.items = {}
  self.selected = 0
  acid_configure_voice(5, 1, 5, 80, 60, 120)
end
```

When another app launches yours, it can pass along a string.
`acid_launch_arg()` gives you that string, or `""` if there wasn't one. This is
how File Manager tells Editor which file to open.

### `on_touch(x, y, pressed)`

`x` and `y` are **relative to your window**: `(0, 0)` is your window's own
top-left corner, not the screen's. `pressed` is `true` for a press or a drag,
and `false` when the button is released.

The system sorts out three things before a touch reaches you:

- **The title bar is never yours.** A press at `y < 16` is used to drag the
  window or close it. You never get a touch that *starts* above `y = 16`.
- **A gesture belongs to the window it started in.** Once a press lands in your
  window, every drag and the final release come to you, *even after the
  pointer leaves your window*. That means `x` and `y` can be **negative, or
  bigger than your window**. Clamp them before you use them to work out what
  was hit.
- **A gesture that started somewhere else never leaks in.** If a drag began on
  a close button, on the desktop strip or over nothing, the window that's
  under the pointer now gets nothing from it.

Watch out for Lua's integer division, `//`. It rounds down, so a touch above
your first row gives a negative row number, not row 0. Check the range
yourself, as this complete app does (it handles keys too, which the next
section covers):

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

Notice the `+ 1` in `ITEMS[i + 1]`. Row numbers start at 0, but Lua lists
start at 1, so you add 1 only at the moment you index the list. All the
built-in apps follow this habit.

> **Presses repeat while held.** While a mouse button is held down, you get
> `pressed == true` over and over, about every 16 ms, not just once. So
> anything that *does* something on a press (firing a shot, switching a mode,
> adding to a counter) needs a guard. Without one, it happens again on every
> tick. `PickerApp` doesn't need a guard, because setting `selected` to the
> same row twice changes nothing. See [§3.5](#35-touch-debouncing).

### `on_key(code, pressed)`

Only the focused window gets key events. `code` is one of two things:

- for a printable ASCII character, its byte value, with Shift already applied
  (you get `A`, not `a` plus a Shift key);
- for a special key, one of the `AcidKeys` constants:

```lua snippet
AcidKeys = {
  ENTER = 257, BACKSPACE = 258, ESCAPE = 259, TAB = 260,
  DELETE = 261, UP = 262, DOWN = 263, LEFT = 264, RIGHT = 265,
}
```

That table is already set up for you by `lib/acid_keys.lua`. It's shown here
so you can see the values. To turn a code into a character, use
`string.char(code)`.

Every key sends two events: `pressed = true` when it goes down and
`pressed = false` when it comes back up. There's no auto-repeat, so holding a
key down sends one press, then one release when you let go. Most apps only
care about presses and start with `if not pressed then return end`, as
`PickerApp` does.

A game that needs to know which keys are *held* keeps a table of them, set
on the press and cleared on the release:

```lua snippet
function MyGame:on_key(code, pressed)
  self.held[code] = pressed or nil
end
```

The release carries the same `code` as its press, even if Shift changed in
between, and it reaches the window that got the press even if another window
has been focused since. Clear the table when your window loses focus anyway
(`self:focused()` turns false): keys pressed while another window was
focused never reach you, so their releases won't either.

Each window has a small queue that holds up to 8 waiting events. If your app
is slow to collect them, the extra events are dropped rather than holding up
the system. So a slow callback can lose keys and touches. **Keep your
callbacks short.**

### `on_resize(w, h)`

Called when the user resized a resizable window, with the new size. Re-run
your layout here; `redraw` follows straight after. The default does nothing.

```lua snippet
function MyApp:on_resize(w, h)
  self.cols = math.floor(w / 6)
end
```

### `on_idle`

Called whenever the app has waited for an event and none arrived. How long it
waits is set by `poll_timeout_ms`, so `on_idle` runs *at most* that often.
This is where an animated app moves on to its next frame.

```lua snippet
function MyApp:on_idle()
  if not self:focused() then return end
  self.phase = self.phase + 1
  self:redraw()
end
```

### `on_destroy`

Called once, after the loop ends, however it ended. Use it to stop any notes
that are playing, close an overlay, or save your data to disk.

```lua snippet
function MyApp:on_destroy()
  acid_stop_note(MyApp.VOICE)
  acid_overlay_close()
end
```

> When your app ends, the system releases any voices it was playing, whatever
> the reason it ended. So if you forget `acid_stop_note` here, you won't leave
> a note droning on. Do it anyway. It gives a clean fade instead of a sudden
> cut, and it's the same habit that keeps your sound right while the app is
> *running*.

`on_destroy` does **not** run if your app stops because of a Lua error, or is
shut down for going over a limit
([§2.6](02-apps-and-manifests.md#26-built-in-and-cart-level-apps)). Those
leave the loop through an error, not an event. The system still cleans up your
window, overlay and voices.

### `redraw`

Repaints the whole window. By default it just draws the empty window:

```lua snippet
function AcidApp:redraw()
  acid_clear_user_area()
  acid_draw_window_frame(self:window_title())
  acid_draw_window_border()
end
```

Yours should follow the same order: **clear, frame, your content, border**.
The border really does have to come last. See
[§4.3](04-graphics.md#43-window-chrome) for why.

## 3.2 The event loop

Here is the loop itself, straight from `acid_app.lua`:

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
    elseif kind == "resized" then
      self:on_resize(a, b)
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

`acid_poll_event(timeout_ms)` waits for the next event for your window. It
returns **several values**, and the first one tells you what kind of event it
is:

| Returns | Meaning |
|---|---|
| nothing (`nil`) | The timeout expired with nothing waiting → `on_idle` |
| `"close"` | The close button, or the kernel ending your app → loop ends |
| `"moved"` | Your window was dragged; repaint and then acknowledge |
| `"resized", w, h` | Your window was resized; re-lay out (`on_resize`), repaint and then acknowledge |
| `"key", code, pressed` | A key event |
| `"touch", x, y, pressed` | A touch event |

Since the kind always comes first, `AcidApp` can read all four values into
`kind, a, b, c` and decide what to do based on `kind`.

You won't normally call `acid_poll_event` yourself, because `AcidApp` does it
for you. You only need it if you write a loop of your own, as `AcidGame` does
([chapter 6](06-games.md)).

### `"moved"` and `acid_notify_redraw_done`

`acid_notify_redraw_done` tells the system that a window which was moved has
finished repainting.

**At the moment it does nothing, and does no harm.** The system doesn't wait
for apps to repaint, and it never actually sends `"moved"`. `AcidApp` and
`AcidGame` handle the event and call the function anyway, so apps will be
ready if `"moved"` is sent in future.

If you write your own loop, handle `"moved"` the way `AcidApp` does, and call
`acid_notify_redraw_done()` after you repaint. `AcidGame` calls it without
repainting, because it repaints everything on its next tick anyway.

### Errors and the loop

If any of your callbacks raises an error, `start` stops and your script ends:

- the window closes,
- the system frees everything the app was using,
- the error is printed to the terminal as `Acid OS v3: <path>: <message>`.

`start` doesn't catch errors for you. If one step can fail without the whole
app needing to stop, wrap that step in `pcall` yourself.

## 3.3 `poll_timeout_ms`

This sets how long `acid_poll_event` waits when nothing is happening, and so
how often `on_idle` runs. The default is **200 ms**.

```lua snippet
function MyApp:poll_timeout_ms()
  return 40      -- ~25 fps
end
```

It's a **method, not a constant**, because the right value can change while
your app runs. For example, the Terminal waits 33 ms between frames while one
of its easter-egg animations is playing, then goes back to 200 ms:

```lua snippet
function TerminalApp:poll_timeout_ms()
  return AcidEggs.active() and AcidEggs.TICK_MS or 200
end
```

This complete app animates at about 25 frames a second, but only while its
window is focused:

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

The loop never waits less than 1 ms: that's what `math.max(..., 1)` is for.
A negative timeout counts as 0, and a timeout of `0` doesn't wait at all. So
without that minimum, `return 0` would keep one CPU core at 100% and call
`on_idle` as fast as it possibly could. The minimum stops that from happening,
but don't rely on it.

**Pick the largest number that still looks right.** Every app runs in its own
thread with its own Lua interpreter. An app that waits 200 ms and wakes five
times a second costs almost nothing.

There's also a "stopped responding" limit
([§2.6](02-apps-and-manifests.md#26-built-in-and-cart-level-apps)). It counts
from the last time your app got control back from `acid_poll_event`, so a long
poll timeout will never trip it. Only a single callback that keeps working for
more than 2 seconds (1 second for a cart) will.

## 3.4 Focus, titles and quitting

### `focused()`

Returns true when your window has keyboard focus. In Acid OS that also means
it's the top window, because focus and stacking order always change together.
It's a method, so call it as `self:focused()`.

Use it to skip work nobody can see:

```lua snippet
function MyApp:on_idle()
  if not self:focused() then return end
  self:advance_animation()
  self:redraw()
end
```

This matters most for apps that redraw themselves **every tick**, like games,
instead of waiting to be asked. See
[§6.3](06-games.md#63-focus-and-the-z-order-trap).

### `window_title`

Your window's title comes from your class name automatically. For example,
`DemoTouchApp` becomes `"Demo Touch"`: a trailing `App` is dropped, the words
are split apart, and the result is cut to 16 characters. The class name is the
string you gave `extend`, so choose it with the title in mind.

To show something else, override the method:

```lua snippet
function CounterApp:window_title()
  return "Counter " .. self.count
end
```

**Keep titles to 16 characters.** Narrow windows are about 140 pixels wide,
and the title is **not cut off at the close button**. A title that's too long
runs straight into it.

### `quit`

Ends the loop from inside your app:

```lua snippet
function MyApp:on_key(code, pressed)
  if pressed and code == AcidKeys.ESCAPE then self:quit() end
end
```

The loop can also end in two other ways: the close button in the title bar,
or the system shutting your app down. Both of those arrive as the `"close"`
event, not through `quit`.

> `quit` only sets `self.running`. `AcidGame` runs its own loop with its own
> flag and ignores that field on purpose, so **`quit` does nothing in a
> game**. See [§6.1](06-games.md#61-acidgame).

## 3.5 Touch debouncing

This is the most common bug in Acid OS apps.

While a touch is held down, you get `pressed == true` on **every tick**, not
once per press. So anything that *does* something on a press needs to happen
once per *hold*, not once per event. "Debouncing" just means filtering out
those repeats:

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

Some things *should* repeat: dragging a paddle, painting, moving a slider.
Those just read `x` and `y` on every event and don't need to remember
anything:

```lua snippet
function MyApp:on_touch(x, y, pressed)
  if not pressed then return end
  self.paddle_x = clamp(x - PADDLE_W // 2)
end
```

The rule is about **doing**, not moving. Ask yourself: "If someone rests their
finger here for a second, should this happen sixty times?" If not, guard it.

There's a second way to do this without a flag, when the action changes some
state: compare against the state you're already in. This complete app is a
four-key keyboard. It plays one note per key, even when you slide the pointer
across the keys:

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

The built-in Piano (`piano.lua`) works the same way. It plays one note per
key, and starts a new note each time the pointer slides onto a different key.

You don't need a special case for "no key": in Lua, `nil == nil` is true, so
the check `offset == self.active_offset` handles it already.

---

[← Apps and manifests](02-apps-and-manifests.md) · [Contents](README.md) · [Next: Graphics →](04-graphics.md)
