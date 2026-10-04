-- Sprite Paint: pixel art and animated sprites, saved as .spr text files
-- that games load with AcidSprite.load (apps/lib/acid_sprite.lua). The
-- sprite and its undo history live in SpriteDoc, the tools in SpriteTools,
-- every rectangle in SpriteLayout. See
-- docs/superpowers/specs/2026-10-04-sprite-paint-design.md.

SpritePaintApp = AcidApp:extend("SpritePaintApp")
SpritePaintApp.HOME = "v3/fsroot/Home"
SpritePaintApp.NAME_MAX = 24
SpritePaintApp.FPS_STEPS = { 2, 4, 6, 8, 12, 15, 20, 30 }
SpritePaintApp.BG = 0x050607         -- THEME_BG
SpritePaintApp.PANEL = 0x0B1712      -- THEME_PANEL
SpritePaintApp.SEL_BG = 0x123322     -- THEME_PANEL's hover shade
SpritePaintApp.TEXT = 0xD4E6DB       -- THEME_TEXT
SpritePaintApp.MUTED = 0x9DAAA3      -- THEME_MUTED
SpritePaintApp.HARD = 0x00FF66       -- THEME_HARD
SpritePaintApp.GRID = 0x1E2622
SpritePaintApp.CHECK_A = 0x141816    -- transparent cells: a two-tone checker
SpritePaintApp.CHECK_B = 0x22282A
SpritePaintApp.SWATCH_EDGE = 0x3A4440  -- a dim outline so dark swatches show on the background

local TOOL_KEYS = { p = "pen", f = "fill", e = "eraser", i = "picker", l = "line", m = "mirror" }

function SpritePaintApp:window_title() return "Sprite Paint" end

function SpritePaintApp:on_create()
  self.tool = "pen"
  self.mirror = false
  self.key = "1"
  self.picker_open = false
  self.cmd_open = false
  self.prompt = nil       -- { kind = "name" | "size", text = "" } while asking
  self.armed = nil        -- "new" / "close" after one press on a dirty doc
  self.playing = false
  self.touch_held = false
  self.gesture = nil
  self.message = nil
  self.path = nil
  local arg = acid_launch_arg()
  if arg ~= nil and arg ~= "" then
    self:open_path(arg)
  else
    self.doc = SpriteDoc.blank(16, 16)
  end
  self:layout(acid_window_size())
end

function SpritePaintApp:layout(w, h)
  self.L = SpriteLayout.compute(w, h, self.doc.s.w, self.doc.s.h)
end

function SpritePaintApp:on_resize(w, h)
  if self.gesture then self:end_gesture() end
  self:layout(w, h)
end

-- Opens a file; a bad one leaves a new sprite and the error showing.
function SpritePaintApp:open_path(path)
  local s, err = AcidSprite.load(path)
  if s then
    self.doc = SpriteDoc.new(s)
    self.path = path
  else
    self.doc = SpriteDoc.blank(16, 16)
    self.path = nil
    self.message = err
  end
end

function SpritePaintApp:new_sprite(size)
  self.doc = SpriteDoc.blank(size, size)
  self.path = nil
  self.playing = false
  self:layout(self.L.w, self.L.h)
  self:redraw()
end

function SpritePaintApp:file_name()
  return self.path and self.path:match("[^/]*$") or "untitled"
end

-- Drawing ----------------------------------------------------------------

function SpritePaintApp:redraw()
  self.doc:take_full()
  self.doc:take_changes()
  acid_clear_user_area()
  acid_draw_window_frame(self:window_title())
  self:draw_canvas()
  self:draw_side()
  self:draw_status()
  self:draw_bar()
  -- Last, so a finished border means a finished frame (the golden test
  -- waits for it).
  acid_draw_window_border()
end

-- Brings the screen up to date after the doc changed: only the changed
-- cells unless the doc asks for a full canvas redraw.
function SpritePaintApp:refresh()
  if self.doc:take_full() then
    self.doc:take_changes()
    self:draw_canvas()
  else
    for _, c in ipairs(self.doc:take_changes()) do self:draw_cell(c[1], c[2]) end
  end
  self:draw_side()
  self:draw_status()
  self:draw_bar()
end

function SpritePaintApp:draw_canvas()
  local c = self.L.canvas
  if self.L.grid then acid_fill_rect(c.x, c.y, c.w, c.h, self.GRID) end
  for y = 0, self.doc.s.h - 1 do
    for x = 0, self.doc.s.w - 1 do self:draw_cell(x, y) end
  end
end

