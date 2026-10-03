-- AcidSprite: pixel sprites drawn from picture-strings. Each row is a
-- string; each character is a palette key, '.' is transparent. Same-colour
-- horizontal runs merge into one rect (one overlay call instead of many),
-- and flip mirrors each row.
-- Draws on the overlay, so the caller must own it (acid_overlay_open).

AcidSprite = {}

function AcidSprite.width(rows)
  return #rows[1]
end

function AcidSprite.height(rows)
  return #rows
end

function AcidSprite.draw(rows, x, y, scale, palette, flip)
  for r = 1, #rows do
    AcidSprite.draw_row(rows[r], x, y + (r - 1) * scale, scale, palette, flip)
  end
end

function AcidSprite.draw_row(row, x, y, scale, palette, flip)
  local len = #row
  -- Column c (0-based) as seen after an optional mirror.
  local function at(c)
    if flip then c = len - 1 - c end
    return row:sub(c + 1, c + 1)
  end
  local c = 0
  while c < len do
    local ch = at(c)
    local color = nil
    if ch ~= "." then color = palette[ch] end
    if color == nil then
      c = c + 1
    else
      local run = 1
      while c + run < len and at(c + run) == ch do
        run = run + 1
      end
      acid_overlay_fill_rect(x + c * scale, y, run * scale, scale, color)
      c = c + run
    end
  end
end
