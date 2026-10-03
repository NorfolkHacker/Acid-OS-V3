-- Headless tests for Acid Blaster (apps/acid_blaster.lua): SFX lifecycle,
-- enemy colours, stars.

-- Enemy spawns come from math.random, and Lua seeds it randomly per
-- state; a few spawn points miss the target, so an unseeded run could
-- (about 1 in 400) see no game over within tick_to_game_over's limit.
math.randomseed(1)

local G = GAME
local A = AcidBlaster
local OVER = A.OVER_VOICE

local function tick_to_game_over(limit)
  limit = limit or 400
  for n = 1, limit do
    G:on_tick()
    if G.game_over then return n end
  end
  return nil
end

local function over_events()
  local out = {}
  for _, n in ipairs(NOTES) do
    if n[2] == OVER then out[#out + 1] = n end
  end
  return out
end

local function kinds(events)
  local out = {}
  for _, e in ipairs(events) do out[#out + 1] = e[1] end
  return out
end

local function all(list, pred)
  for _, v in ipairs(list) do
    if not pred(v) then return false end
  end
  return true
end

local function count(list, pred)
  local n = 0
  for _, v in ipairs(list) do
    if pred(v) then n = n + 1 end
  end
  return n
end

local function one_by_one_rects()
  return count(RECTS, function(r) return r[3] == 1 and r[4] == 1 end)
end

group("game-over sound lifecycle")
G:reset_game()
NOTES = {}
ok(tick_to_game_over(), "reaches game over by flying an enemy into the target")
eq(#over_events() > 0, true, "game over triggers the OVER voice")
eq((over_events()[1] or {})[1], "play", "game over plays a note on the OVER voice")
local gate_ms = A.OVER_TICKS * AcidGame.TICK_MS
local pass_ms = A.OVER_ARP_COUNT * A.OVER_ARP_RATE_MS
eq(gate_ms <= pass_ms, true,
   "game-over gate (" .. gate_ms .. "ms) does not outlast one arp pass (" .. pass_ms .. "ms)")
NOTES = {}
for _ = 1, A.OVER_TICKS do G:on_tick() end
eq(kinds(over_events()), { "stop" }, "the OVER voice is stopped after OVER_TICKS ticks")

group("restarting mid-sound")
G:reset_game()
NOTES = {}
tick_to_game_over()
-- Guarded: a round with no game over fails here with a message, not a nil index.
eq((over_events()[1] or {})[1], "play", "game over sounded again on the second round")
NOTES = {}
G:on_touch(10, 30, false)
G:on_touch(10, 30, true)
eq(G.game_over, false, "tapping restarts the game")
eq(kinds(over_events()), { "stop" }, "restarting stops the game-over voice immediately")
NOTES = {}
for _ = 1, 15 do G:on_tick() end
eq(G.game_over, false, "the fresh round is still running")
eq(count(over_events(), function(n) return n[1] == "play" end), 0,
   "no stray re-trigger of the OVER voice after the restart")

group("enemy colours")
G:reset_game()
local colors = {}
for _ = 1, 200 do
  G:spawn_enemy()
  colors[#colors + 1] = G.enemies[#G.enemies].color
end
ok(all(colors, function(c) return math.type(c) == "integer" end), "every enemy spawns with a colour")
local greens = count(colors, function(c) return c == A.ENEMY_COLOR end)
ok(greens > #colors // 2, "most enemies are still the usual green (" .. greens .. "/200)")
ok(greens < #colors, "but some spawn a different colour (" .. (200 - greens) .. "/200)")
local shades = {}
for _, c in ipairs(colors) do
  if c ~= A.ENEMY_COLOR then shades[c] = true end
end
local distinct = 0
for _ in pairs(shades) do distinct = distinct + 1 end
ok(distinct > 1, "the off-colour enemies are not all one single alternate shade")

G:reset_game()
G.enemies = { { x = 100, y = 100, dx = 0, dy = 0, color = 0xFF00FF } }
RECTS = {}
G:draw()
ok(count(RECTS, function(r) return r[5] == 0xFF00FF end) > 0, "an enemy is drawn in its own colour")

group("background stars")
G:reset_game()
local stars = G.stars
eq(#stars, A.STAR_COUNT, "a full sky is generated")
ok(all(stars, function(s) return s.y >= A.STAR_TOP end), "no star sits in the SCORE line's text band")
ok(all(stars, function(s) return s.y < A.TITLE_BAR_H + A.PLAY_H end), "no star sits below the play field")
ok(all(stars, function(s)
  return s.x >= A.STAR_MARGIN and s.x <= A.WINDOW_W - 1 - A.STAR_MARGIN
     and s.y <= A.WINDOW_H - 1 - A.STAR_MARGIN
end), "no star sits on the window border or a rounded corner")
ok(all(stars, function(s)
  for _, c in ipairs(A.STAR_COLORS) do
    if c == s.color then return true end
  end
  return false
end), "stars only use the dim star palette")
ok(all(stars, function(s)
  local ddx = s.x - A.CENTER_X
  local ddy = s.y - A.CENTER_Y
  return (ddx * ddx + ddy * ddy) > (A.TARGET_R1 * A.TARGET_R1)
end), "no star is hidden under the target")

G:reset_game()
G:draw()
RECTS = {}
G:on_tick()
eq(one_by_one_rects(), A.STAR_COUNT, "every star is repainted on an incremental frame")

G:reset_game()
G:on_tick()
for _ = 1, 400 do
  RECTS = {}
  G:on_tick()
  if G.game_over then break end
end
eq(one_by_one_rects(), A.STAR_COUNT, "the stars survive the game-over screen")

group("one shot per press, restart needs a fresh press")
G:on_touch(10, 30, false)
G:reset_game()
local shots = 0
local real_check = G.check_tap
G.check_tap = function(self, x, y) shots = shots + 1; return real_check(self, x, y) end
G:on_touch(10, 30, true)
G:on_touch(10, 30, true)
G:on_touch(10, 30, true)
eq(shots, 1, "a held press fires one shot")
G:on_touch(10, 30, false)
G:on_touch(10, 30, true)
eq(shots, 2, "another shot needs a release and a new press")
G.check_tap = nil
G:on_touch(10, 30, false)
G:on_touch(10, 30, true)
tick_to_game_over()
G:on_touch(10, 30, true)
ok(G.game_over, "a press held through game over does not restart")
G:on_touch(10, 30, false)
G:on_touch(10, 30, true)
eq(G.game_over, false, "releasing then pressing again restarts")
