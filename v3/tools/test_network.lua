-- Headless tests for Network (apps/network.lua): it reads the network info
-- at start, re-reads at most every 3 s, and redraws only when something
-- changed.

local G = GAME

local function has(list, v)
  for _, x in ipairs(list) do if x == v then return true end end
  return false
end

group("refresh")
eq({ G.host, G.ip, G.connected }, { "acid-box", "192.168.1.20", true }, "start reads the network info")
NETWORK = { "acid-box", "192.168.1.99", true }
CLOCK = 1000
G:on_idle()
eq(G.ip, "192.168.1.20", "refresh waits 3 s")
CLOCK = 3000
G:on_idle()
eq(G.ip, "192.168.1.99", "and then picks up the change")
DRAW_CALLS = 0
CLOCK = 6000
G:on_idle()
eq(DRAW_CALLS, 0, "an unchanged readout doesn't redraw")
NETWORK = { "acid-box", "none", false }
CLOCK = 9000
TEXTS = {}
G:on_idle()
ok(has(TEXTS, "no address found"), "a lost address redraws as disconnected")
