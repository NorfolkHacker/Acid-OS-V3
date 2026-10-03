-- The editor's text buffer: the lines, the cursor, every mutation, and
-- undo/redo, plus the selection, the clipboard and find.
--
-- Calls no acid_* binding on purpose -- it is tables and strings and
-- nothing else, so v3/tools/test_editor.lua runs it in the test Lua state
-- with no OS underneath it. Everything that needs to draw lives in
-- editor.lua.
--
-- Positions (cx, cy, x, y, the mark, the indexes take_dirty reports) are
-- 0-based; the lines table is a 1-based sequence, so each access adds 1.
--
-- Every mutation goes through exactly two primitives: insert text at a
-- position, and delete a range. A newline is just text, so splitting and
-- joining lines are not separate operations, and undo has two record
-- types to invert rather than six.
Buffer = {}
Buffer.__index = Buffer

-- Deliberately a cap on records, not on bytes: an unbounded history in a
-- long editing session is the kind of slow leak that shows up as a
-- mysterious allocation failure hours later.
Buffer.UNDO_MAX = 200

-- Splits on "\n": every piece, trailing empties kept.
local function split_keep(text)
  local parts = {}
  for part in (text .. "\n"):gmatch("(.-)\n") do parts[#parts + 1] = part end
  return parts
end

function Buffer.new(lines)
  local self = setmetatable({}, Buffer)
  if lines == nil or #lines == 0 then lines = { "" } end
  self._lines = lines
  self.cx = 0
  self.cy = 0
  self._undo = {}
  self._redo = {}
  self._modified = false
  -- True once something has closed the current typing run, so the next
  -- inserted character starts a fresh undo record instead of joining
  -- the previous one. See insert_char.
  self._group_closed = false
  self._dirty = {}
  self._dirty_all = true
  self._mark_x = nil
  self._mark_y = nil
  self.clipboard = ""
  return self
end

function Buffer:lines()
  return self._lines
end

function Buffer:line_count()
  return #self._lines
end

function Buffer:line(i)
  return self._lines[i + 1] or ""
end

function Buffer:current_line()
  return self:line(self.cy)
end

function Buffer:modified()
  return self._modified
end

-- Called after a successful save: the text is unchanged, but it is no
-- longer different from what's on disk.
function Buffer:mark_saved()
  self._modified = false
end

-- ---- cursor ----

function Buffer:set_cursor(x, y)
  if y < 0 then y = 0 end
  if y >= #self._lines then y = #self._lines - 1 end
  if x < 0 then x = 0 end
  if x > #self:line(y) then x = #self:line(y) end
  self.cx = x
  self.cy = y
  self:end_group()
end

function Buffer:move(dx, dy)
  if dy ~= 0 then
    local ny = self.cy + dy
    if ny < 0 then ny = 0 end
    if ny >= #self._lines then ny = #self._lines - 1 end
    self.cy = ny
    -- A shorter line can't hold the old column; clamping rather than
    -- remembering the "desired" column keeps this to one rule, and the
    -- window is wide enough that the difference rarely shows.
    if self.cx > #self:current_line() then self.cx = #self:current_line() end
  end
  if dx ~= 0 then
    self.cx = self.cx + dx
    if self.cx < 0 then
      if self.cy > 0 then
        self.cy = self.cy - 1
        self.cx = #self:current_line()
      else
        self.cx = 0
      end
    elseif self.cx > #self:current_line() then
      if self.cy < #self._lines - 1 then
        self.cy = self.cy + 1
        self.cx = 0
      else
        self.cx = #self:current_line()
      end
    end
  end
  self:end_group()
end

-- ---- primitives (no undo record: the wrappers below own that, so undo
--      itself can use these to put text back without recording the
--      put-back as a fresh edit) ----

-- Returns the new cursor position, x then y.
function Buffer:raw_insert(x, y, text)
  if text == nil or #text == 0 then return x, y end
  local parts = split_keep(text)
  local src = self:line(y)
  local head = src:sub(1, x)
  local tail = src:sub(x + 1)
  if #parts == 1 then
    self._lines[y + 1] = head .. parts[1] .. tail
    self:mark_dirty(y)
    return x + #parts[1], y
  end
  self._lines[y + 1] = head .. parts[1]
  local i = 1
  while i < #parts - 1 do
    table.insert(self._lines, y + i + 1, parts[i + 1])
    i = i + 1
  end
  local last = parts[#parts]
  table.insert(self._lines, y + #parts, last .. tail)
  self:mark_dirty_all()
  return #last, y + #parts - 1
end

-- Start must not come after end. Returns the text removed, so a caller
-- can record it for undo.
function Buffer:raw_delete(sx, sy, ex, ey)
  if sy == ey then
    local src = self:line(sy)
    local text = src:sub(sx + 1, ex)
    self._lines[sy + 1] = src:sub(1, sx) .. src:sub(ex + 1)
    self:mark_dirty(sy)
    return text
  end
  local parts = { self:line(sy):sub(sx + 1) }
  local i = sy + 1
  while i < ey do
    parts[#parts + 1] = self:line(i)
    i = i + 1
  end
  parts[#parts + 1] = self:line(ey):sub(1, ex)
  self._lines[sy + 1] = self:line(sy):sub(1, sx) .. self:line(ey):sub(ex + 1)
  i = ey
  while i > sy do
    table.remove(self._lines, i + 1)
    i = i - 1
  end
  self:mark_dirty_all()
  return table.concat(parts, "\n")
end

-- ---- recording edits ----

function Buffer:insert_text(text)
  if text == nil or #text == 0 then return end
  local bx = self.cx
  local by = self.cy
  local ex, ey = self:raw_insert(bx, by, text)
  self:push_undo({ type = "ins", x = bx, y = by, text = text, cx = bx, cy = by })
  self.cx = ex
  self.cy = ey
  self._modified = true
end

function Buffer:delete_range(sx, sy, ex, ey)
  local text = self:raw_delete(sx, sy, ex, ey)
  self:push_undo({ type = "del", x = sx, y = sy, text = text, cx = self.cx, cy = self.cy })
  self.cx = sx
  self.cy = sy
  self._modified = true
  return text
end

-- A run of typed non-space characters coalesces into one undo record, so
-- undo steps back by word rather than by letter -- the difference
-- between undo being useful and being a way to watch your own typing in
-- reverse. A space, a newline, a cursor move or any other kind of edit
-- ends the run.
function Buffer:insert_char(ch)
  if ch ~= " " and self:coalescable() then
    self:raw_insert(self.cx, self.cy, ch)
    local rec = self._undo[#self._undo]
    rec.text = rec.text .. ch
    self.cx = self.cx + 1
    self._modified = true
    self._redo = {}
    return
  end
  self:insert_text(ch)
  if ch == " " then self:end_group() end
end

function Buffer:split_line()
  self:insert_text("\n")
  self:end_group()
end

function Buffer:backspace()
  if self.cx == 0 and self.cy == 0 then return end
  if self.cx > 0 then
    self:delete_range(self.cx - 1, self.cy, self.cx, self.cy)
  else
    local prev_len = #self:line(self.cy - 1)
    self:delete_range(prev_len, self.cy - 1, 0, self.cy)
  end
  self:end_group()
end

function Buffer:delete_forward()
  if self.cx < #self:current_line() then
    self:delete_range(self.cx, self.cy, self.cx + 1, self.cy)
  elseif self.cy < #self._lines - 1 then
    self:delete_range(self.cx, self.cy, 0, self.cy + 1)
  end
  self:end_group()
end

-- ---- undo ----

function Buffer:end_group()
  self._group_closed = true
end

function Buffer:undo()
  local rec = table.remove(self._undo)
  if rec == nil then return false end
  self:apply(rec, true)
  self._redo[#self._redo + 1] = rec
  self:end_group()
  return true
end

function Buffer:redo()
  local rec = table.remove(self._redo)
  if rec == nil then return false end
  self:apply(rec, false)
  self._undo[#self._undo + 1] = rec
  self:end_group()
  return true
end

-- ---- highlight cache support ----

-- Which lines' cached tokens went stale since the last call, as a list of
-- indexes or "all" when the line count itself changed (every index past
-- the edit shifted, so a list would have to name most of the file
-- anyway). Clears as it reports, the same read-and-clear shape
-- gfx_take_dirty uses in C. The indexes are 0-based.
function Buffer:take_dirty()
  local return_all = self._dirty_all
  local out = return_all and "all" or self._dirty
  self._dirty = {}
  self._dirty_all = false
  return out
end

-- ---- selection ----
--
-- A mark, not shift-and-arrow: Shift is resolved into the character at
-- translate time, so a shifted arrow is indistinguishable from a plain
-- one and shift-selection cannot be implemented at all here. Setting a
-- mark and then moving is the same idea reached by the one road that's
-- open.

function Buffer:mark_set()
  return self._mark_y ~= nil
end

function Buffer:toggle_mark()
  if self:mark_set() then
    self:clear_mark()
  else
    self._mark_x = self.cx
    self._mark_y = self.cy
  end
end

function Buffer:clear_mark()
  self._mark_x = nil
  self._mark_y = nil
end

-- { sx, sy, ex, ey } in document order, or nil when there's no mark or the
-- mark is exactly on the cursor -- so no caller has to ask which end
-- came first, and none has to special-case a zero-width span.
--
-- Clamps stale mark coordinates to keep them in range, in case the buffer
-- mutated elsewhere and left the mark invalid. Read-time clamping only; the
-- mark is not updated. This prevents selected_text and delete_selection from
-- producing nil or corrupting the buffer.
function Buffer:selection_range()
  if not self:mark_set() then return nil end
  -- Clamp mark_y into range, then clamp mark_x to that line's length
  local my = self._mark_y
  if my < 0 then my = 0 end
  if my >= #self._lines then my = #self._lines - 1 end
  local mx = self._mark_x
  if mx < 0 then mx = 0 end
  if mx > #self:line(my) then mx = #self:line(my) end
  -- Use clamped coordinates for comparison and return
  if mx == self.cx and my == self.cy then return nil end
  if my < self.cy or (my == self.cy and mx < self.cx) then
    return { mx, my, self.cx, self.cy }
  else
    return { self.cx, self.cy, mx, my }
  end
end

function Buffer:selected_text()
  local r = self:selection_range()
  if r == nil then return nil end
  local sx, sy, ex, ey = r[1], r[2], r[3], r[4]
  if sy == ey then return self:line(sy):sub(sx + 1, ex) end
  local parts = { self:line(sy):sub(sx + 1) }
  local i = sy + 1
  while i < ey do
    parts[#parts + 1] = self:line(i)
    i = i + 1
  end
  parts[#parts + 1] = self:line(ey):sub(1, ex)
  return table.concat(parts, "\n")
end

function Buffer:delete_selection()
  local r = self:selection_range()
  if r == nil then return false end
  self:clear_mark()
  self:delete_range(r[1], r[2], r[3], r[4])
  self:end_group()
  return true
end

-- ---- clipboard ----
--
-- App-local. A clipboard shared with the Terminal and File Manager would
-- be a kernel service with its own ownership and lifetime questions;
-- this is the version that earns its keep today.

function Buffer:copy()
  local t = self:selected_text()
  if t == nil then return false end
  self.clipboard = t
  return true
end

function Buffer:cut()
  if not self:copy() then return false end
  return self:delete_selection()
end

function Buffer:paste()
  if self.clipboard == nil or #self.clipboard == 0 then return false end
  if self:mark_set() then self:delete_selection() end
  self:insert_text(self.clipboard)
  self:end_group()
  return true
end

-- ---- find ----

-- Searches forward from (from_x, from_y), wrapping to the top of the
-- buffer exactly once, and returns { x, y } or nil. It wraps because a
-- query that only appears above the cursor still has to be findable --
-- scanning to the end and stopping would report "not found" for text
-- plainly on screen.
function Buffer:find(query, from_x, from_y)
  if query == nil or #query == 0 then return nil end
  local n = #self._lines
  local i = 0
  while i <= n do
    local y = (from_y + i) % n
    local start = (i == 0) and from_x or 0
    local hit = self:line(y):find(query, start + 1, true)
    if hit ~= nil then return { hit - 1, y } end
    i = i + 1
  end
  return nil
end

-- ---- private ----

function Buffer:coalescable()
  if self._group_closed then return false end
  local rec = self._undo[#self._undo]
  if rec == nil then return false end
  if rec.type ~= "ins" then return false end
  if rec.text:find("\n", 1, true) ~= nil then return false end
  return rec.y == self.cy and rec.x + #rec.text == self.cx
end

function Buffer:push_undo(rec)
  self._undo[#self._undo + 1] = rec
  if #self._undo > Buffer.UNDO_MAX then table.remove(self._undo, 1) end
  self._redo = {}
  self._group_closed = false
end

function Buffer:apply(rec, inverse)
  local x, y, text = rec.x, rec.y, rec.text
  local insert
  if rec.type == "ins" then insert = not inverse else insert = inverse end
  if insert then
    self:raw_insert(x, y, text)
  else
    local ex, ey = self:end_of(x, y, text)
    self:raw_delete(x, y, ex, ey)
  end
  if inverse then
    self:set_cursor(rec.cx, rec.cy)
  elseif rec.type == "ins" then
    local ex, ey = self:end_of(x, y, text)
    self:set_cursor(ex, ey)
  else
    self:set_cursor(x, y)
  end
  self._modified = true
end

function Buffer:end_of(x, y, text)
  if #text == 0 then return x, y end
  local parts = split_keep(text)
  if #parts == 1 then return x + #text, y end
  return #parts[#parts], y + #parts - 1
end

function Buffer:mark_dirty(i)
  for _, d in ipairs(self._dirty) do
    if d == i then return end
  end
  self._dirty[#self._dirty + 1] = i
end

function Buffer:mark_dirty_all()
  self._dirty_all = true
end
