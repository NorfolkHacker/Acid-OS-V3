AcidBlaster = AcidGame:extend("AcidBlaster")

-- Constants are AcidBlaster.X fields.

local A = AcidBlaster

-- Must match the spawn call for this app and the title bar height --
-- no generic "ask my own window size" binding exists.
A.WINDOW_W = 250
A.WINDOW_H = 180
A.TITLE_BAR_H = 16
A.PLAY_H = A.WINDOW_H - A.TITLE_BAR_H
A.CENTER_X = A.WINDOW_W // 2
A.CENTER_Y = A.TITLE_BAR_H + A.PLAY_H // 2

A.ENEMY_R = 8
A.TAP_TOLERANCE = 6

A.BG_COLOR = 0x050607     -- THEME_BG
A.ENEMY_COLOR = 0x00FF66  -- THEME_HARD
A.TEXT_COLOR = 0xD4E6DB   -- THEME_TEXT

-- Every enemy used to be the one THEME_HARD green, which is exactly the
-- "everything looks like the same few shades of green" that AcidPalette
-- was written for (it names this game in its own comment). Most stay
-- green so the play field still reads as this app's, but one enemy in
-- ENEMY_ALT_CHANCE spawns on a random point of the full hue wheel --
-- occasional, not a rainbow swarm. Purely cosmetic: the colour rides
-- along in the enemy table and only ever reaches draw_alien, so nothing
-- about spawning, movement, tapping or scoring reads it.
A.ENEMY_ALT_CHANCE = 5

-- A few fixed stars behind the play field, so the background is a night
-- sky the enemies fly across instead of flat black. Deliberately dim
-- (well below TEXT/ENEMY brightness) and 1px, so they never compete
-- with an enemy for the player's eye or get mistaken for a tiny one.
-- Kept clear of the SCORE line's 8px-tall text at the top, which draws
-- its own background and would otherwise fight with any star under it.
-- STAR_MARGIN keeps every star clear of the 1px window outline AND of
-- the 3px rounded corners behind it -- the stars repaint after the
-- border on every incremental frame, so one placed on the edge would sit
-- on top of the outline, or outside the rounded corner entirely, for the
-- whole game.
A.STAR_COUNT = 14
A.STAR_MARGIN = 4
A.STAR_TOP = A.TITLE_BAR_H + 12
A.STAR_COLORS = { 0x1E2B26, 0x1E2B26, 0x35514A, 0x6B8F84 }

-- A small pixel-art alien instead of a plain filled circle -- an
-- original silhouette (not a copy of any specific game's own alien
-- glyph), drawn as a grid of small squares to match this OS's existing
-- blocky look (Tetris/Breakout draw the same way) rather than
-- introducing a new rendering style just for this one enemy. Each row
-- is one string, 'X' a filled cell, '.' empty; rows must all be the
-- same length. Collision/spawn/despawn logic is untouched -- ENEMY_R is
-- still the physics radius, this only changes what gets drawn at
-- (e.x, e.y).
A.ALIEN_PATTERN = {
  ".XXXX.",
  "XXXXXX",
  "XX..XX",
  "XXXXXX",
  ".X..X.",
  "X....X",
}
A.ALIEN_CELL = 2
A.ALIEN_W = #A.ALIEN_PATTERN[1] * A.ALIEN_CELL
A.ALIEN_H = #A.ALIEN_PATTERN * A.ALIEN_CELL

-- The center every enemy is actually flying toward had no visual marker
-- at all before -- a new player has no way to know what they're
-- defending. A concentric ring-and-dot bullseye (three acid_fill_circle
-- calls: accent ring, background gap, accent center) makes the target
-- obvious without needing a new drawing primitive.
A.TARGET_R1 = 11
A.TARGET_R2 = 7
A.TARGET_R3 = 3

