-- System Monitor re-lays out every page when the window is resized, and
-- keeps its page.
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
for _, size in ipairs({ { 360, 260 }, { 160, 120 } }) do
  for page = 0, 3 do
    G.page = page
    resize_app(size[1], size[2])
    local fits, what = drawn_inside_window()
    if fits then fits, what = drawn_text_clear() end
    ok(fits, "page " .. names[page + 1] .. " fits at " .. size[1] .. "x" .. size[2] .. (what and (": " .. what) or ""))
    eq(G.page, page, "page " .. names[page + 1] .. " is kept by the resize")
  end
end
