-- Acid Snake: steer a snake round a walled board, eat pellets, grow.
-- The body runs through the hue wheel; every fifth pellet a rainbow trip
-- pellet appears for a few seconds, worth five times as much, and eating
-- it sends a colour ripple down the snake. The snake speeds up as it
-- grows.
--
-- Arrows or W/A/S/D turn (one press, one turn; up to two turns queue so
-- a quick double press makes a U-turn). A tap turns towards the tap,
-- seen from the head. P pauses.

AcidSnake = AcidGame:extend("AcidSnake")

local N = AcidSnake

N.TICK_MS = 33
N.TITLE_BAR_H = 16
N.CELL = 8
N.BG_COLOR = 0x050607     -- THEME_BG
N.TEXT_COLOR = 0xD4E6DB   -- THEME_TEXT
N.MUTED_COLOR = 0x9DAAA3  -- THEME_MUTED
N.BOARD_COLOR = 0x0B0F12
N.START_LEN = 4
N.PELLET_POINTS = 10
N.TRIP_POINTS = 50
N.TRIP_EVERY = 5          -- pellets per trip pellet
N.TRIP_TICKS = 180        -- how long a trip pellet stays (about 6 s)
N.RIPPLE_TICKS = 60
N.MAX_TURNS = 2
N.SLOWEST = 6             -- ticks per move at the start
N.FASTEST = 2
N.GROW_PER_SPEEDUP = 8    -- segments per step faster
N.DEAD_LOCK_TICKS = 12    -- a press this soon after dying is ignored

-- Each sfx gate is exactly one pass of its arpeggio (count * rate =
-- ticks * TICK_MS): an arp wraps until note-off.
N.SFX = {
  eat   = { voice = 0, notes = { 60, 67, 1, 1 }, count = 2, rate = 33, volume = 30, ticks = 2 },
  trip  = { voice = 1, notes = { 60, 64, 67, 72 }, count = 4, rate = 33, volume = 35, ticks = 4 },
  death = { voice = 2, notes = { 45, 41, 38, 33 }, count = 4, rate = 66, volume = 40, ticks = 8 },
}
N.FILTER_MODE_LP = 1

local UP, DOWN, LEFT, RIGHT = { 0, -1 }, { 0, 1 }, { -1, 0 }, { 1, 0 }
local TURN_KEYS = {
  [AcidKeys.UP] = UP, [AcidKeys.DOWN] = DOWN, [AcidKeys.LEFT] = LEFT, [AcidKeys.RIGHT] = RIGHT,
  [string.byte("w")] = UP, [string.byte("W")] = UP,
  [string.byte("s")] = DOWN, [string.byte("S")] = DOWN,
  [string.byte("a")] = LEFT, [string.byte("A")] = LEFT,
  [string.byte("d")] = RIGHT, [string.byte("D")] = RIGHT,
}

function AcidSnake:on_create()
  acid_configure_filter(220, 3, N.FILTER_MODE_LP)
  acid_configure_voice(0, 1, 2, 30, 50, 60)
  acid_configure_voice(1, 1, 2, 40, 60, 80)
  acid_configure_voice(2, 1, 8, 150, 35, 250)
  self.sfx = {}
  self.ticks = 0
  self.best = 0
  self.touch_held = false
  self:layout()
  self:to_title()
end

-- The board sits under the title bar and one HUD text line, centred.
function AcidSnake:layout()
  local w, h = acid_window_size()
  local cw, ch = acid_font_size()
  self.w, self.h, self.cw, self.ch = w, h, cw, ch
  self.hud_y = N.TITLE_BAR_H + 2
  local top = self.hud_y + ch + 4
  self.cols = (w - 8) // N.CELL
  self.rows = (h - 4 - top) // N.CELL
  self.bx = (w - self.cols * N.CELL) // 2
  self.by = top
end

function AcidSnake:to_title()
  self:stop_all_sfx()
  self.state = "title"
  self.score = 0
end

