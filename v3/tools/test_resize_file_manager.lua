-- File Manager re-counts its rows, moves its scroll bar, keeps the selection
-- visible and clamps a preview's scroll when the window is resized.
local G = GAME
local many = {}
for i = 1, 30 do many[i] = string.format("a_rather_long_file_name_%02d.txt", i) end
FS["v3/fsroot/Many"] = many
G.dir = "v3/fsroot/Many"
G:scan_dir()
G.selected = 25
G:ensure_listing_scroll()

local function selected_visible(what)
  ok(G.selected >= G.scroll and G.selected < G.scroll + G:visible_listing_rows(), "the selected row is still visible " .. what)
end

local function fits_clear(what)
  local fits, why = drawn_inside_window()
  local clear, w2 = drawn_text_clear()
  ok(fits and clear, "fits " .. what .. (why and (": " .. why) or "") .. (w2 and (": " .. w2) or ""))
end

resize_app(400, 300)
eq(G:visible_listing_rows(), (300 - 16) // (FONT_H + 4) - 1, "visible rows at 400x300")
eq({ G:bar_geometry() }, { 400 - 7, 16 + FONT_H + 4, 300 - 1 - (16 + FONT_H + 4) }, "the scroll bar follows the new size")
selected_visible("at 400x300")
fits_clear("at 400x300")

resize_app(120, 80)
selected_visible("at the minimum")
fits_clear("at the minimum")

-- A preview's scroll clamps to the new, larger window's maximum.
local file = {}
for i = 1, 50 do file[i] = "line " .. i end
FS["v3/fsroot/Many/big.txt"] = table.concat(file, "\n")
resize_app(220, 160)
G:open_preview("big.txt")
local lines = {}
for l in (FS["v3/fsroot/Many/big.txt"] .. "\n"):gmatch("(.-)\n") do lines[#lines + 1] = l end
G.preview_scroll = G:max_preview_scroll(lines)
resize_app(400, 300)
eq(G.preview_scroll, G:max_preview_scroll(lines), "preview scroll clamps to the new maximum")
fits_clear("with the preview at 400x300")
