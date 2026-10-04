-- SpritePicker: the colours offered when a palette slot is edited. Rows
-- 0-3 are 12 hue-wheel steps at full, 3/4, 1/2 and 1/4 brightness; row 4
-- is 8 greys from black to white.
SpritePicker = {}
SpritePicker.COUNT = 4 * 12 + 8

function SpritePicker.shade(rgb, quarters)
  local r, g, b = rgb >> 16 & 255, rgb >> 8 & 255, rgb & 255
  return (r * quarters // 4) << 16 | (g * quarters // 4) << 8 | (b * quarters // 4)
end

-- Colour i, 0-based.
function SpritePicker.color(i)
  local col, row = i % 12, i // 12
  if row < 4 then return SpritePicker.shade(AcidPalette.hue(col, 12), 4 - row) end
  local g = col * 255 // 7
  return g << 16 | g << 8 | g
end

function SpritePicker.rect(L, i)
  local p = L.picker
  return p.x + (i % 12) * p.cw, p.y + (i // 12) * p.ch, p.cw, p.ch
end

-- The colour index under (x, y), or nil.
function SpritePicker.hit(L, x, y)
  local p = L.picker
  if x < p.x or y < p.y then return nil end
  local col, row = (x - p.x) // p.cw, (y - p.y) // p.ch
  if col >= 12 or row >= 5 then return nil end
  local i = row * 12 + col
  if i >= SpritePicker.COUNT then return nil end
  return i
end
