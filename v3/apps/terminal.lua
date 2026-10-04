-- Terminal -- a tiny shell over the sandboxed fsroot (help, clear, pwd,
-- cd, ls, cat, echo, run <app>), plus the undocumented easter eggs.

-- Splits on "\n", dropping trailing empty strings.
local function split_lines(text)
  local lines = {}
  for line in (text .. "\n"):gmatch("(.-)\n") do lines[#lines + 1] = line end
  while #lines > 0 and lines[#lines] == "" do lines[#lines] = nil end
  return lines
end

-- Splits awk-style: runs of whitespace separate words.
local function split_words(text)
  local words = {}
  for w in text:gmatch("%S+") do words[#words + 1] = w end
  return words
end

local function split_path(path)
  local parts = {}
  for p in path:gmatch("[^/]+") do parts[#parts + 1] = p end
  return parts
end

local function strip(s) return (s:match("^%s*(.-)%s*$")) end

local CW, CH = acid_font_size()
local WW, WH = acid_window_size()

TerminalApp = AcidApp:extend("TerminalApp")

TerminalApp.WINDOW_W = WW
TerminalApp.WINDOW_H = WH
TerminalApp.COLS = (WW - 8) // CW -- columns that fit, past the 2 px margins
TerminalApp.TITLE_BAR_H = 16
TerminalApp.LINE_H = CH + 2
TerminalApp.ROOT_DIR = "v3/fsroot"
TerminalApp.ROOT_SEGMENTS = split_path(TerminalApp.ROOT_DIR)
TerminalApp.PROMPT = "$ "

TerminalApp.BODY_BG = 0x050607       -- THEME_BG
TerminalApp.TEXT_COLOR = 0xD4E6DB    -- THEME_TEXT
TerminalApp.PROMPT_COLOR = 0x00FF66  -- THEME_HARD
TerminalApp.CURSOR_COLOR = 0x00FF66  -- THEME_HARD

function TerminalApp:on_create()
  self.cwd = TerminalApp.ROOT_DIR
  self.lines = { "Acid OS v3 terminal -- type help", "" }
  self.input = ""
  self.history = {}
  self.history_pos = 0 -- 0-based; +1 at each history access
end

function TerminalApp:visible_lines()
  return (TerminalApp.WINDOW_H - TerminalApp.TITLE_BAR_H) // TerminalApp.LINE_H - 1
end

function TerminalApp:redraw()
  acid_clear_user_area()
  acid_draw_window_frame(self:window_title())
  self:draw_scrollback()
  self:draw_input_line()
  acid_draw_window_border()
end

function TerminalApp:draw_scrollback()
  local T = TerminalApp
  local y = T.TITLE_BAR_H
  local n = self:visible_lines()
  local start = #self.lines > n and #self.lines - n or 0
  local i = start
  while i < #self.lines do
    acid_fill_rect(0, y, T.WINDOW_W, T.LINE_H, T.BODY_BG)
    acid_draw_text(self.lines[i + 1]:sub(1, T.COLS), 2, y + 1, T.TEXT_COLOR, T.BODY_BG)
    y = y + T.LINE_H
    i = i + 1
  end
  -- Pad any remaining rows (fewer lines than fit) so old content from a
  -- taller previous frame can never show through underneath.
  while y < T.WINDOW_H - T.LINE_H do
    acid_fill_rect(0, y, T.WINDOW_W, T.LINE_H, T.BODY_BG)
    y = y + T.LINE_H
  end
end

function TerminalApp:draw_input_line()
  local T = TerminalApp
  local y = T.WINDOW_H - T.LINE_H
  acid_fill_rect(0, y, T.WINDOW_W, T.LINE_H, T.BODY_BG)
  local text = T.PROMPT .. self.input
  acid_draw_text(text:sub(1, T.COLS), 2, y + 1, T.PROMPT_COLOR, T.BODY_BG)
  local cx = 2 + math.min(#text, T.COLS) * CW
  acid_fill_rect(cx, y + T.LINE_H - 2, CW, 2, T.CURSOR_COLOR)
end

function TerminalApp:on_key(code, pressed)
  if not pressed then return end
  if code == AcidKeys.ENTER then
    self:submit()
  elseif code == AcidKeys.BACKSPACE then
    if #self.input > 0 then self.input = self.input:sub(1, -2) end
  elseif code == AcidKeys.UP then
    self:history_prev()
  elseif code == AcidKeys.DOWN then
    self:history_next()
  elseif code >= 32 and code <= 126 then
    self.input = self.input .. string.char(code)
  end
  self:redraw()
  -- on_idle only fires when acid_poll_event times out, so a burst of
  -- keystrokes would otherwise stall the animation. AcidEggs.step is
  -- guarded by its own TICK_MS check, which makes this call free whenever
  -- it isn't time for a frame yet.
  AcidEggs.step()
end

function TerminalApp:submit()
  local line = self.input
  self.lines[#self.lines + 1] = TerminalApp.PROMPT .. line
  if strip(line) ~= "" then self.history[#self.history + 1] = line end
  self.history_pos = #self.history
  self.input = ""
  if strip(line) ~= "" then self:run_command(strip(line)) end
end

function TerminalApp:history_prev()
  if #self.history == 0 or self.history_pos <= 0 then return end
  self.history_pos = self.history_pos - 1
  self.input = self.history[self.history_pos + 1]
end

function TerminalApp:history_next()
  if self.history_pos >= #self.history then return end
  self.history_pos = self.history_pos + 1
  self.input = self.history_pos < #self.history and self.history[self.history_pos + 1] or ""
end

function TerminalApp:run_command(line)
  -- The easter eggs (apps/lib/acid_eggs.lua), matched before any real
  -- command and deliberately undocumented: nothing is printed, they are
  -- absent from cmd_help, and the animation is the whole response. Matched
  -- case-insensitively on the whole line, which submit has already
  -- stripped, so only a bare word with no arguments fires one.
  --
  -- Whether or not the egg actually started, a refused start (one
  -- already in flight, or the overlay's canvas failed to allocate) must
  -- stay just as silent, rather than falling through to "command not
  -- found: dave" and announcing that the word means something.
  local lowered = line:lower()
  for _, name in ipairs(AcidEggs.names()) do
    if name == lowered then
      AcidEggs.start(lowered)
      return
    end
  end
  local parts = split_words(line)
  local cmd = parts[1]
  local args = {}
  for i = 2, #parts do args[#args + 1] = parts[i] end
  if cmd == "help" then
    self:cmd_help()
  elseif cmd == "clear" then
    self.lines = {}
  elseif cmd == "pwd" then
    self.lines[#self.lines + 1] = self.cwd
  elseif cmd == "cd" then
    self:cmd_cd(args)
  elseif cmd == "ls" then
    self:cmd_ls(args)
  elseif cmd == "cat" then
    self:cmd_cat(args)
  elseif cmd == "echo" then
    self.lines[#self.lines + 1] = table.concat(args, " ")
  elseif cmd == "run" or cmd == "open" then
    self:cmd_run(args)
  else
    self.lines[#self.lines + 1] = "command not found: " .. cmd
  end
end

function TerminalApp:cmd_help()
  self.lines[#self.lines + 1] = "help, clear, pwd, cd, ls, cat, echo, run <app>"
end

-- Resolves a user-typed path (absolute-from-root with a leading "/", or
-- relative to self.cwd) into a clean, normalized path that can never climb
-- above ROOT_DIR -- by segment, not by string matching a literal ".."
-- argument. Special-casing only a bare ".." and pasting anything else
-- onto self.cwd unresolved would let an EMBEDDED ".." segment, e.g.
-- `cd foo/../../../..`, through to the filesystem, which the real host
-- resolves normally, walking this whole simulator process (running with
-- the real user's own file permissions) out of fsroot entirely.
-- Every resolved path is built from a segment list seeded at ROOT_SEGMENTS
-- (for an absolute path) or self.cwd's own segments (for a relative one) --
-- ".." pops one segment but is refused once the list is already down to
-- #ROOT_SEGMENTS, so there is no string this can ever produce that isn't
-- inside ROOT_DIR. self.cwd itself is always the RESULT of a previous
-- resolve_path call, so it's always already clean -- this never has to
-- re-normalize it.
function TerminalApp:resolve_path(arg)
  -- No argument at all (bare `cd`) goes to ROOT_DIR, same as a real
  -- shell's `cd` with no args going home -- distinct from an explicit
  -- "." argument, which means "stay in cwd".
  if arg == nil or arg == "" then return TerminalApp.ROOT_DIR end
  local segments, rest
  if arg:sub(1, 1) == "/" then
    segments = {}
    for i, s in ipairs(TerminalApp.ROOT_SEGMENTS) do segments[i] = s end
    rest = arg:sub(2)
  else
    segments = split_path(self.cwd)
    rest = arg
  end
  for _, part in ipairs(split_path(rest)) do
    if part == "." then
      -- stay put
    elseif part == ".." then
      if #segments > #TerminalApp.ROOT_SEGMENTS then segments[#segments] = nil end
    else
      segments[#segments + 1] = part
    end
  end
  return table.concat(segments, "/")
end

function TerminalApp:cmd_cd(args)
  local target = self:resolve_path(args[1])
  local names, err = acid_fs_list(target)
  if not names then
    self.lines[#self.lines + 1] = "cd: " .. (args[1] or "") .. ": " .. err
    return
  end
  self.cwd = target
end

function TerminalApp:cmd_ls(args)
  local target = #args == 0 and self.cwd or self:resolve_path(args[1])
  local ents, err = acid_fs_list(target)
  if not ents then
    self.lines[#self.lines + 1] = "ls: " .. err
    return
  end
  local names = {}
  for _, ent in ipairs(ents) do
    if ent ~= "." and ent ~= ".." then names[#names + 1] = ent end
  end
  table.sort(names)
  for _, n in ipairs(names) do self.lines[#self.lines + 1] = n end
end

function TerminalApp:cmd_cat(args)
  if #args == 0 then
    self.lines[#self.lines + 1] = "cat: missing file"
    return
  end
  local text, err = acid_fs_read(self:resolve_path(args[1]))
  if not text then
    self.lines[#self.lines + 1] = "cat: " .. err
    return
  end
  for _, l in ipairs(split_lines(text)) do self.lines[#self.lines + 1] = l end
end

function TerminalApp:cmd_run(args)
  if #args == 0 then
    self.lines[#self.lines + 1] = "run: missing app name"
    return
  end
  local query = args[1]:lower()
  local count = acid_launcher_count()
  local i = 0
  while i < count do
    local name = acid_launcher_name(i)
    if name:lower() == query then
      acid_launcher_spawn(i)
      return
    end
    i = i + 1
  end
  self.lines[#self.lines + 1] = "run: no app named " .. args[1]
end

-- While an egg is in flight the loop needs to wake up every frame rather
-- than every 200ms. Typing is unaffected: a keystroke still arrives as an
-- event the moment it happens.
function TerminalApp:poll_timeout_ms()
  return AcidEggs.active() and AcidEggs.TICK_MS or 200
end

function TerminalApp:on_idle()
  AcidEggs.step()
end

function TerminalApp:on_destroy()
  -- Closing the terminal mid-flight takes the overlay with it, rather
  -- than leaving a sprite frozen on the screen with nothing left running
  -- to clear it.
  AcidEggs.abort()
end

TerminalApp:new():start()
