-- Headless tests for Breakout (apps/breakout.lua): its SFX lifecycle (a
-- game-over or restart must never leave a voice sounding).

local G = GAME
local OVER = Breakout.OVER_VOICE
local PADDLE = Breakout.PADDLE_VOICE

local function voices_of(kind)
  local out = {}
  for _, n in ipairs(NOTES) do
    if n[1] == kind then out[#out + 1] = n[2] end
  end
  return out
end

local function sorted(t)
  local c = { table.unpack(t) }
  table.sort(c)
  return c
end

local function lose_the_ball()
  G.ball.y = Breakout.WINDOW_H + Breakout.BALL_R + 10
  G:on_tick()
end

group("game-over sound lifecycle")
G:reset_game()
NOTES = {}
lose_the_ball()
ok(G.game_over, "losing the ball ends the game")
eq(voices_of("play"), { OVER }, "game over sounds the OVER voice")
NOTES = {}
for _ = 1, 8 do G:on_tick() end
eq(voices_of("stop"), { OVER }, "the OVER voice is stopped on schedule")

group("restarting mid-sound")
G:reset_game()
lose_the_ball()
NOTES = {}
G:on_touch(100, 100, false)
G:on_touch(100, 100, true)
eq(G.game_over, false, "tapping restarts the game")
eq(voices_of("stop"), { OVER }, "restarting stops the game-over voice immediately")

G:reset_game()
G:trigger_sfx(PADDLE, Breakout.PADDLE_NOTES, 2, 20, 25, 8)
lose_the_ball()
NOTES = {}
G:on_touch(100, 100, false)
G:on_touch(100, 100, true)
eq(sorted(voices_of("stop")), sorted({ PADDLE, OVER }), "restarting stops every voice still in flight")
NOTES = {}
for _ = 1, 10 do G:on_tick() end
eq(#voices_of("play"), 0, "no stray re-trigger after the restart")
eq(voices_of("stop"), {}, "and nothing is stopped twice")

group("restart needs a fresh press")
G:on_touch(100, 100, false)
G:reset_game()
G:on_touch(100, 100, true)
lose_the_ball()
G:on_touch(100, 100, true)
ok(G.game_over, "a press held through game over does not restart")
G:on_touch(100, 100, true)
ok(G.game_over, "and neither does it on later held ticks")
G:on_touch(100, 100, false)
G:on_touch(100, 100, true)
eq(G.game_over, false, "releasing then pressing again restarts")
