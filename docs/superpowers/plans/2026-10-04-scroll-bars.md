# Scroll Bars (Editor, Terminal, Load Cart) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give Editor, Terminal and Load Cart the same right-hand scroll bar File Manager has. Terminal also gains scroll-back.

**Architecture:**
- Each app uses the existing `AcidScrollbar` library (`apps/lib/acid_scrollbar.lua`) the way File Manager does:
  - a `bar_geometry()` method;
  - drawing after the content;
  - a press handled before the app's own touch handling;
  - held samples driving a thumb drag.
- Each app's manifest gains `lib/acid_scrollbar.lua` in `libs`.
- There are no kernel, API or library changes.

**Tech Stack:** Lua 5.4 apps. Headless Lua suites run by `v3/crates/acid-lua/tests/game_tests.rs`.

**Spec:** `docs/superpowers/specs/2026-10-04-scroll-bars-design.md`

## Global Constraints

- **Repo:** all paths are relative to `/home/norfolkh/acid-os-v3`. Cargo commands run from `v3/`.
- **What changes:** no changes to any `src/` under `v3/crates`, and no changes to `apps/lib/acid_scrollbar.lua`. Only these change:
  - `v3/apps`;
  - `v3/tools`;
  - `v3/crates/acid-lua/tests/game_tests.rs`;
  - `docs/manual-v3`.
- **Lua environment:** the headless Lua state has only the `string`, `table` and `math` libraries.
- **Assertion counts:** each suite's count is pinned in `game_tests.rs`. The counts below were measured by running this plan's exact code under Lua 5.4 against the current tree.
- **The bar:** it is `AcidScrollbar.WIDTH` (6) px wide at `x = window_w − 1 − WIDTH`, its track spans exactly the rows it scrolls, and it is drawn only when `AcidScrollbar.needed`. Text and row backgrounds stop short of the bar's column whether or not the bar is showing.
- **Goldens:** no golden frame shows these apps. If any golden test fails, stop and report it. Never write a golden.
- **Commits:** every commit message ends with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## How the code is given

Each task's app changes come as a unified diff. Save it to a file, and apply it with `git apply <file>` from the repo root. The diffs were generated against the tree at the plan's commit. New test files are given whole.

---

### Task 1: Terminal — scroll bar and scroll-back

**Files:**
- Modify: `v3/apps/terminal.lua`, `v3/apps/terminal.app.toml` (via the patch)
- Create: `v3/tools/test_scroll_terminal.lua`
- Modify: `v3/tools/test_terminal_large.lua` (one appended assertion), `v3/crates/acid-lua/tests/game_tests.rs`

**Interfaces produced:**
- Fields `TerminalApp.BAR_X` and `TerminalApp.LINE_COLS`, set in `layout()`.
- Instance fields `scroll` and `follow`.
- Methods:
  - `scroll_offset()`;
  - `bar_geometry() -> x, y, h`;
  - `draw_scrollbar()`;
  - `set_scroll(offset)`;
  - `on_touch(x, y, pressed)`.

- [ ] **Step 1: Write the failing tests.**
  - Create `v3/tools/test_scroll_terminal.lua`:

````lua
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
````

  - Append this line to the end of `v3/tools/test_terminal_large.lua`:

````lua
eq({ G:bar_geometry() }, { 520 - 7, 16, 15 * 18 }, "the scroll bar spans the 15 Large rows beside them")
````

  - In `game_tests.rs`:
    - Add `"v3/apps/lib/acid_scrollbar.lua",` right after `"v3/apps/lib/acid_eggs.lua",` in the file lists of `terminal`, `terminal_large` and `terminal_resize`. The terminal now references `AcidScrollbar`.
    - Change `terminal_large`'s count from `3` to `4`.
    - Add the new suite after `terminal_resize`:

````rust
#[test]
fn terminal_scroll() {
    run_suite_with("WIN_W, WIN_H = 260, 160", &with_libs(&["v3/tools/game_test_env.lua", "v3/apps/lib/acid_sprite.lua", "v3/apps/lib/acid_eggs.lua", "v3/apps/lib/acid_scrollbar.lua", "v3/apps/terminal.lua", "v3/tools/test_scroll_terminal.lua"]), 17);
}
````

