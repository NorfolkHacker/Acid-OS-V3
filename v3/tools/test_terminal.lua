-- Headless tests for Terminal (apps/terminal.lua): lexical path resolution
-- that can't climb above the fsroot, ls/cat/cd and their error strings, run
-- by launcher name, history, and the eggs driving the poll timeout.

local G = GAME

local function type_line(s)
  for i = 1, #s do G:on_key(s:byte(i), true) end
  G:on_key(AcidKeys.ENTER, true)
end
local function last(n) return G.lines[#G.lines - (n or 1) + 1] end

FS["v3/fsroot"] = { "Help", "Home" }
FS["v3/fsroot/Help"] = { "about.txt" }
FS["v3/fsroot/Help/about.txt"] = "one\ntwo\n"
FS["v3/fsroot/Home"] = { "notes.txt", "b.txt", "a.txt" }

group("start")
eq(G.lines[1], "Acid OS v3 terminal -- type help", "the welcome line names v3")
eq(G.cwd, "v3/fsroot", "the shell starts at the fsroot")

group("paths")
eq(G:resolve_path(".."), "v3/fsroot", ".. at the root stays at the root")
eq(G:resolve_path("/Help"), "v3/fsroot/Help", "an absolute path maps under the root")
eq(G:resolve_path("Help/./"), "v3/fsroot/Help", ". and empty parts are ignored")
type_line("cd Home")
eq(G.cwd, "v3/fsroot/Home", "cd enters a directory")
eq(G:resolve_path("../Help"), "v3/fsroot/Help", ".. climbs one level")
eq(G:resolve_path("Home/../../../.."), "v3/fsroot", "a multi-segment climb stops at the root")
type_line("cd nowhere")
eq(last(), "cd: nowhere: not found", "cd into a missing directory reports v3's error")
eq(G.cwd, "v3/fsroot/Home", "and stays put")

group("ls and cat")
type_line("ls")
eq({ last(3), last(2), last(1) }, { "a.txt", "b.txt", "notes.txt" }, "ls lists sorted names")
type_line("cat /Help/about.txt")
eq({ last(2), last(1) }, { "one", "two" }, "cat prints the file's lines")
type_line("cat")
eq(last(), "cat: missing file", "cat needs a file")
type_line("cat /Help/none")
eq(last(), "cat: not found", "cat of a missing file reports v3's error")

group("help and run")
type_line("help")
eq(last(), "help, clear, pwd, cd, ls, cat, echo, run <app>, play <file>", "help lists the commands")
LAUNCHER = {
  { "v3/apps/about.lua", "About", 180, 150, false, "" },
  { "v3/apps/sysmon.lua", "System Monitor", 200, 160, false, "" },
}
CALLS = {}
type_line("run ABOUT")
eq(CALLS, { { "launch", 0 } }, "run matches a launcher name case-insensitively")
CALLS = {}
type_line("run system   Monitor")
eq(CALLS, { { "launch", 1 } }, "run matches a two-word name, however it's spaced")
type_line("open sprite thing")
eq(last(), "run: no app named sprite thing", "a missing two-word name is reported whole")
type_line("run nothing")
eq(last(), "run: no app named nothing", "run reports a missing app")

group("echo and history")
type_line("echo hi")
eq(last(), "hi", "echo prints its arguments")
G:on_key(AcidKeys.UP, true)
eq(G.input, "echo hi", "Up recalls the last line")
G:on_key(AcidKeys.UP, true)
eq(G.input, "run nothing", "Up again goes further back")
G:on_key(AcidKeys.DOWN, true)
G:on_key(AcidKeys.DOWN, true)
eq(G.input, "", "Down past the newest line clears the input")
type_line("ls nowhere")
eq(last(), "ls: not found", "ls of a missing directory reports v3's error")

group("eggs")
eq(G:poll_timeout_ms(), 200, "idle polling is 200 ms")
type_line("DAVE")
ok(AcidEggs.active(), "an egg's name starts it, in any case")
eq(G:poll_timeout_ms(), AcidEggs.TICK_MS, "a running egg polls at its tick")
G:on_destroy()
ok(not AcidEggs.active(), "closing the window aborts the egg")

group("play")
FS["v3/fsroot/Home/a.trk"] = "acid-track 1\n"
SOUND_CALLS = {}
type_line("play /Home/a.trk")
eq(SOUND_CALLS, { { "parse", 1 }, { "play", 1, 0, 0 } }, "play loads a .trk and plays it from the start")
FS["v3/fsroot/Home/z.snd"] = "gate on\n"
SOUND_CALLS = {}
type_line("play /Home/z.snd")
eq(SOUND_CALLS, { { "stop" }, { "free", 1 }, { "sound_load", 1 }, { "sound_play", 1, "", 40 } },
  "playing a .snd stops the song first, then starts the sound")
SOUND_CALLS = {}
type_line("play")
eq(SOUND_CALLS, { { "stop" }, { "sound_stop", 1 }, { "sound_free", 1 } }, "play on its own stops everything, including a song a script started")
type_line("play /Home/missing.trk")
eq(last(), "play: not found", "a missing file is reported")
type_line("play notes.txt")
eq(last(), "play: not a .trk or .snd file", "other files are refused")
