-- Editor re-lays out when the window is resized: the status line follows
-- the bottom edge, and the cursor stays inside the visible rows and columns.
local L = EditorLayout
local G = GAME

local lines = {}
for i = 1, 100 do lines[i] = string.rep("0123456789", 8) end
G.buf = Buffer.new(lines)
G.buf:set_cursor(70, 89) -- line 90, column 70 (the buffer counts from 0)
G:ensure_scroll()

local function check(w, h)
  local tag = w .. "x" .. h
  resize_app(w, h)
  local row = G.buf.cy - G.scroll_y
  local col = G.buf.cx - G.scroll_x
  ok(row >= 0 and row < G:visible_lines(), "cursor row is visible at " .. tag)
  ok(col >= 0 and col < G:visible_cols(), "cursor column is visible at " .. tag)
  local fits, what = drawn_inside_window()
  local clear, w2 = drawn_text_clear()
  ok(fits and clear, "fits at " .. tag .. (what and (": " .. what) or "") .. (w2 and (": " .. w2) or ""))
end

resize_app(600, 400)
eq({ L.WINDOW_W, L.WINDOW_H, L.STATUS_Y }, { 600, 400, 400 - (FONT_H + 2) }, "the layout follows the new window size")
check(600, 400)
check(200, 100)
eq(EditorCmd.CMD_CELL_CHARS * L.CHAR_W * 6 <= 200, true, "six command cells fit the narrow width")
