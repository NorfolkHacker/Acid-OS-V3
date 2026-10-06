-- Tests for the half of the cart loader that actually touches files --
-- browsing a card, reading a cart, and writing it into the apps dir. The
-- pure decisions live in apps/cart/cartfile.lua and are covered by
-- test_cart.lua; this covers what CartApp does with them, against fakes
-- of the host cart folders (CART_FS) and the apps dir (FS), since
-- "refuses to overwrite desktop.lua" is a claim about files, not strings.
-- Run by game_tests.rs.

local HELLO_CART = "-- name: Hello Acid\n-- w: 200\n-- h: 150\n-- desc: Example cart\n-- libs: lib/acid_palette.lua, ../../etc/passwd.lua\n\nHelloApp = {}\n"

CART_ROOTS = { "v3/carts" }
CART_FS["v3/carts"] = { "hello.cart", "impostor.cart", "desktop.cart", "tetris.cart",
                        "big.cart", "notes.txt", "games", "escape" }
CART_FS["v3/carts/hello.cart"] = HELLO_CART
CART_FS["v3/carts/impostor.cart"] = "-- name: Hello Acid\n\nImpostor = {}\n"
CART_FS["v3/carts/desktop.cart"] = "-- name: Not The Desktop\n\nEvil = {}\n"
CART_FS["v3/carts/tetris.cart"] = "-- name: Not Tetris\n\nEvil = {}\n"
CART_FS["v3/carts/big.cart"] = string.rep("-", 257 * 1024)
CART_FS["v3/carts/notes.txt"] = "just notes\n"
CART_FS["v3/carts/games"] = { "demo.cart" }
CART_FS["v3/carts/games/demo.cart"] = "Demo = {}\n"
CART_LINKS["v3/carts/escape"] = true
-- A .wasm cart: magic + version, then a custom section "acid" (id 0).
local WASM_PAYLOAD = "name: Wasm Toy\nw: 180\nh: 120\n"
local WASM_CART = "\0asm\1\0\0\0" .. "\0" .. string.char(5 + #WASM_PAYLOAD) .. "\4acid" .. WASM_PAYLOAD
CART_FS["v3/carts"][#CART_FS["v3/carts"] + 1] = "toy.wasm"
CART_FS["v3/carts/toy.wasm"] = WASM_CART

FS["v3/apps/lib"] = { "acid_palette.lua", "acid_game.lua" }
FS["v3/apps/desktop.lua"] = "-- the real desktop\n"
FS["v3/apps/tetris.lua"] = "-- the real tetris\n"
FS["v3/apps/tetris.app.toml"] = "name = Tetris\nw = 160\nh = 160\n"

local app = GAME
-- The app started before the fixtures above existed; restart it so it
-- reads them.
app:on_create()

local function has(list, v)
  for _, x in ipairs(list) do if x == v then return true end end
  return false
end

local function index_of(list, v)
  for i, x in ipairs(list) do if x == v then return i end end
  return nil
end

local function spawned()
  local out = {}
  for _, c in ipairs(CALLS) do
    if c[1] == "spawn" then out[#out + 1] = { c[2], c[3], c[4], c[5] } end
  end
  return out
end

-- Opens the repo cart root and selects the entry with this filename,
-- the way tapping its row does.
local function select_cart(name)
  app:open_root("v3/carts")
  local i = 0
  while i < #app.entries do
    if app.entries[i + 1].name == name then break end
    i = i + 1
  end
  if i >= #app.entries then error("no such entry: " .. name) end
  app.selected = i
  app:open_selected()
end

-- ---------------------------------------------------------------- roots

group("roots: which cards are offered")
eq(has(app.roots, "v3/carts"), true, "the repo's own carts directory is a root")
eq(has(app.roots, "/etc"), false, "nothing outside the configured roots is offered")
eq(app.screen, "roots", "the app opens on the card list")

-- -------------------------------------------------------------- listing

app:open_root("v3/carts")

group("listing: what a card shows")
local names = {}
for i, e in ipairs(app.entries) do names[i] = e.name end
eq(has(names, "hello.cart"), true, "a .cart file is listed")
eq(has(names, "games"), true, "a subdirectory is listed")
eq(has(names, "notes.txt"), false, "an unrelated file on the card is not listed")
eq(has(names, ".."), false, "the root itself offers no way up out of it")
eq(has(names, "escape"), false, "a symlink is skipped rather than followed out of the root")

group("listing: descending and coming back")
app.selected = index_of(names, "games") - 1
app:open_selected()
eq(app.dir, "v3/carts/games", "opening a directory descends into it")
local sub = {}
for i, e in ipairs(app.entries) do sub[i] = e.name end
eq(has(sub, ".."), true, "a subdirectory offers a way back up")
app:go_up()
eq(app.dir, "v3/carts", "going up returns to the root")
app:go_up()
eq(app.screen, "roots", "going up from the root returns to the card list")

-- ------------------------------------------------------- a clean install

select_cart("hello.cart")

group("confirm: what the cart asked for")
eq(app.screen, "confirm", "selecting a cart opens the confirm screen")
eq(app.status, "fresh", "an unused slot is a fresh install")
local spec = app.spec
eq(spec.name, "Hello Acid", "the header's name is used")
eq(spec.w, 200, "the header's width is used")
eq(spec.script_path, "v3/apps/hello.lua", "the destination is inside the apps dir")
eq(spec.libs, { "lib/acid_palette.lua" },
   "a real module is kept and the traversal entry beside it is dropped")

app:install()

group("install: what landed on disk")
eq(app.screen, "done", "a successful install moves on")
eq(FS["v3/apps/hello.lua"] ~= nil, true, "the source was written")
eq(FS["v3/apps/hello.lua"], HELLO_CART, "the source is a byte-for-byte copy of the cart")
eq(FS["v3/apps/hello.app.toml"],
   "name = Hello Acid\nw = 200\nh = 150\ndesc = Example cart\nlibs = lib/acid_palette.lua\nsource = cart\n",
   "the generated manifest carries the header and marks itself cart-installed")

group("install: running it straight away")
app:launch()
eq(#LAUNCHER, 1, "the new app is registered once")
eq(LAUNCHER[1], { "v3/apps/hello.lua", "Hello Acid", 200, 150, false, "lib/acid_palette.lua" },
   "it registers with the manifest's own values")
eq(spawned(), { { "v3/apps/hello.lua", 200, 150, "" } }, "and is spawned")
app:launch()
eq(#LAUNCHER, 1, "launching twice does not register it twice")

-- ------------------------------------------------ a name already in use

select_cart("impostor.cart")

group("name clash: a cart claiming an installed app's name")
eq(app.status, "fresh", "its own slot is free, so the install itself is allowed")
eq(app.name_clash, "v3/apps/hello.lua",
   "but the confirm screen knows whose name it is taking")

select_cart("hello.cart")
eq(app.name_clash, nil, "reinstalling a cart does not collide with itself")

-- ------------------------------------------------------- replacing a cart

select_cart("hello.cart")

group("replace: a slot this loader filled before")
eq(app.status, "replace", "an installed cart offers a replace, not a fresh install")
app:install()
eq(app.screen, "done", "the replace goes through")

-- ------------------------------------------------- refusing built-in apps

select_cart("desktop.cart")

group("protected: a built-in app with no manifest of its own")
eq(app.status, "protected", "desktop.lua's slot is protected")
app:install()
eq(FS["v3/apps/desktop.lua"], "-- the real desktop\n", "the real desktop is untouched")
eq(FS["v3/apps/desktop.app.toml"] ~= nil, false, "and no manifest was invented for it")
eq(app.screen, "confirm", "the app stays on the confirm screen rather than reporting success")

select_cart("tetris.cart")

group("protected: a built-in app with a manifest")
eq(app.status, "protected", "a manifest with no cart marker is protected")
app:install()
eq(FS["v3/apps/tetris.lua"], "-- the real tetris\n", "the real tetris is untouched")
eq(FS["v3/apps/tetris.app.toml"], "name = Tetris\nw = 160\nh = 160\n", "its manifest is untouched")

-- ---------------------------------------------------------- oversized

select_cart("big.cart")

group("oversized: refused before it is even read")
eq(app.screen, "browse", "an outsized cart never reaches the confirm screen")
eq(app.message == nil, false, "and says why")
eq(FS["v3/apps/big.lua"] ~= nil, false, "nothing was written")

-- ------------------------------------------- a cart whose name is hostile

group("hostile name: cannot reach outside the apps dir")
app:open_root("v3/carts")
app:inspect_cart({ name = "../../../../etc/cron.cart", path = "v3/carts/games/demo.cart",
                   dir = false, size = 10 })
eq(app.spec.script_path, "v3/apps/cron.lua", "a traversal filename still installs inside the apps dir")
app:install()
eq(FS["v3/apps/cron.lua"] ~= nil, true, "it installed under its sanitised name")
eq(FS["../etc/cron.lua"] ~= nil, false, "and nothing was written outside the tree")

-- ------------------------------------- a file that vanishes after listing

group("unreadable size: a listed cart whose stat fails")
-- Listed, but with no CART_FS entry of its own, so acid_cart_stat fails
-- (a file removed after listing, or a lossy non-UTF-8 name).
local card = CART_FS["v3/carts"]
card[#card + 1] = "ghost.cart"
app:open_root("v3/carts")
local gi
for i, e in ipairs(app.entries) do if e.name == "ghost.cart" then gi = i end end
app.selected = gi - 1
app:open_selected()
eq(app.screen == "browse" and app.message, "empty file", "a cart whose size cannot be read says empty file and stays on the card")

-- ------------------------------------------------- a failed manifest write

-- Spec §14.2: a .lua in the apps dir with no manifest beside it runs at
-- built-in trust, so the manifest (which says source = cart) goes down
-- first and the source only after it. If the manifest write fails, the
-- cart's code must never reach the disk.
group("install order: the manifest goes first")
FAIL_WRITES["v3/apps/fresh.app.toml"] = true
app:open_root("v3/carts")
app:inspect_cart({ name = "fresh.cart", path = "v3/carts/games/demo.cart", dir = false, size = 10 })
eq(app.status, "fresh", "an unused slot")
CALLS = {}
app:install()
eq(FS["v3/apps/fresh.lua"], nil, "a failed manifest write leaves no source behind")
local wrote_lua = false
for _, c in ipairs(CALLS) do
  if c[1] == "write" and c[2] == "v3/apps/fresh.lua" then wrote_lua = true end
end
eq(wrote_lua, false, "the source write was never even attempted")
eq(app.screen, "confirm", "it does not report success")
eq(app.message, "could not write v3/apps/fresh.app.toml", "and says which file failed")
FAIL_WRITES["v3/apps/fresh.app.toml"] = nil

group("install order: a failed source write after the manifest")
FAIL_WRITES["v3/apps/fresh.lua"] = true
app:inspect_cart({ name = "fresh.cart", path = "v3/carts/games/demo.cart", dir = false, size = 10 })
app:install()
eq(FS["v3/apps/fresh.lua"], nil, "no source")
eq(app.screen, "confirm", "it does not report success")
eq(app.message, "wrote manifest but not source -- not installed", "and says so plainly")
FAIL_WRITES["v3/apps/fresh.lua"] = nil

-- ------------------------------------------------------------ a wasm cart

-- Spec §15.4: same install order and messages as a .cart, but the bytes
-- are written as <slug>.wasm and the manifest says runtime = wasm.
group("wasm: listing, install and run")
app:open_root("v3/carts")
local wnames = {}
for i, e in ipairs(app.entries) do wnames[i] = e.name end
eq(has(wnames, "toy.wasm"), true, "a .wasm file is listed")
select_cart("toy.wasm")
eq(app.screen, "confirm", "selecting it opens the confirm screen")
eq(app.spec.script_path, "v3/apps/toy.wasm", "the destination is a .wasm in the apps dir")
CALLS = {}
app:install()
eq(app.screen, "done", "the install succeeds")
eq(FS["v3/apps/toy.app.toml"], "name = Wasm Toy\nw = 180\nh = 120\nruntime = wasm\nsource = cart\n", "the manifest says runtime = wasm")
eq(FS["v3/apps/toy.wasm"], WASM_CART, "the module is copied byte for byte")
local order = {}
for _, c in ipairs(CALLS) do if c[1] == "write" then order[#order + 1] = c[2] end end
eq(order, { "v3/apps/toy.app.toml", "v3/apps/toy.wasm" }, "the manifest is written before the module")
CALLS = {}
app:launch()
eq(spawned()[1][1], "v3/apps/toy.wasm", "RUN spawns the .wasm")

-- ------------------------------------------- slots shared by .lua and .wasm

-- A desktop.wasm cart must see the real desktop.lua as occupying the slot.
group("wasm: the slot check covers .lua too")
CART_FS["v3/carts"][#CART_FS["v3/carts"] + 1] = "desktop.wasm"
CART_FS["v3/carts/desktop.wasm"] = WASM_CART
select_cart("desktop.wasm")
eq(app.status, "protected", "a desktop.wasm cart finds desktop.lua's slot protected")
CALLS = {}
app:install()
local wrote = false
for _, c in ipairs(CALLS) do if c[1] == "write" then wrote = true end end
eq(wrote, false, "and nothing is written")
eq(FS["v3/apps/desktop.app.toml"], nil, "no manifest lands for the desktop")

-- A .wasm cart over an installed .lua cart replaces it, and the old .lua
-- goes: it would otherwise sit on disk, dead, with a stale launcher entry.
group("wasm: replacing across runtimes")
local SWAP_PAYLOAD = "name: Swap\nw: 100\nh: 100\n"
local SWAP_WASM = "\0asm\1\0\0\0" .. "\0" .. string.char(5 + #SWAP_PAYLOAD) .. "\4acid" .. SWAP_PAYLOAD
FS["v3/apps/swap.lua"] = "-- old lua cart\n"
FS["v3/apps/swap.app.toml"] = "name = Swap\nw = 100\nh = 100\nsource = cart\n"
acid_launcher_register("v3/apps/swap.lua", "Swap", 100, 100, false, "")
CART_FS["v3/carts"][#CART_FS["v3/carts"] + 1] = "swap.wasm"
CART_FS["v3/carts/swap.wasm"] = SWAP_WASM
CART_FS["v3/carts"][#CART_FS["v3/carts"] + 1] = "swap.cart"
CART_FS["v3/carts/swap.cart"] = "-- name: Swap\n\nSwap = {}\n"
select_cart("swap.wasm")
eq(app.status, "replace", "a .wasm cart over an installed .lua cart is a replace")
eq(app.name_clash, nil, "the old .lua entry is the cart itself, not a name clash")
CALLS = {}
app:install()
eq(app.screen, "done", "the replace goes through")
eq(FS["v3/apps/swap.wasm"], SWAP_WASM, "the module is written")
eq(FS["v3/apps/swap.lua"], nil, "the old .lua is deleted")
local seen = {}
for _, c in ipairs(CALLS) do
  if c[1] == "write" or c[1] == "delete" then seen[#seen + 1] = c[1] .. " " .. c[2] end
end
eq(seen, { "write v3/apps/swap.app.toml", "write v3/apps/swap.wasm", "delete v3/apps/swap.lua" },
   "the old script goes only after the new one has landed")

-- And back again: a .cart over the .wasm cart removes the .wasm.
select_cart("swap.cart")
eq(app.status, "replace", "a .cart over an installed .wasm cart is a replace")
app:install()
eq(FS["v3/apps/swap.lua"], "-- name: Swap\n\nSwap = {}\n", "the .lua is written")
eq(FS["v3/apps/swap.wasm"], nil, "the old .wasm is deleted")

group("wasm: a protected slot's other script is not the cart's own")
FS["v3/apps/guard.lua"] = "-- a built-in\n"
acid_launcher_register("v3/apps/guard.lua", "Guard", 100, 100, false, "")
CART_FS["v3/carts"][#CART_FS["v3/carts"] + 1] = "guard.wasm"
local GUARD_PAYLOAD = "name: Guard\n"
CART_FS["v3/carts/guard.wasm"] = "\0asm\1\0\0\0" .. "\0" .. string.char(5 + #GUARD_PAYLOAD) .. "\4acid" .. GUARD_PAYLOAD
select_cart("guard.wasm")
eq(app.status, "protected", "a built-in's slot is protected")
eq(app.name_clash, "v3/apps/guard.lua", "and taking the built-in's name is still a clash")
app:install()
eq(FS["v3/apps/guard.lua"], "-- a built-in\n", "the built-in is untouched")

group("category: a game cart lands in Games")
CART_FS["v3/carts"][#CART_FS["v3/carts"] + 1] = "rocks.cart"
CART_FS["v3/carts/rocks.cart"] = "-- name: Rocks\n-- category: game\n\nRocks = {}\n"
select_cart("rocks.cart")
eq(app.spec.category, "game", "the confirm screen knows it is a game")
TEXTS = {}
app:draw_confirm()
eq(has(TEXTS, "game"), true, "and shows it")
app:install()
eq(FS["v3/apps/rocks.app.toml"], "name = Rocks\nw = 220\nh = 160\ncategory = game\nsource = cart\n",
   "the manifest says category = game, before source = cart")
