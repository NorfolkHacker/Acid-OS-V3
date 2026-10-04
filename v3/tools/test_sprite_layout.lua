-- Headless tests for SpriteLayout and SpritePicker (apps/sprite/layout.lua,
-- apps/sprite/picker.lua).

local function inside(L, r, what)
  ok(r.x >= 0 and r.y >= 0 and r.x + r.w <= L.w and r.y + r.h <= L.h, what .. " is inside the window")
end
local function all_inside(L, sw, sh, what)
  local rects = {}
  for _, b in ipairs(L.buttons) do rects[#rects + 1] = b end
  for _, s in ipairs(L.swatches) do rects[#rects + 1] = { x = s.x - 1, y = s.y - 1, w = s.w + 2, h = s.h + 2 } end
  for _, b in ipairs(L.bar) do rects[#rects + 1] = b end
  for _, b in ipairs(L.cmds) do rects[#rects + 1] = b end
  rects[#rects + 1] = { x = L.canvas.x, y = L.canvas.y, w = L.canvas.w, h = L.canvas.h }
  rects[#rects + 1] = { x = L.preview1.x, y = L.preview1.y, w = sw, h = sh }
  if L.preview2 then rects[#rects + 1] = { x = L.preview2.x, y = L.preview2.y, w = 2 * sw, h = 2 * sh } end
  rects[#rects + 1] = { x = L.picker.x, y = L.picker.y, w = 12 * L.picker.cw, h = 5 * L.picker.ch }
  for _, r in ipairs(rects) do
    if r.x < 1 or r.y < 16 or r.x + r.w > L.w - 1 or r.y + r.h > L.h - 1 then
      ok(false, what .. ": a rect at " .. r.x .. "," .. r.y .. " size " .. r.w .. "x" .. r.h .. " leaves the user area")
      return
    end
  end
  ok(true, what .. ": every rect is inside the user area")
end

group("SpriteLayout: the default window")
local L = SpriteLayout.compute(360, 260, 16, 16)
eq({ L.col_x, L.msg_y, L.bar_y }, { 255, 239, 249 }, "column, message line and frame bar")
eq({ L.canvas.x, L.canvas.y, L.canvas.cell, L.grid }, { 24, 23, 13, true }, "a 16x16 sprite gets centred 13 px cells")
eq({ L.buttons[1].id, L.buttons[1].x, L.buttons[1].y, L.buttons[6].id, L.buttons[6].x, L.buttons[6].y },
  { "pen", 255, 19, "mirror", 323, 32 }, "two rows of three tool buttons")
eq({ L.swatches[1].key, L.swatches[1].x, L.swatches[1].y, L.swatches[16].key, L.swatches[16].x, L.swatches[16].y },
  { "0", 255, 47, "f", 321, 113 }, "a 4x4 palette under the buttons")
eq({ L.preview1.x, L.preview1.y, L.preview2 and L.preview2.x, L.preview2 and L.preview2.y }, { 255, 136, 291, 136 },
  "1x and 2x previews under the palette")
eq({ L.bar[1].id, L.bar[1].x, L.bar[2].id, L.bar[2].x, L.bar[9].id, L.bar[9].x }, { "cmd", 5, "prev", 29, "fps", 167 },
  "frame bar items at fixed places")
eq({ L.cmds[1].label, L.cmds[2].id, L.cmds[6].id }, { "s:save", "saveas", "close" }, "the command strip")
all_inside(L, 16, 16, "360x260, 16x16")
eq(SpriteLayout.compute(360, 260, 32, 32).canvas.cell, 6, "a 32x32 sprite gets 6 px cells")
all_inside(SpriteLayout.compute(360, 260, 32, 32), 32, 32, "360x260, 32x32")

group("SpriteLayout: the minimum window")
local M = SpriteLayout.compute(280, 200, 32, 32)
eq({ M.canvas.cell, M.grid }, { 4, true }, "32x32 at the minimum still gets 4 px cells and a grid")
eq(M.preview2, nil, "no room for the 2x preview of a 32x32 sprite")
ok(SpriteLayout.compute(280, 200, 16, 16).preview2 ~= nil, "a 16x16 sprite keeps its 2x preview")
all_inside(M, 32, 32, "280x200, 32x32")
all_inside(SpriteLayout.compute(280, 200, 8, 8), 8, 8, "280x200, 8x8")

group("SpriteLayout: hit-testing")
eq({ SpriteLayout.cell_at(L, 16, 16, 24, 23) }, { 0, 0 }, "the canvas's top-left pixel is cell 0,0")
eq({ SpriteLayout.cell_at(L, 16, 16, 24 + 13 * 16 - 1, 23 + 13 * 16 - 1) }, { 15, 15 }, "its bottom-right pixel is 15,15")
eq({ SpriteLayout.cell_at(L, 16, 16, 37, 36) }, { 1, 1 }, "13 px on is the next cell")
eq({ SpriteLayout.cell_at(L, 16, 16, 23, 23) }, {}, "left of the canvas is no cell")
eq({ SpriteLayout.cell_at(L, 16, 16, 24 + 13 * 16, 23) }, {}, "right of the canvas is no cell")
eq(SpriteLayout.find(L.bar, 30, L.bar_y).id, "prev", "a tap on '<' finds prev")
eq(SpriteLayout.find(L.bar, 25, L.bar_y), nil, "a tap between items finds nothing")
eq(SpriteLayout.find(L.swatches, 300, 100).key, "a", "a tap on a swatch finds its key (row 2, column 2)")

group("SpritePicker")
eq({ SpritePicker.color(0), SpritePicker.color(12), SpritePicker.color(36) }, { 0xFF0000, 0xBF0000, 0x3F0000 },
  "red at full, 3/4 and 1/4 brightness")
eq({ SpritePicker.color(48), SpritePicker.color(55) }, { 0x000000, 0xFFFFFF }, "the grey row runs black to white")
eq(SpritePicker.hit(L, L.picker.x, L.picker.y), 0, "the picker's top-left cell is colour 0")
eq(SpritePicker.hit(L, L.picker.x + 7 * 8, L.picker.y + 4 * 12), 55, "the last grey")
eq(SpritePicker.hit(L, L.picker.x + 8 * 8, L.picker.y + 4 * 12), nil, "past the last grey is nothing")
eq({ SpritePicker.rect(L, 13) }, { L.picker.x + 8, L.picker.y + 12, 8, 12 }, "colour 13 is second row, second column")
