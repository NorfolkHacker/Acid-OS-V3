-- TrkSong: Acid Tracker's in-memory .trk song and its text form. The
-- reader of record is the kernel's (acid_song_parse): the tracker only
-- reads text that has already parsed there, and `write` emits exactly the
-- canonical text acid-sound's writer does, so files round-trip byte for
-- byte. A song is one order list of patterns; every pattern holds all
-- TRACKS tracks, so its length is every track's length.
--
-- The model: { title, speed, instruments = { [n] = ins }, order = { pattern
-- numbers }, loop = 0-based entry, patterns = { [n] = rows } }, where each
-- row is a list of TRACKS cells { note, inst, cmd, param }.

TrkSong = {}
TrkSong.TRACKS = 8
TrkSong.MAX_ROWS = 64
TrkSong.MAX_PATTERN = 0x3F        -- 64 patterns, 00..3F
TrkSong.MAX_ORDER = 128
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
  arp = { -48, 48 }, cutoff = { 0, 255 }, res = { 0, 15 }, speed = { 1, 31 },
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
           vib = { 0, 0 }, arp = {}, filter = nil }
end

function TrkSong.script(name, path, block)
  return { name = name, kind = "script", path = path, block = block }
end

function TrkSong.empty_cell() return { note = 0, inst = 0, cmd = "", param = 0 } end

function TrkSong.empty_row()
  local row = {}
  for t = 1, TrkSong.TRACKS do row[t] = TrkSong.empty_cell() end
  return row
end

function TrkSong.empty_rows(n)
  local rows = {}
  for i = 1, n do rows[i] = TrkSong.empty_row() end
  return rows
end

function TrkSong.copy_rows(rows)
  local out = {}
  for i, row in ipairs(rows) do
    out[i] = {}
    for t, c in ipairs(row) do out[i][t] = { note = c.note, inst = c.inst, cmd = c.cmd, param = c.param } end
  end
  return out
end

-- The lowest pattern number not in use, or nil when all 64 are.
function TrkSong.free_pattern(s)
  for n = 0, TrkSong.MAX_PATTERN do
    if not s.patterns[n] then return n end
  end
end

-- Drops the patterns the order doesn't play; how many went.
function TrkSong.clean(s)
  local used, n = {}, 0
  for _, p in ipairs(s.order) do used[p] = true end
  for p in pairs(s.patterns) do
    if not used[p] then
      s.patterns[p] = nil
      n = n + 1
    end
  end
  return n
end

-- A new song: one lead instrument and one empty pattern.
function TrkSong.new()
  local s = { title = "untitled", speed = 6, instruments = {}, order = { 0 }, loop = 0, patterns = {} }
  s.instruments[1] = TrkSong.builtin("Lead")
  s.patterns[0] = TrkSong.empty_rows(TrkSong.DEFAULT_ROWS)
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

