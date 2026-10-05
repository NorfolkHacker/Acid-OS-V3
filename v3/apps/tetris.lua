Tetris = AcidGame:extend("Tetris")

-- Constants are Tetris.X fields. The grid is a 1-based list of ROWS rows
-- of COLS cells, `false` meaning empty; x/y values are 0-based and +1 is
-- added at each access.

Tetris.WINDOW_W = 160
Tetris.WINDOW_H = 160
Tetris.TITLE_BAR_H = 16

Tetris.BG_COLOR = 0x050607      -- THEME_BG
Tetris.TEXT_COLOR = 0xD4E6DB    -- THEME_TEXT
Tetris.GRID_LINE = 0x0B1712     -- THEME_PANEL

Tetris.CELL = 8
Tetris.COLS = 8
Tetris.ROWS = 16
Tetris.GRID_X = 4
Tetris.GRID_Y = Tetris.TITLE_BAR_H + 4
Tetris.GRID_W = Tetris.COLS * Tetris.CELL
Tetris.GRID_H = Tetris.ROWS * Tetris.CELL

Tetris.PANEL_X = Tetris.GRID_X + Tetris.GRID_W + 8

-- One fall step every FALL_TICKS on_tick calls -- AcidGame's own TICK_MS
-- (50ms) times this is the real fall interval (10 * 50ms = 500ms/row).
Tetris.FALL_TICKS = 10

