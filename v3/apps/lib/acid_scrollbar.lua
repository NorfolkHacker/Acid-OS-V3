-- A vertical scroll bar: the maths and the drawing, nothing else. The app
-- keeps its own scroll offset (the first visible row) and, while a drag is
-- in progress, the grab point `press` returned; every function here takes
-- what it needs as arguments, so one module serves any number of lists.
--
-- Track geometry is (x, y, h): the bar's top-left corner and its height in
-- pixels. Rows are counted as `total` rows of which `visible` fit; offsets
-- run from 0 to max_offset(total, visible). Positions passed to press and
-- drag are relative to the track's top.
AcidScrollbar = {}

AcidScrollbar.WIDTH = 6
-- Below this a thumb is too small to grab, however long the list.
AcidScrollbar.MIN_THUMB = 8
AcidScrollbar.TRACK_COLOR = 0x0B1712  -- THEME_PANEL
AcidScrollbar.THUMB_COLOR = 0x00FF66  -- THEME_HARD

function AcidScrollbar.needed(total, visible)
  return total > visible
end

function AcidScrollbar.max_offset(total, visible)
  return math.max(0, total - visible)
end

local function clamp(offset, total, visible)
  return math.max(0, math.min(offset, AcidScrollbar.max_offset(total, visible)))
end

-- The thumb's top (relative to the track) and height: as tall as the share
-- of rows on screen, and as far down as the offset is through its range.
function AcidScrollbar.thumb(h, total, visible, offset)
  local th = math.min(h, math.max(AcidScrollbar.MIN_THUMB, h * visible // total))
  local max = AcidScrollbar.max_offset(total, visible)
  if max == 0 then return 0, th end
  return (h - th) * clamp(offset, total, visible) // max, th
end

-- A press at `rel_y`. On the thumb it returns the unchanged offset and the
-- grab point (to hand to drag while the press is held); above or below the
-- thumb it returns the offset one screenful up or down, and no grab.
function AcidScrollbar.press(h, total, visible, offset, rel_y)
  local ty, th = AcidScrollbar.thumb(h, total, visible, offset)
  if rel_y < ty then
    return clamp(offset - visible, total, visible), nil
  elseif rel_y >= ty + th then
    return clamp(offset + visible, total, visible), nil
  end
  return offset, rel_y - ty
end

-- The offset that puts the thumb's grab point under the pointer at `rel_y`,
-- rounded to the nearest row and clamped to the list.
function AcidScrollbar.drag(h, total, visible, grab, rel_y)
  local _, th = AcidScrollbar.thumb(h, total, visible, 0)
  local travel = h - th
  if travel <= 0 then return 0 end
  local top = math.max(0, math.min(rel_y - grab, travel))
  return clamp((top * AcidScrollbar.max_offset(total, visible) + travel // 2) // travel, total, visible)
end

-- Whether window point (px, py) is on the bar.
function AcidScrollbar.hit(x, y, h, px, py)
  return px >= x and px < x + AcidScrollbar.WIDTH and py >= y and py < y + h
end

-- Draws the track and thumb; nothing when every row already fits.
function AcidScrollbar.draw(x, y, h, total, visible, offset)
  if not AcidScrollbar.needed(total, visible) then return end
  acid_fill_rect(x, y, AcidScrollbar.WIDTH, h, AcidScrollbar.TRACK_COLOR)
  local ty, th = AcidScrollbar.thumb(h, total, visible, offset)
  acid_fill_rect(x + 1, y + ty, AcidScrollbar.WIDTH - 2, th, AcidScrollbar.THUMB_COLOR)
end
