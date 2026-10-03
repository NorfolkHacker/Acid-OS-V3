-- Config -- system-wide settings.
-- Two real knobs (master output volume via the audio global gain stage,
-- and the desktop wallpaper on/off): every other candidate "setting" is
-- either a compile-time constant no runtime code ever reads again, or has
-- no shared state to adjust -- a toggle that changes nothing when tapped is
-- worse than not having it. Add a section here only when there's a real
-- acid_* binding backing it.
ConfigApp = AcidApp:extend("ConfigApp")

ConfigApp.WINDOW_W = 180
ConfigApp.WINDOW_H = 140
ConfigApp.TITLE_BAR_H = 16

ConfigApp.TEXT_COLOR = 0xD4E6DB  -- THEME_TEXT
ConfigApp.MUTED_COLOR = 0x9DAAA3 -- THEME_MUTED
ConfigApp.BG_COLOR = 0x050607    -- THEME_BG
ConfigApp.PANEL_COLOR = 0x0B1712 -- THEME_PANEL
ConfigApp.HARD_COLOR = 0x00FF66  -- THEME_HARD

ConfigApp.BTN_SIZE = 20
ConfigApp.STEP = 10

ConfigApp.BAR_X = 4
ConfigApp.BAR_Y = 44
ConfigApp.BAR_W = 172
ConfigApp.BAR_H = 14

-- The wallpaper toggle, below the volume section (btn_y == BAR_Y + BAR_H
-- + 8 == 66, buttons end at 86 -- see redraw's own layout).
ConfigApp.WALLPAPER_LABEL_Y = 98
ConfigApp.WALLPAPER_BTN_Y = 114
ConfigApp.WALLPAPER_BTN_H = 20

function ConfigApp:on_create()
  -- Reads the kernel's actual current state rather than assuming a
  -- default -- if Config is closed and reopened (or another window, like
  -- the desktop, already changed it) this must show what's REALLY set,
  -- not silently reset it. Same reasoning for both settings below.
  self.volume = acid_get_volume()
  self.wallpaper_on = acid_get_wallpaper_enabled()
end

function ConfigApp:window_title()
  return "Config"
end

function ConfigApp:redraw()
  local C = ConfigApp
  acid_clear_user_area()
  acid_draw_window_frame(self:window_title())

  local y = C.TITLE_BAR_H + 4
  acid_draw_text("VOLUME", 4, y, C.MUTED_COLOR, C.BG_COLOR)
  local pct = self.volume .. "%"
  acid_draw_text(pct, C.WINDOW_W - 4 - #pct * 6, y, C.TEXT_COLOR, C.BG_COLOR)

  acid_fill_rect(C.BAR_X, C.BAR_Y, C.BAR_W, C.BAR_H, C.PANEL_COLOR)
  local filled = C.BAR_W * self.volume // 100
  if filled > 0 then acid_fill_rect(C.BAR_X, C.BAR_Y, filled, C.BAR_H, C.HARD_COLOR) end

  local btn_y = C.BAR_Y + C.BAR_H + 8
  self:draw_button(C.BAR_X, btn_y, "-")
  self:draw_button(C.WINDOW_W - C.BAR_X - C.BTN_SIZE, btn_y, "+")

  acid_draw_text("WALLPAPER", 4, C.WALLPAPER_LABEL_Y, C.MUTED_COLOR, C.BG_COLOR)
  self:draw_toggle()

  acid_draw_window_border()
end

function ConfigApp:draw_toggle()
  local C = ConfigApp
  local label = self.wallpaper_on and "ON" or "OFF"
  local bg = self.wallpaper_on and C.HARD_COLOR or C.PANEL_COLOR
  local fg = self.wallpaper_on and C.BG_COLOR or C.TEXT_COLOR
  acid_fill_rect(C.BAR_X, C.WALLPAPER_BTN_Y, C.BAR_W, C.WALLPAPER_BTN_H, bg)
  acid_draw_text(label, C.BAR_X + C.BAR_W // 2 - #label * 3, C.WALLPAPER_BTN_Y + 6, fg, bg)
end

function ConfigApp:draw_button(x, y, label)
  local C = ConfigApp
  acid_fill_rect(x, y, C.BTN_SIZE, C.BTN_SIZE, C.PANEL_COLOR)
  acid_draw_text(label, x + C.BTN_SIZE // 2 - 3, y + C.BTN_SIZE // 2 - 4, C.TEXT_COLOR, C.PANEL_COLOR)
end

-- The router resends a TOUCH event on every ~16ms tick for as long as the
-- mouse stays held -- without this guard, one press-and-hold on a button
-- fires the +/- step a dozen-plus times instead of once (a single click
-- on "-" jumps straight from 100% to 0%).
function ConfigApp:on_touch(x, y, pressed)
  local C = ConfigApp
  if not pressed then
    self.touch_held = false
    return
  end
  if self.touch_held then return end
  self.touch_held = true

  local btn_y = C.BAR_Y + C.BAR_H + 8
  if y >= btn_y and y < btn_y + C.BTN_SIZE then
    if x >= C.BAR_X and x < C.BAR_X + C.BTN_SIZE then
      self:set_volume(self.volume - C.STEP)
    elseif x >= C.WINDOW_W - C.BAR_X - C.BTN_SIZE and x < C.WINDOW_W - C.BAR_X then
      self:set_volume(self.volume + C.STEP)
    end
    return
  end

  if y >= C.WALLPAPER_BTN_Y and y < C.WALLPAPER_BTN_Y + C.WALLPAPER_BTN_H
      and x >= C.BAR_X and x < C.BAR_X + C.BAR_W then
    self:set_wallpaper(not self.wallpaper_on)
  end
end

function ConfigApp:set_volume(v)
  if v < 0 then v = 0 end
  if v > 100 then v = 100 end
  if v == self.volume then return end
  self.volume = v
  acid_set_volume(self.volume)
  self:redraw()
end

function ConfigApp:set_wallpaper(on)
  if on == self.wallpaper_on then return end
  self.wallpaper_on = on
  acid_set_wallpaper_enabled(self.wallpaper_on)
  self:redraw()
end

ConfigApp:new():start()
