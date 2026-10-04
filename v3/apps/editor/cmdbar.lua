-- Command mode: ESC raises a strip of single-key commands over the bottom
-- of the text area, one keypress runs one, ESC closes it.
--
-- Not a menu bar, and not Ctrl/Alt chords, because neither is reachable
-- here: the input layer drops Ctrl, Alt and the function keys outright,
-- and the hardware target is a tablet with no keyboard to press them on
-- anyway. A strip of plain letters is the one surface that works
-- identically typed and tapped -- see the design doc.
--
-- Prompts (find, goto, save-as) keep the strip up and take text on the
-- status row; everything else acts immediately. A mixin of EditorApp:
-- functions are copied onto the class, the constants stay here.
EditorCmd = {}

-- Parses the leading integer, 0 if there isn't one.
local function to_i(s)
  return tonumber(s:match("^%s*([-+]?%d+)")) or 0
end

local function strip(s)
  return (s:match("^%s*(.-)%s*$"))
end

local function split_lines(text)
  local lines = {}
  for line in (text .. "\n"):gmatch("(.-)\n") do lines[#lines + 1] = line end
  while #lines > 0 and lines[#lines] == "" do lines[#lines] = nil end
  return lines
end

-- Three fixed rows, not a paging list: at 65 columns everything fits
-- with room to spare, and a fixed strip means a command never moves,
-- which is what makes the tap targets learnable.
EditorCmd.CMD_ROWS = {
  { { "s", "save" }, { "a", "save-as" }, { "q", "close" }, { "!", "run" },
    { "u", "undo" }, { "r", "redo" } },
  { { "x", "cut" }, { "c", "copy" }, { "v", "paste" }, { "m", "mark" },
    { "/", "find" }, { "n", "next" } },
  { { "g", "goto" }, { "t", "top" }, { "b", "bottom" }, { "h", "hilite" },
    { "p", "prev" }, { "?", "keys" } },
}

-- 10 characters a cell at Normal (60 px, six cells in 420); at Large the
-- same six cells would be wider than the window, so they narrow to fit.
-- The longest cell is "h hilite", 8 characters.
-- Recomputed when the window is resized (EditorApp:on_resize).
function EditorCmd.fit_cells()
  EditorCmd.CMD_CELL_CHARS = math.min(10, (EditorLayout.WINDOW_W - 2) // (6 * EditorLayout.CHAR_W))
end
EditorCmd.fit_cells()
EditorCmd.CMD_BG = 0x123322         -- THEME_PANEL's documented button-hover shade
EditorCmd.CMD_KEY_COLOR = 0x00FF66  -- THEME_HARD
EditorCmd.CMD_TEXT_COLOR = 0xD4E6DB -- THEME_TEXT

EditorCmd.DEFAULT_RUN_W = 240
EditorCmd.DEFAULT_RUN_H = 170

function EditorCmd.cmd_strip_y(self)
  return EditorLayout.STATUS_Y - #EditorCmd.CMD_ROWS * EditorLayout.LINE_H
end

-- The open flag is cmd_is_open, not cmd_open: Lua shares one namespace
-- between fields and methods, and cmd_open() is the method.
function EditorCmd.cmd_active(self)
  return self.cmd_is_open and true or false
end

function EditorCmd.cmd_open(self)
  self.cmd_is_open = true
  self.message = nil
end

function EditorCmd.cmd_close(self)
  self.cmd_is_open = false
  self.prompt = nil
  self.prompt_text = nil
end

-- A prompt keeps the strip up and takes text on the status row. The
-- three that need text are find, goto and save-as; everything else acts
-- on the keypress.
function EditorCmd.cmd_prompt_active(self)
  return self.prompt ~= nil
end

function EditorCmd.cmd_prompt_open(self, kind, label)
  self.prompt = kind
  self.prompt_label = label
  self.prompt_text = ""
  self.cmd_is_open = true
end

function EditorCmd.cmd_prompt_key(self, code)
  if not self:cmd_prompt_active() then return false end
  if code == AcidKeys.ESCAPE then
    self:cmd_close()
  elseif code == AcidKeys.ENTER then
    local text = self.prompt_text
    local kind = self.prompt
    self:cmd_close()
    self:cmd_prompt_submit(kind, text)
  elseif code == AcidKeys.BACKSPACE then
    if #self.prompt_text > 0 then
      self.prompt_text = self.prompt_text:sub(1, #self.prompt_text - 1)
    end
  elseif code >= 32 and code <= 126 then
    self.prompt_text = self.prompt_text .. string.char(code)
  end
  return true
end

function EditorCmd.cmd_prompt_submit(self, kind, text)
  if kind == "find" then
    if #text == 0 then return end
    self.last_query = text
    self:find_from(self.buf.cx + 1, self.buf.cy)
  elseif kind == "goto" then
    local n = to_i(text)
    if n < 1 or n > self.buf:line_count() then
      self.message = "no line " .. text
    else
      self.buf:set_cursor(0, n - 1)
    end
  elseif kind == "saveas" then
    if #text == 0 then return end
    -- Don't leave path pointing at an unwritable file: try the new
    -- path and only keep it if the write actually succeeded, otherwise
    -- a later plain "s" would fail too with no way back to the file
    -- that did work.
    local prev_path = self.path
    self.path = text
    if not self:save_file() then self.path = prev_path end
  end
  self:ensure_scroll()
end

-- Search forward from a position, wrapping, and move there. Separate
-- from cmd_prompt_submit so find-next and find-prev reuse it without
-- reopening the prompt.
function EditorCmd.find_from(self, x, y)
  local hit = self.buf:find(self.last_query, x, y)
  if hit == nil then
    self.message = "not found: " .. self.last_query
    return
  end
  self.buf:set_cursor(hit[1], hit[2])
  self.message = self.last_query
end

function EditorCmd.find_prev_from(self, x, y)
  -- No backward search in Buffer on purpose: with wraparound, the
  -- previous match is just "the last match reached by scanning forward
  -- from here", and one search direction is one thing to get right.
  if self.last_query == nil then
    self.message = "no search yet"
    return
  end
  local best = nil
  local pos = { 0, 0 }
  local n = 0
  while n < 10000 do
    local hit = self.buf:find(self.last_query, pos[1], pos[2])
    if hit == nil then break end
    if best ~= nil and hit[1] == best[1] and hit[2] == best[2] then break end
    if hit[2] > y or (hit[2] == y and hit[1] >= x) then break end
    best = hit
    pos = { hit[1] + 1, hit[2] }
    n = n + 1
  end
  if best == nil then
    self.message = "no earlier match"
    return
  end
  self.buf:set_cursor(best[1], best[2])
  self.message = self.last_query
end

function EditorCmd.draw_cmd_prompt(self)
  local L = EditorLayout
  acid_fill_rect(0, L.STATUS_Y, L.WINDOW_W, L.LINE_H, EditorCmd.CMD_BG)
  local text = self.prompt_label .. ": " .. self.prompt_text .. "_"
  acid_draw_text(text:sub(1, L.WINDOW_W // L.CHAR_W - 1), 2, L.STATUS_Y + 1,
                 EditorCmd.CMD_TEXT_COLOR, EditorCmd.CMD_BG)
end

-- Returns true when the key was consumed, so on_key can stop. An
-- unrecognised key closes the strip rather than sitting there swallowing
-- input -- a command surface you can get stuck inside is worse than one
-- you occasionally have to reopen.
-- quit_armed survives only the ESC that reopens the strip for a second
-- "q" -- cmd_key closes the strip before cmd_run("q") runs, so the user
-- has no way to press "q" twice without an ESC in between, and that ESC
-- must not be mistaken for a cancel. Every OTHER way out of the strip
-- (an explicit ESC-cancel here, or a stray non-printable key here) does
-- disarm it, same as any other command letter (see cmd_run's preamble)
-- or any key on the normal editing path (see editor.lua's on_key).
function EditorCmd.cmd_key(self, code)
  if not self:cmd_active() then return false end
  if code == AcidKeys.ESCAPE then
    self:cmd_close()
    self.quit_armed = false
    return true
  end
  self:cmd_close()
  if code < 32 or code > 126 then
    self.quit_armed = false
    return true
  end
  self:cmd_run(string.char(code))
  return true
end

function EditorCmd.cmd_run(self, ch)
  local was_armed = self.quit_armed
  self.quit_armed = false
  if ch == "q" then self.quit_armed = was_armed end
  if ch == "s" then
    self:save_file()
  elseif ch == "u" then
    if not self.buf:undo() then self.message = "nothing to undo" end
  elseif ch == "r" then
    if not self.buf:redo() then self.message = "nothing to redo" end
  elseif ch == "m" then
    self.buf:toggle_mark()
    self.message = self.buf:mark_set() and "mark set" or "mark cleared"
  elseif ch == "c" then
    self.message = self.buf:copy() and "copied" or "no selection"
  elseif ch == "x" then
    self.message = self.buf:cut() and "cut" or "no selection"
  elseif ch == "v" then
    self.message = self.buf:paste() and "pasted" or "clipboard empty"
  elseif ch == "t" then
    self.buf:set_cursor(0, 0)
  elseif ch == "b" then
    self.buf:set_cursor(0, self.buf:line_count() - 1)
  elseif ch == "/" then
    self:cmd_prompt_open("find", "find")
  elseif ch == "n" then
    if self.last_query == nil then
      self.message = "no search yet"
    else
      self:find_from(self.buf.cx + 1, self.buf.cy)
    end
  elseif ch == "p" then
    self:find_prev_from(self.buf.cx, self.buf.cy)
  elseif ch == "g" then
    self:cmd_prompt_open("goto", "line")
  elseif ch == "a" then
    self:cmd_prompt_open("saveas", "save as")
  elseif ch == "q" then
    self:cmd_quit()
  elseif ch == "!" then
    self:cmd_run_file()
  elseif ch == "h" then
    self.hl_on = not self.hl_on
    self.message = self.hl_on and "highlight on" or "highlight off"
  elseif ch == "?" then
    self.message = "ESC then a letter; see the strip"
  else
    self.message = "no command '" .. ch .. "'"
  end
  self:ensure_scroll()
end

-- Save, then launch the file being edited as a live app window. The
-- whole point of this editor is that fsroot/App is a real symlink to
-- v3/apps, so a change to an app's source is live on its next launch
-- with no rebuild step -- this makes that a two-keystroke loop instead
-- of a trip through the File Manager.
function EditorCmd.cmd_run_file(self)
  if self.path:sub(-4) ~= ".lua" then
    self.message = "not a lua file"
    return
  end
  -- Running the editor's own source (or one of its mixins/acid_app.lua)
  -- from inside itself doesn't open a harmless second window -- it
  -- breaks the spawned one. The cause is the fsroot/App
  -- symlink mismatch (see AcidApp:canonical_app_path): a launch through
  -- it doesn't match the registry's canonical v3/apps path. Refusing
  -- here is the editor protecting itself even if that mapping is ever
  -- undone elsewhere.
  if self:own_source(self.path) then
    self.message = "can't run the editor's own source from itself"
    return
  end
  if not self:save_file() then return end
  local w, h = self:run_geometry()
  -- canonical_app_path is a method on AcidApp, resolved through self's
  -- class chain (EditorApp extends AcidApp), same as the bare `quit`
  -- call in cmd_quit.
  acid_spawn_app(self:canonical_app_path(self.path), w, h, "")
  self.message = "running " .. self:file_label()
end

-- The app's own manifest decides its window size, exactly as the Menu
-- and File Manager do. A .lua with no manifest beside it is still worth
-- running -- it just gets a default-sized window.
function EditorCmd.run_geometry(self)
  local toml = self.path:sub(1, #self.path - 4) .. ".app.toml"
  local text = acid_fs_read(toml)
  if text == nil then return EditorCmd.DEFAULT_RUN_W, EditorCmd.DEFAULT_RUN_H end
  local w = nil
  local h = nil
  for _, line in ipairs(split_lines(text)) do
    line = strip(line)
    if line ~= "" and line:sub(1, 1) ~= "#" then
      local eq = line:find("=", 1, true)
      if eq then
        local key = strip(line:sub(1, eq - 1))
        local value = strip(line:sub(eq + 1))
        if key == "w" then w = to_i(value) end
        if key == "h" then h = to_i(value) end
      end
    end
  end
  if w == nil or h == nil or w < 1 or h < 1 then
    return EditorCmd.DEFAULT_RUN_W, EditorCmd.DEFAULT_RUN_H
  end
  return w, h
end

-- Two presses to lose unsaved work, and the second one has to be the
-- same key -- a status-line confirmation rather than a dialog, because
-- this app framework has no dialog concept and a confirmation needs
-- none.
--
-- Closing goes through AcidApp:quit, which ends the run loop; the app
-- then returns from its script and the kernel tears the window down.
-- acid_close_window takes a window INDEX and deliberately refuses to
-- close the caller's own window -- it's for one app closing another,
-- like sysmon, not this.
function EditorCmd.cmd_quit(self)
  if not self.buf:modified() or self.quit_armed then
    self:quit()
    return
  end
  self.quit_armed = true
  self.message = "unsaved -- ESC q again to close"
end

function EditorCmd.draw_cmd_strip(self)
  local L = EditorLayout
  local C = EditorCmd
  local y = self:cmd_strip_y()
  acid_fill_rect(0, y, L.WINDOW_W, #C.CMD_ROWS * L.LINE_H, C.CMD_BG)
  for row = 0, #C.CMD_ROWS - 1 do
    local cells = C.CMD_ROWS[row + 1]
    for i = 0, #cells - 1 do
      local x = 2 + i * C.CMD_CELL_CHARS * L.CHAR_W
      acid_draw_text(cells[i + 1][1], x, y + row * L.LINE_H + 1, C.CMD_KEY_COLOR, C.CMD_BG)
      acid_draw_text(cells[i + 1][2], x + 2 * L.CHAR_W, y + row * L.LINE_H + 1,
                     C.CMD_TEXT_COLOR, C.CMD_BG)
    end
  end
end
