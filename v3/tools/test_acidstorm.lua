-- Headless tests for AcidStorm (apps/acidstorm.lua): screens, latched
-- controls, pointer aim, combat, waves, sound lifecycle, layout.

math.randomseed(1)

local G = GAME
local S = AcidStorm
local W, H = 320, 256

local function count(kind)
  local n = 0
  for _, e in ipairs(G.ents) do
    if e.kind == kind then n = n + 1 end
  end
  return n
end

local function only(kind)
  local keep = {}
  for _, e in ipairs(G.ents) do
    if e.kind == kind then keep[#keep + 1] = e end
  end
  G.ents = keep
end

-- An arena with nothing in it but one tough hulk parked in the far
-- corner, so the wave doesn't count as cleared.
local function arena()
  only("none")
  G:add("hulk", 300, 240, 5).hp = 99
end

local function stops(voice)
  local n = 0
  for _, e in ipairs(NOTES) do
    if e[1] == "stop" and e[2] == voice then n = n + 1 end
  end
  return n
end

local function fresh_frame() TEXT_AT, RECTS = {}, {} end

local function has_text(s)
  for _, t in ipairs(TEXT_AT) do
    if t[1] == s then return true end
  end
  return false
end

group("title")
eq(G.state, "title", "opens on the title screen")
eq(S.TICK_MS, 33, "33 ms tick")
G:on_key(string.byte("w"), true)
eq(G.state, "title", "a move key doesn't start the game")
fresh_frame()
G:on_tick()
ok(has_text("ACIDSTORM"), "the title is drawn")
G:on_key(32, true)
eq(G.state, "playing", "space starts the game")
eq(G.lives, 3, "three lives")
eq(G.wave, 1, "wave 1")
eq(count("grunt"), 6, "wave 1 has 6 grunts")
eq(count("enforcer"), 0, "no enforcers on wave 1")
eq(count("hulk"), 0, "no hulks on wave 1")
eq(count("human"), 5, "wave 1 has 5 humans")
eq(count("electrode"), 2, "wave 1 has 2 electrodes")

group("latched movement")
arena()
G.invuln = 0
local x0, y0 = G.player.x, G.player.y
G:on_key(string.byte("d"), true)
G:on_tick(); G:on_tick()
ok(G.player.x > x0, "D keeps moving right with no key held")
eq(G.player.y, y0, "and only right")
G:on_key(string.byte("W"), true)
local x1, y1 = G.player.x, G.player.y
G:on_tick()
ok(G.player.x > x1 and G.player.y < y1, "W adds up: a diagonal (shift works too)")
G:on_key(string.byte("a"), true)
eq(G.move_x, 0, "the opposite key stops that axis")
eq(G.move_y, -1, "and leaves the other")
G:on_key(32, true)
eq({ G.move_x, G.move_y }, { 0, 0 }, "space stops moving")
for _ = 1, 200 do G:on_key(string.byte("a"), true); G:on_tick() end
eq(G.player.x, G.ax0 + G.player.r, "the arena wall stops the player")
G:on_key(32, true)

group("firing")
arena()
G:on_key(AcidKeys.UP, true)
G:on_tick()
eq(count("pbullet"), 1, "an arrow fires")
for _ = 1, 9 do G:on_tick() end
ok(count("pbullet") >= 3, "and keeps firing")
eq({ G.player.face_x, G.player.face_y }, { 0, -1 }, "facing the arrow")
G:on_key(AcidKeys.RIGHT, true)
eq({ G.aim_x, G.aim_y }, { 1, 0 }, "another arrow turns the aim")
G:on_key(AcidKeys.RIGHT, true)
eq({ G.aim_x, G.aim_y }, { 0, 0 }, "the same arrow again stops firing")
arena()
G:on_tick(); G:on_tick(); G:on_tick()
eq(count("pbullet"), 0, "no bullets once stopped")

group("pointer aim")
arena()
G.player.x, G.player.y = 100, 100
G.fire_cd = 0
G:on_touch(200, 100, true)
G:on_tick()
eq(count("pbullet"), 1, "holding the pointer fires")
eq({ G.player.face_x, G.player.face_y }, { 1.0, 0.0 }, "at the pointer")
G:on_touch(200, 100, false)
eq(G.pointer, nil, "letting go stops aiming")
G:on_tick()
G.fire_cd = 0
arena()
G:on_tick()
eq(count("pbullet"), 0, "and firing")

group("combat")
arena()
G.player.x, G.player.y = 100, 120
local g = G:add("grunt", 140, 120, 3.5)
G.score = 0
NOTES = {}
G:on_key(AcidKeys.RIGHT, true)
for _ = 1, 10 do G:on_tick() end
ok(g.dead, "a bullet kills a grunt")
eq(G.score, 100, "worth 100")
eq(NOTES[1] and NOTES[1][2], S.SFX.kill.voice, "a kill plays the kill sound")
G:on_key(AcidKeys.RIGHT, true)
arena()
local hk = G:add("hulk", 140, 120, 5); hk.hp = 3
G:on_key(AcidKeys.RIGHT, true)
for _ = 1, 30 do
  G:on_tick()
  if hk.hp < 3 then break end
end
ok(not hk.dead and hk.hp < 3, "a hulk takes more than one hit")
for _ = 1, 20 do G:on_tick() end
ok(hk.dead, "but goes down")
G:on_key(AcidKeys.RIGHT, true)

group("humans")
arena()
G.player.x, G.player.y = 100, 120
G.humans_alive = 1
local hu = G:add("human", 101, 120, 3)
hu.timer = 10
local before = G.score
G:on_tick()
ok(hu.dead, "touching a human rescues it")
eq(G.score - before, 1000, "worth 1000")
eq(G.humans_alive, 0, "one fewer human")

group("getting hit")
arena()
G.player.x, G.player.y = 100, 120
G.invuln = 0
G.lives = 3
G:add("electrode", 100, 120, 4)
G:on_tick()
eq(G.lives, 2, "an electrode costs a life")
ok(G.invuln > 0, "then a moment of invulnerability")
G.player.x, G.player.y = 100, 120
G:on_tick()
eq(G.lives, 2, "no second hit while invulnerable")

group("wave clear")
only("none")
G.score = 0
G.humans_alive = 2
G:on_tick()
eq(G.state, "wave_clear", "no hostiles left clears the wave")
eq(G.score, 100 * 1 + 50 * 2, "bonus: 100 per wave plus 50 per human left")
for _ = 1, math.ceil(S.WAVE_CLEAR_SECS / S.DT) + 1 do G:on_tick() end
eq(G.state, "playing", "the next wave starts on its own")
eq(G.wave, 2, "wave 2")
eq(count("enforcer"), 1, "enforcers from wave 2")
G:start_wave(3)
eq(count("hulk"), 1, "hulks from wave 3")

group("pause")
G:on_key(string.byte("p"), true)
ok(G.paused, "P pauses")
local px = G.player.x
G.move_x = 1
G:on_tick()
eq(G.player.x, px, "nothing moves while paused")
G:on_key(string.byte("d"), true)
eq(G.move_x, 1, "move keys are ignored while paused")
G:on_key(string.byte("p"), true)
ok(not G.paused, "P again resumes")
G:stop_input()

group("game over")
arena()
G.lives = 1
G.invuln = 0
G:add("electrode", G.player.x, G.player.y, 4)
NOTES = {}
G:on_tick()
eq(G.state, "game_over", "the last life ends the game")
eq(NOTES[1] and NOTES[1][2], S.SFX.death.voice, "with the death sound")
G:on_key(32, true)
eq(G.state, "game_over", "space right away does nothing (lockout)")
G:on_touch(50, 50, true)
for _ = 1, 20 do G:on_tick(); G:on_touch(50, 50, true) end
eq(G.state, "game_over", "a held press doesn't leave game over")
ok(stops(S.SFX.death.voice) > 0, "the death note is turned off")
G:on_touch(50, 50, false)
fresh_frame()
G:on_tick()
ok(has_text("GAME OVER"), "game over is drawn")
G:on_touch(50, 50, true)
eq(G.state, "title", "a fresh press goes back to the title")
G:on_touch(50, 50, false)

group("sound lifecycle")
G:start_game()
NOTES = {}
G:play_sfx("rescue")
G:to_title()
eq(stops(S.SFX.rescue.voice), 1, "leaving the game stops a playing sound")
eq(#G.sfx, 0, "and forgets it")
G:start_game()
NOTES = {}
G:play_sfx("kill"); G:play_sfx("kill")
eq(#G.sfx, 1, "one gate per voice")
for _ = 1, S.SFX.kill.ticks do G:tick_sfx() end
eq(stops(S.SFX.kill.voice), 1, "the gate ends with a note-off")
for name, s in pairs(S.SFX) do
  eq(s.count * s.rate, s.ticks * S.TICK_MS, name .. ": the gate is one pass of its arpeggio")
end

group("layout")
G:start_game()
for _ = 1, 30 do G:on_tick() end
fresh_frame()
G:redraw()
local fits, what = drawn_inside_window()
ok(fits, "play fits the window" .. (what and (": " .. what) or ""))
local clear, which = drawn_text_clear()
ok(clear, "no text overlaps" .. (which and (": " .. which) or ""))
FONT_W, FONT_H = 12, 16
G:layout()
G:to_title()
fresh_frame()
G:redraw()
fits, what = drawn_inside_window()
ok(fits, "the title fits at Large text" .. (what and (": " .. what) or ""))
clear, which = drawn_text_clear()
ok(clear, "the title's text doesn't overlap at Large" .. (which and (": " .. which) or ""))
G:start_game()
G.lives = 1
G.invuln = 0
G:add("electrode", G.player.x, G.player.y, 4)
G:on_tick()
fresh_frame()
G.time = 0
G:redraw()
fits, what = drawn_inside_window()
ok(fits, "game over fits at Large text" .. (what and (": " .. what) or ""))
clear, which = drawn_text_clear()
ok(clear, "game over text doesn't overlap at Large" .. (which and (": " .. which) or ""))
FONT_W, FONT_H = 6, 8
G:layout()
