-- Lua syntax highlighting for the editor (spec 13.4): one line in, a list of
-- { text, colour } runs out, covering the line in order with nothing
-- dropped.
--
-- Per line on purpose. An edit then invalidates exactly one cache entry
-- (Buffer:take_dirty), which is what keeps redraw cheap while typing. The
-- cost is that multi-line strings and multi-line comments are not
-- understood -- getting those right means re-tokenizing from the top of
-- the file on every keystroke, which is the wrong trade for the payoff.
-- A [[ long string closes on its own line or runs to the end of it.
--
-- Calls no acid_* binding, so v3/tools/test_editor.lua runs it headless.
Hl = {}

-- Not the kernel theme's five chrome colours -- the wallpaper's own neon
-- palette, so highlighted source reads as part of this OS rather than as a
-- generic editor theme dropped into it.
Hl.KEYWORD = 0xFF2D78   -- neon pink
Hl.STRING  = 0xFFD400   -- yellow
Hl.NUMBER  = 0x00E5FF   -- cyan
Hl.SYMBOL  = 0xB026FF   -- violet -- THEME_VIOLET, shared with capitalised names
Hl.IVAR    = 0xFF7A00   -- orange (self)
Hl.COMMENT = 0x9DAAA3   -- THEME_MUTED
Hl.PLAIN   = 0xD4E6DB   -- THEME_TEXT

local KEYWORD_SET = {}
for _, w in ipairs({ "and", "break", "do", "else", "elseif", "end", "false",
                     "for", "function", "goto", "if", "in", "local", "nil",
                     "not", "or", "repeat", "return", "then", "true", "until",
                     "while" }) do
  KEYWORD_SET[w] = true
end
Hl.KEYWORDS = KEYWORD_SET

-- One character at the 0-based index i, or "" past either end.
local function at(line, i)
  return line:sub(i + 1, i + 1)
end

function Hl.tokenize(line)
  local out = {}
  local i = 0
  local n = #line
  while i < n do
    local ch = at(line, i)
    if ch == "-" and at(line, i + 1) == "-" then
      -- A comment runs to the end of the line, --[[ included.
      out[#out + 1] = { line:sub(i + 1), Hl.COMMENT }
      i = n
    elseif ch == '"' or ch == "'" then
      local stop = Hl.string_end(line, i, ch, n)
      out[#out + 1] = { line:sub(i + 1, stop), Hl.STRING }
      i = stop
    elseif ch == "[" and at(line, i + 1) == "[" then
      -- A long string ends after the next ]] on this line, or at the end
      -- of the line.
      local close = line:find("]]", i + 3, true)
      local stop = close and (close + 1) or n
      out[#out + 1] = { line:sub(i + 1, stop), Hl.STRING }
      i = stop
    elseif Hl.ident_start(ch) then
      local j = i
      while j < n and Hl.ident_char(at(line, j)) do j = j + 1 end
      local word = line:sub(i + 1, j)
      out[#out + 1] = { word, Hl.word_color(word) }
      i = j
    elseif Hl.digit(ch) then
      local j = Hl.number_end(line, i, n)
      out[#out + 1] = { line:sub(i + 1, j), Hl.NUMBER }
      i = j
    else
      local j = i
      while j < n and Hl.plain_at(line, j) do j = j + 1 end
      if j == i then j = i + 1 end
      out[#out + 1] = { line:sub(i + 1, j), Hl.PLAIN }
      i = j
    end
  end
  return out
end

-- Index just past the closing quote, or the end of the line for a string
-- that never closes -- an unterminated quote is a line you are still
-- typing, and colouring the rest of it as a string is what makes that
-- visible.
function Hl.string_end(line, start, quote, n)
  local j = start + 1
  while j < n do
    local c = at(line, j)
    if c == "\\" then
      j = j + 2
    else
      if c == quote then return j + 1 end
      j = j + 1
    end
  end
  return n
end

-- Digits, underscores and hex letters, plus a '.' only when a digit
-- follows it -- so "1.5" is one number but "(1):max" is a number and then
-- a method call.
function Hl.number_end(line, start, n)
  local j = start
  while j < n do
    local c = at(line, j)
    if Hl.digit(c) or c == "_" or Hl.hex_char(c) then
      j = j + 1
    elseif c == "." and Hl.digit(at(line, j + 1)) then
      j = j + 1
    else
      break
    end
  end
  return j
end

function Hl.word_color(word)
  if word == "self" then return Hl.IVAR end
  if KEYWORD_SET[word] then return Hl.KEYWORD end
  if word:sub(1, 1):match("^[A-Z]$") then return Hl.SYMBOL end
  return Hl.PLAIN
end

function Hl.plain_at(line, j)
  local c = at(line, j)
  if c == "-" and at(line, j + 1) == "-" then return false end
  if c == '"' or c == "'" then return false end
  if c == "[" and at(line, j + 1) == "[" then return false end
  if Hl.ident_start(c) or Hl.digit(c) then return false end
  return true
end

function Hl.ident_start(c)
  if c == nil or c == "" then return false end
  return c:match("^[A-Za-z_]$") ~= nil
end

function Hl.ident_char(c)
  if c == nil or c == "" then return false end
  return c:match("^[A-Za-z0-9_]$") ~= nil
end

function Hl.digit(c)
  if c == nil or c == "" then return false end
  return c:match("^[0-9]$") ~= nil
end

function Hl.hex_char(c)
  if c == nil or c == "" then return false end
  return c:match("^[xXa-fA-F]$") ~= nil
end
