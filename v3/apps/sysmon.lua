-- System Monitor -- a paged window onto Acid OS's own kernel/compositor/
-- audio internals (not generic OS bookkeeping: there's no process table or
-- heap allocator here worth showing, this OS's own distinctive machinery
-- is its windows, its dirty-flag compositor, and its hand-built synth).
--
--   Page 1: WINDOWS -- every open window, [X] to close one (two clicks,
--           same "click again to confirm" safety as a real task manager)
--   Page 2: TASKS -- every real kernel task (not just windowed apps --
--           the router, the timer service, IDLE too), each one's state
--           and CPU%, plus this process's real memory footprint
--   Page 3: COMPOSITOR -- a live bar graph of composited vs. skipped
--           frames/sec over the last ~20 seconds
--   Page 4: SYNTH -- one box per voice, lit while it's sounding
--
-- Tap the "<" / ">" arrows in the bottom nav bar to switch pages.
SysMon = AcidApp:extend("SysMon")

local CW, CH = acid_font_size()
local WW, WH = acid_window_size()

SysMon.TITLE_BAR_H = 16
SysMon.LINE_H = CH + 3
SysMon.NAV_H = CH + 4
SysMon.PAGES = { "windows", "tasks", "compositor", "synth" }
SysMon.HIST_LEN = 20
-- Refreshing the task table suspends the scheduler for its duration, so it
-- is sampled once a second, not on every ~200ms on_idle tick, same
-- throttling idea as the compositor history below.
SysMon.TASK_SAMPLE_MS = 1000
SysMon.HISTORY_MS = 1000 -- between history samples

SysMon.BG_COLOR = 0x050607    -- THEME_BG
SysMon.TEXT_COLOR = 0xD4E6DB  -- THEME_TEXT
SysMon.MUTED_COLOR = 0x9DAAA3 -- THEME_MUTED
SysMon.HARD_COLOR = 0x00FF66  -- THEME_HARD
SysMon.PANEL_COLOR = 0x0B1712 -- THEME_PANEL

SysMon.BTN_W = 5 * CW
SysMon.ROW_H = CH + 4
-- Columns of a window's name before the close button (68 = the fixed
-- right margin that leaves 22 columns at Normal).
SysMon.LABEL_COLS = (WW - 68) // CW

function SysMon:on_create()
  self.page = 0
  self.kill_armed = nil
  self.hist_composited = {}
  self.hist_skipped = {}
  self.prev_composited = acid_composited_frames()
  self.prev_skipped = acid_skipped_frames()
  self.next_sample_at = acid_now_ms() + SysMon.HISTORY_MS
  self.next_task_sample_at = 0
  self.row_indices = {}
  self:sample_tasks()
  self:redraw()
end

function SysMon:content_bottom()
  return WH - SysMon.NAV_H
end

function SysMon:on_idle()
  self:sample_history()
  self:sample_tasks()
  self:redraw()
end

function SysMon:sample_tasks()
  local now = acid_now_ms()
  if now < self.next_task_sample_at then return end
  self.next_task_sample_at = now + SysMon.TASK_SAMPLE_MS
  acid_refresh_tasks()
end

