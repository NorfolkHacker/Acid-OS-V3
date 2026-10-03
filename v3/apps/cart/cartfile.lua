-- Everything the cart loader decides BEFORE it touches a file -- pure
-- string work, no acid_* bindings, no IO, so tools/test_cart.lua can run
-- the whole lot headless (same split as editor/buffer.lua).
--
-- A .cart is just Lua source on a host card. Installing one means writing
-- it into v3/apps as <slug>.lua plus a generated <slug>.app.toml, which is
-- exactly the point where a hostile (or merely careless) cart could try to
-- reach somewhere it shouldn't. That's what this module is: the filename
-- becomes a slug that CANNOT contain a path, the header's `libs` is an
-- allow-list membership test rather than a path, the window size is
-- clamped to the real screen, and every header value has its control
-- characters stripped so a value can never inject a second manifest line.
--
-- What it deliberately does NOT do is trust the cart's code. An installed
-- cart runs in its own Lua state with the cart-level limits (16 MB, a
-- 1000 ms "stopped responding" limit, writes only under Home); the checks
-- here protect the OS's own files from a malicious NAME, not from a
-- malicious PROGRAM. The kernel decides a cart's trust from the manifest's
-- `source = cart` line, which is why manifest_text keeps it LAST: the
-- LATER key wins when a manifest is parsed.
--
-- A global table whose functions are called with `.`.
Cartfile = {}

Cartfile.CART_EXT = ".cart"
-- Spec §15.4: a WebAssembly cart. Its header is a custom section instead
-- of a comment block, and it installs as <slug>.wasm.
Cartfile.WASM_EXT = ".wasm"

-- Where an installed cart lands -- the same directory desktop.lua scans
-- for <name>.app.toml manifests at boot, which is what makes a cart
-- show up in the Menu on the next one.
Cartfile.APPS_DIR = "v3/apps"

-- Refuse anything that isn't plausibly a hand-written program. 256 KB is
-- far past any app in v3/apps and still small enough to read into a
-- string comfortably.
Cartfile.MAX_BYTES = 256 * 1024

Cartfile.MAX_SLUG = 24    -- destination filename length, sans extension
Cartfile.MAX_TEXT = 40    -- longest header value kept (name, desc)
Cartfile.MAX_LIBS = 8     -- most modules one cart may pull in

-- Window bounds: the 640x360 screen less the desktop strip. A cart
-- asking for something outside this gets clamped rather than refused -- a
-- bad number is a typo, not an attack, but an unclamped one is a window
-- nobody can reach the title bar of.
Cartfile.MIN_W = 80
Cartfile.MAX_W = 640
Cartfile.MIN_H = 48
Cartfile.MAX_H = 336
Cartfile.DEFAULT_W = 220
Cartfile.DEFAULT_H = 160

-- The only header keys that mean anything. Anything else in the comment
-- block is a comment, including keys we might add later -- an unknown
-- key never reaches the generated manifest.
Cartfile.HEADER_KEYS = { "name", "w", "h", "desc", "libs" }

local function includes(list, value)
  for _, v in ipairs(list) do
    if v == value then return true end
  end
  return false
end

-- Trims leading and trailing whitespace.
local function strip(s)
  return s:match("^%s*(.-)%s*$")
end

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

-- The 0-based index of the last "/", or nil.
local function rindex_slash(s)
  for i = #s, 1, -1 do
    if s:sub(i, i) == "/" then return i - 1 end
  end
  return nil
end

-- True for host files the browser should even offer. ".cart" on its own
-- has no stem to make a filename out of, so it isn't one. ".wasm" carts
-- are offered the same way (spec §15.4).
function Cartfile.cart(name)
  if name == nil then return false end
  if Cartfile.cart_ext(name) then return #name > #Cartfile.CART_EXT end
  if Cartfile.wasm_ext(name) then return #name > #Cartfile.WASM_EXT end
  return false
end

