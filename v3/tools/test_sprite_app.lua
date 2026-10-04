-- Sprite Paint (apps/sprite.lua) opened from the Menu: drawing with each
-- tool, the palette and colour picker, held taps, and resizing.
local G = GAME
local blank = string.rep(".", 16)

group("Sprite Paint: from the Menu")
eq({ G.doc.s.w, G.doc.s.h, G.doc:frame_count(), G.key, G.tool, G.path }, { 16, 16, 1, "1", "pen" },
  "a new 16x16 sprite, colour 1, the pencil, no file")
TEXT_AT, RECTS = {}, {}
G:redraw()
sp_fits("the first frame fits the window")
ok(sp_shown("new  untitled"), "the message line says new, untitled")

group("Sprite Paint: pencil, undo, mirror, eraser")
sp_tap_cell(2, 3)
eq(G.doc:get(2, 3), "1", "a pencil tap paints a cell")
eq(G.doc.dirty, true, "and makes the doc dirty")
sp_drag({ { 0, 5 }, { 4, 5 } })
eq(G.doc:rows()[6], "11111" .. string.rep(".", 11), "a fast drag leaves no gaps")
G:action("undo")
eq(G.doc:rows()[6], blank, "one drag is one undo step")
sp_tap(sp_button("mirror"))
eq(G.mirror, true, "the mirror button turns mirror on")
sp_tap_cell(0, 0)
eq(G.doc:rows()[1], "1" .. string.rep(".", 14) .. "1", "mirror paints both sides")
sp_tap(sp_button("mirror"))
sp_tap(sp_button("eraser"))
sp_tap_cell(0, 0)
eq({ G.tool, G.doc:get(0, 0), G.doc:get(15, 0) }, { "eraser", ".", "1" }, "the eraser clears one cell")

group("Sprite Paint: line")
sp_tap(sp_button("line"))
local x0, y0 = sp_cell_xy(0, 8)
local x1, y1 = sp_cell_xy(5, 8)
G:on_touch(x0, y0, true)
G:on_touch(x1, y1, true)
eq(G.doc:rows()[9], blank, "while held, the line is only a preview")
G:on_touch(x1, y1, false)
eq(G.doc:rows()[9], "111111" .. string.rep(".", 10), "the release commits it, both ends included")
G:action("undo")
eq(G.doc:rows()[9], blank, "a line is one undo step")
G:action("redo")

group("Sprite Paint: fill and picker")
sp_tap(sp_swatch("2"))
eq(G.key, "2", "a swatch tap selects its colour")
RECTS = {}
sp_tap(sp_swatch("4"))
local found_clear = false
for _, r in ipairs(RECTS) do
  if r[1] == G.L.col_x - 1 and r[2] == SpriteLayout.TOP and r[3] >= SpriteLayout.COL_W + 1 and r[5] == G.BG then
    found_clear = true
    break
  end
end
ok(found_clear, "selecting another swatch clears a rect at col_x - 1 covering the highlight")
RECTS = {}
G:draw_side()
local sx, sy
for _, s in ipairs(G.L.swatches) do
  if s.key == "0" then
    sx, sy = s.x, s.y
    break
  end
end
local found_swatch_edge = false
for _, r in ipairs(RECTS) do
  if r[1] == sx - 1 and r[2] == sy - 1 and r[3] == 22 and r[4] == 22 and r[5] == G.SWATCH_EDGE then
    found_swatch_edge = true
    break
  end
end
ok(found_swatch_edge, "dark swatches have a visible edge outline")
sp_tap(sp_swatch("2"))
sp_tap(sp_button("fill"))
sp_tap_cell(15, 15)
eq({ G.doc:get(15, 15), G.doc:get(3, 8), G.doc:get(2, 3) }, { "2", "1", "1" }, "fill covers the open area and stops at drawn cells")
G:action("undo")
sp_tap(sp_button("picker"))
sp_tap_cell(3, 8)
eq(G.key, "1", "the picker picks a cell's colour")
sp_tap_cell(10, 10)
eq(G.tool, "eraser", "picking a transparent cell selects the eraser")

group("Sprite Paint: editing a palette colour")
sp_tap(sp_swatch("3"))
eq(G.picker_open, false, "the first tap only selects")
sp_tap(sp_swatch("3"))
eq(G.picker_open, true, "a second tap on the selected swatch opens the picker")
TEXT_AT, RECTS = {}, {}
G:redraw()
sp_fits("the picker fits the window")
local px, py = SpritePicker.rect(G.L, 12)
sp_tap(px + 1, py + 1)
eq({ G.picker_open, G.doc.s.palette["3"] }, { false, 0xBF0000 }, "choosing a colour sets the slot and closes the picker")
sp_tap(sp_swatch("3"))
G:on_key(AcidKeys.ESCAPE, true)
eq({ G.picker_open, G.doc.s.palette["3"] }, { false, 0xBF0000 }, "ESC closes the picker without a change")
G:action("undo")
eq(G.doc.s.palette["3"], SpriteDoc.default_palette()["3"], "a colour edit is one undo step")

group("Sprite Paint: a held tap acts once")
local bx, by = sp_bar("add")
for _ = 1, 5 do G:on_touch(bx, by, true) end
G:on_touch(bx, by, false)
eq(G.doc:frame_count(), 2, "holding + adds one frame")

group("Sprite Paint: resizing")
resize_app(500, 400)
sp_fits("at 500x400")
eq(G.L.canvas.cell, 22, "the cells grow with the window")
sp_tap(sp_button("pen"))
sp_tap(sp_swatch("4"))
sp_tap_cell(7, 7)
eq(G.doc:get(7, 7), "4", "taps land on the right cell after growing")
resize_app(280, 200)
sp_fits("at the minimum")
sp_tap_cell(15, 15)
eq(G.doc:get(15, 15), "4", "taps land on the right cell at the minimum")
