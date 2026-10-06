-- Acid Tracker: a GoatTracker-style tracker for .trk songs, four channels
-- of two voices. The song is a TrkSong (tracker/song.lua), the cursor and
-- every edit a TrkEdit (tracker/edit.lua), positions TrkLayout
-- (tracker/layout.lua) and the Esc command line TrkCmd (tracker/cmd.lua).
-- The kernel plays the song: each change goes over with acid_song_update,
-- so edits are heard while it plays. See
-- docs/superpowers/specs/2026-10-06-acid-tracker-design.md §7.

TrackerApp = AcidApp:extend("TrackerApp")
TrackerApp.BG = 0x050607         -- THEME_BG
TrackerApp.PANEL = 0x0B1712      -- THEME_PANEL
TrackerApp.SEL_BG = 0x123322     -- THEME_PANEL's hover shade
TrackerApp.TEXT = 0xD4E6DB       -- THEME_TEXT
TrackerApp.MUTED = 0x9DAAA3      -- THEME_MUTED
TrackerApp.HARD = 0x00FF66       -- THEME_HARD
TrackerApp.DIM = 0x4A5650        -- a muted channel's notes
TrackerApp.BEAT = 0x0E1E18       -- every fourth row
TrackerApp.PLAY_MS = 40          -- how often the play position is read
TrackerApp.EDITOR_PATH = "v3/apps/editor.lua"
TrackerApp.EDITOR_W, TrackerApp.EDITOR_H = 420, 280   -- editor.app.toml's size
TrackerApp.HELP = "F1 play  F2 from row  F4 stop  F5-8 mute  Esc command  Space edit"

function TrackerApp:window_title() return "Acid Tracker" end

function TrackerApp:on_create()
  self.handle = nil
  self.path = nil
  self.playing = false
  self.play_pos = nil      -- { order, row } while playing
  self.muted = { false, false, false, false }
  self.cmd = nil           -- the command line's text while it's open
  self.armed = nil         -- "new" / "o" / "q" after one go on an unsaved song
  self.message = nil
  self.held = {}           -- key code -> channel, so its release ends the preview
  local arg = acid_launch_arg()
  if arg ~= nil and arg ~= "" then self:open_path(arg) else self:new_song() end
  self:layout(acid_window_size())
end

function TrackerApp:layout(w, h) self.L = TrkLayout.compute(w, h) end
function TrackerApp:on_resize(w, h) self:layout(w, h) end

function TrackerApp:on_destroy()
  if self.playing then acid_song_stop() end
  if self.handle then acid_song_free(self.handle) end
end

-- Songs ------------------------------------------------------------------

-- Text goes to the kernel's parser first (the reader of record); only then
-- is the Lua model read from it.
function TrackerApp:load_text(text)
  local handle, warnings = acid_song_parse(text)
  if not handle then return nil, warnings end
  local song, err = TrkSong.parse(text)
  if not song then
    acid_song_free(handle)
    return nil, err
  end
  return handle, song, warnings
end

function TrackerApp:adopt(handle, song, warnings)
  if self.playing then self:stop() end
  if self.handle then acid_song_free(self.handle) end
  self.handle = handle
  self.E = TrkEdit.new(song)
  self.synced = true
  self.armed = nil
  self.message = type(warnings) == "table" and warnings[1] or nil
end

function TrackerApp:new_song()
  local song = TrkSong.new()
  local handle, warnings = acid_song_parse(TrkSong.write(song))
  self:adopt(handle, song, warnings)
  self.path = nil
end

-- A file that won't load leaves a new song, and says why.
function TrackerApp:open_path(path)
  local text, err = acid_fs_read(path)
  if text then
    local handle, song, warnings = self:load_text(text)
    if handle then
      self:adopt(handle, song, warnings)
      self.path = path
      return
    end
    err = song
  end
  self:new_song()
  self.message = "can't open: " .. tostring(err)
end

-- Sends the edited song to the kernel; a playing copy keeps its place.
function TrackerApp:sync()
  if self.synced or not self.handle then return end
  self.synced = true
  local warnings, err = acid_song_update(self.handle, TrkSong.write(self.E.song))
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
  for ch = 1, 4 do
    if self.muted[ch] then acid_song_mute(ch, true) end
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
    self.E:set_order(o)
  else
    return
  end
  self:redraw()
