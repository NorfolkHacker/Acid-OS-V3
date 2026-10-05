-- Acid Invaders: a marching alien formation, shields that crumble, bombs,
-- and a saucer across the top. Each row of aliens runs through the hue
-- wheel, a kill leaves a hue ring spreading out, and shooting the saucer
-- gives a rainbow triple shot for a few seconds. The formation speeds up
-- as it thins out and starts lower each wave.
--
-- Keys arrive as presses only, so movement latches: Left/A or Right/D set
-- the cannon moving that way (the other way stops it), Down/S stops.
-- Space, Up or W fires; one volley is in the air at a time. Holding the
-- pointer slides the cannon under it, and a fresh tap fires. P pauses.

AcidInvaders = AcidGame:extend("AcidInvaders")

local V = AcidInvaders

V.TICK_MS = 33
V.DT = V.TICK_MS / 1000
V.TITLE_BAR_H = 16
V.BG_COLOR = 0x050607     -- THEME_BG
V.TEXT_COLOR = 0xD4E6DB   -- THEME_TEXT
V.MUTED_COLOR = 0x9DAAA3  -- THEME_MUTED

V.COLS, V.ROWS = 8, 5
V.PIX = 2                 -- pixels per sprite cell
V.ALIEN_W, V.ALIEN_H = 14, 10
V.GAP_X, V.GAP_Y = 22, 16
V.MARCH_DX = 3
V.DROP = 6
V.WAVE_DROP = 6           -- each wave starts this much lower...
V.MAX_WAVE_DROPS = 4      -- ...up to this many times
V.ROW_POINTS = { 30, 20, 20, 10, 10 }
V.SAUCER_POINTS = 100
V.SAUCER_W, V.SAUCER_H = 16, 7
V.SAUCER_SPEED = 50
V.SAUCER_EVERY = 600      -- ticks between saucers (about 20 s)

V.PLAYER_W = 13
V.PLAYER_SPEED = 100
V.SHOT_SPEED = 240
V.BOMB_SPEED = 90
V.BOMB_EVERY = 30         -- ticks between bomb attempts at wave 1
V.TRIP_TICKS = 180        -- triple shot after a saucer (about 6 s)
V.INVULN_SECS = 1.5
V.WAVE_PAUSE_TICKS = 45
V.DEAD_LOCK_TICKS = 12

V.SHIELDS = 3
V.SHIELD_PATTERN = {
  "..XXXXXXX..",
  ".XXXXXXXXX.",
  "XXXXXXXXXXX",
  "XXXXXXXXXXX",
  "XXXXXXXXXXX",
  "XXX.....XXX",
  "XX.......XX",
}

-- Original silhouettes, two frames each, 7x5 cells.
V.ALIENS = {
  { { "..XXX..", ".XXXXX.", "XX.X.XX", "XXXXXXX", ".X...X." },
    { "..XXX..", ".XXXXX.", "XX.X.XX", "XXXXXXX", "X.....X" } },
  { { "X.....X", ".XXXXX.", "XX.X.XX", "XXXXXXX", "X.X.X.X" },
    { ".X...X.", ".XXXXX.", "XX.X.XX", "XXXXXXX", ".X.X.X." } },
  { { ".XXXXX.", "XXXXXXX", "X.XXX.X", "XXXXXXX", "..X.X.." },
    { ".XXXXX.", "XXXXXXX", "X.XXX.X", "XXXXXXX", ".X...X." } },
}
V.ROW_KIND = { 1, 2, 2, 3, 3 }

-- Each sfx gate is exactly one pass of its arpeggio (count * rate =
-- ticks * TICK_MS): an arp wraps until note-off.
V.SFX = {
  kill   = { voice = 0, notes = { 55, 48, 1, 1 }, count = 2, rate = 33, volume = 30, ticks = 2 },
  saucer = { voice = 1, notes = { 60, 64, 67, 72 }, count = 4, rate = 33, volume = 35, ticks = 4 },
  death  = { voice = 2, notes = { 40, 37, 33, 28 }, count = 4, rate = 66, volume = 40, ticks = 8 },
}
V.FILTER_MODE_LP = 1

