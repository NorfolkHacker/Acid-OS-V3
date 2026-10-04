-- Every app this runs on draws only inside its window, at whatever font
-- and window size the prelude set.
TEXT_AT, RECTS = {}, {}
GAME:redraw()
local fits, what = drawn_inside_window()
if fits then fits, what = drawn_text_clear() end
ok(fits, "everything drawn fits the window" .. (what and (": " .. what) or ""))
