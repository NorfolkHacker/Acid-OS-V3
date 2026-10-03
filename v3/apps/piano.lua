Piano = AcidApp:extend("Piano")

-- Constants are Piano.X fields. Offset tables are Lua sequences, so every
-- index into them adds 1 at the access site; index-valued data
-- (BLACK_AFTER_WHITE) holds 0-based values.

Piano.WINDOW_W = 200
Piano.WINDOW_H = 100
Piano.TITLE_BAR_H = 16
Piano.KEY_Y = Piano.TITLE_BAR_H
Piano.KEY_H = Piano.WINDOW_H - Piano.TITLE_BAR_H

Piano.BG_COLOR = 0x050607      -- THEME_BG
Piano.WHITE_COLOR = 0xD4E6DB   -- THEME_TEXT
Piano.BLACK_COLOR = 0x0B1712   -- THEME_PANEL
Piano.PRESSED_COLOR = 0x00FF66 -- THEME_HARD

Piano.VOICE = 5
Piano.FILTER_MODE_LP = 1
Piano.ROOT_ONA = 40

-- Standard one-octave piano layout, same shape a real keyboard has:
-- 7 white keys (C..B), 5 black keys sitting in the gaps between them
-- (none between E/F or B/C, same as a real keyboard). Values are
-- semitone offsets from ROOT_ONA, not raw ona -- WHITE_OFFSETS[0] is C.
Piano.WHITE_OFFSETS = { 0, 2, 4, 5, 7, 9, 11 }
Piano.BLACK_OFFSETS = { 1, 3, 6, 8, 10 }
-- Which white key each black key sits just after (0-indexed) -- e.g.
-- BLACK_OFFSETS[0] (C#) sits after WHITE_OFFSETS[0] (C).
Piano.BLACK_AFTER_WHITE = { 0, 1, 3, 4, 5 }

Piano.WHITE_COUNT = #Piano.WHITE_OFFSETS
Piano.KEY_W = Piano.WINDOW_W // Piano.WHITE_COUNT
Piano.BLACK_W = Piano.KEY_W * 2 // 3
Piano.BLACK_H = Piano.KEY_H * 3 // 5

-- The 0-based position of value, or nil.
local function index_of(list, value)
  for i, v in ipairs(list) do
    if v == value then return i - 1 end
  end
  return nil
end

function Piano:on_create()
  -- Same filter this codebase's other synth users (acid_blaster,
  -- breakout) already configure -- a mild low-pass warms the default
  -- raw pulse wave instead of leaving it harsh. Unlike those, this
  -- voice gets a real sustain and a natural release instead of a short
  -- percussive envelope: a held piano key should keep sounding while
  -- held, then fade out on release, not blip and cut off.
  acid_configure_filter(180, 3, Piano.FILTER_MODE_LP)
  acid_configure_voice(Piano.VOICE, 1, 5, 80, 60, 120)
  self.active_offset = nil
end

