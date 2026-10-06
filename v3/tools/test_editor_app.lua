-- Headless tests for Editor (apps/editor.lua + editor/*.lua): what opens,
-- the temp-file-then-rename save with a final newline, the own-source .bak,
-- failures that never touch the file, the two-press close, running a file
-- at its manifest's size, goto and find.

local G = GAME
local function key(k) G:on_key(k, true) end
local function type_text(s) for i = 1, #s do key(s:byte(i)) end end
local function esc(ch) key(AcidKeys.ESCAPE); key(ch:byte()) end
local function find_call(kind)
  for _, c in ipairs(CALLS) do if c[1] == kind then return c end end
  return nil
end

group("opening")
eq(G.path, "v3/fsroot/Home/notes.txt", "no launch argument opens the notes file")
eq(G.buf:lines(), { "" }, "a missing file opens empty")
eq(G.hl_on, false, "highlighting is off for a .txt file")
FS["v3/fsroot/Home/a.lua"] = "local x = 1\nreturn x\n"
LAUNCH_ARG = "v3/fsroot/Home/a.lua"
G:on_create()
eq(G.buf:lines(), { "local x = 1", "return x" }, "the launch argument's file opens")
eq(G.hl_on, true, "highlighting is on for a .lua file")

group("save")
type_text("-- hi")
CALLS = {}
esc("s")
local tmp = "v3/fsroot/Home/a.lua.editor-save-tmp"
eq(CALLS, { { "write", tmp }, { "rename", tmp, "v3/fsroot/Home/a.lua" } }, "save writes a temp file, then renames it over the file")
eq(FS["v3/fsroot/Home/a.lua"], "-- hilocal x = 1\nreturn x\n", "the saved text ends with a newline")
eq(G.message, "saved", "and reports saved")
eq(G.buf:modified(), false, "saving clears the modified flag")

FS["v3/apps/editor.lua"] = "old"
LAUNCH_ARG = "v3/apps/editor.lua"
G:on_create()
type_text("x")
CALLS = {}
esc("s")
eq(CALLS[1], { "write", "v3/apps/editor.lua.bak" }, "an own-source file is backed up first")
eq(FS["v3/apps/editor.lua.bak"], "old", "the backup holds the file as it was on disk")
FAIL_WRITES["v3/apps/editor.lua.bak"] = true
type_text("y")
esc("s")
eq(G.message, "saved (backup failed)", "a failed backup still saves")
FAIL_WRITES = {}

LAUNCH_ARG = "v3/fsroot/Home/a.lua"
G:on_create()
type_text("z")
FAIL_WRITES[tmp] = true
esc("s")
eq(G.message, "save failed", "a failed write reports save failed")
eq(FS["v3/fsroot/Home/a.lua"], "-- hilocal x = 1\nreturn x\n", "and leaves the file untouched")
FAIL_WRITES = {}
FAIL_RENAMES[tmp] = true
esc("s")
eq(G.message, "save failed", "a failed rename reports save failed")
eq(FS[tmp], nil, "and its temp file is deleted")
FAIL_RENAMES = {}

FAIL_WRITES[tmp] = "read only"
esc("s")
eq(G.message, "read only: save as to Home, or turn on DEV MODE in Config", "a locked save says how to get round it")
FAIL_WRITES = {}

group("close")
esc("q")
eq(G.running, nil, "q with unsaved changes doesn't close")
eq(G.message, "unsaved -- ESC q again to close", "and warns")
type_text("x")
esc("q")
eq(G.running, nil, "typing disarms the quit: the next q only warns again")
esc("q")
eq(G.running, false, "a second q closes")

group("run")
FS["v3/fsroot/App/tetris.lua"] = "-- t"
FS["v3/fsroot/App/tetris.app.toml"] = "name = Tetris\nw = 160\nh = 150\n"
LAUNCH_ARG = "v3/fsroot/App/tetris.lua"
G:on_create()
CALLS = {}
esc("!")
eq(find_call("spawn"), { "spawn", "v3/apps/tetris.lua", 160, 150, "" }, "! saves, then runs the canonical path at its manifest's size")
local function index_of(kind) for i, c in ipairs(CALLS) do if c[1] == kind then return i end end end
eq((index_of("rename") or 99) < (index_of("spawn") or 0), true, "the save happens before the run")
FS["v3/fsroot/Home/g.txt"] = "one\ntwo\nthree\n"
LAUNCH_ARG = "v3/fsroot/Home/g.txt"
G:on_create()
esc("!")
eq(G.message, "not a lua file", "! refuses a file that isn't Lua")

group("goto and find")
esc("g")
type_text("3")
key(AcidKeys.ENTER)
eq(G.buf.cy, 2, "goto 3 moves to the third line")
esc("g")
type_text("9")
key(AcidKeys.ENTER)
eq(G.message, "no line 9", "goto past the end says so")
esc("t")
esc("/")
type_text("thr")
key(AcidKeys.ENTER)
eq({ G.buf.cx, G.buf.cy }, { 0, 2 }, "find moves to the match")
