-- TrkSong: Acid Tracker's in-memory .trk song and its text form. The
-- reader of record is the kernel's (acid_song_parse): the tracker only
-- reads text that has already parsed there, and `write` emits exactly the
-- canonical text acid-sound's writer does, so files round-trip byte for
-- byte. See docs/superpowers/specs/2026-10-06-acid-tracker-design.md §3.

TrkSong = {}
TrkSong.CHANNELS = 4
TrkSong.MAX_ROWS = 64
TrkSong.MAX_PATTERN = 0x7F
TrkSong.MAX_INSTRUMENT = 0x3F
TrkSong.NOTE_NONE = 0
TrkSong.NOTE_OFF = 255
TrkSong.WAVES = { "pulse", "saw", "tri", "noise" }   -- [wave + 1]
TrkSong.COMMANDS = "123489AF"
TrkSong.FILTER_MODES = { [1] = "lp", [2] = "bp", [4] = "hp" }
TrkSong.DEFAULT_ROWS = 16
-- What acid-sound's parser accepts; edits stay inside these so a saved
-- file always loads.
TrkSong.RANGE = {
  adsr = { 0, 100000 }, sustain = { 0, 100 }, duty = { 1, 99 }, pwm = { -50, 50 }, vib = { 0, 15 },
  arp = { -48, 48 }, detune = { -768, 768 }, cutoff = { 0, 255 }, res = { 0, 15 },
  speed = { 1, 31 }, donor = { 1, 4 }, transpose = { -48, 48 },
}

local NAMES = { "C-", "C#", "D-", "D#", "E-", "F-", "F#", "G-", "G#", "A-", "A#", "B-" }

function TrkSong.clamp(v, what)
  local r = TrkSong.RANGE[what]
  return math.max(r[1], math.min(r[2], v))
end

-- "C-4" -> 40 (piano key, A0 = 1), or nil.
function TrkSong.parse_note(s)
  if #s ~= 3 or not s:sub(3, 3):match("%d") then return nil end
  local semi
  for i, n in ipairs(NAMES) do
    if n == s:sub(1, 2) then semi = i - 1 end
  end
  if not semi then return nil end
  local ona = tonumber(s:sub(3, 3)) * 12 + semi - 8
  if ona < 1 or ona > 88 then return nil end
  return ona
end

