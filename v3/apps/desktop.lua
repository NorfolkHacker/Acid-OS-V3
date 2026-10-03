-- The desktop: the taskbar strip across the top of the screen (Menu/Back
-- button, one button per open window, a clock) and the Menu dropdown
-- launcher.
DesktopApp = AcidApp:extend("DesktopApp")

-- Must match the kernel_spawn_app(...) script_path that spawns this app
-- (the boot code) -- app_name is literally that path (the kernel registers
-- it verbatim), which is how the taskbar recognizes and skips its own
-- window.
DesktopApp.MY_APP_NAME = "v3/apps/desktop.lua"

-- Must match the boot code's own screen width (it hardcodes 640 directly in
-- its kernel_spawn_app(MY_APP_NAME, 0, 0, 640, ...) call) -- there's no
-- shared header between Lua and the Rust boot code to pull this from, so
-- it's kept in sync by comment, the same way MY_APP_NAME already has to be.
DesktopApp.SCREEN_W = 640

-- The visible taskbar strip's height -- matches the kernel's desktop strip
-- height (24), which is what the router uses to decide a touch belongs to
-- the desktop strip unconditionally, bypassing normal window z-order
-- hit-testing. Below this row, hit-testing is normal, and desktop only wins
-- it by being registered taller than this (see TOTAL_H) and briefly raised
-- to the front while its dropdown is open.
DesktopApp.STRIP_H = 24

DesktopApp.BUTTON_W = 60
DesktopApp.BUTTON_H = 18
DesktopApp.BUTTON_MARGIN_X = 2
DesktopApp.BUTTON_MARGIN_Y = 2

-- Always the leftmost slot in the strip -- Menu to open the dropdown,
-- Back to close it -- so there's one consistent place to tap regardless
-- of what's currently on screen. Moved here from the rightmost slot per
-- explicit user request; reserving a whole slot for it still means it
-- can never collide with a real window button, which now start at slot
-- 1 instead of slot 0 (see MAX_TASKBAR_SLOTS and draw_strip/on_touch's
-- own "+1"/"-1" slot-index comments below).
DesktopApp.MENU_SLOT_X = 0

-- Reserved space at the right edge of the strip for the clock (see
-- draw_clock) -- "23:59 31/12" is 11 chars, 66px at 6px/char; 90 leaves
-- real margin either side without eating into a whole extra BUTTON_W
-- column's worth of taskbar space unnecessarily.
DesktopApp.CLOCK_W = 90

-- How many window buttons actually fit in the strip once Menu's own
-- slot 0 and the clock's reserved space are both set aside. Windows
-- beyond this many are simply not listed in the taskbar (the window
-- itself is still open and usable, just not represented here) -- a
-- small-screen limitation that predates the launcher, just newly
-- reachable now that opening a 5th app is actually possible.
DesktopApp.MAX_TASKBAR_SLOTS = (DesktopApp.SCREEN_W - DesktopApp.BUTTON_W - DesktopApp.CLOCK_W) // DesktopApp.BUTTON_W

-- The dropdown: a small rectangle directly under the Menu button (NOT
-- the full screen width -- an explicit user request, since a single
-- column of app names never needed the other 480+ px of a 640px-wide
-- screen and it read as an oversized, out-of-place bar). Wide enough
-- for the longest real app name today ("System Monitor", 14 chars) with
-- a little breathing room either side.
DesktopApp.DROPDOWN_W = 150
DesktopApp.MAX_LAUNCHER_ITEMS = 10
DesktopApp.ITEM_H = 18
DesktopApp.DROPDOWN_H = DesktopApp.ITEM_H * DesktopApp.MAX_LAUNCHER_ITEMS
-- Desktop's own registered window still has to be the FULL screen width
-- (the taskbar strip above it spans the whole top edge) and TOTAL_H
-- tall (so the router's normal, non-strip hit-testing finds desktop
-- at all beneath the strip while the dropdown is open) -- only what
-- gets DRAWN inside that canvas shrank to a rectangle, not desktop's
-- own registered bounds. Must match the boot code's own
-- kernel_spawn_app(MY_APP_NAME, 0, 0, 640, TOTAL_H, 0) call, synced by
-- comment on both sides, same as SCREEN_W above.
DesktopApp.TOTAL_H = DesktopApp.STRIP_H + DesktopApp.DROPDOWN_H

