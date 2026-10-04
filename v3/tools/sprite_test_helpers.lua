-- Shared by the Sprite Paint suites: pointer helpers in window pixels.
-- Loaded after the app, so GAME is the app under test.
function sp_tap(x, y) GAME:on_touch(x, y, true); GAME:on_touch(x, y, false) end
function sp_cell_xy(cx, cy)
  local c = GAME.L.canvas
  return c.x + cx * c.cell + 1, c.y + cy * c.cell + 1
end
function sp_tap_cell(cx, cy) sp_tap(sp_cell_xy(cx, cy)) end
-- Held samples over the cells, then a release at the last one.
function sp_drag(cells)
  for _, c in ipairs(cells) do
    local x, y = sp_cell_xy(c[1], c[2])
    GAME:on_touch(x, y, true)
  end
  local last = cells[#cells]
  local x, y = sp_cell_xy(last[1], last[2])
  GAME:on_touch(x, y, false)
end
function sp_button(id)
  for _, b in ipairs(GAME.L.buttons) do if b.id == id then return b.x + 1, b.y + 1 end end
end
function sp_swatch(key)
  for _, s in ipairs(GAME.L.swatches) do if s.key == key then return s.x + 1, s.y + 1 end end
end
function sp_bar(id)
  for _, b in ipairs(GAME.L.bar) do if b.id == id then return b.x + 1, b.y + 1 end end
end
function sp_keys(text)
  for i = 1, #text do GAME:on_key(text:byte(i), true) end
end
function sp_fits(what)
  local a, why = drawn_inside_window()
  local b, why2 = drawn_text_clear()
  ok(a and b, what .. (why and (": " .. why) or "") .. (why2 and (": " .. why2) or ""))
end
function sp_shown(text)
  for _, t in ipairs(TEXT_AT) do if t[1] == text then return true end end
  return false
end
