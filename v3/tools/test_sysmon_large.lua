-- System Monitor at Large: every one of its 4 pages draws only inside the
-- window, with the lists full.
local G = GAME

for i = 0, 7 do
  WINDOWS[i] = { "v3/apps/some_long_named_app_" .. i .. ".lua", 0, 0, 100, 100, i == 1 }
end
TASKS = {}
for i = 1, 14 do TASKS[i] = { "v3/apps/sysmon.", "blocked", 100 } end
MEM_KB = 123456
FRAMES.composited, FRAMES.skipped = 60, 12
for i = 1, 25 do
  CLOCK = i * 1000
  FRAMES.composited = FRAMES.composited + 60
  FRAMES.skipped = FRAMES.skipped + 12
  G:on_idle()
end
VOICES = 8

local names = { "windows", "tasks", "compositor", "synth" }
for page = 0, 3 do
  TEXT_AT, RECTS = {}, {}
  G.page = page
  G:redraw()
  local fits, what = drawn_inside_window()
  if fits then fits, what = drawn_text_clear() end
  ok(fits, "page " .. names[page + 1] .. " fits at Large" .. (what and (": " .. what) or ""))
end
