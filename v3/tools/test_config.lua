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