-- One cell, in key's colour (the doc's key if not given). With the grid
-- on, cells are drawn one pixel short so the grid shows between them.
function SpritePaintApp:draw_cell(x, y, key)
  local c = self.L.canvas
  local k = key or self.doc:get(x, y)
  local size = c.cell - (self.L.grid and 1 or 0)
  local color
  if k == "." then
    color = (x + y) % 2 == 0 and self.CHECK_A or self.CHECK_B
  else
    color = self.doc.s.palette[k]
  end
  acid_fill_rect(c.x + x * c.cell, c.y + y * c.cell, size, size, color)
end

-- A frame's rows at a scale, merging runs of one key into one rect.
local function draw_rows(rows, x, y, scale, palette)
  for r = 1, #rows do
    local row = rows[r]
    local c = 1
    while c <= #row do
      local ch = row:sub(c, c)
      local run = 1
      while c + run <= #row and row:sub(c + run, c + run) == ch do run = run + 1 end
      if ch ~= "." then
        acid_fill_rect(x + (c - 1) * scale, y + (r - 1) * scale, run * scale, scale, palette[ch])
      end
      c = c + run
    end
  end
end

function SpritePaintApp:draw_side()
  local L = self.L
  -- Selected swatch highlight overhangs the column by 1 px on the left, so clear it
  acid_fill_rect(L.col_x - 1, SpriteLayout.TOP, SpriteLayout.COL_W + 1, L.side_h, self.BG)
  if self.picker_open then
    self:draw_picker()
  else
    self:draw_buttons()
    self:draw_palette()
  end
  self:draw_preview()
end

