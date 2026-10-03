-- Headless tests for System Monitor (apps/sysmon.lua): page wrap, the
-- two-tap close (including focused-row alignment), once-a-second history
-- capped at HIST_LEN, bar scaling, the tasks page rows, and its string
-- helpers.

local G = GAME
local S = SysMon

local function has(list, v)
  for _, x in ipairs(list) do if x == v then return true end end
  return false
end
local function tap(x, y) G:on_touch(x, y, false); G:on_touch(x, y, true) end
local function closes()
  local out = {}
  for _, c in ipairs(CALLS) do
    if c[1] == "close" then out[#out + 1] = c[2] end
  end
  return out
end

group("start")
eq(REFRESHES, 1, "tasks are sampled once at start")

group("pages")
tap(4, 150)
eq(G.page, 3, "< on the first page wraps to the last")
tap(195, 150)
eq(G.page, 0, "> on the last page wraps to the first")

group("closing windows")
WINDOWS[0] = { "v3/apps/desktop.lua", 0, 0, 640, 204, false }
WINDOWS[1] = { "v3/apps/about.lua", 38, 52, 180, 150, true }
WINDOWS[2] = { "v3/apps/config.lua", 56, 70, 180, 140, false }
G:redraw()
eq(G.row_indices, { 0, false, 2 }, "one row entry per window, false for the focused one")
CALLS = {}
tap(180, 56)
eq(G.kill_armed, 2, "the first tap on a row below the focused one arms that window")
eq(closes(), {}, "and closes nothing")
tap(180, 56)
eq(closes(), { 2 }, "the second tap closes it")
eq(G.kill_armed, nil, "and disarms")
CALLS = {}
tap(180, 44)
eq(closes(), {}, "the focused row has no close button")
eq(G.kill_armed, nil, "and arms nothing")
tap(180, 32)
tap(195, 150)
eq(G.kill_armed, nil, "turning the page disarms")

group("history")
FRAMES.composited = 60
FRAMES.skipped = 2
CLOCK = 1000
G:on_idle()
eq(G.hist_composited, { 60 }, "a second's composited frames become one sample")
eq(G.hist_skipped, { 2 }, "and so do skipped ones")
CLOCK = 1500
G:on_idle()
eq(#G.hist_composited, 1, "samples are once a second")
for i = 1, 25 do
  CLOCK = 1000 + i * 1000
  G:on_idle()
end
eq(#G.hist_composited, S.HIST_LEN, "history keeps the last HIST_LEN seconds")

group("bar graph")
RECTS = {}
G:draw_bar_graph(50, 41, { 10, 30 }, { 10, 0 })
eq(RECTS, {
  { 2, 65, 7, 13, S.MUTED_COLOR },
  { 2, 78, 7, 13, S.HARD_COLOR },
  { 10, 51, 7, 40, S.HARD_COLOR },
}, "bars scale to the busiest second, skipped stacked above composited")

group("tasks page")
TASKS = { { "router", "running", 12 }, { "v3/apps/sysmon.", "blocked", 3 } }
G.page = 1
TEXTS = {}
G:redraw()
ok(has(TEXTS, "MEM 1234K"), "the tasks page shows memory")
ok(has(TEXTS, "sysmon      blocked  3%"), "and one padded row per task")
MEM_KB = -1
TEXTS = {}
G:redraw()
ok(has(TEXTS, "MEM n/a"), "unknown memory reads n/a")

group("helpers")
eq(G:short_name("v3/apps/sysmon."), "sysmon", "a name Linux cut mid-extension loses the dot")
eq(G:short_name("v3/apps/about.lua"), "about", "and a whole one loses .lua")
eq(G:pad("abcdefghijkl", 4), "abcd", "pad truncates")
eq(G:pad("ab", 4), "ab  ", "and pads with spaces")
