-- Headless tests for the cart loader's pure module (apps/cart/cartfile.lua).
-- Nothing here calls an acid_* binding or touches the filesystem. Run by
-- v3/crates/acid-lua/tests/game_tests.rs (eq/ok/group come from
-- game_test_env.lua).

local LIBS = { "lib/acid_app.lua", "lib/acid_game.lua", "lib/acid_keys.lua" }

-- ------------------------------------------------------------------ slug
--
-- The destination filename a cart installs as. This is the security-
-- critical one: whatever the host file is called, the result has to be a
-- bare name that can only ever land inside v3/apps.

group("slug: ordinary names")
eq(Cartfile.slug("hello_acid.cart"), "hello_acid", "plain name keeps its stem")
eq(Cartfile.slug("Acid Snake.cart"), "acid_snake", "spaces and case fold to lowercase underscores")
eq(Cartfile.slug("/media/usb/games/Blaster.cart"), "blaster", "a full path keeps only the basename")

group("slug: hostile names")
eq(Cartfile.slug("../../desktop.cart"), "desktop", "traversal components are stripped, not obeyed")
eq(Cartfile.slug("/etc/passwd.cart"), "passwd", "an absolute path cannot escape the apps dir")
eq(Cartfile.slug("a/../../../b.cart"), "b", "traversal in the middle is stripped too")
eq(Cartfile.slug("evil\nname.cart"), "evil_name", "a newline in the name cannot inject a second path")
eq(Cartfile.slug("weird!!!name.cart"), "weird_name", "a run of junk collapses to one underscore")
eq(Cartfile.slug("..cart"), nil, "a name with no usable characters is refused")
eq(Cartfile.slug("....cart"), nil, "dots alone are refused")
eq(Cartfile.slug(""), nil, "an empty name is refused")
eq(Cartfile.slug("/"), nil, "a bare slash is refused")
eq(Cartfile.slug("___.cart"), nil, "underscores alone are refused")
eq(Cartfile.slug(string.rep("z", 60) .. ".cart"), string.rep("z", Cartfile.MAX_SLUG), "an overlong name is truncated")

group("slug: extension handling")
eq(Cartfile.slug("game.CART"), "game", "the .cart suffix matches case-insensitively")
eq(Cartfile.slug("notes.txt"), "notes_txt", "a non-cart extension is folded into the name, not stripped")

-- ---------------------------------------------------------------- cart

group("cart: which host files are even offered")
eq(Cartfile.cart("snake.cart"), true, "a .cart file is offered")
eq(Cartfile.cart("SNAKE.CART"), true, "case does not matter")
eq(Cartfile.cart("snake.lua"), false, "a stray .lua is not")
eq(Cartfile.cart(".cart"), false, "a bare extension with no stem is not")
eq(Cartfile.cart("cart"), false, "a file merely named cart is not")

-- --------------------------------------------------------- parse_header

local HEADER = [[
-- name: Acid Snake
-- w: 160
-- h: 140
-- desc: Snake, but it melts
-- libs: lib/acid_game.lua, lib/acid_keys.lua

local SnakeApp = AcidApp:extend("SnakeApp")
]]

group("parse_header: the documented form")
eq(Cartfile.parse_header(HEADER)["name"], "Acid Snake", "name is read")
eq(Cartfile.parse_header(HEADER)["w"], "160", "w is read as its raw string")
eq(Cartfile.parse_header(HEADER)["desc"], "Snake, but it melts", "desc is read")
eq(Cartfile.parse_header(HEADER)["libs"], "lib/acid_game.lua, lib/acid_keys.lua", "libs is read raw")

group("parse_header: tolerated spellings")
eq(Cartfile.parse_header("--name:Tight\n")["name"], "Tight", "no spaces around the colon still parses")
eq(Cartfile.parse_header("-- NAME: Shouty\n")["name"], "Shouty", "keys are case-insensitive")
eq(Cartfile.parse_header("-- name: CRLF\r\n-- w: 90\r\n")["name"], "CRLF", "a CRLF file leaves no stray carriage return")
eq(Cartfile.parse_header("-- name: CRLF\r\n-- w: 90\r\n")["w"], "90", "CRLF on a numeric field too")
eq(Cartfile.parse_header("\n\n-- name: Late\n")["name"], "Late", "blank lines before the header are skipped")

