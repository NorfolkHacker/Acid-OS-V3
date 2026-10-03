Breakout = AcidGame:extend("Breakout")

-- Constants are Breakout.X fields.

Breakout.WINDOW_W = 200
Breakout.WINDOW_H = 160
Breakout.TITLE_BAR_H = 16

Breakout.BG_COLOR = 0x050607      -- THEME_BG
Breakout.TEXT_COLOR = 0xD4E6DB    -- THEME_TEXT

Breakout.BRICK_COLS = 8
Breakout.BRICK_ROWS = 4
Breakout.BRICK_GAP = 1
Breakout.BRICK_W = (Breakout.WINDOW_W - (Breakout.BRICK_COLS + 1) * Breakout.BRICK_GAP) // Breakout.BRICK_COLS
Breakout.BRICK_H = 8
Breakout.BRICK_TOP = Breakout.TITLE_BAR_H + 4
-- One distinct hue per row from the real 256-step color wheel
-- (AcidPalette, loaded into every app automatically -- see the app loader)
-- instead of the handful of reused UI theme colors every piece of
-- on-screen content in this codebase used before.
Breakout.BRICK_COLORS = {}
local brick_row = 0
while brick_row < Breakout.BRICK_ROWS do
  Breakout.BRICK_COLORS[#Breakout.BRICK_COLORS + 1] = AcidPalette.hue(brick_row * 256 // Breakout.BRICK_ROWS)
  brick_row = brick_row + 1
end

Breakout.PADDLE_W = 32
Breakout.PADDLE_H = 4
Breakout.PADDLE_Y = Breakout.WINDOW_H - 10

Breakout.BALL_R = 2
Breakout.BALL_SPEED = 3

-- Distinct voices from acid_blaster's (0/1) so the two never fight over
-- the same voice if somehow both were open at once.
Breakout.BRICK_VOICE = 2
Breakout.PADDLE_VOICE = 3
Breakout.OVER_VOICE = 4
Breakout.FILTER_MODE_LP = 1

Breakout.BRICK_NOTES = { 60, 57, 1, 1 }
Breakout.PADDLE_NOTES = { 40, 44, 1, 1 }
Breakout.OVER_NOTES = { 30, 27, 23, 18 }

function Breakout:on_create()
  acid_configure_filter(180, 3, Breakout.FILTER_MODE_LP)
  acid_configure_voice(Breakout.BRICK_VOICE, 1, 2, 30, 50, 50)
  acid_configure_voice(Breakout.PADDLE_VOICE, 1, 2, 30, 50, 50)
  acid_configure_voice(Breakout.OVER_VOICE, 1, 5, 60, 35, 40)
  self:reset_game()
end

function Breakout:reset_game()
  self.score = 0
  self.game_over = false
  self.win = false
  self.paddle_x = (Breakout.WINDOW_W - Breakout.PADDLE_W) // 2
  self.ball = {
    x = Breakout.WINDOW_W // 2,
    y = Breakout.PADDLE_Y - Breakout.BALL_R - 1,
    dx = Breakout.BALL_SPEED,
    dy = -Breakout.BALL_SPEED,
  }
  self.bricks = {}
  local row = 0
  while row < Breakout.BRICK_ROWS do
    local col = 0
    while col < Breakout.BRICK_COLS do
      self.bricks[#self.bricks + 1] = {
        x = Breakout.BRICK_GAP + col * (Breakout.BRICK_W + Breakout.BRICK_GAP),
        y = Breakout.BRICK_TOP + row * (Breakout.BRICK_H + Breakout.BRICK_GAP),
        color = Breakout.BRICK_COLORS[row % #Breakout.BRICK_COLORS + 1],
        alive = true,
      }
      col = col + 1
    end
    row = row + 1
  end
  -- Silence anything still gated before dropping the bookkeeping that
  -- would have silenced it. A restart tap arrives from on_touch while
  -- the game-over sting is usually still playing, and tick_sfx is the
  -- ONLY thing that ever sends a note-off -- clearing self.sfx without
  -- this leaves the voice with no note-off coming, so its arpeggio
  -- cycles and its envelope sustains forever. (Same fix as
  -- acid_blaster's; the shorter gate here only made it rarer to
  -- hit, not impossible.)
  self:stop_all_sfx()
  self.sfx = {}
  self.just_destroyed = {}
  self.drawn_ball = nil
  self.drawn_paddle_x = nil
  self.drawn_game_over = false
  self.needs_frame = true
  self.was_focused = false
end

function Breakout:trigger_sfx(voice, notes, count, rate_ms, volume, ticks)
  acid_play_note(voice, notes[1], volume)
  acid_trigger_arp(voice, notes[1], notes[2], notes[3], notes[4], count, rate_ms)
  self.sfx[#self.sfx + 1] = { voice = voice, ticks = ticks }
end

-- Safe to call before the first reset_game, when self.sfx is still nil.
function Breakout:stop_all_sfx()
  if not self.sfx then return end
  for _, s in ipairs(self.sfx) do acid_stop_note(s.voice) end
  self.sfx = {}
end

function Breakout:tick_sfx()
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

function Breakout:on_touch(x, y, pressed)
  -- The router repeats TOUCH every tick while held. Restarting at game
  -- over needs a press that began after it, so a press already held when
  -- the ball was lost doesn't restart instantly.
  if not pressed then
    self.touch_held = false
    return
  end
  local fresh = not self.touch_held
  self.touch_held = true
  if self.game_over then
    if fresh then self:reset_game() end
    return
  end
  self.paddle_x = x - Breakout.PADDLE_W // 2
  if self.paddle_x < 0 then self.paddle_x = 0 end
  if self.paddle_x > Breakout.WINDOW_W - Breakout.PADDLE_W then
    self.paddle_x = Breakout.WINDOW_W - Breakout.PADDLE_W
  end
end

function Breakout:on_tick()
  if not self.game_over then
    self:update_ball()
  end
  self:tick_sfx()
  local is_focused = self:focused()
  if is_focused and not self.was_focused then self.needs_frame = true end
  self.was_focused = is_focused
  if is_focused then self:draw() end
end

function Breakout:update_ball()
  local b = self.ball
  b.x = b.x + b.dx
  b.y = b.y + b.dy

  -- Clamped one pixel further in than the wall itself (BALL_R + 1 /
  -- WINDOW_W - 2 - BALL_R, not BALL_R / WINDOW_W - 1 - BALL_R) so the
  -- ball's circle never overlaps column 0 or WINDOW_W - 1 -- exactly
  -- where acid_draw_window_border's 1px side lines live. Without this
  -- margin the ball sat flush against the border on a side bounce, and
  -- every frame's erase_ball (a BG_COLOR circle at the ball's own
  -- position) painted straight over that border pixel, visibly eating a
  -- notch out of the green edge on every side hit (reported live).
  if b.x - Breakout.BALL_R < 1 then
    b.x = Breakout.BALL_R + 1
    b.dx = -b.dx
  elseif b.x + Breakout.BALL_R > Breakout.WINDOW_W - 2 then
    b.x = Breakout.WINDOW_W - 2 - Breakout.BALL_R
    b.dx = -b.dx
  end
  if b.y - Breakout.BALL_R < Breakout.TITLE_BAR_H then
    b.y = Breakout.TITLE_BAR_H + Breakout.BALL_R
    b.dy = -b.dy
  end

  if b.y + Breakout.BALL_R >= Breakout.PADDLE_Y and b.y + Breakout.BALL_R <= Breakout.PADDLE_Y + Breakout.PADDLE_H and
     b.dy > 0 and b.x >= self.paddle_x - Breakout.BALL_R and b.x <= self.paddle_x + Breakout.PADDLE_W + Breakout.BALL_R then
    b.dy = -b.dy
    local offset = b.x - (self.paddle_x + Breakout.PADDLE_W // 2)
    b.dx = Breakout.BALL_SPEED + offset // 6
    if b.dx < -3 then b.dx = -3 end
    if b.dx > 3 then b.dx = 3 end
    if b.dx == 0 then b.dx = 1 end
    self:trigger_sfx(Breakout.PADDLE_VOICE, Breakout.PADDLE_NOTES, 2, 20, 25, 2)
  end

  if b.y - Breakout.BALL_R > Breakout.WINDOW_H - 1 then
    self.game_over = true
    self:trigger_sfx(Breakout.OVER_VOICE, Breakout.OVER_NOTES, 2, 25, 35, 2)
    return
  end

  local hit = nil
  for _, brick in ipairs(self.bricks) do
    if brick.alive and
       b.x + Breakout.BALL_R >= brick.x and b.x - Breakout.BALL_R <= brick.x + Breakout.BRICK_W and
       b.y + Breakout.BALL_R >= brick.y and b.y - Breakout.BALL_R <= brick.y + Breakout.BRICK_H then
      hit = brick
      break
    end
  end
  if not hit then return end
  hit.alive = false
  self.just_destroyed[#self.just_destroyed + 1] = hit
  self.score = self.score + 1
  b.dy = -b.dy
  self:trigger_sfx(Breakout.BRICK_VOICE, Breakout.BRICK_NOTES, 2, 15, 30, 2)

  for _, brick in ipairs(self.bricks) do
    if brick.alive then return end
  end
  self.win = true
  self.game_over = true
  -- OVER_NOTES.reverse
  local o = Breakout.OVER_NOTES
  self:trigger_sfx(Breakout.OVER_VOICE, { o[4], o[3], o[2], o[1] }, 2, 25, 35, 2)
end

function Breakout:draw()
  if self.needs_frame then
    acid_clear_user_area()
    acid_draw_window_frame(self:window_title())
    for _, brick in ipairs(self.bricks) do
      if brick.alive then self:draw_brick(brick) end
    end
    self.drawn_paddle_x = nil
    self.drawn_ball = nil
    self.drawn_game_over = false
    acid_draw_window_border()
    self.needs_frame = false
  end

  if #self.just_destroyed > 0 then
    for _, brick in ipairs(self.just_destroyed) do
      self:erase_rect(brick.x, brick.y, Breakout.BRICK_W, Breakout.BRICK_H)
    end
    self.just_destroyed = {}
  end

  if self.game_over then
    if not self.drawn_game_over then
      self:erase_ball()
      self:erase_paddle()
      self:draw_game_over()
      self.drawn_game_over = true
    end
    return
  end

  self:erase_ball()
  acid_fill_circle(self.ball.x, self.ball.y, Breakout.BALL_R, Breakout.TEXT_COLOR)
  self.drawn_ball = { x = self.ball.x, y = self.ball.y }

  if self.drawn_paddle_x == self.paddle_x then return end
  self:erase_paddle()
  acid_fill_rect(self.paddle_x, Breakout.PADDLE_Y, Breakout.PADDLE_W, Breakout.PADDLE_H, Breakout.TEXT_COLOR)
  self.drawn_paddle_x = self.paddle_x
end

function Breakout:draw_brick(brick)
  acid_fill_rect(brick.x, brick.y, Breakout.BRICK_W, Breakout.BRICK_H, brick.color)
end

function Breakout:erase_rect(x, y, w, h)
  acid_fill_rect(x, y, w, h, Breakout.BG_COLOR)
end

function Breakout:erase_ball()
  if not self.drawn_ball then return end
  acid_fill_circle(self.drawn_ball.x, self.drawn_ball.y, Breakout.BALL_R, Breakout.BG_COLOR)
  self.drawn_ball = nil
end

function Breakout:erase_paddle()
  if not self.drawn_paddle_x then return end
  acid_fill_rect(self.drawn_paddle_x, Breakout.PADDLE_Y, Breakout.PADDLE_W, Breakout.PADDLE_H, Breakout.BG_COLOR)
  self.drawn_paddle_x = nil
end

function Breakout:draw_game_over()
  local label = self.win and "YOU WIN" or "GAME OVER"
  acid_draw_text(label, (Breakout.WINDOW_W - #label * 6) // 2, Breakout.WINDOW_H // 2 - 10, Breakout.TEXT_COLOR, Breakout.BG_COLOR)
  local score_label = "SCORE: " .. self.score
  acid_draw_text(score_label, (Breakout.WINDOW_W - #score_label * 6) // 2, Breakout.WINDOW_H // 2 + 4, Breakout.TEXT_COLOR, Breakout.BG_COLOR)
end

Breakout:new():start()
