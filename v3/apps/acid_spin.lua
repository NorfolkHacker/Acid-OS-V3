-- Acid Spin: a spinning 3D shape drawn with the acid_mesh_* calls.
-- Left/Right (or a tap in the left/right third) change the shape, Space (or
-- a tap in the middle third) cycles wire / solid / both, Up/Down change the
-- speed. Angles are 0..255 per turn.

AcidSpin = AcidGame:extend("AcidSpin")

AcidSpin.TICK_MS = 33
AcidSpin.TITLE_BAR_H = 16
AcidSpin.BG_COLOR = 0x050607     -- THEME_BG
AcidSpin.MUTED_COLOR = 0x9DAAA3  -- THEME_MUTED
AcidSpin.SPEED_MIN = 1
AcidSpin.SPEED_MAX = 8
AcidSpin.AFTERIMAGE_PERCENT = { 25, 40, 55, 70 }  -- oldest first

local SHAPE_NAMES = { "cube", "pyramid", "octahedron", "sphere", "torus", "acid star" }
local MODE_NAMES = { [0] = "wire", [1] = "solid", [2] = "both" }

-- The acid star: a stellated octahedron. The octahedron's six points, then
-- eight spike tips (one over each face), each face replaced by three
-- triangles up to its tip. The faces follow the built-in octahedron's
-- winding so the outward side matches.
local function build_star()
  local points = {
    100, 0, 0,  -100, 0, 0,  0, 100, 0,  0, -100, 0,  0, 0, 100,  0, 0, -100,
  }
  local faces = {}
  for k = 0, 7 do
    local x, y, z = k & 1, k >> 1 & 1, k >> 2 & 1
    local tip = 7 + k
    points[#points + 1] = x == 0 and 80 or -80
    points[#points + 1] = y == 0 and 80 or -80
    points[#points + 1] = z == 0 and 80 or -80
    local a, b, c = 1 + x, 3 + y, 5 + z
    if (x + y + z) % 2 ~= 0 then b, c = c, b end
    faces[#faces + 1] = { a, b, tip }
    faces[#faces + 1] = { b, c, tip }
    faces[#faces + 1] = { c, a, tip }
  end
  return points, faces
end

function AcidSpin:on_create()
  self.meshes = {}
  for i = 1, 5 do
    self.meshes[i] = acid_mesh_builtin(SHAPE_NAMES[i])
  end
  self.meshes[6] = acid_mesh_new(build_star())
  self.shape = 1
  self.mode = 0
  self.speed = 3
  self.rx, self.ry, self.rz = 0, 0, 0
  self.step = 0
  self.touch_held = false
  self.frozen = false
  self:remember_pose()
  self:layout()
  if GAME_FREEZE or acid_launch_arg() == "freeze" then self:freeze() end
end

-- The one place the frozen pose lives: used by tests (GAME_FREEZE) and by
-- the golden-frame launch argument "freeze".
function AcidSpin:freeze()
  self.shape = 1
  self.mode = 1
  self.rx, self.ry, self.rz = 20, 30, 0
  self.step = 0
  self.frozen = true
  self:remember_pose()
end

-- The previous angles, for the wire-mode afterimages (oldest first).
function AcidSpin:remember_pose()
  self.history = {}
  for i = 1, #AcidSpin.AFTERIMAGE_PERCENT do
    self.history[i] = { self.rx, self.ry, self.rz }
  end
end

-- Everything derived from the window size. Rotated built-ins reach about
-- 174 model units (100 * sqrt(3)); size 64 is one unit per pixel, so
-- 174 * size / 64 = 45% of the smaller of the window width and the height
-- under the title bar, i.e. size = 64 * 0.45 * min / 174.
function AcidSpin:layout()
  local w, h = acid_window_size()
  self.w, self.h = w, h
  self.cx = w // 2
  self.cy = AcidSpin.TITLE_BAR_H + (h - AcidSpin.TITLE_BAR_H) // 2
  local area = math.min(w, h - AcidSpin.TITLE_BAR_H)
  self.size = math.max(1, area * 45 * 64 // (100 * 174))
end

function AcidSpin:on_resize(w, h)
  self:layout()
  self:redraw()
end

local function scaled(color, percent)
  local r = (color >> 16 & 255) * percent // 100
  local g = (color >> 8 & 255) * percent // 100
  local b = (color & 255) * percent // 100
  return (r << 16) | (g << 8) | b
end

function AcidSpin:next_shape(d)
  self.shape = (self.shape - 1 + d) % #self.meshes + 1
end

function AcidSpin:next_mode()
  self.mode = (self.mode + 1) % 3
end

function AcidSpin:change_speed(d)
  self.speed = math.max(AcidSpin.SPEED_MIN, math.min(AcidSpin.SPEED_MAX, self.speed + d))
end

function AcidSpin:on_key(code, pressed)
  if not pressed then return end
  if code == AcidKeys.RIGHT then self:next_shape(1)
  elseif code == AcidKeys.LEFT then self:next_shape(-1)
  elseif code == 32 then self:next_mode()
  elseif code == AcidKeys.UP then self:change_speed(1)
  elseif code == AcidKeys.DOWN then self:change_speed(-1)
  else return end
  self:redraw()
end

function AcidSpin:on_touch(x, y, pressed)
  -- The router repeats TOUCH every tick while held: act once per press.
  if not pressed then
    self.touch_held = false
    return
  end
  if self.touch_held then return end
  self.touch_held = true
  if x < self.w // 3 then self:next_shape(-1)
  elseif x > self.w * 2 // 3 then self:next_shape(1)
  else self:next_mode() end
  self:redraw()
end

function AcidSpin:on_tick()
  local frozen = self.frozen or GAME_FREEZE
  if not frozen then
    local hist = self.history
    table.remove(hist, 1)
    hist[#hist + 1] = { self.rx, self.ry, self.rz }
    self.rx = (self.rx + self.speed) % 256
    self.ry = (self.ry + self.speed * 2 // 3) % 256
    self.rz = (self.rz + self.speed // 2) % 256
    self.step = self.step + 2
  end
  -- Frozen, nothing moves: draw once, then again only when the focus
  -- changes (a resize, key or tap redraws by itself).
  local focused = self:focused()
  local focus_changed = focused ~= self.was_focused
  self.was_focused = focused
  if focused and (not frozen or focus_changed) then self:redraw() end
end

function AcidSpin:redraw()
  acid_clear_user_area()
  acid_draw_window_frame(self:window_title())
  local color = AcidPalette.hue(self.step)
  local id = self.meshes[self.shape]
  if id then
    if self.mode == 0 then
      for i, a in ipairs(self.history) do
        acid_mesh_draw(id, self.cx, self.cy, self.size, a[1], a[2], a[3], 0,
          scaled(color, AcidSpin.AFTERIMAGE_PERCENT[i]))
      end
    end
    acid_mesh_draw(id, self.cx, self.cy, self.size, self.rx, self.ry, self.rz, self.mode, color)
  end
  -- The label goes on last, over the shape, a line height up from the
  -- bottom so it fits at Large text too.
  local cw, ch = acid_font_size()
  local label = SHAPE_NAMES[self.shape] .. " " .. MODE_NAMES[self.mode] .. " x" .. self.speed
  acid_draw_text(label:sub(1, (self.w - 8) // cw), 4, self.h - ch - 4, AcidSpin.MUTED_COLOR, AcidSpin.BG_COLOR)
  acid_draw_window_border()
end

function AcidSpin:on_destroy()
  for _, id in ipairs(self.meshes) do acid_mesh_free(id) end
end

AcidSpin:new():start()
