-- name: Hello Acid
-- w: 200
-- h: 150
-- desc: Example cart -- colour-cycling bars
-- libs: lib/acid_palette.lua
--
-- The header above is the cart format; Phase 5's cart loader reads it.
-- The rest is an ordinary Acid OS app.

local HelloAcidApp = AcidApp:extend("HelloAcidApp")

-- Must match the manifest's w/h.
local WINDOW_W = 200
local WINDOW_H = 150
local TITLE_BAR_H = 16
local BAR_H = 8
local TEXT_Y = 70

function HelloAcidApp:on_create()
  self.step = 0
end

-- ~25 fps while focused; the base class fires on_idle at this interval.
function HelloAcidApp:poll_timeout_ms()
  return 40
end

function HelloAcidApp:on_idle()
  if not self:focused() then return end
  self.step = self.step + 3
  self:redraw()
end

function HelloAcidApp:redraw()
  acid_clear_user_area()
  acid_draw_window_frame(self:window_title())
  local y = TITLE_BAR_H
  local row = 0
  while y < WINDOW_H do
    local h = BAR_H
    if y + BAR_H > WINDOW_H then h = WINDOW_H - y end
    acid_fill_rect(0, y, WINDOW_W, h, AcidPalette.hue(self.step + row * 12))
    y = y + BAR_H
    row = row + 1
  end
  local label_bg = AcidPalette.hue(self.step + ((TEXT_Y - TITLE_BAR_H) // BAR_H) * 12)
  acid_fill_rect(0, TEXT_Y, WINDOW_W, BAR_H, label_bg)
  acid_draw_text("HELLO FROM A CART", 36, TEXT_Y, 0x050607, label_bg)
  acid_draw_window_border()
end

HelloAcidApp:new():start()