local function has_ext(name, ext)
  if name == nil or #name < #ext then return false end
  return name:sub(#name - #ext + 1, #name):lower() == ext
end

function Cartfile.cart_ext(name)
  return has_ext(name, Cartfile.CART_EXT)
end

function Cartfile.wasm_ext(name)
  return has_ext(name, Cartfile.WASM_EXT)
end

-- The destination filename, from the host file's name. This is the
-- security-critical one, so it works by construction rather than by
-- blacklist: only [a-z0-9_] survive, everything else collapses to a
-- single underscore, so there is no input -- "../../desktop.cart",
-- "/etc/passwd.cart", a name with an embedded newline -- that can
-- produce a slug containing a path separator, a traversal component or
-- a line break. nil means "no usable name", which the caller reports
-- rather than guessing at.
function Cartfile.slug(filename)
  if filename == nil then return nil end
  local base = Cartfile.basename(filename)
  if Cartfile.cart_ext(base) then
    base = base:sub(1, #base - #Cartfile.CART_EXT)
  elseif Cartfile.wasm_ext(base) then
    base = base:sub(1, #base - #Cartfile.WASM_EXT)
  end

  local out = ""
  local underscore = false
  local i = 0
  while i < #base do
    local ch = base:sub(i + 1, i + 1):lower()
    if (ch >= "a" and ch <= "z") or (ch >= "0" and ch <= "9") then
      out = out .. ch
      underscore = false
    elseif not underscore then
      out = out .. "_"
      underscore = true
    end
    i = i + 1
  end

  out = Cartfile.trim_underscores(out)
  if #out == 0 then return nil end
  -- Truncating can expose a trailing underscore that wasn't trailing
  -- before, so trim again rather than installing "my_long_name_".
  local trimmed = Cartfile.trim_underscores(out:sub(1, Cartfile.MAX_SLUG))
  if #trimmed == 0 then return nil end
  return trimmed
end

function Cartfile.basename(path)
  local idx = rindex_slash(path)
  if idx == nil then return path end
  return path:sub(idx + 2, #path)
end

function Cartfile.trim_underscores(s)
  while #s > 0 and s:sub(1, 1) == "_" do s = s:sub(2, #s) end
  while #s > 0 and s:sub(#s, #s) == "_" do s = s:sub(1, #s - 1) end
  return s
end

-- The one filter every header goes through, whichever container it came
-- from: only HEADER_KEYS count, the value is cleaned, a blank value is
-- dropped, and the first value for a key wins.
local function accept_header_field(fields, key, raw_value)
  if includes(Cartfile.HEADER_KEYS, key) and not fields[key] then
    local value = Cartfile.clean_text(raw_value)
    if #value > 0 then fields[key] = value end
  end
end

-- The leading `-- key: value` comment block. Reading stops at the first
-- line that isn't blank and isn't a comment -- i.e. at the cart's first
-- line of real code -- so a `-- name:` further down (in a comment inside
-- the program, say) is never mistaken for metadata. First value wins.
function Cartfile.parse_header(text)
  local fields = {}
  if text == nil then return fields end
  for _, raw in ipairs(split_lines(text)) do
    local line = strip(raw)
    if #line > 0 then
      if line:sub(1, 2) ~= "--" then break end
      local body = strip(line:sub(3, #line))
      local colon = body:find(":", 1, true)
      if colon ~= nil then
        colon = colon - 1  -- 0-based index
        local key = strip(body:sub(1, colon)):lower()
        accept_header_field(fields, key, body:sub(colon + 2, #body))
      end
    end
  end
  return fields
end

-- Header values end up as manifest lines and as on-screen text, so
-- anything outside printable ASCII becomes a space: that's what stops a
-- value carrying a newline (which would inject a whole extra manifest
-- key) or a control code the 8x8 font has no glyph for.
function Cartfile.clean_text(s)
  local out = {}
  for i = 1, #s do
    local b = s:byte(i)
    if b < 32 or b > 126 then
      out[i] = " "
    else
      out[i] = string.char(b)
    end
  end
  return strip(table.concat(out)):sub(1, Cartfile.MAX_TEXT)
end

-- LEB128 u32 at `pos` (1-based): 7 bits per byte, low first, high bit
-- meaning "more", at most 5 bytes. Returns value, next_pos, or nil for a
-- truncated or over-long encoding.
local function read_leb(bytes, pos)
  local result, shift = 0, 0
  for _ = 1, 5 do
    local b = bytes:byte(pos)
    if b == nil then return nil end
    pos = pos + 1
    result = result | ((b & 0x7f) << shift)
    if b < 0x80 then
      if result > 0xffffffff then return nil end
      return result, pos
    end
    shift = shift + 7
  end
  return nil
end

-- A WASM cart's header (spec §15.4): the payload of the custom section
-- named "acid", whose `key: value` lines go through the same filter as a
-- .cart's comment block. Anything malformed -- bad magic or version, a
-- truncated LEB128, a section running past the end -- or a module with no
-- such section gives {}, never an error: a hostile module can only lose
-- its metadata.
function Cartfile.wasm_header(bytes)
  if bytes == nil or bytes:sub(1, 8) ~= "\0asm\1\0\0\0" then return {} end
  local fields = {}
  local found = false
  local pos = 9
  -- Walks to the very end: a malformed tail after the acid section voids
  -- the header too (spec §15.4, "malformed -> {}").
  while pos <= #bytes do
    local id = bytes:byte(pos)
    local size, body = read_leb(bytes, pos + 1)
    if size == nil then return {} end
    local last = body + size - 1
    if last > #bytes then return {} end
    if id == 0 then
      local name_len, name_pos = read_leb(bytes, body)
      if name_len == nil or name_pos + name_len - 1 > last then return {} end
      if not found and bytes:sub(name_pos, name_pos + name_len - 1) == "acid" then
        found = true
        local payload = bytes:sub(name_pos + name_len, last)
        for _, raw in ipairs(split_lines(payload)) do
          local line = strip(raw)
          local colon = line:find(":", 1, true)
          if colon ~= nil then
            local key = strip(line:sub(1, colon - 1)):lower()
            accept_header_field(fields, key, line:sub(colon + 1, #line))
          end
        end
      end
    end
    pos = last + 1
  end
  return fields
end

-- A cart's requested window edge, clamped into what the screen can
-- actually show. A missing/blank value takes the caller's default; a
-- non-numeric one reads as 0 and clamps up to the minimum, so the
-- result is never a zero-size window.
function Cartfile.dimension(raw, fallback, min, max)
  if raw == nil or #strip(raw) == 0 then return fallback end
  local v = to_i(raw)
  if v < min then return min end
  if v > max then return max end
  return v
end

-- Modules the cart asked for, kept only if they're in `available` (the
-- real contents of apps/lib, passed in by the caller). Membership, not
-- path validation: "../../../etc/passwd.lua" isn't in the list, so it's
-- dropped without this needing to reason about traversal at all. The
-- loaded-module list is what the loader feeds to the VM, so this is the
-- one header field with teeth.
function Cartfile.filter_libs(raw, available)
  if raw == nil then return {} end
  local out = {}
  for entry in (raw .. ","):gmatch("([^,]*),") do
    local name = strip(entry)
    if #name > 0 and includes(available, name) and not includes(out, name) then
      out[#out + 1] = name
      if #out >= Cartfile.MAX_LIBS then break end
    end
  end
  return out
end

-- nil formats as "".
local function str(v)
  if v == nil then return "" end
  return tostring(v)
end

-- The generated <slug>.app.toml. `source = cart` is load-bearing: it's
-- how a later install knows this slot was installed from a cart and may
-- be replaced, how every hand-written app in v3/apps is recognised as
-- off-limits (see replaceable), and how the kernel marks the app as
-- cart-level. It stays the LAST line, since a later key wins when a
-- manifest is parsed.
function Cartfile.manifest_text(fields)
  local lines = {}
  lines[#lines + 1] = "name = " .. str(fields.name)
  lines[#lines + 1] = "w = " .. str(fields.w)
  lines[#lines + 1] = "h = " .. str(fields.h)
  local desc = fields.desc
  if desc and #desc > 0 then lines[#lines + 1] = "desc = " .. desc end
  local libs = fields.libs
  if libs and #libs > 0 then lines[#lines + 1] = "libs = " .. table.concat(libs, ", ") end
  -- Spec §15.4: tells the launchers to run <stem>.wasm, not <stem>.lua.
  -- Before `source = cart`, which must stay last (later key wins).
  if fields.runtime == "wasm" then lines[#lines + 1] = "runtime = wasm" end
  lines[#lines + 1] = "source = cart"
  return table.concat(lines, "\n") .. "\n"
end

-- Whether a cart may write over the manifest already in that slot. nil
-- (nothing there) is a fresh install; anything without `source = cart`
-- is a built-in app and is refused outright rather than confirmed --
-- a cart called "desktop.cart" must not be able to replace the desktop,
-- even by a user tapping through a prompt.
function Cartfile.replaceable(fields)
  if fields == nil then return true end
  return fields["source"] == "cart"
end

-- What may be done to the <slug> slot in v3/apps, given whether a
-- script (<slug>.lua or <slug>.wasm) is already there and whatever
-- <slug>.app.toml parsed to
-- (nil = no manifest). Three answers, and the caller only ever offers
-- an install for the first two.
--
-- The bare-.lua case is the one worth spelling out: apps/desktop.lua has
-- no manifest of its own (the desktop isn't launchable from the Menu it
-- draws), so a manifest lookup alone would read that slot as empty and
-- happily let a cart called "desktop.cart" overwrite the desktop. An
-- existing .lua we didn't install is protected whether or not anything
-- documents it.
function Cartfile.destination_status(script_exists, manifest_fields)
  if not script_exists and manifest_fields == nil then return "fresh" end
  if script_exists and Cartfile.replaceable(manifest_fields) and manifest_fields ~= nil then
    return "replace"
  end
  return "protected"
end

-- The already-registered app a cart's name would collide with, or nil.
-- `entries` is {{path, name}, ...} straight from the launcher registry.
--
-- This is deliberately only a warning's worth of information, not a
-- refusal: two apps may legitimately want the same display name, and
-- the files are protected either way (see destination_status). What it
-- prevents is the quiet case -- a cart installing to a free slot under
-- a built-in's name and, because Terminal's `run` takes the first
-- case-insensitive match in registry order and a cart picks its own
-- filename, answering to that name first. An app never collides with
-- itself, so replacing a cart in place is not a clash. `script_path` is
-- the cart's own path, or a list of every path that counts as the cart
-- (a slot's .lua and .wasm, when one runtime replaces the other).
function Cartfile.name_clash(name, script_path, entries)
  local own = script_path
  if type(own) ~= "table" then own = { own } end
  local function is_self(path)
    for _, p in ipairs(own) do if p == path then return true end end
    return false
  end
  local target = name:lower()
  for _, entry in ipairs(entries) do
    if not is_self(entry[1]) then
      if entry[2] and entry[2]:lower() == target then return entry[1] end
    end
  end
  return nil
end

function Cartfile.size_ok(bytes)
  return bytes > 0 and bytes <= Cartfile.MAX_BYTES
end

-- Where ".." goes from `dir`, or nil if that would leave `root`. The
-- browser only ever moves by parent_dir/child_dir, so the reachable set
-- is exactly the tree under one configured cart root.
function Cartfile.parent_dir(dir, root)
  if dir == root then return nil end
  local idx = rindex_slash(dir)
  if idx == nil then return nil end
  local parent
  if idx == 0 then parent = "/" else parent = dir:sub(1, idx) end
  local prefix = root .. "/"
  if not (parent == root or parent:sub(1, #prefix) == prefix) then return nil end
  return parent
end

-- Where a listed entry name goes from `dir`. Names come from a directory
-- listing, so "." and ".." are the expected junk; a name with a separator
-- in it shouldn't be possible at all, and is refused rather than joined.
function Cartfile.child_dir(dir, name)
  if name == nil or #name == 0 then return nil end
  if name == "." or name == ".." then return nil end
  if name:find("/", 1, true) then return nil end
  local base = dir
  if dir:sub(#dir, #dir) == "/" then base = dir:sub(1, #dir - 1) end
  return base .. "/" .. name
end

-- Everything above, composed: the install the UI shows on its confirm
-- screen and then carries out. nil means the filename yielded no usable
-- slug, the one case with nothing sensible to install as.
function Cartfile.from_cart(filename, bytes, available_libs)
  local s = Cartfile.slug(filename)
  if s == nil then return nil end
  -- A .wasm keeps its header in a custom section and takes no Lua
  -- modules; it is always cart-level (spec §15.4).
  local wasm = Cartfile.wasm_ext(Cartfile.basename(filename))
  local fields
  if wasm then fields = Cartfile.wasm_header(bytes) else fields = Cartfile.parse_header(bytes) end
  return {
    slug = s,
    name = fields["name"] or Cartfile.default_name(s),
    w = Cartfile.dimension(fields["w"], Cartfile.DEFAULT_W, Cartfile.MIN_W, Cartfile.MAX_W),
    h = Cartfile.dimension(fields["h"], Cartfile.DEFAULT_H, Cartfile.MIN_H, Cartfile.MAX_H),
    desc = fields["desc"] or "",
    libs = wasm and {} or Cartfile.filter_libs(fields["libs"], available_libs),
    runtime = wasm and "wasm" or "lua",
    script_path = Cartfile.APPS_DIR .. "/" .. s .. (wasm and Cartfile.WASM_EXT or ".lua"),
    toml_path = Cartfile.APPS_DIR .. "/" .. s .. ".app.toml",
  }
end

-- "acid_snake" -> "Acid Snake", for a cart that shipped no `-- name:`.
function Cartfile.default_name(s)
  local words = {}
  for word in (s .. "_"):gmatch("([^_]*)_") do
    if #word > 0 then word = word:sub(1, 1):upper() .. word:sub(2, #word) end
    words[#words + 1] = word
  end
  -- Drop trailing empties.
  while #words > 0 and words[#words] == "" do words[#words] = nil end
  return table.concat(words, " ")
end
