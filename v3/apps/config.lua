-- Config -- system-wide settings.
-- Three real knobs (master output volume via the audio global gain stage,
-- the desktop wallpaper on/off, and the font scale for newly opened apps),
-- plus Developer Mode (unlocks the OS's own source) and RESTART, which
-- starts the OS again at its boot screen:
-- every other candidate "setting" is either a compile-time constant no
-- runtime code ever reads again, or has no shared state to adjust -- a
-- toggle that changes nothing when tapped is worse than not having it.
-- Add a section here only when there's a real acid_* binding backing it.
ConfigApp = AcidApp:extend("ConfigApp")

ConfigApp.WINDOW_W = 180
ConfigApp.WINDOW_H = 298
ConfigApp.TITLE_BAR_H = 16

ConfigApp.TEXT_COLOR = 0xD4E6DB  -- THEME_TEXT
ConfigApp.MUTED_COLOR = 0x9DAAA3 -- THEME_MUTED
ConfigApp.BG_COLOR = 0x050607    -- THEME_BG
ConfigApp.PANEL_COLOR = 0x0B1712 -- THEME_PANEL
ConfigApp.HARD_COLOR = 0x00FF66  -- THEME_HARD
ConfigApp.ALERT_COLOR = 0xB026FF -- THEME_VIOLET -- the armed RESTART button

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

-- The font scale setting, below wallpaper (buttons 156..176).
ConfigApp.FONT_LABEL_Y = 140
ConfigApp.FONT_BTN_Y = 156
ConfigApp.FONT_BTN_H = 20

-- DEV MODE, below the font note (button 212..232): while on, the OS's own
-- source (Source, v3/apps) is writable. Off at every boot.
ConfigApp.DEV_LABEL_Y = 196
ConfigApp.DEV_BTN_Y = 212
ConfigApp.DEV_BTN_H = 20
ConfigApp.DEV_NOTE_Y = 236

-- RESTART, below DEV MODE (button 268..288). It takes two presses:
-- the first arms it, a second within RESTART_CONFIRM_MS restarts.
ConfigApp.SYSTEM_LABEL_Y = 252
ConfigApp.RESTART_BTN_Y = 268
ConfigApp.RESTART_BTN_H = 20
ConfigApp.RESTART_CONFIRM_MS = 3000

function ConfigApp:on_create()
  -- Reads the kernel's actual current state rather than assuming a
  -- default -- if Config is closed and reopened (or another window, like
  -- the desktop, already changed it) this must show what's REALLY set,
  -- not silently reset it. Same reasoning for all three settings below.
  self.volume = acid_get_volume()
  self.wallpaper_on = acid_get_wallpaper_enabled()
  self.font_scale = acid_get_font_scale()
  self.dev_mode = acid_get_dev_mode()
  self.restart_armed = false
  self.restart_failed = false
  self.armed_at = 0
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

  acid_draw_text("FONT", 4, C.FONT_LABEL_Y, C.MUTED_COLOR, C.BG_COLOR)
  self:draw_font_buttons()

  acid_draw_text("applies to newly opened apps", 4, 180, C.MUTED_COLOR, C.BG_COLOR)

  acid_draw_text("DEV MODE", 4, C.DEV_LABEL_Y, C.MUTED_COLOR, C.BG_COLOR)
  self:draw_dev()

  acid_draw_text("SYSTEM", 4, C.SYSTEM_LABEL_Y, C.MUTED_COLOR, C.BG_COLOR)
  self:draw_restart()

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

function ConfigApp:draw_dev()
  local C = ConfigApp
  local label = self.dev_mode and "ON" or "OFF"
  local bg = self.dev_mode and C.HARD_COLOR or C.PANEL_COLOR
  local fg = self.dev_mode and C.BG_COLOR or C.TEXT_COLOR
  acid_fill_rect(C.BAR_X, C.DEV_BTN_Y, C.BAR_W, C.DEV_BTN_H, bg)
  acid_draw_text(label, C.BAR_X + C.BAR_W // 2 - #label * 3, C.DEV_BTN_Y + 6, fg, bg)
  local note = self.dev_mode and "system source is writable" or "system source is read-only"
  acid_draw_text(note, 4, C.DEV_NOTE_Y, self.dev_mode and C.HARD_COLOR or C.MUTED_COLOR, C.BG_COLOR)
end

function ConfigApp:set_dev(on)
  if acid_set_dev_mode(on) then self.dev_mode = on end
  self:redraw()
end

function ConfigApp:draw_font_buttons()
  local C = ConfigApp
  local BTN_W = 84

  -- NORMAL button
  local normal_bg = self.font_scale == 1 and C.HARD_COLOR or C.PANEL_COLOR
  local normal_fg = self.font_scale == 1 and C.BG_COLOR or C.TEXT_COLOR
  acid_fill_rect(C.BAR_X, C.FONT_BTN_Y, BTN_W, C.FONT_BTN_H, normal_bg)
  acid_draw_text("NORMAL", C.BAR_X + BTN_W // 2 - 18, C.FONT_BTN_Y + 6, normal_fg, normal_bg)

  -- LARGE button
  local large_bg = self.font_scale == 2 and C.HARD_COLOR or C.PANEL_COLOR
  local large_fg = self.font_scale == 2 and C.BG_COLOR or C.TEXT_COLOR
  acid_fill_rect(92, C.FONT_BTN_Y, BTN_W, C.FONT_BTN_H, large_bg)
  acid_draw_text("LARGE", 92 + BTN_W // 2 - 15, C.FONT_BTN_Y + 6, large_fg, large_bg)
end

function ConfigApp:draw_restart()
  local C = ConfigApp
  local label, bg, fg = "RESTART", C.PANEL_COLOR, C.TEXT_COLOR
  if self.restart_armed then
    label, bg, fg = "SURE? PRESS AGAIN", C.ALERT_COLOR, C.BG_COLOR
  elseif self.restart_failed then
    label, fg = "RESTART FAILED", C.MUTED_COLOR
  end
  acid_fill_rect(C.BAR_X, C.RESTART_BTN_Y, C.BAR_W, C.RESTART_BTN_H, bg)
  acid_draw_text(label, C.BAR_X + C.BAR_W // 2 - #label * 3, C.RESTART_BTN_Y + 6, fg, bg)
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

  if y >= C.RESTART_BTN_Y and y < C.RESTART_BTN_Y + C.RESTART_BTN_H
      and x >= C.BAR_X and x < C.BAR_X + C.BAR_W then
    self:press_restart()
    return
  end
  -- A press anywhere else calls off an armed RESTART, and clears a
  -- RESTART FAILED.
  if self.restart_armed or self.restart_failed then
    self.restart_armed = false
    self.restart_failed = false
    self:redraw()
  end

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
    return
  end

  if y >= C.FONT_BTN_Y and y < C.FONT_BTN_Y + C.FONT_BTN_H then
    if x >= C.BAR_X and x < C.BAR_X + 84 then
      self:set_font(1)
    elseif x >= 92 and x < 92 + 84 then
      self:set_font(2)
    end
  end

  if y >= C.DEV_BTN_Y and y < C.DEV_BTN_Y + C.DEV_BTN_H
      and x >= C.BAR_X and x < C.BAR_X + C.BAR_W then
    self:set_dev(not self.dev_mode)
  end
end

-- The first press arms RESTART; a second, while it's still armed,
-- restarts. on_idle can be starved by held touches, so an armed button
-- older than RESTART_CONFIRM_MS counts as disarmed here too. On success acid_restart doesn't return: the OS starts again at
-- its boot screen.
function ConfigApp:press_restart()
  local stale = self.restart_armed
    and acid_now_ms() - self.armed_at >= ConfigApp.RESTART_CONFIRM_MS
  if not self.restart_armed or stale then
    self.restart_armed = true
    self.restart_failed = false
    self.armed_at = acid_now_ms()
  else
    self.restart_armed = false
    self.restart_failed = not acid_restart()
  end
  self:redraw()
end

-- An armed RESTART left alone for RESTART_CONFIRM_MS disarms itself.
function ConfigApp:on_idle()
  if self.restart_armed and acid_now_ms() - self.armed_at >= ConfigApp.RESTART_CONFIRM_MS then
    self.restart_armed = false
    self:redraw()
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

function ConfigApp:set_font(scale)
  -- Guard on the live setting: a cart may have changed it since we drew.
  if scale == acid_get_font_scale() then return end
  self.font_scale = scale
  acid_set_font_scale(self.font_scale)
  self:redraw()
end

ConfigApp:new():start()
