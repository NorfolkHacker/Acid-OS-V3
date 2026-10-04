AboutApp = AcidApp:extend("AboutApp")

-- Shows Help/about.txt in a fixed window. Constants are AboutApp.X fields;
-- sizes derive from the font and window size so it works at Large too.

local CW, CH = acid_font_size()
local WW, WH = acid_window_size()

AboutApp.TITLE_BAR_H = 16
AboutApp.LINE_H = CH + 2
AboutApp.COLS = (WW - 24) // CW
AboutApp.ABOUT_FILE = "v3/fsroot/Help/about.txt"

AboutApp.TEXT_COLOR = 0xD4E6DB -- THEME_TEXT
AboutApp.BG_COLOR = 0x050607   -- THEME_BG

-- Splits on "\n": empty fields between newlines are kept,
-- trailing empty fields are dropped.
local function split_lines(text)
  local lines = {}
  for line in (text .. "\n"):gmatch("(.-)\n") do lines[#lines + 1] = line end
  while #lines > 0 and lines[#lines] == "" do lines[#lines] = nil end
  return lines
end

function AboutApp:on_create()
  self.lines = self:read_lines()
end

-- Reads Help/about.txt rather than hardcoding its own copy of the same
-- text -- one place to update, and it doubles as a live example of what
-- file_manager's Help folder is for.
function AboutApp:read_lines()
  local text = acid_fs_read(AboutApp.ABOUT_FILE)
  -- An unreadable file falls back to one line (acid_fs_read returns nil).
  if not text then return { "Acid OS v3" } end
  return split_lines(text)
end

function AboutApp:redraw()
  acid_clear_user_area()
  acid_draw_window_frame(self:window_title())
  local y = AboutApp.TITLE_BAR_H + 4
  for _, line in ipairs(self.lines) do
    if y + AboutApp.LINE_H > WH then break end
    acid_draw_text(line:sub(1, AboutApp.COLS), 4, y, AboutApp.TEXT_COLOR, AboutApp.BG_COLOR)
    y = y + AboutApp.LINE_H
  end
  acid_draw_window_border()
end

AboutApp:new():start()