DesktopApp.BG_COLOR = 0x0B1712      -- THEME_PANEL
-- Only used for the strip row itself now -- see on_create's own comment
-- for why the rest of desktop's (much taller) registered window is
-- repainted with the real wallpaper instead of this flat color.
DesktopApp.SCREEN_BG_COLOR = 0x050607 -- THEME_BG
DesktopApp.ACCENT_COLOR = 0x00FF66  -- THEME_HARD
DesktopApp.TEXT_COLOR = 0xD4E6DB    -- THEME_TEXT
DesktopApp.TEXT_DARK = 0x050607     -- THEME_BG -- used as the label color on an
                                    -- accent-filled focused button, for contrast
                                    -- (mirrors docs/BUILD_LOG.md's documented
                                    -- "pressed state inverts to a solid --hard
                                    -- fill, label switches to a dark color" rule).

-- Directory the launcher scans for <name>.app.toml manifests at boot.
-- Every *.lua file in here that has NO matching manifest (desktop.lua
-- itself, lib/*.lua) is simply never discovered -- the manifest, not the
-- directory, is what makes something launchable.
DesktopApp.APPS_DIR = "v3/apps"

-- Parses the leading integer, 0 if there isn't one.
local function to_i(s)
  return tonumber(s:match("^%s*([-+]?%d+)")) or 0
end

-- Splits on "\n"; drops trailing empty strings.
local function split_lines(text)
  local lines = {}
  for line in (text .. "\n"):gmatch("(.-)\n") do lines[#lines + 1] = line end
  while #lines > 0 and lines[#lines] == "" do lines[#lines] = nil end
  return lines
end

-- AcidApp#start already calls redraw once, automatically, right after
-- on_create -- this override exists only to build the launcher list
-- before that first redraw runs (so Menu's entry count is right from
-- frame one), not to skip that default behavior.
--
-- self.mode starts as "windows": starting it unset would make on_touch's
-- `~= "windows"` check swallow taskbar taps until the first dropdown
-- close.
function DesktopApp:on_create()
  self.mode = "windows"
  -- The strip row is real chrome (Menu/clock/window buttons), always a
  -- flat panel color -- draw_strip repaints it immediately after this
  -- anyway (AcidApp#start's automatic first redraw), this is just so the
  -- canvas never has raw zero-initialized black baked in before that.
  acid_fill_rect(0, 0, self.SCREEN_W, self.STRIP_H, self.SCREEN_BG_COLOR)
  -- Below the strip is desktop's own registered bounds but NOT chrome --
  -- it's only ever drawn on while the dropdown is open (see TOTAL_H's own
  -- comment on why desktop is registered this tall at all). Closed, it
  -- should show the real wallpaper, the same as any other unclaimed part
  -- of the screen -- acid_repaint_region does exactly that (see its own
  -- kernel-side comment), so reuse it here instead of a flat SCREEN_BG_COLOR
  -- fill, which used to paint a permanent black rectangle over the
  -- wallpaper's sun/stars/hills the instant desktop started.
  acid_repaint_region(0, self.STRIP_H, self.SCREEN_W, self.DROPDOWN_H)
  self:scan_launchable_apps()
end

function DesktopApp:on_idle()
  -- Nothing has to touch the desktop strip itself for the taskbar to go
  -- stale -- clicking directly from one app window to another is the
  -- common case. Checking on every idle timeout (roughly 5Hz, the
  -- existing 200ms acid_poll_event timeout) keeps the focus highlight
  -- and window list live without a dedicated notification channel --
  -- but only actually REDRAW when something's different from last time
  -- (self.last_state below), otherwise this unconditionally clears and
  -- repaints the whole strip 5 times a second with identical content,
  -- which is a real, continuous flicker for something that's visually a
  -- no-op almost all of the time. The dropdown never goes stale on its
  -- own (it's a fixed table, not live window state), so it never needs
  -- this at all.
  if self.mode == "launcher" then return end
  self:redraw_if_changed()
end

-- AcidApp#start calls this both for the very first paint (right after
-- on_create, self.last_state still nil so redraw_if_changed always draws
-- that time) and for every "moved" event -- and "moved" fires any time
-- ANY window's drag touches desktop's registered bounds, which (see
-- TOTAL_H's comment) covers virtually the whole screen, not just the
-- visible 24px strip. Before this delegated to redraw_if_changed it
-- unconditionally cleared and redrew the strip -- including the Menu
-- button -- on every single tick of dragging some OTHER, unrelated
-- window around, since that window's drag rect almost always overlaps
-- desktop's oversized dropdown-hit-test bounds even though the strip's
-- own visible content never changed. Reported live as "the word Menu
-- also flickers when moving the piano window". redraw_if_changed already
-- existed for exactly this reason on the on_idle path (see its own
-- comment) -- reusing it here covers the "moved" path with no new logic.
function DesktopApp:redraw()
  self:redraw_if_changed()
end

-- Same as redraw, but a no-op when the window list and focus state are
-- identical to last time -- see on_idle's comment for why this matters
-- (a plain redraw here would flicker the whole strip 5 times a second
-- for no visible change almost all of the time). Never called in
-- "launcher" mode (on_idle guards it, and on_touch only calls this from
-- the window-activate path, which needs mode == "windows"); it can run
-- with mode still nil before the first dropdown close.
function DesktopApp:redraw_if_changed()
  local windows = self:active_windows()
  local sig = self:state_signature(windows)
  if sig == self.last_state then return end
  self.last_state = sig
  self:draw_strip(windows)
  self:draw_menu_button()
  -- state_signature includes Config's wallpaper toggle (see its own
  -- comment) precisely so a change there lands here too -- the
  -- dropdown-closed area below the strip was already painted once, in
  -- on_create/close_menu, with whatever the toggle said at the time, and
  -- nothing else ever asks desktop to repaint it. Guarded the same way
  -- close_menu itself is guarded: never repaint over the dropdown's own
  -- drawn content while it's actually open (redraw, unlike this method's
  -- other callers, can run in "launcher" mode too -- see its own comment).
  if self.mode ~= "launcher" then
    acid_repaint_region(0, self.STRIP_H, self.SCREEN_W, self.DROPDOWN_H)
  end
end

function DesktopApp:on_touch(x, y, pressed)
  -- The router sends a TOUCH event on every ~16ms tick for as long as the
  -- mouse stays held, not just once on the initial press (demo_touch
  -- relies on exactly that, to draw a continuous trail while dragging).
  -- Without this guard, holding down on the Menu slot re-toggled the
  -- mode on every single one of those ticks -- Menu/Back/Menu/Back --
  -- which looked like intense flicker (reported live); holding on a
  -- dropdown row would have been worse, rapid-firing acid_launcher_spawn
  -- for as long as it was held. A plain click should act once per press.
  if not pressed then
    self.touch_held = false
    return
  end
  if self.touch_held then return end
  self.touch_held = true

  if y < self.STRIP_H then
    -- The strip row: Menu/Back, or a normal taskbar entry. Never both --
    -- in_menu_slot is checked first, so it always wins ties.
    if self:in_menu_slot(x) then
      if self.mode == "launcher" then self:close_menu() else self:open_menu() end
      return
    end
    if self.mode ~= "windows" then return end
    -- Slot 0 is Menu's own (already handled above by in_menu_slot, which
    -- always wins ties) -- window buttons start at slot 1, so this is the
    -- window-list index, not the raw column number.
    local window_slot = x // self.BUTTON_W - 1
    if window_slot < 0 or window_slot >= self.MAX_TASKBAR_SLOTS then return end
    local entry = self:active_windows()[window_slot + 1]
    if not entry then return end
    acid_activate_window(entry[1])
    self:redraw_if_changed()
    return
  end

  -- Below the strip: only meaningful while the dropdown is actually open
  -- (desktop only ever wins this region's hit-test then -- see TOTAL_H's
  -- comment). If the dropdown is closed, desktop still technically owns
  -- this rect (it's part of its registered window, for when the dropdown
  -- IS open) -- but the router's own generic body-touch handling (this
  -- event only reaches Lua at all because desktop won a hit-test) has
  -- ALREADY called kernel_router_activate_window on desktop before this
  -- method even runs, unconditionally, for any click landing on any
  -- window's body. Landing here specifically means the click was on
  -- empty desktop background -- no real window happened to cover that
  -- point -- and that automatic raise would otherwise leave desktop
  -- incorrectly stuck on top of this region (winning future hit-tests
  -- meant for whatever's normally there) until something else happens
  -- to get activated. Give it back immediately; nothing actually needs
  -- to change on screen since desktop draws nothing here anyway.
  if self.mode ~= "launcher" then
    acid_send_self_to_back()
    return
  end
  -- A tap to the right of the dropdown's own rectangle is a tap on empty
  -- desktop background, not on any row -- close the menu (the ordinary
  -- "tap outside to dismiss" a dropdown gets everywhere else) instead of
  -- silently doing nothing while leaving desktop stuck on top of
  -- whatever window actually lives under that empty space.
  if x >= self.DROPDOWN_W then
    self:close_menu()
    return
  end
  local row = (y - self.STRIP_H) // self.ITEM_H
  local indices = self:menu_indices()
  if row >= 0 and row < self.MAX_LAUNCHER_ITEMS and row < #indices then
    acid_launcher_spawn(indices[row + 1])
  end
  self:close_menu()
end

function DesktopApp:open_menu()
  self.mode = "launcher"
  local idx = self:my_window_index()
  -- Raise desktop above every other window so its now-larger registered
  -- bounds (TOTAL_H tall, not just STRIP_H) actually win hit-tests below
  -- the strip, and so the dropdown visibly draws on top of whatever's
  -- there. acid_activate_window is a no-op repaint-wise here (harmless)
  -- since desktop draws the dropdown itself, right below.
  if idx then acid_activate_window(idx) end
  self:draw_strip()
  self:draw_dropdown()
  self:draw_menu_button()
end

function DesktopApp:close_menu()
  self.mode = "windows"
  -- Give back both things open_menu claimed: z-order (so desktop stops
  -- winning hit-tests for whatever window actually lives below the
  -- strip) and the screen region itself (so that window's real content
  -- is visibly there again, not desktop's now-stale dropdown pixels).
  -- Order matters -- send_self_to_back must happen BEFORE the repaint,
  -- so the repaint's own z-order walk correctly treats desktop as the
  -- bottom-most window for this region instead of the topmost.
  acid_send_self_to_back()
  acid_repaint_region(0, self.STRIP_H, self.SCREEN_W, self.DROPDOWN_H)
  -- Explicit, unconditional draw_strip/draw_menu_button here, NOT redraw --
  -- redraw now delegates to redraw_if_changed (see its own comment), which
  -- would wrongly skip this if the window list/focus signature happens to
  -- be unchanged from before the dropdown opened. The Menu button's own
  -- label (Back -> Menu) depends on self.mode, which state_signature
  -- doesn't track, so that skip would leave it stuck reading "Back" after
  -- closing.
  self:draw_strip()
  self:draw_menu_button()
end

-- Desktop's own kernel index -- active_windows deliberately excludes it
-- (see its own comment), so this does the same unfiltered scan to find
-- it specifically. Only needed for the one acid_activate_window(idx)
-- call in open_menu.
function DesktopApp:my_window_index()
  local i = 0
  local max = acid_window_max()
  while i < max do
    local info = { acid_window_info(i) }
    if info[1] and info[1] == self.MY_APP_NAME then return i end
    i = i + 1
  end
  return nil
end

-- Finds every <name>.app.toml in APPS_DIR and registers the matching
-- <name>.lua as launchable. A manifest is a plain text file, a few
-- "key = value" lines -- not real TOML, just enough of its syntax to
-- read by hand with string splitting (a full TOML library would be a lot
-- of machinery for four fields). Malformed or unreadable
-- manifests are skipped, not fatal -- one bad file shouldn't take the
-- whole launcher down.
--
--   name = Demo Touch
--   w = 140
--   h = 100
function DesktopApp:scan_launchable_apps()
  -- Parallel to the kernel-side registry (acid_launcher_*), indexed the
  -- same way -- true unless the manifest says `menu = false`. Apps a user
  -- should reach by clicking their icon in File Manager rather than
  -- from this dropdown (games, Piano, etc.) opt out this way; they stay
  -- registered in the kernel registry regardless, so Terminal's `run`/
  -- `open` still finds them by name -- only THIS dropdown filters on it.
  self.menu_visible = {}
  local entries = acid_fs_list(self.APPS_DIR)
  -- A directory that can't even be opened shouldn't stop desktop.lua
  -- itself from starting -- it just means an empty launcher, a visible,
  -- self-explanatory state on its own.
  if not entries then return end
  local names = {}
  for _, entry in ipairs(entries) do
    if entry:sub(-#".app.toml") == ".app.toml" then names[#names + 1] = entry end
  end

  table.sort(names)
  for _, entry in ipairs(names) do
    -- pcall around each manifest: nothing a manifest does may crash the
    -- boot-only desktop (spec 11.3).
    pcall(self.register_launchable, self, self.APPS_DIR .. "/" .. entry)
  end
end

-- Body wrapped in pcall: a w/h digit run beyond i32 makes the binding
-- raise, and the manifest must then be skipped (spec 11.3), not take the
-- desktop down.
function DesktopApp:register_launchable(toml_path)
  pcall(self.register_launchable_unsafe, self, toml_path)
end

function DesktopApp:register_launchable_unsafe(toml_path)
  local stem = toml_path:sub(1, #toml_path - #".app.toml")
  local fields = self:parse_manifest(toml_path)
  -- One malformed/unreadable manifest shouldn't take the whole scan
  -- down -- just skip it and keep going with the rest.
  if not fields then return end
  -- Spec §15.4: `runtime = wasm` means the app is <stem>.wasm.
  local script_path = stem .. (fields["runtime"] == "wasm" and ".wasm" or ".lua")
  if not (fields["name"] and fields["w"] and fields["h"]) then return end
  -- true only for the handful of apps explicitly allowed more than one
  -- window at once (Editor, File Manager, Terminal) via `multi = true`
  -- in their own manifest -- every other app defaults to singleton
  -- (acid_launcher_spawn/acid_spawn_app focus the existing window
  -- instead of opening a second one).
  local multi = fields["multi"] == "true"
  -- An app with no `libs` line passes "", which the kernel reads as
  -- "no modules" -- see its own comment on the slot's libs.
  local libs = fields["libs"] or ""
  if not acid_launcher_register(script_path, fields["name"], to_i(fields["w"]),
                                to_i(fields["h"]), multi, libs) then
    return
  end
  self.menu_visible[#self.menu_visible + 1] = fields["menu"] ~= "false"
end

-- Registry indices (acid_launcher_name/spawn's own index space) that
-- this dropdown should actually list -- see scan_launchable_apps'
-- comment on menu_visible. A 1-based list of 0-based registry indices.
function DesktopApp:menu_indices()
  local indices = {}
  local i = 0
  local count = acid_launcher_count()
  while i < count do
    if self.menu_visible[i + 1] then indices[#indices + 1] = i end
    i = i + 1
  end
  return indices
end

function DesktopApp:parse_manifest(path)
  local text = acid_fs_read(path)
  if not text then return nil end
  local fields = {}
  for _, line in ipairs(split_lines(text)) do
    line = line:match("^%s*(.-)%s*$")
    if line ~= "" and line:sub(1, 1) ~= "#" then
      local eq = line:find("=", 1, true)
      if eq then
        local key = line:sub(1, eq - 1):match("^%s*(.-)%s*$")
        local value = line:sub(eq + 1):match("^%s*(.-)%s*$")
        fields[key] = value
      end
    end
  end
  return fields
end

function DesktopApp:in_menu_slot(x)
  return x < self.MENU_SLOT_X + self.BUTTON_W
end

-- A plain value that changes if and only if what the strip (and, see
-- redraw_if_changed, the dropdown-closed area below it) would actually
-- LOOK like changes: which apps are open, in which slots, which one is
-- focused, the clock text, and Config's wallpaper on/off toggle. Lua
-- can't compare tables by value, so this is one concatenated string. Position/size aren't included -- the
-- taskbar never draws those -- so a window being dragged around the
-- screen doesn't cause the strip to think it needs a redraw every tick.
-- The clock and wallpaper flag ARE included deliberately -- they're the
-- parts of this that can change on their own (a minute ticking over, a
-- setting flipped in another window), with no window-list change to
-- trigger a redraw otherwise.
function DesktopApp:state_signature(windows)
  local parts = {}
  for _, entry in ipairs(windows) do
    local info = entry[2]
    parts[#parts + 1] = info[1] .. "\1" .. tostring(info[6]) .. "\2"
  end
  return table.concat(parts) .. self:clock_text() .. "\3" .. tostring(acid_get_wallpaper_enabled())
end

-- { { kernel_index, info }, ... } for every in-use window except this one
-- (info is { name, x, y, w, h, focused }). Recomputed on every call rather
-- than cached -- at most 8 entries, and avoids any staleness between what's
-- drawn and what a tap acts on.
function DesktopApp:active_windows()
  local list = {}
  local i = 0
  local max = acid_window_max()
  while i < max do
    local info = { acid_window_info(i) }
    if info[1] and info[1] ~= self.MY_APP_NAME then list[#list + 1] = { i, info } end
    i = i + 1
  end
  return list
end

function DesktopApp:draw_strip(windows)
  windows = windows or self:active_windows()
  acid_fill_rect(0, 0, self.SCREEN_W, self.STRIP_H, self.BG_COLOR)
  -- +1: slot 0 is reserved for the Menu button (see MENU_SLOT_X/
  -- MAX_TASKBAR_SLOTS above), window buttons start at slot 1.
  for slot = 0, math.min(#windows, self.MAX_TASKBAR_SLOTS) - 1 do
    self:draw_button(slot + 1, windows[slot + 1][2])
  end
  self:draw_clock()
end

-- This machine's real wall clock (acid_local_time). Minute resolution, not
-- seconds -- a taskbar clock ticking every second would force a full strip
-- repaint every second for no useful gain here.
function DesktopApp:clock_text()
  local _, mon, day, hour, min = acid_local_time()
  return string.format("%02d:%02d %02d/%02d", hour, min, day, mon)
end

function DesktopApp:draw_clock()
  local text = self:clock_text()
  local x = self.SCREEN_W - #text * 6 - 4
  acid_draw_text(text, x, 8, self.TEXT_COLOR, self.BG_COLOR)
end

function DesktopApp:draw_button(slot, info)
  local name = info[1]
  local focused = info[6]
  local x = slot * self.BUTTON_W + self.BUTTON_MARGIN_X
  local y = self.BUTTON_MARGIN_Y
  local w = self.BUTTON_W - self.BUTTON_MARGIN_X * 2
  local h = self.BUTTON_H
  local bg = focused and self.ACCENT_COLOR or self.BG_COLOR
  local fg = focused and self.TEXT_DARK or self.TEXT_COLOR
  acid_fill_rect(x, y, w, h, bg)
  acid_draw_text(self:short_name(name), x + 3, y + 5, fg, bg)
end

-- A small rectangle (DROPDOWN_W wide, not the full screen) directly
-- under the Menu button, one row per launchable app -- see DROPDOWN_W's
-- own comment on why this isn't full-width.
function DesktopApp:draw_dropdown()
  acid_fill_rect(0, self.STRIP_H, self.DROPDOWN_W, self.DROPDOWN_H, self.BG_COLOR)
  local indices = self:menu_indices()
  local i = 0
  while i < #indices and i < self.MAX_LAUNCHER_ITEMS do
    local y = self.STRIP_H + i * self.ITEM_H
    acid_draw_text(acid_launcher_name(indices[i + 1]):sub(1, 22), 6, y + 4, self.TEXT_COLOR, self.BG_COLOR)
    i = i + 1
  end
end

-- Always the same reserved leftmost slot (MENU_SLOT_X) -- highlighted
-- like a focused window button while the dropdown is open, as a "you are
-- here, tap to close" cue; plain otherwise.
function DesktopApp:draw_menu_button()
  local in_launcher = (self.mode == "launcher")
  local label = in_launcher and "Back" or "Menu"
  local bg = in_launcher and self.ACCENT_COLOR or self.BG_COLOR
  local fg = in_launcher and self.TEXT_DARK or self.TEXT_COLOR
  local x = self.MENU_SLOT_X + self.BUTTON_MARGIN_X
  local y = self.BUTTON_MARGIN_Y
  local w = self.BUTTON_W - self.BUTTON_MARGIN_X * 2
  local h = self.BUTTON_H
  acid_fill_rect(x, y, w, h, bg)
  acid_draw_text(label, x + 3, y + 5, fg, bg)
end

-- app_name is a full script path (e.g. "v3/apps/acid_blaster.lua") -- show
-- just the filename, stripped of directory and extension, truncated to
-- fit the button.
function DesktopApp:short_name(path)
  local base = path:match("[^/]*$")
  local dot = base:match("^.*()%.")
  if dot then base = base:sub(1, dot - 1) end
  if #base > 8 then base = base:sub(1, 8) end
  return base
end

DesktopApp:new():start()
