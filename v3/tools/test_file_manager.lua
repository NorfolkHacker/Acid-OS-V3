-- Headless tests for File Manager (apps/file_manager.lua): the listing (..
-- only below the root, directories vs sized files, violet manifests),
-- keeping the selection on screen, what Enter does to each kind of entry,
-- the preview, going up a level, and taps acting once per hold.

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

group("wasm manifests")
-- Spec §15.4: a runtime = wasm manifest launches <name>.wasm, still
-- through the canonical v3/apps path.
FS["v3/fsroot/App/toy.app.toml"] = "name = Toy\nw = 120\nh = 90\nruntime = wasm\nsource = cart\n"
G.dir = "v3/fsroot/App"
CALLS = {}
G:launch_manifest("toy.app.toml")
eq(CALLS[1], { "spawn", "v3/apps/toy.wasm", 120, 90, "" }, "a runtime = wasm manifest launches its .wasm through the canonical path")
