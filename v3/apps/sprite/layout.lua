-- SpriteLayout: every rectangle Sprite Paint draws or hit-tests, for a
-- window size and a sprite size. Recomputed on create, resize, new and
-- load. The app is not font-scalable, so text is always 6x8.
SpriteLayout = {}
SpriteLayout.TITLE_H = 16
SpriteLayout.MARGIN = 4
SpriteLayout.COL_W = 100                -- the right-hand column
SpriteLayout.TOP = SpriteLayout.TITLE_H + 3
SpriteLayout.CH_W, SpriteLayout.CH_H = 6, 8
SpriteLayout.BTN_W, SpriteLayout.BTN_H, SpriteLayout.BTN_GAP = 32, 12, 2
SpriteLayout.SWATCH, SpriteLayout.SWATCH_GAP = 20, 2
SpriteLayout.PREVIEW_GAP = 4
SpriteLayout.MAX_PREVIEW_1X = 32     -- the 1x preview's slot: the largest sprite

-- { label, id }, laid out in two rows of three.
SpriteLayout.TOOLS = {
  { "pen", "pen" }, { "fill", "fill" }, { "rub", "eraser" },
  { "pick", "picker" }, { "line", "line" }, { "mir", "mirror" },
}
-- The frame bar: { id, width in characters }. Fixed widths, so nothing
-- moves as the labels change ("8/8", "stop", "30fps" are the widest).
SpriteLayout.BAR = {
  { "cmd", 3 }, { "prev", 1 }, { "count", 3 }, { "next", 1 }, { "add", 1 },
  { "dup", 3 }, { "del", 3 }, { "play", 4 }, { "fps", 5 },
}
-- The command strip: { key, id, label }.
SpriteLayout.CMDS = {
  { "s", "save", "save" }, { "a", "saveas", "as" }, { "n", "new", "new" },
  { "u", "undo", "undo" }, { "r", "redo", "redo" }, { "q", "close", "close" },
}
-- The colour picker: 12 hues by 4 shades, then a row of 8 greys.
SpriteLayout.PICKER_COLS, SpriteLayout.PICKER_ROWS = 12, 5
SpriteLayout.PICKER_CW, SpriteLayout.PICKER_CH = 8, 12

-- Lays text items left to right from x, one character apart.
local function row_items(list, x, y, width_of)
  local items = {}
  for _, e in ipairs(list) do
    local chars = width_of(e)
    items[#items + 1] = { entry = e, x = x, y = y - 1, w = chars * SpriteLayout.CH_W, h = SpriteLayout.CH_H + 2 }
    x = x + (chars + 1) * SpriteLayout.CH_W
  end
  return items
end

function SpriteLayout.compute(w, h, sw, sh)
  local S = SpriteLayout
  local L = { w = w, h = h }
  L.col_x = w - 1 - S.MARGIN - S.COL_W
  L.bar_y = h - 1 - 2 - S.CH_H
  L.msg_y = L.bar_y - 2 - S.CH_H
  L.text_x = 1 + S.MARGIN
  L.text_w = w - 2 * L.text_x

  L.buttons = {}
  for i, t in ipairs(S.TOOLS) do
    local c, r = (i - 1) % 3, (i - 1) // 3
    L.buttons[i] = { id = t[2], label = t[1], w = S.BTN_W, h = S.BTN_H,
      x = L.col_x + c * (S.BTN_W + S.BTN_GAP), y = S.TOP + r * (S.BTN_H + 1) }
  end
  local pal_y = S.TOP + 2 * (S.BTN_H + 1) + 2
  L.swatches = {}
  for i = 0, 15 do
    local c, r = i % 4, i // 4
    L.swatches[i + 1] = { key = AcidSprite.KEYS:sub(i + 1, i + 1), w = S.SWATCH, h = S.SWATCH,
      x = L.col_x + c * (S.SWATCH + S.SWATCH_GAP), y = pal_y + r * (S.SWATCH + S.SWATCH_GAP) }
  end
  local prev_y = pal_y + 4 * (S.SWATCH + S.SWATCH_GAP) + 1
  L.preview1 = { x = L.col_x, y = prev_y, scale = 1 }
  if prev_y + 2 * sh <= L.msg_y - 2 then
    L.preview2 = { x = L.col_x + S.MAX_PREVIEW_1X + S.PREVIEW_GAP, y = prev_y, scale = 2 }
  end
  L.side_h = L.msg_y - 3 - S.TOP

  L.picker = { x = L.col_x + 2, y = S.TOP, cw = S.PICKER_CW, ch = S.PICKER_CH }

  local ax, ay = L.text_x, S.TOP
  local aw, ah = L.col_x - S.MARGIN - ax, L.msg_y - 3 - ay
  local cell = math.max(1, math.min(aw // sw, ah // sh))
  L.canvas = { cell = cell, w = cell * sw, h = cell * sh,
    x = ax + (aw - cell * sw) // 2, y = ay + (ah - cell * sh) // 2 }
  L.grid = cell >= 4

  L.bar = row_items(S.BAR, L.text_x, L.bar_y, function(e) return e[2] end)
  for _, it in ipairs(L.bar) do it.id = it.entry[1] end
  L.cmds = row_items(S.CMDS, L.text_x, L.bar_y, function(e) return #e[1] + 1 + #e[3] end)
  for _, it in ipairs(L.cmds) do it.id, it.label = it.entry[2], it.entry[1] .. ":" .. it.entry[3] end
  return L
end

-- The first item whose rectangle holds (x, y), or nil.
function SpriteLayout.find(items, x, y)
  for _, it in ipairs(items) do
    if x >= it.x and x < it.x + it.w and y >= it.y and y < it.y + it.h then return it end
  end
end

-- The sprite cell under (x, y), or nil off the canvas.
function SpriteLayout.cell_at(L, sw, sh, x, y)
  local c = L.canvas
  if x < c.x or y < c.y then return nil end
  local cx, cy = (x - c.x) // c.cell, (y - c.y) // c.cell
  if cx >= sw or cy >= sh then return nil end
  return cx, cy
end
