# 8. Cookbook

[← System APIs](07-system-apis.md) · [Contents](README.md) · [Next: API reference →](09-api-reference.md)

Handy recipes, good habits, and the mistakes that tend to cost the most
time.

## 8.1 Testing an app without the window

Normally your app runs on its own thread, in a real window. That's the right
way to check your layout looks good. It's the wrong way to check something like
"does my sound code really stop every voice?"

For that kind of check there's a **headless harness**: a way to run your app
with no window at all. It's a Lua file that swaps every `acid_*` call for a
stand-in (a "stub") that just *records* what it was called with. Your app is
loaded on top, and then your test checks the records. All the built-in apps are
tested this way, from `v3/tools/`.

The harness is `v3/tools/game_test_env.lua`. It's loaded **before** your app,
and it gives you:

- **Recording stubs for the audio calls.** `NOTES` holds `{"play", voice, ona,
  volume}`, `{"arp", ...}` and `{"stop", voice}` entries, in order.
- **Stubs for the drawing calls.** These fill `RECTS` and `TEXTS`, and count
  calls in `DRAW_CALLS`.
- **A clock you control.** `acid_now_ms` returns `CLOCK`, so a test moves time
  forward by setting it.
- **Fake system calls**, backed by plain tables you can set directly: `FS`,
  `WINDOWS`, `LAUNCHER`, `TASKS`, `TIME`, `NETWORK` and `VOLUME`. There's also
  a `CALLS` log of everything that would change the system (spawns, closes,
  activations).
- **Stand-in `AcidApp` and `AcidGame` classes.** Their `start` runs
  `on_create` and stores your app object in the global `GAME`, **without
  entering the event loop**. So the last line of your app,
  `MyApp:new():start()`, hands you the object to poke at.
- **Three checking helpers:** `eq(actual, expected, what)`, `ok(cond, what)`
  and `group(name)`. They count passes and collect failures.

A test file then drives the app by calling its methods:

```lua snippet
-- v3/tools/test_mygame.lua
local G = GAME

group("shooting")
G:on_create()
G:on_touch(100, 60, true)
eq(NOTES[#NOTES][1], "play", "a tap plays the shot sound")

for _ = 1, 10 do G:on_tick() end
local last = NOTES[#NOTES]
eq(last[1], "stop", "and the shot sound is stopped again within ten ticks")
```

To run it, add a test to `v3/crates/acid-lua/tests/game_tests.rs`. It lists
the files in the order they load (the core libraries, then the harness, then
your app, then your test), plus the number of checks the test should make:

```rust
#[test]
fn mygame() {
    run_suite(&with_libs(&["v3/tools/game_test_env.lua", "v3/apps/mygame.lua", "v3/tools/test_mygame.lua"]), 2);
}
```

The number has to match exactly. That way a test can't quietly pass by
checking less than it used to. Run it with:

```sh
cargo test --manifest-path v3/Cargo.toml -p acid-lua --test game_tests
```

What to look for:

- **It passes.** Any Lua error in your app shows up with a file and line
  number.
- **Every `"play"` is followed, sooner or later, by a `"stop"`** for the same
  voice. If not, that's a note you never stopped.
- **The draw counts look sensible.** A tick that makes tens of thousands of
  `acid_fill_rect` calls is repainting far more than it needs to.

The stubs aren't the real system. They won't tell you that something is cut off
at the edge of the window, or that the border is missing. For that, run the
app. Or make it a `lua app` example in this manual: those run on the real
system, and the tests fail unless the app leaves its window open with the
border drawn.

## 8.2 Conventions worth following

