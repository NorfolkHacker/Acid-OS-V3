-- File Manager: browse the sandboxed fsroot, preview text files, open
-- .lua files in Editor and launch apps from their .app.toml manifests.

FileManagerApp = AcidApp:extend("FileManagerApp")

-- Must match file_manager.app.toml and the kernel's title bar height.
FileManagerApp.WINDOW_W = 220
FileManagerApp.WINDOW_H = 160
FileManagerApp.TITLE_BAR_H = 16
FileManagerApp.ROW_H = 12
FileManagerApp.ROOT_DIR = "v3/fsroot"

FileManagerApp.BG_COLOR = 0x0B1712      -- THEME_PANEL -- header row
FileManagerApp.BODY_BG = 0x050607       -- THEME_BG -- list/preview rows
FileManagerApp.TEXT_COLOR = 0xD4E6DB    -- THEME_TEXT
FileManagerApp.DIR_COLOR = 0x00FF66     -- THEME_HARD -- accent for directory entries
FileManagerApp.TOML_COLOR = 0xB026FF    -- THEME_VIOLET -- .app.toml manifests, i.e. the
                                        -- entries that actually launch something when
                                        -- clicked (activate_selected below), as opposed
                                        -- to the .lua beside them, which only opens in
                                        -- the editor
FileManagerApp.SEL_BG = 0x123322        -- THEME_PANEL's documented button-hover shade,
                                        -- reused for the selected-row highlight

