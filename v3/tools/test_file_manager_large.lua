-- File Manager at Large (12x16 text in a 440x304 window): the listing and
-- its scroll bar fit, the row pitch is FONT_H + 4, and taps hit the row
-- drawn under them.

local G = GAME
local many = {}
for i = 1, 30 do many[i] = string.format("a_rather_long_file_name_%02d.txt", i) end
FS["v3/fsroot/Many"] = many
G.dir = "v3/fsroot/Many"
G:scan_dir()
eq(G:visible_listing_rows(), 13, "13 rows fit below the header at Large")
TEXT_AT, RECTS = {}, {}
G:redraw()
local fits, what = drawn_inside_window()
if fits then fits, what = drawn_text_clear() end
ok(fits, "the listing and its scroll bar fit the window, text not overlapping" .. (what and (": " .. what) or ""))
eq({ G:bar_geometry() }, { 433, 36, 267 }, "the scroll bar sits inside the right border, below the taller header")
G:on_touch(10, 36 + 20 * 2 + 5, false)
G:on_touch(10, 36 + 20 * 2 + 5, true)
eq(G.preview_name, many[2], "a tap on the third row opens the entry under it (rows are 20 px)")
