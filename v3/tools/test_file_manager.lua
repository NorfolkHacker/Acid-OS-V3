-- Headless tests for File Manager (apps/file_manager.lua): the listing (..
-- only below the root, directories vs sized files, violet manifests),
-- keeping the selection on screen, what Enter does to each kind of entry,
-- the preview, going up a level, taps acting once per hold, folders listed
-- before files, and the scroll bar on the listing and the preview.

local G = GAME
local F = FileManagerApp

local function has(list, v)
  for _, x in ipairs(list) do if x == v then return true end end
  return false
end
local function key(k) G:on_key(k, true) end
local function names()
  local out = {}
  for i, e in ipairs(G.entries) do out[i] = e.name end
  return out
end

group("listing")
eq(G.entries[1].name, "(error: not found)", "an unreadable root lists the error")
local long = {}
for i = 1, 20 do long[i] = "line " .. i end
local many = {}
for i = 1, 20 do many[i] = string.format("f%02d.txt", i) end
FS["v3/fsroot"] = { "readme.txt", "Help", "App", "long.txt", "Many" }
FS["v3/fsroot/readme.txt"] = "hello"
FS["v3/fsroot/long.txt"] = table.concat(long, "\n")
FS["v3/fsroot/Help"] = { "about.txt" }
FS["v3/fsroot/Help/about.txt"] = "a\nb\nc\n"
FS["v3/fsroot/App"] = { "tetris.lua", "tetris.app.toml" }
FS["v3/fsroot/App/tetris.app.toml"] = "name = Tetris\nw = 160\nh = 160\n"
FS["v3/fsroot/App/tetris.lua"] = "-- code"
FS["v3/fsroot/Many"] = many
G:on_create()
eq(names(), { "App", "Help", "Many", "long.txt", "readme.txt" }, "the root lists sorted names with no ..")
eq({ G.entries[1].dir, G.entries[5].dir, G.entries[5].size }, { true, false, 5 }, "directories and file sizes are told apart")
TEXTS = {}
G:redraw()
ok(has(TEXTS, "[App]"), "a directory row is bracketed")
ok(has(TEXTS, " readme.txt (5B)"), "a file row shows its size")
eq(G:entry_color({ name = "t.app.toml", dir = false }), F.TOML_COLOR, "manifests are violet")

group("opening entries")
key(AcidKeys.DOWN)
key(AcidKeys.ENTER)
eq(G.dir, "v3/fsroot/Help", "Enter on a directory opens it")
eq(names(), { "..", "about.txt" }, ".. appears below the root")
key(AcidKeys.ENTER)
eq(G.dir, "v3/fsroot", "Enter on the .. row goes up a level")
key(AcidKeys.DOWN)
key(AcidKeys.ENTER)
key(AcidKeys.DOWN)
key(AcidKeys.ENTER)
eq(G.preview, "a\nb\nc\n", "Enter on a plain file opens a preview")
key(AcidKeys.ESCAPE)
eq(G.preview, nil, "Escape closes the preview")
key(AcidKeys.BACKSPACE)
eq(G.dir, "v3/fsroot", "Backspace goes up a level")
key(AcidKeys.ENTER)
CALLS = {}
key(AcidKeys.DOWN)
key(AcidKeys.ENTER)
eq(CALLS[1], { "spawn", "v3/apps/tetris.lua", 160, 160, "" }, "a manifest launches its app through the canonical v3/apps path")
key(AcidKeys.DOWN)
key(AcidKeys.ENTER)
eq(CALLS[2], { "spawn", "v3/apps/editor.lua", 420, 280, "v3/fsroot/App/tetris.lua" }, "a .lua opens in Editor with its path")

group("preview and taps")
G.dir = "v3/fsroot"
G:scan_dir()
G:open_preview("long.txt")
for _ = 1, 15 do key(AcidKeys.DOWN) end
eq(G.preview_scroll, 9, "preview scrolling stops at the last page")
key(AcidKeys.UP)
eq(G.preview_scroll, 8, "and scrolls back up")
G:on_touch(10, 50, false)
G:on_touch(10, 50, true)
eq(G.preview, nil, "a tap closes the preview")
G:on_touch(10, 50, true)
eq(G.dir, "v3/fsroot", "holding the tap doesn't act again")
G:on_touch(10, 50, false)
G:on_touch(10, 50, true)
eq(G.dir, "v3/fsroot/Help", "a fresh tap on a row opens it")

group("scrolling")
G.dir = "v3/fsroot/Many"
G:scan_dir()
for _ = 1, 15 do key(AcidKeys.DOWN) end
eq(G.scroll, 5, "the selection scrolls into view")