local function parse_cell(text)
  local f = {}
  for w in text:gmatch("%S+") do f[#f + 1] = w end
  if #f ~= 4 then return nil end
  local c = {}
  if f[1] == "..." then c.note = 0
  elseif f[1] == "===" then c.note = TrkSong.NOTE_OFF
  else c.note = TrkSong.parse_note(f[1]) end
  c.inst = f[2] == ".." and 0 or hex2(f[2])
  if f[3] == "." then
    c.cmd = ""
  elseif #f[3] == 1 and TrkSong.COMMANDS:find(f[3], 1, true) then
    c.cmd = f[3]
  end
  c.param = f[4] == ".." and 0 or hex2(f[4])
  if c.note and c.inst and c.cmd and c.param then return c end
end

-- TRACKS cells split by "|".
local function parse_row(line)
  local row = {}
  for part in (line .. "|"):gmatch("([^|]*)|") do
    local c = parse_cell(part)
    if not c then return nil end
    row[#row + 1] = c
  end
  if #row == TrkSong.TRACKS then return row end
end

local function parse_instrument(f)
  local num = f[2] and not f[2].quoted and hex2(f[2].text)
  if num and (num < 1 or num > TrkSong.MAX_INSTRUMENT) then return nil, "instrument number out of range" end
  if not num or not f[3] or not f[3].quoted then return nil end
  local name = f[3].text
  if f[4] and not f[4].quoted and f[4].text == "script" then
    if not (f[5] and f[5].quoted and f[6] and not f[7]) then return nil end
    return num, TrkSong.script(name, f[5].text, f[6].text)
  end
  local b, i, bad = TrkSong.builtin(name), 4, nil
  -- The next number, which must lie in lo..hi (out of range is an error,
  -- as in the kernel's parser, never a clamp).
  local function nxt(what, lo, hi)
    local v = int(f[i])
    i = i + 1
    if v == nil then
      bad = bad or ("expected a number after " .. what)
    elseif v < lo or v > hi then
      bad = bad or (what .. " out of range")
    end
    return v or lo
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
      b.adsr = { nxt("adsr", 0, 100000), nxt("adsr", 0, 100000), nxt("adsr", 0, 100000), nxt("adsr", 0, 100000) }
    elseif key == "duty" then
      b.duty = nxt("duty", 1, 99)
    elseif key == "pwm" then
      b.pwm = nxt("pwm", -50, 50)
    elseif key == "vib" then
      b.vib = { nxt("vib", 0, 15), nxt("vib", 0, 15) }
    elseif key == "arp" then
      b.arp = {}
      while #b.arp < 3 and int(f[i]) do b.arp[#b.arp + 1] = nxt("arp", -48, 48) end
      if #b.arp == 0 then return nil, "expected a number after arp" end
    elseif key == "filter" then
      local mode = ({ lp = 1, bp = 2, hp = 4 })[word()]
      if not mode then return nil end
      b.filter = { mode, nxt("filter cutoff", 0, 255), nxt("filter resonance", 0, 15) }
    else
      return nil
    end
  end
  if bad then return nil, bad end
  return num, b
end

-- "order 00 01 00 loop 0" -> the pattern list and the 0-based loop entry.
local function parse_order(f)
  local order, i = {}, 2
  while f[i] and f[i].text ~= "loop" do
    local p = hex2(f[i].text)
    if not p then return nil end
    if p > TrkSong.MAX_PATTERN then return nil, "order pattern out of range" end
    order[#order + 1] = p
    i = i + 1
  end
  local loop = int(f[i + 1])
  if #order == 0 or not loop or f[i + 2] then return nil end
  if #order > TrkSong.MAX_ORDER then return nil, "the order is too long" end
  if loop < 0 or loop >= #order then return nil, "loop out of range" end
  return order, loop
end

-- Text acid_song_parse accepted -> a song, or nil and why.
function TrkSong.parse(text)
  local lines = {}
  for line in (text .. "\n"):gmatch("(.-)\r?\n") do lines[#lines + 1] = line end
  local s = { title = "", speed = 6, instruments = {}, order = nil, loop = 0, patterns = {} }
  local i = 1
  while lines[i] and lines[i]:match("^%s*$") do i = i + 1 end
  if not lines[i] or lines[i]:match("^%s*(.-)%s*$") ~= "acid-track 2" then
    return nil, "not an acid-track 2 file"
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
        if not s.speed or s.speed < 1 or s.speed > 31 then return nil, n .. ": speed out of range" end
      elseif word == "instrument" then
        local num, ins = parse_instrument(f)
        if not num then return nil, n .. ": " .. (ins or "bad instrument") end
        s.instruments[num] = ins
      elseif word == "order" then
        local order, loop = parse_order(f)
        if not order then return nil, n .. ": " .. (loop or "bad order") end
        s.order, s.loop = order, loop
      elseif word == "pattern" then
        local num, len = hex2(f[2] and f[2].text), int(f[3])
        if not num or not len then return nil, n .. ": bad pattern" end
        if num > TrkSong.MAX_PATTERN then return nil, n .. ": pattern number out of range" end
        if len < 1 or len > TrkSong.MAX_ROWS then return nil, n .. ": pattern length out of range" end
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
  if not s.speed then return nil, "bad header" end
  if not s.order then return nil, "no order" end
  for _, p in ipairs(s.order) do
    if not s.patterns[p] then return nil, "the order uses a missing pattern" end
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

-- One track's cell as the file writes it, e.g. "C-4 01 4 22".
function TrkSong.cell_text(c)
  local note = "..."
  if c.note == TrkSong.NOTE_OFF then note = "===" elseif c.note ~= TrkSong.NOTE_NONE then note = TrkSong.note_name(c.note) end
  local inst = c.inst == 0 and ".." or string.format("%02X", c.inst)
  local cmd = c.cmd == "" and "." or c.cmd
  local param = (c.cmd == "" and c.param == 0) and ".." or string.format("%02X", c.param)
  return note .. " " .. inst .. " " .. cmd .. " " .. param
end

-- One row: every track's cell, split by " | ".
function TrkSong.row_text(row)
  local cells = {}
  for t = 1, TrkSong.TRACKS do cells[t] = TrkSong.cell_text(row[t]) end
  return table.concat(cells, " | ")
end

-- The canonical text, exactly as acid-sound's song::write produces it.
function TrkSong.write(s)
  local out = {}
  local function add(x) out[#out + 1] = x end
  add("acid-track 2\n")
  add("title " .. s.title .. "\n")
  add("speed " .. s.speed .. "\n")
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
    end
    add("\n")
  end
  add("order")
  for _, p in ipairs(s.order) do add(string.format(" %02X", p)) end
  add(" loop " .. s.loop .. "\n")
  for _, num in ipairs(sorted_keys(s.patterns)) do
    local rows = s.patterns[num]
    add(string.format("\npattern %02X %d\n", num, #rows))
    for _, r in ipairs(rows) do add(TrkSong.row_text(r) .. "\n") end
  end
  return table.concat(out)
end
