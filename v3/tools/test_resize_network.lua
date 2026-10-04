-- Network re-lays out when the window is resized, with no overlap.
local function check(w, h)
  resize_app(w, h)
  local fits, what = drawn_inside_window()
  if fits then fits, what = drawn_text_clear() end
  ok(fits, "fits at " .. w .. "x" .. h .. (what and (": " .. what) or ""))
end
check(320, 160)
check(140, 80)
