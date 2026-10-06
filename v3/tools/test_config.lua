-- Headless tests for Config (apps/config.lua): state is read back from the
-- kernel, -/+ step by 10 once per press and clamp to 0..100, and the
-- wallpaper toggle flips.

local G = GAME

local function has(list, v)
  for _, x in ipairs(list) do if x == v then return true end end
  return false
end
local function tap(x, y) G:on_touch(x, y, false); G:on_touch(x, y, true) end

group("start")
eq(G.volume, 50, "the volume is read back from the kernel")
eq(G.wallpaper_on, true, "and so is the wallpaper flag")

group("volume")
CALLS = {}
tap(10, 70)
eq(CALLS, { { "set_volume", 40 } }, "- steps down by 10")
G:on_touch(10, 70, true)
eq(VOLUME, 40, "holding - steps once")
G:set_volume(130)
eq(VOLUME, 100, "the volume clamps at 100")
CALLS = {}
tap(160, 70)
eq(CALLS, {}, "+ at 100 changes nothing")
G:set_volume(-5)
eq(VOLUME, 0, "and clamps at 0")

group("wallpaper")
CALLS = {}
tap(50, 120)
eq(CALLS, { { "set_wallpaper", false } }, "the toggle turns the wallpaper off")
eq(G.wallpaper_on, false, "and remembers it")

group("drawing")
TEXTS = {}
G:redraw()
ok(has(TEXTS, "0%"), "redraw shows the volume")
ok(has(TEXTS, "OFF"), "and the toggle state")

group("font")
FONT_SCALE = 1
GAME:on_create()
TEXTS = {}
GAME:redraw()
ok(has(TEXTS, "FONT"), "a FONT section is drawn")
ok(has(TEXTS, "applies to newly opened apps"), "with a note that open windows keep their size")
CALLS = {}
GAME:on_touch(130, 160, false)
GAME:on_touch(130, 160, true)
eq(CALLS[1], { "set_font_scale", 2 }, "tapping LARGE sets Large")
eq(GAME.font_scale, 2, "and Config shows it")
GAME:on_touch(130, 160, true)
eq(#CALLS, 1, "holding the tap doesn't set it again")
GAME:on_touch(20, 160, false)
GAME:on_touch(20, 160, true)
eq(CALLS[2], { "set_font_scale", 1 }, "tapping NORMAL sets Normal")

CALLS = {}
FONT_SCALE = 2
GAME:on_touch(20, 160, false)
GAME:on_touch(20, 160, true)
eq(CALLS[1], { "set_font_scale", 1 }, "NORMAL still works after a cart changed the setting behind Config's back")

group("restart")
local function restarts()
  local n = 0
  for _, c in ipairs(CALLS) do if c[1] == "restart" then n = n + 1 end end
  return n
end
GAME:on_create()
WIN_W, WIN_H = 180, 298
TEXTS, TEXT_AT, RECTS = {}, {}, {}
GAME:redraw()
ok(has(TEXTS, "SYSTEM") and has(TEXTS, "RESTART"), "a SYSTEM section with a RESTART button is drawn")
local fits, why = drawn_inside_window()
ok(fits, "everything fits the 180x298 window" .. (why and (": " .. why) or ""))
CALLS, TEXTS = {}, {}
tap(90, 278)
eq({ GAME.restart_armed, restarts() }, { true, 0 }, "the first press only arms it")
ok(has(TEXTS, "SURE? PRESS AGAIN"), "and asks for a second press")
GAME:on_touch(90, 278, true)
eq(restarts(), 0, "holding that press doesn't restart")
tap(90, 278)
eq(restarts(), 1, "a second press restarts")
tap(90, 278)
CLOCK = CLOCK + 2999
GAME:on_idle()
eq(GAME.restart_armed, true, "it stays armed for up to 3 seconds")
CLOCK = CLOCK + 1
GAME:on_idle()
eq(GAME.restart_armed, false, "then disarms itself")
tap(90, 278)
tap(10, 70)
eq(GAME.restart_armed, false, "a press anywhere else disarms it")
tap(90, 278)
RESTART_OK = false
TEXTS = {}
tap(90, 278)
ok(has(TEXTS, "RESTART FAILED"), "a refused restart says so")
tap(10, 70)
eq(GAME.restart_failed, false, "a press elsewhere clears RESTART FAILED")
RESTART_OK = true
-- on_idle can be starved by held touches, so a stale armed button must not restart.
CALLS, TEXTS = {}, {}
tap(90, 278)
CLOCK = CLOCK + 3000
tap(90, 278)
eq(restarts(), 0, "a press after the 3 seconds are up doesn't restart")
eq(GAME.restart_armed, true, "it arms again instead")
ok(has(TEXTS, "SURE? PRESS AGAIN"), "and asks for a second press again")

group("developer mode")
DEV_MODE = false
GAME:on_create()
eq(GAME.dev_mode, false, "Developer Mode is read back from the kernel")
TEXTS = {}
GAME:redraw()
ok(has(TEXTS, "DEV MODE") and has(TEXTS, "system source is read-only"), "a DEV MODE row says source is read-only")
CALLS, TEXTS = {}, {}
tap(90, 220)
eq({ CALLS[1], GAME.dev_mode }, { { "set_dev_mode", true }, true }, "tapping it turns Developer Mode on")
ok(has(TEXTS, "system source is writable"), "and then says source is writable")
tap(90, 220)
eq({ CALLS[2], DEV_MODE }, { { "set_dev_mode", false }, false }, "tapping again turns it off")
