-- TrkEdit: Acid Tracker's cursor and every edit, on a TrkSong. No drawing
-- here, so it is tested on its own (tools/test_trk_edit.lua). The grid
-- shows each channel's pattern at the view's order position; typing goes
-- into the cursor channel's pattern, which every order entry using that
-- pattern shares (tracker semantics).

TrkEdit = {}
-- Cursor slots in one channel's row "C-4 01 4 22 E-4": { column, hex digit }.
TrkEdit.SLOTS = { { "note" }, { "inst", 0 }, { "inst", 1 }, { "cmd" }, { "param", 0 }, { "param", 1 }, { "note2" } }
-- Two piano rows, GoatTracker style: semitones from C of the octave.
TrkEdit.PIANO = {
  z = 0, s = 1, x = 2, d = 3, c = 4, v = 5, g = 6, b = 7, h = 8, n = 9, j = 10, m = 11,
  q = 12, ["2"] = 13, w = 14, ["3"] = 15, e = 16, r = 17, ["5"] = 18, t = 19, ["6"] = 20,
  y = 21, ["7"] = 22, u = 23, i = 24,
}
TrkEdit.MAX_ENTRIES = 64
TrkEdit.INS_FIELDS = { "wave", "a", "d", "s", "r", "duty", "pwm", "vd", "vs", "flt", "cut", "res", "v2", "det" }
TrkEdit.FIELD_LABELS = {
  wave = "wave", a = "a", d = "d", s = "s", r = "r", duty = "duty", pwm = "pwm", vd = "vib", vs = "spd",
  flt = "flt", cut = "cut", res = "res", v2 = "v2", det = "det",
}
local FILTERS = { "off", "lp", "bp", "hp" }
local FILTER_MASK = { lp = 1, bp = 2, hp = 4 }
local VOICE2 = { "off", "detune", "octave", "fifth", "ring" }

function TrkEdit.new(song)
  return setmetatable({
    song = song, ch = 1, row = 0, slot_i = 1, order = 0, octave = 4, inst = 1, edit = false,
    focus = "grid", ord_ch = 1, ord_pos = 0, ord_digit = 0, ins_field = 1, dirty = false,
  }, { __index = TrkEdit })
end

