-- Editor: a text editor for app source.
-- The three mixins (layout, command strip, touch) live in editor/*.lua
-- and are loaded before this file via editor.app.toml's libs.

EditorApp = AcidApp:extend("EditorApp")

-- Mixin: copy every function field of the module onto the
-- class. Constants stay on the module and are read from there.
local function mixin(cls, mod)
  for k, v in pairs(mod) do
    if type(v) == "function" then cls[k] = v end
  end
end

mixin(EditorApp, EditorLayout)
mixin(EditorApp, EditorCmd)
mixin(EditorApp, EditorTouch)

local function split_lines(text)
  local lines = {}
  for line in (text .. "\n"):gmatch("(.-)\n") do lines[#lines + 1] = line end
  while #lines > 0 and lines[#lines] == "" do lines[#lines] = nil end
  return lines
end

EditorApp.DEFAULT_FILE = "v3/fsroot/Home/notes.txt"

EditorApp.BG_COLOR = 0x0B1712      -- THEME_PANEL -- status line
EditorApp.BODY_BG = 0x050607       -- THEME_BG -- text body
EditorApp.GUTTER_BG = 0x050607     -- THEME_BG
EditorApp.GUTTER_COLOR = 0x9DAAA3  -- THEME_MUTED
EditorApp.TEXT_COLOR = 0xD4E6DB    -- THEME_TEXT
EditorApp.CURSOR_COLOR = 0x00FF66  -- THEME_HARD
EditorApp.STATUS_COLOR = 0x9DAAA3  -- THEME_MUTED

function EditorApp:on_create()
  -- File Manager launches this with a specific file to view/edit
  -- (clicking a .lua file spawns Editor with that path as the launch
  -- arg -- see file_manager.lua); opened directly from Menu with no
  -- arg, it falls back to the general notes file it always edited
  -- before. Either way the file is genuinely editable and saveable,
  -- including files under fsroot/App (a real symlink to v3/apps) --
  -- there's no separate read-only mode, by design: this is meant to
  -- double as a live way to tweak an app's own source and see the
  -- change on its next launch, no rebuild step.
  local arg = acid_launch_arg()
  self.path = (arg == "") and EditorApp.DEFAULT_FILE or arg
  self.buf = Buffer.new(self:read_lines())
  self.scroll_y = 0
  self.scroll_x = 0
  self.message = nil
  -- On for Lua, off for anything else -- a .txt file has no syntax to
  -- show and colouring prose at random is worse than leaving it alone.
  -- ESC h overrides it for this window.
  self.hl_on = self.path:sub(-4) == ".lua"
  self.hl_cache = {}
end

function EditorApp:on_resize(w, h)
  EditorLayout.compute(w, h)
  EditorCmd.fit_cells()
  self:ensure_scroll()
end

function EditorApp:read_lines()
  local text = acid_fs_read(self.path)
  if text == nil then return { "" } end
  return split_lines(text)
end

-- Sibling temp path, not touching path until the new content is fully
-- written and closed. Truncating the target in place the instant a write
-- starts means a write that fails partway (a full disk, a yanked SD card
-- on the hw target, anything) leaves the original file gone, not merely
-- unsaved, with no way back in from inside this OS. Writing to a sibling
-- first and renaming it over path only if that write fully succeeds
-- means a failure before the rename leaves the original byte-for-byte
-- untouched. The suffix is one no real file is likely to already be
-- using -- path itself, mid-edit in this very window, is the one path a
-- plain ".tmp" or "~" convention risks colliding with.
EditorApp.SAVE_TMP_SUFFIX = ".editor-save-tmp"

function EditorApp:save_file()
  local backup_failed = not self:backup_own_source()
  local tmp = self.path .. EditorApp.SAVE_TMP_SUFFIX
  -- Trailing newline, not just lines joined by one -- POSIX text files
  -- end in one, and this app regularly saves real source files under
  -- fsroot/App (the live v3/apps symlink): saving without it would strip
  -- an existing app's final newline on every save, which is diff noise
  -- against git history for no reason.
  local wrote = acid_fs_write(tmp, table.concat(self.buf:lines(), "\n") .. "\n") ~= nil
  local saved = false
  if wrote then
    saved = acid_fs_rename(tmp, self.path) ~= nil
  end
  -- A half-written temp file (write failed) or a temp file the rename
  -- couldn't place (rename failed) is debris either way -- clean it up
  -- rather than leaving it for the user to find later. Best-effort: if
  -- even this fails there is nothing more useful to do about it.
  if not saved then
    if acid_fs_size(tmp) ~= nil then acid_fs_delete(tmp) end
  end
  if saved then
    self.buf:mark_saved()
    self.message = backup_failed and "saved (backup failed)" or "saved"
  else
    self.message = "save failed"
  end
  return saved
end

-- Only for the files listed in EditorLayout.OWN_SOURCE_ROOTS /
-- OWN_SOURCE_RELATIVE_PATHS -- the user chose this scope explicitly over
-- backing up every save, since this app is the one editor that can edit
-- and then immediately re-run the very code it's running as. Copies the
-- CURRENT on-disk contents (not the buffer -- the buffer is what's about
-- to overwrite it) to "<path>.bak" before that happens. A failure here
-- (missing file on a first save, an unwritable sibling, anything) must
-- never block the real save -- a user who can't save at all is worse off
-- than one whose backup didn't take -- so this always returns rather
-- than raising, and save_file only uses the result to add a note to
-- message.
function EditorApp:backup_own_source()
  if not self:own_source(self.path) then return true end
  -- A first save of a brand new own-source file has nothing to back up
  -- -- that's not a backup failure worth a "(backup failed)" note next
  -- to "saved", it's just the expected shape of creating something new.
  if acid_fs_size(self.path) == nil then return true end
  local current = acid_fs_read(self.path)
  if current == nil then return false end
  return acid_fs_write(self.path .. ".bak", current) ~= nil
end

function EditorApp:visible_lines()
  return (EditorLayout.STATUS_Y - EditorLayout.TEXT_Y) // EditorLayout.LINE_H
end

function EditorApp:visible_cols()
  return (EditorLayout.WINDOW_W - EditorLayout.TEXT_X) // EditorLayout.CHAR_W
end

function EditorApp:file_label()
  local slash = nil
  for i = #self.path, 1, -1 do
    if self.path:sub(i, i) == "/" then slash = i; break end
  end
  return slash and self.path:sub(slash + 1) or self.path
end

function EditorApp:redraw()
  acid_clear_user_area()
  acid_draw_window_frame(self:window_title())
  self:draw_gutter()
  self:draw_lines()
  self:draw_cursor()
  if self:cmd_active() then self:draw_cmd_strip() end
  if self:cmd_prompt_active() then
    self:draw_cmd_prompt()
  else
    self:draw_status()
  end
  acid_draw_window_border()
end

function EditorApp:draw_status()
  local L = EditorLayout
  acid_fill_rect(0, L.STATUS_Y, L.WINDOW_W, L.LINE_H, EditorApp.BG_COLOR)
  local left = self.message
  if not left then
    left = self:file_label() .. (self.buf:modified() and " *" or "")
  end
  local right = (self.buf.cy + 1) .. "," .. (self.buf.cx + 1) .. "  " ..
                self.buf:line_count() .. "L" .. (self.hl_on and "  hl" or "")
  -- 28 was right for the old 240px window (35 columns total); at 420px
  -- (70 columns) the right-hand field only ever needs ~14-23 of them
  -- (see right, above -- even a 4-digit cursor position/line count plus
  -- "  hl" is 20 chars), so 28 was clipping real messages mid-word, e.g.
  -- "unsaved -- ESC q again to close" (31 chars) lost its last word.
  -- 48 leaves the right field a comfortable margin: left[0,48] ends at
  -- pixel 2+48*6=290 (at Normal), and `right` only reaches x=298 (its start pixel, at Normal)
  -- at a 4-digit cursor row/col or line count -- an 8px gap -- and stays
  -- clear at any line count this editor is actually used at (nothing in
  -- this codebase's own source, the largest realistic file it edits,
  -- tops even 3 digits). Only a 5-digit line count (99999+) would ever
  -- collide, which is not a real file size here.
  -- At Large the same 48 columns would run under the right-hand field, so
  -- the left part also stops one column short of where the right begins.
  local right_x = L.WINDOW_W - #right * L.CHAR_W - 2
  local left_cols = math.min(48, (right_x - 2) // L.CHAR_W - 1)
  acid_draw_text(left:sub(1, left_cols), 2, L.STATUS_Y + 1, EditorApp.STATUS_COLOR, EditorApp.BG_COLOR)
  acid_draw_text(right, right_x, L.STATUS_Y + 1,
                 EditorApp.STATUS_COLOR, EditorApp.BG_COLOR)
end

function EditorApp:draw_gutter()
  local L = EditorLayout
  acid_fill_rect(0, L.TEXT_Y, L.GUTTER_W, L.STATUS_Y - L.TEXT_Y, EditorApp.GUTTER_BG)
  local i = 0
  while i < self:visible_lines() do
    local idx = self.scroll_y + i
    if idx >= self.buf:line_count() then break end
    local num = tostring(idx + 1)
    acid_draw_text(num, L.GUTTER_W - #num * L.CHAR_W - 2,
                   L.TEXT_Y + i * L.LINE_H + 1, EditorApp.GUTTER_COLOR, EditorApp.GUTTER_BG)
    i = i + 1
  end
end

EditorApp.SEL_BG = 0x123322  -- THEME_PANEL's documented button-hover shade,
                             -- the same highlight file_manager.lua uses
                             -- for its selected row

function EditorApp:draw_lines()
  local L = EditorLayout
  self:hl_invalidate()
  local i = 0
  while i < self:visible_lines() do
    local idx = self.scroll_y + i
    local y = L.TEXT_Y + i * L.LINE_H
    acid_fill_rect(L.TEXT_X, y, L.WINDOW_W - L.TEXT_X, L.LINE_H, EditorApp.BODY_BG)
    local sel = self:selection_span(idx)
    if sel ~= nil then
      local sx = sel[1] - self.scroll_x
      local ex = sel[2] - self.scroll_x
      if sx < 0 then sx = 0 end
      if ex > self:visible_cols() then ex = self:visible_cols() end
      if ex > sx then
        acid_fill_rect(L.TEXT_X + sx * L.CHAR_W, y, (ex - sx) * L.CHAR_W, L.LINE_H, EditorApp.SEL_BG)
      end
    end
    if idx < self.buf:line_count() then
      if self.hl_on then
        self:draw_hl_line(idx, y)
      else
        local visible_text = self.buf:line(idx):sub(self.scroll_x + 1, self.scroll_x + self:visible_cols())
        acid_draw_text(visible_text, L.TEXT_X, y + 1, EditorApp.TEXT_COLOR, EditorApp.BODY_BG)
      end
    end
    i = i + 1
  end
end

-- Tokens for one line, tokenized on first sight and kept until that
-- line changes. Buffer reports what went stale (take_dirty); "all" means
-- the line count itself moved, so every cached index past the edit is
-- wrong and the cheapest correct answer is to start over.
function EditorApp:hl_tokens(index)
  local cached = self.hl_cache[index + 1]
  if cached ~= nil then return cached end
  local toks = Hl.tokenize(self.buf:line(index))
  self.hl_cache[index + 1] = toks
  return toks
end

-- Runs once per redraw, from draw_lines, so it must fire even when
-- highlighting is off -- otherwise a buffer edited with colour disabled
-- keeps stale tokens once it's switched back on. take_dirty is
-- read-and-clear, so calling this again from hl_tokens (once per line)
-- would always see an empty result after the first line -- dead work in
-- a per-line loop -- which is why it lives here only.
function EditorApp:hl_invalidate()
  local dirty = self.buf:take_dirty()
  if dirty == "all" then
    self.hl_cache = {}
    return
  end
  for _, i in ipairs(dirty) do self.hl_cache[i + 1] = nil end
end

function EditorApp:draw_hl_line(index, y)
  local L = EditorLayout
  local col = 0
  local limit = self.scroll_x + self:visible_cols()
  for _, t in ipairs(self:hl_tokens(index)) do
    local text = t[1]
    local start_col = col
    col = col + #text
    if col > self.scroll_x then
      if start_col >= limit then break end
      local cut = self.scroll_x - start_col
      if cut < 0 then cut = 0 end
      local vis = text:sub(cut + 1)
      local screen_col = start_col + cut - self.scroll_x
      local room = self:visible_cols() - screen_col
      if #vis > room then vis = vis:sub(1, room) end
      acid_draw_text(vis, L.TEXT_X + screen_col * L.CHAR_W, y + 1, t[2], EditorApp.BODY_BG)
    end
  end
end

-- The {start_col, end_col} of the selection on one line, or nil. A line
-- fully inside a multi-line selection runs to its own length plus one,
-- so the newline it swallowed is visible as a highlighted cell rather
-- than the selection appearing to stop short at the end of the text.
function EditorApp:selection_span(index)
  local r = self.buf:selection_range()
  if r == nil then return nil end
  local sx, sy, ex, ey = r[1], r[2], r[3], r[4]
  if index < sy or index > ey then return nil end
  local from = (index == sy) and sx or 0
  local to = (index == ey) and ex or (#self.buf:line(index) + 1)
  if to <= from then return nil end
  return { from, to }
end

function EditorApp:draw_cursor()
  local L = EditorLayout
  local row = self.buf.cy - self.scroll_y
  if row < 0 or row >= self:visible_lines() then return end
  local col = self.buf.cx - self.scroll_x
  if col < 0 or col >= self:visible_cols() then return end
  local x = L.TEXT_X + col * L.CHAR_W
  local y = L.TEXT_Y + row * L.LINE_H
  acid_fill_rect(x, y + L.LINE_H - 2, L.CHAR_W, 2, EditorApp.CURSOR_COLOR)
end

function EditorApp:on_touch(x, y, pressed)
  if pressed then self.message = nil end
  if self:editor_touch(x, y, pressed) then self:redraw() end
end

function EditorApp:on_key(code, pressed)
  if not pressed then return end
  if self:cmd_prompt_key(code) then self:redraw(); return end
  if self:cmd_key(code) then self:redraw(); return end
  self.message = nil
  -- Not on ESCAPE: this is the ESCAPE that reopens the strip after "q"
  -- auto-closed it (cmd_key's own ESCAPE branch handles the cancel
  -- case, where the strip was already open) -- see cmd_key's comment.
  if code ~= AcidKeys.ESCAPE then self.quit_armed = false end
  if code == AcidKeys.ESCAPE then
    self:cmd_open()
    self:redraw()
    return
  elseif code == AcidKeys.UP then
    self.buf:move(0, -1)
  elseif code == AcidKeys.DOWN then
    self.buf:move(0, 1)
  elseif code == AcidKeys.LEFT then
    self.buf:move(-1, 0)
  elseif code == AcidKeys.RIGHT then
    self.buf:move(1, 0)
  elseif code == AcidKeys.ENTER then
    self.buf:split_line()
  elseif code == AcidKeys.BACKSPACE then
    self.buf:backspace()
  elseif code == AcidKeys.DELETE then
    self.buf:delete_forward()
  elseif code == AcidKeys.TAB then
    -- Two spaces, not a tab character: every width calculation in this
    -- app counts characters, and a literal tab would make the cursor
    -- column and the drawn column disagree from that point on.
    self.buf:insert_text("  ")
  elseif code >= 32 and code <= 126 then
    self.buf:insert_char(string.char(code))
  end
  self:ensure_scroll()
  self:redraw()
end

function EditorApp:ensure_scroll()
  if self.buf.cy < self.scroll_y then
    self.scroll_y = self.buf.cy
  elseif self.buf.cy >= self.scroll_y + self:visible_lines() then
    self.scroll_y = self.buf.cy - self:visible_lines() + 1
  end
  if self.buf.cx < self.scroll_x then
    self.scroll_x = self.buf.cx
  elseif self.buf.cx >= self.scroll_x + self:visible_cols() then
    self.scroll_x = self.buf.cx - self:visible_cols() + 1
  end
end

EditorApp:new():start()
