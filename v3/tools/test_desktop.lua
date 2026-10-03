-- Headless tests for the desktop (apps/desktop.lua): manifest discovery
-- (sorted, malformed skipped, menu = false hidden), Menu/Back once per
-- press, dropdown rows, the tap-outside close, taskbar activation, giving
-- z-order back on a bare background tap, the clock format, and redrawing
-- only on a changed state signature.

local G = GAME

local function tap(x, y) G:on_touch(x, y, false); G:on_touch(x, y, true) end

group("discovery")
eq(#LAUNCHER, 0, "an unreadable apps directory leaves the launcher empty")
FS["v3/apps"] = { "zeta.app.toml", "about.app.toml", "about.lua", "bad.app.toml", "huge.app.toml", "game.app.toml", "lib" }
FS["v3/apps/about.app.toml"] = "# a comment\nname = About\nw = 180\nh = 150\n"
FS["v3/apps/bad.app.toml"] = "name = Bad\n"
FS["v3/apps/game.app.toml"] = "name = Game\nw = 160\nh = 160\nmenu = false\n"
FS["v3/apps/huge.app.toml"] = "name = Huge\nw = 99999999999\nh = 80\n"
FS["v3/apps/zeta.app.toml"] = "name = Zeta\nw = 100\nh = 80\nmulti = true\nlibs = lib/x.lua\n"
G:on_create()
eq(#LAUNCHER, 3, "a manifest without w and h, or with one out of i32 range, is skipped")
eq(LAUNCHER[1], { "v3/apps/about.lua", "About", 180, 150, false, "" }, "manifests register in sorted order, .app.toml -> .lua")
eq(LAUNCHER[3], { "v3/apps/zeta.lua", "Zeta", 100, 80, true, "lib/x.lua" }, "multi and libs are read")
eq(G:menu_indices(), { 0, 2 }, "menu = false apps stay registered but out of the dropdown")

WINDOWS[0] = { "v3/apps/desktop.lua", 0, 0, 640, 204, false }
WINDOWS[1] = { "v3/apps/about.lua", 38, 52, 180, 150, true }

group("taskbar before Menu")
CALLS = {}
tap(65, 10)
eq(CALLS[1], { "activate", 1 }, "a taskbar tap works before Menu was ever opened")

group("menu")
CALLS = {}
TEXTS = {}
RECTS = {}
tap(10, 10)
eq(G.mode, "launcher", "Menu opens the dropdown")
-- Spec 11.6: the open dropdown's golden waits for 5b, so pin what
-- draw_dropdown draws through the recorded calls instead.
local function has(list, v)
  for _, x in ipairs(list) do
    if x == v then return true end
  end
  return false
end
ok(has(TEXTS, "About") and has(TEXTS, "Zeta") and has(TEXTS, "Back"), "the dropdown lists the visible apps, and the button reads Back")
ok(not has(TEXTS, "Game"), "a menu = false app isn't listed")
local found = false
for _, r in ipairs(RECTS) do
  if r[1] == 0 and r[2] == 24 and r[3] == 150 and r[4] == 180 and r[5] == DesktopApp.BG_COLOR then found = true end
end
ok(found, "the dropdown panel is filled below the strip")
eq(CALLS[1], { "activate", 0 }, "opening raises desktop, found by its own path")
G:on_touch(10, 10, true)
eq(G.mode, "launcher", "holding Menu doesn't toggle it back")
CALLS = {}
tap(10, 10)
eq(G.mode, "windows", "Back closes the dropdown")
eq({ CALLS[1], CALLS[2] }, { { "to_back" }, { "repaint", 0, 24, 640, 180 } }, "closing goes to the back, then repaints below the strip")

group("dropdown")
tap(10, 10)
CALLS = {}
tap(20, 24 + 18 + 5)
eq(CALLS[1], { "launch", 2 }, "the second row launches the second visible app")
eq(G.mode, "windows", "and closes the dropdown")
tap(10, 10)
CALLS = {}
tap(200, 60)
eq(CALLS[1], { "to_back" }, "a tap right of the dropdown closes it without launching")

group("taskbar")
CALLS = {}
tap(65, 10)
eq(CALLS[1], { "activate", 1 }, "slot 1 activates the first window that isn't desktop")
CALLS = {}
tap(300, 100)
eq(CALLS, { { "to_back" } }, "a tap on bare desktop below the closed strip gives z-order back")

group("clock and redraws")
eq(G:clock_text(), "09:05 02/10", "the clock is HH:MM DD/MM")
G:redraw()
DRAW_CALLS = 0
G:on_idle()
eq(DRAW_CALLS, 0, "an unchanged strip isn't redrawn")
TIME[5] = 6
G:on_idle()
ok(DRAW_CALLS > 0, "a new minute redraws it")
DRAW_CALLS = 0
CALLS = {}
WALLPAPER = false
G:on_idle()
ok(DRAW_CALLS > 0, "so does the wallpaper flag")
eq(CALLS[#CALLS], { "repaint", 0, 24, 640, 180 }, "and repaints below the strip")
tap(10, 10)
DRAW_CALLS = 0
TIME[5] = 7
G:on_idle()
eq(DRAW_CALLS, 0, "idle never redraws while the dropdown is open")

group("names")
eq(G:short_name("v3/apps/acid_blaster.lua"), "acid_bla", "taskbar names lose dir and extension, cut to 8")

group("wasm manifests")
-- Spec §15.4: runtime = wasm means the app is <name>.wasm, not <name>.lua.
FS["v3/apps/wasmy.app.toml"] = "name = Wasmy\nw = 90\nh = 70\nruntime = wasm\nsource = cart\n"
LAUNCHER = {}
G:register_launchable("v3/apps/wasmy.app.toml")
eq(LAUNCHER[1], { "v3/apps/wasmy.wasm", "Wasmy", 90, 70, false, "" }, "a runtime = wasm manifest registers its .wasm")