-- The order entry a channel shows at the view's position (held to its list).
function TrkEdit:entry(ch)
  local es = self.song.orders[ch].entries
  return es[math.min(self.order, #es - 1) + 1]
end

function TrkEdit:rows(ch) return self.song.patterns[self:entry(ch).pattern] end

function TrkEdit:max_rows()
  local m = 0
  for ch = 1, TrkSong.CHANNELS do m = math.max(m, #self:rows(ch)) end
  return m
end

function TrkEdit:order_count()
  local m = 0
  for ch = 1, TrkSong.CHANNELS do m = math.max(m, #self.song.orders[ch].entries) end
  return m
end

function TrkEdit:slot()
  local s = TrkEdit.SLOTS[self.slot_i]
  return { col = s[1], digit = s[2] }
end

-- The cursor's row, after pulling the cursor inside a shorter pattern.
function TrkEdit:cell()
  local rows = self:rows(self.ch)
  if self.row >= #rows then self.row = #rows - 1 end
  return rows[self.row + 1]
end

function TrkEdit:move_row(d)
  self.row = (self.row + d) % #self:rows(self.ch)
end

function TrkEdit:move_slot(d)
  self.slot_i = self.slot_i + d
  if self.slot_i < 1 then
    self.slot_i = #TrkEdit.SLOTS
    self.ch = (self.ch - 2) % TrkSong.CHANNELS + 1
  elseif self.slot_i > #TrkEdit.SLOTS then
    self.slot_i = 1
    self.ch = self.ch % TrkSong.CHANNELS + 1
  end
  self:cell()
end

-- Tab: the channels, then the orders panel, then the instrument.
function TrkEdit:next_focus()
  if self.focus == "grid" then
    if self.ch < TrkSong.CHANNELS then
      self.ch = self.ch + 1
      self:cell()
    else
      self.focus = "orders"
    end
  elseif self.focus == "orders" then
    self.focus = "ins"
  else
    self.focus = "grid"
    self.ch = 1
    self:cell()
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
    elseif slot.col == "param" then cell.param = 0
    else cell.note2 = 0 end
    self:move_row(1)
    return true
  end
  if slot.col == "note" or slot.col == "note2" then
    if slot.col == "note" and code == string.byte("`") then
      cell.note = TrkSong.NOTE_OFF
      self:move_row(1)
      return true
    end
    local ona = self:piano_note(code)
    if not ona then return false end
    if slot.col == "note" then
      cell.note = ona
      if cell.inst == 0 then cell.inst = self.inst end
    else
      cell.note2 = ona
    end
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

function TrkEdit:set_length(n)
  n = math.max(1, math.min(TrkSong.MAX_ROWS, n)) -- the file format's 1..64
  local rows = self:rows(self.ch)
  while #rows < n do rows[#rows + 1] = { note = 0, inst = 0, cmd = "", param = 0, note2 = 0 } end
  while #rows > n do rows[#rows] = nil end
  self:cell()
end

function TrkEdit:ensure_pattern(n)
  if not self.song.patterns[n] then self.song.patterns[n] = TrkSong.empty_rows(TrkSong.DEFAULT_ROWS) end
end

function TrkEdit:clamp_ord()
  self.ord_pos = math.min(self.ord_pos, #self.song.orders[self.ord_ch].entries - 1)
  self.ord_digit = 0
end

-- One key in the orders panel: whether the song changed.
function TrkEdit:orders_key(code)
  local K = AcidKeys
  local o = self.song.orders[self.ord_ch]
  local e = o.entries[self.ord_pos + 1]
  if code == K.UP then
    self.ord_ch = (self.ord_ch - 2) % TrkSong.CHANNELS + 1
    self:clamp_ord()
    return false
  elseif code == K.DOWN then
    self.ord_ch = self.ord_ch % TrkSong.CHANNELS + 1
    self:clamp_ord()
    return false
  elseif code == K.LEFT or code == K.RIGHT then
    local d = code == K.LEFT and -1 or 1
    self.ord_pos = math.max(0, math.min(#o.entries - 1, self.ord_pos + d))
    self.ord_digit = 0
    self:set_order(self.ord_pos)
    return false
  elseif code == string.byte("+") or code == string.byte("-") then
    local d = code == string.byte("+") and 1 or -1
    e.transpose = TrkSong.clamp(e.transpose + d, "transpose")
    return true
  elseif code == string.byte("l") then
    o.loop = self.ord_pos
    return true
  elseif code == K.ENTER then
    if #o.entries >= TrkEdit.MAX_ENTRIES then return false end
    table.insert(o.entries, self.ord_pos + 2, { pattern = e.pattern, transpose = e.transpose })
    self.ord_pos = self.ord_pos + 1
    return true
  elseif code == K.DELETE then
    if #o.entries == 1 then return false end
    table.remove(o.entries, self.ord_pos + 1)
    if o.loop >= #o.entries then o.loop = #o.entries - 1 end
    self:clamp_ord()
    return true
  end
  local v = hex_digit(code)
  if not v then return false end
  local p = self.ord_digit == 0 and (v * 16 + e.pattern % 16) or (e.pattern - e.pattern % 16 + v)
  e.pattern = math.min(p, TrkSong.MAX_PATTERN)
  self:ensure_pattern(e.pattern)
  self.ord_digit = 1 - self.ord_digit
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
  if id == "v2" then return ins.voice2 end
  if id == "det" then return ins.voice2 == "detune" and tostring(ins.detune) or "-" end
end

-- Changes one field of a built-in by d, inside acid-sound's ranges. False
-- when the field doesn't apply (cutoff with no filter, detune unless
-- voice 2 detunes).
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
  elseif id == "v2" then
    ins.voice2 = cycle(VOICE2, ins.voice2, step)
  elseif id == "det" then
    if ins.voice2 ~= "detune" then return false end
    ins.detune = TrkSong.clamp(ins.detune + d, "detune")
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