-- A sprite's filled cells merged into horizontal runs, so each row costs
-- one rect per run rather than one per cell.
local function runs(pattern)
  local out = {}
  for row, line in ipairs(pattern) do
    local start = nil
    for col = 1, #line + 1 do
      local filled = line:sub(col, col) == "X"
      if filled and not start then start = col end
      if not filled and start then
        out[#out + 1] = { start - 1, row - 1, col - start }
        start = nil
      end
    end
  end
  return out
end

local ALIEN_RUNS = {}
for k, frames in ipairs(V.ALIENS) do
  ALIEN_RUNS[k] = { runs(frames[1]), runs(frames[2]) }
end

local function draw_runs(list, x, y, pix, color)
  for _, r in ipairs(list) do
    acid_fill_rect(x + r[1] * pix, y + r[2] * pix, r[3] * pix, pix, color)
  end
end

function AcidInvaders:on_create()
  acid_configure_filter(200, 3, V.FILTER_MODE_LP)
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

function AcidInvaders:layout()
  local w, h = acid_window_size()
  local cw, ch = acid_font_size()
  self.w, self.h, self.cw, self.ch = w, h, cw, ch
  self.hud_y = V.TITLE_BAR_H + 2
  self.ax0, self.ax1 = 4, w - 4
  self.ay0, self.ay1 = self.hud_y + ch + 4, h - 4
  self.player_y = self.ay1 - 10
  self.shield_y = self.player_y - 34
end

function AcidInvaders:to_title()
  self:stop_all_sfx()
  self.state = "title"
  self.score = 0
end

function AcidInvaders:start_game()
  self:stop_all_sfx()
  self.score = 0
  self.lives = 3
  self.paused = false
  self.player_x = (self.ax0 + self.ax1) / 2
  self.move = 0
  self.pointer = nil
  self:build_shields()
  self:start_wave(1)
end

function AcidInvaders:start_wave(wave)
  self.state = "playing"
  self.wave = wave
  self.aliens = {}
  local y0 = self.ay0 + 14 + math.min(wave - 1, V.MAX_WAVE_DROPS) * V.WAVE_DROP
  local width = (V.COLS - 1) * V.GAP_X + V.ALIEN_W
  local x0 = (self.ax0 + self.ax1 - width) // 2
  for row = 1, V.ROWS do
    for col = 1, V.COLS do
      self.aliens[#self.aliens + 1] = {
        row = row, col = col, x = x0 + (col - 1) * V.GAP_X, y = y0 + (row - 1) * V.GAP_Y,
      }
    end
  end
  self.march_dir = 1
  self.march_timer = 0
  self.frame = 1
  self.shots = {}
  self.bombs = {}
  self.rings = {}
  self.saucer = nil
  self.saucer_timer = V.SAUCER_EVERY
  self.bomb_timer = V.BOMB_EVERY
  self.trip = 0
  self.invuln = 0
end

function AcidInvaders:build_shields()
  self.shields = {}
  local cells_w = #V.SHIELD_PATTERN[1]
  local sw = cells_w * V.PIX
  local span = self.ax1 - self.ax0
  for i = 1, V.SHIELDS do
    local cx = self.ax0 + span * i // (V.SHIELDS + 1)
    local x0 = cx - sw // 2
    for row, line in ipairs(V.SHIELD_PATTERN) do
      for col = 1, #line do
        if line:sub(col, col) == "X" then
          self.shields[#self.shields + 1] = { x = x0 + (col - 1) * V.PIX, y = self.shield_y + (row - 1) * V.PIX }
        end
      end
    end
  end
end

-- ---- rules ----

-- Fewer aliens, faster march: one step every (alive // 4) ticks, at
-- least every tick.
function AcidInvaders:march_every()
  return math.max(1, #self.aliens // 4)
end

function AcidInvaders:march()
  local dx = V.MARCH_DX * self.march_dir
  local edge = false
  for _, a in ipairs(self.aliens) do
    if a.x + dx < self.ax0 + 2 or a.x + V.ALIEN_W + dx > self.ax1 - 2 then edge = true break end
  end
  for _, a in ipairs(self.aliens) do
    if edge then a.y = a.y + V.DROP else a.x = a.x + dx end
  end
  if edge then self.march_dir = -self.march_dir end
  self.frame = 3 - self.frame
  -- Aliens marching through a shield wear it away.
  for _, a in ipairs(self.aliens) do
    if a.y + V.ALIEN_H >= self.shield_y then
      for _, c in ipairs(self.shields) do
        if not c.dead and c.x + V.PIX > a.x and c.x < a.x + V.ALIEN_W and c.y + V.PIX > a.y and c.y < a.y + V.ALIEN_H then
          c.dead = true
        end
      end
    end
  end
  for _, a in ipairs(self.aliens) do
    if a.y + V.ALIEN_H >= self.player_y then return self:game_over() end
  end
end

function AcidInvaders:fire()
  if #self.shots > 0 then return end
  local x, y = self.player_x, self.player_y - 4
  self.shots[#self.shots + 1] = { x = x, y = y, vx = 0 }
  if self.trip > 0 then
    self.shots[#self.shots + 1] = { x = x, y = y, vx = -60 }
    self.shots[#self.shots + 1] = { x = x, y = y, vx = 60 }
  end
end

-- The lowest alien in a random column with any left drops a bomb.
function AcidInvaders:drop_bomb()
  local lowest = {}
  for _, a in ipairs(self.aliens) do
    local cur = lowest[a.col]
    if not cur or a.y > cur.y then lowest[a.col] = a end
  end
  local cols = {}
  for _, a in pairs(lowest) do cols[#cols + 1] = a end
  if #cols == 0 then return end
  table.sort(cols, function(p, q) return p.col < q.col end)
  local a = cols[math.random(1, #cols)]
  self.bombs[#self.bombs + 1] = { x = a.x + V.ALIEN_W // 2, y = a.y + V.ALIEN_H }
end

function AcidInvaders:max_bombs()
  return math.min(2 + self.wave, 6)
end

-- Whether a point hits a shield cell; if so the cell is gone.
function AcidInvaders:hit_shield(x, y)
  for _, c in ipairs(self.shields) do
    if not c.dead and x >= c.x and x < c.x + V.PIX and y >= c.y and y < c.y + V.PIX then
      c.dead = true
      return true
    end
  end
  return false
end

function AcidInvaders:ring(x, y, color)
  self.rings[#self.rings + 1] = { x = x, y = y, t = 0, color = color }
end

-- Shots and bombs move several pixels a tick but shield cells are only
-- 2 px, so they move in sub-steps of at most this, checking each one.
V.SUBSTEP = 2

local function substeps(dist)
  return math.max(1, math.ceil(math.abs(dist) / V.SUBSTEP))
end

-- Whether a shot at its current point hits something (and deals with it).
function AcidInvaders:shot_hits(s)
  if s.y < self.ay0 or s.x < self.ax0 or s.x > self.ax1 then return true end
  if self:hit_shield(s.x, s.y) then return true end
  for i, a in ipairs(self.aliens) do
    if s.x >= a.x and s.x < a.x + V.ALIEN_W and s.y >= a.y and s.y < a.y + V.ALIEN_H then
      table.remove(self.aliens, i)
      self.score = self.score + V.ROW_POINTS[a.row]
      self:ring(a.x + V.ALIEN_W // 2, a.y + V.ALIEN_H // 2, AcidPalette.hue(a.row * 40 + self.ticks * 2))
      self:play_sfx("kill")
      return true
    end
  end
  local sc = self.saucer
  if sc and s.x >= sc.x and s.x < sc.x + V.SAUCER_W and s.y >= sc.y and s.y < sc.y + V.SAUCER_H then
    self.score = self.score + V.SAUCER_POINTS
    self.trip = V.TRIP_TICKS
    self:ring(sc.x + V.SAUCER_W // 2, sc.y + V.SAUCER_H // 2, 0xFFFFFF)
    self.saucer = nil
    self:play_sfx("saucer")
    return true
  end
  return false
end

function AcidInvaders:update_shots(dt)
  local keep = {}
  for _, s in ipairs(self.shots) do
    local n = substeps(V.SHOT_SPEED * dt)
    local gone = self:shot_hits(s)
    for _ = 1, n do
      if gone then break end
      s.x = s.x + s.vx * dt / n
      s.y = s.y - V.SHOT_SPEED * dt / n
      gone = self:shot_hits(s)
    end
    if not gone then keep[#keep + 1] = s end
  end
  self.shots = keep
end

function AcidInvaders:bomb_hits_player(b)
  return self.invuln <= 0 and math.abs(b.x - self.player_x) <= V.PLAYER_W / 2
    and b.y >= self.player_y - 4 and b.y <= self.player_y + 4
end

function AcidInvaders:update_bombs(dt)
  local keep = {}
  for _, b in ipairs(self.bombs) do
    local n = substeps(V.BOMB_SPEED * dt)
    local gone = false
    for _ = 1, n do
      b.y = b.y + V.BOMB_SPEED * dt / n
      if self:bomb_hits_player(b) then
        -- A hit clears every bomb, so the rest of this list is moot.
        return self:player_hit()
      end
      if b.y > self.ay1 or self:hit_shield(b.x, b.y) then gone = true break end
    end
    if not gone then keep[#keep + 1] = b end
  end
  self.bombs = keep
end

function AcidInvaders:player_hit()
  self:ring(self.player_x, self.player_y, 0xFF3C3C)
  self:play_sfx("death")
  self.lives = self.lives - 1
  self.bombs = {}
  if self.lives <= 0 then return self:game_over() end
  self.invuln = V.INVULN_SECS
end

function AcidInvaders:game_over()
  self.state = "dead"
  self.dead_ticks = 0
  if self.score > self.best then self.best = self.score end
  self.move = 0
  self.pointer = nil
end

function AcidInvaders:step(dt)
  if self.invuln > 0 then self.invuln = self.invuln - dt end
  if self.trip > 0 then self.trip = self.trip - 1 end

  -- Cannon: the pointer, while held, wins over the latched direction.
  local half = V.PLAYER_W / 2
  if self.pointer then
    local d = self.pointer.x - self.player_x
    local stepx = V.PLAYER_SPEED * dt
    if math.abs(d) <= stepx then self.player_x = self.pointer.x
    else self.player_x = self.player_x + (d > 0 and stepx or -stepx) end
  else
    self.player_x = self.player_x + self.move * V.PLAYER_SPEED * dt
  end
  self.player_x = math.max(self.ax0 + half, math.min(self.ax1 - half, self.player_x))

  self.march_timer = self.march_timer + 1
  if self.march_timer >= self:march_every() then
    self.march_timer = 0
    self:march()
    if self.state ~= "playing" then return end
  end

  self.bomb_timer = self.bomb_timer - 1
  if self.bomb_timer <= 0 then
    if #self.bombs < self:max_bombs() then self:drop_bomb() end
    self.bomb_timer = math.max(8, V.BOMB_EVERY - self.wave * 3)
  end

  self.saucer_timer = self.saucer_timer - 1
  if self.saucer_timer <= 0 and not self.saucer then
    local from_left = math.random(0, 1) == 0
    self.saucer = {
      x = from_left and self.ax0 or self.ax1 - V.SAUCER_W,
      y = self.ay0 + 2, vx = from_left and V.SAUCER_SPEED or -V.SAUCER_SPEED,
    }
    self.saucer_timer = V.SAUCER_EVERY
  end
  local sc = self.saucer
  if sc then
    sc.x = sc.x + sc.vx * dt
    if sc.x < self.ax0 or sc.x + V.SAUCER_W > self.ax1 then self.saucer = nil end
  end

  self:update_shots(dt)
  self:update_bombs(dt)
  if self.state ~= "playing" then return end

  if #self.aliens == 0 then
    self.state = "wave_clear"
    self.wave_ticks = 0
    self.shots, self.bombs, self.saucer = {}, {}, nil
  end
end

function AcidInvaders:tick_rings()
  local keep = {}
  for _, r in ipairs(self.rings) do
    r.t = r.t + 1
    if r.t < 12 then keep[#keep + 1] = r end
  end
  self.rings = keep
end

function AcidInvaders:on_tick()
  local focused = self:focused()
  -- Unfocused, keys and touches go elsewhere: hold still, and drop a
  -- pointer whose release we may never see.
  if not focused then self.pointer = nil end
  if focused then
    self.ticks = self.ticks + 1
    if self.state == "playing" and not self.paused then
      self:step(V.DT)
      self:tick_rings()
    elseif self.state == "wave_clear" then
      self:tick_rings()
      self.wave_ticks = self.wave_ticks + 1
      if self.wave_ticks >= V.WAVE_PAUSE_TICKS then self:start_wave(self.wave + 1) end
    elseif self.state == "dead" then
      self:tick_rings()
      self.dead_ticks = self.dead_ticks + 1
    end
  end
  self:tick_sfx()
  if focused then self:redraw() end
end

-- ---- input ----

local LEFT_KEYS = { [AcidKeys.LEFT] = true, [string.byte("a")] = true, [string.byte("A")] = true }
local RIGHT_KEYS = { [AcidKeys.RIGHT] = true, [string.byte("d")] = true, [string.byte("D")] = true }
local STOP_KEYS = { [AcidKeys.DOWN] = true, [string.byte("s")] = true, [string.byte("S")] = true }
local FIRE_KEYS = { [32] = true, [AcidKeys.UP] = true, [string.byte("w")] = true, [string.byte("W")] = true }

function AcidInvaders:on_key(code, pressed)
  if not pressed then return end
  local confirm = code == 32 or code == AcidKeys.ENTER
  if self.state == "title" then
    if confirm then self:start_game() end
  elseif self.state == "dead" then
    if confirm and self.dead_ticks >= V.DEAD_LOCK_TICKS then self:start_game() end
  elseif code == string.byte("p") or code == string.byte("P") then
    if self.state == "playing" then self.paused = not self.paused end
  elseif self.paused then
    return
  elseif LEFT_KEYS[code] then
    self.move = self.move == 1 and 0 or -1
  elseif RIGHT_KEYS[code] then
    self.move = self.move == -1 and 0 or 1
  elseif STOP_KEYS[code] then
    self.move = 0
  elseif FIRE_KEYS[code] and self.state == "playing" then
    self:fire()
  end
end

function AcidInvaders:on_touch(x, y, pressed)
  if not pressed then
    self.touch_held = false
    self.pointer = nil
    return
  end
  -- The router repeats TOUCH every tick while held: firing and the
  -- screens act once per press, the cannon follows for as long as it's
  -- down.
  local fresh = not self.touch_held
  self.touch_held = true
  if self.state == "title" then
    if fresh then self:start_game() end
  elseif self.state == "dead" then
    if fresh and self.dead_ticks >= V.DEAD_LOCK_TICKS then self:start_game() end
  elseif not self.paused then
    self.pointer = { x = x, y = y }
    if fresh and self.state == "playing" then self:fire() end
  end
end

-- ---- sound ----

function AcidInvaders:play_sfx(name)
  local s = V.SFX[name]
  -- One gate per voice: a retrigger replaces the earlier one, so its
  -- note-off can't cut the new note short.
  for i = #self.sfx, 1, -1 do
    if self.sfx[i].voice == s.voice then table.remove(self.sfx, i) end
  end
  acid_play_note(s.voice, s.notes[1], s.volume)
  acid_trigger_arp(s.voice, s.notes[1], s.notes[2], s.notes[3], s.notes[4], s.count, s.rate)
  self.sfx[#self.sfx + 1] = { voice = s.voice, ticks = s.ticks }
end

function AcidInvaders:tick_sfx()
  for i = #self.sfx, 1, -1 do
    local s = self.sfx[i]
    s.ticks = s.ticks - 1
    if s.ticks <= 0 then
      acid_stop_note(s.voice)
      table.remove(self.sfx, i)
    end
  end
end

function AcidInvaders:stop_all_sfx()
  if not self.sfx then return end
  for _, s in ipairs(self.sfx) do acid_stop_note(s.voice) end
  self.sfx = {}
end

function AcidInvaders:on_destroy()
  self:stop_all_sfx()
end

-- ---- drawing ----

function AcidInvaders:text(s, x, y, color)
  acid_draw_text(s, x, y, color, V.BG_COLOR)
end

function AcidInvaders:centred(s, y, color)
  self:text(s, (self.w - #s * self.cw) // 2, y, color)
end

function AcidInvaders:draw_hud()
  self:text(string.format("SCORE %05d", self.score), 4, self.hud_y, 0x00FFDC)
  local right
  if self.state == "title" then
    right = string.format("BEST %05d", self.best)
  else
    right = string.format("W%02d L%d", self.wave, math.max(self.lives, 0))
  end
  self:text(right, self.w - 4 - #right * self.cw, self.hud_y, 0xFF3CC8)
end

function AcidInvaders:draw_rings()
  for _, r in ipairs(self.rings) do
    local rad = 3 + r.t
    -- A ring: the colour, then the background punched out of the middle.
    -- Kept inside the play area so it never paints over the HUD.
    if r.y - rad >= self.ay0 and r.y + rad <= self.ay1 and r.x - rad >= self.ax0 and r.x + rad <= self.ax1 then
      acid_fill_circle(r.x, r.y, rad, r.color)
      acid_fill_circle(r.x, r.y, rad - 2, V.BG_COLOR)
    end
  end
end

function AcidInvaders:draw_aliens()
  for _, a in ipairs(self.aliens) do
    local color = AcidPalette.hue(a.row * 40 + self.ticks * 2)
    draw_runs(ALIEN_RUNS[V.ROW_KIND[a.row]][self.frame], math.floor(a.x), math.floor(a.y), V.PIX, color)
  end
end

function AcidInvaders:draw_player()
  if self.invuln > 0 and math.floor(self.invuln * 12) % 2 == 0 then return end
  local x = math.floor(self.player_x - V.PLAYER_W / 2)
  local y = self.player_y
  local color = self.trip > 0 and AcidPalette.hue(self.ticks * 12) or 0x00FFB4
  acid_fill_rect(x, y, V.PLAYER_W, 4, color)
  acid_fill_rect(x + 2, y - 2, V.PLAYER_W - 4, 2, color)
  acid_fill_rect(x + 5, y - 4, 3, 2, color)
end

function AcidInvaders:draw_play()
  self:draw_rings()
  for _, c in ipairs(self.shields) do
    if not c.dead then acid_fill_rect(c.x, c.y, V.PIX, V.PIX, 0x3CDC78) end
  end
  self:draw_aliens()
  local sc = self.saucer
  if sc then
    local x, y = math.floor(sc.x), sc.y
    local color = AcidPalette.hue(self.ticks * 10)
    acid_fill_rect(x + 4, y, 8, 2, color)
    acid_fill_rect(x, y + 2, V.SAUCER_W, 3, color)
    acid_fill_rect(x + 2, y + 5, V.SAUCER_W - 4, 2, color)
  end
  for _, s in ipairs(self.shots) do
    local color = self.trip > 0 and AcidPalette.hue(self.ticks * 20 + math.floor(s.vx)) or 0xFFFFFF
    acid_fill_rect(math.floor(s.x), math.floor(s.y), 1, 4, color)
  end
  for _, b in ipairs(self.bombs) do
    local wiggle = (self.ticks // 3) % 2
    acid_fill_rect(math.floor(b.x) - wiggle, math.floor(b.y) - 4, 2, 4, 0xFF5A28)
  end
  if self.state ~= "dead" then self:draw_player() end
end

function AcidInvaders:draw_title()
  local ch = self.ch
  local mid = (self.ay0 + self.ay1) // 2
  -- The three aliens marching in place along the top.
  local frame = (self.ticks // 15) % 2 + 1
  for k = 1, 3 do
    local x = self.w // 2 - 50 + (k - 1) * 40
    draw_runs(ALIEN_RUNS[k][frame], x, mid - ch * 4 - 16, V.PIX, AcidPalette.hue(k * 60 + self.ticks * 3))
  end
  self:centred("ACID INVADERS", mid - ch * 4, AcidPalette.hue(self.ticks * 4))
  if (self.ticks // 15) % 2 == 0 then self:centred("SPACE OR TAP TO START", mid - ch, 0x00FFFF) end
  self:centred("LEFT RIGHT MOVE  DOWN STOP", mid + ch * 2, V.MUTED_COLOR)
  self:centred("SPACE FIRE  P PAUSE", mid + ch * 3 + 2, V.MUTED_COLOR)
  self:centred("SHOOT THE SAUCER", mid + ch * 4 + 4, V.MUTED_COLOR)
end

function AcidInvaders:redraw()
  acid_clear_user_area()
  acid_draw_window_frame(self:window_title())
  self:draw_hud()
  if self.state == "title" then
    self:draw_title()
  else
    self:draw_play()
    local mid = (self.ay0 + self.ay1) // 2
    if self.state == "wave_clear" then
      self:centred(string.format("WAVE %02d", self.wave + 1), mid, 0x00FFB4)
    elseif self.state == "dead" then
      self:centred("GAME OVER", mid - self.ch * 2, 0xFF285A)
      self:centred(string.format("SCORE %05d", self.score), mid, V.TEXT_COLOR)
      if self.dead_ticks >= V.DEAD_LOCK_TICKS then
        self:centred("SPACE OR TAP", mid + self.ch * 2, 0x00FFFF)
      end
    elseif self.paused then
      self:centred("PAUSED", mid, V.TEXT_COLOR)
    end
  end
  acid_draw_window_border()
end

AcidInvaders:new():start()
