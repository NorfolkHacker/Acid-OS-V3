-- Network -- this device's real hostname and LAN address. There is
-- nothing to configure here: Acid OS has no network stack of its own yet,
-- so this is a status readout, not a settings panel -- see Config for the one app that changes something.
-- Re-reads on a small timer since the underlying address can change (DHCP
-- renewal, cable unplugged) without this app itself doing anything.
NetworkApp = AcidApp:extend("NetworkApp")

local CW, CH = acid_font_size()

NetworkApp.TITLE_BAR_H = 16
NetworkApp.LINE_H = CH + 4
NetworkApp.REFRESH_MS = 3000

NetworkApp.BG_COLOR = 0x050607    -- THEME_BG
NetworkApp.TEXT_COLOR = 0xD4E6DB  -- THEME_TEXT
NetworkApp.MUTED_COLOR = 0x9DAAA3 -- THEME_MUTED
NetworkApp.HARD_COLOR = 0x00FF66  -- THEME_HARD

-- Everything derived from the window size lives here, so a resize can
-- redo it: the columns text is cut to, and the gap between the sections,
-- which closes up when the window is too short for it.
function NetworkApp:layout()
  local ww, wh = acid_window_size()
  NetworkApp.COLS = (ww - 20) // CW
  local roomy = NetworkApp.TITLE_BAR_H + 6 + 4 * NetworkApp.LINE_H + 8 + CH <= wh
  self.gap = roomy and 4 or 0
end

function NetworkApp:on_resize(w, h)
  self:layout()
end

function NetworkApp:on_create()
  self:layout()
  self.host = "?"
  self.ip = "?"
  self.connected = false
  self.next_refresh_at = 0
  self:refresh()
end

function NetworkApp:window_title()
  return "Network"
end

function NetworkApp:on_idle()
  self:refresh()
end

function NetworkApp:refresh()
  local now = acid_now_ms()
  if now < self.next_refresh_at then return end
  self.next_refresh_at = now + NetworkApp.REFRESH_MS
  local host, ip, connected = acid_network_info()
  local changed = host ~= self.host or ip ~= self.ip or connected ~= self.connected
  self.host = host
  self.ip = ip
  self.connected = connected
  if changed then self:redraw() end
end

function NetworkApp:redraw()
  acid_clear_user_area()
  acid_draw_window_frame(self:window_title())

  local y = NetworkApp.TITLE_BAR_H + 6
  local dot_color = self.connected and NetworkApp.HARD_COLOR or NetworkApp.MUTED_COLOR
  acid_fill_rect(4, y + (CH - 4) // 2, 6, 6, dot_color)
  acid_draw_text(self.connected and "connected" or "no address found", 14, y,
    self.connected and NetworkApp.TEXT_COLOR or NetworkApp.MUTED_COLOR, NetworkApp.BG_COLOR)
  y = y + NetworkApp.LINE_H + self.gap

  acid_draw_text("HOST", 4, y, NetworkApp.MUTED_COLOR, NetworkApp.BG_COLOR)
  y = y + NetworkApp.LINE_H
  acid_draw_text(self.host:sub(1, NetworkApp.COLS), 4, y, NetworkApp.TEXT_COLOR, NetworkApp.BG_COLOR)
  y = y + NetworkApp.LINE_H + self.gap

  acid_draw_text("IP", 4, y, NetworkApp.MUTED_COLOR, NetworkApp.BG_COLOR)
  y = y + NetworkApp.LINE_H
  acid_draw_text(self.ip:sub(1, NetworkApp.COLS), 4, y, NetworkApp.TEXT_COLOR, NetworkApp.BG_COLOR)

  acid_draw_window_border()
end

NetworkApp:new():start()
