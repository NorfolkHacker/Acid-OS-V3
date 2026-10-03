-- AcidPalette: a vivid hue wheel for app *content* (bars, items, effects),
-- so apps aren't limited to the few chrome colours. `//` is floor
-- division and Lua's `%` floors too, so negative steps wrap.

AcidPalette = {}

function AcidPalette.hue(step, steps)
  steps = steps or 256
  local h = (step % steps) * 360 // steps
  local sector = h // 60
  local f = h % 60
  local rise = 255 * f // 60
  local fall = 255 - rise
  local r, g, b
  if sector == 0 then r, g, b = 255, rise, 0
  elseif sector == 1 then r, g, b = fall, 255, 0
  elseif sector == 2 then r, g, b = 0, 255, rise
  elseif sector == 3 then r, g, b = 0, fall, 255
  elseif sector == 4 then r, g, b = rise, 0, 255
  else r, g, b = 255, 0, fall
  end
  return (r << 16) | (g << 8) | b
end
