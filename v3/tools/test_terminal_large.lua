-- Terminal at Large (12x16 text, 520x304 window): 15 scrollback lines, 42
-- columns, and everything drawn (including the cursor) stays in the window.
local G = GAME
eq(G:visible_lines(), 15, "15 scrollback lines fit at Large")
G.lines = {}
for i = 1, 30 do G.lines[i] = string.rep("x", 60) end
G.input = string.rep("y", 50)
TEXT_AT, RECTS = {}, {}
G:redraw()
local fits, what = drawn_inside_window()
if fits then fits, what = drawn_text_clear() end
ok(fits, "scrollback, prompt and cursor fit the window" .. (what and (": " .. what) or ""))
local longest = 0
for _, t in ipairs(TEXT_AT) do longest = math.max(longest, #t[1]) end
eq(longest, 42, "lines are clipped at 42 columns of 12 px")
