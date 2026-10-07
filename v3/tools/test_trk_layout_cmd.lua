-- TrkLayout (apps/tracker/layout.lua) and TrkCmd (apps/tracker/cmd.lua).

group("layout at the default size")
local L = TrkLayout.compute(480, 320)
eq({ L.status_y, L.head_y, L.grid_y }, { 19, 29, 38 }, "the status line, track headings, then the grid")
eq({ L.bar_y, L.ord_y, L.ins_y, L.msg_y }, { 257, 266, 284, 310 }, "the tracks' scroll bar, orders, instrument and message lines")
eq(L.rows, 21, "21 pattern rows show, 10 px apart")
eq({ L.visible, TrkLayout.track_x(L, 0), TrkLayout.track_x(L, 5) }, { 6, 23, 383 }, "six of the eight tracks fit, 12 characters apart")
ok(TrkLayout.track_x(L, L.visible - 1) + TrkLayout.CELL_CHARS * TrkLayout.CH_W <= 480 - L.x, "and the last one shown fits")
local S = TrkLayout.compute(420, 240)
eq({ S.rows, S.visible }, { 13, 5 }, "the smallest window still shows 13 rows and 5 tracks")
eq(TrkLayout.compute(640, 320).visible, 8, "a wide window shows all eight")

group("commands")
eq(TrkCmd.parse("speed 9"), { name = "speed", args = { "9" }, rest = "9" }, "a command and its argument")
eq(TrkCmd.parse("title  My   Song "), { name = "title", args = { "My", "Song" }, rest = "My   Song" },
  "rest keeps the text as typed")
eq({ TrkCmd.parse("zap") }, { nil, "unknown command: zap" }, "an unknown command is named")
eq({ TrkCmd.parse("donor 4") }, { nil, "unknown command: donor" }, "there is no donor channel any more")
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
