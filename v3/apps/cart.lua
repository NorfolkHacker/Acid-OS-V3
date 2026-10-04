-- Load Cart -- install a program from a card.
--
-- A .cart is plain Lua source living OUTSIDE this OS: on a USB stick, an
-- SD card, a directory on the host. This app browses the handful of places
-- those turn up, shows what a cart says about itself, and -- once you
-- confirm -- copies it into v3/apps as <slug>.lua with a generated
-- <slug>.app.toml beside it, which is exactly the shape desktop.lua's boot
-- scan already registers. That's the whole point: an app can now be
-- written, edited and versioned outside this repo and carried in, instead
-- of having to be born in v3/apps.
--
-- Where the carts come from: acid_cart_roots(). Only those trees are
-- reachable -- this is a cart loader, not a general host-filesystem
-- browser. The PLATFORM decides which folders exist and are offered (spec
-- section 14.4: v3/carts, $HOME/carts, /media, /mnt, /run/media, only
-- those that are really there), so this app carries no root list of its
-- own.
--
-- What this app checks, and what it doesn't: every decision about the
-- DESTINATION -- the filename, the window size, which modules load,
-- whether that slot may be written at all -- goes through cart/cartfile.lua
-- (read its header comment) and nothing here builds a path by hand. What
-- nothing here can check is the cart's own CODE: once installed, it runs
-- in its own Lua state with the cart-level limits. A cart is trusted code
-- you chose to carry in, exactly like a program copied onto any other
-- machine.
CartApp = AcidApp:extend("CartApp")

-- Must match cart.app.toml.
local WINDOW_W = 300
local WINDOW_H = 210
local TITLE_BAR_H = 16      -- the kernel's title bar height
local HEADER_H = 12
local ROW_H = 12
local BTN_H = 16
local BTN_W = 84
local BTN_GAP = 8
local FOOTER_H = BTN_H + 8
-- The scroll bar's column, just inside the right border; list rows stop
-- short of it.
local BAR_X = WINDOW_W - 1 - AcidScrollbar.WIDTH

local BG_COLOR = 0x050607      -- THEME_BG
local PANEL_COLOR = 0x0B1712   -- THEME_PANEL -- header/footer strips
local TEXT_COLOR = 0xD4E6DB    -- THEME_TEXT
local MUTED_COLOR = 0x9DAAA3   -- THEME_MUTED
local HARD_COLOR = 0x00FF66    -- THEME_HARD -- accent/selection only
local ALERT_COLOR = 0xB026FF   -- THEME_VIOLET -- refusals and warnings
local SEL_BG = 0x123322        -- THEME_PANEL's documented button-hover shade

-- Where apps' shared modules live. The apps dir itself agrees with
-- Cartfile.APPS_DIR by construction -- the module builds every
-- destination path itself.
local LIB_DIR = "v3/apps/lib"

-- Trims leading and trailing whitespace.
local function strip(s)
  return s:match("^%s*(.-)%s*$")
end

