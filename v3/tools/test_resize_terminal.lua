-- Terminal re-counts its visible rows and columns when the window is resized.
GAME.lines = {}
for i = 1, 40 do GAME.lines[i] = string.rep(string.char(96 + (i % 26) + 1), 79) .. string.char(65 + (i % 26)) end
GAME.input = string.rep("x", 50)

local function check(w, h)
  resize_app(w, h)
  eq(GAME:visible_lines(), (h - 16) // (FONT_H + 2) - 1, "visible rows at " .. w .. "x" .. h)
  local fits, what = drawn_inside_window()
  local clear, w2 = drawn_text_clear()
  ok(fits and clear, "fits at " .. w .. "x" .. h .. (what and (": " .. what) or "") .. (w2 and (": " .. w2) or ""))
end

check(400, 300)
local longest = 0
for _, t in ipairs(TEXT_AT) do longest = math.max(longest, #t[1]) end
eq(longest, (400 - 8) // FONT_W, "lines re-clip to the new width")

check(160, 80)
local cols = (160 - 8) // FONT_W
local want = GAME.lines[#GAME.lines]:sub(1, cols)
local found = false
for _, t in ipairs(TEXT_AT) do if t[1] == want then found = true end end
ok(found, "the last scrollback line is still drawn at the minimum")