-- White key i spans [x, x+w) -- w is KEY_W for every key except the
-- last, which takes whatever's left over so the keys always cover the
-- full window width exactly, regardless of whether WINDOW_W happens to
-- divide evenly by 7 (it doesn't: 200 / 7 leaves a 4px remainder).
function Piano:white_key_x(index)
  return index * Piano.KEY_W
end

function Piano:white_key_w(index)
  if index == Piano.WHITE_COUNT - 1 then return Piano.WINDOW_W - self:white_key_x(index) end
  return Piano.KEY_W
end

function Piano:black_key_x(slot)
  local white_index = Piano.BLACK_AFTER_WHITE[slot + 1]
  return self:white_key_x(white_index) + Piano.KEY_W - Piano.BLACK_W // 2
end

-- Black keys are drawn on top of (and are narrower than) the white
-- keys, so a touch landing in a black key's rect must win the hit test
-- even though it's also geometrically inside a white key's rect.
function Piano:hit_test(x)
  local slot = 0
  while slot < #Piano.BLACK_OFFSETS do
    local bx = self:black_key_x(slot)
    if x >= bx and x < bx + Piano.BLACK_W then return Piano.BLACK_OFFSETS[slot + 1] end
    slot = slot + 1
  end
  local index = x // Piano.KEY_W
  if index >= Piano.WHITE_COUNT then index = Piano.WHITE_COUNT - 1 end
  return Piano.WHITE_OFFSETS[index + 1]
end

function Piano:on_touch(x, y, pressed)
  if not pressed then
    if not self.active_offset then return end
    acid_stop_note(Piano.VOICE)
    local old = self.active_offset
    self.active_offset = nil
    self:redraw_offset(old)
    return
  end

  local offset = self:hit_test(x)
  if offset == self.active_offset then return end
  local old = self.active_offset
  self.active_offset = offset
  acid_play_note(Piano.VOICE, Piano.ROOT_ONA + offset, 45)
  if old then self:redraw_offset(old) end
  self:redraw_offset(offset)
end

-- Full clear + redraw every key -- correct, but only ever needed for a
-- genuine full repaint (the initial paint, or AcidApp#start's :moved
-- handler after a drag). NOT what on_touch calls per keypress -- doing
-- a full acid_clear_user_area on every single press/release flashed the
-- whole keyboard to blank before redrawing it (reported live as "its
-- redraw is flickery"). See redraw_offset for the actual per-press path.
function Piano:redraw()
  acid_clear_user_area()
  acid_draw_window_frame(self:window_title())
  self:draw_white_keys()
  self:draw_black_keys()
  acid_draw_window_border()
end

-- Repaints only the one key whose highlight state just changed --
-- a white key also repaints any black key(s) that visually overlap its
-- edges (a black key straddles the boundary between two white keys),
-- since a plain fill_rect over just the white key would otherwise erase
-- the sliver of black key sitting on top of it. A black key never needs
-- this the other way around -- it's drawn on top of, and entirely
-- contained within, the white keys under it.
function Piano:redraw_offset(offset)
  local white_index = index_of(Piano.WHITE_OFFSETS, offset)
  if white_index then
    self:draw_white_key(white_index)
    local slot = 0
    while slot < #Piano.BLACK_AFTER_WHITE do
      local wi = Piano.BLACK_AFTER_WHITE[slot + 1]
      if wi == white_index or wi == white_index - 1 then self:draw_black_key(slot) end
      slot = slot + 1
    end
    return
  end
  local black_slot = index_of(Piano.BLACK_OFFSETS, offset)
  if black_slot then self:draw_black_key(black_slot) end
end

function Piano:draw_white_keys()
  local i = 0
  while i < Piano.WHITE_COUNT do
    self:draw_white_key(i)
    i = i + 1
  end
end

function Piano:draw_white_key(index)
  local pressed = Piano.WHITE_OFFSETS[index + 1] == self.active_offset
  acid_fill_rect(self:white_key_x(index), Piano.KEY_Y, self:white_key_w(index), Piano.KEY_H,
                 pressed and Piano.PRESSED_COLOR or Piano.WHITE_COLOR)
  -- Dividing line against the previous key -- acid_fill_rect's clip
  -- against the window's own bounds means index 0's line at x=0 draws
  -- harmlessly over the border's own left edge.
  acid_fill_rect(self:white_key_x(index), Piano.KEY_Y, 1, Piano.KEY_H, Piano.BLACK_COLOR)
end

function Piano:draw_black_keys()
  local slot = 0
  while slot < #Piano.BLACK_OFFSETS do
    self:draw_black_key(slot)
    slot = slot + 1
  end
end

function Piano:draw_black_key(slot)
  local pressed = Piano.BLACK_OFFSETS[slot + 1] == self.active_offset
  acid_fill_rect(self:black_key_x(slot), Piano.KEY_Y, Piano.BLACK_W, Piano.BLACK_H,
                 pressed and Piano.PRESSED_COLOR or Piano.BLACK_COLOR)
end

Piano:new():start()
