-- Headless tests for Tetris (apps/tetris.lua): scoring is cleared^2 * 100,
-- the line sound arpeggiates 2 notes (4 for 3+ lines), walls and locked
-- cells block a piece, and a blocked spawn ends the game with the over
-- sound, which tick_sfx stops after its ticks.

local G = GAME
local T = Tetris

local function fill_row(y)
  for x = 1, T.COLS do G.grid[y][x] = 0x123456 end
end

local function row_empty(y)
  for x = 1, T.COLS do
    if G.grid[y][x] then return false end
  end
  return true
end

local function voices_of(kind)
  local out = {}
  for _, n in ipairs(NOTES) do
    if n[1] == kind then out[#out + 1] = n[2] end
  end
  return out
end

group("line clears")
G:on_create()
NOTES = {}
fill_row(T.ROWS)
G:clear_lines()
eq(G.score, 100, "one line scores 100")
ok(row_empty(T.ROWS), "the full row is removed")
eq(#G.grid, T.ROWS, "the grid keeps its height")
eq(NOTES[1], { "play", T.VOICE, 60, 35 }, "a line clear plays the line sound")
eq(NOTES[2][7], 2, "a single clear arpeggiates two notes")

G:on_create()
NOTES = {}
fill_row(T.ROWS)
fill_row(T.ROWS - 1)
fill_row(T.ROWS - 2)
G:clear_lines()
eq(G.score, 900, "three lines score 3*3*100")
eq(NOTES[2][7], 4, "three or more lines arpeggiate all four notes")

group("collision")
G:on_create()
G.piece_name = "O"
ok(not G:piece_fits(-2, 0, 0), "a piece can't pass the left wall")
ok(not G:piece_fits(T.COLS - 2, 0, 0), "or the right wall")
ok(G:piece_fits(0, 0, 0), "an empty grid fits it at the top")
G.grid[2][2] = 0x123456
ok(not G:piece_fits(0, 0, 0), "it can't overlap a locked cell")

group("game over")
G:on_create()
NOTES = {}
for y = 1, T.ROWS do fill_row(y) end
G:spawn_piece()
ok(G.game_over, "a blocked spawn ends the game")
eq(NOTES[1], { "play", T.VOICE, 40, 30 }, "game over plays the over sound")
NOTES = {}
for _ = 1, 3 do G:on_tick() end
eq(voices_of("stop"), { T.VOICE }, "the over sound stops after its ticks")

-- Tetris's on_touch acts once per press: touch_held swallows the
-- repeated pressed=true events of a held tap until a release clears it.
group("touch hold guard")
G:on_create()
G.piece_name = "O"
G.px = 4
G.py = 0
G.rotation = 0
G:on_touch(0, 50, true)
eq(G.px, 3, "a held left touch moves the piece once")
G:on_touch(0, 50, true)
G:on_touch(0, 50, true)
eq(G.px, 3, "repeated pressed events while held do nothing")
G:on_touch(0, 50, false)
eq(G.px, 3, "the release itself does not act")
G:on_touch(0, 50, true)
eq(G.px, 2, "a new press after the release acts again")
G.piece_name = "T"
G.px = 4
G:on_touch(0, 50, false)
G:on_touch(80, 50, true)
G:on_touch(80, 50, true)
eq(G.rotation, 1, "a held centre touch rotates once")

group("restart stops the sound")
G:on_create()
for y = 1, T.ROWS do fill_row(y) end
G:spawn_piece()
ok(G.game_over, "the board is over, its sound still gated")
NOTES = {}
G:on_touch(80, 50, false)
G:on_touch(80, 50, true)
eq(G.game_over, false, "a tap restarts")
eq(voices_of("stop"), { T.VOICE }, "restarting stops the voice immediately")
eq(G.sfx_ticks, nil, "and clears the pending sfx ticks")
NOTES = {}
for _ = 1, 5 do G:on_tick() end
eq(voices_of("stop"), {}, "tick_sfx doesn't stop it a second time")