group("parse_header: where the header stops")
eq(Cartfile.parse_header("local foo = 1\n-- name: Sneaky\n")["name"], nil,
   "a header line after real code is ignored")
eq(Cartfile.parse_header("-- name: First\nlocal foo = 1\n-- name: Second\n")["name"], "First",
   "only the leading comment block counts")
eq(Cartfile.parse_header("local foo = 1\nlocal bar = 2\n"), {}, "a cart with no header parses to nothing")
eq(Cartfile.parse_header("-- just a comment\n-- name: Real\n")["name"], "Real",
   "an unrelated comment does not end the header")
eq(Cartfile.parse_header("-- name: X\n-- evil: rm -rf\n")["evil"], nil, "unknown keys are dropped")

group("parse_header: value hygiene")
eq(#Cartfile.parse_header("-- name: " .. string.rep("n", 90) .. "\n")["name"], Cartfile.MAX_TEXT,
   "an overlong value is truncated")
eq(Cartfile.parse_header("-- desc: tab\there\n")["desc"], "tab here", "control characters become spaces")
eq(Cartfile.parse_header("-- name:   \n")["name"], nil, "an empty value is dropped, not stored blank")

-- ----------------------------------------------------------- dimension

group("dimension: clamping a cart's window request")
eq(Cartfile.dimension("160", 220, Cartfile.MIN_W, Cartfile.MAX_W), 160, "an in-range width is kept")
eq(Cartfile.dimension(nil, 220, Cartfile.MIN_W, Cartfile.MAX_W), 220, "a missing width falls back")
eq(Cartfile.dimension("", 220, Cartfile.MIN_W, Cartfile.MAX_W), 220, "an empty width falls back")
eq(Cartfile.dimension("99999", 220, Cartfile.MIN_W, Cartfile.MAX_W), Cartfile.MAX_W,
   "an absurd width is clamped to the screen")
eq(Cartfile.dimension("-40", 220, Cartfile.MIN_W, Cartfile.MAX_W), Cartfile.MIN_W,
   "a negative width is clamped up")
eq(Cartfile.dimension("abc", 220, Cartfile.MIN_W, Cartfile.MAX_W), Cartfile.MIN_W,
   "a non-numeric width lands on the minimum, never zero")
eq(Cartfile.dimension("140", 160, Cartfile.MIN_H, Cartfile.MAX_H), 140, "heights clamp on their own bounds")

-- ----------------------------------------------------------- filter_libs

group("filter_libs: only modules that actually exist")
eq(Cartfile.filter_libs("lib/acid_game.lua, lib/acid_keys.lua", LIBS),
   { "lib/acid_game.lua", "lib/acid_keys.lua" }, "known modules pass through")
eq(Cartfile.filter_libs("lib/acid_game.lua", LIBS), { "lib/acid_game.lua" }, "a single module passes")
eq(Cartfile.filter_libs(nil, LIBS), {}, "no libs line means no modules")
eq(Cartfile.filter_libs("", LIBS), {}, "an empty libs line means no modules")

group("filter_libs: hostile entries")
eq(Cartfile.filter_libs("../../../etc/passwd.lua", LIBS), {}, "traversal is not a known module")
eq(Cartfile.filter_libs("/etc/shadow", LIBS), {}, "an absolute path is not a known module")
eq(Cartfile.filter_libs("lib/../desktop.lua", LIBS), {}, "traversal dressed as a lib path is dropped")
eq(Cartfile.filter_libs("desktop.lua", LIBS), {}, "a real app outside lib/ cannot be pulled in")
eq(Cartfile.filter_libs("lib/acid_game.lua, lib/nope.lua", LIBS), { "lib/acid_game.lua" },
   "one bad entry does not poison the good ones")