group("folders first")
FS["v3/fsroot/Mixed"] = { "b.txt", "zeta", "a.txt", "Alpha" }
FS["v3/fsroot/Mixed/zeta"] = {}
FS["v3/fsroot/Mixed/Alpha"] = {}
G.dir = "v3/fsroot/Mixed"
G:scan_dir()
eq(names(), { "..", "Alpha", "zeta", "a.txt", "b.txt" }, ".. first, then folders, then files, each sorted")

group("scroll bar")
-- Many: .. plus 20 files = 21 rows, 11 visible, offsets 0-10. The bar is
-- the 6 px column inside the right border (x 213-218), its track starting
-- below the path header (y 28, 131 px): thumb 68 px, 63 px of travel.
local function thumbs()
  local n = 0
  for _, r in ipairs(RECTS) do if r[5] == AcidScrollbar.THUMB_COLOR then n = n + 1 end end
  return n
end
local function tap(x, y) G:on_touch(x, y, false); G:on_touch(x, y, true) end
G.dir = "v3/fsroot/Many"
G:scan_dir()
RECTS = {}
G:redraw()
eq(thumbs(), 1, "an overflowing listing draws a scroll bar")
tap(215, 120)
eq(G.scroll, 10, "a tap below the thumb pages down")
eq(G.selected, 0, "without moving the selection")
eq(G.dir, "v3/fsroot/Many", "or opening anything")
tap(215, 28 + 63 + 5)
G:on_touch(215, 28 + 5, true)
eq(G.scroll, 0, "dragging the thumb to the top scrolls back")
G:on_touch(400, 28 + 63 + 5, true)
eq(G.scroll, 10, "the drag keeps following the pointer off the bar")
G:on_touch(400, 28 + 63 + 5, false)
eq(G.bar_grab, nil, "releasing ends the drag")
tap(10, 40)
eq(G.preview_name, "f11.txt", "a tap on a row after scrolling opens the row under the pointer")
G.preview = nil
G.dir = "v3/fsroot"
G:scan_dir()
G:open_preview("long.txt")
tap(215, 150)
eq(G.preview_scroll, 9, "the preview has a scroll bar too")
ok(G.preview ~= nil, "and tapping it doesn't close the preview")
G.preview = nil
G.dir = "v3/fsroot/Help"
G:scan_dir()
RECTS = {}
G:redraw()
eq(thumbs(), 0, "no scroll bar when everything fits")

group("wasm manifests")
-- Spec §15.4: a runtime = wasm manifest launches <name>.wasm, still
-- through the canonical v3/apps path.
FS["v3/fsroot/App/toy.app.toml"] = "name = Toy\nw = 120\nh = 90\nruntime = wasm\nsource = cart\n"
G.dir = "v3/fsroot/App"
CALLS = {}
G:launch_manifest("toy.app.toml")
eq(CALLS[1], { "spawn", "v3/apps/toy.wasm", 120, 90, "" }, "a runtime = wasm manifest launches its .wasm through the canonical path")

group("sprite files")
FS["v3/fsroot/Art"] = { "ship.spr" }
FS["v3/fsroot/Art/ship.spr"] = "acid-sprite 1\n"
G.dir = "v3/fsroot/Art"
G:scan_dir()
for i, e in ipairs(G.entries) do
  if e.name == "ship.spr" then G.selected = i - 1 end
end
CALLS = {}
G:activate_selected()
eq(CALLS[1], { "spawn", "v3/apps/sprite.lua", 360, 260, "v3/fsroot/Art/ship.spr" }, "a .spr opens in Sprite Paint with its path")

group("song and sound files")
FS["v3/fsroot/Music"] = { "a.trk", "b.snd" }
FS["v3/fsroot/Music/a.trk"] = "acid-track 1\n"
FS["v3/fsroot/Music/b.snd"] = "gate on\n"
G.dir = "v3/fsroot/Music"
G:scan_dir()
local function pick(name)
  for i, e in ipairs(G.entries) do
    if e.name == name then G.selected = i - 1 end
  end
end
pick("a.trk")
CALLS = {}
G:activate_selected()
eq(CALLS[1], { "spawn", "v3/apps/tracker.lua", 480, 320, "v3/fsroot/Music/a.trk" }, "a .trk opens in Acid Tracker with its path")
pick("b.snd")
CALLS = {}
G:activate_selected()
eq(CALLS[1], { "spawn", "v3/apps/editor.lua", 420, 280, "v3/fsroot/Music/b.snd" }, "a .snd opens in the Editor")
