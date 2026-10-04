-- SpriteDoc: the sprite Sprite Paint is editing, which frame is current,
-- and the undo history. Pure data: no drawing and no file I/O.
-- The sprite is the table AcidSprite.parse returns. Cell coordinates are
-- 0-based; frames and doc.frame are 1-based like Lua lists.
SpriteDoc = {}
SpriteDoc.__index = SpriteDoc
SpriteDoc.HISTORY = 32

-- 0 black, 1-9 and a-c twelve steps round the hue wheel, d/e greys, f white.
function SpriteDoc.default_palette()
  local p = { ["0"] = 0x000000, d = 0x555555, e = 0xAAAAAA, f = 0xFFFFFF }
  for i = 1, 12 do
    p[AcidSprite.KEYS:sub(i + 1, i + 1)] = AcidPalette.hue(i - 1, 12)
  end
  return p
end

local function blank_rows(w, h)
  local rows = {}
  for y = 1, h do rows[y] = string.rep(".", w) end
  return rows
end

local function copy_list(t)
  local c = {}
  for i, v in ipairs(t) do c[i] = v end
  return c
end

local function copy_map(t)
  local c = {}
  for k, v in pairs(t) do c[k] = v end
  return c
end

-- Wraps a parsed sprite. Palette keys the file left out get their default
-- colour, so all 16 swatches always have one (and are all saved).
function SpriteDoc.new(s)
  for k, v in pairs(SpriteDoc.default_palette()) do
    if s.palette[k] == nil then s.palette[k] = v end
  end
  return setmetatable({
    s = s, frame = 1, undo_stack = {}, redo_stack = {},
    dirty = false, changed = {}, full = true,
  }, SpriteDoc)
end

function SpriteDoc.blank(w, h)
  return SpriteDoc.new({ w = w, h = h, fps = AcidSprite.DEFAULT_FPS, palette = {}, frames = { blank_rows(w, h) } })
end

function SpriteDoc:rows() return self.s.frames[self.frame] end
function SpriteDoc:frame_count() return #self.s.frames end

-- The key at a cell of the current frame, or nil off the sprite.
function SpriteDoc:get(x, y)
  if x < 0 or y < 0 or x >= self.s.w or y >= self.s.h then return nil end
  return self:rows()[y + 1]:sub(x + 1, x + 1)
end

-- Sets a cell of the current frame; off the sprite, or no change, does
-- nothing. A real change is recorded for take_changes.
function SpriteDoc:set(x, y, key)
  local old = self:get(x, y)
  if old == nil or old == key then return end
  local rows = self:rows()
  local row = rows[y + 1]
  rows[y + 1] = row:sub(1, x) .. key .. row:sub(x + 2)
  self.changed[#self.changed + 1] = { x, y }
end

-- The cells changed since the last call, as {x, y} pairs.
function SpriteDoc:take_changes()
  local c = self.changed
  self.changed = {}
  return c
end

-- Whether something changed that needs the whole canvas redrawn (a frame
-- switch, a palette edit, undo/redo) since the last call.
function SpriteDoc:take_full()
  local f = self.full
  self.full = false
  return f
end

function SpriteDoc:frame_next()
  self.frame = self.frame % #self.s.frames + 1
  self.full = true
end

function SpriteDoc:frame_prev()
  self.frame = (self.frame - 2) % #self.s.frames + 1
  self.full = true
end

-- Undo snapshots hold every frame's row list (the row strings are shared,
-- not copied), the current frame, the palette and fps.
function SpriteDoc:snapshot()
  local frames = {}
  for i, rows in ipairs(self.s.frames) do frames[i] = copy_list(rows) end
  return { frames = frames, frame = self.frame, palette = copy_map(self.s.palette), fps = self.s.fps }
end

local function same(a, b)
  if a.fps ~= b.fps or #a.frames ~= #b.frames then return false end
  for i, rows in ipairs(a.frames) do
    for y, r in ipairs(rows) do
      if b.frames[i][y] ~= r then return false end
    end
  end
  for k, v in pairs(a.palette) do
    if b.palette[k] ~= v then return false end
  end
  for k in pairs(b.palette) do
    if a.palette[k] == nil then return false end
  end
  return true
end

-- Puts a snapshot back. The snapshot is off its stack by now, so its
-- tables are taken over rather than copied.
function SpriteDoc:restore(snap)
  self.s.frames = snap.frames
  self.s.palette = snap.palette
  self.s.fps = snap.fps
  self.frame = snap.frame
  self.changed = {}
  self.full = true
  self.dirty = true
end

local function push_capped(stack, snap)
  stack[#stack + 1] = snap
  if #stack > SpriteDoc.HISTORY then table.remove(stack, 1) end
end

-- begin_step/end_step bracket one undoable action. end_step returns
-- whether it was a step: an action that changed nothing isn't one.
function SpriteDoc:begin_step()
  self.pending = self:snapshot()
end

function SpriteDoc:end_step()
  local before = self.pending
  self.pending = nil
  if before == nil or same(before, self:snapshot()) then return false end
  push_capped(self.undo_stack, before)
  self.redo_stack = {}
  self.dirty = true
  return true
end

function SpriteDoc:undo()
  local snap = table.remove(self.undo_stack)
  if not snap then return false end
  push_capped(self.redo_stack, self:snapshot())
  self:restore(snap)
  return true
end

function SpriteDoc:redo()
  local snap = table.remove(self.redo_stack)
  if not snap then return false end
  push_capped(self.undo_stack, self:snapshot())
  self:restore(snap)
  return true
end

-- Frame edits are each one step, and refuse past the limits.
function SpriteDoc:frame_add()
  if #self.s.frames >= AcidSprite.MAX_FRAMES then return false end
  self:begin_step()
  table.insert(self.s.frames, self.frame + 1, blank_rows(self.s.w, self.s.h))
  self.frame = self.frame + 1
  self.full = true
  return self:end_step()
end

function SpriteDoc:frame_dup()
  if #self.s.frames >= AcidSprite.MAX_FRAMES then return false end
  self:begin_step()
  table.insert(self.s.frames, self.frame + 1, copy_list(self:rows()))
  self.frame = self.frame + 1
  self.full = true
  return self:end_step()
end

function SpriteDoc:frame_del()
  if #self.s.frames <= 1 then return false end
  self:begin_step()
  table.remove(self.s.frames, self.frame)
  if self.frame > #self.s.frames then self.frame = #self.s.frames end
  self.full = true
  return self:end_step()
end

function SpriteDoc:set_color(key, rgb)
  self:begin_step()
  self.s.palette[key] = rgb
  self.full = true
  return self:end_step()
end

function SpriteDoc:set_fps(n)
  self:begin_step()
  self.s.fps = math.max(1, math.min(AcidSprite.MAX_FPS, n))
  return self:end_step()
end

function SpriteDoc:mark_saved()
  self.dirty = false
end