eq(Cartfile.filter_libs("lib/acid_game.lua, lib/acid_game.lua", LIBS), { "lib/acid_game.lua" },
   "duplicates collapse")
eq(#Cartfile.filter_libs(string.rep("lib/acid_game.lua,", 19) .. "lib/acid_game.lua", LIBS), 1,
   "a flood of duplicates still collapses to one")

-- --------------------------------------------------------- manifest_text

group("manifest_text: the .app.toml a cart installs as")
local MANI = Cartfile.manifest_text({ name = "Acid Snake", w = 160, h = 140,
                                      desc = "Snake, but it melts", libs = { "lib/acid_game.lua" } })
local function has(s, part) return s:find(part, 1, true) ~= nil end
eq(has(MANI, "name = Acid Snake\n"), true, "carries the name")
eq(has(MANI, "w = 160\n"), true, "carries the width")
eq(has(MANI, "h = 140\n"), true, "carries the height")
eq(has(MANI, "desc = Snake, but it melts\n"), true, "carries the description")
eq(has(MANI, "libs = lib/acid_game.lua\n"), true, "carries the module list")
eq(has(MANI, "source = cart\n"), true, "marks itself as cart-installed")
eq(MANI:sub(#MANI, #MANI), "\n", "ends with a newline like every other manifest")

group("manifest_text: omissions")
local BARE = Cartfile.manifest_text({ name = "Bare", w = 100, h = 100, desc = "", libs = {} })
eq(has(BARE, "libs"), false, "no modules means no libs line at all")
eq(has(BARE, "desc"), false, "no description means no desc line")

group("manifest_text: resizing opt-in")
local RHEAD = "-- name: Rz\n-- resizable: true\n-- min_w: 100\n-- min_h: 60\n"
local RSPEC = Cartfile.from_cart("rz.cart", RHEAD, LIBS)
eq(RSPEC.resizable, true, "resizable = true is accepted")
eq(RSPEC.min_w, 100, "min_w is read")
eq(RSPEC.min_h, 60, "min_h is read")
local RMANI = Cartfile.manifest_text(RSPEC)
eq(has(RMANI, "resizable = true\n"), true, "the manifest carries resizable")
eq(has(RMANI, "min_w = 100\n"), true, "the manifest carries min_w")
eq(has(RMANI, "min_h = 60\n"), true, "the manifest carries min_h")
eq(RMANI:find("min_h = 60\n", 1, true) < RMANI:find("source = cart", 1, true), true, "all before source = cart")
local YES = Cartfile.manifest_text(Cartfile.from_cart("rz.cart", "-- resizable: yes\n", LIBS))
eq(has(YES, "resizable"), false, "only the exact value true opts in")
eq(Cartfile.from_cart("rz.cart", "-- min_w: abc\n", LIBS).min_w, Cartfile.MIN_W, "a junk min_w clamps to the minimum")
eq(has(BARE, "min_w"), false, "no header, no minimums")

group("manifest_text: category = game")
local GSPEC = Cartfile.from_cart("g.cart", "-- name: G\n-- category: game\n", LIBS)
eq(GSPEC.category, "game", "category: game is read")
eq(Cartfile.from_cart("g.cart", "-- category:  GaMe \n", LIBS).category, "game", "case and spaces don't matter")
eq(Cartfile.from_cart("g.cart", "-- category: app\n", LIBS).category, nil, "app is the default, so nothing is kept")
eq(Cartfile.from_cart("g.cart", "-- category: toy\n", LIBS).category, nil, "an unknown category is dropped")
eq(Cartfile.from_cart("g.cart", "-- name: G\n", LIBS).category, nil, "no line, no category")
local GMANI = Cartfile.manifest_text(GSPEC)
eq(has(GMANI, "category = game\n"), true, "the manifest carries category = game")
eq(GMANI:find("category = game", 1, true) < GMANI:find("source = cart", 1, true), true, "before source = cart")
eq(has(BARE, "category"), false, "no category means no category line")

group("manifest_text: menu = false")
local MSPEC = Cartfile.from_cart("m.cart", "-- name: M\n-- menu: false\n", LIBS)
eq(MSPEC.menu, false, "menu: false is read")
eq(Cartfile.from_cart("m.cart", "-- menu:  FaLsE \n", LIBS).menu, false, "case and spaces don't matter")
eq(Cartfile.from_cart("m.cart", "-- menu: true\n", LIBS).menu, nil, "true is the default, so nothing is kept")
eq(Cartfile.from_cart("m.cart", "-- menu: no\n", LIBS).menu, nil, "only false opts out")
local MMANI = Cartfile.manifest_text(MSPEC)
eq(has(MMANI, "menu = false\n"), true, "the manifest carries menu = false")
eq(MMANI:find("menu = false", 1, true) < MMANI:find("source = cart", 1, true), true, "before source = cart")
eq(has(BARE, "menu"), false, "no menu line means no menu line in the manifest")

-- -------------------------------------------------------- replaceable

group("replaceable: what a cart may overwrite")
eq(Cartfile.replaceable({ name = "Acid Snake", source = "cart" }), true,
   "a previously installed cart may be replaced")
eq(Cartfile.replaceable({ name = "Desktop" }), false,
   "a built-in app with no source marker is protected")
eq(Cartfile.replaceable({ name = "X", source = "builtin" }), false,
   "any other source value is protected")
eq(Cartfile.replaceable({}), false, "an unreadable/empty manifest is protected")
eq(Cartfile.replaceable(nil), true, "nothing there at all is a fresh install")

-- ------------------------------------------------------------ name_clash
--
-- Slot protection is about files; this is about NAMES. Terminal's `run`
-- resolves an app by the first case-insensitive name match in registry
-- order, and a cart picks its own filename (hence its sort position), so
-- a cart calling itself "Editor" from an unused slot would answer to
-- `run editor` before the real Editor does. Nothing here blocks that --
-- the confirm screen just has to be able to say so.

local REGISTRY = { { "v3/apps/editor.lua", "Editor" }, { "v3/apps/tetris.lua", "Tetris" } }

group("name_clash: spotting a name already in use")
eq(Cartfile.name_clash("Editor", "v3/apps/aaa.lua", REGISTRY), "v3/apps/editor.lua",
   "a cart taking a built-in's name reports the app it collides with")
eq(Cartfile.name_clash("editor", "v3/apps/aaa.lua", REGISTRY), "v3/apps/editor.lua",
   "the comparison ignores case, exactly as Terminal's lookup does")
eq(Cartfile.name_clash("Acid Snake", "v3/apps/acid_snake.lua", REGISTRY), nil,
   "an unused name is free")
eq(Cartfile.name_clash("Editor", "v3/apps/editor.lua", REGISTRY), nil,
   "an app does not collide with itself -- reinstalling a cart keeps its own name")
eq(Cartfile.name_clash("Editor", "v3/apps/aaa.lua", {}), nil,
   "an empty registry collides with nothing")
eq(Cartfile.name_clash("Editor", { "v3/apps/editor.wasm", "v3/apps/editor.lua" }, REGISTRY), nil,
   "every path listed as the cart's own is itself, not a clash")
eq(Cartfile.name_clash("Tetris", { "v3/apps/editor.wasm", "v3/apps/editor.lua" }, REGISTRY), "v3/apps/tetris.lua",
   "a list of own paths still spots a clash with another app")

-- --------------------------------------------------- destination_status
--
-- What the confirm screen is allowed to offer for a slug, given what is
-- already sitting in that slot in v3/apps.

group("destination_status: an empty slot")
eq(Cartfile.destination_status(false, nil), "fresh", "nothing there at all installs cleanly")

group("destination_status: a slot this loader filled before")
eq(Cartfile.destination_status(true, { name = "Acid Snake", source = "cart" }), "replace",
   "a previously installed cart offers a replace")

group("destination_status: slots that must never be written")
eq(Cartfile.destination_status(true, { name = "Tetris" }), "protected",
   "a built-in app with a manifest is protected")
eq(Cartfile.destination_status(true, nil), "protected",
   "a bare .lua with no manifest at all -- desktop.lua -- is protected, not mistaken for empty")
eq(Cartfile.destination_status(false, { name = "Ghost" }), "protected",
   "a manifest with no .lua beside it is still someone else's slot")
eq(Cartfile.destination_status(true, { name = "X", source = "builtin" }), "protected",
   "any other source marker is protected")

-- ----------------------------------------------------------- size_ok

group("size_ok: refusing outsized carts")
eq(Cartfile.size_ok(1024), true, "an ordinary cart fits")
eq(Cartfile.size_ok(0), false, "an empty file is refused")
eq(Cartfile.size_ok(Cartfile.MAX_BYTES), true, "exactly the cap fits")
eq(Cartfile.size_ok(Cartfile.MAX_BYTES + 1), false, "one byte over is refused")

-- --------------------------------------------------- browse containment

group("parent_dir: .. stops dead at the root it started in")
eq(Cartfile.parent_dir("/media/usb/games", "/media"), "/media/usb", "one level up inside the root")
eq(Cartfile.parent_dir("/media/usb", "/media"), "/media", "up to the root itself")
eq(Cartfile.parent_dir("/media", "/media"), nil, "the root has no parent to offer")
eq(Cartfile.parent_dir("v3/carts", "v3/carts"), nil, "a relative root behaves the same")
eq(Cartfile.parent_dir("v3/carts/demo", "v3/carts"), "v3/carts", "a relative root walks back to itself")

group("child_dir: descending")
eq(Cartfile.child_dir("/media", "usb"), "/media/usb", "joins one path component")
eq(Cartfile.child_dir("/media/", "usb"), "/media/usb", "a trailing slash does not double up")
eq(Cartfile.child_dir("/media", ".."), nil, "'..' is never a descendable name")
eq(Cartfile.child_dir("/media", "."), nil, "'.' is never a descendable name")
eq(Cartfile.child_dir("/media", "a/b"), nil, "a name with a separator is refused")
eq(Cartfile.child_dir("/media", ""), nil, "an empty name is refused")

-- ------------------------------------------------------------ from_cart
--
-- The whole validated install spec the UI acts on -- everything above,
-- composed.

group("from_cart: a well-formed cart")
local SPEC = Cartfile.from_cart("Acid Snake.cart", HEADER, LIBS)
eq(SPEC.slug, "acid_snake", "slug comes from the filename, not the header")
eq(SPEC.name, "Acid Snake", "name comes from the header")
eq(SPEC.w, 160, "width is parsed")
eq(SPEC.h, 140, "height is parsed")
eq(SPEC.libs, { "lib/acid_game.lua", "lib/acid_keys.lua" }, "modules are filtered")
eq(SPEC.script_path, "v3/apps/acid_snake.lua", "destination source path is inside the apps dir")
eq(SPEC.toml_path, "v3/apps/acid_snake.app.toml", "destination manifest sits beside it")

group("from_cart: a cart with no header at all")
local BARESPEC = Cartfile.from_cart("mystery.cart", "local x = 1\n", LIBS)
eq(BARESPEC.name, "Mystery", "the name falls back to the filename, title-cased")
eq(BARESPEC.w, Cartfile.DEFAULT_W, "the width falls back to the default")
eq(BARESPEC.h, Cartfile.DEFAULT_H, "the height falls back to the default")
eq(BARESPEC.libs, {}, "no modules are loaded by default")

group("from_cart: names that cannot install")
eq(Cartfile.from_cart("..cart", "local x = 1\n", LIBS), nil, "an unusable filename yields no spec")

group("from_cart: the header cannot reach outside the apps dir")
local EVIL = Cartfile.from_cart("../../../../etc/cron.cart",
                                "-- name: ../../evil\n-- libs: ../../../etc/passwd.lua\n", LIBS)
eq(EVIL.script_path, "v3/apps/cron.lua", "a traversal filename still lands in the apps dir")
eq(EVIL.libs, {}, "a traversal libs entry is dropped")
eq(EVIL.name, "../../evil", "the name is only ever display text, so it is kept verbatim")

-- ------------------------------------------------------------ .wasm carts
--
-- Spec §15.4: a .wasm cart carries its header in a custom section named
-- "acid" instead of a leading comment block, and installs as
-- <slug>.wasm with `runtime = wasm` in its manifest.

-- LEB128 u32, as the WASM binary format writes section sizes.
local function leb(n)
  local out = ""
  repeat
    local b = n % 128
    n = n // 128
    if n > 0 then b = b + 128 end
    out = out .. string.char(b)
  until n == 0
  return out
end

local WASM_HEAD = "\0asm\1\0\0\0"
local function custom(name, payload)
  local body = leb(#name) .. name .. payload
  return "\0" .. leb(#body) .. body
end
local ACID_PAYLOAD = "name: Wasm Thing\nw: 300\nh: 100\n"
local MODULE = WASM_HEAD .. custom("acid", ACID_PAYLOAD)

group("wasm_header: the acid custom section")
eq(Cartfile.wasm_header(MODULE), { name = "Wasm Thing", w = "300", h = "100" }, "the acid section's lines are parsed")
eq(Cartfile.wasm_header(WASM_HEAD), {}, "a module with no sections has no header")
eq(Cartfile.wasm_header("\0asx\1\0\0\0" .. custom("acid", ACID_PAYLOAD)), {}, "bad magic gives nothing")
eq(Cartfile.wasm_header(WASM_HEAD .. "\0\x80"), {}, "a truncated section size gives nothing")
eq(Cartfile.wasm_header(WASM_HEAD .. "\0\x40abc"), {}, "a size past the end gives nothing")
eq(Cartfile.wasm_header(WASM_HEAD .. custom("other", ACID_PAYLOAD)), {}, "a custom section with another name is ignored")
eq(Cartfile.wasm_header(WASM_HEAD .. custom("other", "name: No\n") .. custom("acid", ACID_PAYLOAD)),
   { name = "Wasm Thing", w = "300", h = "100" }, "the acid section is found after an earlier one")
eq(Cartfile.wasm_header(WASM_HEAD .. custom("acid", "name: A\nname: B\nbogus: x\n")), { name = "A" },
   "first value wins and unknown keys are dropped, as in parse_header")

eq(Cartfile.wasm_header(MODULE .. "\0\x40abc"), {}, "a malformed tail after the acid section voids the header")

group("wasm: cart, slug, from_cart, manifest_text")
eq(Cartfile.cart("x.wasm"), true, "a .wasm file is offered")
eq(Cartfile.cart("X.WASM"), true, "case-insensitively")
eq(Cartfile.slug("My Thing.wasm"), "my_thing", "the .wasm suffix is stripped from the slug")
local WSPEC = Cartfile.from_cart("thing.wasm", MODULE, LIBS)
eq(WSPEC.script_path, "v3/apps/thing.wasm", "a wasm cart installs as <slug>.wasm")
eq(WSPEC.toml_path, "v3/apps/thing.app.toml", "with its manifest beside it")
eq(WSPEC.runtime, "wasm", "its runtime is wasm")
eq(WSPEC.libs, {}, "it takes no Lua modules")
eq(WSPEC.w, 300, "the header's width is used")
eq(WSPEC.name, "Wasm Thing", "and its name")
eq(Cartfile.manifest_text(WSPEC), "name = Wasm Thing\nw = 300\nh = 100\nruntime = wasm\nsource = cart\n",
   "runtime = wasm sits just before the final source = cart")
eq(SPEC.runtime, "lua", "a .cart has the lua runtime")
local GWASM = Cartfile.from_cart("g.wasm", WASM_HEAD .. custom("acid", "name: G\ncategory: game\n"), LIBS)
eq(GWASM.category, "game", "a wasm cart's acid section can say category: game")
local MWASM = Cartfile.from_cart("m.wasm", WASM_HEAD .. custom("acid", "name: M\nmenu: false\n"), LIBS)
eq(Cartfile.manifest_text(MWASM), "name = M\nw = 220\nh = 160\nmenu = false\nruntime = wasm\nsource = cart\n",
   "a wasm cart's acid section can say menu: false")
