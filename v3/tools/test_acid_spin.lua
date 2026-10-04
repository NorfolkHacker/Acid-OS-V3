-- Headless tests for Acid Spin (apps/acid_spin.lua).

local G = GAME
local W, H = 240, 200

local function snapshot()
  return { G.rx, G.ry, G.rz, G.step }
end

local function last_draw() return MESH_DRAWS[#MESH_DRAWS] end

local function fresh_frame()
  MESH_DRAWS, TEXT_AT, RECTS = {}, {}, {}
end

group("setup")
eq(#MESH_NEWS, 1, "one custom mesh is built")
eq(#MESH_NEWS[1][1], 42, "the star has 14 points")
eq(#MESH_NEWS[1][2], 24, "the star has 24 triangles")
eq(#G.meshes, 6, "six shapes")
eq(G.shape, 1, "starts on the cube")
eq(G.mode, 0, "starts in wire mode")
eq(G.speed, 3, "starts at speed 3")
eq(G.TICK_MS, 33, "33 ms tick")

group("shapes")
G:on_key(AcidKeys.RIGHT, true)
eq(G.shape, 2, "right goes to the next shape")
for _ = 1, 5 do G:on_key(AcidKeys.RIGHT, true) end
eq(G.shape, 1, "right wraps after 6")
G:on_key(AcidKeys.LEFT, true)
eq(G.shape, 6, "left wraps backwards")
G:on_key(AcidKeys.LEFT, true)
eq(G.shape, 5, "left goes to the previous shape")
G:on_key(AcidKeys.RIGHT, false)
eq(G.shape, 5, "a key release does nothing")
G.shape = 1

group("modes")
G:on_key(32, true)
eq(G.mode, 1, "space: wire to solid")
G:on_key(32, true)
eq(G.mode, 2, "space: solid to both")
G:on_key(32, true)
eq(G.mode, 0, "space: both to wire")

group("speed")
G:on_key(AcidKeys.UP, true)
eq(G.speed, 4, "up speeds up")
for _ = 1, 10 do G:on_key(AcidKeys.UP, true) end
eq(G.speed, 8, "speed clamps at 8")
for _ = 1, 10 do G:on_key(AcidKeys.DOWN, true) end
eq(G.speed, 1, "speed clamps at 1")
G.speed = 3

group("taps")
G:on_touch(10, 100, true)
eq(G.shape, 6, "left third: previous shape")
G:on_touch(10, 100, true)
eq(G.shape, 6, "a held tap acts once")
G:on_touch(10, 100, false)
G:on_touch(W - 10, 100, true)
eq(G.shape, 1, "right third: next shape")
G:on_touch(W - 10, 100, false)
G:on_touch(W // 2, 100, true)
eq(G.mode, 1, "middle third: cycle the mode")
G:on_touch(W // 2, 100, true)
eq(G.mode, 1, "a held middle tap acts once")
G:on_touch(W // 2, 100, false)
G:on_touch(W // 3, 100, true)
eq(G.mode, 2, "x = w/3 is the middle third")
G:on_touch(W // 3, 100, false)
G:on_touch(W // 2, 100, true)
G:on_touch(W // 2, 100, false)
eq(G.mode, 0, "mode wraps back to wire")

group("ticks")
G.rx, G.ry, G.rz, G.step, G.speed = 250, 250, 250, 0, 3
G:on_tick()
eq(snapshot(), { (250 + 3) % 256, (250 + 2) % 256, (250 + 1) % 256, 2 }, "angles advance by speed, 2/3 and 1/2; step by 2")
G.speed = 8
G.rx, G.ry, G.rz = 0, 0, 0
G:on_tick()
eq({ G.rx, G.ry, G.rz }, { 8, 5, 4 }, "speed 8 advances 8, 5, 4")
G.speed = 3

group("redraw counts")
G.mode = 0
fresh_frame()
G:on_tick()
eq(#MESH_DRAWS, 5, "wire mode: four afterimages and the current shape")
local cur = last_draw()
eq(cur[9], AcidPalette.hue(G.step), "the current shape has the hue colour")
ok(MESH_DRAWS[1][9] ~= cur[9], "afterimages are dimmer than the current shape")
local function lum(c) return (c >> 16 & 255) + (c >> 8 & 255) + (c & 255) end
ok(lum(MESH_DRAWS[1][9]) < lum(MESH_DRAWS[2][9]) and lum(MESH_DRAWS[2][9]) < lum(MESH_DRAWS[3][9])
  and lum(MESH_DRAWS[3][9]) < lum(MESH_DRAWS[4][9]) and lum(MESH_DRAWS[4][9]) < lum(cur[9]),
  "afterimages brighten oldest to newest")
G.mode = 1
fresh_frame()
G:on_tick()
eq(#MESH_DRAWS, 1, "solid mode: one draw")
G.mode = 2
fresh_frame()
G:on_tick()
eq(#MESH_DRAWS, 1, "both mode: one draw")
G.mode = 0

group("placement")
fresh_frame()
G:redraw()
cur = last_draw()
eq({ cur[2], cur[3] }, { W // 2, 16 + (H - 16) // 2 }, "centred below the title bar")
local fits, what = drawn_inside_window()
ok(fits, "everything fits the window" .. (what and (": " .. what) or ""))
-- 174 model units times size/64 stays inside the area under the title bar.
ok(174 * cur[4] // 64 <= (H - 16) // 2, "the rotated shape fits vertically")
ok(174 * cur[4] // 64 <= W // 2, "the rotated shape fits horizontally")
local label
for _, t in ipairs(TEXT_AT) do if t[3] == H - 12 then label = t[1] end end
ok(label and label:find("cube", 1, true), "the label row names the shape")

group("resize")
resize_app(400, 300)
cur = last_draw()
eq({ cur[2], cur[3] }, { 200, 158 }, "re-centred at 400x300")
ok(cur[4] > 30, "the shape grows with the window")
fits, what = drawn_inside_window()
ok(fits, "fits at 400x300" .. (what and (": " .. what) or ""))
resize_app(120, 100)
cur = last_draw()
eq({ cur[2], cur[3] }, { 60, 58 }, "re-centred at the minimum")
ok(174 * cur[4] // 64 <= 42, "the shape fits at the minimum")
fits, what = drawn_inside_window()
ok(fits, "fits at 120x100" .. (what and (": " .. what) or ""))
resize_app(W, H)

group("freeze")
G:freeze()
eq({ G.shape, G.mode, G.rx, G.ry, G.rz, G.step }, { 1, 1, 20, 30, 0, 0 }, "freeze() sets the golden pose")
G.mode = 0
GAME_FREEZE = true
local before = snapshot()
G:on_tick()
G:on_tick()
eq(snapshot(), before, "a frozen tick advances nothing")
GAME_FREEZE = nil
G.frozen = nil
G:on_tick()
ok(snapshot()[4] ~= before[4], "unfrozen ticks advance again")
LAUNCH_ARG = "freeze"
local G2 = AcidSpin:new()
G2:on_create()
eq({ G2.shape, G2.mode, G2.rx, G2.ry, G2.rz, G2.step }, { 1, 1, 20, 30, 0, 0 }, "the freeze launch argument sets the pose")
G2:on_tick()
eq({ G2.rx, G2.ry, G2.rz, G2.step }, { 20, 30, 0, 0 }, "and keeps it fixed")
LAUNCH_ARG = ""

group("destroy")
G:on_destroy()
eq(#MESH_FREES, 6, "every mesh is freed")