- [ ] **Step 2: Run them and watch them fail.**
  - Run: `cargo test -p acid-lua --test game_tests terminal`
  - Expected:
    - `terminal_scroll` FAILS, because the field `bar_geometry` is nil;
    - `terminal_large` FAILS, for the same reason;
    - `terminal` and `terminal_resize` pass.

- [ ] **Step 3: Implement.** Apply this patch with `git apply`:

````diff
--- a/v3/apps/terminal.lua
+++ b/v3/apps/terminal.lua
@@ -41,12 +41,16 @@
 
 -- Everything derived from the window size lives here, so a resize can
 -- redo it: the window's extent and the columns that fit past the 2 px
--- margins. The row count follows from WINDOW_H in visible_lines.
+-- margins. The input line gets every column; scroll-back lines stop short
+-- of the scroll bar's column. The row count follows from WINDOW_H in
+-- visible_lines.
 function TerminalApp:layout()
   local ww, wh = acid_window_size()
   TerminalApp.WINDOW_W = ww
   TerminalApp.WINDOW_H = wh
   TerminalApp.COLS = (ww - 8) // CW
+  TerminalApp.BAR_X = ww - 1 - AcidScrollbar.WIDTH
+  TerminalApp.LINE_COLS = (TerminalApp.BAR_X - 1 - 2) // CW
 end
 
 function TerminalApp:on_resize(w, h)
@@ -57,6 +61,10 @@
   self:layout()
   self.cwd = TerminalApp.ROOT_DIR
   self.lines = { "Acid OS v3 terminal -- type help", "" }
+  -- Scroll-back: the first line shown, used only while `follow` is off.
+  -- Following (the default) always shows the newest lines.
+  self.scroll = 0
+  self.follow = true
   self.input = ""
   self.history = {}
   self.history_pos = 0 -- 0-based; +1 at each history access
@@ -70,30 +78,88 @@
   acid_clear_user_area()
   acid_draw_window_frame(self:window_title())
   self:draw_scrollback()
+  self:draw_scrollbar()
   self:draw_input_line()
   acid_draw_window_border()
 end
 
