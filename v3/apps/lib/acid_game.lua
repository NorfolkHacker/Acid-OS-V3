-- AcidGame: the base for games that run on a fixed tick rather than only
-- reacting to events. on_tick fires every TICK_MS milliseconds, paced by
-- acid_now_ms; events are delivered in between. If a tick runs late, the
-- schedule resets from now instead of firing a burst of catch-up ticks.
-- The loop keeps its own `running`, so quit() does not end a game; only a
-- close event does.

AcidGame = AcidApp:extend("AcidGame")
AcidGame.TICK_MS = 50

function AcidGame:on_tick() end

function AcidGame:start()
  self:on_create()
  local running = true
  local tick = self.TICK_MS
  local next_tick_at = acid_now_ms() + tick
  while running do
    local remaining = next_tick_at - acid_now_ms()
    if remaining < 0 then remaining = 0 end
    local kind, a, b, c = acid_poll_event(remaining)
    if kind == "close" then
      running = false
    elseif kind == "moved" then
      acid_notify_redraw_done()
    elseif kind == "resized" then
      self:on_resize(a, b)
    elseif kind == "key" then
      self:on_key(a, b)
    elseif kind == "touch" then
      self:on_touch(a, b, c)
    end
    if running and acid_now_ms() >= next_tick_at then
      self:on_tick()
      next_tick_at = next_tick_at + tick
      if next_tick_at < acid_now_ms() then
        next_tick_at = acid_now_ms() + tick
      end
    end
  end
  self:on_destroy()
end
