-- Editor's scroll bar (apps/editor.lua, editor/touch.lua): beside the text
-- rows, paging and dragging the view without moving the cursor or the
-- selection, and the next key bringing the view back to the cursor.
-- 420x280 at Normal: 25 rows of 64 columns.
local G = GAME
local L = EditorLayout
local S = AcidScrollbar

local function load(n, third)
  local lines = {}
  for i = 1, n do lines[i] = "line " .. i end
  if third then lines[3] = third end
  G.buf = Buffer.new(lines)
  G.scroll_y, G.scroll_x = 0, 0
end
local function frame()
  TEXT_AT, RECTS = {}, {}
  G:redraw()
end
local function track_drawn()
  for _, r in ipairs(RECTS) do
    if r[1] == 413 and r[5] == S.TRACK_COLOR then return true end
  end
  return false
end
local function touch(x, y) G:on_touch(x, y, true) end
local function release() G:on_touch(0, 0, false) end
local function key(k) G:on_key(k, true) end

group("geometry")
load(100, string.rep("x", 200))
frame()
eq({ G:bar_geometry() }, { 413, 16, 250 }, "the bar sits inside the right border, beside the 25 text rows")
eq(G:visible_cols(), 64, "the text keeps 64 columns beside it")
ok(track_drawn(), "100 lines need the bar")
local clear = true
for _, t in ipairs(TEXT_AT) do
  if t[3] < L.STATUS_Y and t[2] + #t[1] * FONT_W > 413 then clear = false end
end
for _, r in ipairs(RECTS) do
  if r[2] < L.STATUS_Y and r[1] < 413 and r[1] + r[3] > 413 then clear = false end
end
ok(clear, "no text or row background reaches the bar's column")
local tclear, twhy = drawn_text_clear()
ok(tclear, "and no text spills past the window" .. (twhy and (": " .. twhy) or ""))
G.hl_on = true
load(100, "local x = \"" .. string.rep("y", 200) .. "\" -- comment")
frame()
clear = true
for _, t in ipairs(TEXT_AT) do
  if t[3] < L.STATUS_Y and t[2] + #t[1] * FONT_W > 413 then clear = false end
end
for _, r in ipairs(RECTS) do
  if r[2] < L.STATUS_Y and r[1] < 413 and r[1] + r[3] > 413 then clear = false end
end
ok(clear, "with highlighting on nothing reaches the bar's column either")
G.hl_on = false
ok(EditorLayout.own_source(G, "v3/apps/lib/acid_scrollbar.lua") and EditorLayout.own_source(G, "v3/fsroot/Source/lib/acid_scrollbar.lua"),
  "the scroll bar lib Editor loads counts as its own source")
load(100)

group("paging and dragging move only the view")
G.buf:set_cursor(2, 1)
G.buf:toggle_mark()
G.buf:set_cursor(4, 1)
local sel = G.buf:selection_range()
touch(415, 16 + 249)
eq({ G.scroll_y, G.buf.cx, G.buf.cy }, { 25, 4, 1 }, "a press below the thumb pages down, the cursor stays")
eq(G.buf:selection_range(), sel, "and the selection stays")
touch(415, 16 + 249)
touch(300, 100)
eq({ G.scroll_y, G.buf.cx, G.buf.cy }, { 25, 4, 1 }, "the rest of that press pages no more and selects nothing")
release()
local ty = S.thumb(250, 100, 25, 25)
touch(415, 16 + ty + 1)
touch(415, 16 + 250)
eq({ G.scroll_y, G.buf.cx, G.buf.cy }, { 75, 4, 1 }, "dragging the thumb to the bottom shows the last lines")
touch(415, 16)
eq(G.scroll_y, 0, "and to the top shows the first")
release()

group("a key brings the view back to the cursor")
touch(415, 16 + 249)
release()
G.buf:clear_mark()
key(AcidKeys.DOWN)
eq({ G.buf.cy, G.scroll_y }, { 2, 2 }, "Down moves the cursor and scrolls back to it")

group("the command strip")
key(AcidKeys.ESCAPE)
touch(415, 16 + 100)
release()
eq({ G:cmd_active(), G.scroll_y }, { true, 27 }, "a press on the bar above the strip pages without closing it")
key(AcidKeys.ESCAPE)

group("taps elsewhere still edit")
touch(L.TEXT_X + 3 * L.CHAR_W + 1, 16 + 2 * L.LINE_H + 1)
release()
eq({ G.buf.cx, G.buf.cy }, { 3, 29 }, "a text tap still places the cursor under it")

group("no bar when everything fits")
load(3)
G.buf:set_cursor(0, 2)
frame()
ok(not track_drawn(), "3 lines need no bar")
touch(415, 16 + 1)
release()
eq({ G.scroll_y, G.buf.cx, G.buf.cy }, { 0, 6, 0 }, "a press where the bar would be is an ordinary text tap, clamped to the line's end")

group("resizing")
load(100)
resize_app(600, 400)
eq({ G:bar_geometry() }, { 593, 16, 370 }, "the bar follows a resize")
local fits, why = drawn_inside_window()
ok(fits, "and everything still fits" .. (why and (": " .. why) or ""))

group("resizing keeps a scrolled view")
resize_app(420, 280)
load(100)
G.buf:set_cursor(1, 3)
touch(415, 16 + 249)
release()
eq(G.scroll_y, 25, "paged away from the cursor")
resize_app(600, 400)
eq({ G.scroll_y, G.buf.cx, G.buf.cy }, { 25, 1, 3 }, "growing the window keeps the paged view and the cursor")
load(100)
G.scroll_y = 75
resize_app(600, 400)
eq(G.scroll_y, S.max_offset(100, G:visible_lines()), "growing clamps a view that now shows past the end")

group("commands that stay put keep the view")
resize_app(420, 280)
load(100)
G.buf:set_cursor(0, 3)
touch(415, 16 + 249)
release()
key(AcidKeys.ESCAPE)
key(string.byte("h"))
eq({ G.scroll_y, G.buf.cy }, { 25, 3 }, "ESC h toggles highlighting without moving the view")
key(AcidKeys.ESCAPE)
key(string.byte("t"))
eq({ G.scroll_y, G.buf.cy }, { 0, 0 }, "ESC t moves the cursor and scrolls to it")
