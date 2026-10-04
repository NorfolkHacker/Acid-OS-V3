-- AcidSprite: pixel sprites drawn from picture-strings. Each row is a
-- string; each character is a palette key, '.' is transparent. Same-colour
-- horizontal runs merge into one rect (one overlay call instead of many),
-- and flip mirrors each row.
-- Draws on the overlay, so the caller must own it (acid_overlay_open).

AcidSprite = {}

function AcidSprite.width(rows)
  return #rows[1]
end

function AcidSprite.height(rows)
  return #rows
end

function AcidSprite.draw(rows, x, y, scale, palette, flip)
  for r = 1, #rows do
    AcidSprite.draw_row(rows[r], x, y + (r - 1) * scale, scale, palette, flip)
  end
end

function AcidSprite.draw_row(row, x, y, scale, palette, flip)
  local len = #row
  -- Column c (0-based) as seen after an optional mirror.
  local function at(c)
    if flip then c = len - 1 - c end
    return row:sub(c + 1, c + 1)
  end
  local c = 0
  while c < len do
    local ch = at(c)
    local color = nil
    if ch ~= "." then color = palette[ch] end
    if color == nil then
      c = c + 1
    else
      local run = 1
      while c + run < len and at(c + run) == ch do
        run = run + 1
      end
      acid_overlay_fill_rect(x + c * scale, y, run * scale, scale, color)
      c = c + run
    end
  end
end

-- Sprite files (.spr): the text format Sprite Paint saves and games load.
-- See docs/manual-v3/04-graphics.md §4.6. parse never raises: a bad file
-- returns nil and "line N: reason".
AcidSprite.MAX_SIZE = 32
AcidSprite.MAX_FRAMES = 8
AcidSprite.MAX_FPS = 30
AcidSprite.DEFAULT_FPS = 6
AcidSprite.KEYS = "0123456789abcdef"

-- The text's lines, without the empty one after a final newline, and with
-- any trailing \r removed.
local function split_lines(text)
  local list = {}
  for line in (text .. "\n"):gmatch("(.-)\n") do list[#list + 1] = (line:gsub("\r$", "")) end
  if #list > 1 and list[#list] == "" and text:sub(-1) == "\n" then list[#list] = nil end
  return list
end

local function words(line)
  local out = {}
  for w in line:gmatch("%S+") do out[#out + 1] = w end
  return out
end

-- A decimal integer from lo to hi, or nil.
local function int_in(s, lo, hi)
  if s == nil or not s:match("^%d+$") or #s > 3 then return nil end
  local n = tonumber(s)
  if n < lo or n > hi then return nil end
  return n
end

function AcidSprite.parse(text)
  if type(text) ~= "string" then return nil, "line 0: not text" end
  local s = { palette = {}, frames = {} }
  local header, fps, rows = false, nil, nil
  local list = split_lines(text)
  local n = 0
  local function fail(msg) return nil, "line " .. n .. ": " .. msg end
  for i, line in ipairs(list) do
    n = i
    if line == "" or line:sub(1, 1) == "#" then
      -- skipped
    elseif not header then
      if line ~= "acid-sprite 1" then return fail("expected 'acid-sprite 1'") end
      header = true
    elseif rows and #rows < s.h then
      if #line ~= s.w then return fail("row is " .. #line .. " wide, expected " .. s.w) end
      for c = 1, #line do
        local ch = line:sub(c, c)
        if ch ~= "." and s.palette[ch] == nil then return fail("'" .. ch .. "' is not a palette key") end
      end
      rows[#rows + 1] = line
    else
      local w = words(line)
      local cmd = w[1]
      if cmd == "size" then
        if s.w then return fail("size given twice") end
        local sw, sh = int_in(w[2], 1, AcidSprite.MAX_SIZE), int_in(w[3], 1, AcidSprite.MAX_SIZE)
        if #w ~= 3 or not sw or not sh then return fail("size must be two numbers from 1 to 32") end
        s.w, s.h = sw, sh
      elseif cmd == "fps" then
        if fps then return fail("fps given twice") end
        fps = int_in(w[2], 1, AcidSprite.MAX_FPS)
        if #w ~= 2 or not fps then return fail("fps must be from 1 to 30") end
      elseif cmd == "pal" then
        if #s.frames > 0 then return fail("pal after frame") end
        local key, hex = w[2], w[3]
        if #w ~= 3 or #key ~= 1 or not AcidSprite.KEYS:find(key, 1, true) then
          return fail("palette key must be one of 0-9 a-f")
        end
        if s.palette[key] then return fail("palette key '" .. key .. "' given twice") end
        if not hex:match("^%x%x%x%x%x%x$") then return fail("colour must be 6 hex digits") end
        s.palette[key] = tonumber(hex, 16)
      elseif cmd == "frame" and #w == 1 then
        if not s.w then return fail("frame before size") end
        if #s.frames == AcidSprite.MAX_FRAMES then return fail("more than 8 frames") end
        rows = {}
        s.frames[#s.frames + 1] = rows
      else
        return fail("unknown line '" .. line .. "'")
      end
    end
  end
  n = math.max(#list, 1)
  if not header then return fail("missing 'acid-sprite 1'") end
  if not s.w then return fail("missing size") end
  if #s.frames == 0 then return fail("no frames") end
  if #rows < s.h then return fail("frame " .. #s.frames .. " has " .. #rows .. " of " .. s.h .. " rows") end
  s.fps = fps or AcidSprite.DEFAULT_FPS
  return s
end

-- The text for a parsed (or edited) sprite: header, size, fps, the palette
-- in key order with lowercase hex, then the frames. No comments or blank
-- lines, and a final newline.
function AcidSprite.serialize(s)
  local out = { "acid-sprite 1", "size " .. s.w .. " " .. s.h, "fps " .. s.fps }
  for i = 1, #AcidSprite.KEYS do
    local k = AcidSprite.KEYS:sub(i, i)
    if s.palette[k] then out[#out + 1] = string.format("pal %s %06x", k, s.palette[k]) end
  end
  for _, rows in ipairs(s.frames) do
    out[#out + 1] = "frame"
    for _, r in ipairs(rows) do out[#out + 1] = r end
  end
  return table.concat(out, "\n") .. "\n"
end

-- Reads and parses a .spr file. Returns the sprite, or nil and a message.
function AcidSprite.load(path)
  local text, err = acid_fs_read(path)
  if not text then return nil, err or "read failed" end
  return AcidSprite.parse(text)
end