-- Once a second (matching a real "per-second" rate stat), turns the two
-- ever-growing cumulative counters the router tracks into a delta -- the
-- actual thing worth graphing -- and keeps the last HIST_LEN of them.
function SysMon:sample_history()
  local now = acid_now_ms()
  if now < self.next_sample_at then return end
  self.next_sample_at = now + SysMon.HISTORY_MS
  local cur_c = acid_composited_frames()
  local cur_s = acid_skipped_frames()
  self.hist_composited[#self.hist_composited + 1] = cur_c - self.prev_composited
  if #self.hist_composited > SysMon.HIST_LEN then table.remove(self.hist_composited, 1) end
  self.hist_skipped[#self.hist_skipped + 1] = cur_s - self.prev_skipped
  if #self.hist_skipped > SysMon.HIST_LEN then table.remove(self.hist_skipped, 1) end
  self.prev_composited = cur_c
  self.prev_skipped = cur_s
end

function SysMon:redraw()
  acid_clear_user_area()
  acid_draw_window_frame(self:window_title())
  if self.page == 0 then
    self:draw_windows_page()
  elseif self.page == 1 then
    self:draw_tasks_page()
  elseif self.page == 2 then
    self:draw_compositor_page()
  else
    self:draw_synth_page()
  end
  self:draw_nav()
  acid_draw_window_border()
end

-- List of { index, info } where info is the packed acid_window_info results
-- (info[1] = name ... info[6] = focused).
function SysMon:open_windows()
  local list = {}
  local max = acid_window_max()
  for i = 0, max - 1 do
    local info = { acid_window_info(i) }
    if info[1] ~= nil then list[#list + 1] = { i, info } end
  end
  return list
end

-- self.row_indices has exactly one entry per drawn row, in the same order --
-- including the focused row, as false (Lua arrays can't hold nil
-- placeholders) -- so on_touch's row-number hit test (computed straight
-- from y, the same way every row's y was) can index straight into it.
-- Only pushing entries for rows that actually GOT a close button (skipping
-- the focused one) would shift every row after the focused one out of
-- alignment, so a tap on any close button below the focused row would land
-- on the wrong window's index or out of bounds.
function SysMon:draw_windows_page()
  local y = SysMon.TITLE_BAR_H + 2
  acid_draw_text("WINDOWS", 2, y, SysMon.MUTED_COLOR, SysMon.BG_COLOR)
  y = y + SysMon.LINE_H
  self.row_indices = {}
  for _, entry in ipairs(self:open_windows()) do
    local index, info = entry[1], entry[2]
    if y + SysMon.ROW_H > self:content_bottom() then break end
    local focused = info[6]
    local label = (focused and "> " or "  ") .. self:short_name(info[1])
    local color = focused and SysMon.HARD_COLOR or SysMon.TEXT_COLOR
    acid_draw_text(label:sub(1, SysMon.LABEL_COLS), 2, y + 2, color, SysMon.BG_COLOR)
    if focused then
      self.row_indices[#self.row_indices + 1] = false
    else
      self:draw_close_button(index, y)
      self.row_indices[#self.row_indices + 1] = index
    end
    y = y + SysMon.ROW_H
  end
end

function SysMon:draw_close_button(index, y)
  local armed = self.kill_armed == index
  local label = armed and "sure?" or "close"
  local bg = armed and SysMon.HARD_COLOR or SysMon.PANEL_COLOR
  local fg = armed and 0x050607 or SysMon.MUTED_COLOR
  local x = WW - SysMon.BTN_W - 4
  acid_fill_rect(x, y, SysMon.BTN_W, SysMon.ROW_H - 1, bg)
  acid_draw_text(label, x + 2, y + 2, fg, bg)
end

-- Every task the scheduler actually knows about -- the router, the timer
-- service, IDLE, and one per open app window -- not just the windowed apps
-- the WINDOWS page manages. Read-only: ending an arbitrary task from the
-- outside has no clean shutdown path (a window closes because the app's
-- own event loop notices the close event and exits itself; IDLE or the
-- timer service have no such loop), so there's no [X] here.
function SysMon:draw_tasks_page()
  local y = SysMon.TITLE_BAR_H + 2
  local mem = acid_mem_used_kb()
  local mem_text = mem >= 0 and ("MEM " .. mem .. "K") or "MEM n/a"
  acid_draw_text("TASKS", 2, y, SysMon.MUTED_COLOR, SysMon.BG_COLOR)
  acid_draw_text(mem_text, WW - #mem_text * CW - 2, y, SysMon.MUTED_COLOR, SysMon.BG_COLOR)
  y = y + SysMon.LINE_H
  local count = acid_task_count()
  for i = 0, count - 1 do
    if y + SysMon.ROW_H > self:content_bottom() then break end
    local name, state, cpu = acid_task_info(i)
    local color = state == "running" and SysMon.HARD_COLOR or SysMon.TEXT_COLOR
    local row = self:pad(self:short_name(name), 11) .. " " .. self:pad(state, 8) .. " " .. cpu .. "%"
    acid_draw_text(row, 2, y + 1, color, SysMon.BG_COLOR)
    y = y + SysMon.ROW_H
  end
end

function SysMon:draw_compositor_page()
  local y = SysMon.TITLE_BAR_H + 2
  acid_draw_text("COMPOSITOR", 2, y, SysMon.MUTED_COLOR, SysMon.BG_COLOR)
  y = y + SysMon.LINE_H
  local cur_c = self.hist_composited[#self.hist_composited] or 0
  local cur_s = self.hist_skipped[#self.hist_skipped] or 0
  acid_draw_text("frames/s: " .. cur_c .. "  skip/s: " .. cur_s, 2, y, SysMon.TEXT_COLOR, SysMon.BG_COLOR)
  y = y + SysMon.LINE_H + 2
  self:draw_bar_graph(y, self:content_bottom() - y, self.hist_composited, self.hist_skipped)
end

-- Two-series bar graph, one column per second sampled -- composited
-- frames (bright) stacked on top of skipped ones (dim), scaled to
-- whichever second in the window had the busiest total. Bars, not a line
-- chart, to match this OS's own blocky look.
function SysMon:draw_bar_graph(y0, h, composited, skipped)
  if h <= 4 then return end
  local peak = 1
  for i = 1, #composited do
    local total = composited[i] + skipped[i]
    if total > peak then peak = total end
  end
  local col_w = 8
  local x = 2
  for i = 1, #composited do
    local c = composited[i]
    local s = skipped[i]
    local c_h = c * (h - 1) // peak
    local s_h = s * (h - 1) // peak
    local bar_bottom = y0 + h
    if s_h > 0 then
      acid_fill_rect(x, bar_bottom - s_h - c_h, col_w - 1, s_h, SysMon.MUTED_COLOR)
    end
    if c_h > 0 then
      acid_fill_rect(x, bar_bottom - c_h, col_w - 1, c_h, SysMon.HARD_COLOR)
    end
    x = x + col_w
  end
end

function SysMon:draw_synth_page()
  local y = SysMon.TITLE_BAR_H + 2
  acid_draw_text("SYNTH", 2, y, SysMon.MUTED_COLOR, SysMon.BG_COLOR)
  y = y + SysMon.LINE_H
  local active = acid_active_voice_count()
  acid_draw_text(active .. "/8 voices active", 2, y, SysMon.TEXT_COLOR, SysMon.BG_COLOR)
  y = y + SysMon.LINE_H + 4
  local box = 18
  local gap = 4
  for i = 0, 7 do
    local x = 2 + i * (box + gap)
    local on = i < active
    acid_fill_rect(x, y, box, box, on and SysMon.HARD_COLOR or SysMon.PANEL_COLOR)
  end
end

function SysMon:draw_nav()
  local y = self:content_bottom()
  acid_fill_rect(0, y, WW, SysMon.NAV_H, SysMon.PANEL_COLOR)
  acid_draw_text("<", 4, y + 2, SysMon.TEXT_COLOR, SysMon.PANEL_COLOR)
  acid_draw_text(">", WW - CW - 4, y + 2, SysMon.TEXT_COLOR, SysMon.PANEL_COLOR)
  local label = (self.page + 1) .. "/" .. #SysMon.PAGES
  acid_draw_text(label, (WW - #label * CW) // 2, y + 2, SysMon.MUTED_COLOR, SysMon.PANEL_COLOR)
end

function SysMon:on_touch(x, y, pressed)
  if not pressed then
    self.touch_held = false
    return
  end
  if self.touch_held then return end
  self.touch_held = true

  local n = #SysMon.PAGES
  if y >= self:content_bottom() then
    if x < 20 then
      self:turn_page((self.page - 1 + n) % n)
    elseif x > WW - CW - 14 then
      self:turn_page((self.page + 1) % n)
    end
    return
  end

  if self.page ~= 0 then return end
  local row = (y - SysMon.TITLE_BAR_H - SysMon.LINE_H) // SysMon.ROW_H
  if row < 0 or row >= #self.row_indices then return end
  if x < WW - SysMon.BTN_W - 4 then return end
  local index = self.row_indices[row + 1]
  if not index then return end -- index 0 is truthy in Lua; only false skips
  if self.kill_armed == index then
    acid_close_window(index)
    self.kill_armed = nil
  else
    self.kill_armed = index
  end
  self:redraw()
end

function SysMon:turn_page(page)
  self.page = page
  self.kill_armed = nil
  self:redraw()
end

-- Left-align in a fixed width, truncating anything longer.
function SysMon:pad(str, width)
  local s = str:sub(1, width)
  return s .. string.rep(" ", width - #s)
end

function SysMon:short_name(app_name)
  local base = app_name:match("[^/]*$")
  -- A real task's name (the TASKS page's source, unlike WINDOWS') is
  -- hard-truncated by the kernel at a fixed length before this sees it, so
  -- a long script path loses its ".lua" mid-extension (e.g. "v3/apps/sysmon."
  -- arrives already cut). Stripping a bare trailing "." first handles that
  -- case too, harmlessly, since no real app name otherwise ends in one.
  base = base:gsub("%.$", "")
  base = base:gsub("%.lua$", "")
  return base
end

SysMon:new():start()