**Declare `WINDOW_W`/`WINDOW_H` and keep them matched to the manifest.**
Nothing checks this for you. See [§2.2](02-apps-and-manifests.md#sizing-a-window).

**Name your theme colours.** `local BG_COLOR = 0x050607 -- THEME_BG` is easier
to read than the same number in twenty places. Every built-in app does this.

**Declare things `local`.** In Lua, a global is shared by everything in your
app, including the core libraries. A stray `count = 0` makes a global that can
clash with one of theirs. `local count = 0` can't.

**Use `:` for methods and `.` for plain functions.** `self:redraw()` passes
`self` along, but `self.redraw()` doesn't. Then the first line of `redraw` that
uses `self` fails with "attempt to index a nil value". This is the most common
error in a new app.

**Set up every synth voice you use in `on_create`.** The default sound is a
harsh, unfiltered pulse that starts and stops instantly. See
[§5.4](05-sound.md#54-shaping-the-voice-acid_configure_voice).

**Pick voice numbers nobody else uses, and say which in a comment.** See the
table in [§5.1](05-sound.md#voice-ownership-and-voice-stealing).

**Comment the *why*, not the *what*.** The comments in Acid OS explain
decisions: why the border is drawn last, why the envelope maths is done in whole numbers,
why `tick_sfx` has no guard. Do the same, and the next person to read your code
(probably you) will thank you.

**Keep each callback short.** If your app doesn't get back to `acid_poll_event`
within 2 seconds (1 second for a cart), it's ended with `stopped responding`.
See [§7.10](07-system-apis.md#710-limits).

**Write to `v3/fsroot/Home`, and expect the write to fail sometimes.**
`acid_fs_write` returns `nil, message` rather than raising an error, so check
the first value.

## 8.3 Common mistakes

### Nothing appears

- There's no `.app.toml`, or it's missing `name`, `w` or `h`. The app is then
  quietly never registered. Always check the manifest first.
- You added a *new* app but didn't restart the simulator. Existing apps reload
  from disk every time you launch them, but the list of apps is only built once,
  at boot.
- You're not running from the top folder of the repository, so `v3/apps/...`
  can't be found.
- You forgot `MyApp:new():start()` on the last line. The class was defined, but
  nothing ran.
- There's a Lua error before `start`, so the app ends before it opens a window.
  The message is in the terminal you launched from, as
  `Acid OS v3: <path>: <message>`.

### The window is there but empty

- `redraw` is never called. `AcidApp` calls it once at startup, but `AcidGame`
  **never** does. A game has to paint from `on_tick`.
- You drew before calling `acid_clear_user_area`, which then wiped it.
- You drew at `y < 16`, underneath the title bar.

### Content is clipped or missing at an edge

- `WINDOW_W`/`WINDOW_H` don't match the manifest.
- Every drawing call is **cut off at the edge of your own window**, and
  `acid_draw_text` cuts off letter by letter. But nothing stops you drawing over
  your *own* border or your other drawing. Shorten long strings with
  `s:sub(1, n)`.

### The border vanishes

`acid_draw_window_border` must be the **last** call in `redraw`. Otherwise your
content gets drawn right over the border's pixels.

### A note never stops

You didn't call `acid_stop_note`, and nothing else will do it for you. See
[§6.4](06-games.md#64-the-sound-effect-lifecycle), and check that
`acid_active_voice_count` goes back to zero.

### An arpeggio loops forever

Same cause. An arpeggio keeps cycling until you stop the voice. If you only
wanted it to play once through, stop it after `rate_ms * count` milliseconds.

### An action fires many times from one tap

You're not ignoring repeats while the finger is held down (this is called
"debouncing"). A held press is reported again on every poll. See
[§3.5](03-app-lifecycle.md#35-touch-debouncing).

### A hit test misfires during a drag

A drag belongs to the window where the press started. So `on_touch` keeps
getting events after the finger leaves your window, with `x` and `y` negative
or past your width. Clamp them to your window before you check what was hit.
See
[§3.1](03-app-lifecycle.md#on_touchx-y-pressed).

### A game keeps repainting while covered

You're missing a `focused()` check around the drawing. It can't damage another
window, but it makes the system repaint the screen for nothing. See
[§6.3](06-games.md#63-focus-and-the-z-order-trap).

### Colours come out black, or wrong

You've used RGB565 colour values (the compact 16-bit format). Use full 24-bit
`0xRRGGBB` colours instead.

### Off by one in a list

Lua tables start at **1**, but the app registry, the window table and the task
table all start at **0**. For example, `acid_launcher_name(0)` is the first app.

- A loop over a Lua table is `for i = 1, #t`.
- A loop over the registry is `for i = 0, acid_launcher_count() - 1`.

See
[§7.1](07-system-apis.md#71-launching-other-apps).

### A fraction is cut, not rounded

In Lua 5.4, `/` always gives a decimal number: `7 / 2` is `3.5`. The drawing
calls accept decimals, but **chop off** the fraction (towards zero). So
`acid_fill_rect(0, 0, 3.9, 3, c)` draws a rectangle 3 wide, and a bar that
should be 99.9% full is drawn 99%.

To round properly, use `math.floor(x + 0.5)`. When you want whole-number
division, use `//`.

### `bad argument #3: error converting Lua nil to i32`

The full message is
`bad argument #3: error converting Lua nil to i32 (expected number or string coercible to number)`.
A drawing or sound call got `nil` where it needs a number. Usually that's a
field you forgot to set in `on_create`, or a misspelt name.

If you pass some other non-number, the message says what it was instead
(`Lua table`, `Lua boolean` or `Lua string` in place of `Lua nil`).

The `acid_*` calls **raise an error** when an argument is the wrong type, and an
error you don't catch ends the app. A number too big for 32 bits raises an error
too: `... error converting Lua integer to i32 (out of range)`.

### A `multi` app's two windows disagree

They're two separate Lua VMs that share nothing. That's on purpose. Anything
that must hold across the whole system has to be enforced by the kernel.

### A write says `read only`

The app is cart-level and the path is outside `v3/fsroot/Home/`. See
[§7.11](07-system-apis.md#711-what-a-cart-is-refused).

If a write says `bad path` instead, the path is badly formed, or it points into
a folder that doesn't exist.

### An app ends with no window and a log line

Look at the terminal you started the simulator from. That's where Lua errors,
and the `out of memory` and `stopped responding` lines, are printed.

## 8.4 Recipes

### Centring text

Each character is 6×8 pixels, so a string is `#text * 6` pixels wide.

```lua app
-- w: 200
-- h: 100
local CentreApp = AcidApp:extend("CentreApp")

local WINDOW_W = 200

local function draw_centred(text, y, fg, bg)
  local x = (WINDOW_W - #text * 6) // 2
  acid_draw_text(text, x, y, fg, bg)
end

function CentreApp:redraw()
  acid_clear_user_area()
  acid_draw_window_frame(self:window_title())
  draw_centred("HELLO", 30, 0x00FF66, 0x050607)
  draw_centred("A LONGER LINE", 46, 0xD4E6DB, 0x050607)
  acid_draw_window_border()
end

CentreApp:new():start()
```

### A scrolling list

```lua app
-- w: 200
-- h: 150
local ListApp = AcidApp:extend("ListApp")

local WINDOW_W, WINDOW_H = 200, 150
local TITLE_BAR_H = 16
local ROW_H = 12
local VISIBLE = (WINDOW_H - TITLE_BAR_H - 4) // ROW_H

function ListApp:on_create()
  self.items = {}
  for i = 1, 40 do self.items[i] = "Item number " .. i end
  self.scroll = 0          -- how many items are above the top row
  self.selected = 0        -- zero-based index into items
end

function ListApp:scroll_to(index)
  self.selected = index
  if index < self.scroll then self.scroll = index end
  if index >= self.scroll + VISIBLE then self.scroll = index - VISIBLE + 1 end
  if self.scroll < 0 then self.scroll = 0 end
end

function ListApp:on_key(code, pressed)
  if not pressed then return end
  if code == AcidKeys.DOWN and self.selected < #self.items - 1 then
    self:scroll_to(self.selected + 1)
  elseif code == AcidKeys.UP and self.selected > 0 then
    self:scroll_to(self.selected - 1)
  else
    return
  end
  self:redraw()
end

function ListApp:redraw()
  acid_clear_user_area()
  acid_draw_window_frame(self:window_title())
  for i = 0, VISIBLE - 1 do
    local at = self.scroll + i                -- zero-based item index
    if at >= #self.items then break end
    local y = TITLE_BAR_H + 2 + i * ROW_H
    local on = (at == self.selected)
    acid_fill_rect(1, y, WINDOW_W - 2, ROW_H, on and 0x00FF66 or 0x050607)
    acid_draw_text(self.items[at + 1]:sub(1, 30), 4, y + 2,
                   on and 0x050607 or 0xD4E6DB, on and 0x00FF66 or 0x050607)
  end
  acid_draw_window_border()
end

ListApp:new():start()
```

### A progress bar

```lua snippet
local function draw_bar(x, y, w, h, fraction, fg, bg)
  acid_fill_rect(x, y, w, h, bg)
  local filled = math.floor(w * fraction)
  if filled < 0 then filled = 0 end
  if filled > w then filled = w end
  acid_fill_rect(x, y, filled, h, fg)
end
```

`math.floor` makes the rounding clear. A plain `w * fraction` would also work,
but then the drawing call chops off the fraction for you, and you don't get to
choose how it rounds.

### A repainting-only-when-changed loop

```lua snippet
function MyApp:state_signature()
  return table.concat({ self.score, self.lives, self.selected, self.mode }, "|")
end

function MyApp:redraw_if_changed()
  local sig = self:state_signature()
  if sig == self.drawn_sig then return end
  self.drawn_sig = sig
  self:redraw()
end
```

This only redraws when something visible has changed. In Lua, `==` on two
tables checks whether they're the same table, not whether they hold the same
values. So build a string and compare that instead. The desktop does the same
with its windows' names, which one has focus, and the clock text.

### A button that responds to press and release

```lua app
-- w: 160
-- h: 100
local ButtonApp = AcidApp:extend("ButtonApp")

local BTN = { x = 30, y = 40, w = 100, h = 30 }

function ButtonApp:on_create()
  self.pressed = false
  self.clicks = 0
end

local function in_button(x, y)
  return x >= BTN.x and x < BTN.x + BTN.w and y >= BTN.y and y < BTN.y + BTN.h
end

function ButtonApp:on_touch(x, y, pressed)
  local hit = in_button(x, y)
  if pressed then
    if self.pressed then return end        -- the same hold: ignore
    self.pressed = hit
  else
    -- Released inside the button we pressed: that is a click.
    if self.pressed and hit then self.clicks = self.clicks + 1 end
    self.pressed = false
  end
  self:redraw()
end

function ButtonApp:redraw()
  acid_clear_user_area()
  acid_draw_window_frame(self:window_title())
  local colour = self.pressed and 0x00FF66 or 0x0B1712
  acid_fill_rect(BTN.x, BTN.y, BTN.w, BTN.h, colour)
  local fg = self.pressed and 0x050607 or 0xD4E6DB
  acid_draw_text("CLICKS " .. self.clicks, BTN.x + 16, BTN.y + 11, fg, colour)
  acid_draw_window_border()
end

ButtonApp:new():start()
```

### A confirm-before-quit

```lua app
-- w: 200
-- h: 90
local QuitApp = AcidApp:extend("QuitApp")

function QuitApp:on_create()
  self.confirming = false
end

function QuitApp:on_key(code, pressed)
  if not pressed then return end
  if self.confirming then
    if code == string.byte("y") then self:quit() end
    self.confirming = false
    self:redraw()
  elseif code == AcidKeys.ESCAPE then
    self.confirming = true
    self:redraw()
  end
end

function QuitApp:redraw()
  acid_clear_user_area()
  acid_draw_window_frame(self:window_title())
  local line = self.confirming and "QUIT? Y/N" or "PRESS ESC TO QUIT"
  acid_draw_text(line, 10, 40, 0xD4E6DB, 0x050607)
  acid_draw_window_border()
end

QuitApp:new():start()
```

Remember that `quit` does nothing in an `AcidGame`. A game's loop only ends
when its window is closed.

### Colour-cycling a whole window

```lua app
-- w: 180
-- h: 120
local RainbowApp = AcidApp:extend("RainbowApp")

local WINDOW_H = 120
local TITLE_BAR_H = 16

function RainbowApp:on_create()
  self.step = 0
end

function RainbowApp:poll_timeout_ms()
  return 40
end

function RainbowApp:on_idle()
  if not self:focused() then return end
  self.step = self.step + 3
  self:redraw()
end

function RainbowApp:redraw()
  acid_clear_user_area()
  acid_draw_window_frame(self:window_title())
  local y, row = TITLE_BAR_H, 0
  while y < WINDOW_H do
    local h = (y + 8 > WINDOW_H) and (WINDOW_H - y) or 8
    acid_fill_rect(0, y, 180, h, AcidPalette.hue(self.step + row * 12))
    y = y + 8
    row = row + 1
  end
  acid_draw_window_border()
end

RainbowApp:new():start()
```

### Playing a short melody

A melody is a table of `{ona, ticks}` steps, played one tick at a time.

Notice that the voice is stopped in two places: at the end of the tune, and in
`on_destroy`, in case the window is closed halfway through.

```lua app
-- w: 180
-- h: 80
local MelodyApp = AcidApp:extend("MelodyApp")

local LEAD_VOICE = 5                  -- voice 5: this app's lead
local MELODY = { {40, 2}, {44, 2}, {47, 2}, {52, 4}, {47, 2}, {52, 6} }  -- {ona, ticks}

function MelodyApp:poll_timeout_ms()
  return 100                          -- one tick
end

function MelodyApp:on_create()
  acid_configure_voice(LEAD_VOICE, 0, 5, 80, 60, 150)
  self.index = nil
end

function MelodyApp:start_melody()
  self.index = 1
  self.ticks = 0
end

function MelodyApp:tick_melody()
  if not self.index then return end
  if self.ticks <= 0 then
    if self.index > #MELODY then
      acid_stop_note(LEAD_VOICE)
      self.index = nil
      return
    end
    local ona, ticks = MELODY[self.index][1], MELODY[self.index][2]
    acid_play_note(LEAD_VOICE, ona, 55)
    self.ticks = ticks
    self.index = self.index + 1
  end
  self.ticks = self.ticks - 1
end

function MelodyApp:on_touch(x, y, pressed)
  if pressed and not self.index then self:start_melody() end
end

function MelodyApp:on_idle()
  self:tick_melody()
end

function MelodyApp:on_destroy()
  acid_stop_note(LEAD_VOICE)
end

MelodyApp:new():start()
```

### A stopwatch

Time anything you measure with `acid_now_ms`. Don't count `on_idle` calls:
they come whenever the poll times out, not on a fixed schedule.

```lua app
-- w: 180
-- h: 80
local StopwatchApp = AcidApp:extend("StopwatchApp")

function StopwatchApp:poll_timeout_ms()
  return 50
end

function StopwatchApp:on_create()
  self.started_at = acid_now_ms()
  self.down = false
end

function StopwatchApp:on_touch(x, y, pressed)
  if not pressed then self.down = false; return end
  if self.down then return end
  self.down = true
  self.started_at = acid_now_ms()                -- a tap restarts it
end

function StopwatchApp:on_idle()
  self:redraw()
end

function StopwatchApp:redraw()
  acid_clear_user_area()
  acid_draw_window_frame(self:window_title())
  local elapsed = acid_now_ms() - self.started_at
  local text = string.format("%d.%02d S", elapsed // 1000, (elapsed % 1000) // 10)
  acid_draw_text(text, 10, 36, 0x00FF66, 0x050607)
  acid_draw_text("TAP TO RESTART", 10, 54, 0x9DAAA3, 0x050607)
  acid_draw_window_border()
end

StopwatchApp:new():start()
```

### Saving a setting to a file

The file system calls return `nil, message` and never raise an error. So a
sturdy app:

- checks the first value
- falls back to a default when the file is missing
- shows (or ignores) a failed save, instead of crashing

This complete app keeps a counter in Home. It works whether or not the file or
the folder is there:

```lua app
-- w: 200
-- h: 100
local SettingApp = AcidApp:extend("SettingApp")

local FILE = "v3/fsroot/Home/cookbook_setting.txt"

local function load_setting()
  local text, err = acid_fs_read(FILE)
  if not text then return 0, err end              -- "not found" the first time
  return tonumber(text) or 0, nil                 -- a damaged file reads as the default
end

function SettingApp:on_create()
  self.value, self.note = load_setting()
  self.down = false
end

function SettingApp:save()
  local ok, err = acid_fs_write(FILE, tostring(self.value))
  self.note = ok and "SAVED" or ("NOT SAVED: " .. tostring(err))
end

function SettingApp:on_touch(x, y, pressed)
  if not pressed then self.down = false; return end
  if self.down then return end
  self.down = true
  self.value = self.value + 1
  self:save()
  self:redraw()
end

function SettingApp:redraw()
  acid_clear_user_area()
  acid_draw_window_frame(self:window_title())
  acid_draw_text("VALUE " .. self.value, 10, 34, 0x00FF66, 0x050607)
  acid_draw_text(("TAP TO ADD ONE"):sub(1, 30), 10, 50, 0xD4E6DB, 0x050607)
  if self.note then acid_draw_text(self.note:sub(1, 30), 10, 66, 0x9DAAA3, 0x050607) end
  acid_draw_window_border()
end

SettingApp:new():start()
```

### Listing a folder, safely

```lua snippet
local function list_home()
  local names, err = acid_fs_list("v3/fsroot/Home")
  if not names then return {}, err end            -- keep going with an empty list
  return names, nil
end
```

---

[← System APIs](07-system-apis.md) · [Contents](README.md) · [Next: API reference →](09-api-reference.md)
