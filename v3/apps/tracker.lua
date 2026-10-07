-- Acid Tracker: a tracker for .trk songs: eight tracks of one voice each,
-- and one order list of up to 64 patterns. The song is a TrkSong (tracker/song.lua), the cursor and
-- every edit a TrkEdit (tracker/edit.lua), positions TrkLayout
-- (tracker/layout.lua) and the Esc command line TrkCmd (tracker/cmd.lua).
-- The kernel plays the song: each change goes over with acid_song_update,
-- so edits are heard while it plays. See docs/manual-v3/11-music.md.

TrackerApp = AcidApp:extend("TrackerApp")
TrackerApp.BG = 0x050607         -- THEME_BG
TrackerApp.PANEL = 0x0B1712      -- THEME_PANEL
TrackerApp.TEXT = 0xD4E6DB       -- THEME_TEXT
TrackerApp.MUTED = 0x9DAAA3      -- THEME_MUTED
TrackerApp.HARD = 0x00FF66       -- THEME_HARD
TrackerApp.DIM = 0x4A5650        -- a muted channel's notes
TrackerApp.BEAT = 0x0E1E18       -- every fourth row
TrackerApp.HUES = 16             -- the playing row's bar steps round this many hues
TrackerApp.PLAY_MS = 40          -- how often the play position is read
TrackerApp.EDITOR_PATH = "v3/apps/editor.lua"
TrackerApp.EDITOR_W, TrackerApp.EDITOR_H = 420, 280   -- editor.app.toml's size
TrackerApp.HELP = "F1 play  F2 from row  F4 stop  F5-F12 mute  Esc command  Space edit"
TrackerApp.ORDER_HELP = "<> entry  hex/+- pattern  n new  p copy  Enter repeat  Del remove  l loop"

function TrackerApp:window_title() return "Acid Tracker" end

function TrackerApp:on_create()
  self.handle = nil
  self.path = nil
  self.playing = false
  self.play_pos = nil      -- { order, row } while playing
  self.muted = {}           -- track -> true while muted
  self.first_track = 0     -- the leftmost track shown, 0-based
  self.bar_grab = nil      -- the scroll bar's grab point while its thumb is dragged
  self.cmd = nil           -- the command line's text while it's open
  self.armed = nil         -- "new" / "o" / "q" after one go on an unsaved song
  self.message = nil
  self.held = {}           -- key code -> channel, so its release ends the preview
  self.hue = 0             -- the playing row's bar colour, one step per row
  self.song_error = false  -- the kernel refused the last update: don't save it
  local arg = acid_launch_arg()
  if arg ~= nil and arg ~= "" then self:open_path(arg) else self:new_song() end
  self:layout(acid_window_size())
end

function TrackerApp:layout(w, h)
  self.L = TrkLayout.compute(w, h)
  if self.E then self:scroll_to_cursor() end
end
function TrackerApp:on_resize(w, h) self:layout(w, h) end

function TrackerApp:on_destroy()
  if self.playing then acid_song_stop() end
  if self.handle then acid_song_free(self.handle) end
end

-- Songs ------------------------------------------------------------------

-- Text goes to the kernel's parser first (the reader of record); only then
-- is the Lua model read, from the kernel's own text of it. That is the
-- same text for a current song, and the converted one for an old
-- four-channel song (the fourth result is then true).
function TrackerApp:load_text(text)
  local handle, warnings = acid_song_parse(text)
  if not handle then return nil, warnings end
  local canon = acid_song_text(handle) or text
  local song, err = TrkSong.parse(canon)
  if not song then
    acid_song_free(handle)
    return nil, err
  end
  return handle, song, warnings, canon ~= text and not text:match("^%s*acid%-track 2")
end

function TrackerApp:adopt(handle, song, warnings)
  if self.playing then self:stop() end
  if self.handle then acid_song_free(self.handle) end
  self.handle = handle
  self.E = TrkEdit.new(song)
  self.first_track = 0
  self.synced = true
  self.song_error = false
  self.armed = nil
  self.message = type(warnings) == "table" and warnings[1] or nil
