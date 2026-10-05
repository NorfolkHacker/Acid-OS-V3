-- Acid Rocks: drift a vector ship round a wrapping field of rocks and
-- shoot them to pieces. Big rocks split into two medium, medium into two
-- small. Every rock runs through the hue wheel and drags a dimmed echo of
-- itself, thrust leaves a rainbow exhaust, and a broken rock throws off
-- shards.
--
-- Keys arrive as presses only, so controls latch: Left/A or Right/D set
-- the ship turning (the other way stops it), Up/W toggles thrust, Space
-- fires, Down/S jumps through hyperspace. Holding the pointer turns the
-- ship towards it and fires on its own; a fresh tap fires at once.
-- P pauses.

AcidRocks = AcidGame:extend("AcidRocks")

local R = AcidRocks

R.TICK_MS = 33
R.DT = R.TICK_MS / 1000
R.TITLE_BAR_H = 16
R.BG_COLOR = 0x050607     -- THEME_BG
R.TEXT_COLOR = 0xD4E6DB   -- THEME_TEXT
R.MUTED_COLOR = 0x9DAAA3  -- THEME_MUTED

R.SHIP_R = 5
R.TURN_SPEED = 4.0        -- radians a second
R.THRUST = 140            -- px/s/s
R.DRAG = 0.985            -- per tick
R.MAX_SPEED = 160
R.BULLET_SPEED = 230
R.BULLET_LIFE = 0.8
R.MAX_BULLETS = 4
R.AUTO_FIRE_SECS = 0.2
R.POINTER_DEADZONE = 6
R.INVULN_SECS = 2.0
R.HYPER_COOLDOWN = 1.0
R.SAFE_RADIUS = 40        -- respawn waits until no rock is this close
R.SPAWN_GAP = 60          -- new rocks start at least this far from the ship
R.EXTRA_LIFE_EVERY = 5000
R.WAVE_PAUSE_TICKS = 45
R.DEAD_LOCK_TICKS = 12

-- By size: 3 big, 2 medium, 1 small.
R.ROCK_R = { 5, 9, 16 }
R.ROCK_POINTS = { 100, 50, 20 }
R.ROCK_SPEED = { { 45, 70 }, { 30, 50 }, { 20, 35 } }
R.ROCK_VERTS = 10

-- Each sfx gate is exactly one pass of its arpeggio (count * rate =
-- ticks * TICK_MS): an arp wraps until note-off.
R.SFX = {
  shoot = { voice = 0, notes = { 72, 1, 1, 1 }, count = 1, rate = 33, volume = 20, ticks = 1 },
  boom  = { voice = 1, notes = { 40, 35, 1, 1 }, count = 2, rate = 33, volume = 35, ticks = 2 },
  death = { voice = 2, notes = { 40, 37, 33, 28 }, count = 4, rate = 66, volume = 40, ticks = 8 },
}
R.FILTER_MODE_LP = 1

local TAU = 2 * math.pi

local function frange(a, b) return a + math.random() * (b - a) end

local function scaled(color, percent)
  local r = (color >> 16 & 255) * percent // 100
  local g = (color >> 8 & 255) * percent // 100
  local b = (color & 255) * percent // 100
  return (r << 16) | (g << 8) | b
end

local function line(x1, y1, x2, y2, c)
  acid_draw_line(math.floor(x1), math.floor(y1), math.floor(x2), math.floor(y2), c)
end

function AcidRocks:on_create()
  acid_configure_filter(200, 3, R.FILTER_MODE_LP)
  acid_configure_voice(0, 1, 1, 20, 40, 30)
  acid_configure_voice(1, 1, 2, 60, 40, 120)
  acid_configure_voice(2, 1, 8, 150, 35, 250)
  self.sfx = {}
  self.ticks = 0
  self.best = 0
  self.touch_held = false
  self:layout()
  self:to_title()
end

function AcidRocks:layout()
  local w, h = acid_window_size()
  local cw, ch = acid_font_size()
  self.w, self.h, self.cw, self.ch = w, h, cw, ch
  self.hud_y = R.TITLE_BAR_H + 2
  self.ax0, self.ax1 = 4, w - 4
  self.ay0, self.ay1 = self.hud_y + ch + 4, h - 4
  self.cx, self.cy = (self.ax0 + self.ax1) / 2, (self.ay0 + self.ay1) / 2
