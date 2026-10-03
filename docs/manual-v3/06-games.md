# 6. Games

[← Sound](05-sound.md) · [Contents](README.md) · [Next: System APIs →](07-system-apis.md)

`AcidApp` is event-driven: it sits blocked on its queue and wakes when something
happens. That is right for a text editor and wrong for anything with a ball in
it. `AcidGame` (`v3/apps/lib/acid_game.lua`, always loaded) replaces the event
loop with a **fixed-tick** one.

## 6.1 `AcidGame`

`AcidGame` is itself a subclass of `AcidApp`, so a game extends it the same way:

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

`on_tick` fires every `TICK_MS` milliseconds regardless of whether anything
happened. Everything else works as in `AcidApp`, with three differences:

1. **`on_idle` is never called.** `on_tick` replaces it.
2. **`redraw` is never called automatically.** Not at startup, not on `"moved"`.
   A game repaints its whole scene from `on_tick`; that is the contract. (The
   default `AcidGame` paints nothing at all, so a game that never draws has no
   window border.)
3. **`quit` does nothing.** `AcidGame` runs its own loop with a local flag and
   deliberately ignores `AcidApp`'s `self.running`. The only thing that ends a
   game is the close event (the close dot, or the kernel ending the app). If you
   want a self-close you have to write your own `start`, as in the next section.

`TICK_MS` is a **field on your class**, read through `self.TICK_MS` once, after
`on_create` has run. Unlike `poll_timeout_ms`, which is a method because it
changes at runtime, the tick length is fixed for the life of the game.

## 6.2 The tick loop

This is the real loop, from `acid_game.lua`:

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

`acid_now_ms()` is the platform clock in milliseconds.
Two details worth understanding:

**The poll timeout is whatever is left of this tick.** Events are still handled
promptly (a touch in the middle of a tick is dispatched immediately) but the
tick itself stays on schedule.

**Missed ticks resync rather than burst.** If something put the loop more than a
whole tick behind, `next_tick_at` is reset to *now plus one tick* instead of
catching up. A catch-up burst would look like the game briefly speeding up,
which is worse than a dropped frame.

**`"moved"` is acknowledged but not repainted.** A game redraws its whole scene
next tick anyway. In v3 the acknowledgement is a no-op and the router does not
send `"moved"` at all today ([§3.2](03-app-lifecycle.md#32-the-event-loop)), but
the branch is kept so a game keeps working if it ever does.

### Picking `TICK_MS`

| `TICK_MS` | Rate | Suits |
|---|---|---|
| 33 | 30 Hz | Fast action, smooth motion |
| 50 | 20 Hz | The default across this OS's games: arcade action |
| 100 | 10 Hz | Puzzle games, turn-based movement |
| 500 | 2 Hz | Tetris-style gravity (or use a counter on a faster tick) |

Prefer a fast tick with a counter over a slow tick, when different things in
your game move at different rates:

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

A game repaints itself **directly, every tick**, rather than waiting to be
asked. That is safe: **a game cannot corrupt another window.** Every window
draws into its own canvas and the kernel composites the canvases in z-order, so
a covered game paints only into pixels nobody can see. Even so, a game should
stop drawing while it is not focused, for three reasons:

- **Cost.** Every drawing call marks the screen dirty, so a covered game that
  repaints 20 times a second forces a full recomposite of the screen 20 times a
  second for nothing.
- **Honesty about what "covered" means.** Focus implies topmost, not
  the reverse: a newly opened window is in front but not yet focused
  ([§3.4](03-app-lifecycle.md#34-focus-titles-and-quitting)). A
  game that draws only while focused stops moving on screen the moment you click
  a window beside it, even though it is fully visible, while its simulation
  keeps running. That is the trade the shipped games make; a game that wants to
  keep animating while merely unfocused can draw regardless and accept the cost.
- **Habit.** A full repaint when focus comes back, below, keeps your habits
  right for apps that do share state with what is on screen.

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

Note what is *inside* the guard and what is not. The world keeps turning while
you are covered; only the painting stops.

It is a good habit to force a full repaint when focus comes *back*. A canvas you
stopped drawing to keeps its last frame, so partial-redraw state stays valid,
but the window shows a picture that is behind the world; one full repaint lets
it catch up at once:

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

**Nothing stops a note but an explicit `acid_stop_note`.** Not a short envelope,
not a finished arpeggio, not the end of a tick. A gated-on voice sustains, and a
triggered arpeggio cycles, until you gate it off.

That makes sound effects a *bookkeeping* problem, and this OS's games all solve
it the same way. Learn the pattern; it is the source of the two most annoying
bugs you can ship.

### The pattern

Register every sound you start, count it down, and stop it in exactly one place.

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

Iterate **backwards** when deleting during a walk, as above.

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

A restart tap arrives while the game-over sting is usually still playing.
`tick_sfx` is the only thing that ever sends a note-off, so clearing `self.sfx`
without first stopping its voices leaves them with **no note-off ever coming**.
The arpeggio cycles forever and the envelope sustains forever, for as long as
the app is open.

Both shipped arcade games had exactly this bug. A short envelope only made it
rarer to hit, not impossible.

### Rule 2: an arpeggio's gate must not outlast one pass

A four-note arp at 110 ms takes 440 ms for one pass and then starts over. If you
meant it as a one-shot run, compute the tick count from the arp itself rather
than guessing:

```lua snippet
local OVER_NOTES = { 30, 27, 23, 18 }
local OVER_RATE_MS = 110
local OVER_COUNT = 4
local OVER_TICKS = (OVER_RATE_MS * OVER_COUNT + TICK_MS - 1) // TICK_MS   -- round up

self:trigger_sfx(OVER_VOICE, OVER_NOTES, OVER_COUNT, OVER_RATE_MS, 50, OVER_TICKS)
```

Derive it and the sound stays correct when you retune the effect. Hard-code it
and it drifts the first time you change `rate_ms`.

### Rule 3: `tick_sfx` runs even when nothing else does

Put it *outside* every guard. A game that stops calling `tick_sfx` while paused,
while game-over, or while unfocused has just invented a stuck note.

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

The kernel releases your voices when your task exits either way, so this is not
about leaks: it is about the difference between a clean fade and an abrupt cut,
and about keeping the habit while the app is still running.

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

Things worth noticing: `tick_sfx` is never behind the focus guard; `reset_game`
stops the voices before it clears the list; the hit sound's tick count is derived
from its arp; and both touch paths are debounced with `touch_down`, so a held
tap restarts the game once instead of every tick. `on_touch` clamps the paddle
because `x` can arrive negative or past the window edge
([§3.1](03-app-lifecycle.md#31-the-callbacks)).

With `v3/apps/dodge.app.toml`:

```toml
name = Dodge
w = 200
h = 160
desc = Dodge the falling rocks
menu = false
```

`menu = false` keeps a game out of the Menu dropdown, the convention every
shipped game follows. They are launched instead from the Terminal (`run tetris`)
or by clicking their `.app.toml` in File Manager. Drop the line if you want
yours in the Menu.

---

[← Sound](05-sound.md) · [Contents](README.md) · [Next: System APIs →](07-system-apis.md)
