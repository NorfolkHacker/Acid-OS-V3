-- Load Cart's scroll bar (apps/cart.lua): beside the list rows, paging and
-- dragging the view without moving the selection, and the keys bringing
-- the selection back into view. 13 rows fit the 300x210 window.
local app = GAME
local S = AcidScrollbar

local many = {}
for i = 1, 30 do many[i] = string.format("cart_%02d.cart", i) end
CART_ROOTS = { "v3/carts" }
CART_FS["v3/carts"] = many
for _, n in ipairs(many) do CART_FS["v3/carts/" .. n] = "-- name: X\n" end
app:on_create()
app:open_root("v3/carts")

local function frame()
  TEXT_AT, RECTS = {}, {}
  app:redraw()
end
local function track_drawn()
  for _, r in ipairs(RECTS) do
    if r[1] == 293 and r[5] == S.TRACK_COLOR then return true end
  end
  return false
end
local function touch(x, y) app:on_touch(x, y, true) end
local function release() app:on_touch(0, 0, false) end

group("geometry")
frame()
eq({ app:bar_geometry() }, { 293, 28, 156 }, "the bar sits inside the right border, beside the 13 list rows")
ok(track_drawn(), "30 carts need the bar")
local clear = true
for _, r in ipairs(RECTS) do
  if r[2] >= 28 and r[2] < 184 and r[1] < 293 and r[1] + r[3] > 293 then clear = false end
end
for _, t in ipairs(TEXT_AT) do
  if t[3] >= 28 and t[3] < 184 and t[2] + #t[1] * FONT_W > 293 then clear = false end
end
ok(clear, "list rows and their text stop short of the bar")

group("paging and dragging")
touch(295, 28 + 155)
eq({ app.scroll, app.selected, app.screen }, { 13, 0, "browse" }, "a press below the thumb pages down and leaves the selection")
touch(295, 28 + 155)
eq(app.scroll, 13, "holding the press pages once")
release()
local ty = S.thumb(156, 30, 13, 13)
touch(295, 28 + ty + 1)
touch(295, 28 + 156)
eq({ app.scroll, app.selected }, { 17, 0 }, "dragging the thumb to the bottom shows the last rows")
touch(295, 28)
eq(app.scroll, 0, "and back to the top shows the first")
release()

group("keys bring the selection back")
touch(295, 28 + 155)
release()
app:on_key(AcidKeys.DOWN, true)
eq({ app.selected, app.scroll }, { 1, 1 }, "Down moves the selection and scrolls it back into view")

group("no bar when everything fits")
CART_FS["v3/carts"] = { "one.cart" }
CART_FS["v3/carts/one.cart"] = "-- name: One\n"
app:open_root("v3/carts")
frame()
ok(not track_drawn(), "one cart needs no bar")
touch(295, 30)
release()
eq({ app.screen, app.scroll }, { "confirm", 0 }, "a tap at the right of a row still opens it")