end

function AcidRocks:to_title()
  self:stop_all_sfx()
  self.state = "title"
  self.score = 0
  self.title_rock = self:new_rock(3, self.cx, self.cy)
  self.title_rock.vx, self.title_rock.vy = 0, 0
end

function AcidRocks:start_game()
  self:stop_all_sfx()
  self.score = 0
  self.lives = 3
  self.next_extra = R.EXTRA_LIFE_EVERY
  self.paused = false
  self:start_wave(1)
end

function AcidRocks:reset_ship()
  self.ship = { x = self.cx, y = self.cy, vx = 0, vy = 0, a = -math.pi / 2, r = R.SHIP_R }
  self.turn = 0
  self.thrust = false
  self.invuln = R.INVULN_SECS
end

function AcidRocks:start_wave(wave)
  self.state = "playing"
  self.wave = wave
  self.bullets = {}
  self.parts = {}
  self.rocks = {}
  self.pointer = nil
  self.fire_cd = 0
  self.hyper_cd = 0
  self.waiting = false
  self:reset_ship()
  for _ = 1, math.min(3 + wave, 8) do
    local x, y
    repeat
      x, y = frange(self.ax0, self.ax1), frange(self.ay0, self.ay1)
    until math.abs(x - self.cx) + math.abs(y - self.cy) >= R.SPAWN_GAP * 1.5
    self.rocks[#self.rocks + 1] = self:new_rock(3, x, y)
  end
end

-- A rock of the given size: a lumpy polygon (one radius per vertex),
-- moving in a random direction, spinning, with its own hue.
function AcidRocks:new_rock(size, x, y)
  local a = frange(0, TAU)
  local sp = frange(R.ROCK_SPEED[size][1], R.ROCK_SPEED[size][2])
  local shape = {}
  for i = 1, R.ROCK_VERTS do shape[i] = frange(0.72, 1.12) end
  return {
    size = size, r = R.ROCK_R[size], x = x, y = y,
    vx = math.cos(a) * sp, vy = math.sin(a) * sp,
    a = frange(0, TAU), spin = frange(-1.5, 1.5),
    shape = shape, hue = math.random(0, 255),
  }
end

-- ---- rules ----

function AcidRocks:wrap(e)
  local w, h = self.ax1 - self.ax0, self.ay1 - self.ay0
  if e.x < self.ax0 then e.x = e.x + w elseif e.x >= self.ax1 then e.x = e.x - w end
  if e.y < self.ay0 then e.y = e.y + h elseif e.y >= self.ay1 then e.y = e.y - h end
end

-- Distance between two points across the wrap: the shorter way round.
function AcidRocks:dist(ax, ay, bx, by)
  local w, h = self.ax1 - self.ax0, self.ay1 - self.ay0
  local dx, dy = math.abs(ax - bx), math.abs(ay - by)
  dx, dy = math.min(dx, w - dx), math.min(dy, h - dy)
  return math.sqrt(dx * dx + dy * dy)
end

function AcidRocks:fire()
  if #self.bullets >= R.MAX_BULLETS then return end
  local s = self.ship
  local ca, sa = math.cos(s.a), math.sin(s.a)
  self.bullets[#self.bullets + 1] = {
    x = s.x + ca * (s.r + 2), y = s.y + sa * (s.r + 2),
    vx = s.vx + ca * R.BULLET_SPEED, vy = s.vy + sa * R.BULLET_SPEED,
    life = R.BULLET_LIFE,
  }
  self:play_sfx("shoot")
end

function AcidRocks:hyperspace()
  if self.hyper_cd > 0 then return end
  local s = self.ship
  self:burst(s.x, s.y, 12, 0)
  s.x, s.y = frange(self.ax0 + 10, self.ax1 - 10), frange(self.ay0 + 10, self.ay1 - 10)
  s.vx, s.vy = 0, 0
  self:burst(s.x, s.y, 12, 128)
  self.hyper_cd = R.HYPER_COOLDOWN
end

function AcidRocks:burst(x, y, n, hue)
  for i = 1, n do
    local a, sp = frange(0, TAU), frange(20, 80)
    self.parts[#self.parts + 1] = {
      x = x, y = y, vx = math.cos(a) * sp, vy = math.sin(a) * sp,
      life = frange(0.3, 0.7), hue = hue + i * 9,
    }
  end
end

function AcidRocks:add_score(n)
  self.score = self.score + n
  if self.score >= self.next_extra then
    self.lives = self.lives + 1
    self.next_extra = self.next_extra + R.EXTRA_LIFE_EVERY
  end
end

function AcidRocks:break_rock(i)
  local r = table.remove(self.rocks, i)
  self:add_score(R.ROCK_POINTS[r.size])
  self:burst(r.x, r.y, 4 + r.size * 4, r.hue)
  self:play_sfx("boom")
  if r.size > 1 then
    for _ = 1, 2 do
      local k = self:new_rock(r.size - 1, r.x, r.y)
      k.hue = r.hue + math.random(-40, 40)
      self.rocks[#self.rocks + 1] = k
    end
  end
end

function AcidRocks:ship_hit()
  local s = self.ship
  self:burst(s.x, s.y, 24, 0)
  self:play_sfx("death")
  self.lives = self.lives - 1
  self.thrust = false
  self.turn = 0
  if self.lives <= 0 then
    self.state = "dead"
    self.dead_ticks = 0
    if self.score > self.best then self.best = self.score end
    self.pointer = nil
    return
  end
  -- Back in the middle once it's clear there.
  self.waiting = true
end

function AcidRocks:centre_clear()
  for _, r in ipairs(self.rocks) do
    if self:dist(r.x, r.y, self.cx, self.cy) < R.SAFE_RADIUS + r.r then return false end
  end
  return true
end

function AcidRocks:step(dt)
  for _, r in ipairs(self.rocks) do
    r.x, r.y = r.x + r.vx * dt, r.y + r.vy * dt
    r.a = r.a + r.spin * dt
    self:wrap(r)
  end

  if self.waiting then
    if self:centre_clear() then
      self.waiting = false
      self:reset_ship()
    end
  else
    self:step_ship(dt)
  end

  local keep = {}
  for _, b in ipairs(self.bullets) do
    b.x, b.y = b.x + b.vx * dt, b.y + b.vy * dt
    b.life = b.life - dt
    self:wrap(b)
    local gone = b.life <= 0
    if not gone then
      for i, r in ipairs(self.rocks) do
        if self:dist(b.x, b.y, r.x, r.y) < r.r then
          self:break_rock(i)
          gone = true
          break
        end
      end
    end
    if not gone then keep[#keep + 1] = b end
  end
  self.bullets = keep

  if not self.waiting and self.invuln <= 0 then
    local s = self.ship
    for _, r in ipairs(self.rocks) do
      if self:dist(s.x, s.y, r.x, r.y) < r.r + s.r - 1 then
        self:ship_hit()
        break
      end
    end
  end

  self:step_parts(dt)

  if self.state == "playing" and #self.rocks == 0 then
    self.state = "wave_clear"
    self.wave_ticks = 0
  end
end

function AcidRocks:step_ship(dt)
  local s = self.ship
  if self.invuln > 0 then self.invuln = self.invuln - dt end
  if self.hyper_cd > 0 then self.hyper_cd = self.hyper_cd - dt end
  self.fire_cd = self.fire_cd - dt

  local p = self.pointer
  if p then
    -- Turn towards the pointer, no faster than the keys turn.
    local dx, dy = p.x - s.x, p.y - s.y
    if math.sqrt(dx * dx + dy * dy) > R.POINTER_DEADZONE then
      local want = math.atan(dy, dx)
      local diff = (want - s.a + math.pi) % TAU - math.pi
      local max = R.TURN_SPEED * dt
      s.a = s.a + math.max(-max, math.min(max, diff))
      if self.fire_cd <= 0 then
        self:fire()
        self.fire_cd = R.AUTO_FIRE_SECS
      end
    end
  else
    s.a = s.a + self.turn * R.TURN_SPEED * dt
  end
  s.a = s.a % TAU

  if self.thrust then
    s.vx = s.vx + math.cos(s.a) * R.THRUST * dt
    s.vy = s.vy + math.sin(s.a) * R.THRUST * dt
    -- Rainbow exhaust out of the back.
    if self.ticks % 2 == 0 then
      local back = s.a + math.pi + frange(-0.4, 0.4)
      self.parts[#self.parts + 1] = {
        x = s.x - math.cos(s.a) * s.r, y = s.y - math.sin(s.a) * s.r,
        vx = s.vx + math.cos(back) * 40, vy = s.vy + math.sin(back) * 40,
        life = 0.4, hue = self.ticks * 8,
      }
    end
  end
  s.vx, s.vy = s.vx * R.DRAG, s.vy * R.DRAG
  local sp = math.sqrt(s.vx * s.vx + s.vy * s.vy)
  if sp > R.MAX_SPEED then
    s.vx, s.vy = s.vx / sp * R.MAX_SPEED, s.vy / sp * R.MAX_SPEED
  end
  s.x, s.y = s.x + s.vx * dt, s.y + s.vy * dt
  self:wrap(s)
end

function AcidRocks:step_parts(dt)
  local keep = {}
  for _, p in ipairs(self.parts) do
    p.x, p.y = p.x + p.vx * dt, p.y + p.vy * dt
    p.vx, p.vy = p.vx * 0.94, p.vy * 0.94
    p.life = p.life - dt
    if p.life > 0 and p.x >= self.ax0 and p.x < self.ax1 and p.y >= self.ay0 and p.y < self.ay1 then
      keep[#keep + 1] = p
    end
  end
  self.parts = keep
end

function AcidRocks:on_tick()
  local focused = self:focused()
  -- Unfocused, keys and touches go elsewhere: hold still, and drop a
  -- pointer whose release we may never see.
  if not focused then self.pointer = nil end
  if focused then
    self.ticks = self.ticks + 1
    if self.state == "title" then
      local r = self.title_rock
      r.a = r.a + 0.6 * R.DT
    elseif self.state == "playing" and not self.paused then
      self:step(R.DT)
    elseif self.state == "wave_clear" then
      self:step_parts(R.DT)
      self.wave_ticks = self.wave_ticks + 1
      if self.wave_ticks >= R.WAVE_PAUSE_TICKS then
        self:start_wave(self.wave + 1)
      end
    elseif self.state == "dead" then
      self:step_parts(R.DT)
      self.dead_ticks = self.dead_ticks + 1
    end
  end
  self:tick_sfx()
  if focused then self:redraw() end
end

-- ---- input ----

local LEFT_KEYS = { [AcidKeys.LEFT] = true, [string.byte("a")] = true, [string.byte("A")] = true }
local RIGHT_KEYS = { [AcidKeys.RIGHT] = true, [string.byte("d")] = true, [string.byte("D")] = true }
local THRUST_KEYS = { [AcidKeys.UP] = true, [string.byte("w")] = true, [string.byte("W")] = true }
local HYPER_KEYS = { [AcidKeys.DOWN] = true, [string.byte("s")] = true, [string.byte("S")] = true }

function AcidRocks:on_key(code, pressed)
  if not pressed then return end
  local confirm = code == 32 or code == AcidKeys.ENTER
  if self.state == "title" then
    if confirm then self:start_game() end
  elseif self.state == "dead" then
    if confirm and self.dead_ticks >= R.DEAD_LOCK_TICKS then self:start_game() end
  elseif self.state ~= "playing" then
    return
  elseif code == string.byte("p") or code == string.byte("P") then
    self.paused = not self.paused
  elseif self.paused or self.waiting then
    return
  elseif LEFT_KEYS[code] then
    self.turn = self.turn == 1 and 0 or -1
  elseif RIGHT_KEYS[code] then
    self.turn = self.turn == -1 and 0 or 1
  elseif THRUST_KEYS[code] then
    self.thrust = not self.thrust
  elseif HYPER_KEYS[code] then
    self:hyperspace()
  elseif code == 32 then
    self:fire()
  end
end

function AcidRocks:on_touch(x, y, pressed)
  if not pressed then
    self.touch_held = false
    self.pointer = nil
    return
  end
  -- The router repeats TOUCH every tick while held: the screens and the
  -- first shot act once per press, aiming lasts while it's down.
  local fresh = not self.touch_held
  self.touch_held = true
  if self.state == "title" then
    if fresh then self:start_game() end
  elseif self.state == "dead" then
    if fresh and self.dead_ticks >= R.DEAD_LOCK_TICKS then self:start_game() end
  elseif self.state == "playing" and not self.paused and not self.waiting then
    self.pointer = { x = x, y = y }
    if fresh then
      self:fire()
      self.fire_cd = R.AUTO_FIRE_SECS
    end
  end
end

-- ---- sound ----

function AcidRocks:play_sfx(name)
  local s = R.SFX[name]
  -- One gate per voice: a retrigger replaces the earlier one, so its
  -- note-off can't cut the new note short.
  for i = #self.sfx, 1, -1 do
    if self.sfx[i].voice == s.voice then table.remove(self.sfx, i) end
  end
  acid_play_note(s.voice, s.notes[1], s.volume)
  acid_trigger_arp(s.voice, s.notes[1], s.notes[2], s.notes[3], s.notes[4], s.count, s.rate)
  self.sfx[#self.sfx + 1] = { voice = s.voice, ticks = s.ticks }
end

function AcidRocks:tick_sfx()
  for i = #self.sfx, 1, -1 do
    local s = self.sfx[i]
    s.ticks = s.ticks - 1
    if s.ticks <= 0 then
      acid_stop_note(s.voice)
      table.remove(self.sfx, i)
    end
  end
end

function AcidRocks:stop_all_sfx()
  if not self.sfx then return end
  for _, s in ipairs(self.sfx) do acid_stop_note(s.voice) end
  self.sfx = {}
end

function AcidRocks:on_destroy()
  self:stop_all_sfx()
end

-- ---- drawing ----

function AcidRocks:text(s, x, y, color)
  acid_draw_text(s, x, y, color, R.BG_COLOR)
end

function AcidRocks:centred(s, y, color)
  self:text(s, (self.w - #s * self.cw) // 2, y, color)
end

function AcidRocks:draw_rock_at(r, x, y, a, scale, color)
  local n = #r.shape
  local px, py
  local fx, fy
  for i = 1, n do
    local ang = a + (i - 1) * TAU / n
    local rad = r.r * r.shape[i] * scale
    local vx, vy = x + math.cos(ang) * rad, y + math.sin(ang) * rad
    if px then line(px, py, vx, vy, color) else fx, fy = vx, vy end
    px, py = vx, vy
  end
  line(px, py, fx, fy, color)
end

function AcidRocks:draw_rock(r, scale)
  scale = scale or 1
  local color = AcidPalette.hue(r.hue + self.ticks * 3)
  -- The echo: where it was a tenth of a second ago, dimmed.
  self:draw_rock_at(r, r.x - r.vx * 0.1, r.y - r.vy * 0.1, r.a - r.spin * 0.1, scale, scaled(color, 35))
  self:draw_rock_at(r, r.x, r.y, r.a, scale, color)
end

function AcidRocks:draw_ship()
  local s = self.ship
  if self.invuln > 0 and math.floor(self.invuln * 12) % 2 == 0 then return end
  local ca, sa = math.cos(s.a), math.sin(s.a)
  local nx, ny = s.x + ca * (s.r + 2), s.y + sa * (s.r + 2)
  local lx, ly = s.x + math.cos(s.a + 2.5) * s.r, s.y + math.sin(s.a + 2.5) * s.r
  local rx, ry = s.x + math.cos(s.a - 2.5) * s.r, s.y + math.sin(s.a - 2.5) * s.r
  local color = 0xFFFFFF
  line(nx, ny, lx, ly, color)
  line(nx, ny, rx, ry, color)
  line(lx, ly, rx, ry, color)
  if self.thrust and self.ticks % 2 == 0 then
    local fx, fy = s.x - ca * (s.r + 4), s.y - sa * (s.r + 4)
    line((lx + s.x) / 2, (ly + s.y) / 2, fx, fy, AcidPalette.hue(self.ticks * 8))
    line((rx + s.x) / 2, (ry + s.y) / 2, fx, fy, AcidPalette.hue(self.ticks * 8))
  end
end

-- Rocks and the ship wrap, so their lines run past the play area; paint
-- the margins and the HUD band back over them before the text and border.
function AcidRocks:mask_edges()
  acid_fill_rect(0, R.TITLE_BAR_H, self.w, self.ay0 - R.TITLE_BAR_H, R.BG_COLOR)
  acid_fill_rect(0, self.ay1, self.w, self.h - self.ay1, R.BG_COLOR)
  acid_fill_rect(0, self.ay0, self.ax0, self.ay1 - self.ay0, R.BG_COLOR)
  acid_fill_rect(self.ax1, self.ay0, self.w - self.ax1, self.ay1 - self.ay0, R.BG_COLOR)
end

function AcidRocks:draw_hud()
  self:text(string.format("SCORE %05d", self.score), 4, self.hud_y, 0x00FFDC)
  local right
  if self.state == "title" then
    right = string.format("BEST %05d", self.best)
  else
    right = string.format("W%02d L%d", self.wave, math.max(self.lives, 0))
  end
  self:text(right, self.w - 4 - #right * self.cw, self.hud_y, 0xFF3CC8)
end

-- Title at the top, a big slow rock in the middle, the controls at the
-- bottom: laid out from the font size so it fits at Large too.
function AcidRocks:draw_title()
  local ch = self.ch
  local r = self.title_rock
  r.x, r.y = self.cx, (self.ay0 + self.ay1) / 2
  self:draw_rock(r, 1.6)
  self:mask_edges()
  self:centred("ACID ROCKS", self.ay0 + 8, AcidPalette.hue(self.ticks * 4))
  if (self.ticks // 15) % 2 == 0 then self:centred("SPACE OR TAP TO START", self.ay0 + 12 + ch, 0x00FFFF) end
  local y = self.ay1 - 4 - 3 * (ch + 2)
  self:centred("LEFT RIGHT TURN  UP THRUST", y, R.MUTED_COLOR)
  self:centred("SPACE FIRE  DOWN JUMP", y + ch + 2, R.MUTED_COLOR)
  self:centred("HOLD POINTER TO AIM", y + 2 * (ch + 2), R.MUTED_COLOR)
end

function AcidRocks:redraw()
  acid_clear_user_area()
  acid_draw_window_frame(self:window_title())
  if self.state == "title" then
    self:draw_title()
  else
    for _, p in ipairs(self.parts) do
      acid_fill_rect(math.floor(p.x), math.floor(p.y), 1, 1, AcidPalette.hue(p.hue))
    end
    for _, r in ipairs(self.rocks) do self:draw_rock(r) end
    for _, b in ipairs(self.bullets) do
      acid_fill_rect(math.floor(b.x), math.floor(b.y), 2, 2, 0xFFFFFF)
    end
    if self.state ~= "dead" and not self.waiting then self:draw_ship() end
    self:mask_edges()
    local mid = (self.ay0 + self.ay1) // 2
    if self.state == "wave_clear" then
      self:centred(string.format("WAVE %02d", self.wave + 1), mid, 0x00FFB4)
    elseif self.state == "dead" then
      self:centred("GAME OVER", mid - self.ch * 2, 0xFF285A)
      self:centred(string.format("SCORE %05d", self.score), mid, R.TEXT_COLOR)
      if self.dead_ticks >= R.DEAD_LOCK_TICKS then
        self:centred("SPACE OR TAP", mid + self.ch * 2, 0x00FFFF)
      end
    elseif self.paused then
      self:centred("PAUSED", mid, R.TEXT_COLOR)
    end
  end
  self:draw_hud()
  acid_draw_window_border()
end

AcidRocks:new():start()