-- Each SFX steps through a short note sequence (the synth's own
-- arpeggiator -- see acid_trigger_arp) instead of holding one flat
-- pitch, so a hit sounds like a quick two-tone zap and game-over like an
-- actual descending run, not a plain beep. Unused slots beyond each
-- ARP_COUNT are never read -- 1 is just a valid placeholder (see
-- acid_trigger_arp's own comment).
A.HIT_VOICE = 0
A.HIT_ONA = 55
A.HIT_NOTES = { A.HIT_ONA, A.HIT_ONA - 4, 1, 1 }
A.HIT_ARP_COUNT = 2
A.HIT_ARP_RATE_MS = 18
A.HIT_VOLUME = 35
A.HIT_TICKS = 3
A.OVER_VOICE = 1
A.OVER_ONA = 25
A.OVER_NOTES = { A.OVER_ONA, A.OVER_ONA - 3, A.OVER_ONA - 7, A.OVER_ONA - 12 }
A.OVER_ARP_COUNT = 4
-- The gate (OVER_TICKS * TICK_MS) is deliberately exactly one pass of
-- this arpeggio (OVER_ARP_COUNT * OVER_ARP_RATE_MS = 4 * 75 = 300ms =
-- 6 * 50ms). An arp cycles up-only-WITH-WRAPAROUND until the note is
-- stopped, so a longer gate doesn't hold the last note -- it restarts
-- the descending run and then chops it off mid-way, which is what made
-- game-over drag on past its own ending.
A.OVER_ARP_RATE_MS = 75
A.OVER_VOLUME = 40
A.OVER_TICKS = 6

-- By default every synth voice is an unfiltered pulse wave with an
-- instant attack and instant release -- a hard digital click on every
-- hit, harsh even at a short duration. Routing both SFX voices through
-- the shared filter and giving them a real (if brief) envelope turns
-- that click into a short, filtered "acid" blip instead. Configured
-- once here, not per-hit in trigger_sfx -- these are voice-wide
-- settings, not per-note.
A.FILTER_MODE_LP = 1

-- Round half away from zero: take the floor (ceil for negatives), then
-- step away from zero when the fraction is >= 0.5 (2.5 -> 3, -2.5 -> -3).
-- Returns an integer.
local function round_half_away(v)
  if v >= 0 then
    local d = math.floor(v)
    return math.tointeger(d + ((v - d >= 0.5) and 1 or 0))
  end
  local d = math.ceil(v)
  return math.tointeger(d - ((d - v >= 0.5) and 1 or 0))
end

-- The sign of an integer n: -1, 0 or 1.
local function sign(n)
  if n > 0 then return 1 end
  if n < 0 then return -1 end
  return 0
end

function AcidBlaster:on_create()
  acid_configure_filter(180, 3, A.FILTER_MODE_LP)
  acid_configure_voice(A.HIT_VOICE, 1, 3, 40, 55, 70)
  acid_configure_voice(A.OVER_VOICE, 1, 8, 150, 35, 250)
  self:reset_game()
end

function AcidBlaster:reset_game()
  self.score = 0
  self.enemies = {}
  self.spawn_timer = 0
  self.game_over = false
  -- Silence anything still gated before dropping the bookkeeping that
  -- would have silenced it. A restart tap arrives from on_touch while
  -- the game-over sting is usually still playing (the player is
  -- mid-tapping when they lose), and tick_sfx is the ONLY thing that
  -- ever sends a note-off -- clearing self.sfx without this leaves the
  -- voice with no note-off coming, so its arpeggio cycles and its
  -- envelope sustains forever.
  self:stop_all_sfx()
  self.sfx = {}
  -- Dirty-tracking state for draw() -- see its own comment for why this
  -- exists. needs_frame starts true so the very first draw does a full
  -- clear+chrome paint.
  self.stars = self:build_stars()
  self.drawn_enemies = {}
  self.drawn_score = nil
  self.drawn_game_over = false
  self.needs_frame = true
  self.was_focused = false
end

function AcidBlaster:trigger_sfx(voice, notes, arp_count, arp_rate_ms, volume, ticks)
  acid_play_note(voice, notes[1], volume)
  acid_trigger_arp(voice, notes[1], notes[2], notes[3], notes[4], arp_count, arp_rate_ms)
  self.sfx[#self.sfx + 1] = { voice = voice, ticks = ticks }
end

-- A fresh sky per round. Anything inside the target's outer ring is
-- skipped rather than drawn-then-covered, since draw_target paints over
-- that circle every frame anyway.
function AcidBlaster:build_stars()
  local stars = {}
  while #stars < A.STAR_COUNT do
    local x = A.STAR_MARGIN + math.random(0, A.WINDOW_W - 2 * A.STAR_MARGIN - 1)
    local y = A.STAR_TOP + math.random(0, A.WINDOW_H - A.STAR_MARGIN - A.STAR_TOP - 1)
    local ddx = x - A.CENTER_X
    local ddy = y - A.CENTER_Y
    if (ddx * ddx + ddy * ddy) > (A.TARGET_R1 * A.TARGET_R1) then
      stars[#stars + 1] = { x = x, y = y, color = A.STAR_COLORS[math.random(1, #A.STAR_COLORS)] }
    end
  end
  return stars
end

-- Safe to call before the first reset_game, when self.sfx is still nil.
function AcidBlaster:stop_all_sfx()
  if not self.sfx then return end
  for _, s in ipairs(self.sfx) do acid_stop_note(s.voice) end
  self.sfx = {}
end

function AcidBlaster:tick_sfx()
  local i = #self.sfx - 1
  while i >= 0 do
    local s = self.sfx[i + 1]
    s.ticks = s.ticks - 1
    if s.ticks <= 0 then
      acid_stop_note(s.voice)
      table.remove(self.sfx, i + 1)
    end
    i = i - 1
  end
end

function AcidBlaster:spawn_interval()
  local v = 24 - self.score // 2
  if v < 6 then return 6 end
  return v
end

function AcidBlaster:enemy_speed()
  local v = 2 + self.score // 5
  if v > 8 then return 8 end
  return v
end

function AcidBlaster:spawn_enemy()
  local edge = math.random(0, 3)
  local x, y
  if edge == 0 then
    x = A.ENEMY_R + math.random(0, A.WINDOW_W - 2 * A.ENEMY_R - 1)
    y = A.TITLE_BAR_H + A.ENEMY_R
  elseif edge == 1 then
    x = A.WINDOW_W - 1 - A.ENEMY_R
    y = A.TITLE_BAR_H + A.ENEMY_R + math.random(0, A.PLAY_H - 2 * A.ENEMY_R - 1)
  elseif edge == 2 then
    x = A.ENEMY_R + math.random(0, A.WINDOW_W - 2 * A.ENEMY_R - 1)
    y = A.TITLE_BAR_H + A.PLAY_H - 1 - A.ENEMY_R
  else
    x = A.ENEMY_R
    y = A.TITLE_BAR_H + A.ENEMY_R + math.random(0, A.PLAY_H - 2 * A.ENEMY_R - 1)
  end

  local dx = A.CENTER_X - x
  local dy = A.CENTER_Y - y
  local dist = math.sqrt(dx * dx + dy * dy + 0.0)
  if dist < 1.0 then dist = 1.0 end
  local speed = self:enemy_speed()
  local vx = round_half_away(dx * speed / dist)
  local vy = round_half_away(dy * speed / dist)
  -- Belt-and-braces: with speed >= 2 this cannot actually happen (see
  -- the design spec's note), but a stuck enemy would be a silent, very
  -- confusing bug if it ever did, so guard it anyway.
  if vx == 0 and vy == 0 then
    vx = sign(dx)
    vy = sign(dy)
  end

  local color
  if math.random(0, A.ENEMY_ALT_CHANCE - 1) == 0 then
    color = AcidPalette.hue(math.random(0, 255))
  else
    color = A.ENEMY_COLOR
  end
  self.enemies[#self.enemies + 1] = { x = x, y = y, dx = vx, dy = vy, color = color }
end

-- Returns true if any enemy reached the center this tick. Also removes
-- any enemy that has drifted off the play field without reaching the
-- center (an integer-rounded spawn direction can miss the center by
-- more than ENEMY_R -- see the final review's finding -- so nothing
-- would otherwise ever remove it, and it would fly off forever).
function AcidBlaster:update_enemies()
  local hit_center = false
  local i = #self.enemies - 1
  while i >= 0 do
    local e = self.enemies[i + 1]
    e.x = e.x + e.dx
    e.y = e.y + e.dy
    local ddx = e.x - A.CENTER_X
    local ddy = e.y - A.CENTER_Y
    if (ddx * ddx + ddy * ddy) <= (A.ENEMY_R * A.ENEMY_R) then
      hit_center = true
    elseif e.x - A.ENEMY_R < 0 or e.x + A.ENEMY_R > A.WINDOW_W - 1 or
           e.y - A.ENEMY_R < A.TITLE_BAR_H or e.y + A.ENEMY_R > A.TITLE_BAR_H + A.PLAY_H - 1 then
      table.remove(self.enemies, i + 1)
    end
    i = i - 1
  end
  return hit_center
end

-- Returns true if a tap at (x, y) destroyed an enemy.
function AcidBlaster:check_tap(x, y)
  local hit_index = nil
  local i = 0
  while i < #self.enemies do
    local e = self.enemies[i + 1]
    local ddx = x - e.x
    local ddy = y - e.y
    local limit = A.ENEMY_R + A.TAP_TOLERANCE
    if (ddx * ddx + ddy * ddy) <= (limit * limit) then
      hit_index = i
      break
    end
    i = i + 1
  end
  if not hit_index then return false end
  table.remove(self.enemies, hit_index + 1)
  self.score = self.score + 1
  self:trigger_sfx(A.HIT_VOICE, A.HIT_NOTES, A.HIT_ARP_COUNT, A.HIT_ARP_RATE_MS, A.HIT_VOLUME, A.HIT_TICKS)
  return true
end

function AcidBlaster:on_touch(x, y, pressed)
  -- The router repeats TOUCH every tick while held. A shot, or the restart
  -- at game over, needs a fresh press: a held press fires once, and one
  -- held through game over doesn't restart instantly.
  if not pressed then
    self.touch_held = false
    return
  end
  if self.touch_held then return end
  self.touch_held = true
  if self.game_over then
    self:reset_game()
    return
  end
  self:check_tap(x, y)
end

function AcidBlaster:on_tick()
  if not self.game_over then
    self.spawn_timer = self.spawn_timer - 1
    if self.spawn_timer <= 0 then
      self:spawn_enemy()
      self.spawn_timer = self:spawn_interval()
    end
    if self:update_enemies() then
      self.game_over = true
      self:trigger_sfx(A.OVER_VOICE, A.OVER_NOTES, A.OVER_ARP_COUNT, A.OVER_ARP_RATE_MS, A.OVER_VOLUME, A.OVER_TICKS)
    end
  end
  self:tick_sfx()
  -- Game state (enemy positions, spawns, game-over) keeps advancing
  -- every tick regardless -- only the DRAWING is skipped while covered.
  -- draw() has no z-order awareness at all (it paints straight onto the
  -- shared framebuffer every tick, unlike a static window's redraw,
  -- which only ever runs inside the compositor's own z-order-aware
  -- repaint) -- if it kept drawing while genuinely behind another
  -- window, it would paint over that window's visible content on every
  -- single tick. Skipping it here means the window some other app has
  -- on top stays correctly on top.
  local is_focused = self:focused()
  -- Something else painted over our window's pixels for however long we
  -- were unfocused, so the incremental erase-old/draw-new below (which
  -- assumes the framebuffer still shows what we last drew) is no longer
  -- valid -- force one full clear+chrome repaint on the tick we regain
  -- focus, same as the very first draw.
  if is_focused and not self.was_focused then self.needs_frame = true end
  self.was_focused = is_focused
  if is_focused then self:draw() end
end

-- Redraws only what actually changed since the last draw call, instead
-- of a full acid_clear_user_area + window-frame + border every single
-- tick (20/sec while focused). That full-window clear was visible as a
-- flicker (a black flash the moment before enemies were redrawn, with
-- no double buffering to hide it) and was slow on the software renderer
-- (a ~250x164px fill 20 times a second, for a scene that's mostly a
-- handful of small circles). Erasing just the previously-drawn enemy
-- positions and redrawing just the current ones touches a tiny fraction
-- of that area.
function AcidBlaster:draw()
  if self.needs_frame then
    acid_clear_user_area()
    acid_draw_window_frame(self:window_title())
    acid_draw_window_border()
    -- No draw_stars/draw_target here: both branches below repaint the
    -- background unconditionally (drawn_game_over was just reset, so
    -- the game-over branch runs too), and painting it twice on the
    -- same frame is the slowest frame doing the most redundant work.
    self.drawn_enemies = {}
    self.drawn_score = nil
    self.drawn_game_over = false
    self.needs_frame = false
  end

  if self.game_over then
    if not self.drawn_game_over then
      self:erase_drawn_enemies()
      -- That erase leaves BG-coloured patches over whatever background
      -- the last aliens were standing on, and nothing repaints during
      -- the game-over screen -- so restore the background it swallowed
      -- before the text goes down, or the sky loses stars (and the
      -- target loses bites) for as long as the screen is up.
      self:draw_stars()
      self:draw_target()
      self:draw_game_over()
      self.drawn_game_over = true
    end
    return
  end

  self:erase_drawn_enemies()
  -- Enemies fly straight at the target by design, so erasing one's old
  -- position (a square BG-colored patch, not a circle) routinely nicks
  -- a chunk out of the round target underneath it -- redrawing the
  -- target here, after every erase and before any alien is drawn fresh
  -- this tick, fixes any such nick every frame instead of leaving a
  -- permanent bite out of it (the same erase-overwrites-something-else
  -- bug class as breakout's side-border fix, different shape).
  -- The stars are background too, and an erase patch swallows any star
  -- it covers, so they get repainted here for exactly the same reason
  -- -- before the target, which wins wherever the two would overlap.
  self:draw_stars()
  self:draw_target()
  for _, e in ipairs(self.enemies) do self:draw_alien(e.x, e.y, e.color) end
  self.drawn_enemies = {}
  for _, e in ipairs(self.enemies) do
    self.drawn_enemies[#self.drawn_enemies + 1] = { x = e.x, y = e.y }
  end

  if self.drawn_score == self.score then return end
  acid_draw_text("SCORE: " .. self.score, 4, A.TITLE_BAR_H + 2, A.TEXT_COLOR, A.BG_COLOR)
  self.drawn_score = self.score
end

function AcidBlaster:draw_stars()
  for _, s in ipairs(self.stars) do acid_fill_rect(s.x, s.y, 1, 1, s.color) end
end

function AcidBlaster:draw_target()
  acid_fill_circle(A.CENTER_X, A.CENTER_Y, A.TARGET_R1, A.ENEMY_COLOR)
  acid_fill_circle(A.CENTER_X, A.CENTER_Y, A.TARGET_R2, A.BG_COLOR)
  acid_fill_circle(A.CENTER_X, A.CENTER_Y, A.TARGET_R3, A.ENEMY_COLOR)
end

function AcidBlaster:draw_alien(cx, cy, color)
  local x0 = cx - A.ALIEN_W // 2
  local y0 = cy - A.ALIEN_H // 2
  local row = 0
  while row < #A.ALIEN_PATTERN do
    local line = A.ALIEN_PATTERN[row + 1]
    local col = 0
    while col < #line do
      if line:sub(col + 1, col + 1) == "X" then
        acid_fill_rect(x0 + col * A.ALIEN_CELL, y0 + row * A.ALIEN_CELL, A.ALIEN_CELL, A.ALIEN_CELL, color)
      end
      col = col + 1
    end
    row = row + 1
  end
end

function AcidBlaster:erase_drawn_enemies()
  for _, d in ipairs(self.drawn_enemies) do self:draw_alien(d.x, d.y, A.BG_COLOR) end
  self.drawn_enemies = {}
end

function AcidBlaster:draw_game_over()
  acid_draw_text("GAME OVER", A.CENTER_X - 36, A.CENTER_Y - 10, A.TEXT_COLOR, A.BG_COLOR)
  acid_draw_text("SCORE: " .. self.score, A.CENTER_X - 30, A.CENTER_Y + 6, A.TEXT_COLOR, A.BG_COLOR)
end

AcidBlaster:new():start()