-- Each piece: one cell layout per rotation state, as {x,y} pairs inside
-- a 4x4 box (rotated around that box's own center, not the cell grid) --
-- the standard, simplest-to-hardcode representation for a first cut with
-- no wall-kick table. O has one physical rotation, listed 4 times so
-- PIECES[name][rotation + 1] is always valid regardless of rotation index.
Tetris.PIECES = {
  I = { { {0,1},{1,1},{2,1},{3,1} }, { {2,0},{2,1},{2,2},{2,3} },
        { {0,2},{1,2},{2,2},{3,2} }, { {1,0},{1,1},{1,2},{1,3} } },
  O = { { {1,0},{2,0},{1,1},{2,1} }, { {1,0},{2,0},{1,1},{2,1} },
        { {1,0},{2,0},{1,1},{2,1} }, { {1,0},{2,0},{1,1},{2,1} } },
  T = { { {1,0},{0,1},{1,1},{2,1} }, { {1,0},{1,1},{2,1},{1,2} },
        { {0,1},{1,1},{2,1},{1,2} }, { {1,0},{0,1},{1,1},{1,2} } },
  S = { { {1,0},{2,0},{0,1},{1,1} }, { {1,0},{1,1},{2,1},{2,2} },
        { {1,0},{2,0},{0,1},{1,1} }, { {1,0},{1,1},{2,1},{2,2} } },
  Z = { { {0,0},{1,0},{1,1},{2,1} }, { {2,0},{1,1},{2,1},{1,2} },
        { {0,0},{1,0},{1,1},{2,1} }, { {2,0},{1,1},{2,1},{1,2} } },
  J = { { {0,0},{0,1},{1,1},{2,1} }, { {1,0},{2,0},{1,1},{1,2} },
        { {0,1},{1,1},{2,1},{2,2} }, { {1,0},{1,1},{0,2},{1,2} } },
  L = { { {2,0},{0,1},{1,1},{2,1} }, { {1,0},{1,1},{1,2},{2,2} },
        { {0,1},{1,1},{2,1},{0,2} }, { {0,0},{1,0},{1,1},{1,2} } },
}
-- A Lua table has no insertion order, so the piece order
-- (which the colours and piece index depend on) is spelled out.
Tetris.PIECE_NAMES = { "I", "O", "T", "S", "Z", "J", "L" }

-- A distinct, vivid hue per piece from the real 256-step color wheel
-- (AcidPalette, loaded into every app automatically) instead of cycling
-- through the same few reused UI theme colors, which is what every piece
-- of on-screen content in this codebase did before (the user's own "up
-- the colours" request). Evenly spaced around the full wheel so all seven
-- pieces read as clearly different at a glance.
Tetris.PIECE_COLORS = {}
local piece_i = 0
while piece_i < #Tetris.PIECE_NAMES do
  Tetris.PIECE_COLORS[#Tetris.PIECE_COLORS + 1] = AcidPalette.hue(piece_i * 256 // #Tetris.PIECE_NAMES)
  piece_i = piece_i + 1
end

Tetris.VOICE = 6
Tetris.FILTER_MODE_LP = 1
Tetris.LINE_NOTES = { 60, 64, 67, 72 }
Tetris.OVER_NOTES = { 40, 36, 32, 28 }

-- PIECE_NAMES.index(name), 0-based; names are always valid.
local function piece_index(name)
  for i, n in ipairs(Tetris.PIECE_NAMES) do
    if n == name then return i - 1 end
  end
  return nil
end

local function new_row()
  local row = {}
  for x = 1, Tetris.COLS do row[x] = false end
  return row
end

function Tetris:on_create()
  -- A restart arrives while the game-over (or line) sound may still be
  -- gated; tick_sfx is the only note-off in normal flow, so stop the voice
  -- now and drop the pending tick count so tick_sfx doesn't act on it.
  acid_stop_note(Tetris.VOICE)
  self.sfx_ticks = nil
  acid_configure_filter(180, 3, Tetris.FILTER_MODE_LP)
  acid_configure_voice(Tetris.VOICE, 1, 3, 40, 40, 60)
  self.grid = {}
  for y = 1, Tetris.ROWS do self.grid[y] = new_row() end
  self.score = 0
  self.game_over = false
  self.fall_counter = 0
  self.next_name = self:random_piece_name()
  self:spawn_piece()
end

function Tetris:random_piece_name()
  return Tetris.PIECE_NAMES[acid_now_ms() % #Tetris.PIECE_NAMES + 1]
end

function Tetris:spawn_piece()
  self.piece_name = self.next_name
  self.next_name = self:random_piece_name()
  self.rotation = 0
  self.px = Tetris.COLS // 2 - 2
  self.py = 0
  if not self:piece_fits(self.px, self.py, self.rotation) then self.game_over = true end
  if self.game_over then self:trigger_sfx(Tetris.OVER_NOTES, 3, 25, 30, 3) end
end

function Tetris:cells_for(name, rotation)
  return Tetris.PIECES[name][rotation + 1]
end

function Tetris:piece_fits(px, py, rotation)
  for _, cell in ipairs(self:cells_for(self.piece_name, rotation)) do
    local x = px + cell[1]
    local y = py + cell[2]
    if x < 0 or x >= Tetris.COLS or y >= Tetris.ROWS then return false end
    if y >= 0 then
      if self.grid[y + 1][x + 1] then return false end
    end
  end
  return true
end

function Tetris:lock_piece()
  for _, cell in ipairs(self:cells_for(self.piece_name, self.rotation)) do
    local x = self.px + cell[1]
    local y = self.py + cell[2]
    if y >= 0 then
      self.grid[y + 1][x + 1] = Tetris.PIECE_COLORS[piece_index(self.piece_name) + 1]
    end
  end
  self:clear_lines()
  self:spawn_piece()
end

function Tetris:clear_lines()
  local cleared = 0
  local y = Tetris.ROWS - 1
  while y >= 0 do
    local full = true
    for _, c in ipairs(self.grid[y + 1]) do
      if not c then full = false; break end
    end
    if full then
      table.remove(self.grid, y + 1)
      table.insert(self.grid, 1, new_row())
      cleared = cleared + 1
    else
      y = y - 1
    end
  end
  if cleared == 0 then return end
  self.score = self.score + cleared * cleared * 100
  self:trigger_sfx(Tetris.LINE_NOTES, cleared > 2 and 4 or 2, 20, 35, 3)
end

function Tetris:trigger_sfx(notes, count, rate_ms, volume, ticks)
  acid_play_note(Tetris.VOICE, notes[1], volume)
  acid_trigger_arp(Tetris.VOICE, notes[1], notes[2], notes[3], notes[4], count, rate_ms)
  self.sfx_ticks = ticks
end

function Tetris:tick_sfx()
  if not self.sfx_ticks then return end
  self.sfx_ticks = self.sfx_ticks - 1
  if self.sfx_ticks <= 0 then
    acid_stop_note(Tetris.VOICE)
    self.sfx_ticks = nil
  end
end

function Tetris:on_tick()
  self:tick_sfx()
  if not self.game_over then
    self.fall_counter = self.fall_counter + 1
    if self.fall_counter >= Tetris.FALL_TICKS then
      self.fall_counter = 0
      self:fall()
    end
  end
  self:draw()
end

function Tetris:fall()
  if self:piece_fits(self.px, self.py + 1, self.rotation) then
    self.py = self.py + 1
  else
    self:lock_piece()
  end
end

-- The router sends a TOUCH event on every ~16ms tick for as long as the
-- mouse stays held, not just once on the initial press (see
-- file_manager.lua's own comment on this same guard) -- without it, a
-- single held tap would repeatedly move or spin the piece for as long
-- as the mouse stayed down instead of acting once per tap.
function Tetris:on_touch(x, y, pressed)
  if not pressed then
    self.touch_held = false
    return
  end
  if self.touch_held then return end
  self.touch_held = true
  if self.game_over then
    self:on_create()
    self:draw()
    return
  end
  if x < Tetris.WINDOW_W // 3 then
    self:move(-1)
  elseif x > Tetris.WINDOW_W * 2 // 3 then
    self:move(1)
  else
    self:rotate_piece()
  end
  self:draw()
end

function Tetris:on_key(code, pressed)
  if not pressed then return end
  if self.game_over then
    self:on_create()
    self:draw()
    return
  end
  if code == AcidKeys.LEFT then
    self:move(-1)
  elseif code == AcidKeys.RIGHT then
    self:move(1)
  elseif code == AcidKeys.UP then
    self:rotate_piece()
  elseif code == AcidKeys.DOWN then
    self:fall()
  end
  self:draw()
end

function Tetris:move(dx)
  if self:piece_fits(self.px + dx, self.py, self.rotation) then
    self.px = self.px + dx
  end
end

function Tetris:rotate_piece()
  local next_rotation = (self.rotation + 1) % 4
  if self:piece_fits(self.px, self.py, next_rotation) then
    self.rotation = next_rotation
  end
end

function Tetris:draw()
  if not self:focused() then return end
  -- One frame, shown whole: no flash of the cleared window.
  acid_begin_frame()
  acid_clear_user_area()
  acid_draw_window_frame(self:window_title())
  self:draw_grid()
  self:draw_panel()
  if self.game_over then
    acid_draw_text("GAME OVER", Tetris.GRID_X + 4, Tetris.GRID_Y + Tetris.GRID_H // 2 - 4, Tetris.TEXT_COLOR, Tetris.BG_COLOR)
  end
  acid_draw_window_border()
  acid_end_frame()
end

function Tetris:draw_grid()
  acid_fill_rect(Tetris.GRID_X, Tetris.GRID_Y, Tetris.GRID_W, Tetris.GRID_H, Tetris.GRID_LINE)
  local y = 0
  while y < Tetris.ROWS do
    local x = 0
    while x < Tetris.COLS do
      local color = self.grid[y + 1][x + 1]
      if color then
        acid_fill_rect(Tetris.GRID_X + x * Tetris.CELL + 1, Tetris.GRID_Y + y * Tetris.CELL + 1, Tetris.CELL - 1, Tetris.CELL - 1, color)
      end
      x = x + 1
    end
    y = y + 1
  end
  if not self.game_over then
    for _, cell in ipairs(self:cells_for(self.piece_name, self.rotation)) do
      local cx = self.px + cell[1]
      local cy = self.py + cell[2]
      if cy >= 0 then
        local color = Tetris.PIECE_COLORS[piece_index(self.piece_name) + 1]
        acid_fill_rect(Tetris.GRID_X + cx * Tetris.CELL + 1, Tetris.GRID_Y + cy * Tetris.CELL + 1, Tetris.CELL - 1, Tetris.CELL - 1, color)
      end
    end
  end
end

function Tetris:draw_panel()
  acid_draw_text("SCORE", Tetris.PANEL_X, Tetris.GRID_Y, Tetris.TEXT_COLOR, Tetris.BG_COLOR)
  acid_draw_text(tostring(self.score), Tetris.PANEL_X, Tetris.GRID_Y + 10, Tetris.TEXT_COLOR, Tetris.BG_COLOR)
  acid_draw_text("NEXT", Tetris.PANEL_X, Tetris.GRID_Y + 26, Tetris.TEXT_COLOR, Tetris.BG_COLOR)
  for _, cell in ipairs(self:cells_for(self.next_name, 0)) do
    local color = Tetris.PIECE_COLORS[piece_index(self.next_name) + 1]
    acid_fill_rect(Tetris.PANEL_X + cell[1] * 6, Tetris.GRID_Y + 36 + cell[2] * 6, 5, 5, color)
  end
end

Tetris:new():start()