+-- The first scroll-back line on screen: the newest screenful while
+-- following, otherwise the scrolled-to line, clamped (a resize or `clear`
+-- can leave it past the end).
+function TerminalApp:scroll_offset()
+  local max = AcidScrollbar.max_offset(#self.lines, self:visible_lines())
+  if self.follow then return max end
+  return math.max(0, math.min(self.scroll, max))
+end
+
 function TerminalApp:draw_scrollback()
   local T = TerminalApp
   local y = T.TITLE_BAR_H
   local n = self:visible_lines()
-  local start = #self.lines > n and #self.lines - n or 0
-  local i = start
-  while i < #self.lines do
-    acid_fill_rect(0, y, T.WINDOW_W, T.LINE_H, T.BODY_BG)
-    acid_draw_text(self.lines[i + 1]:sub(1, T.COLS), 2, y + 1, T.TEXT_COLOR, T.BODY_BG)
+  local i = self:scroll_offset()
+  local last = math.min(#self.lines, i + n)
+  while i < last do
+    acid_fill_rect(0, y, T.BAR_X, T.LINE_H, T.BODY_BG)
+    acid_draw_text(self.lines[i + 1]:sub(1, T.LINE_COLS), 2, y + 1, T.TEXT_COLOR, T.BODY_BG)
     y = y + T.LINE_H
     i = i + 1
   end
   -- Pad any remaining rows (fewer lines than fit) so old content from a
   -- taller previous frame can never show through underneath.
   while y < T.WINDOW_H - T.LINE_H do
-    acid_fill_rect(0, y, T.WINDOW_W, T.LINE_H, T.BODY_BG)
+    acid_fill_rect(0, y, T.BAR_X, T.LINE_H, T.BODY_BG)
     y = y + T.LINE_H
   end
 end
 
+-- The scroll bar (lib/acid_scrollbar.lua): the column just inside the right
+-- border, beside the scroll-back rows; the input line below stays full
+-- width.
+function TerminalApp:bar_geometry()
+  local T = TerminalApp
+  return T.BAR_X, T.TITLE_BAR_H, self:visible_lines() * T.LINE_H
+end
+
+function TerminalApp:draw_scrollbar()
+  local x, y, h = self:bar_geometry()
+  AcidScrollbar.draw(x, y, h, #self.lines, self:visible_lines(), self:scroll_offset())
+end
+
+-- Scrolling to the last screenful turns following back on, so new output
+-- shows again without a key press.
+function TerminalApp:set_scroll(offset)
+  self.scroll = offset
+  self.follow = offset >= AcidScrollbar.max_offset(#self.lines, self:visible_lines())
+end
+
+-- The bar is the only thing in the window that takes a touch. The router
+-- resends a held touch every tick: a thumb drag follows every sample, a
+-- press on the track pages once per press.
+function TerminalApp:on_touch(x, y, pressed)
+  if not pressed then
+    self.touch_held = false
+    self.bar_grab = nil
+    return
+  end
+  local bx, by, h = self:bar_geometry()
+  local total, visible = #self.lines, self:visible_lines()
+  if self.bar_grab then
+    local offset = AcidScrollbar.drag(h, total, visible, self.bar_grab, y - by)
+    if offset ~= self:scroll_offset() then
+      self:set_scroll(offset)
+      self:redraw()
+    end
+    return
+  end
+  if self.touch_held then return end
+  self.touch_held = true
+  if not AcidScrollbar.needed(total, visible) or not AcidScrollbar.hit(bx, by, h, x, y) then return end
+  local offset, grab = AcidScrollbar.press(h, total, visible, self:scroll_offset(), y - by)
+  self.bar_grab = grab
+  self:set_scroll(offset)
+  self:redraw()
+end
+
 function TerminalApp:draw_input_line()
   local T = TerminalApp
   local y = T.WINDOW_H - T.LINE_H
@@ -106,6 +172,8 @@
 
 function TerminalApp:on_key(code, pressed)
   if not pressed then return end
+  -- Typing brings scroll-back down to the newest lines.
+  self.follow = true
   if code == AcidKeys.ENTER then
     self:submit()
   elseif code == AcidKeys.BACKSPACE then
--- a/v3/apps/terminal.app.toml
+++ b/v3/apps/terminal.app.toml
@@ -3,7 +3,7 @@
 h = 160
 desc = Command-line shell
 multi = true
-libs = lib/acid_sprite.lua, lib/acid_eggs.lua
+libs = lib/acid_sprite.lua, lib/acid_eggs.lua, lib/acid_scrollbar.lua
 font = scalable
 resizable = true
 min_w = 160
````

- [ ] **Step 4: Run them and watch them pass.**
  - Run: `cargo test -p acid-lua --test game_tests terminal`
  - Expected: `terminal` (28), `terminal_large` (4), `terminal_resize` (6) and `terminal_scroll` (17) all PASS.

- [ ] **Step 5: Commit.**

````bash
git add v3/apps/terminal.lua v3/apps/terminal.app.toml v3/tools/test_scroll_terminal.lua v3/tools/test_terminal_large.lua v3/crates/acid-lua/tests/game_tests.rs
git commit -m "Terminal: scroll bar and scroll-back

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
````

---

### Task 2: Load Cart — scroll bar on the list

**Files:**
- Modify: `v3/apps/cart.lua`, `v3/apps/cart.app.toml` (via the patch)
- Create: `v3/tools/test_scroll_cart.lua`
- Modify: `v3/crates/acid-lua/tests/game_tests.rs`

**Interfaces produced:**
- The local `BAR_X`.
- Methods:
  - `CartApp:bar_geometry() -> x, y, h`;
  - `press_scrollbar(x, y) -> bool`;
  - `drag_scrollbar(y)`.
- The instance field `bar_grab`.

- [ ] **Step 1: Write the failing test.**
  - Create `v3/tools/test_scroll_cart.lua`:

````lua
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
````

  - In `game_tests.rs`:
    - Add `"v3/apps/lib/acid_scrollbar.lua",` right after `"v3/apps/cart/cartfile.lua",` in `cart_install`'s file list. The cart app now references `AcidScrollbar` when it loads.
    - Leave `apps_follow_the_screen_size` alone, because it doesn't load `cart.lua`.
    - Add the new suite after `cart_install`:

````rust
#[test]
fn cart_scroll() {
    run_suite(&with_libs(&["v3/tools/game_test_env.lua", "v3/apps/cart/cartfile.lua", "v3/apps/lib/acid_scrollbar.lua", "v3/apps/cart.lua", "v3/tools/test_scroll_cart.lua"]), 10);
}
````

- [ ] **Step 2: Run it and watch it fail.**
  - Run: `cargo test -p acid-lua --test game_tests cart`
  - Expected:
    - `cart_scroll` FAILS, because the field `bar_geometry` is nil;
    - `cart_install` and `cartfile` pass.

- [ ] **Step 3: Implement.** Apply this patch:

````diff
--- a/v3/apps/cart.lua
+++ b/v3/apps/cart.lua
@@ -36,6 +36,9 @@
 local BTN_W = 84
 local BTN_GAP = 8
 local FOOTER_H = BTN_H + 8
+-- The scroll bar's column, just inside the right border; list rows stop
+-- short of it.
+local BAR_X = WINDOW_W - 1 - AcidScrollbar.WIDTH
 
 local BG_COLOR = 0x050607      -- THEME_BG
 local PANEL_COLOR = 0x0B1712   -- THEME_PANEL -- header/footer strips
@@ -409,14 +412,49 @@
   local i = self.scroll
   while i < count and i < self.scroll + self:visible_rows() do
     local row_bg = (i == self.selected) and SEL_BG or BG_COLOR
-    acid_fill_rect(0, y, WINDOW_W, ROW_H, row_bg)
+    acid_fill_rect(0, y, BAR_X, ROW_H, row_bg)
     acid_draw_text(self:row_text(i):sub(1, 46), 2, y + 2, self:row_color(i), row_bg)
     y = y + ROW_H
     i = i + 1
   end
+  local bx, by, h = self:bar_geometry()
+  AcidScrollbar.draw(bx, by, h, count, self:visible_rows(), self.scroll)
   self:draw_footer(buttons)
 end
 
+-- The scroll bar (lib/acid_scrollbar.lua): the column just inside the right
+-- border, beside the list rows. Paging and dragging move the view only;
+-- the selection stays where it is until a key moves it.
+function CartApp:bar_geometry()
+  return BAR_X, TITLE_BAR_H + HEADER_H, self:visible_rows() * ROW_H
+end
+
+-- A press on the bar pages a screenful or grabs the thumb. Returns whether
+-- the press was on the bar (there is none off the lists, or when every row
+-- fits).
+function CartApp:press_scrollbar(x, y)
+  if self.screen ~= "roots" and self.screen ~= "browse" then return false end
+  local bx, by, h = self:bar_geometry()
+  local total, visible = self:row_count(), self:visible_rows()
+  if not AcidScrollbar.needed(total, visible) or not AcidScrollbar.hit(bx, by, h, x, y) then
+    return false
+  end
+  local offset, grab = AcidScrollbar.press(h, total, visible, self.scroll, y - by)
+  self.bar_grab = grab
+  self.scroll = offset
+  self:redraw()
+  return true
+end
+
+function CartApp:drag_scrollbar(y)
+  local _, by, h = self:bar_geometry()
+  local offset = AcidScrollbar.drag(h, self:row_count(), self:visible_rows(), self.bar_grab, y - by)
+  if offset ~= self.scroll then
+    self.scroll = offset
+    self:redraw()
+  end
+end
+
 function CartApp:empty_text()
   if self.screen == "roots" then return "no cards found -- no cart folder exists" end
   return "no carts in this directory"
@@ -560,10 +598,18 @@
   -- over for as long as a finger stayed down.
   if not pressed then
     self.touch_held = false
+    self.bar_grab = nil
+    return
+  end
+  -- The one place repeated held touches are wanted: a thumb drag follows
+  -- the pointer for as long as the press lasts.
+  if self.bar_grab then
+    self:drag_scrollbar(y)
     return
   end
   if self.touch_held then return end
   self.touch_held = true
+  if self:press_scrollbar(x, y) then return end
 
   local buttons = self:footer_buttons()
   local btn = self:button_at(x, y, #buttons)
--- a/v3/apps/cart.app.toml
+++ b/v3/apps/cart.app.toml
@@ -2,4 +2,4 @@
 w = 300
 h = 210
 desc = Install .cart programs from a card
-libs = cart/cartfile.lua
+libs = cart/cartfile.lua, lib/acid_scrollbar.lua
````

- [ ] **Step 4: Run it and watch it pass.**
  - Run: `cargo test -p acid-lua --test game_tests cart`
  - Expected: `cartfile` (143), `cart_install` (76) and `cart_scroll` (10) all PASS.

- [ ] **Step 5: Commit.**

````bash
git add v3/apps/cart.lua v3/apps/cart.app.toml v3/tools/test_scroll_cart.lua v3/crates/acid-lua/tests/game_tests.rs
git commit -m "Load Cart: scroll bar on the card and cart lists

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
````

---

### Task 3: Editor — scroll bar that moves only the view

**Files:**
- Modify: `v3/apps/editor/layout.lua`, `v3/apps/editor.lua`, `v3/apps/editor/touch.lua`, `v3/apps/editor.app.toml` (via the patch)
- Create: `v3/tools/test_scroll_editor.lua`
- Modify: `v3/tools/test_editor_large.lua` (one appended assertion), `v3/crates/acid-lua/tests/game_tests.rs`

**Interfaces produced:**
- `EditorLayout.BAR_X`, set in `compute`.
- `EditorApp:bar_geometry() -> x, y, h` and `draw_scrollbar()`.
- `EditorTouch.touch_scrollbar(self, x, y) -> bool` and `EditorTouch.drag_scrollbar(self, y) -> bool`.
- The instance field `bar_grab`.
- `visible_cols()` now counts the columns up to one pixel before the bar.

**Note:** `editor/layout.lua` reads `AcidScrollbar.WIDTH` when it loads. The patch therefore puts `lib/acid_scrollbar.lua` **first** in the manifest's `libs`, and the suites load it before `editor/buffer.lua`.

- [ ] **Step 1: Write the failing tests.**
  - Create `v3/tools/test_scroll_editor.lua`:

````lua
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
frame()
ok(not track_drawn(), "3 lines need no bar")
touch(415, 16 + 1)
release()
eq({ G.scroll_y, G.buf.cy }, { 0, 0 }, "a press where the bar would be is an ordinary text tap")

group("resizing")
load(100)
resize_app(600, 400)
eq({ G:bar_geometry() }, { 593, 16, 370 }, "the bar follows a resize")
local fits, why = drawn_inside_window()
ok(fits, "and everything still fits" .. (why and (": " .. why) or ""))
````

  - Append this line to the end of `v3/tools/test_editor_large.lua`:

````lua
eq({ G:bar_geometry() }, { 640 - 7, 16, 23 * 18 }, "the scroll bar spans the 23 Large rows beside them")
````

  - In `game_tests.rs`:
    - Add `"v3/apps/lib/acid_scrollbar.lua",` right after `"v3/tools/game_test_env.lua",` in the file lists of `editor_app`, `editor_resize` and `editor_large`.
    - Leave `editor_modules` alone, because it doesn't load the layout.
    - Change `editor_large`'s count from `6` to `7`.
    - Add the new suite after `editor_large`:

````rust
#[test]
fn editor_scroll() {
    run_suite_with("WIN_W, WIN_H = 420, 280", &with_libs(&[
        "v3/tools/game_test_env.lua", "v3/apps/lib/acid_scrollbar.lua", "v3/apps/editor/buffer.lua", "v3/apps/editor/hl.lua",
        "v3/apps/editor/layout.lua", "v3/apps/editor/cmdbar.lua", "v3/apps/editor/touch.lua",
        "v3/apps/editor.lua", "v3/tools/test_scroll_editor.lua",
    ]), 16);
}
````

- [ ] **Step 2: Run them and watch them fail.**
  - Run: `cargo test -p acid-lua --test game_tests editor`
  - Expected:
    - `editor_scroll` FAILS, because the field `bar_geometry` is nil;
    - `editor_large` FAILS, for the same reason;
    - the others pass.

- [ ] **Step 3: Implement.** Apply this patch:

````diff
--- a/v3/apps/editor/layout.lua
+++ b/v3/apps/editor/layout.lua
@@ -28,6 +28,10 @@
   EditorLayout.WINDOW_W = w
   EditorLayout.WINDOW_H = h
   EditorLayout.STATUS_Y = h - EditorLayout.LINE_H
+  -- The scroll bar's column, just inside the right border (lib/
+  -- acid_scrollbar.lua, which the manifest loads first); text stops
+  -- short of it.
+  EditorLayout.BAR_X = w - 1 - AcidScrollbar.WIDTH
 end
 EditorLayout.compute(acid_window_size())
 
--- a/v3/apps/editor.lua
+++ b/v3/apps/editor.lua
@@ -135,8 +135,10 @@
   return (EditorLayout.STATUS_Y - EditorLayout.TEXT_Y) // EditorLayout.LINE_H
 end
 
+-- The columns between the gutter and the scroll bar's column, leaving
+-- a pixel's gap before the bar.
 function EditorApp:visible_cols()
-  return (EditorLayout.WINDOW_W - EditorLayout.TEXT_X) // EditorLayout.CHAR_W
+  return (EditorLayout.BAR_X - 1 - EditorLayout.TEXT_X) // EditorLayout.CHAR_W
 end
 
 function EditorApp:file_label()
@@ -153,6 +155,7 @@
   self:draw_gutter()
   self:draw_lines()
   self:draw_cursor()
+  self:draw_scrollbar()
   if self:cmd_active() then self:draw_cmd_strip() end
   if self:cmd_prompt_active() then
     self:draw_cmd_prompt()
@@ -217,7 +220,7 @@
   while i < self:visible_lines() do
     local idx = self.scroll_y + i
     local y = L.TEXT_Y + i * L.LINE_H
-    acid_fill_rect(L.TEXT_X, y, L.WINDOW_W - L.TEXT_X, L.LINE_H, EditorApp.BODY_BG)
+    acid_fill_rect(L.TEXT_X, y, L.BAR_X - L.TEXT_X, L.LINE_H, EditorApp.BODY_BG)
     local sel = self:selection_span(idx)
     if sel ~= nil then
       local sx = sel[1] - self.scroll_x
@@ -314,6 +317,21 @@
   acid_fill_rect(x, y + L.LINE_H - 2, L.CHAR_W, 2, EditorApp.CURSOR_COLOR)
 end
 
+-- The scroll bar (lib/acid_scrollbar.lua): the column just inside the
+-- right border, beside the text rows. It moves the view (scroll_y) only:
+-- the cursor and any selection stay put, even off-screen, until a key
+-- brings the view back to the cursor (on_key's ensure_scroll). The open
+-- command strip draws over the bottom of it, as it does over the text.
+function EditorApp:bar_geometry()
+  local L = EditorLayout
+  return L.BAR_X, L.TEXT_Y, self:visible_lines() * L.LINE_H
+end
+
+function EditorApp:draw_scrollbar()
+  local x, y, h = self:bar_geometry()
+  AcidScrollbar.draw(x, y, h, self.buf:line_count(), self:visible_lines(), self.scroll_y)
+end
+
 function EditorApp:on_touch(x, y, pressed)
   if pressed then self.message = nil end
   if self:editor_touch(x, y, pressed) then self:redraw() end
--- a/v3/apps/editor/touch.lua
+++ b/v3/apps/editor/touch.lua
@@ -22,6 +22,7 @@
   if not pressed then
     self.touch_down = false
     self.tap_consumed = false
+    self.bar_grab = nil
     -- Not load-bearing today (every fresh press re-initialises this
     -- anyway, via `fresh or` in touch_gutter), but leaving a gesture
     -- flag set across gestures is exactly the shape that caused the
@@ -35,12 +36,17 @@
   local fresh = not self.touch_down
   self.touch_down = true
 
+  -- A thumb drag follows every held sample, wherever it goes, and touches
+  -- nothing but the view.
+  if self.bar_grab then return self:drag_scrollbar(y) end
+
   -- One-shot targets first, and only on the press that started the
   -- hold: holding a finger on the strip must not re-run its command
   -- every frame.
   if fresh then
     if self:touch_command_strip(x, y) then return true end
     if self:touch_status(x, y) then return true end
+    if self:touch_scrollbar(x, y) then return true end
   end
   if self.tap_consumed then return false end
   if y < L.TEXT_Y or y >= L.STATUS_Y then return false end
@@ -75,6 +81,31 @@
   return self:touch_text(x, y, fresh)
 end
 
+-- A press on the scroll bar pages the view a screenful or grabs the
+-- thumb. The rest of a paging press's hold is consumed, so it never turns
+-- into a text selection.
+function EditorTouch.touch_scrollbar(self, x, y)
+  local bx, by, h = self:bar_geometry()
+  local total, visible = self.buf:line_count(), self:visible_lines()
+  if not AcidScrollbar.needed(total, visible) or not AcidScrollbar.hit(bx, by, h, x, y) then
+    return false
+  end
+  local offset, grab = AcidScrollbar.press(h, total, visible, self.scroll_y, y - by)
+  self.scroll_y = offset
+  self.bar_grab = grab
+  if grab == nil then self.tap_consumed = true end
+  return true
+end
+
+-- Returns whether the view moved, so on_touch redraws only then.
+function EditorTouch.drag_scrollbar(self, y)
+  local _, by, h = self:bar_geometry()
+  local offset = AcidScrollbar.drag(h, self.buf:line_count(), self:visible_lines(), self.bar_grab, y - by)
+  if offset == self.scroll_y then return false end
+  self.scroll_y = offset
+  return true
+end
+
 function EditorTouch.touch_command_strip(self, x, y)
   local L = EditorLayout
   if not self:cmd_active() then return false end
--- a/v3/apps/editor.app.toml
+++ b/v3/apps/editor.app.toml
@@ -3,7 +3,7 @@
 h = 280
 desc = Text editor for app source
 multi = true
-libs = editor/buffer.lua, editor/hl.lua, editor/layout.lua, editor/cmdbar.lua, editor/touch.lua
+libs = lib/acid_scrollbar.lua, editor/buffer.lua, editor/hl.lua, editor/layout.lua, editor/cmdbar.lua, editor/touch.lua
 font = scalable
 resizable = true
 min_w = 200
````

- [ ] **Step 4: Run them and watch them pass.**
  - Run: `cargo test -p acid-lua --test game_tests editor`
  - Expected: `editor_modules` (97), `editor_app` (26), `editor_resize` (8), `editor_large` (7) and `editor_scroll` (16) all PASS.

- [ ] **Step 5: Commit.**

````bash
git add v3/apps/editor.lua v3/apps/editor/layout.lua v3/apps/editor/touch.lua v3/apps/editor.app.toml v3/tools/test_scroll_editor.lua v3/tools/test_editor_large.lua v3/crates/acid-lua/tests/game_tests.rs
git commit -m "Editor: scroll bar that moves the view, not the cursor

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
````

---

### Task 4: Docs and the full check

**Files:**
- Modify: `docs/manual-v3/09-api-reference.md`, `docs/manual-v3/01-getting-started.md`

- [ ] **Step 1: Chapter 9.** In the `### AcidScrollbar` entry, replace the line

````markdown
- The bar is `AcidScrollbar.WIDTH` (6) pixels wide. File Manager uses it.
````

with

````markdown
- The bar is `AcidScrollbar.WIDTH` (6) pixels wide. File Manager, Editor,
  Terminal and Load Cart use it.
````

- [ ] **Step 2: Chapter 1.** In `01-getting-started.md`, after the paragraph that starts "You can also work from inside Acid OS itself" and before the **Sprite Paint** paragraph, add:

````markdown
File Manager, Editor, Terminal and Load Cart show a scroll bar at their
right edge when there's more than fits. Drag its thumb, or click above or
below the thumb to move a screenful. In Terminal it scrolls back through
older output, and typing brings you back to the bottom.
````

- [ ] **Step 3: Run everything.**
  - Run: `cargo test --workspace`
  - Expected: all PASS, including `acid-os`'s `games`, `golden` and `manual` tests. Those boot the real apps with their manifests' `libs`, which checks the new library order for real. If a golden fails, stop and report it.

- [ ] **Step 4: Commit.**

````bash
git add docs/manual-v3/01-getting-started.md docs/manual-v3/09-api-reference.md
git commit -m "Manual: scroll bars in Editor, Terminal and Load Cart

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
````

