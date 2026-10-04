-- About re-clips its lines and re-counts its rows when the window is resized.
local lines = {}
for i = 1, 30 do lines[i] = string.rep(string.char(96 + (i % 26) + 1), 60) end
FS["v3/fsroot/Help/about.txt"] = table.concat(lines, "\n")
GAME.lines = GAME:read_lines()

resize_app(300, 200)
local fits, what = drawn_inside_window()
local clear, w2 = drawn_text_clear()
ok(fits and clear, "fits at 300x200" .. (what and (": " .. what) or "") .. (w2 and (": " .. w2) or ""))
local longest = 0
for _, t in ipairs(TEXT_AT) do longest = math.max(longest, #t[1]) end
eq(longest, (300 - 24) // FONT_W, "lines re-clip to the new width")
resize_app(120, 60)
fits, what = drawn_inside_window()
ok(fits, "fits at its minimum" .. (what and (": " .. what) or ""))