-- Splits on "\n"; drops trailing empty strings.
local function split_lines(text)
  local lines = {}
  for line in (text .. "\n"):gmatch("(.-)\n") do lines[#lines + 1] = line end
  while #lines > 0 and lines[#lines] == "" do lines[#lines] = nil end
  return lines
end

-- The slot's two possible scripts, <stem>.lua and <stem>.wasm.
local function slot_scripts(spec)
  local stem = spec.toml_path:sub(1, #spec.toml_path - #".app.toml")
  return stem .. ".lua", stem .. ".wasm"
end

-- The slot's script that this install is NOT writing: the old cart's
-- file when a .wasm cart replaces a .lua one, or the reverse.
local function sibling_script(spec)
  local lua, wasm = slot_scripts(spec)
  if spec.script_path == lua then return wasm end
  return lua
end

function CartApp:on_create()
  self.touch_held = false
  self.message = nil
  self.roots = acid_cart_roots() or {}
  self.entries = {}
  self.selected = 0
  self.scroll = 0
  self.spec = nil
  self.cart_text = nil
  self.status = nil
  self.name_clash = nil
  self.screen = "roots"
end

function CartApp:window_title()
  return "Load Cart"
end

-- ------------------------------------------------------------ roots

-- Only the roots that are really there -- an empty list is its own
-- legible screen ("no cards found"), better than offering paths that all
-- fail to open when tapped. The platform already filtered them.
function CartApp:existing_roots()
  return acid_cart_roots() or {}
end

function CartApp:is_dir(path)
  return acid_cart_stat(path) == "dir"
end

function CartApp:size_of(path)
  -- nil, err on failure: reported as 0, which reads as "empty file".
  local kind, size = acid_cart_stat(path)
  if kind == nil then return 0 end
  return size
end

function CartApp:read_text(path)
  return (acid_cart_read(path))
end

-- Modules a cart may ask for, as the manifest spells them
-- ("lib/acid_game.lua") -- the real contents of apps/lib, read fresh so
-- this never drifts from what's actually installable. Cartfile.filter_libs
-- keeps only entries in this list, which is what stops a `-- libs:` line
-- naming a path of its own.
function CartApp:available_libs()
  local libs = {}
  local names = acid_fs_list(LIB_DIR)
  if names == nil then return {} end
  for _, ent in ipairs(names) do
    if ent:sub(-4) == ".lua" then libs[#libs + 1] = "lib/" .. ent end
  end
  return libs
end

-- ----------------------------------------------------------- browsing

function CartApp:open_root(root)
  self.root = root
  self.dir = root
  self.message = nil
  self:scan_dir()
  self.screen = "browse"
end

-- Directories and .cart files only. Everything else on the card is
-- invisible here -- this app has no business showing the contents of a
-- stranger's USB stick, and a cart is the only thing it can act on.
-- Symlinks never get this far: acid_cart_list leaves them out, because
-- the browser's containment (Cartfile.parent_dir/child_dir) is path
-- arithmetic, and a symlink inside a cart root pointing at /etc would
-- silently hand this app a tree outside every root it's allowed in.
function CartApp:scan_dir()
  self.entries = {}
  self.selected = 0
  self.scroll = 0
  local up = Cartfile.parent_dir(self.dir, self.root)
  if up then self.entries[#self.entries + 1] = { name = "..", dir = true, path = up } end
  local names = acid_cart_list(self.dir)
  if names == nil then
    self.message = "cannot read this directory"
    return
  end
  for _, name in ipairs(names) do
    if name ~= "." and name ~= ".." then
      local path = Cartfile.child_dir(self.dir, name)
      if path ~= nil then
        if self:is_dir(path) then
          self.entries[#self.entries + 1] = { name = name, dir = true, path = path }
        elseif Cartfile.cart(name) then
          self.entries[#self.entries + 1] = { name = name, dir = false, path = path, size = self:size_of(path) }
        end
      end
    end
  end
end

function CartApp:visible_rows()
  return (WINDOW_H - TITLE_BAR_H - HEADER_H - FOOTER_H) // ROW_H
end

function CartApp:ensure_scroll()
  if self.selected < self.scroll then
    self.scroll = self.selected
  elseif self.selected >= self.scroll + self:visible_rows() then
    self.scroll = self.selected - self:visible_rows() + 1
  end
end

function CartApp:row_count()
  if self.screen == "roots" then return #self.roots end
  return #self.entries
end

-- ------------------------------------------------------------ opening

function CartApp:open_selected()
  if self.screen == "roots" then
    local root = self.roots[self.selected + 1]
    if root == nil then return end
    self:open_root(root)
    self:redraw()
    return
  end
  local entry = self.entries[self.selected + 1]
  if entry == nil then return end
  if entry.dir then
    self.dir = entry.path
    self.message = nil
    self:scan_dir()
    self:redraw()
  else
    self:inspect_cart(entry)
  end
end

-- Reads the cart and works out what installing it would mean, without
-- writing anything. Everything the confirm screen shows comes from
-- Cartfile.from_cart -- including the destination paths, so what's on
-- screen is literally what gets written.
function CartApp:inspect_cart(entry)
  local bytes = entry.size
  if not Cartfile.size_ok(bytes) then
    if bytes > 0 then
      self.message = "too big: " .. bytes .. "B (max " .. Cartfile.MAX_BYTES .. ")"
    else
      self.message = "empty file"
    end
    self:redraw()
    return
  end
  local text = self:read_text(entry.path)
  if text == nil then
    self.message = "cannot read " .. entry.name
    self:redraw()
    return
  end
  local spec = Cartfile.from_cart(entry.name, text, self:available_libs())
  if spec == nil then
    self.message = "no usable app name in " .. entry.name
    self:redraw()
    return
  end
  self.spec = spec
  self.spec.source = entry.path
  self.spec.bytes = bytes
  self.cart_text = text
  self.status = self:destination_status(spec)
  -- On a replace, both of the slot's scripts are the cart itself: a .wasm
  -- replacing a .lua cart (or the reverse) does not clash with its own old
  -- entry. A protected slot's other script is someone else's.
  local own = { spec.script_path }
  if self.status == "replace" then own[2] = sibling_script(spec) end
  self.name_clash = Cartfile.name_clash(spec.name, own, self:registry_entries())
  self.message = nil
  self.screen = "confirm"
  self:redraw()
end

-- A slot is occupied if EITHER <stem>.lua OR <stem>.wasm is there: a
-- desktop.wasm cart must not find the slot "fresh" just because only
-- desktop.lua exists, or it would install a cart-trust manifest for the
-- desktop and let a later desktop.cart replace desktop.lua (spec §15.4).
function CartApp:destination_status(spec)
  local lua, wasm = slot_scripts(spec)
  local exists = self:file_exists(lua) or self:file_exists(wasm)
  return Cartfile.destination_status(exists, self:read_manifest(spec.toml_path))
end

function CartApp:file_exists(path)
  return acid_fs_size(path) ~= nil
end

-- nil means "no manifest there", which destination_status reads
-- differently from an empty one -- see its own comment.
function CartApp:read_manifest(path)
  local text = acid_fs_read(path)
  if text == nil then return nil end
  local fields = {}
  for _, line in ipairs(split_lines(text)) do
    line = strip(line)
    if #line > 0 and line:sub(1, 1) ~= "#" then
      local eq = line:find("=", 1, true)
      if eq then
        eq = eq - 1  -- 0-based index
        fields[strip(line:sub(1, eq))] = strip(line:sub(eq + 2, #line))
      end
    end
  end
  return fields
end

-- ---------------------------------------------------------- installing

function CartApp:install()
  if self.spec == nil or self.cart_text == nil then return end
  if self.status == "protected" then
    self.message = "that slot belongs to a built-in app"
    self:redraw()
    return
  end
  -- The manifest (which says source = cart) goes down before the source:
  -- a .lua in the apps dir with no manifest beside it runs at built-in
  -- trust (spec §14.2), so the cart's code must never land first.
  local manifest = Cartfile.manifest_text(self.spec)
  if not self:write_file(self.spec.toml_path, manifest) then
    self.message = "could not write " .. self.spec.toml_path
    self:redraw()
    return
  end
  if not self:write_file(self.spec.script_path, self.cart_text) then
    -- The manifest landed but the source didn't, so there is nothing for
    -- it to launch -- say so plainly rather than reporting a success that
    -- leaves a half-installed app behind.
    self.message = "wrote manifest but not source -- not installed"
    self:redraw()
    return
  end
  -- A replace across runtimes leaves the old cart's other script behind;
  -- remove it now the new one has landed. It is the old cart's own file:
  -- a replace is only offered when the manifest says source = cart.
  if self.status == "replace" then
    local old = sibling_script(self.spec)
    if self:file_exists(old) then acid_fs_delete(old) end
  end
  self.message = nil
  self.screen = "done"
  self:redraw()
end

function CartApp:write_file(path, text)
  return acid_fs_write(path, text) == true
end

-- Registers the freshly installed cart with the launcher so it can run
-- right now, then spawns it. The Menu dropdown itself only rebuilds at
-- boot (desktop.lua builds its menu once in on_create), so the cart appears
-- in the Menu on the next boot, and this button is how you play it in the
-- meantime.
function CartApp:launch()
  if self.spec == nil then return end
  if not self:registered(self.spec.script_path) then self:register_spec() end
  acid_spawn_app(self.spec.script_path, self.spec.w, self.spec.h, "")
end

-- The launcher registry as Cartfile.name_clash wants it -- every app
-- registered at boot (Menu-visible or not), plus anything installed
-- since.
function CartApp:registry_entries()
  local entries = {}
  local i = 0
  local count = acid_launcher_count()
  while i < count do
    entries[#entries + 1] = { acid_launcher_path(i), acid_launcher_name(i) }
    i = i + 1
  end
  return entries
end

function CartApp:registered(path)
  local i = 0
  local count = acid_launcher_count()
  while i < count do
    if acid_launcher_path(i) == path then return true end
    i = i + 1
  end
  return false
end

function CartApp:register_spec()
  acid_launcher_register(self.spec.script_path, self.spec.name, self.spec.w, self.spec.h,
                         false, table.concat(self.spec.libs, ", "))
end

-- ------------------------------------------------------------ drawing

function CartApp:redraw()
  acid_clear_user_area()
  acid_draw_window_frame(self:window_title())
  if self.screen == "roots" then
    self:draw_list("CARDS", #self.roots, { "OPEN" })
  elseif self.screen == "browse" then
    self:draw_list(self.dir, #self.entries, { "OPEN", "CARDS" })
  elseif self.screen == "confirm" then
    self:draw_confirm()
  elseif self.screen == "done" then
    self:draw_done()
  end
  acid_draw_window_border()
end

function CartApp:draw_header(label)
  acid_fill_rect(0, TITLE_BAR_H, WINDOW_W, HEADER_H, PANEL_COLOR)
  acid_draw_text(label:sub(1, 46), 2, TITLE_BAR_H + 2, MUTED_COLOR, PANEL_COLOR)
end

function CartApp:draw_list(label, count, buttons)
  if count > self:visible_rows() then
    self:draw_header(label .. " (" .. (self.selected + 1) .. "/" .. count .. ")")
  else
    self:draw_header(label)
  end
  local y = TITLE_BAR_H + HEADER_H
  if count == 0 then
    acid_fill_rect(0, y, WINDOW_W, ROW_H, BG_COLOR)
    acid_draw_text(self:empty_text(), 2, y + 2, MUTED_COLOR, BG_COLOR)
  end
  local i = self.scroll
  while i < count and i < self.scroll + self:visible_rows() do
    local row_bg = (i == self.selected) and SEL_BG or BG_COLOR
    acid_fill_rect(0, y, BAR_X, ROW_H, row_bg)
    acid_draw_text(self:row_text(i):sub(1, 46), 2, y + 2, self:row_color(i), row_bg)
    y = y + ROW_H
    i = i + 1
  end
  local bx, by, h = self:bar_geometry()
  AcidScrollbar.draw(bx, by, h, count, self:visible_rows(), self.scroll)
  self:draw_footer(buttons)
end

-- The scroll bar (lib/acid_scrollbar.lua): the column just inside the right
-- border, beside the list rows. Paging and dragging move the view only;
-- the selection stays where it is until a key moves it.
function CartApp:bar_geometry()
  return BAR_X, TITLE_BAR_H + HEADER_H, self:visible_rows() * ROW_H
end

-- A press on the bar pages a screenful or grabs the thumb. Returns whether
-- the press was on the bar (there is none off the lists, or when every row
-- fits).
function CartApp:press_scrollbar(x, y)
  if self.screen ~= "roots" and self.screen ~= "browse" then return false end
  local bx, by, h = self:bar_geometry()
  local total, visible = self:row_count(), self:visible_rows()
  if not AcidScrollbar.needed(total, visible) or not AcidScrollbar.hit(bx, by, h, x, y) then
    return false
  end
  local offset, grab = AcidScrollbar.press(h, total, visible, self.scroll, y - by)
  self.bar_grab = grab
  self.scroll = offset
  self:redraw()
  return true
end

function CartApp:drag_scrollbar(y)
  local _, by, h = self:bar_geometry()
  local offset = AcidScrollbar.drag(h, self:row_count(), self:visible_rows(), self.bar_grab, y - by)
  if offset ~= self.scroll then
    self.scroll = offset
    self:redraw()
  end
end

function CartApp:empty_text()
  if self.screen == "roots" then return "no cards found -- no cart folder exists" end
  return "no carts in this directory"
end

function CartApp:row_text(i)
  if self.screen == "roots" then return self.roots[i + 1] end
  local e = self.entries[i + 1]
  if e.dir then return "[" .. e.name .. "]" end
  return " " .. e.name .. " (" .. e.size .. "B)"
end

function CartApp:row_color(i)
  if self.screen == "roots" then return HARD_COLOR end
  local e = self.entries[i + 1]
  if e.dir then return HARD_COLOR end
  return TEXT_COLOR
end

function CartApp:draw_confirm()
  local spec = self.spec
  self:draw_header("INSTALL CART")
  local y = TITLE_BAR_H + HEADER_H
  y = self:draw_field(y, "name", spec.name)
  y = self:draw_field(y, "size", spec.w .. "x" .. spec.h .. "  " .. spec.bytes .. "B")
  y = self:draw_field(y, "desc", #spec.desc == 0 and "(none)" or spec.desc)
  y = self:draw_field(y, "libs", #spec.libs == 0 and "(none)" or table.concat(spec.libs, ", "))
  y = self:draw_field(y, "from", spec.source)
  y = self:draw_field(y, "into", spec.script_path)
  acid_fill_rect(0, y, WINDOW_W, ROW_H, BG_COLOR)
  acid_draw_text(self:status_text():sub(1, 46), 2, y + 2, self:status_color(), BG_COLOR)
  if self.name_clash ~= nil then
    y = y + ROW_H
    acid_fill_rect(0, y, WINDOW_W, ROW_H, BG_COLOR)
    acid_draw_text(("name already answers to " .. self.name_clash):sub(1, 46), 2, y + 2, ALERT_COLOR, BG_COLOR)
  end
  if self.status == "protected" then
    self:draw_footer({ "BACK" })
  else
    self:draw_footer({ self:confirm_label(), "BACK" })
  end
end

function CartApp:draw_field(y, label, value)
  acid_fill_rect(0, y, WINDOW_W, ROW_H, BG_COLOR)
  acid_draw_text(label, 2, y + 2, MUTED_COLOR, BG_COLOR)
  acid_draw_text(tostring(value):sub(1, 40), 40, y + 2, TEXT_COLOR, BG_COLOR)
  return y + ROW_H
end

function CartApp:confirm_label()
  if self.status == "replace" then return "REPLACE" end
  return "INSTALL"
end

function CartApp:status_text()
  if self.status == "replace" then
    return "replaces the cart already installed there"
  elseif self.status == "protected" then
    return "REFUSED: " .. self.spec.slug .. " is a built-in app"
  end
  return "new install"
end

function CartApp:status_color()
  if self.status == "fresh" then return HARD_COLOR end
  return ALERT_COLOR
end

function CartApp:draw_done()
  self:draw_header("INSTALLED")
  local y = TITLE_BAR_H + HEADER_H
  y = self:draw_field(y, "app", self.spec.name)
  y = self:draw_field(y, "src", self.spec.script_path)
  y = self:draw_field(y, "man", self.spec.toml_path)
  acid_fill_rect(0, y, WINDOW_W, ROW_H * 2, BG_COLOR)
  acid_draw_text("joins the Menu at next boot --", 2, y + 2, MUTED_COLOR, BG_COLOR)
  acid_draw_text("RUN starts it now.", 2, y + ROW_H + 2, MUTED_COLOR, BG_COLOR)
  self:draw_footer({ "RUN", "CARDS" })
end

-- The footer carries this screen's buttons on the left and whatever the
-- last action had to say on the right -- one line, so an error never
-- pushes the buttons off screen.
function CartApp:draw_footer(buttons)
  local y = self:footer_y()
  acid_fill_rect(0, y, WINDOW_W, FOOTER_H, PANEL_COLOR)
  local i = 0
  while i < #buttons do
    local x = self:button_x(i)
    acid_fill_rect(x, y + 4, BTN_W, BTN_H, SEL_BG)
    acid_draw_text(buttons[i + 1], x + 4, y + 4 + BTN_H // 2 - 4, HARD_COLOR, SEL_BG)
    i = i + 1
  end
  if self.message == nil then return end
  acid_draw_text(self.message:sub(1, 24), self:button_x(#buttons) + 4, y + 8, ALERT_COLOR, PANEL_COLOR)
end

function CartApp:footer_y()
  return WINDOW_H - FOOTER_H
end

function CartApp:button_x(i)
  return 2 + i * (BTN_W + BTN_GAP)
end

-- Which footer button a tap landed on, or nil. Mirrors draw_footer's own
-- geometry -- the two are kept together deliberately.
function CartApp:button_at(x, y, count)
  if y < self:footer_y() + 4 or y > self:footer_y() + 4 + BTN_H then return nil end
  local i = 0
  while i < count do
    local bx = self:button_x(i)
    if x >= bx and x < bx + BTN_W then return i end
    i = i + 1
  end
  return nil
end

function CartApp:footer_buttons()
  if self.screen == "roots" then
    return { "OPEN" }
  elseif self.screen == "browse" then
    return { "OPEN", "CARDS" }
  elseif self.screen == "confirm" then
    if self.status == "protected" then return { "BACK" } end
    return { self:confirm_label(), "BACK" }
  elseif self.screen == "done" then
    return { "RUN", "CARDS" }
  end
  return {}
end

-- ------------------------------------------------------------- input

function CartApp:on_touch(x, y, pressed)
  -- Same one-press-per-hold discipline as every other tap-to-act app in
  -- this OS (file_manager.lua, sysmon.lua):
  -- the router resends TOUCH every ~16ms while held, and without this
  -- guard one tap-and-hold on INSTALL would run the install over and
  -- over for as long as a finger stayed down.
  if not pressed then
    self.touch_held = false
    self.bar_grab = nil
    return
  end
  -- The one place repeated held touches are wanted: a thumb drag follows
  -- the pointer for as long as the press lasts.
  if self.bar_grab then
    self:drag_scrollbar(y)
    return
  end
  if self.touch_held then return end
  self.touch_held = true
  if self:press_scrollbar(x, y) then return end

  local buttons = self:footer_buttons()
  local btn = self:button_at(x, y, #buttons)
  if btn then
    self:press_button(buttons[btn + 1])
    return
  end
  if self.screen == "confirm" or self.screen == "done" then return end

  local top = TITLE_BAR_H + HEADER_H
  if y < top or y >= self:footer_y() then return end
  local row = (y - top) // ROW_H + self.scroll
  if row < 0 or row >= self:row_count() then return end
  self.selected = row
  self:open_selected()
end

function CartApp:press_button(label)
  if label == "OPEN" then
    self:open_selected()
  elseif label == "CARDS" then
    self:back_to_roots()
  elseif label == "BACK" then
    self:back_to_browse()
  elseif label == "INSTALL" or label == "REPLACE" then
    self:install()
  elseif label == "RUN" then
    self:launch()
  end
end

function CartApp:back_to_roots()
  self.roots = self:existing_roots()
  self.screen = "roots"
  self.selected = 0
  self.scroll = 0
  self.message = nil
  self:redraw()
end

function CartApp:back_to_browse()
  self.screen = "browse"
  self.spec = nil
  self.cart_text = nil
  self.name_clash = nil
  self.message = nil
  self:redraw()
end

function CartApp:on_key(code, pressed)
  if not pressed then return end
  if self.screen == "confirm" then
    if code == AcidKeys.ENTER then
      self:install()
    elseif code == AcidKeys.ESCAPE or code == AcidKeys.BACKSPACE then
      self:back_to_browse()
    end
    return
  end
  if self.screen == "done" then
    if code == AcidKeys.ENTER then
      self:launch()
    elseif code == AcidKeys.ESCAPE or code == AcidKeys.BACKSPACE then
      self:back_to_roots()
    end
    return
  end
  if code == AcidKeys.UP then
    if self.selected > 0 then self.selected = self.selected - 1 end
    self:ensure_scroll()
    self:redraw()
  elseif code == AcidKeys.DOWN then
    if self.selected < self:row_count() - 1 then self.selected = self.selected + 1 end
    self:ensure_scroll()
    self:redraw()
  elseif code == AcidKeys.ENTER then
    self:open_selected()
  elseif code == AcidKeys.BACKSPACE or code == AcidKeys.ESCAPE then
    self:go_up()
  end
end

-- BACKSPACE walks back out: up one directory, out to the card list at
-- the root, and no further.
function CartApp:go_up()
  if self.screen == "roots" then return end
  local up = Cartfile.parent_dir(self.dir, self.root)
  if up == nil then
    self:back_to_roots()
  else
    self.dir = up
    self.message = nil
    self:scan_dir()
    self:redraw()
  end
end

CartApp:new():start()
