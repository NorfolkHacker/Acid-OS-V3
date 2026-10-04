-- Terminal's scroll bar and scroll-back (apps/terminal.lua): the bar beside
-- the scroll-back rows, paging and dragging, and typing or new output
-- bringing the view back to the newest lines. 260x160 at Normal: 13 rows.
local G = GAME
local S = AcidScrollbar

local function lines(n)
  G.lines = {}
  for i = 1, n do G.lines[i] = "line " .. i end
end
local function frame()
  TEXT_AT, RECTS = {}, {}
  G:redraw()
end
local function first_row()
  for _, t in ipairs(TEXT_AT) do
    if t[3] == 17 then return t[1] end
  end
end
local function track_drawn()
  for _, r in ipairs(RECTS) do
    if r[1] == 253 and r[5] == S.TRACK_COLOR then return true end
  end
  return false
end
local function touch(x, y) G:on_touch(x, y, true) end
local function release() G:on_touch(0, 0, false) end

group("geometry")
lines(40)
frame()
eq({ G:bar_geometry() }, { 253, 16, 130 }, "the bar sits inside the right border, beside the 13 scroll-back rows")
ok(track_drawn(), "40 lines need the bar")
eq({ G:scroll_offset(), first_row() }, { 27, "line 28" }, "the view starts on the newest lines")
local clear = true
for _, t in ipairs(TEXT_AT) do
  if t[3] < 146 and t[2] + #t[1] * FONT_W > 253 then clear = false end
end
ok(clear, "no scroll-back text reaches the bar's column")
G.lines[40] = string.rep("w", 80)
frame()
local fits, why = drawn_inside_window()
ok(fits, "a long line still fits" .. (why and (": " .. why) or ""))
lines(40)

group("paging and dragging")
touch(255, 17)
frame()
eq({ G:scroll_offset(), G.follow, first_row() }, { 14, false, "line 15" }, "a press above the thumb pages up one screenful")
touch(255, 17)
eq(G:scroll_offset(), 14, "holding the press doesn't page again")
release()
local ty = S.thumb(130, 40, 13, 14)
touch(255, 16 + ty + 1)
touch(255, 16)
eq({ G:scroll_offset(), G.follow }, { 0, false }, "dragging the thumb to the top shows the first lines")
touch(255, 16 + 130)
eq({ G:scroll_offset(), G.follow }, { 27, true }, "dragging it to the bottom follows the newest lines again")
release()

group("back to the bottom")
touch(255, 17)
release()
G:on_key(string.byte("e"), true)
eq({ G:scroll_offset(), G.follow }, { 27, true }, "a key press returns to the newest lines")
touch(255, 17)
release()
G.input = "echo hi"
G:on_key(AcidKeys.ENTER, true)
frame()
eq(G:scroll_offset(), #G.lines - 13, "running a command shows its output at the bottom")
ok(G.lines[#G.lines] == "hi", "the output is the newest line")

group("no bar when everything fits")
lines(5)
frame()
ok(not track_drawn(), "5 lines need no bar")
touch(255, 17)
release()
eq({ G:scroll_offset(), G.follow }, { 0, true }, "a press where the bar would be does nothing")
lines(40)
touch(100, 50)
release()
eq({ G:scroll_offset(), G.follow }, { 27, true }, "a press anywhere else does nothing")

group("resizing")
resize_app(400, 300)
eq({ G:bar_geometry() }, { 393, 16, 270 }, "the bar follows a resize")
eq(G:scroll_offset(), 40 - 27, "a view at the bottom stays at the bottom")
