-- Headless tests for Acid Invaders (apps/acid_invaders.lua): formation,
-- marching, the cannon, shots, shields, bombs, saucer and trip shot,
-- waves, game over, sound, layout.

math.randomseed(1)

local G = GAME
local V = AcidInvaders

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

local function shields_left()
  local n = 0
  for _, c in ipairs(G.shields) do
    if not c.dead then n = n + 1 end
  end
  return n
end

-- A fresh game with one alien, parked well away from everything, and no
-- bombs or saucer on the way.
local function quiet(x, y)
  G:start_game()
  G.aliens = { { row = 5, col = 1, x = x or 20, y = y or 60 } }
  G.bomb_timer, G.saucer_timer, G.march_timer = 1e9, 1e9, -1e9
end

group("title")
eq(G.state, "title", "opens on the title screen")
fresh_frame()
G:on_tick()
ok(has_text("ACID INVADERS"), "the title is drawn")
G:on_key(AcidKeys.LEFT, true)
eq(G.state, "title", "a move key doesn't start the game")
G:on_key(32, true)
eq(G.state, "playing", "space starts")
eq(#G.aliens, V.COLS * V.ROWS, "a full 8x5 formation")
eq(G.lives, 3, "three lives")
eq(shields_left(), 3 * 59, "three whole shields")
eq({ G.aliens[1].x, G.aliens[1].y }, { 76, 44 }, "the formation is centred")

group("marching")
G:start_game()
G.bomb_timer, G.saucer_timer = 1e9, 1e9
eq(G:march_every(), 10, "40 aliens step every 10 ticks")
local x0 = G.aliens[1].x
for _ = 1, 9 do G:on_tick() end
eq(G.aliens[1].x, x0, "not before")
G:on_tick()
eq(G.aliens[1].x, x0 + V.MARCH_DX, "then 3 px right")
eq(G.frame, 2, "and the sprites animate")
-- Push it to the right wall.
for _ = 1, 200 do
  G:march()
  if G.march_dir == -1 then break end
end
eq(G.march_dir, -1, "the wall turns it round")
eq(G.aliens[1].y, 44 + V.DROP, "one row lower")
G.aliens = { G.aliens[1] }
eq(G:march_every(), 1, "the last alien steps every tick")

group("cannon")
quiet()
local px = G.player_x
G:on_key(AcidKeys.LEFT, true)
G:on_tick(); G:on_tick()
ok(G.player_x < px, "left keeps it moving with no key held")
G:on_key(string.byte("d"), true)
eq(G.move, 0, "the other way stops it")
G:on_key(string.byte("D"), true)
eq(G.move, 1, "then moves that way")
G:on_key(AcidKeys.DOWN, true)
eq(G.move, 0, "down stops")
G:on_key(AcidKeys.RIGHT, true)
for _ = 1, 300 do G:on_tick() end
eq(G.player_x, G.ax1 - V.PLAYER_W / 2, "the wall stops it")
G:on_key(AcidKeys.DOWN, true)

group("shooting")
quiet(113, 80)  -- in the gap between the first two shields
G.player_x = 120
G:on_key(32, true)
eq(#G.shots, 1, "space fires")
G:on_key(AcidKeys.UP, true)
eq(#G.shots, 1, "one shot in the air at a time")
local before = G.score
NOTES = {}
for _ = 1, 40 do G:on_tick() end
eq(#G.aliens, 0, "the shot kills the alien")
eq(G.score - before, V.ROW_POINTS[5], "worth its row's points")
eq(NOTES[1] and NOTES[1][2], V.SFX.kill.voice, "with the kill sound")
eq(G.state, "wave_clear", "the last alien clears the wave")

group("shields")
quiet()
local cell
for _, c in ipairs(G.shields) do if not c.dead then cell = c break end end
ok(G:hit_shield(cell.x, cell.y), "a point on a shield cell hits it")
ok(cell.dead, "and knocks it out")
ok(not G:hit_shield(cell.x, cell.y), "a dead cell lets things through")
local s1 = G.shields[1]
G.player_x = s1.x + 1
local count = shields_left()
G.shots = {}
G:fire()
for _ = 1, 20 do G:on_tick() end
eq(shields_left(), count - 1, "your own shot wears a shield away too")
G.shots = { { x = s1.x + 1, y = G.shield_y + 40, vx = 0 } }
G:update_shots(1)  -- a whole second in one go: far further than a cell
eq(shields_left(), count - 2, "even a long step can't tunnel through a shield")

group("bombs")
quiet()
G.player_x = 60
G.invuln = 0
NOTES = {}
G.bombs = { { x = 60, y = G.player_y - 6 } }
G:on_tick()
eq(G.lives, 2, "a bomb on the cannon costs a life")
eq(#G.bombs, 0, "and clears the bombs")
eq(NOTES[1] and NOTES[1][2], V.SFX.death.voice, "with the death sound")
ok(G.invuln > 0, "then a moment of safety")
G.bombs = { { x = 60, y = G.player_y - 6 } }
G:on_tick()
eq(G.lives, 2, "no hit while safe")
G:start_game()
G.bombs = {}
G:drop_bomb()
eq(#G.bombs, 1, "the formation drops bombs")
eq(G.bombs[1].y, 44 + 4 * V.GAP_Y + V.ALIEN_H, "from its bottom row")
eq(G:max_bombs(), 3, "3 bombs at once on wave 1")

group("saucer and trip shot")
quiet()
G.saucer_timer = 1
G:on_tick()
ok(G.saucer ~= nil, "a saucer comes out on its timer")
local sc = G.saucer
G.player_x = sc.x + V.SAUCER_W / 2
G.shots = { { x = sc.x + 8, y = sc.y + 3, vx = 0 } }
NOTES = {}
local sb = G.score
G:shot_hits(G.shots[1])
eq(G.saucer, nil, "a shot brings it down")
eq(G.score - sb, V.SAUCER_POINTS, "worth 100")
eq(G.trip, V.TRIP_TICKS, "and gives the trip shot")
eq(NOTES[1] and NOTES[1][2], V.SFX.saucer.voice, "with the saucer sound")
G.shots = {}
G:fire()
eq(#G.shots, 3, "the trip shot fires three")
G.shots = {}
G.trip = 1
G:on_tick()
G.shots = {}
G:fire()
eq(#G.shots, 1, "then wears off")

group("touch")
quiet()
G.player_x = 100
G:on_touch(200, 200, true)
eq(#G.shots, 1, "a tap fires")
G:on_tick()
ok(G.player_x > 100, "and the cannon slides towards it")
G.shots = {}
G:on_touch(200, 200, true)
eq(#G.shots, 0, "a held tap doesn't fire again")
for _ = 1, 60 do G:on_touch(200, 200, true); G:on_tick() end
eq(G.player_x, 200, "it stops under the pointer")
G:on_touch(200, 200, false)
eq(G.pointer, nil, "letting go lets it be")

group("waves")
G:start_game()
G.aliens = {}
G:on_tick()
eq(G.state, "wave_clear", "no aliens clears the wave")
fresh_frame()
G:redraw()
ok(has_text("WAVE 02"), "the next wave is announced")
for _ = 1, V.WAVE_PAUSE_TICKS do G:on_tick() end
eq(G.state, "playing", "and starts on its own")
eq(G.wave, 2, "wave 2")
eq(G.aliens[1].y, 44 + V.WAVE_DROP, "a row lower")
eq(G:max_bombs(), 4, "with more bombs")
G:start_wave(20)
eq(G.aliens[1].y, 44 + V.MAX_WAVE_DROPS * V.WAVE_DROP, "but never too low")

group("game over")
quiet()
G.lives = 1
G.invuln = 0
G.score = 90
G.best = 0
G.bombs = { { x = G.player_x, y = G.player_y - 2 } }
G:on_tick()
eq(G.state, "dead", "losing the last life ends the game")
eq(G.best, 90, "the best score is kept")
G:on_key(32, true)
eq(G.state, "dead", "space straight away does nothing")
for _ = 1, V.DEAD_LOCK_TICKS do G:on_tick() end
ok(stops(V.SFX.death.voice) > 0, "the death note is turned off")
fresh_frame()
G:redraw()
ok(has_text("GAME OVER"), "game over is drawn")
G:on_key(32, true)
eq(G.state, "playing", "then space plays again")
-- At the right wall, so the next step is a drop onto the cannon's row.
quiet(G.ax1 - 2 - V.ALIEN_W, G.player_y - V.ALIEN_H - 2)
G.march_dir = 1
G.march_timer = 1e9
G:on_tick()
eq(G.state, "dead", "aliens dropping onto the cannon's row end the game")

group("pause")
quiet()
G:on_key(string.byte("p"), true)
G:on_key(AcidKeys.LEFT, true)
eq(G.move, 0, "keys are ignored while paused")
local ax = G.aliens[1].x
G.march_timer = 0
for _ = 1, 30 do G:on_tick() end
eq(G.aliens[1].x, ax, "nothing moves while paused")
G:on_key(string.byte("P"), true)
ok(not G.paused, "P again resumes")

group("sound lifecycle")
quiet()
NOTES = {}
G:play_sfx("saucer")
G:start_game()
eq(stops(V.SFX.saucer.voice), 1, "restarting stops a playing sound")
G:play_sfx("kill"); G:play_sfx("kill")
eq(#G.sfx, 1, "one gate per voice")
for name, s in pairs(V.SFX) do
  eq(s.count * s.rate, s.ticks * V.TICK_MS, name .. ": the gate is one pass of its arpeggio")
end

group("layout")
G:start_game()
G.saucer = { x = 100, y = G.ay0 + 2, vx = 50 }
G:ring(150, 100, 0xFFFFFF)
G:fire()
G:drop_bomb()
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
local hud_bottom = G.hud_y + FONT_H
local below = true
for _, r in ipairs(RECTS) do
  if r[2] < hud_bottom and r[2] + r[4] > G.hud_y and r[1] > 0 then below = false end
end
ok(below, "the title's aliens stay below the HUD at Large")
G:start_game()
G:game_over()
G.dead_ticks = V.DEAD_LOCK_TICKS
fresh_frame()
G:redraw()
fits, what = drawn_inside_window()
ok(fits, "game over fits at Large text" .. (what and (": " .. what) or ""))
clear, which = drawn_text_clear()
ok(clear, "game over text doesn't overlap at Large" .. (which and (": " .. which) or ""))
FONT_W, FONT_H = 6, 8
G:layout()

group("whole pixels")
eq(NON_INT, {}, "every drawing call got whole pixels")
