-- Network -- this device's real hostname and LAN address. There is
-- nothing to configure here: Acid OS has no network stack of its own yet,
-- so this is a status readout, not a settings panel -- see Config for the one app that changes something.
-- Re-reads on a small timer since the underlying address can change (DHCP
-- renewal, cable unplugged) without this app itself doing anything.
NetworkApp = AcidApp:extend("NetworkApp")

NetworkApp.WINDOW_W = 200
NetworkApp.WINDOW_H = 110
NetworkApp.TITLE_BAR_H = 16
NetworkApp.LINE_H = 12
NetworkApp.REFRESH_MS = 3000

NetworkApp.BG_COLOR = 0x050607    -- THEME_BG
NetworkApp.TEXT_COLOR = 0xD4E6DB  -- THEME_TEXT
NetworkApp.MUTED_COLOR = 0x9DAAA3 -- THEME_MUTED
NetworkApp.HARD_COLOR = 0x00FF66  -- THEME_HARD

function NetworkApp:on_create()
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
  acid_fill_rect(4, y + 2, 6, 6, dot_color)
  acid_draw_text(self.connected and "connected" or "no address found", 14, y,
    self.connected and NetworkApp.TEXT_COLOR or NetworkApp.MUTED_COLOR, NetworkApp.BG_COLOR)
  y = y + NetworkApp.LINE_H + 4

  acid_draw_text("HOST", 4, y, NetworkApp.MUTED_COLOR, NetworkApp.BG_COLOR)
  y = y + NetworkApp.LINE_H
  acid_draw_text(self.host:sub(1, 30), 4, y, NetworkApp.TEXT_COLOR, NetworkApp.BG_COLOR)
  y = y + NetworkApp.LINE_H + 4

  acid_draw_text("IP", 4, y, NetworkApp.MUTED_COLOR, NetworkApp.BG_COLOR)
  y = y + NetworkApp.LINE_H
  acid_draw_text(self.ip:sub(1, 30), 4, y, NetworkApp.TEXT_COLOR, NetworkApp.BG_COLOR)

  acid_draw_window_border()
end

NetworkApp:new():start()