end

function TrackerApp:edit_script()
  local ins = self.E.song.instruments[self.E.inst]
  if ins and ins.kind == "script" then
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
  elseif code == K.F4 then
    self:stop()
  elseif code >= K.F5 and code <= K.F8 then
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
    changed = E:orders_key(code)
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
  elseif c.name == "speed" or c.name == "donor" or c.name == "len" then
    local n = TrkCmd.int(a[1])
    local hi = ({ speed = 31, donor = 4, len = TrkSong.MAX_ROWS })[c.name]
    if not n or n < 1 or n > hi then return self:usage(c.name) end
    if c.name == "speed" then
      E.song.speed = n
    elseif c.name == "donor" then
      E.song.donor = n
    else
      E:set_length(n)
    end
    self:changed()
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
  return string.format("ORD %02X/%02X  ROW %02X  SPD %d  OCT %d  INS %02X %s%s",
    pos[1], E:order_count() - 1, pos[2], E.song.speed, E.octave, E.inst,
    ins and ins.name:sub(1, 12) or "--", tag)
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

-- The rows scroll past a fixed middle line: the cursor's row, or the
-- playing row while the song plays here.
function TrackerApp:draw_grid()
  local L, E, S = self.L, self.E, TrkLayout
  for ch = 1, 4 do
    local m = self.muted[ch]
    acid_draw_text(m and (ch .. " muted") or tostring(ch), L.ch_x[ch], L.head_y, m and self.DIM or self.MUTED, self.BG)
  end
  local following = self.play_pos ~= nil and self.play_pos[1] == E.order
  local center = following and self.play_pos[2] or E.row
  local top = center - L.rows // 2
  local width = (S.ROWNUM_CHARS + 4 * S.COL_CHARS - 1) * S.CH_W
  local slot = E:slot()
  local max_rows = E:max_rows()
  for i = 0, L.rows - 1 do
    local r = top + i
    if r >= 0 and r < max_rows then
      local y = L.grid_y + i * S.CH_H
      local bg = (following and r == self.play_pos[2]) and self.SEL_BG or (r % 4 == 0 and self.BEAT or self.BG)
      acid_fill_rect(L.x, y, width, S.CH_H, bg)
      acid_draw_text(string.format("%02X", r), L.x, y, self.MUTED, bg)
      for ch = 1, 4 do
        local row = E:rows(ch)[r + 1]
        if row then
          local off, w
          if E.focus == "grid" and ch == E.ch and r == E.row then
            off, w = S.SLOT_CHARS[slot.col], S.SLOT_W[slot.col]
            if slot.digit then off, w = off + slot.digit, 1 end
          end
          local fg = self.muted[ch] and self.DIM or self.TEXT
          self:draw_row_text(TrkSong.row_text(row), L.ch_x[ch], y, fg, bg, off, w, E.edit and self.HARD or self.MUTED)
        end
      end
    end
  end
end

-- One line per channel: its loop point, then the entries around the
-- view's order position ("03+5" is pattern 03 transposed up 5).
function TrackerApp:draw_orders()
  local L, E, S = self.L, self.E, TrkLayout
  acid_fill_rect(L.x, L.ord_y - 1, L.w - 2 * L.x, S.ORDER_LINES * S.CH_H + 1, self.PANEL)
  local per = math.max(1, (L.text_cols - 6) // 6)
  for ch = 1, 4 do
    local o = E.song.orders[ch]
    local y = L.ord_y + (ch - 1) * S.CH_H
    acid_draw_text(string.format("%d L%02X", ch, o.loop), L.x, y, self.MUTED, self.PANEL)
    local first = math.max(0, math.min(E.order - per // 2, #o.entries - per))
    for i = first, math.min(#o.entries - 1, first + per - 1) do
      local e = o.entries[i + 1]
      local label = string.format("%02X", e.pattern) .. (e.transpose ~= 0 and string.format("%+d", e.transpose) or "")
      local sel = E.focus == "orders" and E.ord_ch == ch and E.ord_pos == i
      local fg = sel and self.BG or (i == E.order and self.HARD or self.TEXT)
      acid_draw_text(label, L.x + (6 + (i - first) * 6) * S.CH_W, y, fg, sel and self.HARD or self.PANEL)
    end
  end
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
