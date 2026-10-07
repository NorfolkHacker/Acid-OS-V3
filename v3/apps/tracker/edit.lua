-- TrkEdit: Acid Tracker's cursor and every edit, on a TrkSong. No drawing
-- here, so it is tested on its own (tools/test_trk_edit.lua). The grid
-- shows the pattern at the view's order position, all eight tracks of it;
-- typing goes into that pattern, which every order entry using it shares
-- (tracker semantics).

TrkEdit = {}
-- Cursor slots in one track's cell "C-4 01 4 22": { column, hex digit }.
TrkEdit.SLOTS = { { "note" }, { "inst", 0 }, { "inst", 1 }, { "cmd" }, { "param", 0 }, { "param", 1 } }
-- Two piano rows, GoatTracker style: semitones from C of the octave.
TrkEdit.PIANO = {
  z = 0, s = 1, x = 2, d = 3, c = 4, v = 5, g = 6, b = 7, h = 8, n = 9, j = 10, m = 11,
  q = 12, ["2"] = 13, w = 14, ["3"] = 15, e = 16, r = 17, ["5"] = 18, t = 19, ["6"] = 20,
  y = 21, ["7"] = 22, u = 23, i = 24,
}
TrkEdit.INS_FIELDS = { "wave", "a", "d", "s", "r", "duty", "pwm", "vd", "vs", "flt", "cut", "res" }
TrkEdit.FIELD_LABELS = {
  wave = "wave", a = "a", d = "d", s = "s", r = "r", duty = "duty", pwm = "pwm", vd = "vib", vs = "spd",
  flt = "flt", cut = "cut", res = "res",
}
local FILTERS = { "off", "lp", "bp", "hp" }
local FILTER_MASK = { lp = 1, bp = 2, hp = 4 }
local TRACKS = TrkSong.TRACKS

function TrkEdit.new(song)
  return setmetatable({
    song = song, ch = 1, row = 0, slot_i = 1, order = 0, octave = 4, inst = 1, edit = false,
    focus = "grid", ord_pos = 0, ord_digit = 0, ins_field = 1, dirty = false,
  }, { __index = TrkEdit })
end