-- Editor's own window size (editor.app.toml) -- kept in sync by comment,
-- the same convention this codebase already uses for other cross-file
-- constants (e.g. desktop's SCREEN_W).
FileManagerApp.EDITOR_PATH = "v3/apps/editor.lua"
FileManagerApp.EDITOR_W = 420
FileManagerApp.EDITOR_H = 280

-- Splits on "\n"; drops trailing empty strings.
local function split_lines(text)
  local lines = {}
  for line in (text .. "\n"):gmatch("(.-)\n") do lines[#lines + 1] = line end
  while #lines > 0 and lines[#lines] == "" do lines[#lines] = nil end
  return lines
end

-- Parses the leading integer, 0 if there isn't one.
local function to_i(s)
  return tonumber(s:match("^%s*([-+]?%d+)")) or 0
end

local function ends_with(s, suffix)
  return #s >= #suffix and s:sub(-#suffix) == suffix
end

local function trim(s)
  return s:match("^%s*(.-)%s*$")
end

function FileManagerApp:on_create()
  self.dir = self.ROOT_DIR
  self.entries = {}
  self.selected = 0
  self.scroll = 0
  self.preview = nil       -- nil = browsing; a string = previewing this
                           -- file's content
  self.preview_name = nil
  self.preview_scroll = 0
  self:scan_dir()
end

function FileManagerApp:scan_dir()
  self.entries = {}
  self.scroll = 0
  if self.dir ~= self.ROOT_DIR then
    self.entries[#self.entries + 1] = { name = "..", dir = true, size = 0 }
  end
  local list, err = acid_fs_list(self.dir)
  if not list then
    self.entries[#self.entries + 1] = { name = "(error: " .. err .. ")", dir = false, size = 0 }
  else
    local names = {}
    for _, ent in ipairs(list) do
      if ent ~= "." and ent ~= ".." then names[#names + 1] = ent end
    end
    table.sort(names)
    for _, name in ipairs(names) do
      local path = self.dir .. "/" .. name
      local is_dir = acid_fs_list(path) ~= nil
      local size = 0
      if not is_dir then size = acid_fs_size(path) or 0 end
      self.entries[#self.entries + 1] = { name = name, dir = is_dir, size = size }
    end
  end
  self.selected = 0
end

function FileManagerApp:visible_rows()
  return (self.WINDOW_H - self.TITLE_BAR_H) // self.ROW_H
end

-- How many entry rows the listing has room for, below its own one-row
-- path header -- the number draw_listing/on_touch/scrolling all need to
-- agree on, previously duplicated as a bare `visible_rows - 1` in each.
function FileManagerApp:visible_listing_rows()
  return self:visible_rows() - 1
end

-- Keeps self.selected on screen by moving self.scroll to match, same idea as
-- editor's own ensure_scroll -- without this, a directory with more
-- entries than fit on screen (v3/apps, now browsable via fsroot/App,
-- easily has more files than this window's dozen or so visible rows)
-- left every entry past the first screenful permanently unreachable:
-- arrow-key selection moved self.selected past the visible range with
-- nothing on screen ever scrolling to show it, and a tap below the
-- visible rows had nothing real to hit-test against anyway.
function FileManagerApp:ensure_listing_scroll()
  if self.selected < self.scroll then
    self.scroll = self.selected
  elseif self.selected >= self.scroll + self:visible_listing_rows() then
    self.scroll = self.selected - self:visible_listing_rows() + 1
  end
end

function FileManagerApp:redraw()
  acid_clear_user_area()
  acid_draw_window_frame(self:window_title())
  if self.preview then
    self:draw_preview()
  else
    self:draw_listing()
  end
  acid_draw_window_border()
end

function FileManagerApp:draw_listing()
  local y = self.TITLE_BAR_H
  acid_fill_rect(0, y, self.WINDOW_W, self.ROW_H, self.BG_COLOR)
  local n = #self.entries
  local label = self.dir
  if n > self:visible_listing_rows() then
    label = self.dir .. " (" .. (self.selected + 1) .. "/" .. n .. ")"
  end
  acid_draw_text(label, 2, y + 2, self.TEXT_COLOR, self.BG_COLOR)
  y = y + self.ROW_H
  local i = self.scroll
  while i < n and i < self.scroll + self:visible_listing_rows() do
    local e = self.entries[i + 1]
    local row_bg = (i == self.selected) and self.SEL_BG or self.BODY_BG
    acid_fill_rect(0, y, self.WINDOW_W, self.ROW_H, row_bg)
    local entry_label
    if e.dir then
      entry_label = "[" .. e.name .. "]"
    else
      entry_label = " " .. e.name .. " (" .. e.size .. "B)"
    end
    local color = self:entry_color(e)
    acid_draw_text(entry_label:sub(1, 34), 2, y + 2, color, row_bg)
    y = y + self.ROW_H
    i = i + 1
  end
end

-- Directories green, launchable .toml manifests violet, everything else
-- plain text -- so a glance at v3/apps tells you which half of each
-- <name>.lua / <name>.app.toml pair is the one that starts the app.
function FileManagerApp:entry_color(e)
  if e.dir then return self.DIR_COLOR end
  if ends_with(e.name, ".toml") then return self.TOML_COLOR end
  return self.TEXT_COLOR
end

function FileManagerApp:draw_preview()
  local lines = split_lines(self.preview)
  acid_fill_rect(0, self.TITLE_BAR_H, self.WINDOW_W, self.ROW_H, self.BG_COLOR)
  local header = self.preview_name
  if #lines > self:visible_listing_rows() then
    header = self.preview_name .. " (" .. (self.preview_scroll + 1) .. "/" .. #lines .. ")"
  end
  acid_draw_text(header, 2, self.TITLE_BAR_H + 2, self.TEXT_COLOR, self.BG_COLOR)
  local y = self.TITLE_BAR_H + self.ROW_H
  local i = self.preview_scroll
  while i < #lines and i < self.preview_scroll + self:visible_listing_rows() do
    acid_fill_rect(0, y, self.WINDOW_W, self.ROW_H, self.BODY_BG)
    acid_draw_text(lines[i + 1]:sub(1, 34), 2, y + 2, self.TEXT_COLOR, self.BODY_BG)
    y = y + self.ROW_H
    i = i + 1
  end
end

function FileManagerApp:max_preview_scroll(lines)
  local over = #lines - self:visible_listing_rows()
  return over > 0 and over or 0
end

function FileManagerApp:on_touch(x, y, pressed)
  -- The router sends a TOUCH event on every ~16ms tick for as long as
  -- the mouse stays held, not just once on the initial press (demo_touch
  -- relies on exactly that, to draw a continuous trail while dragging).
  -- Without this guard, holding down on a row re-ran activate_selected
  -- -- reopening the file and redrawing -- on every single one of those
  -- ticks, which looked like the file rapidly opening and closing
  -- (reported live as "intense flicker" while holding a row). A plain
  -- click should select/open once, not repeatedly for as long as it's
  -- held.
  if not pressed then
    self.touch_held = false
    return
  end
  if self.touch_held then return end
  self.touch_held = true
  if self.preview then
    self.preview = nil
    self:redraw()
    return
  end
  local row = (y - self.TITLE_BAR_H) // self.ROW_H - 1 + self.scroll
  if row < 0 or row >= #self.entries then return end
  self.selected = row
  self:activate_selected()
end

function FileManagerApp:on_key(code, pressed)
  if not pressed then return end
  if self.preview then
    local lines = split_lines(self.preview)
    if code == AcidKeys.ESCAPE then
      self.preview = nil
    elseif code == AcidKeys.UP then
      if self.preview_scroll > 0 then self.preview_scroll = self.preview_scroll - 1 end
    elseif code == AcidKeys.DOWN then
      if self.preview_scroll < self:max_preview_scroll(lines) then
        self.preview_scroll = self.preview_scroll + 1
      end
    end
    self:redraw()
    return
  end
  if code == AcidKeys.UP then
    if self.selected > 0 then self.selected = self.selected - 1 end
    self:ensure_listing_scroll()
    self:redraw()
  elseif code == AcidKeys.DOWN then
    if self.selected < #self.entries - 1 then self.selected = self.selected + 1 end
    self:ensure_listing_scroll()
    self:redraw()
  elseif code == AcidKeys.ENTER then
    self:activate_selected()
  elseif code == AcidKeys.BACKSPACE then
    self:go_up()
  end
end

function FileManagerApp:activate_selected()
  local entry = self.entries[self.selected + 1]
  if not entry then return end
  if entry.name == ".." then
    self:go_up()
  elseif entry.dir then
    self.dir = self.dir .. "/" .. entry.name
    self:scan_dir()
    self:redraw()
  elseif ends_with(entry.name, ".app.toml") then
    self:launch_manifest(entry.name)
  elseif ends_with(entry.name, ".lua") then
    acid_spawn_app(self.EDITOR_PATH, self.EDITOR_W, self.EDITOR_H, self.dir .. "/" .. entry.name)
  else
    self:open_preview(entry.name)
  end
end

-- Clicking an app's manifest launches it directly -- games/Piano/etc
-- are deliberately left out of desktop's Menu dropdown now (they opt
-- out via `menu = false` in their own .app.toml) specifically so this
-- is how you reach them: click their icon here instead. Uses the same
-- plain "key = value" manifest format desktop's own parser reads;
-- duplicated rather than shared since the two apps want different
-- things from a manifest (desktop also needs the `menu` flag and
-- registers into the launcher list, this just needs enough to spawn
-- once).
function FileManagerApp:launch_manifest(name)
  local path = self.dir .. "/" .. name
  local fields = {}
  -- This pcall covers the read and the parse, and also the spawn's
  -- integer conversion, which can raise on a digit run beyond i32.
  local ok = pcall(function()
    local text = acid_fs_read(path)
    if not text then error("unreadable") end
    for _, line in ipairs(split_lines(text)) do
      line = trim(line)
      if line ~= "" and line:sub(1, 1) ~= "#" then
        local eq = line:find("=", 1, true)
        if eq then
          fields[trim(line:sub(1, eq - 1))] = trim(line:sub(eq + 1))
        end
      end
    end
    if not (fields["w"] and fields["h"]) then return end
    -- Spec §15.4: `runtime = wasm` means the app is <stem>.wasm.
    local ext = fields["runtime"] == "wasm" and ".wasm" or ".lua"
    local script_path = self.dir .. "/" .. name:sub(1, #name - #".app.toml") .. ext
    -- canonical_app_path is defined on AcidApp (v3/apps/lib/acid_app.lua),
    -- not here -- browsing to a manifest under fsroot/App (which this app
    -- itself makes possible) built script_path still under fsroot/App, and
    -- the launcher registry only matches the canonical v3/apps form (see
    -- AcidApp's comment on why). A method call here resolves through
    -- self's class chain (FileManagerApp extends AcidApp), the same way
    -- cmdbar's bare `quit` call already relies on AcidApp.
    acid_spawn_app(self:canonical_app_path(script_path), to_i(fields["w"]), to_i(fields["h"]), "")
  end)
  if not ok then return end
end

function FileManagerApp:go_up()
  if self.dir == self.ROOT_DIR then return end
  local slash
  for i = #self.dir, 1, -1 do
    if self.dir:sub(i, i) == "/" then slash = i; break end
  end
  self.dir = slash and self.dir:sub(1, slash - 1) or self.ROOT_DIR
  self:scan_dir()
  self:redraw()
end

function FileManagerApp:open_preview(name)
  local path = self.dir .. "/" .. name
  self.preview_scroll = 0
  local text, err = acid_fs_read(path)
  if text then
    self.preview = text
  else
    self.preview = "(cannot open: " .. err .. ")"
  end
  self.preview_name = name
  self:redraw()
end

FileManagerApp:new():start()