end

function TrackerApp:new_song()
  local song = TrkSong.new()
  local handle, warnings = acid_song_parse(TrkSong.write(song))
  self:adopt(handle, song, warnings)
  self.path = nil
  if not handle then self.message = "new song failed: " .. tostring(warnings) end
end

-- A file that won't load keeps the open song (or a new one at launch) and says why.
function TrackerApp:open_path(path)
  local text, err = acid_fs_read(path)
  if text then
    local handle, song, warnings, converted = self:load_text(text)
    if handle then
      self:adopt(handle, song, warnings)
      self.path = path
      if converted then
        self.E.dirty = true
        self.message = "a 4-channel song, now 8 tracks: :w saves it that way"
      end
      return
    end
    err = song
  end
  -- Keep whatever is open; only a launch with nothing open falls back.
  if not self.E then self:new_song() end
  self.message = "can't open: " .. tostring(err)
end

-- Sends the edited song to the kernel; a playing copy keeps its place.
function TrackerApp:sync()
  if self.synced or not self.handle then return end
  self.synced = true
  local warnings, err = acid_song_update(self.handle, TrkSong.write(self.E.song))
  self.song_error = not warnings
  if not warnings then
    self.message = "song error: " .. tostring(err)
  elseif warnings[1] then
    self.message = warnings[1]
  end
end

-- After any edit: unsaved, and heard at once while the song plays.
function TrackerApp:changed()
  self.E.dirty = true
  self.synced = false
  self.armed = nil
  if self.playing then self:sync() end
end

-- new, o and q on an unsaved song need a second go.
function TrackerApp:confirmed(id)
  if not self.E.dirty or self.armed == id then
    self.armed = nil
    return true
  end
  self.armed = id
  self.message = "unsaved: " .. id .. " again to discard"
  return false
end

function TrackerApp:save(name)
  local path = self.path
  if name then
    path = TrkCmd.home_path(name)
    if not path then
      self.message = "not a file name"
      return
    end
  end
  if not path then
    self.message = "usage: " .. TrkCmd.USAGE.w
    return
  end
  -- Only text the kernel accepts is saved, so a saved file always loads:
  -- pending edits go over first, and a refused song waits for a good sync.
  self:sync()
  if self.song_error then
    self.message = "can't save: the song has an error"
    return
  end
  local ok, err = acid_fs_write(path, TrkSong.write(self.E.song))
  if ok then
    self.path = path
    self.E.dirty = false
    self.message = "saved " .. path:match("[^/]*$")
  else
    self.message = "save failed: " .. tostring(err)
  end
end

-- Playback ---------------------------------------------------------------

function TrackerApp:play(order, row)
  if not self.handle then return end
  self:sync()
  acid_song_play(self.handle, order, row)
  self.playing = true
  self.play_pos = { order, row }
  for t = 1, TrkSong.TRACKS do
    if self.muted[t] then acid_song_mute(t, true) end
  end
end

function TrackerApp:stop()
  acid_song_stop()
  self.playing = false
  self.play_pos = nil
end

function TrackerApp:toggle_mute(ch)
  self.muted[ch] = not self.muted[ch]
  if self.playing then acid_song_mute(ch, self.muted[ch]) end
end

function TrackerApp:preview(ch, note)
  if not self.handle then return end
  self:sync()
  acid_song_preview(self.handle, ch, note, self.E.inst)
end

function TrackerApp:poll_timeout_ms()
  return self.playing and self.PLAY_MS or 200
end

