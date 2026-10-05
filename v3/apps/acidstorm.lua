-- AcidStorm: an arena shooter, ported from RaveOS's SDL demo
-- (demos/acidstorm). Grunts chase you, enforcers circle and shoot, hulks
-- hunt the wandering humans; rescue the humans, clear the hostiles, next
-- wave.
--
-- W/A/S/D move and the arrows fire, for as long as they're held; hold two
-- for a diagonal. Holding the pointer down fires at the pointer instead.
-- P pauses. Speeds are per second, stepped by the fixed tick.

AcidStorm = AcidGame:extend("AcidStorm")

local S = AcidStorm

S.TICK_MS = 33
S.DT = S.TICK_MS / 1000
S.TITLE_BAR_H = 16
S.BG_COLOR = 0x050607     -- THEME_BG
S.TEXT_COLOR = 0xD4E6DB   -- THEME_TEXT
S.MUTED_COLOR = 0x9DAAA3  -- THEME_MUTED

S.PLASMA_CELL = 16
-- Under play the plasma is a faint backdrop, dark enough that every sprite
-- stands out against it; the title gets it brighter, but not so loud the
-- text fights it.
S.PLAY_PLASMA_PERCENT = 11
S.TITLE_PLASMA_PERCENT = 45
S.PLAYER_R = 3.5
S.PLAYER_SPEED = 78
S.BULLET_SPEED = 230
S.FIRE_EVERY = 0.11
S.EBULLET_SPEED = 90
S.WAVE_CLEAR_SECS = 2.2
S.GAME_OVER_LOCK_SECS = 0.4
S.POINTER_DEADZONE = 4

local HOSTILE = { grunt = true, enforcer = true, hulk = true }
local POINTS = { grunt = 100, enforcer = 150, hulk = 500 }

-- Each sfx gate is exactly one pass of its arpeggio (count * rate =
-- ticks * TICK_MS): an arp wraps around until note-off, so a longer gate
-- would restart the run and cut it off.
S.SFX = {
  kill   = { voice = 0, notes = { 60, 55, 1, 1 }, count = 2, rate = 33, volume = 30, ticks = 2 },
  rescue = { voice = 1, notes = { 60, 64, 67, 72 }, count = 4, rate = 33, volume = 35, ticks = 4 },
  death  = { voice = 2, notes = { 40, 37, 33, 28 }, count = 4, rate = 66, volume = 40, ticks = 8 },
}
S.FILTER_MODE_LP = 1

local function frange(a, b) return a + math.random() * (b - a) end

local function hit(a, b)
  local dx, dy, r = a.x - b.x, a.y - b.y, a.r + b.r
  return dx * dx + dy * dy <= r * r
end

local function scaled(color, percent)
  local r = (color >> 16 & 255) * percent // 100
  local g = (color >> 8 & 255) * percent // 100
  local b = (color & 255) * percent // 100
  return (r << 16) | (g << 8) | b
end

function AcidStorm:on_create()
  acid_configure_filter(200, 3, S.FILTER_MODE_LP)
  acid_configure_voice(0, 1, 2, 30, 50, 60)
  acid_configure_voice(1, 1, 2, 40, 60, 80)
  acid_configure_voice(2, 1, 8, 150, 35, 250)
  self.sfx = {}
  self.time = 0
  self.touch_held = false
  self.was_focused = false
  self:stop_input()
  self:layout()
  self:to_title()
end

-- The arena sits under the title bar and one HUD text line.
function AcidStorm:layout()
  local w, h = acid_window_size()
  local cw, ch = acid_font_size()
  self.w, self.h, self.cw, self.ch = w, h, cw, ch
  self.hud_y = S.TITLE_BAR_H + 2
  self.ax0, self.ax1 = 4, w - 4
  self.ay0, self.ay1 = self.hud_y + ch + 4, h - 4
end

function AcidStorm:to_title()
  self:stop_all_sfx()
  self.state = "title"
  self.ents = {}
  self.paused = false
  self.score = 0
  self.lives = 0
  self.wave = 0
end

function AcidStorm:start_game()
  self:stop_all_sfx()
  self.score = 0
  self.lives = 3
  self.paused = false
  self:start_wave(1)
