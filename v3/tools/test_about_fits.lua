-- About with a long about.txt: everything drawn fits the window, lines are
-- cut to the columns that fit, and sit FONT_H + 2 apart.
local lines = {}
for i = 1, 30 do lines[i] = string.rep(string.char(96 + (i % 26) + 1), 45) end
FS["v3/fsroot/Help/about.txt"] = table.concat(lines, "\n")
GAME.lines = GAME:read_lines()
TEXT_AT, RECTS = {}, {}
GAME:redraw()

local fits, what = drawn_inside_window()
if fits then fits, what = drawn_text_clear() end
ok(fits, "everything drawn fits the window" .. (what and (": " .. what) or ""))

local longest = 0
for _, t in ipairs(TEXT_AT) do if #t[1] > longest then longest = #t[1] end end
eq(longest, (WIN_W - 24) // FONT_W, "lines are cut to the columns that fit")

local spaced = #TEXT_AT > 1
for i = 2, #TEXT_AT do
  if TEXT_AT[i][3] - TEXT_AT[i - 1][3] ~= FONT_H + 2 then spaced = false end
end
ok(spaced, "consecutive lines are FONT_H + 2 apart")