function SpritePaintApp:draw_buttons()
  for _, b in ipairs(self.L.buttons) do
    local on = b.id == self.tool or (b.id == "mirror" and self.mirror)
    local bg = on and self.SEL_BG or self.PANEL
    acid_fill_rect(b.x, b.y, b.w, b.h, bg)
    acid_draw_text(b.label, b.x + (b.w - #b.label * SpriteLayout.CH_W) // 2, b.y + 2, on and self.HARD or self.TEXT, bg)
  end
end

function SpritePaintApp:draw_palette()
  for _, s in ipairs(self.L.swatches) do
    acid_fill_rect(s.x - 1, s.y - 1, s.w + 2, s.h + 2, s.key == self.key and self.HARD or self.SWATCH_EDGE)
    acid_fill_rect(s.x, s.y, s.w, s.h, self.doc.s.palette[s.key])
  end
end

function SpritePaintApp:draw_picker()
  for i = 0, SpritePicker.COUNT - 1 do
    local x, y, w, h = SpritePicker.rect(self.L, i)
    acid_fill_rect(x, y, w, h, SpritePicker.color(i))
  end
end

function SpritePaintApp:draw_preview()
  local s = self.doc.s
  for _, p in ipairs({ self.L.preview1, self.L.preview2 }) do
    acid_fill_rect(p.x, p.y, s.w * p.scale, s.h * p.scale, self.PANEL)
    draw_rows(self.doc:rows(), p.x, p.y, p.scale, s.palette)
  end
end

function SpritePaintApp:status_text()
  local p = self.prompt
  if p and p.kind == "name" then return "save as: " .. p.text .. "_" end
  if p and p.kind == "size" then return "new size: 8, 1=16, 3=32 (esc)" end
  local state = self.message
  if not state then
    state = self.doc.dirty and "unsaved" or (self.path and "saved" or "new")
  end
  return state .. "  " .. self:file_name()
end

function SpritePaintApp:draw_status()
  local L = self.L
  acid_fill_rect(L.text_x, L.msg_y, L.text_w, SpriteLayout.CH_H, self.BG)
  local text = self:status_text():sub(1, L.text_w // SpriteLayout.CH_W)
  acid_draw_text(text, L.text_x, L.msg_y, self.prompt and self.HARD or self.MUTED, self.BG)
end

function SpritePaintApp:bar_label(id)
  local d = self.doc
  if id == "prev" then return "<" end
  if id == "next" then return ">" end
  if id == "count" then return d.frame .. "/" .. d:frame_count() end
  if id == "add" then return "+" end
  if id == "play" then return self.playing and "stop" or "play" end
  if id == "fps" then return d.s.fps .. "fps" end
  return id
end

function SpritePaintApp:draw_bar()
  local L = self.L
  acid_fill_rect(L.text_x, L.bar_y - 1, L.text_w, SpriteLayout.CH_H + 2, self.BG)
  if self.cmd_open then
    for _, it in ipairs(L.cmds) do acid_draw_text(it.label, it.x, L.bar_y, self.TEXT, self.PANEL) end
    return
  end
  for _, it in ipairs(L.bar) do
    local hot = it.id == "play" and self.playing
    acid_draw_text(self:bar_label(it.id), it.x, L.bar_y, hot and self.HARD or self.TEXT, self.BG)
  end
end

-- Touch ------------------------------------------------------------------

-- The router resends a held touch every tick: a canvas gesture follows
-- every sample, every other control acts once per press.
function SpritePaintApp:on_touch(x, y, pressed)
  if not pressed then
    self.touch_held = false
    if self.gesture then self:end_gesture() end
    return
  end
  if self.gesture then
    self:continue_gesture(x, y)
    return
  end
  if self.touch_held then return end
  self.touch_held = true
  self:tap(x, y)
end

function SpritePaintApp:tap(x, y)
  local L = self.L
  if self.prompt then return end
  if self.picker_open then
    local i = SpritePicker.hit(L, x, y)
    if i then self.doc:set_color(self.key, SpritePicker.color(i)) end
    self.picker_open = false
    self:refresh()
    return
  end
  local cx, cy = SpriteLayout.cell_at(L, self.doc.s.w, self.doc.s.h, x, y)
  if self.playing and cx then
    self:stop_playing()
    return
  end
  local item = SpriteLayout.find(self.cmd_open and L.cmds or L.bar, x, y)
  if item then
    self:action(item.id)
    return
  end
  if self.cmd_open and y >= L.bar_y - 1 and y < L.bar_y + SpriteLayout.CH_H + 1 then
    self.cmd_open = false
    self:draw_bar()
    return
  end
  local b = SpriteLayout.find(L.buttons, x, y)
  if b then
    self:select_tool(b.id)
    return
  end
  local s = SpriteLayout.find(L.swatches, x, y)
  if s then
    self:select_key(s.key)
    return
  end
  if cx then self:begin_gesture(cx, cy) end
end

function SpritePaintApp:select_tool(id)
  if id == "mirror" then self.mirror = not self.mirror else self.tool = id end
  self:draw_side()
end

-- A second tap on the selected swatch opens the colour picker for it.
function SpritePaintApp:select_key(key)
  if key == self.key and not self.playing then
    self.picker_open = true
  else
    self.key = key
  end
  self:draw_side()
end

function SpritePaintApp:paint_key()
  return self.tool == "eraser" and "." or self.key
end

function SpritePaintApp:begin_gesture(cx, cy)
  self.message = nil
  self.armed = nil
  local doc = self.doc
  if self.tool == "picker" then
    local k = SpriteTools.pick(doc, cx, cy)
    if k == "." then self.tool = "eraser" else self.key = k end
    self:refresh()
    return
  end
  doc:begin_step()
  if self.tool == "fill" then
    SpriteTools.fill(doc, cx, cy, self.key)
    doc:end_step()
    self:refresh()
    return
  end
  self.gesture = { tool = self.tool, x0 = cx, y0 = cy, x = cx, y = cy, preview = {} }
  if self.tool == "line" then
    self:draw_line_preview()
  else
    SpriteTools.paint(doc, cx, cy, self:paint_key(), self.mirror)
    self:refresh()
  end
end

-- Samples off the canvas are skipped; the next one on it joins up.
function SpritePaintApp:continue_gesture(x, y)
  local g = self.gesture
  local cx, cy = SpriteLayout.cell_at(self.L, self.doc.s.w, self.doc.s.h, x, y)
  if not cx or (cx == g.x and cy == g.y) then return end
  if g.tool == "line" then
    g.x, g.y = cx, cy
    self:draw_line_preview()
    return
  end
  SpriteTools.stroke(self.doc, g.x, g.y, cx, cy, self:paint_key(), self.mirror)
  g.x, g.y = cx, cy
  self:refresh()
end

-- The line so far, drawn over the canvas without touching the doc.
function SpritePaintApp:draw_line_preview()
  local g = self.gesture
  for _, c in ipairs(g.preview) do self:draw_cell(c[1], c[2]) end
  g.preview = {}
  for _, c in ipairs(SpriteTools.line_cells(g.x0, g.y0, g.x, g.y)) do
    g.preview[#g.preview + 1] = c
    if self.mirror then g.preview[#g.preview + 1] = { self.doc.s.w - 1 - c[1], c[2] } end
  end
  for _, c in ipairs(g.preview) do self:draw_cell(c[1], c[2], self.key) end
end

function SpritePaintApp:end_gesture()
  local g = self.gesture
  self.gesture = nil
  if g.tool == "line" then
    for _, c in ipairs(g.preview) do self:draw_cell(c[1], c[2]) end
    SpriteTools.stroke(self.doc, g.x0, g.y0, g.x, g.y, self.key, self.mirror)
  end
  self.doc:end_step()
  self:refresh()
end

-- Frame bar and commands -------------------------------------------------

local function next_fps(fps)
  for _, s in ipairs(SpritePaintApp.FPS_STEPS) do
    if s > fps then return s end
  end
  return SpritePaintApp.FPS_STEPS[1]
end

-- new and close on a dirty doc need a second press.
function SpritePaintApp:confirmed(id)
  if not self.doc.dirty or self.armed == id then
    self.armed = nil
    return true
  end
  self.armed = id
  self.message = "unsaved: " .. id .. " again to discard"
  return false
end

function SpritePaintApp:action(id)
  local d = self.doc
  if self.playing and id ~= "play" then return end
  if id ~= "new" and id ~= "close" then self.armed = nil end
  self.message = nil
  if id == "cmd" then
    self.cmd_open = true
  elseif id == "prev" then
    d:frame_prev()
  elseif id == "next" then
    d:frame_next()
  elseif id == "add" or id == "dup" then
    local done = id == "add" and d:frame_add() or (id == "dup" and d:frame_dup())
    if not done then self.message = "8 frames is the most" end
  elseif id == "del" then
    if not d:frame_del() then self.message = "can't delete the only frame" end
  elseif id == "play" then
    if self.playing then self:stop_playing() else self:start_playing() end
  elseif id == "fps" then
    d:set_fps(next_fps(d.s.fps))
  elseif id == "save" then
    self.cmd_open = false
    self:save()
  elseif id == "saveas" then
    self.cmd_open = false
    self.prompt = { kind = "name", text = "" }
  elseif id == "new" then
    self.cmd_open = false
    if self:confirmed("new") then self.prompt = { kind = "size", text = "" } end
  elseif id == "undo" then
    d:undo()
  elseif id == "redo" then
    d:redo()
  elseif id == "close" then
    self.cmd_open = false
    if self:confirmed("close") then
      self:quit()
      return
    end
  end
  self:refresh()
end

function SpritePaintApp:start_playing()
  self.playing = true
  self.play_at = acid_now_ms()
end

function SpritePaintApp:stop_playing()
  self.playing = false
  self:draw_bar()
end

function SpritePaintApp:poll_timeout_ms()
  if self.playing then return math.max(1, 1000 // self.doc.s.fps) end
  return 200
end

function SpritePaintApp:on_idle()
  if not self.playing then return end
  local now = acid_now_ms()
  if now - self.play_at >= 1000 // self.doc.s.fps then
    self.play_at = now
    self.doc:frame_next()
    self:refresh()
  end
end

-- Saving -------------------------------------------------------------------

function SpritePaintApp:save()
  if not self.path then
    self.prompt = { kind = "name", text = "" }
    return
  end
  self:write(self.path)
end

function SpritePaintApp:write(path)
  local ok, err = acid_fs_write(path, AcidSprite.serialize(self.doc.s))
  if ok then
    self.path = path
    self.doc:mark_saved()
    self.message = "saved"
  else
    self.message = "save failed: " .. tostring(err)
  end
end

-- A plain name, saved under Home with .spr added if it's missing.
function SpritePaintApp:save_as(name)
  name = name:match("^%s*(.-)%s*$")
  if name == "" or name:find("/", 1, true) then
    self.message = "not a file name"
    return
  end
  if name:sub(-4) ~= ".spr" then name = name .. ".spr" end
  self:write(self.HOME .. "/" .. name)
end

-- Keys -------------------------------------------------------------------

local SIZE_KEYS = { [string.byte("8")] = 8, [string.byte("1")] = 16, [string.byte("3")] = 32 }

function SpritePaintApp:prompt_key(code)
  local p = self.prompt
  if code == AcidKeys.ESCAPE then
    self.prompt = nil
  elseif p.kind == "size" then
    local size = SIZE_KEYS[code]
    if not size then return end
    self.prompt = nil
    self:new_sprite(size)
    return
  elseif code == AcidKeys.ENTER then
    self.prompt = nil
    self:save_as(p.text)
  elseif code == AcidKeys.BACKSPACE then
    p.text = p.text:sub(1, -2)
  elseif code >= 32 and code <= 126 and #p.text < self.NAME_MAX then
    p.text = p.text .. string.char(code)
  end
  self:draw_status()
end

function SpritePaintApp:on_key(code, pressed)
  if not pressed then return end
  if self.prompt then
    self:prompt_key(code)
    return
  end
  if self.picker_open then
    if code == AcidKeys.ESCAPE then
      self.picker_open = false
      self:draw_side()
    end
    return
  end
  if code == AcidKeys.ESCAPE then
    self.cmd_open = not self.cmd_open
    self:draw_bar()
    return
  end
  local ch = (code >= 32 and code <= 126) and string.char(code) or nil
  if self.cmd_open then
    for _, c in ipairs(SpriteLayout.CMDS) do
      if ch == c[1] then
        self:action(c[2])
        return
      end
    end
    return
  end
  if ch == " " then
    self:action("play")
  elseif self.playing then
    return
  elseif TOOL_KEYS[ch] then
    self:select_tool(TOOL_KEYS[ch])
  elseif ch == "," then
    self:action("prev")
  elseif ch == "." then
    self:action("next")
  end
end

SpritePaintApp:new():start()