end

-- Forget every held key (a new wave, game over, or focus gone: releases
-- may never arrive).
function AcidStorm:stop_input()
  self.held = {}
  self.move_x, self.move_y = 0, 0
  self.aim_x, self.aim_y = 0, 0
end

-- Somewhere in the arena at least `gap` from the player on some axis.
function AcidStorm:spot(margin, gap)
  local p = self.player
  while true do
    local x = frange(self.ax0 + margin, self.ax1 - margin)
    local y = frange(self.ay0 + margin, self.ay1 - margin)
    if gap == 0 or math.abs(x - p.x) >= gap or math.abs(y - p.y) >= gap then return x, y end
  end
end

function AcidStorm:add(kind, x, y, r)
  local e = { kind = kind, x = x, y = y, vx = 0, vy = 0, r = r, hp = 1, timer = 0, phase = 0 }
  self.ents[#self.ents + 1] = e
  return e
end

function AcidStorm:start_wave(wave)
  self.state = "playing"
  self.wave = wave
  self.ents = {}
  self.player = {
    x = (self.ax0 + self.ax1) / 2, y = (self.ay0 + self.ay1) / 2,
    r = S.PLAYER_R, face_x = 0, face_y = -1,
  }
  self.invuln = 1.0
  self.fire_cd = 0

  local humans = math.min(4 + wave % 3, 8)
  self.humans_alive = humans
  for _ = 1, humans do
    local h = self:add("human", frange(self.ax0 + 10, self.ax1 - 10), frange(self.ay0 + 10, self.ay1 - 10), 3)
    h.timer = frange(0.5, 2.0)
    local a = frange(0, 2 * math.pi)
    h.vx, h.vy = math.cos(a) * 14, math.sin(a) * 14
  end
  for _ = 1, math.min(2 + wave // 2, 12) do
    self:add("electrode", self:spot(12, 24)).r = 4
  end
  for _ = 1, math.min(4 + wave * 2, 26) do
    local x, y = self:spot(4, 40)
    self:add("grunt", x, y, 3.5)
  end
  if wave >= 2 then
    for _ = 1, math.min(1 + (wave - 2) // 2, 8) do
      local x, y = self:spot(10, 40)
      local e = self:add("enforcer", x, y, 3.5)
      e.timer = frange(0.5, 1.5)
      e.phase = frange(0, 2 * math.pi)
    end
  end
  if wave >= 3 then
    for _ = 1, math.min(1 + (wave - 3) // 3, 5) do
      local x, y = self:spot(10, 40)
      self:add("hulk", x, y, 5).hp = 3
    end
  end
end

function AcidStorm:clamp(e)
  e.x = math.max(self.ax0 + e.r, math.min(self.ax1 - e.r, e.x))
  e.y = math.max(self.ay0 + e.r, math.min(self.ay1 - e.r, e.y))
end

function AcidStorm:outside(e)
  return e.x < self.ax0 or e.x > self.ax1 or e.y < self.ay0 or e.y > self.ay1
end

function AcidStorm:burst(x, y, color, n)
  for _ = 1, n do
    local p = self:add("particle", x, y, 0)
    local a, sp = frange(0, 2 * math.pi), frange(20, 90)
    p.vx, p.vy = math.cos(a) * sp, math.sin(a) * sp
    p.timer = frange(0.25, 0.6)
    p.color = color
  end
end

function AcidStorm:player_hit()
  if self.invuln > 0 then return end
  local p = self.player
  self:burst(p.x, p.y, 0xFF3C3C, 24)
  self:play_sfx("death")
  self.lives = self.lives - 1
  if self.lives <= 0 then
    self.state = "game_over"
    self.state_timer = 0
    self:stop_input()
  else
    p.x, p.y = (self.ax0 + self.ax1) / 2, (self.ay0 + self.ay1) / 2
    self.invuln = 2.0
  end
end

-- ---- input ----

-- Key code -> the held action it drives.
local ACTIONS = {
  [string.byte("w")] = "up", [string.byte("W")] = "up",
  [string.byte("s")] = "down", [string.byte("S")] = "down",
  [string.byte("a")] = "left", [string.byte("A")] = "left",
  [string.byte("d")] = "right", [string.byte("D")] = "right",
  [AcidKeys.UP] = "aim_up", [AcidKeys.DOWN] = "aim_down",
  [AcidKeys.LEFT] = "aim_left", [AcidKeys.RIGHT] = "aim_right",
}

function AcidStorm:on_key(code, pressed)
  local action = ACTIONS[code]
  if action then
    -- Held state is tracked on every screen, so a key held through the
    -- title or a wave change still counts once play starts.
    self.held[action] = pressed or nil
    self:read_held()
  end
  if not pressed then return end
  local confirm = code == 32 or code == AcidKeys.ENTER
  if self.state == "title" then
    if confirm then self:start_game() end
  elseif self.state == "game_over" then
    if confirm and self.state_timer > S.GAME_OVER_LOCK_SECS then self:to_title() end
  elseif self.state == "playing" and (code == string.byte("p") or code == string.byte("P")) then
    self.paused = not self.paused
  end
end

-- Held keys -> a movement and an aim direction, each -1, 0 or 1 per axis.
function AcidStorm:read_held()
  local h = self.held
  local function axis(neg, pos) return (h[pos] and 1 or 0) - (h[neg] and 1 or 0) end
  self.move_x, self.move_y = axis("left", "right"), axis("up", "down")
  self.aim_x, self.aim_y = axis("aim_left", "aim_right"), axis("aim_up", "aim_down")
end

function AcidStorm:on_touch(x, y, pressed)
  if not pressed then
    self.touch_held = false
    self.pointer = nil
    return
  end
  -- The router repeats TOUCH every tick while held: the screens act once
  -- per press, aiming follows the pointer for as long as it's down.
  local fresh = not self.touch_held
  self.touch_held = true
  if self.state == "playing" then
    self.pointer = { x = x, y = y }
  elseif fresh and self.state == "title" then
    self:start_game()
  elseif fresh and self.state == "game_over" and self.state_timer > S.GAME_OVER_LOCK_SECS then
    self:to_title()
  end
end

-- ---- update ----

function AcidStorm:update_player(dt)
  local p = self.player
  local mx, my = self.move_x, self.move_y
  if mx ~= 0 and my ~= 0 then mx, my = mx * 0.7071, my * 0.7071 end
  p.x = p.x + mx * S.PLAYER_SPEED * dt
  p.y = p.y + my * S.PLAYER_SPEED * dt
  self:clamp(p)

  local ax, ay = self.aim_x, self.aim_y
  if self.pointer then
    local dx, dy = self.pointer.x - p.x, self.pointer.y - p.y
    local d = math.sqrt(dx * dx + dy * dy)
    if d > S.POINTER_DEADZONE then ax, ay = dx / d, dy / d end
  end
  if ax ~= 0 or ay ~= 0 then
    local len = math.sqrt(ax * ax + ay * ay)
    p.face_x, p.face_y = ax / len, ay / len
    self.fire_cd = self.fire_cd - dt
    if self.fire_cd <= 0 then
      local b = self:add("pbullet", p.x + p.face_x * (p.r + 2), p.y + p.face_y * (p.r + 2), 1.5)
      b.vx, b.vy = p.face_x * S.BULLET_SPEED, p.face_y * S.BULLET_SPEED
      b.timer = 1.5
      self.fire_cd = S.FIRE_EVERY
    end
  elseif mx ~= 0 or my ~= 0 then
    local len = math.sqrt(mx * mx + my * my)
    p.face_x, p.face_y = mx / len, my / len
  end
end

local function steer(e, tx, ty, speed, dt)
  local dx, dy = tx - e.x, ty - e.y
  local d = math.sqrt(dx * dx + dy * dy)
  if d > 0.001 then
    e.x = e.x + dx / d * speed * dt
    e.y = e.y + dy / d * speed * dt
  end
  return dx, dy, d
end

function AcidStorm:update_ent(e, dt)
  local p = self.player
  local k = e.kind
  if k == "pbullet" or k == "ebullet" then
    e.x, e.y = e.x + e.vx * dt, e.y + e.vy * dt
    e.timer = e.timer - dt
    if e.timer <= 0 or self:outside(e) then e.dead = true
    elseif k == "ebullet" and hit(e, p) then e.dead = true; self:player_hit() end
  elseif k == "grunt" then
    steer(e, p.x, p.y, math.min(32 + self.wave * 1.2, 60), dt)
    if hit(e, p) then self:player_hit() end
  elseif k == "enforcer" then
    e.phase = e.phase + dt * 3
    steer(e, p.x + math.cos(e.phase) * 40, p.y + math.sin(e.phase * 1.3) * 40, 26, dt)
    self:clamp(e)
    e.timer = e.timer - dt
    local dx, dy = p.x - e.x, p.y - e.y
    local d = math.sqrt(dx * dx + dy * dy)
    if e.timer <= 0 and d > 0.001 then
      local b = self:add("ebullet", e.x, e.y, 1.5)
      b.vx, b.vy = dx / d * S.EBULLET_SPEED, dy / d * S.EBULLET_SPEED
      b.timer = 3.0
      e.timer = frange(1.0, 1.8)
    end
    if hit(e, p) then self:player_hit() end
  elseif k == "hulk" then
    local target, best = nil, math.huge
    for _, h in ipairs(self.ents) do
      if h.kind == "human" and not h.dead then
        local dx, dy = h.x - e.x, h.y - e.y
        if dx * dx + dy * dy < best then best, target = dx * dx + dy * dy, h end
      end
    end
    steer(e, target and target.x or p.x, target and target.y or p.y, 16, dt)
    if target and hit(e, target) then
      target.dead = true
      self.humans_alive = self.humans_alive - 1
      self:burst(target.x, target.y, 0xFFFFFF, 10)
    end
    if hit(e, p) then self:player_hit() end
  elseif k == "human" then
    e.timer = e.timer - dt
    if e.timer <= 0 then
      local a = frange(0, 2 * math.pi)
      e.vx, e.vy = math.cos(a) * 14, math.sin(a) * 14
      e.timer = frange(0.6, 2.2)
    end
    e.x, e.y = e.x + e.vx * dt, e.y + e.vy * dt
    self:clamp(e)
    if hit(p, e) then
      e.dead = true
      self.humans_alive = self.humans_alive - 1
      self.score = self.score + 1000
      self:burst(e.x, e.y, 0xFFFF78, 14)
      self:play_sfx("rescue")
    end
  elseif k == "electrode" then
    if hit(e, p) then self:player_hit() end
  elseif k == "particle" then
    e.x, e.y = e.x + e.vx * dt, e.y + e.vy * dt
    e.vx, e.vy = e.vx * 0.92, e.vy * 0.92
    e.timer = e.timer - dt
    if e.timer <= 0 or self:outside(e) then e.dead = true end
  end
end

function AcidStorm:shoot_hostiles()
  for _, b in ipairs(self.ents) do
    if b.kind == "pbullet" and not b.dead then
      for _, e in ipairs(self.ents) do
        if HOSTILE[e.kind] and not e.dead and hit(b, e) then
          b.dead = true
          e.hp = e.hp - 1
          if e.hp <= 0 then
            e.dead = true
            self.score = self.score + POINTS[e.kind]
            self:burst(e.x, e.y, e.kind == "hulk" and 0xFFA028 or 0x50FFDC, 16)
            self:play_sfx("kill")
          else
            self:burst(b.x, b.y, 0xFFFFFF, 4)
          end
          break
        end
      end
    end
  end
end

function AcidStorm:sweep()
  local live = {}
  for _, e in ipairs(self.ents) do
    if not e.dead then live[#live + 1] = e end
  end
  self.ents = live
end

function AcidStorm:hostiles()
  local n = 0
  for _, e in ipairs(self.ents) do
    if HOSTILE[e.kind] then n = n + 1 end
  end
  return n
end

function AcidStorm:step(dt)
  if self.invuln > 0 then self.invuln = self.invuln - dt end
  self:update_player(dt)
  -- Only what was there at the start of the step moves this step; things
  -- spawned during it (bullets, bursts) start next step.
  local n = #self.ents
  for i = 1, n do
    local e = self.ents[i]
    if not e.dead then self:update_ent(e, dt) end
    if self.state ~= "playing" then break end
  end
  self:shoot_hostiles()
  self:sweep()
  if self.state == "playing" and self:hostiles() == 0 then
    self.bonus = 100 * self.wave + 50 * self.humans_alive
    self.score = self.score + self.bonus
    self.state = "wave_clear"
    self.state_timer = 0
    self:play_sfx("rescue")
  end
end

function AcidStorm:on_tick()
  local dt = S.DT
  local focused = self:focused()
  -- Unfocused, keys and touches go elsewhere: hold the game still, and
  -- drop a pointer whose release we may never see.
  if not focused then
    self.pointer = nil
    if self.was_focused then self:stop_input() end
  end
  if focused then
    self.time = self.time + dt
    if self.state == "playing" and not self.paused then
      self:step(dt)
    elseif self.state == "wave_clear" then
      self.state_timer = self.state_timer + dt
      if self.state_timer > S.WAVE_CLEAR_SECS then self:start_wave(self.wave + 1) end
    elseif self.state == "game_over" then
      self.state_timer = self.state_timer + dt
      -- Particles from the last hit keep flying out.
      for _, e in ipairs(self.ents) do
        if e.kind == "particle" then self:update_ent(e, dt) end
      end
      self:sweep()
    end
  end
  self:tick_sfx()
  self.was_focused = focused
  if focused then self:redraw() end
end

-- ---- sound ----

function AcidStorm:play_sfx(name)
  local s = S.SFX[name]
  -- One gate per voice: a retrigger replaces the earlier one, so its
  -- note-off can't cut the new note short.
  for i = #self.sfx, 1, -1 do
    if self.sfx[i].voice == s.voice then table.remove(self.sfx, i) end
  end
  acid_play_note(s.voice, s.notes[1], s.volume)
  acid_trigger_arp(s.voice, s.notes[1], s.notes[2], s.notes[3], s.notes[4], s.count, s.rate)
  self.sfx[#self.sfx + 1] = { voice = s.voice, ticks = s.ticks }
end

function AcidStorm:tick_sfx()
  for i = #self.sfx, 1, -1 do
    local s = self.sfx[i]
    s.ticks = s.ticks - 1
    if s.ticks <= 0 then
      acid_stop_note(s.voice)
      table.remove(self.sfx, i)
    end
  end
end

function AcidStorm:stop_all_sfx()
  if not self.sfx then return end
  for _, s in ipairs(self.sfx) do acid_stop_note(s.voice) end
  self.sfx = {}
end

function AcidStorm:on_destroy()
  self:stop_all_sfx()
end

-- ---- drawing ----

-- Coarse plasma: three sine waves summed per cell, onto the hue wheel.
-- Dimmed to a quarter under play so the sprites read.
function AcidStorm:draw_plasma(percent)
  local t = self.time * (self.state == "title" and 1.0 or 0.25)
  local cell = S.PLASMA_CELL
  local y = S.TITLE_BAR_H
  while y < self.h do
    local ch = math.min(cell, self.h - y)
    local x = 0
    while x < self.w do
      local cw = math.min(cell, self.w - x)
      local v = math.sin(x * 0.045 + t) + math.sin(y * 0.06 - t * 1.3) + math.sin((x + y) * 0.03 + t * 0.7)
      local step = math.floor((v + 3) * 42.6 + t * 20)
      acid_fill_rect(x, y, cw, ch, scaled(AcidPalette.hue(step), percent))
      x = x + cell
    end
    y = y + cell
  end
end

function AcidStorm:text(s, x, y, color)
  acid_draw_text(s, x, y, color, S.BG_COLOR)
end

function AcidStorm:centred(s, y, color)
  self:text(s, (self.w - #s * self.cw) // 2, y, color)
end

local function box(x, y, w, h, c)
  acid_fill_rect(x, y, w, 1, c)
  acid_fill_rect(x, y + h - 1, w, 1, c)
  acid_fill_rect(x, y, 1, h, c)
  acid_fill_rect(x + w - 1, y, 1, h, c)
end

function AcidStorm:draw_hud()
  self:text(string.format("SCORE %06d", self.score), 4, self.hud_y, 0x00FFDC)
  local right = string.format("W%02d L%d", self.wave, math.max(self.lives, 0))
  self:text(right, self.w - 4 - #right * self.cw, self.hud_y, 0xFF3CC8)
end

-- Sprites, 1 px per cell, two frames where they walk. Original pixel
-- art: grunts are little robots, enforcers hovering drones with one eye,
-- hulks big brutes, humans little people.
local SPRITES = {
  grunt = {
    { ".XXXXX.", ".X.X.X.", ".XXXXX.", "XXXXXXX", "X.XXX.X", "..X.X..", ".XX..X." },
    { ".XXXXX.", ".X.X.X.", ".XXXXX.", "XXXXXXX", "X.XXX.X", "..X.X..", ".X..XX." },
  },
  enforcer = {
    { "..XXXXX..", ".XX...XX.", "XXX.X.XXX", ".XX...XX.", "..XXXXX..", ".X..X..X." },
    { "..XXXXX..", ".XX...XX.", "XXX.X.XXX", ".XX...XX.", "..XXXXX..", "X...X...X" },
  },
  hulk = {
    { "...XXXXX...", "..XXXXXXX..", "..X..X..X..", "..XXXXXXX..", "XXXXXXXXXXX", "XXXXXXXXXXX",
      "XX.XXXXX.XX", "XX.XXXXX.XX", "...XX.XX...", "...XX.XX...", "..XXX.XXX.." },
  },
  human = {
    { "..X..", ".XXX.", "X.X.X", "..X..", ".X.X.", ".X.X." },
    { "..X..", ".XXX.", "X.X.X", "..X..", ".X.X.", "X...X" },
  },
}

-- Each sprite's filled cells merged into horizontal runs, one rect each.
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
  return { w = #pattern[1], h = #pattern, runs = out }
end

local SPRITE_RUNS = {}
for kind, frames in pairs(SPRITES) do
  SPRITE_RUNS[kind] = {}
  for f, pattern in ipairs(frames) do SPRITE_RUNS[kind][f] = runs(pattern) end
end

-- Draws a kind's sprite centred on (x, y), walking frame by `step`.
local function draw_sprite(kind, x, y, step, color)
  local frames = SPRITE_RUNS[kind]
  local spr = frames[step % #frames + 1]
  local x0, y0 = x - spr.w // 2, y - spr.h // 2
  for _, r in ipairs(spr.runs) do
    acid_fill_rect(x0 + r[1], y0 + r[2], r[3], 1, color)
  end
end

-- Enemy eyes and details over the body colour.
S.GRUNT_EYE = 0xFFFFFF
S.ENFORCER_COLOR = 0xFFD200
S.ENFORCER_EYE = 0xFF2850
S.HULK_COLOR = 0x46DC3C
S.HULK_EYE = 0xFF2828
S.HUMAN_COLOR = 0xFFE6C8

function AcidStorm:draw_ent(e, i)
  local x, y, k = math.floor(e.x), math.floor(e.y), e.kind
  local step = math.floor(self.time * 6) + i
  if k == "pbullet" then
    acid_fill_rect(x - 1, y - 1, 2, 2, 0xFFFFFF)
  elseif k == "ebullet" then
    acid_fill_rect(x - 1, y - 1, 2, 2, 0xFF5A28)
  elseif k == "grunt" then
    -- Warm hues only (red through magenta), drifting per robot.
    draw_sprite("grunt", x, y, step, AcidPalette.hue(200 + (math.floor(self.time * 40) + i * 9) % 70))
    -- In the two gaps of the face row.
    acid_fill_rect(x - 1, y - 2, 1, 1, S.GRUNT_EYE)
    acid_fill_rect(x + 1, y - 2, 1, 1, S.GRUNT_EYE)
  elseif k == "enforcer" then
    draw_sprite("enforcer", x, y, step, S.ENFORCER_COLOR)
    acid_fill_rect(x, y - 1, 1, 1, S.ENFORCER_EYE)
  elseif k == "hulk" then
    draw_sprite("hulk", x, y, 0, S.HULK_COLOR)
    acid_fill_rect(x - 2, y - 3, 2, 1, S.HULK_EYE)
    acid_fill_rect(x + 1, y - 3, 2, 1, S.HULK_EYE)
  elseif k == "human" then
    draw_sprite("human", x, y, step, S.HUMAN_COLOR)
  elseif k == "electrode" then
    local c = math.floor(self.time * 6) % 2 == 1 and 0xFFFF00 or 0xC800FF
    acid_fill_circle(x, y, 3, c)
    acid_draw_line(x - 4, y, x + 4, y, c)
    acid_draw_line(x, y - 4, x, y + 4, c)
  elseif k == "particle" then
    local r = e.timer > 0.4 and 2 or 1
    acid_fill_rect(x, y, r, r, e.color)
  end
end

function AcidStorm:draw_player()
  if self.invuln > 0 and math.floor(self.invuln * 12) % 2 == 0 then return end
  local p = self.player
  local x, y = math.floor(p.x), math.floor(p.y)
  acid_fill_circle(x, y, 4, 0xFFFFFF)
  acid_fill_circle(x, y, 3, 0x00FFB4)
  local bx = math.floor(p.x + p.face_x * (p.r + 3))
  local by = math.floor(p.y + p.face_y * (p.r + 3))
  acid_fill_rect(bx - 1, by - 1, 2, 2, 0xFFFFFF)
end

function AcidStorm:draw_title()
  local ch = self.ch
  local mid = S.TITLE_BAR_H + (self.h - S.TITLE_BAR_H) // 2
  self:centred("ACIDSTORM", mid - ch * 4, AcidPalette.hue(math.floor(self.time * 120)))
  self:centred("AN ACID ARENA SHOOTER", mid - ch * 2, S.TEXT_COLOR)
  if math.floor(self.time * 2) % 2 == 0 then
    self:centred("SPACE OR TAP TO START", mid, 0x00FFFF)
  end
  self:centred("HOLD WASD TO MOVE", mid + ch * 2, S.MUTED_COLOR)
  self:centred("HOLD ARROWS TO FIRE", mid + ch * 3 + 2, S.MUTED_COLOR)
  self:centred("OR HOLD POINTER  P PAUSE", mid + ch * 4 + 4, S.MUTED_COLOR)
end

-- One frame, shown whole: the compositor never catches the window cleared
-- or half-painted.
function AcidStorm:redraw()
  acid_begin_frame()
  self:paint()
  acid_end_frame()
end

function AcidStorm:paint()
  acid_clear_user_area()
  acid_draw_window_frame(self:window_title())
  if self.state == "title" then
    self:draw_plasma(S.TITLE_PLASMA_PERCENT)
    self:draw_title()
    acid_draw_window_border()
    return
  end
  self:draw_plasma(S.PLAY_PLASMA_PERCENT)
  box(self.ax0 - 2, self.ay0 - 2, self.ax1 - self.ax0 + 4, self.ay1 - self.ay0 + 4,
    AcidPalette.hue(math.floor(self.time * 40)))
  for i, e in ipairs(self.ents) do self:draw_ent(e, i) end
  if self.state ~= "game_over" then self:draw_player() end
  self:draw_hud()
  local mid = (self.ay0 + self.ay1) // 2
  if self.state == "wave_clear" then
    self:centred(string.format("WAVE %02d CLEAR", self.wave), mid - self.ch, 0x00FFB4)
    self:centred("BONUS " .. self.bonus, mid + 2, 0xFFE600)
  elseif self.state == "game_over" then
    self:centred("GAME OVER", mid - self.ch * 2, 0xFF285A)
    self:centred(string.format("FINAL SCORE %06d", self.score), mid, S.TEXT_COLOR)
    if math.floor(self.time * 2) % 2 == 0 then self:centred("PRESS SPACE", mid + self.ch * 2, 0x00FFFF) end
  elseif self.paused then
    self:centred("PAUSED", mid, S.TEXT_COLOR)
  end
  acid_draw_window_border()
end

AcidStorm:new():start()