-- The pattern number at the view's order position (held to the list).
function TrkEdit:pattern_num()
  local o = self.song.order
  return o[math.min(self.order, #o - 1) + 1]
end

function TrkEdit:rows() return self.song.patterns[self:pattern_num()] end
function TrkEdit:length() return #self:rows() end
function TrkEdit:order_count() return #self.song.order end

function TrkEdit:slot()
  local s = TrkEdit.SLOTS[self.slot_i]
  return { col = s[1], digit = s[2] }
end

-- The cursor's cell, after pulling the cursor inside a shorter pattern.
function TrkEdit:cell()
  local rows = self:rows()
  if self.row >= #rows then self.row = #rows - 1 end
  return rows[self.row + 1][self.ch]
end

function TrkEdit:move_row(d)
  self.row = (self.row + d) % self:length()
end

function TrkEdit:move_slot(d)
  self.slot_i = self.slot_i + d
  if self.slot_i < 1 then
    self.slot_i = #TrkEdit.SLOTS
    self.ch = (self.ch - 2) % TRACKS + 1
  elseif self.slot_i > #TrkEdit.SLOTS then
    self.slot_i = 1
    self.ch = self.ch % TRACKS + 1
  end
end

-- Tab: the tracks, then the orders panel, then the instrument.
function TrkEdit:next_focus()
  if self.focus == "grid" then
    if self.ch < TRACKS then
      self.ch = self.ch + 1
    else
      self.focus = "orders"
      self.ord_pos = math.min(self.order, self:order_count() - 1)
      self.ord_digit = 0
    end
  elseif self.focus == "orders" then
    self.focus = "ins"
  else
    self.focus = "grid"
    self.ch = 1
  end
end

-- F3: straight to the orders panel (on the entry the grid shows), or back.
function TrkEdit:toggle_orders()
  if self.focus == "orders" then
    self.focus = "grid"
  else
    self.focus = "orders"
    self.ord_pos = math.min(self.order, self:order_count() - 1)
    self.ord_digit = 0
  end
end

function TrkEdit:set_octave(d) self.octave = math.max(0, math.min(7, self.octave + d)) end
function TrkEdit:step_inst(d) self.inst = math.max(1, math.min(TrkSong.MAX_INSTRUMENT, self.inst + d)) end

function TrkEdit:set_order(o)
  self.order = math.max(0, math.min(self:order_count() - 1, o))
  self:cell()
end

-- The note a piano key plays at the current octave, or nil.
function TrkEdit:piano_note(code)
  if code < 32 or code > 126 then return nil end
  local semi = TrkEdit.PIANO[string.char(code)]
  if not semi then return nil end
  local ona = self.octave * 12 + semi - 8
  if ona < 1 or ona > 88 then return nil end
  return ona
end

local function hex_digit(code)
  if code >= 48 and code <= 57 then return code - 48 end
  if code >= 65 and code <= 70 then return code - 55 end
  if code >= 97 and code <= 102 then return code - 87 end
end

-- One key typed into the grid in edit mode: whether the song changed, and
-- a note worth previewing.
function TrkEdit:grid_key(code)
  local cell, slot = self:cell(), self:slot()
  if code == string.byte(".") or code == AcidKeys.DELETE then
    if slot.col == "note" then cell.note = 0
    elseif slot.col == "inst" then cell.inst = 0
    elseif slot.col == "cmd" then cell.cmd, cell.param = "", 0
    else cell.param = 0 end
    self:move_row(1)
    return true
  end
  if slot.col == "note" then
    if code == string.byte("`") then
      cell.note = TrkSong.NOTE_OFF
      self:move_row(1)
      return true
    end
    local ona = self:piano_note(code)
    if not ona then return false end
    cell.note = ona
    if cell.inst == 0 then cell.inst = self.inst end
    self:move_row(1)
    return true, ona
  end
  if slot.col == "cmd" then
    local ch = code >= 32 and code <= 126 and string.char(code):upper()
    if not ch or not TrkSong.COMMANDS:find(ch, 1, true) then return false end
    cell.cmd = ch
    self.slot_i = self.slot_i + 1
    return true
  end
  local v = hex_digit(code)
  if not v then return false end
  local old = cell[slot.col]
  local new = slot.digit == 0 and (v * 16 + old % 16) or (old - old % 16 + v)
  if slot.col == "inst" then new = math.min(new, TrkSong.MAX_INSTRUMENT) end
  cell[slot.col] = new
  if slot.digit == 0 then
    self.slot_i = self.slot_i + 1
  else
    self.slot_i = self.slot_i - 1
    self:move_row(1)
  end
  return true
end

-- The pattern under the cursor gets n rows (1..64), on every track at once.
function TrkEdit:set_length(n)
  n = math.max(1, math.min(TrkSong.MAX_ROWS, n))
  local rows = self:rows()
  while #rows < n do rows[#rows + 1] = TrkSong.empty_row() end
  while #rows > n do rows[#rows] = nil end
  self:cell()
end

-- Pattern n, made empty (as long as `len` rows) if it doesn't exist yet.
function TrkEdit:ensure_pattern(n, len)
  if not self.song.patterns[n] then self.song.patterns[n] = TrkSong.empty_rows(len or TrkSong.DEFAULT_ROWS) end
end

-- Points order entry `pos` (0-based) at pattern n, making it if needed.
function TrkEdit:set_entry(pos, n)
  local o = self.song.order
  self:ensure_pattern(n, #self.song.patterns[o[pos + 1]])
  o[pos + 1] = n
end

-- Inserts pattern n after the orders cursor and moves onto it.
function TrkEdit:insert_entry(n)
  table.insert(self.song.order, self.ord_pos + 2, n)
  if self.song.loop > self.ord_pos then self.song.loop = self.song.loop + 1 end
  self.ord_pos = self.ord_pos + 1
  self.ord_digit = 0
  self:set_order(self.ord_pos)
end

-- One key in the orders panel: whether the song changed, and a message.
--   Left/Right  choose an entry (the grid shows its pattern)
--   0-9 a-f     type its pattern number (a new number makes an empty pattern)
--   + -         the next or previous pattern number
--   n           a new empty pattern after this entry
--   p           a copy of this entry's pattern, as a new pattern, after it
--   Enter       repeat this entry after itself
--   Delete      remove the entry
--   l           loop back to this entry at the end of the song
function TrkEdit:orders_key(code)
  local K = AcidKeys
  local o = self.song.order
  local cur = o[self.ord_pos + 1]
  if code == K.LEFT or code == K.RIGHT then
    local d = code == K.LEFT and -1 or 1
    self.ord_pos = math.max(0, math.min(#o - 1, self.ord_pos + d))
    self.ord_digit = 0
    self:set_order(self.ord_pos)
    return false
  elseif code == string.byte("+") or code == string.byte("-") then
    local d = code == string.byte("+") and 1 or -1
    self:set_entry(self.ord_pos, (cur + d) % (TrkSong.MAX_PATTERN + 1))
    self:cell()
    return true
  elseif code == string.byte("l") then
    self.song.loop = self.ord_pos
    return true
  elseif code == K.ENTER or code == string.byte("n") or code == string.byte("p") then
    if #o >= TrkSong.MAX_ORDER then return false, "the order is full (" .. TrkSong.MAX_ORDER .. " entries)" end
    if code == K.ENTER then
      self:insert_entry(cur)
      return true
    end
    local n = TrkSong.free_pattern(self.song)
    if not n then return false, "all 64 patterns are in use (:clean drops unused ones)" end
    local rows = self.song.patterns[cur]
    self.song.patterns[n] = code == string.byte("n") and TrkSong.empty_rows(#rows) or TrkSong.copy_rows(rows)
    self:insert_entry(n)
    return true, string.format("pattern %02X", n)
  elseif code == K.DELETE then
    if #o == 1 then return false end
    table.remove(o, self.ord_pos + 1)
    if self.song.loop > self.ord_pos or self.song.loop >= #o then self.song.loop = math.max(0, self.song.loop - 1) end
    self.ord_pos = math.min(self.ord_pos, #o - 1)
    self.ord_digit = 0
    self:set_order(self.ord_pos)
    return true
  end
  local v = hex_digit(code)
  if not v then return false end
  local p = self.ord_digit == 0 and (v * 16 + cur % 16) or (cur - cur % 16 + v)
  self:set_entry(self.ord_pos, math.min(p, TrkSong.MAX_PATTERN))
  self.ord_digit = 1 - self.ord_digit
  self:cell()
  return true
end

local function cycle(list, cur, d)
  local i = 1
  for k, v in ipairs(list) do
    if v == cur then i = k end
  end
  return list[(i - 1 + d) % #list + 1]
end

function TrkEdit.field_text(ins, id)
  if id == "wave" then return TrkSong.WAVES[ins.wave + 1] end
  if id == "a" then return tostring(ins.adsr[1]) end
  if id == "d" then return tostring(ins.adsr[2]) end
  if id == "s" then return tostring(ins.adsr[3]) end
  if id == "r" then return tostring(ins.adsr[4]) end
  if id == "duty" then return tostring(ins.duty) end
  if id == "pwm" then return tostring(ins.pwm) end
  if id == "vd" then return tostring(ins.vib[1]) end
  if id == "vs" then return tostring(ins.vib[2]) end
  if id == "flt" then return ins.filter and TrkSong.FILTER_MODES[ins.filter[1]] or "off" end
  if id == "cut" then return ins.filter and tostring(ins.filter[2]) or "-" end
  if id == "res" then return ins.filter and tostring(ins.filter[3]) or "-" end
end

-- Changes one field of a built-in by d, inside acid-sound's ranges. False
-- when the field doesn't apply (cutoff or resonance with no filter).
function TrkEdit.adjust(ins, id, d)
  local step = d > 0 and 1 or -1
  if id == "wave" then
    ins.wave = (ins.wave + step) % #TrkSong.WAVES
  elseif id == "a" or id == "d" or id == "r" then
    local k = ({ a = 1, d = 2, r = 4 })[id]
    ins.adsr[k] = TrkSong.clamp(ins.adsr[k] + d, "adsr")
  elseif id == "s" then
    ins.adsr[3] = TrkSong.clamp(ins.adsr[3] + d, "sustain")
  elseif id == "duty" then
    ins.duty = TrkSong.clamp(ins.duty + d, "duty")
  elseif id == "pwm" then
    ins.pwm = TrkSong.clamp(ins.pwm + d, "pwm")
  elseif id == "vd" then
    ins.vib[1] = TrkSong.clamp(ins.vib[1] + d, "vib")
  elseif id == "vs" then
    ins.vib[2] = TrkSong.clamp(ins.vib[2] + d, "vib")
  elseif id == "flt" then
    local m = cycle(FILTERS, ins.filter and TrkSong.FILTER_MODES[ins.filter[1]] or "off", step)
    if m == "off" then
      ins.filter = nil
    else
      ins.filter = { FILTER_MASK[m], ins.filter and ins.filter[2] or 128, ins.filter and ins.filter[3] or 0 }
    end
  elseif id == "cut" or id == "res" then
    if not ins.filter then return false end
    local k = id == "cut" and 2 or 3
    ins.filter[k] = TrkSong.clamp(ins.filter[k] + d, id == "cut" and "cutoff" or "res")
  end
  return true
end

-- One key in the instrument panel: Up/Down choose a field, Left/Right
-- change it by 1, -/+ by 10. Whether the song changed.
function TrkEdit:ins_key(code)
  local K = AcidKeys
  local ins = self.song.instruments[self.inst]
  if not ins or ins.kind ~= "builtin" then return false end
  local n = #TrkEdit.INS_FIELDS
  if code == K.UP then
    self.ins_field = (self.ins_field - 2) % n + 1
    return false
  elseif code == K.DOWN then
    self.ins_field = self.ins_field % n + 1
    return false
  end
  local d = (code == K.RIGHT and 1) or (code == K.LEFT and -1)
    or (code == string.byte("+") and 10) or (code == string.byte("-") and -10)
  if not d then return false end
  return TrkEdit.adjust(ins, TrkEdit.INS_FIELDS[self.ins_field], d)
end
