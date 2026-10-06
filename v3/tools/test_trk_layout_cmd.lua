-- TrkLayout (apps/tracker/layout.lua) and TrkCmd (apps/tracker/cmd.lua).

group("layout at the default size")
local L = TrkLayout.compute(480, 320)
eq({ L.status_y, L.head_y, L.grid_y }, { 19, 29, 38 }, "the status line, channel headings, then the grid")
eq({ L.ord_y, L.ins_y, L.msg_y }, { 250, 284, 310 }, "orders, instrument and message lines along the bottom")
eq(L.rows, 26, "26 pattern rows show")
eq(L.ch_x, { 23, 119, 215, 311 }, "four channels, 16 characters apart")
ok(L.ch_x[4] + TrkLayout.CELL_CHARS * TrkLayout.CH_W <= 420 - L.x, "the four channels fit even the smallest window")
local S = TrkLayout.compute(420, 240)
eq({ S.rows, S.ord_y }, { 16, 170 }, "the smallest window still shows 16 rows")

group("commands")
eq(TrkCmd.parse("speed 9"), { name = "speed", args = { "9" }, rest = "9" }, "a command and its argument")
eq(TrkCmd.parse("title  My   Song "), { name = "title", args = { "My", "Song" }, rest = "My   Song" },
  "rest keeps the text as typed")
eq({ TrkCmd.parse("zap") }, { nil, "unknown command: zap" }, "an unknown command is named")
eq({ TrkCmd.parse("speed") }, { nil, "usage: speed 1-31" }, "a missing argument shows the usage")
eq({ TrkCmd.parse("  ") }, { nil, "" }, "an empty line is nothing")
eq({ TrkCmd.int("12"), TrkCmd.int("-3"), TrkCmd.int("x"), TrkCmd.int("99999999999999999999") }, { 12, -3 },
  "whole numbers only, and none too big to hold")
eq({ TrkCmd.hex("2"), TrkCmd.hex("3F"), TrkCmd.hex("G1"), TrkCmd.hex("123") }, { 2, 63 },
  "instrument numbers are one or two hex digits")
eq({ TrkCmd.home_path("groove"), TrkCmd.home_path("music/a.trk") },
  { "v3/fsroot/Home/groove.trk", "v3/fsroot/Home/music/a.trk" }, "song names live under Home, .trk added")
eq({ TrkCmd.home_path("../x"), TrkCmd.home_path("/etc/x"), TrkCmd.home_path("a//b"), TrkCmd.home_path("a/") }, {},
  "and can't climb out of it")
