-- SpriteTools: what Sprite Paint's tools do to a SpriteDoc. Pure functions
-- on cell coordinates; the app maps the pointer to cells and brackets each
-- gesture as one undo step.
SpriteTools = {}

-- The cells of a Bresenham line, both ends included, in order from
-- (x0, y0).
function SpriteTools.line_cells(x0, y0, x1, y1)
  local cells = {}
  local dx, dy = math.abs(x1 - x0), -math.abs(y1 - y0)
  local sx = x0 < x1 and 1 or -1
  local sy = y0 < y1 and 1 or -1
  local err = dx + dy
  local x, y = x0, y0
  while true do
    cells[#cells + 1] = { x, y }
    if x == x1 and y == y1 then break end
    local e2 = 2 * err
    if e2 >= dy then err = err + dy; x = x + sx end
    if e2 <= dx then err = err + dx; y = y + sy end
  end
  return cells
end

-- One cell, plus its mirror image across the vertical centre line.
function SpriteTools.paint(doc, x, y, key, mirror)
  doc:set(x, y, key)
  if mirror then doc:set(doc.s.w - 1 - x, y, key) end
end

-- Every cell from (x0, y0) to (x1, y1): Pencil and Eraser join successive
-- pointer samples with this so a fast drag leaves no gaps, and Line
-- commits with it.
function SpriteTools.stroke(doc, x0, y0, x1, y1, key, mirror)
  for _, c in ipairs(SpriteTools.line_cells(x0, y0, x1, y1)) do
    SpriteTools.paint(doc, c[1], c[2], key, mirror)
  end
end

-- 4-way flood fill of the region of (x, y)'s key, transparent included.
function SpriteTools.fill(doc, x, y, key)
  local target = doc:get(x, y)
  if target == nil or target == key then return end
  local stack = { { x, y } }
  while #stack > 0 do
    local c = table.remove(stack)
    local cx, cy = c[1], c[2]
    if doc:get(cx, cy) == target then
      doc:set(cx, cy, key)
      stack[#stack + 1] = { cx + 1, cy }
      stack[#stack + 1] = { cx - 1, cy }
      stack[#stack + 1] = { cx, cy + 1 }
      stack[#stack + 1] = { cx, cy - 1 }
    end
  end
end

function SpriteTools.pick(doc, x, y)
  return doc:get(x, y)
end