-- While playing, the grid follows the song (channel 1's place).
function TrackerApp:on_idle()
  if not self.playing then return end
  local o, r = acid_song_position()
  if not o then
    self.playing = false
    self.play_pos = nil
  elseif self.play_pos[1] ~= o or self.play_pos[2] ~= r then
    self.play_pos = { o, r }
    self.hue = (self.hue + 1) % self.HUES
    self.E:set_order(o)
  else
    return
  end
  self:redraw()
end

function TrackerApp:edit_script()
  local ins = self.E.song.instruments[self.E.inst]
  if ins and ins.kind == "script" then
    if not TrackerApp.script_path_ok(ins.path) then
      self.message = "not a script path"
      return
    end
    acid_spawn_app(self.EDITOR_PATH, self.EDITOR_W, self.EDITOR_H, "v3/fsroot/" .. ins.path)
  else
    self.message = "not a script instrument"
  end
end

-- Keys -------------------------------------------------------------------

function TrackerApp:on_key(code, pressed)
  if not pressed then
    local ch = self.held[code]
    if ch then
      self.held[code] = nil
      if self.handle then acid_song_preview(self.handle, ch, 0, 0) end
    end
    return
  end
  if self.cmd then
    self:cmd_key(code)
    return
  end
  local K, E = AcidKeys, self.E
  if code == K.ESCAPE then
    self.cmd = ""
  elseif code == K.F1 then
    self:play(0, 0)
  elseif code == K.F2 then
    self:play(E.order, E.row)
  elseif code == K.F3 then
    E:toggle_orders()
  elseif code == K.F4 then
    self:stop()
  elseif code >= K.F5 and code <= K.F12 then
    self:toggle_mute(code - K.F5 + 1)
  elseif code == K.TAB then
    E:next_focus()
  elseif code == string.byte(" ") then
    E.edit = not E.edit
  elseif code == string.byte("<") or code == string.byte(">") then
    E:set_octave(code == string.byte("<") and -1 or 1)
  elseif code == string.byte("[") or code == string.byte("]") then
    E:step_inst(code == string.byte("[") and -1 or 1)
  else
    self:focus_key(code)
    return
  end
  self:scroll_to_cursor()
  self:redraw()
end

-- A key for whichever part has focus: the grid, the orders or the instrument.
function TrackerApp:focus_key(code)
  local E, K = self.E, AcidKeys
  local changed, note = false, nil
  if E.focus == "grid" then
    if code == K.UP then E:move_row(-1)
    elseif code == K.DOWN then E:move_row(1)
    elseif code == K.LEFT then E:move_slot(-1)
    elseif code == K.RIGHT then E:move_slot(1)
    elseif E.edit then changed, note = E:grid_key(code)
    else note = E:piano_note(code) end
  elseif E.focus == "orders" then
    local msg
    changed, msg = E:orders_key(code)
    if msg then self.message = msg end
  elseif code == string.byte("e") then
    self:edit_script()
  elseif code == string.byte("r") then
    self.synced = false
    self:sync()
  else
    changed = E:ins_key(code)
  end
  if changed then self:changed() end
  if note then
    self.held[code] = E.ch
    self:preview(E.ch, note)
  end
  self:scroll_to_cursor()
  self:redraw()
end

function TrackerApp:cmd_key(code)
  local K = AcidKeys
  if code == K.ESCAPE then
    self.cmd = nil
  elseif code == K.ENTER then
    local line = self.cmd
    self.cmd = nil
    self:run_command(line)
  elseif code == K.BACKSPACE then
    self.cmd = self.cmd:sub(1, -2)
  elseif code >= 32 and code <= 126 and #self.cmd < self.L.text_cols - 2 then
    self.cmd = self.cmd .. string.char(code)
  end
  self:redraw()
end

-- A script path the .trk file can hold and that stays inside fsroot: no
-- leading /, no empty, . or .. segment, no quote or backslash.
function TrackerApp.script_path_ok(path)
  if path == "" or path:find('["\\]') then return false end
  for seg in (path .. "/"):gmatch("([^/]*)/") do
    if seg == "" or seg == "." or seg == ".." then return false end
  end
  return true
end

-- The playing row's bar: the hue wheel at a quarter brightness, so the
-- row's text stays readable on it.
function TrackerApp:play_bar_color()
  return (AcidPalette.hue(self.hue, self.HUES) >> 2) & 0x3F3F3F
end

function TrackerApp:usage(name) self.message = "usage: " .. TrkCmd.USAGE[name] end

function TrackerApp:run_command(line)
  local c, err = TrkCmd.parse(line)
  self.message = nil
  if not c then
    if err ~= "" then self.message = err end
    return
  end
  if c.name ~= "new" and c.name ~= "o" and c.name ~= "q" then self.armed = nil end
  local E, a = self.E, c.args
  local ins = E.song.instruments[E.inst]
  if c.name == "w" then
    self:save(a[1])
  elseif c.name == "o" then
    local path = TrkCmd.home_path(a[1])
    if not path then
      self.message = "not a file name"
    elseif self:confirmed("o") then
      self:open_path(path)
    end
  elseif c.name == "new" then
    if self:confirmed("new") then self:new_song() end
  elseif c.name == "q" then
    if self:confirmed("q") then self:quit() end
  elseif c.name == "speed" or c.name == "len" then
    local n = TrkCmd.int(a[1])
    local hi = ({ speed = 31, len = TrkSong.MAX_ROWS })[c.name]
    if not n or n < 1 or n > hi then return self:usage(c.name) end
    if c.name == "speed" then
      E.song.speed = n
    else
      E:set_length(n)
    end
    self:changed()
  elseif c.name == "clean" then
    local n = TrkSong.clean(E.song)
    self.message = n == 1 and "dropped 1 unused pattern" or ("dropped " .. n .. " unused patterns")
    if n > 0 then self:changed() end
  elseif c.name == "title" then
    E.song.title = c.rest
    self:changed()
  elseif c.name == "name" then
    if not ins then
      self.message = string.format("no instrument %02X", E.inst)
      return
    end
    ins.name = c.rest
    self:changed()
  elseif c.name == "arp" then
    if not ins or ins.kind ~= "builtin" then
      self.message = "not a built-in instrument"
      return
    end
    local arp = {}
    for i, s in ipairs(a) do
      local n = TrkCmd.int(s)
      if not n or n < -48 or n > 48 then
        self.message = "usage: " .. TrkCmd.USAGE.arp .. " (-48 to 48)"
        return
      end
      arp[i] = n
    end
    ins.arp = arp
    self:changed()
  elseif c.name == "ins" then
    local n = TrkCmd.hex(a[1])
    if not n or n < 1 or n > TrkSong.MAX_INSTRUMENT then return self:usage("ins") end
    if a[2] == "script" and #a == 4 then
      if not TrackerApp.script_path_ok(a[3]) then
        self.message = "not a script path"
        return
      end
      if not a[4]:match("^[%a_][%w_]*$") then
        self.message = "not a block name"
        return
      end
      local old = E.song.instruments[n]
      E.song.instruments[n] = TrkSong.script(old and old.name or "Script", a[3], a[4])
      self:changed()
    elseif #a ~= 1 then
      return self:usage("ins")
    elseif not E.song.instruments[n] then
      E.song.instruments[n] = TrkSong.builtin(string.format("Inst %02X", n))
      self:changed()
    end
    E.inst = n
  end
end

-- Drawing ----------------------------------------------------------------

function TrackerApp:redraw()
  acid_begin_frame()
  acid_clear_user_area()
  acid_draw_window_frame(self:window_title())
  self:draw_status()
  self:draw_grid()
  self:draw_orders()
  self:draw_instrument()
  self:draw_message()
  -- Last, so a finished border means a finished frame (the golden test
  -- waits for it).
  acid_draw_window_border()
  acid_end_frame()
end

-- A full-width line of text, cut to fit.
function TrackerApp:line(text, y, fg, bg)
  acid_draw_text(text:sub(1, self.L.text_cols), self.L.x, y, fg, bg)
end

function TrackerApp:status_text()
  local E = self.E
  local pos = self.play_pos or { E.order, E.row }
  local ins = E.song.instruments[E.inst]
  local tag = self.playing and "  [PLAY]" or (E.edit and "  [EDIT]" or "")
  return string.format("ORD %02X/%02X  PAT %02X  ROW %02X/%02X  SPD %d  OCT %d  INS %02X %s%s",
    pos[1], E:order_count() - 1, E:pattern_num(), pos[2], E:length() - 1, E.song.speed, E.octave, E.inst,
    ins and ins.name:sub(1, 10) or "--", tag)
end

function TrackerApp:message_text()
  if self.cmd then return ":" .. self.cmd .. "_" end
  return self.message or self.HELP
end

function TrackerApp:draw_status()
  self:line(self:status_text(), self.L.status_y, self.E.edit and self.HARD or self.TEXT, self.BG)
end

function TrackerApp:draw_message()
  self:line(self:message_text(), self.L.msg_y, self.cmd and self.HARD or self.MUTED, self.BG)
end

-- One channel's row. On the cursor's cell the field under the cursor is
-- drawn inverted, as its own piece, so no two texts overlap.
function TrackerApp:draw_row_text(text, x, y, fg, bg, off, w, hl)
  if not off then
    acid_draw_text(text, x, y, fg, bg)
    return
  end
  local cw = TrkLayout.CH_W
  if off > 0 then acid_draw_text(text:sub(1, off), x, y, fg, bg) end
  acid_draw_text(text:sub(off + 1, off + w), x + off * cw, y, self.BG, hl)
  if off + w < #text then acid_draw_text(text:sub(off + w + 1), x + (off + w) * cw, y, fg, bg) end
end

-- Keeps the cursor's track on screen while the grid has focus, scrolling
-- sideways as little as it can; and the view inside the eight tracks.
function TrackerApp:scroll_to_cursor()
  local v = self.L.visible
  local t = self.E.ch - 1
  if self.E.focus == "grid" then
    if t < self.first_track then self.first_track = t end
    if t >= self.first_track + v then self.first_track = t - v + 1 end
  end
  self.first_track = math.max(0, math.min(self.first_track, AcidScrollbar.max_offset(TrkSong.TRACKS, v)))
end

-- A pattern that fits shows from row 00; a longer one scrolls only as far
-- as it must to keep the cursor's row (or the playing row, while the song
-- plays here) near the middle. Tracks the window can't fit scroll sideways
-- with the cursor, or with the bar under the grid.
function TrackerApp:draw_grid()
  local L, E, S = self.L, self.E, TrkLayout
  local first, last = self.first_track, math.min(TrkSong.TRACKS, self.first_track + L.visible) - 1
  for t = first, last do
    local m = self.muted[t + 1]
    local x = TrkLayout.track_x(L, t - first)
    acid_draw_text(m and ((t + 1) .. " muted") or tostring(t + 1), x, L.head_y, m and self.DIM or self.MUTED, self.BG)
  end
  local following = self.play_pos ~= nil and self.play_pos[1] == E.order
  local center = following and self.play_pos[2] or E.row
  local rows = E:rows()
  local top = math.max(0, math.min(center - L.rows // 2, #rows - L.rows))
  local width = (S.ROWNUM_CHARS + (last - first + 1) * S.COL_CHARS - 1) * S.CH_W
  local slot = E:slot()
  for i = 0, L.rows - 1 do
    local r = top + i
    local row = rows[r + 1]
    if row then
      local y = L.grid_y + i * S.ROW_H
      local bg = (following and r == self.play_pos[2]) and self:play_bar_color() or (r % 4 == 0 and self.BEAT or self.BG)
      acid_fill_rect(L.x, y, width, S.ROW_H, bg)
      acid_draw_text(string.format("%02X", r), L.x, y, self.MUTED, bg)
      for t = first, last do
        local off, w
        if E.focus == "grid" and t + 1 == E.ch and r == E.row then
          off, w = S.SLOT_CHARS[slot.col], S.SLOT_W[slot.col]
          if slot.digit then off, w = off + slot.digit, 1 end
        end
        local fg = self.muted[t + 1] and self.DIM or self.TEXT
        self:draw_row_text(TrkSong.cell_text(row[t + 1]), TrkLayout.track_x(L, t - first), y, fg, bg, off, w,
          E.edit and self.HARD or self.MUTED)
      end
    end
  end
  AcidScrollbar.draw_h(L.tracks_x, L.bar_y, L.bar_w, TrkSong.TRACKS, L.visible, first)
end

-- The order list around the view's position, its loop point marked "L",
-- then the panel's keys while it has focus.
function TrackerApp:draw_orders()
  local L, E, S = self.L, self.E, TrkLayout
  acid_fill_rect(L.x, L.ord_y - 1, L.w - 2 * L.x, S.ORDER_LINES * S.CH_H + 1, self.PANEL)
  local o = E.song.order
  acid_draw_text("ORDER", L.x, L.ord_y, self.MUTED, self.PANEL)
  local per = math.max(1, (L.text_cols - 6) // 3)
  local at = E.focus == "orders" and E.ord_pos or E.order
  local first = math.max(0, math.min(at - per // 2, #o - per))
  for i = first, math.min(#o - 1, first + per - 1) do
    local sel = E.focus == "orders" and E.ord_pos == i
    local fg = sel and self.BG or (i == E.order and self.HARD or self.TEXT)
    local x = L.x + (6 + (i - first) * 3) * S.CH_W
    acid_draw_text(string.format("%02X", o[i + 1]), x, L.ord_y, fg, sel and self.HARD or self.PANEL)
    if i == E.song.loop then acid_fill_rect(x, L.ord_y + S.CH_H - 1, 2 * S.CH_W, 1, self.MUTED) end
  end
  local help = E.focus == "orders" and self.ORDER_HELP
    or string.format("%d of 64 patterns  loop to %02X  F3 edit the order", self:pattern_count(), E.song.loop)
  self:line(help, L.ord_y + S.CH_H, self.MUTED, self.PANEL)
end

function TrackerApp:pattern_count()
  local n = 0
  for _ in pairs(self.E.song.patterns) do n = n + 1 end
  return n
end

-- A press on the tracks' scroll bar pages sideways or grabs the thumb;
-- while held, the thumb follows the pointer.
function TrackerApp:on_touch(x, y, pressed)
  local L = self.L
  if not pressed then
    self.bar_grab = nil
    return
  end
  local v = L.visible
  if self.bar_grab then
    self.first_track = AcidScrollbar.drag(L.bar_w, TrkSong.TRACKS, v, self.bar_grab, x - L.tracks_x)
  elseif AcidScrollbar.needed(TrkSong.TRACKS, v) and AcidScrollbar.hit_h(L.tracks_x, L.bar_y, L.bar_w, x, y) then
    self.first_track, self.bar_grab = AcidScrollbar.press(L.bar_w, TrkSong.TRACKS, v, self.first_track, x - L.tracks_x)
  else
    return
  end
  self:redraw()
end

-- The current instrument: its name, then its fields (a built-in) or its
-- script (a script instrument).
function TrackerApp:draw_instrument()
  local L, E, S = self.L, self.E, TrkLayout
  acid_fill_rect(L.x, L.ins_y - 1, L.w - 2 * L.x, S.INS_LINES * S.CH_H + 1, self.PANEL)
  local ins = E.song.instruments[E.inst]
  local head = string.format("INS %02X ", E.inst)
  if not ins then
    self:line(head .. string.format("empty: :ins %02X makes one", E.inst), L.ins_y, self.MUTED, self.PANEL)
    return
  end
  self:line(head .. ins.name .. (ins.kind == "script" and "  (script)" or ""), L.ins_y, self.TEXT, self.PANEL)
  if ins.kind == "script" then
    self:line(ins.path .. " : " .. ins.block .. "   e edit  r reload", L.ins_y + S.CH_H, self.MUTED, self.PANEL)
    return
  end
  local x, line = 0, 1
  for i, id in ipairs(TrkEdit.INS_FIELDS) do
    local text = TrkEdit.FIELD_LABELS[id] .. " " .. TrkEdit.field_text(ins, id)
    if x + #text > L.text_cols then x, line = 0, line + 1 end
    if line >= S.INS_LINES then break end
    local sel = E.focus == "ins" and E.ins_field == i
    acid_draw_text(text, L.x + x * S.CH_W, L.ins_y + line * S.CH_H, sel and self.BG or self.TEXT, sel and self.HARD or self.PANEL)
    x = x + #text + 2
  end
end

TrackerApp:new():start()
