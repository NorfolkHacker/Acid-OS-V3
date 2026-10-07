-- Headless tests for AcidScrollbar (apps/lib/acid_scrollbar.lua): thumb
-- size and place, no bar when everything fits, a track tap pages, and a
-- thumb drag maps the pointer back to a scroll offset.

local S = AcidScrollbar
-- A 131 px track showing 11 of 20 rows: thumb 72 px, 59 px of travel, 9 offsets.
local H, TOTAL, VISIBLE = 131, 20, 11

group("thumb")
ok(not S.needed(11, 11), "no bar when everything fits")
ok(S.needed(12, 11), "a bar once one row overflows")
eq(S.max_offset(20, 11), 9, "the last page starts at total - visible")
eq(S.max_offset(5, 11), 0, "and never below 0")
eq({ S.thumb(H, TOTAL, VISIBLE, 0) }, { 0, 72 }, "the thumb is as tall as the visible share, at the top")
eq({ S.thumb(H, TOTAL, VISIBLE, 9) }, { 59, 72 }, "at the last offset it reaches the bottom")
eq({ S.thumb(H, TOTAL, VISIBLE, 5) }, { 32, 72 }, "and moves in proportion between")
eq({ S.thumb(100, 1000, 10, 0) }, { 0, S.MIN_THUMB }, "a huge list still gets a grabbable thumb")

group("press")
eq({ S.press(H, TOTAL, VISIBLE, 0, 10) }, { 0, 10 }, "pressing the thumb starts a drag, remembering where it was grabbed")
eq({ S.press(H, TOTAL, VISIBLE, 0, 100) }, { 9, nil }, "a tap below the thumb pages down, clamped to the end")
eq({ S.press(H, TOTAL, VISIBLE, 9, 10) }, { 0, nil }, "a tap above the thumb pages up, clamped to the start")
eq({ S.press(200, 100, 10, 50, 0) }, { 40, nil }, "a page is one screenful")

group("drag")
eq(S.drag(H, TOTAL, VISIBLE, 10, 10 + 59), 9, "dragging the thumb to the bottom shows the last page")
eq(S.drag(H, TOTAL, VISIBLE, 10, 10 + 30), 5, "halfway is the nearest offset")
eq(S.drag(H, TOTAL, VISIBLE, 10, -50), 0, "dragging past the top stops at the start")
eq(S.drag(H, TOTAL, VISIBLE, 10, 500), 9, "dragging past the bottom stops at the end")

group("draw and hit")
RECTS = {}
S.draw(213, 28, H, TOTAL, VISIBLE, 0)
eq(RECTS, { { 213, 28, 6, 131, S.TRACK_COLOR }, { 214, 28, 4, 72, S.THUMB_COLOR } }, "track, then the thumb inset by 1")
RECTS = {}
S.draw(213, 28, H, TOTAL, VISIBLE, 9)
eq(RECTS[2][2], 28 + 59, "the thumb is drawn at its offset")
RECTS = {}
S.draw(213, 28, H, 11, 11, 0)
eq(#RECTS, 0, "nothing is drawn when everything fits")
ok(S.hit(213, 28, H, 213, 28), "the track's top-left corner is on the bar")
ok(S.hit(213, 28, H, 218, 158), "and so is its bottom-right")
ok(not S.hit(213, 28, H, 212, 50), "left of the bar is not")
ok(not S.hit(213, 28, H, 215, 27), "nor above the track")

group("on its side")
RECTS = {}
S.draw_h(40, 200, H, TOTAL, VISIBLE, 9)
eq(RECTS, { { 40, 200, 131, 6, S.TRACK_COLOR }, { 40 + 59, 201, 72, 4, S.THUMB_COLOR } },
  "a horizontal bar: the track, then the thumb at its offset, inset by 1")
ok(S.hit_h(40, 200, H, 40, 205) and S.hit_h(40, 200, H, 170, 200), "its corners are on it")
ok(not S.hit_h(40, 200, H, 171, 200) and not S.hit_h(40, 200, H, 50, 206), "right of it and below it are not")