function AcidSnake:start_game()
  self:stop_all_sfx()
  self.state = "playing"
  self.paused = false
  self.score = 0
  self.eaten = 0
  self.grow = 0
  self.turns = {}
  self.dir = RIGHT
  self.move_timer = 0
  self.ripple = 0
  self.trip = nil
  local cx, cy = self.cols // 2, self.rows // 2
  self.body = {}
  for i = 0, N.START_LEN - 1 do
    self.body[#self.body + 1] = { x = cx - i, y = cy }
  end
  self:place_pellet()
end

function AcidSnake:occupied(x, y)
  for _, s in ipairs(self.body) do
    if s.x == x and s.y == y then return true end
  end
  if self.pellet and self.pellet.x == x and self.pellet.y == y then return true end
  if self.trip and self.trip.x == x and self.trip.y == y then return true end
  return false
end

-- A random free cell, or nil when the board is full.
function AcidSnake:free_cell()
  local free = {}
  for y = 0, self.rows - 1 do
    for x = 0, self.cols - 1 do
      if not self:occupied(x, y) then free[#free + 1] = { x = x, y = y } end
    end
  end
  if #free == 0 then return nil end
  return free[math.random(1, #free)]
end

function AcidSnake:place_pellet()
  self.pellet = nil
  self.pellet = self:free_cell()
end

function AcidSnake:move_every()
  local v = N.SLOWEST - (#self.body - N.START_LEN) // N.GROW_PER_SPEEDUP
  return math.max(N.FASTEST, v)
end

-- ---- input ----

-- The direction the next queued turn would be checked against.
function AcidSnake:last_dir()
  return self.turns[#self.turns] or self.dir
end

function AcidSnake:turn(d)
  local last = self:last_dir()
  -- Reversing into your own neck, or repeating a heading, is not a turn.
  if (d[1] == -last[1] and d[2] == -last[2]) or (d[1] == last[1] and d[2] == last[2]) then return end
  if #self.turns >= N.MAX_TURNS then return end
  self.turns[#self.turns + 1] = d
end

function AcidSnake:on_key(code, pressed)
  if not pressed then return end
  local confirm = code == 32 or code == AcidKeys.ENTER
  if self.state == "title" then
    if confirm then self:start_game() end
  elseif self.state == "dead" then
    if confirm and self.dead_ticks >= N.DEAD_LOCK_TICKS then self:start_game() end
  elseif code == string.byte("p") or code == string.byte("P") then
    self.paused = not self.paused
  elseif not self.paused and TURN_KEYS[code] then
    self:turn(TURN_KEYS[code])
  end
end

function AcidSnake:on_touch(x, y, pressed)
  -- The router repeats TOUCH every tick while held: act once per press.
  if not pressed then
    self.touch_held = false
    return
  end
  if self.touch_held then return end
  self.touch_held = true
  if self.state == "title" then
    self:start_game()
  elseif self.state == "dead" then
    if self.dead_ticks >= N.DEAD_LOCK_TICKS then self:start_game() end
  elseif not self.paused then
    -- Turn towards the tap, across the way the snake is heading.
    local head = self.body[1]
    local hx = self.bx + head.x * N.CELL + N.CELL // 2
    local hy = self.by + head.y * N.CELL + N.CELL // 2
    local last = self:last_dir()
    if last[1] ~= 0 then
      if y < hy then self:turn(UP) elseif y > hy then self:turn(DOWN) end
    else
      if x < hx then self:turn(LEFT) elseif x > hx then self:turn(RIGHT) end
    end
  end
end

-- ---- update ----

function AcidSnake:die()
  self.state = "dead"
  self.dead_ticks = 0
  if self.score > self.best then self.best = self.score end
  self:play_sfx("death")
end

function AcidSnake:advance()
  if #self.turns > 0 then self.dir = table.remove(self.turns, 1) end
  local head = self.body[1]
  local nx, ny = head.x + self.dir[1], head.y + self.dir[2]
  if nx < 0 or ny < 0 or nx >= self.cols or ny >= self.rows then return self:die() end
  -- The tail moves out of the way this step unless the snake is growing.
  local last = #self.body - (self.grow > 0 and 0 or 1)
  for i = 1, last do
    local s = self.body[i]
    if s.x == nx and s.y == ny then return self:die() end
  end
  table.insert(self.body, 1, { x = nx, y = ny })
  if self.grow > 0 then
    self.grow = self.grow - 1
  else
    table.remove(self.body)
  end

  local p = self.pellet
  if p and p.x == nx and p.y == ny then
    self.score = self.score + N.PELLET_POINTS
    self.grow = self.grow + 1
    self.eaten = self.eaten + 1
    self:play_sfx("eat")
    if self.eaten % N.TRIP_EVERY == 0 and not self.trip then
      self.trip = self:free_cell()
      if self.trip then self.trip.ticks = N.TRIP_TICKS end
    end
    self:place_pellet()
  end
  local t = self.trip
  if t and t.x == nx and t.y == ny then
    self.score = self.score + N.TRIP_POINTS
    self.grow = self.grow + 3
    self.ripple = N.RIPPLE_TICKS
    self.trip = nil
    self:play_sfx("trip")
  end
end

function AcidSnake:on_tick()
  local focused = self:focused()
  if focused then
    self.ticks = self.ticks + 1
    if self.state == "playing" and not self.paused then
      if self.ripple > 0 then self.ripple = self.ripple - 1 end
      if self.trip then
        self.trip.ticks = self.trip.ticks - 1
        if self.trip.ticks <= 0 then self.trip = nil end
      end
      self.move_timer = self.move_timer + 1
      if self.move_timer >= self:move_every() then
        self.move_timer = 0
        self:advance()
      end
    elseif self.state == "dead" then
      self.dead_ticks = self.dead_ticks + 1
    end
  end
  self:tick_sfx()
  -- Unfocused, the game holds still (keys go elsewhere) and draws nothing.
  if focused then self:redraw() end
end

-- ---- sound ----

function AcidSnake:play_sfx(name)
  local s = N.SFX[name]
  -- One gate per voice: a retrigger replaces the earlier one, so its
  -- note-off can't cut the new note short.
  for i = #self.sfx, 1, -1 do
    if self.sfx[i].voice == s.voice then table.remove(self.sfx, i) end
  end
  acid_play_note(s.voice, s.notes[1], s.volume)
  acid_trigger_arp(s.voice, s.notes[1], s.notes[2], s.notes[3], s.notes[4], s.count, s.rate)
  self.sfx[#self.sfx + 1] = { voice = s.voice, ticks = s.ticks }
end

function AcidSnake:tick_sfx()
  for i = #self.sfx, 1, -1 do
    local s = self.sfx[i]
    s.ticks = s.ticks - 1
    if s.ticks <= 0 then
      acid_stop_note(s.voice)
      table.remove(self.sfx, i)
    end
  end
end

function AcidSnake:stop_all_sfx()
  if not self.sfx then return end
  for _, s in ipairs(self.sfx) do acid_stop_note(s.voice) end
  self.sfx = {}
end

function AcidSnake:on_destroy()
  self:stop_all_sfx()
end

-- ---- drawing ----

function AcidSnake:text(s, x, y, color)
  acid_draw_text(s, x, y, color, N.BG_COLOR)
end

function AcidSnake:centred(s, y, color)
  self:text(s, (self.w - #s * self.cw) // 2, y, color)
end

local function box(x, y, w, h, c)
  acid_fill_rect(x, y, w, 1, c)
  acid_fill_rect(x, y + h - 1, w, 1, c)
  acid_fill_rect(x, y, 1, h, c)
  acid_fill_rect(x + w - 1, y, 1, h, c)
end

function AcidSnake:cell_rect(x, y, inset, color)
  local c = N.CELL
  acid_fill_rect(self.bx + x * c + inset, self.by + y * c + inset, c - 2 * inset, c - 2 * inset, color)
end

function AcidSnake:draw_board()
  local bw, bh = self.cols * N.CELL, self.rows * N.CELL
  acid_fill_rect(self.bx, self.by, bw, bh, N.BOARD_COLOR)
  box(self.bx - 1, self.by - 1, bw + 2, bh + 2, AcidPalette.hue(self.ticks * 2))
end

function AcidSnake:draw_snake()
  local dead = self.state == "dead"
  -- A ripple spins the colours fast down the body; normally they drift.
  local spin = self.ripple > 0 and self.ticks * 24 or self.ticks * 3
  for i = #self.body, 1, -1 do
    local s = self.body[i]
    local color
    if dead then
      color = (self.dead_ticks // 4) % 2 == 0 and 0xFF285A or 0x50101E
    else
      color = AcidPalette.hue(spin - i * 16)
    end
    self:cell_rect(s.x, s.y, i == 1 and 0 or 1, color)
  end
  if not dead then
    -- Two eyes on the head, set towards the way it's heading.
    local h, d = self.body[1], self.dir
    local cx = self.bx + h.x * N.CELL + 3 + d[1] * 2
    local cy = self.by + h.y * N.CELL + 3 + d[2] * 2
    local ox, oy = d[2] ~= 0 and 2 or 0, d[1] ~= 0 and 2 or 0
    acid_fill_rect(cx - ox, cy - oy, 2, 2, 0xFFFFFF)
    acid_fill_rect(cx + ox, cy + oy, 2, 2, 0xFFFFFF)
  end
end

function AcidSnake:draw_pellets()
  local p = self.pellet
  if p then
    local inset = (self.ticks // 8) % 2 == 0 and 2 or 1
    self:cell_rect(p.x, p.y, inset, 0xFFE600)
  end
  local t = self.trip
  -- The trip pellet blinks through its last second.
  if t and (t.ticks > 30 or (t.ticks // 3) % 2 == 0) then
    local cx = self.bx + t.x * N.CELL + N.CELL // 2
    local cy = self.by + t.y * N.CELL + N.CELL // 2
    acid_fill_circle(cx, cy, 4, AcidPalette.hue(self.ticks * 16))
    acid_fill_circle(cx, cy, 2, AcidPalette.hue(self.ticks * 16 + 128))
  end
end

function AcidSnake:draw_hud()
  self:text(string.format("SCORE %05d", self.score), 4, self.hud_y, 0x00FFDC)
  local right = string.format("BEST %05d", self.best)
  self:text(right, self.w - 4 - #right * self.cw, self.hud_y, 0xFF3CC8)
end

function AcidSnake:draw_title()
  local ch = self.ch
  local mid = self.by + self.rows * N.CELL // 2
  local title_y = mid - ch * 4
  -- A little snake chasing its tail round the title, under the text.
  for i = 0, 19 do
    local a = (self.ticks * 0.08) - i * 0.07
    local x = self.w // 2 + math.floor(math.cos(a) * 90) - 3
    local y = title_y + ch // 2 + math.floor(math.sin(a) * (ch + 8)) - 3
    acid_fill_rect(x, y, 6, 6, AcidPalette.hue(self.ticks * 3 - i * 10))
  end
  self:centred("ACID SNAKE", title_y, AcidPalette.hue(self.ticks * 4))
  if (self.ticks // 15) % 2 == 0 then self:centred("SPACE OR TAP TO START", mid - ch, 0x00FFFF) end
  self:centred("ARROWS OR WASD TURN", mid + ch * 2, N.MUTED_COLOR)
  self:centred("EAT THE RAINBOW", mid + ch * 3 + 2, N.MUTED_COLOR)
  self:centred("P PAUSE", mid + ch * 4 + 4, N.MUTED_COLOR)
end

-- One frame, shown whole: the compositor never catches the window cleared
-- or half-painted.
function AcidSnake:redraw()
  acid_begin_frame()
  self:paint()
  acid_end_frame()
end

function AcidSnake:paint()
  acid_clear_user_area()
  acid_draw_window_frame(self:window_title())
  self:draw_board()
  self:draw_hud()
  if self.state == "title" then
    self:draw_title()
  else
    self:draw_pellets()
    self:draw_snake()
    local mid = self.by + self.rows * N.CELL // 2
    if self.state == "dead" then
      self:centred("GAME OVER", mid - self.ch * 2, 0xFF285A)
      self:centred(string.format("SCORE %05d", self.score), mid, N.TEXT_COLOR)
      if self.dead_ticks >= N.DEAD_LOCK_TICKS then
        self:centred("SPACE OR TAP", mid + self.ch * 2, 0x00FFFF)
      end
    elseif self.paused then
      self:centred("PAUSED", mid, N.TEXT_COLOR)
    end
  end
  acid_draw_window_border()
end

AcidSnake:new():start()