-- 40 -> "C-4"; held on the keyboard.
function TrkSong.note_name(ona)
  local n = math.max(1, math.min(88, ona)) + 8
  return NAMES[n % 12 + 1] .. (n // 12)
end

function TrkSong.builtin(name)
  return { name = name, kind = "builtin", wave = 0, adsr = { 2, 40, 80, 40 }, duty = 50, pwm = 0,
           vib = { 0, 0 }, arp = {}, filter = nil, voice2 = "off", detune = 0 }
end

function TrkSong.script(name, path, block)
  return { name = name, kind = "script", path = path, block = block }
end

function TrkSong.empty_rows(n)
  local rows = {}
  for i = 1, n do rows[i] = { note = 0, inst = 0, cmd = "", param = 0, note2 = 0 } end
  return rows
end

-- A new song: one lead instrument, each channel on its own empty pattern.
function TrkSong.new()
  local s = { title = "untitled", speed = 6, donor = 4, instruments = {}, orders = {}, patterns = {} }
  s.instruments[1] = TrkSong.builtin("Lead")
  for ch = 1, TrkSong.CHANNELS do
    s.patterns[ch - 1] = TrkSong.empty_rows(TrkSong.DEFAULT_ROWS)
    s.orders[ch] = { entries = { { pattern = ch - 1, transpose = 0 } }, loop = 0 }
  end
  return s
end

-- Reading ---------------------------------------------------------------

-- Whitespace-separated fields; a "quoted" field keeps its spaces.
local function split(line)
  local out, i = {}, 1
  while i <= #line do
    local c = line:sub(i, i)
    if c == " " or c == "\t" then
      i = i + 1
    elseif c == '"' then
      local j = line:find('"', i + 1, true)
      if not j then return nil end
      out[#out + 1] = { text = line:sub(i + 1, j - 1), quoted = true }
      i = j + 1
    else
      local j = line:find("[ \t]", i) or (#line + 1)
      out[#out + 1] = { text = line:sub(i, j - 1) }
      i = j
    end
  end
  return out
end

local function hex2(s)
  if type(s) == "string" and s:match("^%x%x$") then return tonumber(s, 16) end
end

-- A whole number field, or nil (quoted, missing, or too big to hold).
local function int(f)
  if f and not f.quoted and f.text:match("^[+-]?%d+$") then return math.tointeger(tonumber(f.text)) end
end

local function parse_row(line)
  local f = {}
  for w in line:gmatch("%S+") do f[#f + 1] = w end
  if #f ~= 5 then return nil end
  local function note(s, off_ok)
    if s == "..." then return 0 end
    if s == "===" and off_ok then return TrkSong.NOTE_OFF end
    return TrkSong.parse_note(s)
  end
  local r = { note = note(f[1], true), note2 = note(f[5], false) }
  r.inst = f[2] == ".." and 0 or hex2(f[2])
  if f[3] == "." then
    r.cmd = ""
  elseif #f[3] == 1 and TrkSong.COMMANDS:find(f[3], 1, true) then
    r.cmd = f[3]
  end
  r.param = f[4] == ".." and 0 or hex2(f[4])
  if r.note and r.note2 and r.inst and r.cmd and r.param then return r end
end

local function parse_instrument(f)
  local num = f[2] and not f[2].quoted and hex2(f[2].text)
  if not num or not f[3] or not f[3].quoted then return nil end
  local name = f[3].text
  if f[4] and not f[4].quoted and f[4].text == "script" then
    if not (f[5] and f[5].quoted and f[6] and not f[7]) then return nil end
    return num, TrkSong.script(name, f[5].text, f[6].text)
  end
  local b, i, bad = TrkSong.builtin(name), 4, false
  local function nxt()
    local v = int(f[i])
    i = i + 1
    if v == nil then bad = true end
    return v
  end
  local function word()
    local w = f[i] and f[i].text
    i = i + 1
    return w
  end
  while i <= #f do
    local key = word()
    if key == "wave" then
      local w = word()
      b.wave = nil
      for k, n in ipairs(TrkSong.WAVES) do
        if n == w then b.wave = k - 1 end
      end
      if not b.wave then return nil end
    elseif key == "adsr" then
      b.adsr = { nxt(), nxt(), nxt(), nxt() }
    elseif key == "duty" then
      b.duty = nxt()
    elseif key == "pwm" then
      b.pwm = nxt()
    elseif key == "vib" then
      b.vib = { nxt(), nxt() }
    elseif key == "arp" then
      b.arp = {}
      while #b.arp < 3 and int(f[i]) do b.arp[#b.arp + 1] = nxt() end
    elseif key == "filter" then
      local mode = ({ lp = 1, bp = 2, hp = 4 })[word()]
      if not mode then return nil end
      b.filter = { mode, nxt(), nxt() }
    elseif key == "voice2" then
      local m = word()
      if m == "detune" then
        b.voice2, b.detune = "detune", nxt()
      elseif m == "off" or m == "octave" or m == "fifth" or m == "ring" then
        b.voice2 = m
      else
        return nil
      end
    else
      return nil
    end
  end
  if bad then return nil end
  return num, b
end

local function parse_order(f)
  local ch = int(f[2])
  if not ch or ch < 1 or ch > TrkSong.CHANNELS then return nil end
  local entries, i = {}, 3
  while f[i] and f[i].text ~= "loop" do
    local p, t = f[i].text:match("^(%x%x)([+-]%d+)$")
    if not p then p, t = f[i].text:match("^(%x%x)$"), "0" end
    if not p then return nil end
    entries[#entries + 1] = { pattern = tonumber(p, 16), transpose = math.tointeger(tonumber(t)) }
    i = i + 1
  end
  local loop = int(f[i + 1])
  if #entries == 0 or not loop or f[i + 2] then return nil end
  return ch, { entries = entries, loop = loop }
end

-- Text acid_song_parse accepted -> a song, or nil and why.
function TrkSong.parse(text)
  local lines = {}
  for line in (text .. "\n"):gmatch("(.-)\r?\n") do lines[#lines + 1] = line end
  local s = { title = "", speed = 6, donor = 4, instruments = {}, orders = {}, patterns = {} }
  local i = 1
  while lines[i] and lines[i]:match("^%s*$") do i = i + 1 end
  if not lines[i] or lines[i]:match("^%s*(.-)%s*$") ~= "acid-track 1" then
    return nil, "not an acid-track 1 file"
  end
  i = i + 1
  while i <= #lines do
    local n = i
    local line = lines[i]:match("^%s*(.-)%s*$")
    i = i + 1
    local word = line:match("^(%S+)")
    if line == "" or line:sub(1, 1) == "#" then
      -- skip
    elseif word == "title" then
      s.title = line:sub(6):match("^%s*(.-)%s*$")
    else
      local f = split(line)
      if not f then return nil, n .. ": bad line" end
      if word == "speed" then
        s.speed = int(f[2])
      elseif word == "sfx-donor" then
        s.donor = int(f[2])
      elseif word == "instrument" then
        local num, ins = parse_instrument(f)
        if not num then return nil, n .. ": bad instrument" end
        s.instruments[num] = ins
      elseif word == "order" then
        local ch, o = parse_order(f)
        if not ch then return nil, n .. ": bad order" end
        s.orders[ch] = o
      elseif word == "pattern" then
        local num, len = hex2(f[2] and f[2].text), int(f[3])
        if not num or not len then return nil, n .. ": bad pattern" end
        local rows = {}
        for r = 1, len do
          local row = lines[i] and parse_row(lines[i])
          if not row then return nil, i .. ": bad row" end
          rows[r] = row
          i = i + 1
        end
        s.patterns[num] = rows
      else
        return nil, n .. ": unknown line"
      end
    end
  end
  if not s.speed or not s.donor then return nil, "bad header" end
  for ch = 1, TrkSong.CHANNELS do
    if not s.orders[ch] then return nil, "no order for channel " .. ch end
  end
  return s
end

-- Writing ---------------------------------------------------------------

local function sorted_keys(t)
  local k = {}
  for n in pairs(t) do k[#k + 1] = n end
  table.sort(k)
  return k
end

-- One row as the file writes it, e.g. "C-4 01 4 22 E-4".
function TrkSong.row_text(r)
  local function note(n)
    if n == TrkSong.NOTE_NONE then return "..." end
    if n == TrkSong.NOTE_OFF then return "===" end
    return TrkSong.note_name(n)
  end
  local inst = r.inst == 0 and ".." or string.format("%02X", r.inst)
  local cmd = r.cmd == "" and "." or r.cmd
  local param = (r.cmd == "" and r.param == 0) and ".." or string.format("%02X", r.param)
  return note(r.note) .. " " .. inst .. " " .. cmd .. " " .. param .. " " .. note(r.note2)
end

-- The canonical text, exactly as acid-sound's song::write produces it.
function TrkSong.write(s)
  local out = {}
  local function add(x) out[#out + 1] = x end
  add("acid-track 1\n")
  add("title " .. s.title .. "\n")
  add("speed " .. s.speed .. "\n")
  add("sfx-donor " .. s.donor .. "\n")
  for _, num in ipairs(sorted_keys(s.instruments)) do
    local ins = s.instruments[num]
    add(string.format('instrument %02X "%s"', num, (ins.name:gsub('"', "'"))))
    if ins.kind == "script" then
      add(string.format('  script "%s" %s', ins.path, ins.block))
    else
      add(string.format("  wave %s  adsr %d %d %d %d  duty %d", TrkSong.WAVES[ins.wave + 1],
        ins.adsr[1], ins.adsr[2], ins.adsr[3], ins.adsr[4], ins.duty))
      if ins.pwm ~= 0 then add("  pwm " .. ins.pwm) end
      if ins.vib[1] ~= 0 or ins.vib[2] ~= 0 then add("  vib " .. ins.vib[1] .. " " .. ins.vib[2]) end
      if #ins.arp > 0 then
        add("  arp")
        for _, a in ipairs(ins.arp) do add(" " .. a) end
      end
      if ins.filter then
        add(string.format("  filter %s %d %d", TrkSong.FILTER_MODES[ins.filter[1]], ins.filter[2], ins.filter[3]))
      end
      if ins.voice2 == "detune" then
        add("  voice2 detune " .. ins.detune)
      elseif ins.voice2 ~= "off" then
        add("  voice2 " .. ins.voice2)
      end
    end
    add("\n")
  end
  for ch = 1, TrkSong.CHANNELS do
    local o = s.orders[ch]
    add("order " .. ch .. " ")
    for _, e in ipairs(o.entries) do
      add(string.format(" %02X", e.pattern))
      if e.transpose > 0 then
        add("+" .. e.transpose)
      elseif e.transpose < 0 then
        add(tostring(e.transpose))
      end
    end
    add(" loop " .. o.loop .. "\n")
  end
  for _, num in ipairs(sorted_keys(s.patterns)) do
    local rows = s.patterns[num]
    add(string.format("\npattern %02X %d\n", num, #rows))
    for _, r in ipairs(rows) do add(TrkSong.row_text(r) .. "\n") end
  end
  return table.concat(out)
end
