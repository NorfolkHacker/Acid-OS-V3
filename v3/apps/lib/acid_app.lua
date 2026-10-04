-- AcidApp: the base every Acid OS app subclasses. Classes are Lua tables:
--
--   local MyApp = AcidApp:extend("MyApp")   -- the name is the class name
--   function MyApp:on_touch(x, y, pressed) ... end
--   MyApp:new():start()

AcidApp = {}
AcidApp.__index = AcidApp
AcidApp.class_name = "AcidApp"

function AcidApp:extend(class_name)
  local cls = setmetatable({}, { __index = self })
  cls.__index = cls
  cls.class_name = class_name
  return cls
end

function AcidApp:new()
  return setmetatable({}, self)
end

function AcidApp:on_create() end
function AcidApp:on_touch(x, y, pressed) end
function AcidApp:on_key(code, pressed) end
function AcidApp:on_idle() end
function AcidApp:on_destroy() end

function AcidApp:redraw()
  acid_clear_user_area()
  acid_draw_window_frame(self:window_title())
  acid_draw_window_border()
end

-- Derived from the class name so every app gets a real title with no
-- boilerplate (DemoTouchApp -> "Demo Touch"); capped at 16 chars because
-- the title isn't clipped against the close dot. Override for a custom one.
function AcidApp:window_title()
  local name = (self.class_name:gsub("App$", ""))
  name = (name:gsub("(%l)(%u)", "%1 %2"))
  return name:sub(1, 16)
end

-- True if this app's window holds keyboard focus, which is always also
-- the frontmost window (activate_window sets both together).
function AcidApp:focused()
  return acid_am_i_focused()
end

-- Ends the run loop from inside the app. The close dot and the kernel's
-- own close both arrive as a "close" event instead.
function AcidApp:quit()
  self.running = false
end

-- v3/fsroot/App is a symlink to v3/apps. Launch paths are matched by
-- exact string against the canonical v3/apps form, so a path that came
-- through the symlink is mapped back here (the hw target has no
-- realpath).
AcidApp.FSROOT_APP_PREFIX = "v3/fsroot/App/"
AcidApp.CANONICAL_APP_PREFIX = "v3/apps/"

function AcidApp:canonical_app_path(path)
  local prefix = AcidApp.FSROOT_APP_PREFIX
  if path:sub(1, #prefix) ~= prefix then return path end
  return AcidApp.CANONICAL_APP_PREFIX .. path:sub(#prefix + 1)
end

-- Called when the user resized a resizable window; re-run layout here.
function AcidApp:on_resize(w, h) end

-- How long acid_poll_event blocks when nothing arrives, and so how often
-- on_idle fires. 200 ms suits apps that only redraw on input; animating
-- apps override it with a frame interval.
function AcidApp:poll_timeout_ms()
  return 200
end

function AcidApp:start()
  self:on_create()
  self:redraw()
  self.running = true
  while self.running do
    -- Clamped: 0 would busy-spin, and a negative wait means "forever" to
    -- some hosts, so on_idle would never fire again.
    local kind, a, b, c = acid_poll_event(math.max(self:poll_timeout_ms(), 1))
    if kind == "close" then
      self.running = false
    elseif kind == "moved" then
      self:redraw()
      acid_notify_redraw_done()
    elseif kind == "resized" then
      self:on_resize(a, b)
      self:redraw()
      acid_notify_redraw_done()
    elseif kind == "key" then
      self:on_key(a, b)
    elseif kind == "touch" then
      self:on_touch(a, b, c)
    else
      self:on_idle()
    end
  end
  self:on_destroy()
end
