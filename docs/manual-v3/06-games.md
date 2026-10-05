# 6. Games

[← Sound](05-sound.md) · [Contents](README.md) · [Next: System APIs →](07-system-apis.md)

An `AcidApp` sleeps until something happens, such as a key press or a touch,
and then wakes up to deal with it. That's ideal for a text editor, but no good
for anything with a ball in it.

`AcidGame` swaps that for a loop that runs on a **fixed tick**: it wakes up
regularly, many times a second, whether anything happened or not. It lives in
`v3/apps/lib/acid_game.lua` and is always loaded.

## 6.1 `AcidGame`

`AcidGame` is a subclass of `AcidApp`, so you extend it in the same way:

```lua snippet
local Snake = AcidGame:extend("Snake")
Snake.TICK_MS = 50          -- 20 ticks per second

function Snake:on_create() end
function Snake:on_tick() end      -- the new one
function Snake:on_touch(x, y, pressed) end
function Snake:on_key(code, pressed) end
function Snake:on_destroy() end

Snake:new():start()
```

`on_tick` runs every `TICK_MS` milliseconds, whether anything happened or not.
Everything else works as it does in `AcidApp`, with three differences:

1. **`on_idle` is never called.** `on_tick` takes its place.
2. **`redraw` is never called for you**, not at startup and not on `"moved"`.
   Your game is expected to repaint its whole scene from `on_tick`, inside a
   frame so it never flickers ([§6.7](#67-held-keys-and-frames)). The default
   `AcidGame` draws nothing at all, so a game that never draws won't even have a
   window border.
3. **`quit` does nothing.** `AcidGame` runs its own loop and ignores `AcidApp`'s
   `self.running` on purpose. The only thing that ends a game is the close
   event: the user clicking the close dot, or the kernel ending the app. If you
   want your game to close itself, you'll need to write your own `start`, based
   on the loop in the next section.

`TICK_MS` is a **field on your class**. It is read once, through
`self.TICK_MS`, after `on_create` has run, and the tick length then stays the
same for the life of the game. (Compare `poll_timeout_ms` in `AcidApp`, which is
a method because it can change while the app runs.)

## 6.2 The tick loop

This is the actual loop from `acid_game.lua`:

```lua snippet
function AcidGame:start()
  self:on_create()
  local running = true
  local tick = self.TICK_MS
  local next_tick_at = acid_now_ms() + tick
  while running do
    local remaining = next_tick_at - acid_now_ms()
    if remaining < 0 then remaining = 0 end
    local kind, a, b, c = acid_poll_event(remaining)
    if kind == "close" then
      running = false
    elseif kind == "moved" then
      acid_notify_redraw_done()
    elseif kind == "key" then
      self:on_key(a, b)
    elseif kind == "touch" then
      self:on_touch(a, b, c)
    end
    if running and acid_now_ms() >= next_tick_at then
      self:on_tick()
      next_tick_at = next_tick_at + tick
      if next_tick_at < acid_now_ms() then
        next_tick_at = acid_now_ms() + tick
      end
    end
  end
  self:on_destroy()
end
```

`acid_now_ms()` gives you the clock in milliseconds. A few things worth knowing
about this loop:

**It waits for events only until the next tick is due.** So events are still
handled straight away (a touch halfway through a tick is passed to you
immediately), and the ticks stay on schedule.

**If it falls behind, it skips ahead rather than catching up.** If the loop gets
more than a whole tick behind, it sets the next tick to *now plus one tick*.
Running all the missed ticks in a burst would make the game look like it briefly
sped up, which is worse than a dropped frame.

**`"moved"` is acknowledged, but nothing is repainted.** The game will redraw its
whole scene on the next tick anyway. Acid OS doesn't currently send `"moved"` at
all, and the acknowledgement does nothing
([§3.2](03-app-lifecycle.md#32-the-event-loop)). The branch is there so games
keep working if that ever changes.

### Picking `TICK_MS`

| `TICK_MS` | Rate | Suits |
|---|---|---|
| 33 | 30 Hz | Fast action, smooth motion |
| 50 | 20 Hz | Arcade action; what all of Acid OS's games use |
| 100 | 10 Hz | Puzzle games, turn-based movement |
| 500 | 2 Hz | Tetris-style gravity (or use a counter on a faster tick) |

If different things in your game move at different speeds, use a fast tick and
count frames, rather than a slow tick:

```lua snippet
MyGame.TICK_MS = 50

function MyGame:on_tick()
  self.frame = self.frame + 1
  self:move_player()                                    -- every tick
  if self.frame % 10 == 0 then self:drop_piece() end    -- every 500ms
  if self.frame % 40 == 0 then self:spawn_enemy() end   -- every 2s
end
```

## 6.3 Focus and the z-order trap

A game repaints itself **every tick**, without waiting to be asked. That's
safe, because **a game can't draw over another window.** Each window draws onto
its own canvas, and the kernel stacks the canvases in order (the "z-order") to
build the screen. A game that is covered just paints pixels nobody can see.

Even so, a game should stop drawing when it isn't focused. There are three
reasons:

- **Cost.** Every drawing call tells the kernel the screen has changed. A covered
  game that repaints 20 times a second makes the kernel rebuild the whole screen
  20 times a second, for nothing.
- **Focused and visible aren't the same thing.** A focused window is always on
  top, but a window on top isn't always focused: a newly opened window is in
  front before it gets focus
  ([§3.4](03-app-lifecycle.md#34-focus-titles-and-quitting)). And if you click a
  window beside your game, the game loses focus while still fully visible. A
  game that only draws while focused will freeze on screen at that point, though
  the game itself keeps running underneath. The built-in games accept that
  trade-off. If you'd rather keep animating while unfocused, draw regardless and
  accept the cost.
- **Good habits.** Repainting fully when focus comes back (shown below) is the
  right habit for apps whose state really does depend on what's on screen.

So every game's tick ends with a focus check:

```lua snippet
function MyGame:on_tick()
  self:update_world()          -- simulation always runs
  self:tick_sfx()              -- sound bookkeeping always runs
  if self:focused() then       -- drawing only when we are on top
    self:draw()
  end
end
```

Notice what is *inside* the check and what isn't. The game world keeps going
while you're covered; only the drawing stops.

It's a good idea to force a full repaint when focus comes *back*. Your canvas
keeps its last frame while you're not drawing, so anything you were tracking for
partial redraws is still correct. But that frame is out of date, and one full
repaint brings it straight up to date:

```lua snippet
function MyGame:on_tick()
  self:update_world()
  self:tick_sfx()
  local is_focused = self:focused()
  if is_focused and not self.was_focused then
    self.needs_frame = true               -- just regained focus
  end
  self.was_focused = is_focused
  if is_focused then self:draw() end
end
```

## 6.4 The sound-effect lifecycle

**Only `acid_stop_note` stops a note.** A short envelope doesn't, a finished
arpeggio doesn't, and the end of a tick doesn't. A note keeps holding, and an
arpeggio keeps repeating, until you stop it.

So with sound effects, the job is keeping track of what's playing. All of Acid
OS's games do it the same way. It's worth learning the pattern, because getting
it wrong causes the two most annoying bugs you can ship.

### The pattern

Record every sound you start, count it down each tick, and stop it in exactly
one place.

```lua snippet
-- 1. Starting a sound registers it with a tick countdown.
function MyGame:trigger_sfx(voice, notes, count, rate_ms, volume, ticks)
  acid_play_note(voice, notes[1], volume)
  acid_trigger_arp(voice, notes[1], notes[2], notes[3], notes[4], count, rate_ms)
  self.sfx[#self.sfx + 1] = { voice = voice, ticks = ticks }
end

-- 2. tick_sfx is the ONLY place a note is ever stopped.
function MyGame:tick_sfx()
  for i = #self.sfx, 1, -1 do
    local s = self.sfx[i]
    s.ticks = s.ticks - 1
    if s.ticks <= 0 then
      acid_stop_note(s.voice)
      table.remove(self.sfx, i)
    end
  end
end

-- 3. Called from every tick, unconditionally: never behind `focused()`.
function MyGame:on_tick()
  if not self.game_over then self:update_world() end
  self:tick_sfx()
  if self:focused() then self:draw() end
end
```

When you remove items from a list while looping over it, loop **backwards**, as
above.

### Rule 1: a reset must stop voices before it clears the bookkeeping

```lua snippet
function MyGame:reset_game()
  self:stop_all_sfx()        -- first
  self.sfx = {}
  self.score = 0
  -- ...
end

function MyGame:stop_all_sfx()
  if not self.sfx then return end   -- safe before the first reset_game
  for _, s in ipairs(self.sfx) do acid_stop_note(s.voice) end
  self.sfx = {}
end
```

The player usually taps to restart while the game-over sound is still playing.
`tick_sfx` is the only thing that ever stops a note. So if you clear `self.sfx`
without stopping its voices first, **nothing will ever stop them**. The
arpeggio repeats and the note holds for as long as the app is open.

Both of the built-in arcade games had exactly this bug. A short envelope only
made it rarer, not impossible.

### Rule 2: an arpeggio's gate must not outlast one pass

A four-note arpeggio at 110 ms takes 440 ms for one pass, then starts over. If
you want it to play once, work out the tick count from the arpeggio's own
numbers rather than guessing:

```lua snippet
local OVER_NOTES = { 30, 27, 23, 18 }
local OVER_RATE_MS = 110
local OVER_COUNT = 4
local OVER_TICKS = (OVER_RATE_MS * OVER_COUNT + TICK_MS - 1) // TICK_MS   -- round up

self:trigger_sfx(OVER_VOICE, OVER_NOTES, OVER_COUNT, OVER_RATE_MS, 50, OVER_TICKS)
```

Work it out like this and the sound stays right when you tweak the effect.
Hard-code the number and it goes wrong the first time you change `rate_ms`.

### Rule 3: `tick_sfx` runs even when nothing else does

Keep it *outside* every `if`. If your game stops calling `tick_sfx` while it's
paused, on the game-over screen or unfocused, you've just created a stuck note.

```lua snippet
function MyGame:on_tick()
  if not self.game_over then
    self:update_ball()       -- guarded
  end
  self:tick_sfx()            -- never guarded
  if self:focused() then self:draw() end
end
```

### Rule 4: `on_destroy` stops everything

```lua snippet
function MyGame:on_destroy()
  self:stop_all_sfx()
end
```

The kernel releases your voices when your app exits anyway, so this isn't about
leaks. It's about getting a clean fade instead of an abrupt cut, and about
keeping good habits while the app is running.

## 6.5 A minimal game

```lua app
-- w: 200
-- h: 160
local DodgeApp = AcidGame:extend("DodgeApp")

local WINDOW_W = 200
local WINDOW_H = 160
local TITLE_BAR_H = 16
DodgeApp.TICK_MS = 50
local TICK_MS = DodgeApp.TICK_MS

local BG = 0x050607
local TEXT = 0xD4E6DB

local PLAYER_W = 24
local PLAYER_H = 6
local PLAYER_Y = WINDOW_H - 16
local ROCK_R = 3
local HIT_VOICE = 7
local HIT_RATE_MS = 30
local HIT_COUNT = 3
local HIT_TICKS = (HIT_RATE_MS * HIT_COUNT + TICK_MS - 1) // TICK_MS

function DodgeApp:on_create()
  acid_configure_filter(180, 3, 1)
  acid_configure_osc(HIT_VOICE, AcidWaveform.SAW, 50)
  acid_configure_voice(HIT_VOICE, 1, 2, 40, 50, 60)
  self:reset_game()
end

function DodgeApp:reset_game()
  self:stop_all_sfx()
  self.sfx = {}
  self.player_x = (WINDOW_W - PLAYER_W) // 2
  self.rocks = {}
  self.score = 0
  self.over = false
  self.frame = 0
  self.needs_full = true
  self.was_focused = false
  self.touch_down = false
end

function DodgeApp:stop_all_sfx()
  if not self.sfx then return end
  for _, s in ipairs(self.sfx) do acid_stop_note(s.voice) end
  self.sfx = {}
end

function DodgeApp:tick_sfx()
  for i = #self.sfx, 1, -1 do
    local s = self.sfx[i]
    s.ticks = s.ticks - 1
    if s.ticks <= 0 then
      acid_stop_note(s.voice)
      table.remove(self.sfx, i)
    end
  end
end

function DodgeApp:hit_sound()
  acid_play_note(HIT_VOICE, 55, 60)
  acid_trigger_arp(HIT_VOICE, 55, 48, 41, 1, HIT_COUNT, HIT_RATE_MS)
  self.sfx[#self.sfx + 1] = { voice = HIT_VOICE, ticks = HIT_TICKS }
end

function DodgeApp:on_touch(x, y, pressed)
  if not pressed then
    self.touch_down = false
    return
  end
  if self.over then
    -- A held press repeats; restart once per press, not once per tick.
    if not self.touch_down then self:reset_game() end
    self.touch_down = true
    return
  end
  self.touch_down = true
  self.player_x = math.max(0, math.min(WINDOW_W - PLAYER_W, x - PLAYER_W // 2))
end

function DodgeApp:on_tick()
  if not self.over then
    self.frame = self.frame + 1
    if self.frame % 8 == 0 then
      self.rocks[#self.rocks + 1] = { x = 8 + math.random(0, WINDOW_W - 17), y = TITLE_BAR_H }
    end
    for i = #self.rocks, 1, -1 do
      local r = self.rocks[i]
      r.y = r.y + 4
      if r.y >= PLAYER_Y and r.x > self.player_x and r.x < self.player_x + PLAYER_W then
        self:hit_sound()
        self.over = true
      end
      if r.y > WINDOW_H then
        table.remove(self.rocks, i)
        self.score = self.score + 1
      end
    end
  end

  self:tick_sfx()

  local is_focused = self:focused()
  if is_focused and not self.was_focused then self.needs_full = true end
  self.was_focused = is_focused
  if is_focused then self:draw() end
end

function DodgeApp:on_destroy()
  self:stop_all_sfx()
end

function DodgeApp:draw()
  if self.needs_full then
    acid_draw_window_frame(self:window_title())
    self.needs_full = false
  end
  acid_fill_rect(0, TITLE_BAR_H, WINDOW_W, WINDOW_H - TITLE_BAR_H, BG)
  for i, r in ipairs(self.rocks) do
    acid_fill_circle(r.x, r.y, ROCK_R, AcidPalette.hue((i - 1) * 30 + self.score * 7))
  end
  acid_fill_rect(self.player_x, PLAYER_Y, PLAYER_W, PLAYER_H, 0x00FF66)
  acid_draw_text("SCORE " .. self.score, 6, TITLE_BAR_H + 4, TEXT, BG)
  if self.over then
    acid_draw_text("TAP TO RESTART", 46, WINDOW_H // 2, TEXT, BG)
  end
  acid_draw_window_border()
end

DodgeApp:new():start()
```

Things to notice:

- `tick_sfx` is never inside the focus check.
- `reset_game` stops the voices before it clears the list.
- The hit sound's tick count is worked out from its arpeggio.
- Both touch paths use `touch_down`, so holding a tap restarts the game once,
  not every tick.
- `on_touch` keeps the paddle inside the window, because `x` can be negative or
  past the window edge ([§3.1](03-app-lifecycle.md#31-the-callbacks)).

With `v3/apps/dodge.app.toml`:

```toml
name = Dodge
w = 200
h = 160
desc = Dodge the falling rocks
menu = false
```

`menu = false` keeps a game out of the Menu, as all the built-in games do. You
start them from the Terminal instead (`run tetris`), or by clicking their
`.app.toml` in the File Manager. Leave the line out if you want yours in the
Menu.

## 6.6 A worked example: Acid Spin

`v3/apps/acid_spin.lua` is a complete `AcidGame` that draws 3D meshes
([§4.8](04-graphics.md#48-lines-triangles-and-3d)). It shows a tick loop that
only redraws while focused, keyboard and tap controls with a press-once guard,
afterimages built by remembering previous angles, a window that re-centres
when it is resized, and a custom mesh built with `acid_mesh_new`. Like the
games, it isn't in the Menu: open File Manager, go into `App` and click its
`.app.toml` file.

## 6.7 Held keys and frames

Two habits every built-in action game follows.

**Held keys.** A key sends a press when it goes down and a release when it
comes up ([§3.1](03-app-lifecycle.md#31-the-callbacks)), with no auto-repeat
in between. A game that moves while a key is down keeps a table of what's held
and reads it on every tick:

```lua snippet
function MyGame:on_key(code, pressed)
  self.held[code] = pressed or nil
end

function MyGame:on_tick()
  if not self:focused() then
    -- Keys go to the focused window, so a key let go elsewhere never
    -- reaches you: forget everything held.
    self.held = {}
    return
  end
  local dx = (self.held[AcidKeys.RIGHT] and 1 or 0) - (self.held[AcidKeys.LEFT] and 1 or 0)
  self.x = self.x + dx * 2
  self:redraw()
end
```

Act on the *press* for one-off things (fire one shot, rotate a Tetris piece,
start the game), and on the *held* table for anything continuous (moving,
turning, thrusting, firing again while the key stays down).

**Frames.** A game that clears and repaints its whole window every tick wraps
the repaint in `acid_begin_frame` / `acid_end_frame`
([§4.4](04-graphics.md#44-partial-redraws-and-flicker)), so the screen never
shows it half-drawn:

```lua snippet
function MyGame:redraw()
  acid_begin_frame()
  acid_clear_user_area()
  acid_draw_window_frame(self:window_title())
  self:draw_scene()
  acid_draw_window_border()
  acid_end_frame()
end
```

For longer examples, read `acidstorm.lua` (eight-way movement and firing from
held keys), `acid_invaders.lua` (held movement, fire that repeats while held)
and `acid_rocks.lua` (held turning and thrust, vector lines that wrap).

---

[← Sound](05-sound.md) · [Contents](README.md) · [Next: System APIs →](07-system-apis.md)
