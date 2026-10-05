-- Headless tests for Acid Rocks (apps/acid_rocks.lua): latched turning
-- and thrust, wrapping, shooting and splitting, the ship's deaths and
-- respawn, hyperspace, pointer aim, waves, sound, layout.

math.randomseed(1)

local G = GAME
local R = AcidRocks

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

local function fresh_frame() TEXT_AT, RECTS, LINES = {}, {}, {} end

-- A still rock, so tests decide where everything is.
local function rock(size, x, y)
  local r = G:new_rock(size, x, y)
  r.vx, r.vy, r.spin = 0, 0, 0
  return r
end

-- A game with one still rock tucked in a corner (so the wave isn't
-- clear), and the ship safe in the middle.
local function quiet()
  G:start_game()
  G.rocks = { rock(1, 20, 240) }
  G.invuln = 0
end

local function near(a, b, eps) return math.abs(a - b) <= (eps or 1e-6) end

group("title")
eq(G.state, "title", "opens on the title screen")
fresh_frame()
G:on_tick()
ok(has_text("ACID ROCKS"), "the title is drawn")
ok(#LINES > 0, "with a rock in vector lines")
G:on_key(AcidKeys.LEFT, true)
eq(G.state, "title", "a turn key doesn't start the game")
G:on_key(32, true)
eq(G.state, "playing", "space starts")
eq(#G.rocks, 4, "wave 1 has 4 big rocks")
eq(G.lives, 3, "three lives")
local clear = true
for _, r in ipairs(G.rocks) do
  if math.abs(r.x - G.cx) + math.abs(r.y - G.cy) < R.SPAWN_GAP * 1.5 then clear = false end
end
ok(clear, "no rock starts on top of the ship")

group("turning")
quiet()
local a0 = G.ship.a
G:on_key(AcidKeys.RIGHT, true)
G:on_tick(); G:on_tick()
ok(near(G.ship.a, (a0 + 2 * R.TURN_SPEED * R.DT) % (2 * math.pi)), "right keeps turning with no key held")
G:on_key(string.byte("a"), true)
eq(G.turn, 0, "the other way stops it")
G:on_key(string.byte("A"), true)
eq(G.turn, -1, "then turns that way")
G:on_key(AcidKeys.RIGHT, true)
eq(G.turn, 0, "and stops again")

group("thrust and drift")
quiet()
G.ship.a = 0
G:on_key(AcidKeys.UP, true)
ok(G.thrust, "up switches thrust on")
for _ = 1, 10 do G:on_tick() end
ok(G.ship.vx > 0 and near(G.ship.vy, 0), "and pushes the way it faces")
ok(#G.parts > 0, "leaving exhaust")
G:on_key(string.byte("w"), true)
ok(not G.thrust, "up again switches it off")
local v = G.ship.vx
G:on_tick()
ok(G.ship.vx < v and G.ship.vx > 0, "then it drifts, slowing")
G.ship.vx, G.ship.vy = 1e6, 0
G:on_key(AcidKeys.UP, true)
G:on_tick()
ok(G.ship.vx <= R.MAX_SPEED + 1e-6, "never faster than its top speed")
G:on_key(AcidKeys.UP, true)

group("wrapping")
quiet()
G.ship.x, G.ship.y, G.ship.vx, G.ship.vy = G.ax1 - 0.5, 100, 60, 0
G:on_tick()
ok(G.ship.x < G.ax0 + 5, "off the right edge, back on the left")
local e = { x = 100, y = G.ay0 - 1 }
G:wrap(e)
ok(e.y > G.ay1 - 5, "off the top, back at the bottom")
ok(near(G:dist(G.ax0 + 1, 100, G.ax1 - 1, 100), 2), "distance is measured the short way round")

group("shooting")
quiet()
G.ship.a = 0
G:on_key(32, true)
eq(#G.bullets, 1, "space fires")
for _ = 1, 10 do G:on_key(32, true) end
eq(#G.bullets, R.MAX_BULLETS, "at most 4 in the air")
for _ = 1, math.ceil(R.BULLET_LIFE / R.DT) + 1 do G:on_tick() end
eq(#G.bullets, 0, "bullets run out")

group("splitting")
quiet()
G.ship.a = 0
local big = rock(3, G.ship.x + 40, G.ship.y)
G.rocks = { big, rock(1, 20, 240) }
G.score = 0
NOTES = {}
G:on_key(32, true)
for _ = 1, 10 do G:on_tick() end
local sizes = {}
for _, r in ipairs(G.rocks) do sizes[#sizes + 1] = r.size end
table.sort(sizes)
eq(sizes, { 1, 2, 2 }, "a big rock splits into two medium")
eq(G.score, R.ROCK_POINTS[3], "worth 20")
local boomed = false
for _, n in ipairs(NOTES) do if n[1] == "play" and n[2] == R.SFX.boom.voice then boomed = true end end
ok(boomed, "with a boom")
G.rocks = { rock(2, 100, 100), rock(1, 20, 240) }
G:break_rock(1)
eq(#G.rocks, 3, "a medium splits into two small")
G.rocks = { rock(1, 100, 100), rock(1, 20, 240) }
G:break_rock(1)
eq(#G.rocks, 1, "a small one just goes")

group("extra lives")
quiet()
G.score = R.EXTRA_LIFE_EVERY - 10
G:add_score(20)
eq(G.lives, 4, "an extra life at 5000")
G:add_score(20)
eq(G.lives, 4, "only once")

group("crashing and respawn")
quiet()
G.invuln = 0
NOTES = {}
G.rocks = { rock(3, G.ship.x + 10, G.ship.y), rock(1, 20, 240) }
G:on_tick()
eq(G.lives, 2, "hitting a rock costs a life")
ok(G.waiting, "the ship waits to come back")
local played = false
for _, n in ipairs(NOTES) do if n[1] == "play" and n[2] == R.SFX.death.voice then played = true end end
ok(played, "with the death sound")
G:on_key(32, true)
eq(#G.bullets, 0, "no firing while waiting")
G:on_tick()
ok(G.waiting, "not while a rock is in the middle")
G.rocks = { rock(1, 20, 240) }
G:on_tick()
ok(not G.waiting, "back once the middle is clear")
eq({ G.ship.x, G.ship.y }, { G.cx, G.cy }, "in the middle")
ok(G.invuln > 0, "safe for a moment")
G.rocks = { rock(3, G.ship.x + 10, G.ship.y), rock(1, 20, 240) }
G:on_tick()
eq(G.lives, 2, "no crash while safe")

group("hyperspace")
quiet()
G.ship.vx = 50
local hx, hy = G.ship.x, G.ship.y
G:on_key(AcidKeys.DOWN, true)
ok(G.ship.x ~= hx or G.ship.y ~= hy, "down jumps somewhere else")
eq({ G.ship.vx, G.ship.vy }, { 0, 0 }, "standing still")
local jx = G.ship.x
G:on_key(string.byte("s"), true)
eq(G.ship.x, jx, "not again straight away")
for _ = 1, math.ceil(R.HYPER_COOLDOWN / R.DT) + 1 do G:on_tick() end
G:on_key(string.byte("s"), true)
ok(G.ship.x ~= jx, "then again")

group("pointer aim")
quiet()
G.ship.a = 0
G:on_touch(G.ship.x, G.ship.y - 80, true)
eq(#G.bullets, 1, "a tap fires at once")
for _ = 1, 30 do G:on_touch(G.ship.x, G.ship.y - 80, true); G:on_tick() end
ok(near(G.ship.a, 3 * math.pi / 2, 0.01), "holding turns the ship to the pointer")
ok(#G.bullets >= 2, "and keeps firing")
G:on_touch(0, 0, false)
eq(G.pointer, nil, "letting go stops it")

group("waves")
quiet()
G.rocks = {}
G:on_tick()
eq(G.state, "wave_clear", "no rocks clears the wave")
fresh_frame()
G:redraw()
ok(has_text("WAVE 02"), "the next wave is announced")
local lives, score = G.lives, G.score
for _ = 1, R.WAVE_PAUSE_TICKS do G:on_tick() end
eq(G.state, "playing", "and starts on its own")
eq(#G.rocks, 5, "with one more rock")
eq({ G.lives, G.score }, { lives, score }, "keeping lives and score")
G:start_wave(30)
eq(#G.rocks, 8, "never more than 8 to start")

group("game over")
quiet()
G.lives = 1
G.score = 70
G.best = 0
G.rocks = { rock(3, G.ship.x, G.ship.y) }
G:on_tick()
eq(G.state, "dead", "losing the last ship ends the game")
eq(G.best, 70, "the best score is kept")
G:on_key(32, true)
eq(G.state, "dead", "space straight away does nothing")
G:on_touch(9, 99, true)
for _ = 1, R.DEAD_LOCK_TICKS do G:on_tick(); G:on_touch(9, 99, true) end
eq(G.state, "dead", "a held press doesn't restart")
ok(stops(R.SFX.death.voice) > 0, "the death note is turned off")
G:on_touch(9, 99, false)
fresh_frame()
G:redraw()
ok(has_text("GAME OVER"), "game over is drawn")
G:on_touch(9, 99, true)
eq(G.state, "playing", "a fresh tap plays again")
G:on_touch(9, 99, false)

group("pause")
quiet()
G.rocks[1].vx = 30
G:on_key(string.byte("p"), true)
local rx = G.rocks[1].x
for _ = 1, 10 do G:on_tick() end
eq(G.rocks[1].x, rx, "nothing moves while paused")
G:on_key(AcidKeys.LEFT, true)
eq(G.turn, 0, "keys are ignored while paused")
G:on_key(string.byte("P"), true)
ok(not G.paused, "P again resumes")

group("sound lifecycle")
quiet()
NOTES = {}
G:play_sfx("death")
G:start_game()
eq(stops(R.SFX.death.voice), 1, "restarting stops a playing sound")
G:play_sfx("boom"); G:play_sfx("boom")
eq(#G.sfx, 1, "one gate per voice")
for name, s in pairs(R.SFX) do
  eq(s.count * s.rate, s.ticks * R.TICK_MS, name .. ": the gate is one pass of its arpeggio")
end

group("layout")
quiet()
G.rocks = { rock(3, G.ax0 + 2, G.ay0 + 2), rock(2, 150, 100) }
G:on_key(AcidKeys.UP, true)
for _ = 1, 5 do G:on_tick() end
G:on_key(32, true)
fresh_frame()
G:redraw()
local fits, what = drawn_inside_window()
ok(fits, "play fits the window" .. (what and (": " .. what) or ""))
local tclear, which = drawn_text_clear()
ok(tclear, "no text overlaps" .. (which and (": " .. which) or ""))
FONT_W, FONT_H = 12, 16
G:layout()
G:to_title()
G.ticks = 0
fresh_frame()
G:redraw()
fits, what = drawn_inside_window()
ok(fits, "the title fits at Large text" .. (what and (": " .. what) or ""))
tclear, which = drawn_text_clear()
ok(tclear, "title text doesn't overlap at Large" .. (which and (": " .. which) or ""))
G:start_game()
G.lives = 1
G.rocks = { rock(3, G.ship.x, G.ship.y) }
G.invuln = 0
G:on_tick()
G.dead_ticks = R.DEAD_LOCK_TICKS
fresh_frame()
G:redraw()
fits, what = drawn_inside_window()
ok(fits, "game over fits at Large text" .. (what and (": " .. what) or ""))
tclear, which = drawn_text_clear()
ok(tclear, "game over text doesn't overlap at Large" .. (which and (": " .. which) or ""))
FONT_W, FONT_H = 6, 8
G:layout()

group("whole pixels")
eq(NON_INT, {}, "every drawing call got whole pixels")
