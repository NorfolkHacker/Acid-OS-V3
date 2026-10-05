-- Headless tests for Acid Snake (apps/acid_snake.lua): movement, turning,
-- eating, trip pellets, deaths, restart, touch, sound, layout.

math.randomseed(1)

local G = GAME
local N = AcidSnake

local function head() return G.body[1].x, G.body[1].y end

local function stops(voice)
  local n = 0
  for _, e in ipairs(NOTES) do
    if e[1] == "stop" and e[2] == voice then n = n + 1 end
  end
  return n
end

local function has_text(s)
  for _, t in ipairs(TEXT_AT) do
    if t[1] == s then return true end
  end
  return false
end

local function fresh_frame() TEXT_AT, RECTS = {}, {} end

-- Fresh game with the pellet parked out of the way in a corner.
local function fresh()
  G:start_game()
  G.pellet = { x = 0, y = 0 }
end

group("title")
eq(G.state, "title", "opens on the title screen")
eq({ G.cols, G.rows }, { 39, 27 }, "a 39x27 board at 320x256")
fresh_frame()
G:on_tick()
ok(has_text("ACID SNAKE"), "the title is drawn")
G:on_key(AcidKeys.UP, true)
eq(G.state, "title", "a turn key doesn't start the game")
G:on_key(32, true)
eq(G.state, "playing", "space starts")
eq(#G.body, N.START_LEN, "the snake starts 4 long")
eq({ head() }, { 19, 13 }, "in the middle")

group("movement")
fresh()
for _ = 1, N.SLOWEST - 1 do G:on_tick() end
eq({ head() }, { 19, 13 }, "no move before its tick")
G:on_tick()
eq({ head() }, { 20, 13 }, "then one cell right")
eq(#G.body, N.START_LEN, "same length")
G:on_key(AcidKeys.UP, true)
G:advance()
eq({ head() }, { 20, 12 }, "up turns up")
G:on_key(string.byte("a"), true)
G:advance()
eq({ head() }, { 19, 12 }, "A turns left")

group("turn rules")
fresh()
G:on_key(AcidKeys.LEFT, true)
eq(#G.turns, 0, "no reversing into the neck")
G:on_key(AcidKeys.RIGHT, true)
eq(#G.turns, 0, "the same heading is not a turn")
G:on_key(AcidKeys.UP, true)
G:on_key(AcidKeys.LEFT, true)
eq(#G.turns, 2, "two quick turns queue")
G:on_key(AcidKeys.DOWN, true)
eq(#G.turns, 2, "a third doesn't")
G:advance(); G:advance()
eq({ head() }, { 18, 12 }, "the queued U-turn plays out")
eq(G.dir, { -1, 0 }, "heading left")

group("eating")
fresh()
G.pellet = { x = 20, y = 13 }
NOTES = {}
G:advance()
eq(G.score, N.PELLET_POINTS, "a pellet scores")
eq(NOTES[1] and NOTES[1][2], N.SFX.eat.voice, "and plays the eat sound")
ok(G.pellet and not (G.pellet.x == 20 and G.pellet.y == 13), "a new pellet appears elsewhere")
G.pellet = { x = 0, y = 0 }
G:advance()
eq(#G.body, N.START_LEN + 1, "the snake grows one")

group("trip pellets")
fresh()
for i = 1, N.TRIP_EVERY do
  local x = head()
  G.pellet = { x = x + 1, y = 13 }
  G:advance()
  if i < N.TRIP_EVERY then eq(G.trip, nil, "no trip pellet after " .. i) end
end
ok(G.trip ~= nil, "a trip pellet after the fifth")
eq(G.trip.ticks, N.TRIP_TICKS, "on a timer")
G.pellet = { x = 0, y = 0 }
G.paused = false
local trip = G.trip
G.move_timer = -10000
for _ = 1, N.TRIP_TICKS do G:on_tick() end
eq(G.trip, nil, "it disappears when the timer runs out")
G.move_timer = 0
local x = head()
G.trip = { x = x + 1, y = 13, ticks = 100 }
local before, len = G.score, #G.body + G.grow  -- plus growth still owed from the fifth pellet
G:advance()
eq(G.score - before, N.TRIP_POINTS, "eating it scores 50")
eq(G.ripple, N.RIPPLE_TICKS, "and sets off a ripple")
eq(G.trip, nil, "and it's gone")
for _ = 1, 3 do G:advance() end
eq(#G.body, len + 3, "it grows the snake three")

group("speed")
fresh()
eq(G:move_every(), N.SLOWEST, "slowest at the start")
for _ = 1, N.GROW_PER_SPEEDUP do G.body[#G.body + 1] = { x = 0, y = 26 } end
eq(G:move_every(), N.SLOWEST - 1, "one tick faster per 8 segments")
for _ = 1, 200 do G.body[#G.body + 1] = { x = 0, y = 26 } end
eq(G:move_every(), N.FASTEST, "never faster than every 2 ticks")

group("deaths")
fresh()
G.body = { { x = 38, y = 5 }, { x = 37, y = 5 } }
NOTES = {}
G:advance()
eq(G.state, "dead", "the wall kills")
eq(NOTES[1] and NOTES[1][2], N.SFX.death.voice, "with the death sound")
fresh()
-- A loop: heading down into its own body.
G.body = { { x = 5, y = 5 }, { x = 6, y = 5 }, { x = 6, y = 6 }, { x = 5, y = 6 }, { x = 4, y = 6 } }
G.dir = { 0, 1 }
G:advance()
eq(G.state, "dead", "running into itself kills")
fresh()
-- Chasing its own tail: the tail moves out of the way in time.
G.body = { { x = 5, y = 5 }, { x = 6, y = 5 }, { x = 6, y = 6 }, { x = 5, y = 6 } }
G.dir = { 0, 1 }
G:advance()
eq(G.state, "playing", "the cell the tail leaves is safe")
G.body = { { x = 5, y = 5 }, { x = 6, y = 5 }, { x = 6, y = 6 }, { x = 5, y = 6 } }
G.grow = 1
G:advance()
eq(G.state, "dead", "but not while growing")

group("restart")
G.score = 120
G.best = 0
G:die()
eq(G.best, 120, "the best score is kept")
G:on_key(32, true)
eq(G.state, "dead", "space straight after dying is ignored")
for _ = 1, N.DEAD_LOCK_TICKS do G:on_tick() end
ok(stops(N.SFX.death.voice) > 0, "the death note is turned off")
fresh_frame()
G:on_tick()
ok(has_text("GAME OVER"), "game over is drawn")
G:on_key(32, true)
eq(G.state, "playing", "then space plays again")
eq(G.score, 0, "from zero")
eq(G.best, 120, "keeping the best")

group("touch")
fresh()
local hx = G.bx + 19 * N.CELL + 4
local hy = G.by + 13 * N.CELL + 4
G:on_touch(hx + 40, hy - 30, true)
eq(G.turns, { { 0, -1 } }, "heading right, a tap above the head turns up")
G:on_touch(hx + 40, hy - 30, true)
eq(#G.turns, 1, "a held tap turns once")
G:on_touch(hx, hy, false)
G:advance()
G:on_touch(hx - 50, hy - 8, true)
eq(G.turns, { { -1, 0 } }, "heading up, a tap to the left turns left")
G:on_touch(0, 0, false)
G:die()
G:on_touch(50, 100, true)
eq(G.state, "dead", "a tap straight after dying is ignored")
G:on_touch(50, 100, false)
G.dead_ticks = N.DEAD_LOCK_TICKS
G:on_touch(50, 100, true)
eq(G.state, "playing", "a fresh tap later restarts")
G:on_touch(50, 100, false)

group("pause")
fresh()
G:on_key(string.byte("p"), true)
for _ = 1, 30 do G:on_tick() end
eq({ head() }, { 19, 13 }, "nothing moves while paused")
G:on_key(AcidKeys.UP, true)
eq(#G.turns, 0, "turns are ignored while paused")
G:on_key(string.byte("P"), true)
for _ = 1, N.SLOWEST do G:on_tick() end
eq({ head() }, { 20, 13 }, "P again resumes")

group("sound lifecycle")
fresh()
NOTES = {}
G:play_sfx("trip")
G:start_game()
eq(stops(N.SFX.trip.voice), 1, "restarting stops a playing sound")
G:play_sfx("eat"); G:play_sfx("eat")
eq(#G.sfx, 1, "one gate per voice")
for name, s in pairs(N.SFX) do
  eq(s.count * s.rate, s.ticks * N.TICK_MS, name .. ": the gate is one pass of its arpeggio")
end

group("full board")
fresh()
G.body = {}
for y = 0, G.rows - 1 do
  for x = 0, G.cols - 1 do G.body[#G.body + 1] = { x = x, y = y } end
end
G.pellet, G.trip = nil, nil
eq(G:free_cell(), nil, "no free cell on a full board")

group("layout")
fresh()
for _ = 1, 3 do G:advance() end
G.trip = { x = 30, y = 20, ticks = 50 }
fresh_frame()
G:redraw()
local fits, what = drawn_inside_window()
ok(fits, "play fits the window" .. (what and (": " .. what) or ""))
local clear, which = drawn_text_clear()
ok(clear, "no text overlaps" .. (which and (": " .. which) or ""))
FONT_W, FONT_H = 12, 16
G:layout()
G:to_title()
G.ticks = 0
fresh_frame()
G:redraw()
fits, what = drawn_inside_window()
ok(fits, "the title fits at Large text" .. (what and (": " .. what) or ""))
clear, which = drawn_text_clear()
ok(clear, "title text doesn't overlap at Large" .. (which and (": " .. which) or ""))
G:start_game()
G:die()
G.dead_ticks = N.DEAD_LOCK_TICKS
fresh_frame()
G:redraw()
fits, what = drawn_inside_window()
ok(fits, "game over fits at Large text" .. (what and (": " .. what) or ""))
clear, which = drawn_text_clear()
ok(clear, "game over text doesn't overlap at Large" .. (which and (": " .. which) or ""))
FONT_W, FONT_H = 6, 8
G:layout()

group("frames")
eq(FRAME_DEPTH, 0, "every frame begun was ended")
ok(FRAMES_ENDED > 0, "redraws go through frames")

group("whole pixels")
eq(NON_INT, {}, "every drawing call got whole pixels")
