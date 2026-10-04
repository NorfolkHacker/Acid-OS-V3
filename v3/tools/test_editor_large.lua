-- Editor at Large (12x16 text in a 640x456 window): the layout comes from
-- the font and window size, long lines, the gutter, the status line and
-- the command strip all fit, and a tap lands on the cell drawn under it.

local L = EditorLayout
local G = GAME
eq({ L.CHAR_W, L.LINE_H, L.WINDOW_W, L.WINDOW_H }, { 12, 18, 640, 456 }, "the layout comes from the font and window size")
eq({ L.GUTTER_W, L.TEXT_X, L.STATUS_Y }, { 48, 50, 438 }, "and everything derived from them follows")

local lines = {}
for i = 1, 40 do lines[i] = string.rep("0123456789", 8) .. i end
FS["v3/fsroot/Home/long.txt"] = table.concat(lines, "\n") .. "\n"
LAUNCH_ARG = "v3/fsroot/Home/long.txt"
G:on_create()
G.message = "unsaved -- ESC q again to close"
TEXT_AT, RECTS = {}, {}
G:redraw()
local fits, what = drawn_inside_window()
if fits then fits, what = drawn_text_clear() end
ok(fits, "text, gutter and status line fit the window, text not overlapping" .. (what and (": " .. what) or ""))

G.message = nil
G.buf:set_cursor(0, 0)
G:on_key(AcidKeys.ESCAPE, true)
TEXT_AT, RECTS = {}, {}
G:redraw()
fits, what = drawn_inside_window()
ok(fits, "the command strip fits the window" .. (what and (": " .. what) or ""))
eq(EditorCmd.CMD_CELL_CHARS * 12 * 6 <= 640, true, "six command cells fit the width")
G:on_key(AcidKeys.ESCAPE, true)

G.scroll_y, G.scroll_x = 0, 0
G.buf:set_cursor(0, 0)
G:on_touch(L.TEXT_X + 3 * 12 + 1, L.TEXT_Y + 2 * 18 + 1, true)
G:on_touch(L.TEXT_X + 3 * 12 + 1, L.TEXT_Y + 2 * 18 + 1, false)
eq({ G.buf.cx, G.buf.cy }, { 3, 2 }, "a tap puts the cursor on the third line at column 3")
