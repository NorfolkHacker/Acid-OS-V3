# Sprite Paint Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bring back v1's PAINT as **Sprite Paint**, a resizable Lua app for pixel-art and animated sprites. It saves text `.spr` files that any app can load with `AcidSprite.load`.

**Architecture:**
- The `.spr` format code goes in the shared `apps/lib/acid_sprite.lua`, so the editor and games use the same parser.
- The app is `apps/sprite.lua`, with four pure modules in `apps/sprite/`:
  - `doc.lua`: the data and the undo history;
  - `tools.lua`: what the tools do to the doc;
  - `layout.lua`: every rectangle;
  - `picker.lua`: the colours the picker offers.
- File Manager opens `.spr` files in it.
- There are no kernel, API or wasm changes.

**Tech Stack:** Lua 5.4 apps (mlua). Headless Lua suites run by `v3/crates/acid-lua/tests/game_tests.rs`. Rust golden-frame tests in `v3/crates/acid-os/tests`.

**Spec:** `docs/superpowers/specs/2026-10-04-sprite-paint-design.md`

## Global Constraints

- **Repo:** all paths are relative to `/home/norfolkh/acid-os-v3`. Cargo commands run from `v3/`.
- **What changes:** no changes to any `src/` in `v3/crates`. Only these change:
  - `v3/apps`;
  - `v3/fsroot/Home`;
  - `v3/tools`;
  - test files under `v3/crates/*/tests`;
  - `docs/manual-v3`.
- **Lua environment:** the headless Lua state has only the `string`, `table` and `math` libraries, with no `io` or `os`. App code must not use either.
- **Assertion counts:** every Lua suite's count is pinned in `game_tests.rs`. The counts below were measured by running this plan's exact code. If your run passes everything but reports a different count, you changed something: re-check the transcription before touching the count.
- **Font:** Sprite Paint is **not** `font = scalable`. Its text is always 6×8.
- **Goldens:** never create, overwrite or commit a golden (`v3/crates/acid-os/tests/golden/*.ppm`) yourself. Produce the `v3/target/actual-*.ppm` frame and stop. The user approves every golden as a PNG first.
- **Commits:** every commit message ends with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- **Style:** match the surrounding code's comment density and idiom, as in `apps/file_manager.lua` and `apps/editor/*.lua`.

## File structure

| File | Responsibility |
|---|---|
| `v3/apps/lib/acid_sprite.lua` (modify) | Add `parse`, `serialize` and `load` for `.spr`, plus the format constants. |
| `v3/fsroot/Home/acid_ship.spr` (new) | The sample sprite: 16×16, 2 frames. |
| `v3/apps/sprite/doc.lua` (new) | `SpriteDoc`: the cells, frames, palette, undo/redo and the dirty flag. |
| `v3/apps/sprite/tools.lua` (new) | `SpriteTools`: Bresenham lines, paint/mirror, stroke, flood fill, pick. |
| `v3/apps/sprite/layout.lua` (new) | `SpriteLayout`: every rectangle, plus hit-testing. |
| `v3/apps/sprite/picker.lua` (new) | `SpritePicker`: the 56 picker colours and their cells. |
| `v3/apps/sprite.lua` and `sprite.app.toml` (new) | The app. |
| `v3/apps/file_manager.lua` (modify) | `.spr` rows open in Sprite Paint. |
| `v3/tools/test_*.lua` and `sprite_test_helpers.lua` (new) | The suites. |
| `v3/crates/acid-lua/tests/game_tests.rs` (modify) | Register the suites. |
| `v3/crates/acid-os/tests/games.rs` and `golden.rs` (modify) | The boot smoke test and the golden test. |
| `docs/manual-v3/01`, `04`, `09` (modify) | Docs. |

---

### Task 1: The `.spr` format, the sample sprite, and the shipped-files check

**Files:**
- Modify: `v3/apps/lib/acid_sprite.lua` (append)
- Create: `v3/tools/test_acid_sprite_format.lua`, `v3/tools/test_sprite_files.lua`, `v3/fsroot/Home/acid_ship.spr`
- Modify: `v3/crates/acid-lua/tests/game_tests.rs`

**Interfaces:**
- Produces:
  - `AcidSprite.parse(text) -> sprite | nil, "line N: reason"`;
  - `AcidSprite.serialize(sprite) -> text`;
  - `AcidSprite.load(path) -> sprite | nil, msg`;
  - the constants `AcidSprite.MAX_SIZE` (32), `MAX_FRAMES` (8), `MAX_FPS` (30), `DEFAULT_FPS` (6), and `KEYS` (`"0123456789abcdef"`).
- The sprite table is `{ w, h, fps, palette = { [key] = 0xRRGGBB }, frames = { { row strings } } }`.

- [ ] **Step 1: Write the failing suite.** Create `v3/tools/test_acid_sprite_format.lua`:

````lua
-- Headless tests for the .spr format: AcidSprite.parse, serialize and load
-- (apps/lib/acid_sprite.lua).

local function lines(t) return table.concat(t, "\n") .. "\n" end
local function rejects(text, want, what)
  local s, err = AcidSprite.parse(text)
  eq({ s, err }, { nil, want }, what)
end
local function frames(count)
  local t = { "acid-sprite 1", "size 1 1" }
  for _ = 1, count do t[#t + 1] = "frame"; t[#t + 1] = "." end
  return lines(t)
end

local MIN = lines({ "acid-sprite 1", "size 2 1", "pal 1 ff00ff", "frame", "1." })

group("parse: accepts")
local s, err = AcidSprite.parse(MIN)
eq(err, nil, "a minimal file parses")
eq(s, { w = 2, h = 1, fps = 6, palette = { ["1"] = 0xff00ff }, frames = { { "1." } } }, "the minimal file's table")
s = AcidSprite.parse(lines({ "# a comment", "", "acid-sprite 1", "size 1 1", "fps 12", "pal a 00FFcc", "", "frame", "# between", "a", "frame", "." }))
eq(s and s.fps, 12, "fps is read")
eq(s and s.palette.a, 0x00ffcc, "hex digits may be either case")
eq(s and s.frames, { { "a" }, { "." } }, "comments and blank lines are skipped anywhere")
s = AcidSprite.parse("acid-sprite 1\r\nsize 1 1\r\nframe\r\n.\r\n")
eq(s and s.frames, { { "." } }, "CRLF line endings")
s = AcidSprite.parse(lines({ "acid-sprite 1", "size 1 1", "frame", "." }))
eq(s and s.palette, {}, "a palette is optional")
local big = { "acid-sprite 1", "size 32 32", "frame" }
for _ = 1, 32 do big[#big + 1] = string.rep(".", 32) end
s = AcidSprite.parse(lines(big))
eq(s and { s.w, s.h, #s.frames[1] }, { 32, 32, 32 }, "32x32 is the largest size")
s = AcidSprite.parse(frames(8))
eq(s and #s.frames, 8, "8 frames is the most")
local sixteen = { "acid-sprite 1", "size 1 1" }
for i = 1, 16 do sixteen[#sixteen + 1] = "pal " .. AcidSprite.KEYS:sub(i, i) .. " 000000" end
sixteen[#sixteen + 1] = "frame"
sixteen[#sixteen + 1] = "f"
s = AcidSprite.parse(lines(sixteen))
eq(s and s.frames[1][1], "f", "16 palette entries")

group("parse: rejects, naming the line")
rejects(nil, "line 0: not text", "something that isn't text")
rejects("", "line 1: missing 'acid-sprite 1'", "an empty file")
rejects(lines({ "acid-sprite 2" }), "line 1: expected 'acid-sprite 1'", "a wrong header")
rejects(lines({ "acid-sprite 1" }), "line 1: missing size", "no size")
rejects(lines({ "acid-sprite 1", "size 1 1" }), "line 2: no frames", "no frames")
rejects(lines({ "acid-sprite 1", "frame" }), "line 2: frame before size", "a frame before the size")
rejects(lines({ "acid-sprite 1", "size 33 1" }), "line 2: size must be two numbers from 1 to 32", "width 33")
rejects(lines({ "acid-sprite 1", "size 1 0" }), "line 2: size must be two numbers from 1 to 32", "height 0")
rejects(lines({ "acid-sprite 1", "size 4" }), "line 2: size must be two numbers from 1 to 32", "one number")
rejects(lines({ "acid-sprite 1", "size 1 1", "size 1 1" }), "line 3: size given twice", "size twice")
rejects(lines({ "acid-sprite 1", "size 1 1", "fps 0" }), "line 3: fps must be from 1 to 30", "fps 0")
rejects(lines({ "acid-sprite 1", "size 1 1", "fps 31" }), "line 3: fps must be from 1 to 30", "fps 31")
rejects(lines({ "acid-sprite 1", "fps 6", "fps 6" }), "line 3: fps given twice", "fps twice")
rejects(lines({ "acid-sprite 1", "size 1 1", "pal g 000000" }), "line 3: palette key must be one of 0-9 a-f", "key g")
rejects(lines({ "acid-sprite 1", "size 1 1", "pal A 000000" }), "line 3: palette key must be one of 0-9 a-f", "an uppercase key")
rejects(lines({ "acid-sprite 1", "size 1 1", "pal 1 12345" }), "line 3: colour must be 6 hex digits", "5 hex digits")
rejects(lines({ "acid-sprite 1", "size 1 1", "pal 1 12345g" }), "line 3: colour must be 6 hex digits", "a non-hex digit")
local seventeen = { table.unpack(sixteen, 1, 18) }
seventeen[#seventeen + 1] = "pal 0 ffffff"
rejects(lines(seventeen), "line 19: palette key '0' given twice", "a 17th palette entry")
rejects(lines({ "acid-sprite 1", "size 1 1", "frame", ".", "pal 1 000000" }), "line 5: pal after frame", "pal after a frame")
rejects(lines({ "acid-sprite 1", "size 2 1", "frame", "." }), "line 4: row is 1 wide, expected 2", "a short row")
rejects(lines({ "acid-sprite 1", "size 1 1", "frame", "1" }), "line 4: '1' is not a palette key", "an undeclared key")
rejects(lines({ "acid-sprite 1", "size 1 1", "bogus" }), "line 3: unknown line 'bogus'", "an unknown line")
rejects(lines({ "acid-sprite 1", "size 1 2", "frame", "." }), "line 4: frame 1 has 1 of 2 rows", "a frame missing a row")
rejects(frames(9), "line 19: more than 8 frames", "9 frames")

group("serialize")
local text = lines({ "acid-sprite 1", "# drawn by hand", "size 2 1", "pal b 00FFcc", "pal 1 FF00FF", "frame", "1b" })
eq(AcidSprite.serialize(AcidSprite.parse(text)),
  lines({ "acid-sprite 1", "size 2 1", "fps 6", "pal 1 ff00ff", "pal b 00ffcc", "frame", "1b" }),
  "fixed order, palette sorted by key, lowercase hex, no comments")
local once = AcidSprite.serialize(AcidSprite.parse(text))
eq(AcidSprite.serialize(AcidSprite.parse(once)), once, "serialize(parse(t)) is a fixed point")
eq(AcidSprite.parse(once), AcidSprite.parse(text), "and parses to the same table")

group("load")
FS["v3/fsroot/Home/min.spr"] = MIN
eq(AcidSprite.load("v3/fsroot/Home/min.spr"), AcidSprite.parse(MIN), "load reads and parses a file")
eq({ AcidSprite.load("v3/fsroot/Home/none.spr") }, { nil, "not found" }, "a missing file is nil and the read error")
FS["v3/fsroot/Home/bad.spr"] = "nope\n"
eq({ AcidSprite.load("v3/fsroot/Home/bad.spr") }, { nil, "line 1: expected 'acid-sprite 1'" }, "a bad file is nil and the parse error")
````

Register it in `game_tests.rs`, after the existing `acid_sprite` test:

````rust
#[test]
fn acid_sprite_format() {
    run_suite(&["v3/tools/game_test_env.lua", "v3/apps/lib/acid_sprite.lua", "v3/tools/test_acid_sprite_format.lua"], 40);
}
````

- [ ] **Step 2: Run it and watch it fail.**
  - Run: `cargo test -p acid-lua --test game_tests acid_sprite_format`
  - Expected: FAIL, with an error calling the nil field `parse`.

- [ ] **Step 3: Implement.** Append this to the end of `v3/apps/lib/acid_sprite.lua`, leaving the existing functions unchanged:

````lua

-- Sprite files (.spr): the text format Sprite Paint saves and games load.
-- See docs/manual-v3/04-graphics.md §4.6. parse never raises: a bad file
-- returns nil and "line N: reason".
AcidSprite.MAX_SIZE = 32
AcidSprite.MAX_FRAMES = 8
AcidSprite.MAX_FPS = 30
AcidSprite.DEFAULT_FPS = 6
AcidSprite.KEYS = "0123456789abcdef"

-- The text's lines, without the empty one after a final newline, and with
-- any trailing \r removed.
local function split_lines(text)
  local list = {}
  for line in (text .. "\n"):gmatch("(.-)\n") do list[#list + 1] = (line:gsub("\r$", "")) end
  if #list > 1 and list[#list] == "" and text:sub(-1) == "\n" then list[#list] = nil end
  return list
end

local function words(line)
  local out = {}
  for w in line:gmatch("%S+") do out[#out + 1] = w end
  return out
end

-- A decimal integer from lo to hi, or nil.
local function int_in(s, lo, hi)
  if s == nil or not s:match("^%d+$") or #s > 3 then return nil end
  local n = tonumber(s)
  if n < lo or n > hi then return nil end
  return n
end

function AcidSprite.parse(text)
  if type(text) ~= "string" then return nil, "line 0: not text" end
  local s = { palette = {}, frames = {} }
  local header, fps, rows = false, nil, nil
  local list = split_lines(text)
  local n = 0
  local function fail(msg) return nil, "line " .. n .. ": " .. msg end
  for i, line in ipairs(list) do
    n = i
    if line == "" or line:sub(1, 1) == "#" then
      -- skipped
    elseif not header then
      if line ~= "acid-sprite 1" then return fail("expected 'acid-sprite 1'") end
      header = true
    elseif rows and #rows < s.h then
      if #line ~= s.w then return fail("row is " .. #line .. " wide, expected " .. s.w) end
      for c = 1, #line do
        local ch = line:sub(c, c)
        if ch ~= "." and s.palette[ch] == nil then return fail("'" .. ch .. "' is not a palette key") end
      end
      rows[#rows + 1] = line
    else
      local w = words(line)
      local cmd = w[1]
      if cmd == "size" then
        if s.w then return fail("size given twice") end
        local sw, sh = int_in(w[2], 1, AcidSprite.MAX_SIZE), int_in(w[3], 1, AcidSprite.MAX_SIZE)
        if #w ~= 3 or not sw or not sh then return fail("size must be two numbers from 1 to 32") end
        s.w, s.h = sw, sh
      elseif cmd == "fps" then
        if fps then return fail("fps given twice") end
        fps = int_in(w[2], 1, AcidSprite.MAX_FPS)
        if #w ~= 2 or not fps then return fail("fps must be from 1 to 30") end
      elseif cmd == "pal" then
        if #s.frames > 0 then return fail("pal after frame") end
        local key, hex = w[2], w[3]
        if #w ~= 3 or #key ~= 1 or not AcidSprite.KEYS:find(key, 1, true) then
          return fail("palette key must be one of 0-9 a-f")
        end
        if s.palette[key] then return fail("palette key '" .. key .. "' given twice") end
        if not hex:match("^%x%x%x%x%x%x$") then return fail("colour must be 6 hex digits") end
        s.palette[key] = tonumber(hex, 16)
      elseif cmd == "frame" and #w == 1 then
        if not s.w then return fail("frame before size") end
        if #s.frames == AcidSprite.MAX_FRAMES then return fail("more than 8 frames") end
        rows = {}
        s.frames[#s.frames + 1] = rows
      else
        return fail("unknown line '" .. line .. "'")
      end
    end
  end
  n = math.max(#list, 1)
  if not header then return fail("missing 'acid-sprite 1'") end
  if not s.w then return fail("missing size") end
  if #s.frames == 0 then return fail("no frames") end
  if #rows < s.h then return fail("frame " .. #s.frames .. " has " .. #rows .. " of " .. s.h .. " rows") end
  s.fps = fps or AcidSprite.DEFAULT_FPS
  return s
end

-- The text for a parsed (or edited) sprite: header, size, fps, the palette
-- in key order with lowercase hex, then the frames. No comments or blank
-- lines, and a final newline.
function AcidSprite.serialize(s)
  local out = { "acid-sprite 1", "size " .. s.w .. " " .. s.h, "fps " .. s.fps }
  for i = 1, #AcidSprite.KEYS do
    local k = AcidSprite.KEYS:sub(i, i)
    if s.palette[k] then out[#out + 1] = string.format("pal %s %06x", k, s.palette[k]) end
  end
  for _, rows in ipairs(s.frames) do
    out[#out + 1] = "frame"
    for _, r in ipairs(rows) do out[#out + 1] = r end
  end
  return table.concat(out, "\n") .. "\n"
end

-- Reads and parses a .spr file. Returns the sprite, or nil and a message.
function AcidSprite.load(path)
  local text, err = acid_fs_read(path)
  if not text then return nil, err or "read failed" end
  return AcidSprite.parse(text)
end
````

- [ ] **Step 4: Run it and watch it pass.**
  - Run: `cargo test -p acid-lua --test game_tests acid_sprite`
  - Expected: both `acid_sprite` (13) and `acid_sprite_format` (40) PASS.

- [ ] **Step 5: Add the sample sprite and the shipped-files check.** Create `v3/fsroot/Home/acid_ship.spr` with exactly this content:

````text
# A sample sprite: open it in Sprite Paint, or load it with AcidSprite.load.
acid-sprite 1
size 16 16
fps 6
pal 0 000000
pal 1 ff0000
pal 2 ff7f00
pal 3 ffff00
pal 4 80ff00
pal 5 00ff00
pal 6 00ff7f
pal 7 00ffff
pal 8 0080ff
pal 9 0000ff
pal a 7f00ff
pal b ff00ff
pal c ff0080
pal d 555555
pal e aaaaaa
pal f ffffff
frame
.......ff.......
......feef......
......e77e......
.....ee77ee.....
.....eeddee.....
....beeddeeb....
...bbeeddeebb...
..bbbeeddeebbb..
.bbaaeeddeeaabb.
bbaa.eeddee.aabb
ba...eeeeee...ab
b....dd..dd....b
.....33..33.....
.....21..12.....
......1..1......
................
frame
.......ff.......
......feef......
......e77e......
.....ee77ee.....
.....eeddee.....
....beeddeeb....
...bbeeddeebb...
..bbbeeddeebbb..
.bbaaeeddeeaabb.
bbaa.eeddee.aabb
ba...eeeeee...ab
b....dd..dd....b
.....33..33.....
.....32..23.....
.....21..12.....
......1..1......
````

Create `v3/tools/test_sprite_files.lua`:

````lua
-- Every .spr shipped in v3/fsroot/Home parses. SPR is filled in by
-- game_tests.rs: { { name, text }, ... }.
for _, f in ipairs(SPR) do
  local s, err = AcidSprite.parse(f[2])
  ok(s ~= nil, f[1] .. " parses" .. (err and (": " .. err) or ""))
end
````

Add this to `game_tests.rs`:

````rust
#[test]
fn shipped_sprite_files_parse() {
    // Each file goes to Lua as a long-bracket string; the first newline
    // after [==[ is dropped by Lua, so the text arrives unchanged.
    let mut prelude = String::from("SPR = {}\n");
    let mut count = 0;
    for entry in std::fs::read_dir(repo_root().join("v3/fsroot/Home")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "spr") {
            let text = std::fs::read_to_string(&path).unwrap();
            assert!(!text.contains("]==]"), "{} can't be quoted", path.display());
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            prelude.push_str(&format!("SPR[#SPR + 1] = {{ {name:?}, [==[\n{text}]==] }}\n"));
            count += 1;
        }
    }
    assert!(count >= 1, "acid_ship.spr is shipped");
    run_suite_with(&prelude, &["v3/tools/game_test_env.lua", "v3/apps/lib/acid_sprite.lua", "v3/tools/test_sprite_files.lua"], count);
}
````

- [ ] **Step 6: Run it.**
  - Run: `cargo test -p acid-lua --test game_tests sprite`
  - Expected: `acid_sprite`, `acid_sprite_format` and `shipped_sprite_files_parse` PASS.
  - Sanity check: temporarily change `size 16 16` in the sample to `size 16 17`. The test must then fail. Revert the change.

- [ ] **Step 7: Commit.**

````bash
git add v3/apps/lib/acid_sprite.lua v3/tools/test_acid_sprite_format.lua v3/tools/test_sprite_files.lua v3/fsroot/Home/acid_ship.spr v3/crates/acid-lua/tests/game_tests.rs
git commit -m "AcidSprite: .spr sprite files (parse, serialize, load) and a sample ship

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
````

---

### Task 2: SpriteDoc (the data and the undo history)

**Files:**
- Create: `v3/apps/sprite/doc.lua`, `v3/tools/test_sprite_doc.lua`
- Modify: `v3/crates/acid-lua/tests/game_tests.rs`

**Interfaces:**
- Consumes: `AcidSprite.KEYS`, `MAX_FRAMES`, `MAX_FPS`, `DEFAULT_FPS` (Task 1), and `AcidPalette.hue` (a core lib).
- Produces `SpriteDoc`:
  - constructors `SpriteDoc.new(sprite)`, `SpriteDoc.blank(w, h)`, `SpriteDoc.default_palette()`;
  - fields `doc.s` (the sprite table), `doc.frame` (1-based) and `doc.dirty`;
  - cells: `doc:rows()`, `doc:frame_count()`, `doc:get(x, y)`, `doc:set(x, y, key)` (x and y are 0-based);
  - redraw tracking: `doc:take_changes() -> { {x, y}, ... }`, `doc:take_full() -> bool`;
  - frames: `doc:frame_next()`, `doc:frame_prev()`, and `doc:frame_add()`, `doc:frame_dup()`, `doc:frame_del()`, which return a bool;
  - `doc:set_color(key, rgb)` and `doc:set_fps(n)`;
  - undo: `doc:begin_step()`, `doc:end_step() -> bool`, `doc:undo() -> bool`, `doc:redo() -> bool`;
  - `doc:mark_saved()`.

- [ ] **Step 1: Write the failing suite.** Create `v3/tools/test_sprite_doc.lua`:

````lua
-- Headless tests for SpriteDoc (apps/sprite/doc.lua).

group("SpriteDoc: blank and palette")
local d = SpriteDoc.blank(4, 3)
eq({ d.s.w, d.s.h, d.s.fps, d:frame_count(), d.frame }, { 4, 3, 6, 1, 1 }, "a blank doc is 4x3, 6 fps, one frame")
eq(d:rows(), { "....", "....", "...." }, "a blank frame is all transparent")
eq({ d.s.palette["0"], d.s.palette["1"], d.s.palette["5"], d.s.palette.f }, { 0x000000, 0xFF0000, 0x00FF00, 0xFFFFFF },
  "default palette: black, the hue wheel, white")
local n = 0
for _ in pairs(d.s.palette) do n = n + 1 end
eq(n, 16, "the default palette has 16 entries")
local loaded = SpriteDoc.new({ w = 1, h = 1, fps = 6, palette = { ["1"] = 0x123456 }, frames = { { "1" } } })
eq({ loaded.s.palette["1"], loaded.s.palette.f }, { 0x123456, 0xFFFFFF }, "a loaded palette keeps its colours and gains the missing defaults")
eq(loaded.dirty, false, "a new doc is not dirty")

group("SpriteDoc: get and set")
d:set(1, 2, "3")
eq(d:get(1, 2), "3", "set then get")
eq(d:rows()[3], ".3..", "set edits one character of the row")
eq(d:take_changes(), { { 1, 2 } }, "the change is recorded")
eq(d:take_changes(), {}, "taking clears the changes")
d:set(1, 2, "3")
eq(d:take_changes(), {}, "setting the same key changes nothing")
d:set(-1, 0, "3"); d:set(4, 0, "3"); d:set(0, 3, "3")
eq(d:take_changes(), {}, "sets off the sprite are ignored")
eq({ d:get(-1, 0), d:get(4, 0) }, {}, "gets off the sprite are nil")

group("SpriteDoc: frames")
d = SpriteDoc.blank(2, 1)
d:set(0, 0, "1")
eq(d:frame_dup(), true, "dup a frame")
eq({ d:frame_count(), d.frame, d:rows()[1] }, { 2, 2, "1." }, "the copy follows the original and is current")
eq(d:frame_add(), true, "add a frame")
eq({ d:frame_count(), d.frame, d:rows()[1] }, { 3, 3, ".." }, "the blank frame goes after the current one")
d:frame_next()
eq(d.frame, 1, "next wraps to the first frame")
d:frame_prev()
eq(d.frame, 3, "prev wraps to the last frame")
for _ = 1, 5 do d:frame_add() end
eq({ d:frame_count(), d:frame_add(), d:frame_dup() }, { 8, false, false }, "8 frames is the limit")
eq(SpriteDoc.blank(1, 1):frame_del(), false, "the only frame can't be deleted")
d.frame = 8
eq(d:frame_del(), true, "delete a frame")
eq({ d:frame_count(), d.frame }, { 7, 7 }, "deleting the last frame steps back one")
local e = SpriteDoc.blank(1, 1)
e:frame_dup()
e:set(0, 0, "2")
eq({ e.s.frames[1][1], e.s.frames[2][1] }, { ".", "2" }, "a duplicated frame is independent")

group("SpriteDoc: undo and redo")
local u = SpriteDoc.blank(2, 1)
u:begin_step(); u:set(0, 0, "1")
eq(u:end_step(), true, "an action that changed something is a step")
u:begin_step()
eq(u:end_step(), false, "an action that changed nothing is not")
eq(u.dirty, true, "a step makes the doc dirty")
u:mark_saved()
eq(u.dirty, false, "mark_saved clears dirty")
u:begin_step(); u:set(1, 0, "2"); u:end_step()
eq(u:undo(), true, "undo")
eq(u:rows()[1], "1.", "undo goes back one step")
eq(u:take_full(), true, "undo asks for a full redraw")
eq(u:undo(), true, "undo again")
eq(u:rows()[1], "..", "back to blank")
eq(u:undo(), false, "nothing more to undo")
eq(u:redo(), true, "redo")
eq(u:rows()[1], "1.", "redo reapplies")
u:begin_step(); u:set(1, 0, "3"); u:end_step()
eq(u:redo(), false, "a new step clears redo")
u:frame_add()
eq(u:frame_count(), 2, "frame add")
u:undo()
eq({ u:frame_count(), u.frame }, { 1, 1 }, "undo removes the added frame")
u:set_color("1", 0x010203)
u:undo()
eq(u.s.palette["1"], 0xFF0000, "undo restores a palette colour")
u:set_fps(99)
eq(u.s.fps, 30, "fps clamps to 30")
u:set_fps(0)
eq(u.s.fps, 1, "fps clamps to 1")
local cap = SpriteDoc.blank(1, 1)
for i = 1, 40 do cap:begin_step(); cap:set(0, 0, i % 2 == 0 and "1" or "2"); cap:end_step() end
local undos = 0
while cap:undo() do undos = undos + 1 end
eq(undos, 32, "the history keeps 32 steps")
````

Register it:

````rust
#[test]
fn sprite_doc() {
    run_suite(&with_libs(&["v3/tools/game_test_env.lua", "v3/apps/lib/acid_sprite.lua", "v3/apps/sprite/doc.lua", "v3/tools/test_sprite_doc.lua"]), 43);
}
````

- [ ] **Step 2: Run it and watch it fail.**
  - Run: `cargo test -p acid-lua --test game_tests sprite_doc`
  - Expected: FAIL, with `v3/apps/sprite/doc.lua: No such file`.

- [ ] **Step 3: Implement.** Create `v3/apps/sprite/doc.lua`:

````lua
-- SpriteDoc: the sprite Sprite Paint is editing, which frame is current,
-- and the undo history. Pure data: no drawing and no file I/O.
-- The sprite is the table AcidSprite.parse returns. Cell coordinates are
-- 0-based; frames and doc.frame are 1-based like Lua lists.
SpriteDoc = {}
SpriteDoc.__index = SpriteDoc
SpriteDoc.HISTORY = 32

-- 0 black, 1-9 and a-c twelve steps round the hue wheel, d/e greys, f white.
function SpriteDoc.default_palette()
  local p = { ["0"] = 0x000000, d = 0x555555, e = 0xAAAAAA, f = 0xFFFFFF }
  for i = 1, 12 do
    p[AcidSprite.KEYS:sub(i + 1, i + 1)] = AcidPalette.hue(i - 1, 12)
  end
  return p
end

local function blank_rows(w, h)
  local rows = {}
  for y = 1, h do rows[y] = string.rep(".", w) end
  return rows
end

local function copy_list(t)
  local c = {}
  for i, v in ipairs(t) do c[i] = v end
  return c
end

local function copy_map(t)
  local c = {}
  for k, v in pairs(t) do c[k] = v end
  return c
end

-- Wraps a parsed sprite. Palette keys the file left out get their default
-- colour, so all 16 swatches always have one (and are all saved).
function SpriteDoc.new(s)
  for k, v in pairs(SpriteDoc.default_palette()) do
    if s.palette[k] == nil then s.palette[k] = v end
  end
  return setmetatable({
    s = s, frame = 1, undo_stack = {}, redo_stack = {},
    dirty = false, changed = {}, full = true,
  }, SpriteDoc)
end

function SpriteDoc.blank(w, h)
  return SpriteDoc.new({ w = w, h = h, fps = AcidSprite.DEFAULT_FPS, palette = {}, frames = { blank_rows(w, h) } })
end

function SpriteDoc:rows() return self.s.frames[self.frame] end
function SpriteDoc:frame_count() return #self.s.frames end

-- The key at a cell of the current frame, or nil off the sprite.
function SpriteDoc:get(x, y)
  if x < 0 or y < 0 or x >= self.s.w or y >= self.s.h then return nil end
  return self:rows()[y + 1]:sub(x + 1, x + 1)
end

-- Sets a cell of the current frame; off the sprite, or no change, does
-- nothing. A real change is recorded for take_changes.
function SpriteDoc:set(x, y, key)
  local old = self:get(x, y)
  if old == nil or old == key then return end
  local rows = self:rows()
  local row = rows[y + 1]
  rows[y + 1] = row:sub(1, x) .. key .. row:sub(x + 2)
  self.changed[#self.changed + 1] = { x, y }
end

-- The cells changed since the last call, as {x, y} pairs.
function SpriteDoc:take_changes()
  local c = self.changed
  self.changed = {}
  return c
end

-- Whether something changed that needs the whole canvas redrawn (a frame
-- switch, a palette edit, undo/redo) since the last call.
function SpriteDoc:take_full()
  local f = self.full
  self.full = false
  return f
end

function SpriteDoc:frame_next()
  self.frame = self.frame % #self.s.frames + 1
  self.full = true
end

function SpriteDoc:frame_prev()
  self.frame = (self.frame - 2) % #self.s.frames + 1
  self.full = true
end

-- Undo snapshots hold every frame's row list (the row strings are shared,
-- not copied), the current frame, the palette and fps.
function SpriteDoc:snapshot()
  local frames = {}
  for i, rows in ipairs(self.s.frames) do frames[i] = copy_list(rows) end
  return { frames = frames, frame = self.frame, palette = copy_map(self.s.palette), fps = self.s.fps }
end

local function same(a, b)
  if a.fps ~= b.fps or #a.frames ~= #b.frames then return false end
  for i, rows in ipairs(a.frames) do
    for y, r in ipairs(rows) do
      if b.frames[i][y] ~= r then return false end
    end
  end
  for k, v in pairs(a.palette) do
    if b.palette[k] ~= v then return false end
  end
  for k in pairs(b.palette) do
    if a.palette[k] == nil then return false end
  end
  return true
end

-- Puts a snapshot back. The snapshot is off its stack by now, so its
-- tables are taken over rather than copied.
function SpriteDoc:restore(snap)
  self.s.frames = snap.frames
  self.s.palette = snap.palette
  self.s.fps = snap.fps
  self.frame = snap.frame
  self.changed = {}
  self.full = true
  self.dirty = true
end

local function push_capped(stack, snap)
  stack[#stack + 1] = snap
  if #stack > SpriteDoc.HISTORY then table.remove(stack, 1) end
end

-- begin_step/end_step bracket one undoable action. end_step returns
-- whether it was a step: an action that changed nothing isn't one.
function SpriteDoc:begin_step()
  self.pending = self:snapshot()
end

function SpriteDoc:end_step()
  local before = self.pending
  self.pending = nil
  if before == nil or same(before, self:snapshot()) then return false end
  push_capped(self.undo_stack, before)
  self.redo_stack = {}
  self.dirty = true
  return true
end

function SpriteDoc:undo()
  local snap = table.remove(self.undo_stack)
  if not snap then return false end
  push_capped(self.redo_stack, self:snapshot())
  self:restore(snap)
  return true
end

function SpriteDoc:redo()
  local snap = table.remove(self.redo_stack)
  if not snap then return false end
  push_capped(self.undo_stack, self:snapshot())
  self:restore(snap)
  return true
end

-- Frame edits are each one step, and refuse past the limits.
function SpriteDoc:frame_add()
  if #self.s.frames >= AcidSprite.MAX_FRAMES then return false end
  self:begin_step()
  table.insert(self.s.frames, self.frame + 1, blank_rows(self.s.w, self.s.h))
  self.frame = self.frame + 1
  self.full = true
  return self:end_step()
end

function SpriteDoc:frame_dup()
  if #self.s.frames >= AcidSprite.MAX_FRAMES then return false end
  self:begin_step()
  table.insert(self.s.frames, self.frame + 1, copy_list(self:rows()))
  self.frame = self.frame + 1
  self.full = true
  return self:end_step()
end

function SpriteDoc:frame_del()
  if #self.s.frames <= 1 then return false end
  self:begin_step()
  table.remove(self.s.frames, self.frame)
  if self.frame > #self.s.frames then self.frame = #self.s.frames end
  self.full = true
  return self:end_step()
end

function SpriteDoc:set_color(key, rgb)
  self:begin_step()
  self.s.palette[key] = rgb
  self.full = true
  return self:end_step()
end

function SpriteDoc:set_fps(n)
  self:begin_step()
  self.s.fps = math.max(1, math.min(AcidSprite.MAX_FPS, n))
  return self:end_step()
end

function SpriteDoc:mark_saved()
  self.dirty = false
end
````

- [ ] **Step 4: Run it and watch it pass.**
  - Run: `cargo test -p acid-lua --test game_tests sprite_doc`
  - Expected: PASS (43).

- [ ] **Step 5: Commit.**

````bash
git add v3/apps/sprite/doc.lua v3/tools/test_sprite_doc.lua v3/crates/acid-lua/tests/game_tests.rs
git commit -m "Sprite Paint: SpriteDoc, the sprite being edited and its undo history

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
````

---

### Task 3: SpriteTools

**Files:**
- Create: `v3/apps/sprite/tools.lua`, `v3/tools/test_sprite_tools.lua`
- Modify: `v3/crates/acid-lua/tests/game_tests.rs`

**Interfaces:**
- Consumes `SpriteDoc` (Task 2).
- Produces:
  - `SpriteTools.line_cells(x0, y0, x1, y1) -> { {x, y}, ... }`;
  - `SpriteTools.paint(doc, x, y, key, mirror)`;
  - `SpriteTools.stroke(doc, x0, y0, x1, y1, key, mirror)`;
  - `SpriteTools.fill(doc, x, y, key)`;
  - `SpriteTools.pick(doc, x, y) -> key | nil`.

- [ ] **Step 1: Write the failing suite.** Create `v3/tools/test_sprite_tools.lua`:

````lua
-- Headless tests for SpriteTools (apps/sprite/tools.lua).

group("SpriteTools: lines in every direction, both ends included")
local L = SpriteTools.line_cells
eq(L(0, 0, 0, 0), { { 0, 0 } }, "a point is one cell")
eq(L(0, 0, 3, 0), { { 0, 0 }, { 1, 0 }, { 2, 0 }, { 3, 0 } }, "east")
eq(L(3, 0, 0, 0), { { 3, 0 }, { 2, 0 }, { 1, 0 }, { 0, 0 } }, "west")
eq(L(0, 0, 0, 2), { { 0, 0 }, { 0, 1 }, { 0, 2 } }, "south")
eq(L(0, 2, 0, 0), { { 0, 2 }, { 0, 1 }, { 0, 0 } }, "north")
eq(L(0, 0, 2, 2), { { 0, 0 }, { 1, 1 }, { 2, 2 } }, "south-east")
eq(L(2, 2, 0, 0), { { 2, 2 }, { 1, 1 }, { 0, 0 } }, "north-west")
eq(L(0, 2, 2, 0), { { 0, 2 }, { 1, 1 }, { 2, 0 } }, "north-east")
eq(L(2, 0, 0, 2), { { 2, 0 }, { 1, 1 }, { 0, 2 } }, "south-west")
eq(L(0, 0, 4, 2), { { 0, 0 }, { 1, 1 }, { 2, 1 }, { 3, 2 }, { 4, 2 } }, "a shallow line steps one cell at a time")

group("SpriteTools: pencil, eraser and mirror")
local d = SpriteDoc.blank(4, 2)
SpriteTools.paint(d, 0, 0, "1", false)
eq(d:rows(), { "1...", "...." }, "paint sets one cell")
SpriteTools.paint(d, 1, 1, "2", true)
eq(d:rows(), { "1...", ".22." }, "mirror also sets the cell mirrored across the centre")
SpriteTools.paint(d, 0, 0, ".", true)
eq(d:rows(), { "....", ".22." }, "the eraser paints '.'")
SpriteTools.stroke(d, 0, 0, 3, 0, "3", false)
eq(d:rows()[1], "3333", "a stroke fills the gap between two samples")
SpriteTools.stroke(d, 0, 1, 1, 1, "4", true)
eq(d:rows()[2], "4444", "a mirrored stroke")

group("SpriteTools: fill")
local f = SpriteDoc.new({ w = 5, h = 3, fps = 6, palette = {}, frames = { { "..1..", "..1..", "11..." } } })
SpriteTools.fill(f, 0, 0, "2")
eq(f:rows(), { "221..", "221..", "11..." }, "fill stops at other keys and doesn't cross diagonals")
SpriteTools.fill(f, 4, 2, "3")
eq(f:rows(), { "22133", "22133", "11333" }, "fill spreads through a '.' region")
f:take_changes()
SpriteTools.fill(f, 2, 0, "1")
eq(f:take_changes(), {}, "filling with the same key changes nothing")
SpriteTools.fill(f, 9, 9, "1")
eq(f:rows()[1], "22133", "a fill off the sprite does nothing")

group("SpriteTools: picker")
eq(SpriteTools.pick(f, 0, 0), "2", "pick returns the key under the cell")
eq(SpriteTools.pick(f, 9, 0), nil, "pick off the sprite is nil")
````

Register it:

````rust
#[test]
fn sprite_tools() {
    run_suite(&with_libs(&["v3/tools/game_test_env.lua", "v3/apps/lib/acid_sprite.lua", "v3/apps/sprite/doc.lua", "v3/apps/sprite/tools.lua", "v3/tools/test_sprite_tools.lua"]), 21);
}
````

- [ ] **Step 2: Run it and watch it fail.**
  - Run: `cargo test -p acid-lua --test game_tests sprite_tools`
  - Expected: FAIL, with a missing-file error.

- [ ] **Step 3: Implement.** Create `v3/apps/sprite/tools.lua`:

````lua
-- SpriteTools: what Sprite Paint's tools do to a SpriteDoc. Pure functions
-- on cell coordinates; the app maps the pointer to cells and brackets each
-- gesture as one undo step.
SpriteTools = {}

-- The cells of a Bresenham line, both ends included, in order from
-- (x0, y0).
function SpriteTools.line_cells(x0, y0, x1, y1)
  local cells = {}
  local dx, dy = math.abs(x1 - x0), -math.abs(y1 - y0)
  local sx = x0 < x1 and 1 or -1
  local sy = y0 < y1 and 1 or -1
  local err = dx + dy
  local x, y = x0, y0
  while true do
    cells[#cells + 1] = { x, y }
    if x == x1 and y == y1 then break end
    local e2 = 2 * err
    if e2 >= dy then err = err + dy; x = x + sx end
    if e2 <= dx then err = err + dx; y = y + sy end
  end
  return cells
end

-- One cell, plus its mirror image across the vertical centre line.
function SpriteTools.paint(doc, x, y, key, mirror)
  doc:set(x, y, key)
  if mirror then doc:set(doc.s.w - 1 - x, y, key) end
end

-- Every cell from (x0, y0) to (x1, y1): Pencil and Eraser join successive
-- pointer samples with this so a fast drag leaves no gaps, and Line
-- commits with it.
function SpriteTools.stroke(doc, x0, y0, x1, y1, key, mirror)
  for _, c in ipairs(SpriteTools.line_cells(x0, y0, x1, y1)) do
    SpriteTools.paint(doc, c[1], c[2], key, mirror)
  end
end

-- 4-way flood fill of the region of (x, y)'s key, transparent included.
function SpriteTools.fill(doc, x, y, key)
  local target = doc:get(x, y)
  if target == nil or target == key then return end
  local stack = { { x, y } }
  while #stack > 0 do
    local c = table.remove(stack)
    local cx, cy = c[1], c[2]
    if doc:get(cx, cy) == target then
      doc:set(cx, cy, key)
      stack[#stack + 1] = { cx + 1, cy }
      stack[#stack + 1] = { cx - 1, cy }
      stack[#stack + 1] = { cx, cy + 1 }
      stack[#stack + 1] = { cx, cy - 1 }
    end
  end
end

function SpriteTools.pick(doc, x, y)
  return doc:get(x, y)
end
````

- [ ] **Step 4: Run it and watch it pass.**
  - Run: `cargo test -p acid-lua --test game_tests sprite_tools`
  - Expected: PASS (21).

- [ ] **Step 5: Commit.**

````bash
git add v3/apps/sprite/tools.lua v3/tools/test_sprite_tools.lua v3/crates/acid-lua/tests/game_tests.rs
git commit -m "Sprite Paint: SpriteTools (line, pencil/mirror, fill, picker)

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
````

---

### Task 4: SpriteLayout and SpritePicker

**Files:**
- Create: `v3/apps/sprite/layout.lua`, `v3/apps/sprite/picker.lua`, `v3/tools/test_sprite_layout.lua`
- Modify: `v3/crates/acid-lua/tests/game_tests.rs`

**Interfaces:**
- Consumes `AcidSprite.KEYS` and `AcidPalette.hue`.
- Produces `SpriteLayout.compute(w, h, sw, sh) -> L`, where `L` has:
  - the window size: `w`, `h`;
  - positions: `col_x`, `bar_y`, `msg_y`, `text_x`, `text_w`, `side_h`;
  - `buttons[i] = { id, label, x, y, w, h }` with ids `pen fill eraser picker line mirror`;
  - `swatches[i] = { key, x, y, w, h }`;
  - previews: `preview1` and `preview2`, each `{ x, y, scale }`. `preview2` may be nil;
  - `picker = { x, y, cw, ch }`;
  - the canvas: `canvas = { x, y, w, h, cell }` and `grid`;
  - `bar[i] = { id, x, y, w, h }` with ids `cmd prev count next add dup del play fps`;
  - `cmds[i] = { id, label, x, y, w, h }` with ids `save saveas new undo redo close`.

  Also produced:
  - the constants `SpriteLayout.TOP`, `COL_W`, `CH_W`, `CH_H` and `CMDS`;
  - `SpriteLayout.find(items, x, y)` and `SpriteLayout.cell_at(L, sw, sh, x, y) -> cx, cy | nil`;
  - `SpritePicker.COUNT` (56), `SpritePicker.shade(rgb, quarters)`, `SpritePicker.color(i)`, `SpritePicker.rect(L, i) -> x, y, w, h`, and `SpritePicker.hit(L, x, y) -> i | nil`.

- [ ] **Step 1: Write the failing suite.** Create `v3/tools/test_sprite_layout.lua`:

````lua
-- Headless tests for SpriteLayout and SpritePicker (apps/sprite/layout.lua,
-- apps/sprite/picker.lua).

local function inside(L, r, what)
  ok(r.x >= 0 and r.y >= 0 and r.x + r.w <= L.w and r.y + r.h <= L.h, what .. " is inside the window")
end
local function all_inside(L, sw, sh, what)
  local rects = {}
  for _, b in ipairs(L.buttons) do rects[#rects + 1] = b end
  for _, s in ipairs(L.swatches) do rects[#rects + 1] = { x = s.x - 1, y = s.y - 1, w = s.w + 2, h = s.h + 2 } end
  for _, b in ipairs(L.bar) do rects[#rects + 1] = b end
  for _, b in ipairs(L.cmds) do rects[#rects + 1] = b end
  rects[#rects + 1] = { x = L.canvas.x, y = L.canvas.y, w = L.canvas.w, h = L.canvas.h }
  rects[#rects + 1] = { x = L.preview1.x, y = L.preview1.y, w = sw, h = sh }
  if L.preview2 then rects[#rects + 1] = { x = L.preview2.x, y = L.preview2.y, w = 2 * sw, h = 2 * sh } end
  rects[#rects + 1] = { x = L.picker.x, y = L.picker.y, w = 12 * L.picker.cw, h = 5 * L.picker.ch }
  for _, r in ipairs(rects) do
    if r.x < 1 or r.y < 16 or r.x + r.w > L.w - 1 or r.y + r.h > L.h - 1 then
      ok(false, what .. ": a rect at " .. r.x .. "," .. r.y .. " size " .. r.w .. "x" .. r.h .. " leaves the user area")
      return
    end
  end
  ok(true, what .. ": every rect is inside the user area")
end

group("SpriteLayout: the default window")
local L = SpriteLayout.compute(360, 260, 16, 16)
eq({ L.col_x, L.msg_y, L.bar_y }, { 255, 239, 249 }, "column, message line and frame bar")
eq({ L.canvas.x, L.canvas.y, L.canvas.cell, L.grid }, { 24, 23, 13, true }, "a 16x16 sprite gets centred 13 px cells")
eq({ L.buttons[1].id, L.buttons[1].x, L.buttons[1].y, L.buttons[6].id, L.buttons[6].x, L.buttons[6].y },
  { "pen", 255, 19, "mirror", 323, 32 }, "two rows of three tool buttons")
eq({ L.swatches[1].key, L.swatches[1].x, L.swatches[1].y, L.swatches[16].key, L.swatches[16].x, L.swatches[16].y },
  { "0", 255, 47, "f", 321, 113 }, "a 4x4 palette under the buttons")
eq({ L.preview1.x, L.preview1.y, L.preview2 and L.preview2.x, L.preview2 and L.preview2.y }, { 255, 136, 291, 136 },
  "1x and 2x previews under the palette")
eq({ L.bar[1].id, L.bar[1].x, L.bar[2].id, L.bar[2].x, L.bar[9].id, L.bar[9].x }, { "cmd", 5, "prev", 29, "fps", 167 },
  "frame bar items at fixed places")
eq({ L.cmds[1].label, L.cmds[2].id, L.cmds[6].id }, { "s:save", "saveas", "close" }, "the command strip")
all_inside(L, 16, 16, "360x260, 16x16")
eq(SpriteLayout.compute(360, 260, 32, 32).canvas.cell, 6, "a 32x32 sprite gets 6 px cells")
all_inside(SpriteLayout.compute(360, 260, 32, 32), 32, 32, "360x260, 32x32")

group("SpriteLayout: the minimum window")
local M = SpriteLayout.compute(280, 200, 32, 32)
eq({ M.canvas.cell, M.grid }, { 4, true }, "32x32 at the minimum still gets 4 px cells and a grid")
eq(M.preview2, nil, "no room for the 2x preview of a 32x32 sprite")
ok(SpriteLayout.compute(280, 200, 16, 16).preview2 ~= nil, "a 16x16 sprite keeps its 2x preview")
all_inside(M, 32, 32, "280x200, 32x32")
all_inside(SpriteLayout.compute(280, 200, 8, 8), 8, 8, "280x200, 8x8")

group("SpriteLayout: hit-testing")
eq({ SpriteLayout.cell_at(L, 16, 16, 24, 23) }, { 0, 0 }, "the canvas's top-left pixel is cell 0,0")
eq({ SpriteLayout.cell_at(L, 16, 16, 24 + 13 * 16 - 1, 23 + 13 * 16 - 1) }, { 15, 15 }, "its bottom-right pixel is 15,15")
eq({ SpriteLayout.cell_at(L, 16, 16, 37, 36) }, { 1, 1 }, "13 px on is the next cell")
eq({ SpriteLayout.cell_at(L, 16, 16, 23, 23) }, {}, "left of the canvas is no cell")
eq({ SpriteLayout.cell_at(L, 16, 16, 24 + 13 * 16, 23) }, {}, "right of the canvas is no cell")
eq(SpriteLayout.find(L.bar, 30, L.bar_y).id, "prev", "a tap on '<' finds prev")
eq(SpriteLayout.find(L.bar, 25, L.bar_y), nil, "a tap between items finds nothing")
eq(SpriteLayout.find(L.swatches, 300, 100).key, "a", "a tap on a swatch finds its key (row 2, column 2)")

group("SpritePicker")
eq({ SpritePicker.color(0), SpritePicker.color(12), SpritePicker.color(36) }, { 0xFF0000, 0xBF0000, 0x3F0000 },
  "red at full, 3/4 and 1/4 brightness")
eq({ SpritePicker.color(48), SpritePicker.color(55) }, { 0x000000, 0xFFFFFF }, "the grey row runs black to white")
eq(SpritePicker.hit(L, L.picker.x, L.picker.y), 0, "the picker's top-left cell is colour 0")
eq(SpritePicker.hit(L, L.picker.x + 7 * 8, L.picker.y + 4 * 12), 55, "the last grey")
eq(SpritePicker.hit(L, L.picker.x + 8 * 8, L.picker.y + 4 * 12), nil, "past the last grey is nothing")
eq({ SpritePicker.rect(L, 13) }, { L.picker.x + 8, L.picker.y + 12, 8, 12 }, "colour 13 is second row, second column")
````

Register it:

````rust
#[test]
fn sprite_layout() {
    run_suite(&with_libs(&["v3/tools/game_test_env.lua", "v3/apps/lib/acid_sprite.lua", "v3/apps/sprite/layout.lua", "v3/apps/sprite/picker.lua", "v3/tools/test_sprite_layout.lua"]), 29);
}
````

- [ ] **Step 2: Run it and watch it fail.**
  - Run: `cargo test -p acid-lua --test game_tests sprite_layout`
  - Expected: FAIL, with a missing-file error.

- [ ] **Step 3: Implement.** Create `v3/apps/sprite/layout.lua`:

````lua
-- SpriteLayout: every rectangle Sprite Paint draws or hit-tests, for a
-- window size and a sprite size. Recomputed on create, resize, new and
-- load. The app is not font-scalable, so text is always 6x8.
SpriteLayout = {}
SpriteLayout.TITLE_H = 16
SpriteLayout.MARGIN = 4
SpriteLayout.COL_W = 100                -- the right-hand column
SpriteLayout.TOP = SpriteLayout.TITLE_H + 3
SpriteLayout.CH_W, SpriteLayout.CH_H = 6, 8
SpriteLayout.BTN_W, SpriteLayout.BTN_H, SpriteLayout.BTN_GAP = 32, 12, 2
SpriteLayout.SWATCH, SpriteLayout.SWATCH_GAP = 20, 2
SpriteLayout.PREVIEW_GAP = 4
SpriteLayout.MAX_PREVIEW_1X = 32     -- the 1x preview's slot: the largest sprite

-- { label, id }, laid out in two rows of three.
SpriteLayout.TOOLS = {
  { "pen", "pen" }, { "fill", "fill" }, { "rub", "eraser" },
  { "pick", "picker" }, { "line", "line" }, { "mir", "mirror" },
}
-- The frame bar: { id, width in characters }. Fixed widths, so nothing
-- moves as the labels change ("8/8", "stop", "30fps" are the widest).
SpriteLayout.BAR = {
  { "cmd", 3 }, { "prev", 1 }, { "count", 3 }, { "next", 1 }, { "add", 1 },
  { "dup", 3 }, { "del", 3 }, { "play", 4 }, { "fps", 5 },
}
-- The command strip: { key, id, label }.
SpriteLayout.CMDS = {
  { "s", "save", "save" }, { "a", "saveas", "as" }, { "n", "new", "new" },
  { "u", "undo", "undo" }, { "r", "redo", "redo" }, { "q", "close", "close" },
}
-- The colour picker: 12 hues by 4 shades, then a row of 8 greys.
SpriteLayout.PICKER_COLS, SpriteLayout.PICKER_ROWS = 12, 5
SpriteLayout.PICKER_CW, SpriteLayout.PICKER_CH = 8, 12

-- Lays text items left to right from x, one character apart.
local function row_items(list, x, y, width_of)
  local items = {}
  for _, e in ipairs(list) do
    local chars = width_of(e)
    items[#items + 1] = { entry = e, x = x, y = y - 1, w = chars * SpriteLayout.CH_W, h = SpriteLayout.CH_H + 2 }
    x = x + (chars + 1) * SpriteLayout.CH_W
  end
  return items
end

function SpriteLayout.compute(w, h, sw, sh)
  local S = SpriteLayout
  local L = { w = w, h = h }
  L.col_x = w - 1 - S.MARGIN - S.COL_W
  L.bar_y = h - 1 - 2 - S.CH_H
  L.msg_y = L.bar_y - 2 - S.CH_H
  L.text_x = 1 + S.MARGIN
  L.text_w = w - 2 * L.text_x

  L.buttons = {}
  for i, t in ipairs(S.TOOLS) do
    local c, r = (i - 1) % 3, (i - 1) // 3
    L.buttons[i] = { id = t[2], label = t[1], w = S.BTN_W, h = S.BTN_H,
      x = L.col_x + c * (S.BTN_W + S.BTN_GAP), y = S.TOP + r * (S.BTN_H + 1) }
  end
  local pal_y = S.TOP + 2 * (S.BTN_H + 1) + 2
  L.swatches = {}
  for i = 0, 15 do
    local c, r = i % 4, i // 4
    L.swatches[i + 1] = { key = AcidSprite.KEYS:sub(i + 1, i + 1), w = S.SWATCH, h = S.SWATCH,
      x = L.col_x + c * (S.SWATCH + S.SWATCH_GAP), y = pal_y + r * (S.SWATCH + S.SWATCH_GAP) }
  end
  local prev_y = pal_y + 4 * (S.SWATCH + S.SWATCH_GAP) + 1
  L.preview1 = { x = L.col_x, y = prev_y, scale = 1 }
  if prev_y + 2 * sh <= L.msg_y - 2 then
    L.preview2 = { x = L.col_x + S.MAX_PREVIEW_1X + S.PREVIEW_GAP, y = prev_y, scale = 2 }
  end
  L.side_h = L.msg_y - 3 - S.TOP

  L.picker = { x = L.col_x + 2, y = S.TOP, cw = S.PICKER_CW, ch = S.PICKER_CH }

  local ax, ay = L.text_x, S.TOP
  local aw, ah = L.col_x - S.MARGIN - ax, L.msg_y - 3 - ay
  local cell = math.max(1, math.min(aw // sw, ah // sh))
  L.canvas = { cell = cell, w = cell * sw, h = cell * sh,
    x = ax + (aw - cell * sw) // 2, y = ay + (ah - cell * sh) // 2 }
  L.grid = cell >= 4

  L.bar = row_items(S.BAR, L.text_x, L.bar_y, function(e) return e[2] end)
  for _, it in ipairs(L.bar) do it.id = it.entry[1] end
  L.cmds = row_items(S.CMDS, L.text_x, L.bar_y, function(e) return #e[1] + 1 + #e[3] end)
  for _, it in ipairs(L.cmds) do it.id, it.label = it.entry[2], it.entry[1] .. ":" .. it.entry[3] end
  return L
end

-- The first item whose rectangle holds (x, y), or nil.
function SpriteLayout.find(items, x, y)
  for _, it in ipairs(items) do
    if x >= it.x and x < it.x + it.w and y >= it.y and y < it.y + it.h then return it end
  end
end

-- The sprite cell under (x, y), or nil off the canvas.
function SpriteLayout.cell_at(L, sw, sh, x, y)
  local c = L.canvas
  if x < c.x or y < c.y then return nil end
  local cx, cy = (x - c.x) // c.cell, (y - c.y) // c.cell
  if cx >= sw or cy >= sh then return nil end
  return cx, cy
end
````

Create `v3/apps/sprite/picker.lua`:

````lua
-- SpritePicker: the colours offered when a palette slot is edited. Rows
-- 0-3 are 12 hue-wheel steps at full, 3/4, 1/2 and 1/4 brightness; row 4
-- is 8 greys from black to white.
SpritePicker = {}
SpritePicker.COUNT = 4 * 12 + 8

function SpritePicker.shade(rgb, quarters)
  local r, g, b = rgb >> 16 & 255, rgb >> 8 & 255, rgb & 255
  return (r * quarters // 4) << 16 | (g * quarters // 4) << 8 | (b * quarters // 4)
end

-- Colour i, 0-based.
function SpritePicker.color(i)
  local col, row = i % 12, i // 12
  if row < 4 then return SpritePicker.shade(AcidPalette.hue(col, 12), 4 - row) end
  local g = col * 255 // 7
  return g << 16 | g << 8 | g
end

function SpritePicker.rect(L, i)
  local p = L.picker
  return p.x + (i % 12) * p.cw, p.y + (i // 12) * p.ch, p.cw, p.ch
end

-- The colour index under (x, y), or nil.
function SpritePicker.hit(L, x, y)
  local p = L.picker
  if x < p.x or y < p.y then return nil end
  local col, row = (x - p.x) // p.cw, (y - p.y) // p.ch
  if col >= 12 or row >= 5 then return nil end
  local i = row * 12 + col
  if i >= SpritePicker.COUNT then return nil end
  return i
end
````

- [ ] **Step 4: Run it and watch it pass.**
  - Run: `cargo test -p acid-lua --test game_tests sprite_layout`
  - Expected: PASS (29).

- [ ] **Step 5: Commit.**

````bash
git add v3/apps/sprite/layout.lua v3/apps/sprite/picker.lua v3/tools/test_sprite_layout.lua v3/crates/acid-lua/tests/game_tests.rs
git commit -m "Sprite Paint: layout and colour picker

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
````

---

### Task 5: The Sprite Paint app

**Files:**
- Create: `v3/apps/sprite.lua`, `v3/apps/sprite.app.toml`, `v3/tools/sprite_test_helpers.lua`, `v3/tools/test_sprite_app.lua`, `v3/tools/test_sprite_app_cmds.lua`
- Modify: `v3/crates/acid-lua/tests/game_tests.rs`, `v3/crates/acid-os/tests/games.rs`

**Interfaces:**
- Consumes everything from Tasks 1–4, plus these system calls:
  - `acid_launch_arg`, `acid_window_size`, `acid_now_ms`;
  - `acid_fs_read` (through `AcidSprite.load`) and `acid_fs_write`;
  - drawing: `acid_fill_rect`, `acid_draw_text`, `acid_clear_user_area`, `acid_draw_window_frame`, `acid_draw_window_border`;
  - `AcidKeys`.
- Produces the `SpritePaintApp` class. The suites read `G.doc`, `G.L`, `G.tool`, `G.key`, `G.mirror`, `G.picker_open`, `G.prompt`, `G.message`, `G.path`, `G.playing` and `G.running`, and call:
  - `G:action(id)`, `G:status_text()`, `G:redraw()`;
  - `G:on_touch`, `G:on_key`, `G:on_idle`, `G:on_create`, `G:poll_timeout_ms()`.

**Notes:**
- In the test env, the `AcidApp` stub has no `redraw`, and `start()` only calls `on_create`. The app therefore draws its own chrome in `redraw()`.
- The window border is drawn **last**, so the golden test (Task 7) can wait for it.

- [ ] **Step 1: Write the manifest.** Create `v3/apps/sprite.app.toml`:

````toml
name = Sprite Paint
w = 360
h = 260
desc = Pixel art and animated sprites
multi = true
libs = lib/acid_sprite.lua, sprite/doc.lua, sprite/tools.lua, sprite/layout.lua, sprite/picker.lua
resizable = true
min_w = 280
min_h = 200
````

- [ ] **Step 2: Write the failing suites.** Create `v3/tools/sprite_test_helpers.lua`:

````lua
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
````

Create `v3/tools/test_sprite_app.lua`:

````lua
-- Sprite Paint (apps/sprite.lua) opened from the Menu: drawing with each
-- tool, the palette and colour picker, held taps, and resizing.
local G = GAME
local blank = string.rep(".", 16)

group("Sprite Paint: from the Menu")
eq({ G.doc.s.w, G.doc.s.h, G.doc:frame_count(), G.key, G.tool, G.path }, { 16, 16, 1, "1", "pen" },
  "a new 16x16 sprite, colour 1, the pencil, no file")
TEXT_AT, RECTS = {}, {}
G:redraw()
sp_fits("the first frame fits the window")
ok(sp_shown("new  untitled"), "the message line says new, untitled")

group("Sprite Paint: pencil, undo, mirror, eraser")
sp_tap_cell(2, 3)
eq(G.doc:get(2, 3), "1", "a pencil tap paints a cell")
eq(G.doc.dirty, true, "and makes the doc dirty")
sp_drag({ { 0, 5 }, { 4, 5 } })
eq(G.doc:rows()[6], "11111" .. string.rep(".", 11), "a fast drag leaves no gaps")
G:action("undo")
eq(G.doc:rows()[6], blank, "one drag is one undo step")
sp_tap(sp_button("mirror"))
eq(G.mirror, true, "the mirror button turns mirror on")
sp_tap_cell(0, 0)
eq(G.doc:rows()[1], "1" .. string.rep(".", 14) .. "1", "mirror paints both sides")
sp_tap(sp_button("mirror"))
sp_tap(sp_button("eraser"))
sp_tap_cell(0, 0)
eq({ G.tool, G.doc:get(0, 0), G.doc:get(15, 0) }, { "eraser", ".", "1" }, "the eraser clears one cell")

group("Sprite Paint: line")
sp_tap(sp_button("line"))
local x0, y0 = sp_cell_xy(0, 8)
local x1, y1 = sp_cell_xy(5, 8)
G:on_touch(x0, y0, true)
G:on_touch(x1, y1, true)
eq(G.doc:rows()[9], blank, "while held, the line is only a preview")
G:on_touch(x1, y1, false)
eq(G.doc:rows()[9], "111111" .. string.rep(".", 10), "the release commits it, both ends included")
G:action("undo")
eq(G.doc:rows()[9], blank, "a line is one undo step")
G:action("redo")

group("Sprite Paint: fill and picker")
sp_tap(sp_swatch("2"))
eq(G.key, "2", "a swatch tap selects its colour")
sp_tap(sp_button("fill"))
sp_tap_cell(15, 15)
eq({ G.doc:get(15, 15), G.doc:get(3, 8), G.doc:get(2, 3) }, { "2", "1", "1" }, "fill covers the open area and stops at drawn cells")
G:action("undo")
sp_tap(sp_button("picker"))
sp_tap_cell(3, 8)
eq(G.key, "1", "the picker picks a cell's colour")
sp_tap_cell(10, 10)
eq(G.tool, "eraser", "picking a transparent cell selects the eraser")

group("Sprite Paint: editing a palette colour")
sp_tap(sp_swatch("3"))
eq(G.picker_open, false, "the first tap only selects")
sp_tap(sp_swatch("3"))
eq(G.picker_open, true, "a second tap on the selected swatch opens the picker")
TEXT_AT, RECTS = {}, {}
G:redraw()
sp_fits("the picker fits the window")
local px, py = SpritePicker.rect(G.L, 12)
sp_tap(px + 1, py + 1)
eq({ G.picker_open, G.doc.s.palette["3"] }, { false, 0xBF0000 }, "choosing a colour sets the slot and closes the picker")
sp_tap(sp_swatch("3"))
G:on_key(AcidKeys.ESCAPE, true)
eq({ G.picker_open, G.doc.s.palette["3"] }, { false, 0xBF0000 }, "ESC closes the picker without a change")
G:action("undo")
eq(G.doc.s.palette["3"], SpriteDoc.default_palette()["3"], "a colour edit is one undo step")

group("Sprite Paint: a held tap acts once")
local bx, by = sp_bar("add")
for _ = 1, 5 do G:on_touch(bx, by, true) end
G:on_touch(bx, by, false)
eq(G.doc:frame_count(), 2, "holding + adds one frame")

group("Sprite Paint: resizing")
resize_app(500, 400)
sp_fits("at 500x400")
eq(G.L.canvas.cell, 22, "the cells grow with the window")
sp_tap(sp_button("pen"))
sp_tap(sp_swatch("4"))
sp_tap_cell(7, 7)
eq(G.doc:get(7, 7), "4", "taps land on the right cell after growing")
resize_app(280, 200)
sp_fits("at the minimum")
sp_tap_cell(15, 15)
eq(G.doc:get(15, 15), "4", "taps land on the right cell at the minimum")
````

Create `v3/tools/test_sprite_app_cmds.lua`:

````lua
-- Sprite Paint (apps/sprite.lua): opening files, saving, new and close,
-- the frame bar, playback and keys.
local G = GAME
local HOME = "v3/fsroot/Home/"
local function writes()
  local n = 0
  for _, c in ipairs(CALLS) do if c[1] == "write" then n = n + 1 end end
  return n
end
local function reopen(arg)
  LAUNCH_ARG = arg
  G:on_create()
  TEXT_AT, RECTS = {}, {}
  G:redraw()
end

group("Sprite Paint: opening a file")
FS[HOME .. "two.spr"] = "acid-sprite 1\nsize 2 1\nfps 4\npal 1 ff00ff\nframe\n1.\nframe\n.1\n"
reopen(HOME .. "two.spr")
eq({ G.doc.s.w, G.doc:frame_count(), G.doc.s.fps, G.path }, { 2, 2, 4, HOME .. "two.spr" }, "the launch path's sprite is open")
ok(sp_shown("saved  two.spr"), "the message line names the file")
sp_fits("a 2x1 sprite fits the window")
FS[HOME .. "bad.spr"] = "junk\n"
reopen(HOME .. "bad.spr")
eq({ G.doc.s.w, G.path }, { 16, nil }, "a bad file opens a new sprite instead")
ok(sp_shown("line 1: expected 'acid-sprite 1'  untitled"), "and shows why")

group("Sprite Paint: save and save-as")
reopen(HOME .. "two.spr")
sp_tap_cell(1, 0)
G:on_key(AcidKeys.ESCAPE, true)
TEXT_AT, RECTS = {}, {}
G:redraw()
sp_fits("the command strip fits the window")
sp_keys("s")
eq(FS[HOME .. "two.spr"], AcidSprite.serialize(G.doc.s), "s saves the serialized sprite to its file")
eq({ G.doc.dirty, G:status_text() }, { false, "saved  two.spr" }, "and marks it saved")
G:on_key(AcidKeys.ESCAPE, true)
sp_keys("a")
sp_keys("hero")
TEXT_AT, RECTS = {}, {}
G:redraw()
ok(sp_shown("save as: hero_"), "save-as prompts for a name")
G:on_key(AcidKeys.ENTER, true)
eq({ FS[HOME .. "hero.spr"], G.path }, { AcidSprite.serialize(G.doc.s), HOME .. "hero.spr" }, "save-as adds .spr and saves under Home")
local before = writes()
G:on_key(AcidKeys.ESCAPE, true)
sp_keys("aa/b")
G:on_key(AcidKeys.ENTER, true)
eq({ writes(), G.message }, { before, "not a file name" }, "a name with a slash is refused")
G:on_key(AcidKeys.ESCAPE, true)
sp_keys("a")
G:on_key(AcidKeys.ENTER, true)
eq({ writes(), G.message }, { before, "not a file name" }, "an empty name is refused")
FAIL_WRITES[HOME .. "hero.spr"] = true
sp_tap(sp_swatch("2"))
sp_tap_cell(0, 0)
G:on_key(AcidKeys.ESCAPE, true)
sp_keys("s")
eq({ G.message, G.doc.dirty }, { "save failed: disk full", true }, "a failed write says so and stays unsaved")
FAIL_WRITES = {}
reopen("")
G:on_key(AcidKeys.ESCAPE, true)
sp_keys("s")
eq(G.prompt and G.prompt.kind, "name", "saving an untitled sprite asks for a name")
G:on_key(AcidKeys.ESCAPE, true)
eq(G.prompt, nil, "ESC cancels the prompt")

group("Sprite Paint: new and close")
sp_tap_cell(0, 0)
G:on_key(AcidKeys.ESCAPE, true)
sp_keys("n")
eq({ G.prompt, G.message }, { nil, "unsaved: new again to discard" }, "new on an unsaved sprite asks to be pressed again")
G:on_key(AcidKeys.ESCAPE, true)
sp_keys("n")
eq(G.prompt and G.prompt.kind, "size", "the second press asks for a size")
sp_keys("3")
eq({ G.doc.s.w, G.doc.s.h, G.L.canvas.cell, G.doc.dirty }, { 32, 32, 6, false }, "3 makes a clean 32x32 sprite")
sp_tap_cell(0, 0)
G.running = true
G:on_key(AcidKeys.ESCAPE, true)
sp_keys("q")
eq(G.running, true, "close on an unsaved sprite asks to be pressed again")
G:on_key(AcidKeys.ESCAPE, true)
sp_keys("q")
eq(G.running, false, "the second press closes")

group("Sprite Paint: frame bar")
reopen(HOME .. "two.spr")
sp_tap(sp_bar("next"))
eq(G.doc.frame, 2, "> goes to the next frame")
sp_tap(sp_bar("prev"))
eq(G.doc.frame, 1, "< goes back")
sp_tap(sp_bar("dup"))
eq({ G.doc:frame_count(), G.doc.frame }, { 3, 2 }, "dup copies the frame")
sp_tap(sp_bar("del"))
eq({ G.doc:frame_count(), G.doc.frame }, { 2, 2 }, "del deletes it and shows the frame after")
sp_tap(sp_bar("fps"))
eq(G.doc.s.fps, 6, "fps steps up 4 -> 6")
TEXT_AT, RECTS = {}, {}
G:redraw()
ok(sp_shown("2/2") and sp_shown("6fps"), "the bar shows the frame and fps")

group("Sprite Paint: playback")
sp_tap(sp_bar("play"))
eq({ G.playing, G:poll_timeout_ms() }, { true, 166 }, "play runs at the sprite's fps")
CLOCK = CLOCK + 166
G:on_idle()
eq(G.doc.frame, 1, "the next frame (wrapping round) shows after 1/fps seconds")
G:action("add")
eq(G.doc:frame_count(), 2, "editing is off while playing")
local before_cell = G.doc:get(0, 0)
sp_tap_cell(0, 0)
eq({ G.playing, G.doc:get(0, 0), G.doc.frame }, { false, before_cell, 1 }, "a canvas tap stops on the frame showing, without painting")
sp_keys(" ")
eq(G.playing, true, "space plays")
sp_keys(" ")
eq(G.playing, false, "space stops")

group("Sprite Paint: keys")
sp_keys("l")
eq(G.tool, "line", "l selects the line")
sp_keys("m")
eq(G.mirror, true, "m toggles mirror")
sp_keys(",")
eq(G.doc.frame, 2, ", goes to the previous frame (wrapping round)")
sp_keys(".")
eq(G.doc.frame, 1, ". goes to the next frame (wrapping round)")
````

Register them in `game_tests.rs`:

````rust
const SPRITE_APP: [&str; 7] = [
    "v3/tools/game_test_env.lua",
    "v3/apps/lib/acid_sprite.lua",
    "v3/apps/sprite/doc.lua",
    "v3/apps/sprite/tools.lua",
    "v3/apps/sprite/layout.lua",
    "v3/apps/sprite/picker.lua",
    "v3/apps/sprite.lua",
];

fn sprite_app_suite(test: &'static str, assertions: usize) {
    let files: Vec<&str> = SPRITE_APP.iter().copied().chain(["v3/tools/sprite_test_helpers.lua", test]).collect();
    run_suite_with("WIN_W, WIN_H = 360, 260", &with_libs(&files), assertions);
}

#[test]
fn sprite_app() {
    sprite_app_suite("v3/tools/test_sprite_app.lua", 29);
}

#[test]
fn sprite_app_commands() {
    sprite_app_suite("v3/tools/test_sprite_app_cmds.lua", 36);
}
````

`with_libs` takes `&[&'static str]`. Every string here is a literal, so the
`Vec<&'static str>` works. If the borrow checker disagrees, make `files` a
`Vec<&'static str>` explicitly.

- [ ] **Step 3: Run them and watch them fail.**
  - Run: `cargo test -p acid-lua --test game_tests sprite_app`
  - Expected: FAIL, with `v3/apps/sprite.lua: No such file`.

- [ ] **Step 4: Implement.** Create `v3/apps/sprite.lua`:

````lua
-- Sprite Paint: pixel art and animated sprites, saved as .spr text files
-- that games load with AcidSprite.load (apps/lib/acid_sprite.lua). The
-- sprite and its undo history live in SpriteDoc, the tools in SpriteTools,
-- every rectangle in SpriteLayout. See
-- docs/superpowers/specs/2026-10-04-sprite-paint-design.md.

SpritePaintApp = AcidApp:extend("SpritePaintApp")
SpritePaintApp.HOME = "v3/fsroot/Home"
SpritePaintApp.NAME_MAX = 24
SpritePaintApp.FPS_STEPS = { 2, 4, 6, 8, 12, 15, 20, 30 }
SpritePaintApp.BG = 0x050607         -- THEME_BG
SpritePaintApp.PANEL = 0x0B1712      -- THEME_PANEL
SpritePaintApp.SEL_BG = 0x123322     -- THEME_PANEL's hover shade
SpritePaintApp.TEXT = 0xD4E6DB       -- THEME_TEXT
SpritePaintApp.MUTED = 0x9DAAA3      -- THEME_MUTED
SpritePaintApp.HARD = 0x00FF66       -- THEME_HARD
SpritePaintApp.GRID = 0x1E2622
SpritePaintApp.CHECK_A = 0x141816    -- transparent cells: a two-tone checker
SpritePaintApp.CHECK_B = 0x22282A

local TOOL_KEYS = { p = "pen", f = "fill", e = "eraser", i = "picker", l = "line", m = "mirror" }

function SpritePaintApp:window_title() return "Sprite Paint" end

function SpritePaintApp:on_create()
  self.tool = "pen"
  self.mirror = false
  self.key = "1"
  self.picker_open = false
  self.cmd_open = false
  self.prompt = nil       -- { kind = "name" | "size", text = "" } while asking
  self.armed = nil        -- "new" / "close" after one press on a dirty doc
  self.playing = false
  self.touch_held = false
  self.gesture = nil
  self.message = nil
  self.path = nil
  local arg = acid_launch_arg()
  if arg ~= nil and arg ~= "" then
    self:open_path(arg)
  else
    self.doc = SpriteDoc.blank(16, 16)
  end
  self:layout(acid_window_size())
end

function SpritePaintApp:layout(w, h)
  self.L = SpriteLayout.compute(w, h, self.doc.s.w, self.doc.s.h)
end

function SpritePaintApp:on_resize(w, h)
  if self.gesture then self:end_gesture() end
  self:layout(w, h)
end

-- Opens a file; a bad one leaves a new sprite and the error showing.
function SpritePaintApp:open_path(path)
  local s, err = AcidSprite.load(path)
  if s then
    self.doc = SpriteDoc.new(s)
    self.path = path
  else
    self.doc = SpriteDoc.blank(16, 16)
    self.path = nil
    self.message = err
  end
end

function SpritePaintApp:new_sprite(size)
  self.doc = SpriteDoc.blank(size, size)
  self.path = nil
  self.playing = false
  self:layout(self.L.w, self.L.h)
  self:redraw()
end

function SpritePaintApp:file_name()
  return self.path and self.path:match("[^/]*$") or "untitled"
end

-- Drawing ----------------------------------------------------------------

function SpritePaintApp:redraw()
  self.doc:take_full()
  self.doc:take_changes()
  acid_clear_user_area()
  acid_draw_window_frame(self:window_title())
  self:draw_canvas()
  self:draw_side()
  self:draw_status()
  self:draw_bar()
  -- Last, so a finished border means a finished frame (the golden test
  -- waits for it).
  acid_draw_window_border()
end

-- Brings the screen up to date after the doc changed: only the changed
-- cells unless the doc asks for a full canvas redraw.
function SpritePaintApp:refresh()
  if self.doc:take_full() then
    self.doc:take_changes()
    self:draw_canvas()
  else
    for _, c in ipairs(self.doc:take_changes()) do self:draw_cell(c[1], c[2]) end
  end
  self:draw_side()
  self:draw_status()
  self:draw_bar()
end

function SpritePaintApp:draw_canvas()
  local c = self.L.canvas
  if self.L.grid then acid_fill_rect(c.x, c.y, c.w, c.h, self.GRID) end
  for y = 0, self.doc.s.h - 1 do
    for x = 0, self.doc.s.w - 1 do self:draw_cell(x, y) end
  end
end

-- One cell, in key's colour (the doc's key if not given). With the grid
-- on, cells are drawn one pixel short so the grid shows between them.
function SpritePaintApp:draw_cell(x, y, key)
  local c = self.L.canvas
  local k = key or self.doc:get(x, y)
  local size = c.cell - (self.L.grid and 1 or 0)
  local color
  if k == "." then
    color = (x + y) % 2 == 0 and self.CHECK_A or self.CHECK_B
  else
    color = self.doc.s.palette[k]
  end
  acid_fill_rect(c.x + x * c.cell, c.y + y * c.cell, size, size, color)
end

-- A frame's rows at a scale, merging runs of one key into one rect.
local function draw_rows(rows, x, y, scale, palette)
  for r = 1, #rows do
    local row = rows[r]
    local c = 1
    while c <= #row do
      local ch = row:sub(c, c)
      local run = 1
      while c + run <= #row and row:sub(c + run, c + run) == ch do run = run + 1 end
      if ch ~= "." then
        acid_fill_rect(x + (c - 1) * scale, y + (r - 1) * scale, run * scale, scale, palette[ch])
      end
      c = c + run
    end
  end
end

function SpritePaintApp:draw_side()
  local L = self.L
  acid_fill_rect(L.col_x, SpriteLayout.TOP, SpriteLayout.COL_W, L.side_h, self.BG)
  if self.picker_open then
    self:draw_picker()
  else
    self:draw_buttons()
    self:draw_palette()
  end
  self:draw_preview()
end

function SpritePaintApp:draw_buttons()
  for _, b in ipairs(self.L.buttons) do
    local on = b.id == self.tool or (b.id == "mirror" and self.mirror)
    local bg = on and self.SEL_BG or self.PANEL
    acid_fill_rect(b.x, b.y, b.w, b.h, bg)
    acid_draw_text(b.label, b.x + (b.w - #b.label * SpriteLayout.CH_W) // 2, b.y + 2, on and self.HARD or self.TEXT, bg)
  end
end

function SpritePaintApp:draw_palette()
  for _, s in ipairs(self.L.swatches) do
    if s.key == self.key then acid_fill_rect(s.x - 1, s.y - 1, s.w + 2, s.h + 2, self.HARD) end
    acid_fill_rect(s.x, s.y, s.w, s.h, self.doc.s.palette[s.key])
  end
end

function SpritePaintApp:draw_picker()
  for i = 0, SpritePicker.COUNT - 1 do
    local x, y, w, h = SpritePicker.rect(self.L, i)
    acid_fill_rect(x, y, w, h, SpritePicker.color(i))
  end
end

function SpritePaintApp:draw_preview()
  local s = self.doc.s
  for _, p in ipairs({ self.L.preview1, self.L.preview2 }) do
    acid_fill_rect(p.x, p.y, s.w * p.scale, s.h * p.scale, self.PANEL)
    draw_rows(self.doc:rows(), p.x, p.y, p.scale, s.palette)
  end
end

function SpritePaintApp:status_text()
  local p = self.prompt
  if p and p.kind == "name" then return "save as: " .. p.text .. "_" end
  if p and p.kind == "size" then return "new size: 8, 1=16, 3=32 (esc)" end
  local state = self.message
  if not state then
    state = self.doc.dirty and "unsaved" or (self.path and "saved" or "new")
  end
  return state .. "  " .. self:file_name()
end

function SpritePaintApp:draw_status()
  local L = self.L
  acid_fill_rect(L.text_x, L.msg_y, L.text_w, SpriteLayout.CH_H, self.BG)
  local text = self:status_text():sub(1, L.text_w // SpriteLayout.CH_W)
  acid_draw_text(text, L.text_x, L.msg_y, self.prompt and self.HARD or self.MUTED, self.BG)
end

function SpritePaintApp:bar_label(id)
  local d = self.doc
  if id == "prev" then return "<" end
  if id == "next" then return ">" end
  if id == "count" then return d.frame .. "/" .. d:frame_count() end
  if id == "add" then return "+" end
  if id == "play" then return self.playing and "stop" or "play" end
  if id == "fps" then return d.s.fps .. "fps" end
  return id
end

function SpritePaintApp:draw_bar()
  local L = self.L
  acid_fill_rect(L.text_x, L.bar_y - 1, L.text_w, SpriteLayout.CH_H + 2, self.BG)
  if self.cmd_open then
    for _, it in ipairs(L.cmds) do acid_draw_text(it.label, it.x, L.bar_y, self.TEXT, self.PANEL) end
    return
  end
  for _, it in ipairs(L.bar) do
    local hot = it.id == "play" and self.playing
    acid_draw_text(self:bar_label(it.id), it.x, L.bar_y, hot and self.HARD or self.TEXT, self.BG)
  end
end

-- Touch ------------------------------------------------------------------

-- The router resends a held touch every tick: a canvas gesture follows
-- every sample, every other control acts once per press.
function SpritePaintApp:on_touch(x, y, pressed)
  if not pressed then
    self.touch_held = false
    if self.gesture then self:end_gesture() end
    return
  end
  if self.gesture then
    self:continue_gesture(x, y)
    return
  end
  if self.touch_held then return end
  self.touch_held = true
  self:tap(x, y)
end

function SpritePaintApp:tap(x, y)
  local L = self.L
  if self.prompt then return end
  if self.picker_open then
    local i = SpritePicker.hit(L, x, y)
    if i then self.doc:set_color(self.key, SpritePicker.color(i)) end
    self.picker_open = false
    self:refresh()
    return
  end
  local cx, cy = SpriteLayout.cell_at(L, self.doc.s.w, self.doc.s.h, x, y)
  if self.playing and cx then
    self:stop_playing()
    return
  end
  local item = SpriteLayout.find(self.cmd_open and L.cmds or L.bar, x, y)
  if item then
    self:action(item.id)
    return
  end
  if self.cmd_open and y >= L.bar_y - 1 and y < L.bar_y + SpriteLayout.CH_H + 1 then
    self.cmd_open = false
    self:draw_bar()
    return
  end
  local b = SpriteLayout.find(L.buttons, x, y)
  if b then
    self:select_tool(b.id)
    return
  end
  local s = SpriteLayout.find(L.swatches, x, y)
  if s then
    self:select_key(s.key)
    return
  end
  if cx then self:begin_gesture(cx, cy) end
end

function SpritePaintApp:select_tool(id)
  if id == "mirror" then self.mirror = not self.mirror else self.tool = id end
  self:draw_side()
end

-- A second tap on the selected swatch opens the colour picker for it.
function SpritePaintApp:select_key(key)
  if key == self.key and not self.playing then
    self.picker_open = true
  else
    self.key = key
  end
  self:draw_side()
end

function SpritePaintApp:paint_key()
  return self.tool == "eraser" and "." or self.key
end

function SpritePaintApp:begin_gesture(cx, cy)
  self.message = nil
  self.armed = nil
  local doc = self.doc
  if self.tool == "picker" then
    local k = SpriteTools.pick(doc, cx, cy)
    if k == "." then self.tool = "eraser" else self.key = k end
    self:refresh()
    return
  end
  doc:begin_step()
  if self.tool == "fill" then
    SpriteTools.fill(doc, cx, cy, self.key)
    doc:end_step()
    self:refresh()
    return
  end
  self.gesture = { tool = self.tool, x0 = cx, y0 = cy, x = cx, y = cy, preview = {} }
  if self.tool == "line" then
    self:draw_line_preview()
  else
    SpriteTools.paint(doc, cx, cy, self:paint_key(), self.mirror)
    self:refresh()
  end
end

-- Samples off the canvas are skipped; the next one on it joins up.
function SpritePaintApp:continue_gesture(x, y)
  local g = self.gesture
  local cx, cy = SpriteLayout.cell_at(self.L, self.doc.s.w, self.doc.s.h, x, y)
  if not cx or (cx == g.x and cy == g.y) then return end
  if g.tool == "line" then
    g.x, g.y = cx, cy
    self:draw_line_preview()
    return
  end
  SpriteTools.stroke(self.doc, g.x, g.y, cx, cy, self:paint_key(), self.mirror)
  g.x, g.y = cx, cy
  self:refresh()
end

-- The line so far, drawn over the canvas without touching the doc.
function SpritePaintApp:draw_line_preview()
  local g = self.gesture
  for _, c in ipairs(g.preview) do self:draw_cell(c[1], c[2]) end
  g.preview = {}
  for _, c in ipairs(SpriteTools.line_cells(g.x0, g.y0, g.x, g.y)) do
    g.preview[#g.preview + 1] = c
    if self.mirror then g.preview[#g.preview + 1] = { self.doc.s.w - 1 - c[1], c[2] } end
  end
  for _, c in ipairs(g.preview) do self:draw_cell(c[1], c[2], self.key) end
end

function SpritePaintApp:end_gesture()
  local g = self.gesture
  self.gesture = nil
  if g.tool == "line" then
    for _, c in ipairs(g.preview) do self:draw_cell(c[1], c[2]) end
    SpriteTools.stroke(self.doc, g.x0, g.y0, g.x, g.y, self.key, self.mirror)
  end
  self.doc:end_step()
  self:refresh()
end

-- Frame bar and commands -------------------------------------------------

local function next_fps(fps)
  for _, s in ipairs(SpritePaintApp.FPS_STEPS) do
    if s > fps then return s end
  end
  return SpritePaintApp.FPS_STEPS[1]
end

-- new and close on a dirty doc need a second press.
function SpritePaintApp:confirmed(id)
  if not self.doc.dirty or self.armed == id then
    self.armed = nil
    return true
  end
  self.armed = id
  self.message = "unsaved: " .. id .. " again to discard"
  return false
end

function SpritePaintApp:action(id)
  local d = self.doc
  if self.playing and id ~= "play" then return end
  if id ~= "new" and id ~= "close" then self.armed = nil end
  self.message = nil
  if id == "cmd" then
    self.cmd_open = true
  elseif id == "prev" then
    d:frame_prev()
  elseif id == "next" then
    d:frame_next()
  elseif id == "add" or id == "dup" then
    local done = id == "add" and d:frame_add() or (id == "dup" and d:frame_dup())
    if not done then self.message = "8 frames is the most" end
  elseif id == "del" then
    if not d:frame_del() then self.message = "can't delete the only frame" end
  elseif id == "play" then
    if self.playing then self:stop_playing() else self:start_playing() end
  elseif id == "fps" then
    d:set_fps(next_fps(d.s.fps))
  elseif id == "save" then
    self.cmd_open = false
    self:save()
  elseif id == "saveas" then
    self.cmd_open = false
    self.prompt = { kind = "name", text = "" }
  elseif id == "new" then
    self.cmd_open = false
    if self:confirmed("new") then self.prompt = { kind = "size", text = "" } end
  elseif id == "undo" then
    d:undo()
  elseif id == "redo" then
    d:redo()
  elseif id == "close" then
    self.cmd_open = false
    if self:confirmed("close") then
      self:quit()
      return
    end
  end
  self:refresh()
end

function SpritePaintApp:start_playing()
  self.playing = true
  self.play_at = acid_now_ms()
end

function SpritePaintApp:stop_playing()
  self.playing = false
  self:draw_bar()
end

function SpritePaintApp:poll_timeout_ms()
  if self.playing then return math.max(1, 1000 // self.doc.s.fps) end
  return 200
end

function SpritePaintApp:on_idle()
  if not self.playing then return end
  local now = acid_now_ms()
  if now - self.play_at >= 1000 // self.doc.s.fps then
    self.play_at = now
    self.doc:frame_next()
    self:refresh()
  end
end

-- Saving -------------------------------------------------------------------

function SpritePaintApp:save()
  if not self.path then
    self.prompt = { kind = "name", text = "" }
    return
  end
  self:write(self.path)
end

function SpritePaintApp:write(path)
  local ok, err = acid_fs_write(path, AcidSprite.serialize(self.doc.s))
  if ok then
    self.path = path
    self.doc:mark_saved()
    self.message = "saved"
  else
    self.message = "save failed: " .. tostring(err)
  end
end

-- A plain name, saved under Home with .spr added if it's missing.
function SpritePaintApp:save_as(name)
  name = name:match("^%s*(.-)%s*$")
  if name == "" or name:find("/", 1, true) then
    self.message = "not a file name"
    return
  end
  if name:sub(-4) ~= ".spr" then name = name .. ".spr" end
  self:write(self.HOME .. "/" .. name)
end

-- Keys -------------------------------------------------------------------

local SIZE_KEYS = { [string.byte("8")] = 8, [string.byte("1")] = 16, [string.byte("3")] = 32 }

function SpritePaintApp:prompt_key(code)
  local p = self.prompt
  if code == AcidKeys.ESCAPE then
    self.prompt = nil
  elseif p.kind == "size" then
    local size = SIZE_KEYS[code]
    if not size then return end
    self.prompt = nil
    self:new_sprite(size)
    return
  elseif code == AcidKeys.ENTER then
    self.prompt = nil
    self:save_as(p.text)
  elseif code == AcidKeys.BACKSPACE then
    p.text = p.text:sub(1, -2)
  elseif code >= 32 and code <= 126 and #p.text < self.NAME_MAX then
    p.text = p.text .. string.char(code)
  end
  self:draw_status()
end

function SpritePaintApp:on_key(code, pressed)
  if not pressed then return end
  if self.prompt then
    self:prompt_key(code)
    return
  end
  if self.picker_open then
    if code == AcidKeys.ESCAPE then
      self.picker_open = false
      self:draw_side()
    end
    return
  end
  if code == AcidKeys.ESCAPE then
    self.cmd_open = not self.cmd_open
    self:draw_bar()
    return
  end
  local ch = (code >= 32 and code <= 126) and string.char(code) or nil
  if self.cmd_open then
    for _, c in ipairs(SpriteLayout.CMDS) do
      if ch == c[1] then
        self:action(c[2])
        return
      end
    end
    return
  end
  if ch == " " then
    self:action("play")
  elseif self.playing then
    return
  elseif TOOL_KEYS[ch] then
    self:select_tool(TOOL_KEYS[ch])
  elseif ch == "," then
    self:action("prev")
  elseif ch == "." then
    self:action("next")
  end
end

SpritePaintApp:new():start()
````

- [ ] **Step 5: Run them and watch them pass.**
  - Run: `cargo test -p acid-lua --test game_tests sprite_app`
  - Expected: `sprite_app` (29) and `sprite_app_commands` (36) PASS.

- [ ] **Step 6: Add the boot smoke test.** In `v3/crates/acid-os/tests/games.rs`, next to `fn piano()`, add:

````rust
#[test]
fn sprite_paint() { boots_and_draws("sprite"); }
````

  - Run: `cargo test -p acid-os --test games sprite_paint`
  - Expected: PASS. This proves the real Lua runtime loads the manifest's libs and the app draws without a Lua error.

- [ ] **Step 7: Commit.**

````bash
git add v3/apps/sprite.lua v3/apps/sprite.app.toml v3/tools/sprite_test_helpers.lua v3/tools/test_sprite_app.lua v3/tools/test_sprite_app_cmds.lua v3/crates/acid-lua/tests/game_tests.rs v3/crates/acid-os/tests/games.rs
git commit -m "Sprite Paint: the app (tools, palette, frames, playback, save)

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
````

---

### Task 6: File Manager opens `.spr` files in Sprite Paint

**Files:**
- Modify: `v3/apps/file_manager.lua`, `v3/tools/test_file_manager.lua`, `v3/crates/acid-lua/tests/game_tests.rs` (the `file_manager` count, 33 → 34)

- [ ] **Step 1: Write the failing test.** Append this to the end of `v3/tools/test_file_manager.lua`:

````lua
group("sprite files")
FS["v3/fsroot/Art"] = { "ship.spr" }
FS["v3/fsroot/Art/ship.spr"] = "acid-sprite 1\n"
G.dir = "v3/fsroot/Art"
G:scan_dir()
for i, e in ipairs(G.entries) do
  if e.name == "ship.spr" then G.selected = i - 1 end
end
CALLS = {}
G:activate_selected()
eq(CALLS[1], { "spawn", "v3/apps/sprite.lua", 360, 260, "v3/fsroot/Art/ship.spr" }, "a .spr opens in Sprite Paint with its path")
````

Change the `file_manager` test's count in `game_tests.rs` from `33` to `34`.

  - Run: `cargo test -p acid-lua --test game_tests file_manager`
  - Expected: `file_manager` FAILS. The `.spr` opens a preview instead, so `CALLS[1]` is nil.

- [ ] **Step 2: Implement.** In `v3/apps/file_manager.lua`, next to `EDITOR_PATH`/`EDITOR_W`/`EDITOR_H` (around line 29), add:

````lua
FileManagerApp.SPRITE_PATH = "v3/apps/sprite.lua"
FileManagerApp.SPRITE_W = 360                 -- sprite.app.toml's size
FileManagerApp.SPRITE_H = 260
````

In `activate_selected`, after the `.lua` branch and before the `else` that opens a preview, add:

````lua
  elseif ends_with(entry.name, ".spr") then
    acid_spawn_app(self.SPRITE_PATH, self.SPRITE_W, self.SPRITE_H, self.dir .. "/" .. entry.name)
````

Also update the file's header comment, which lists what File Manager opens, to mention `.spr` files.

- [ ] **Step 3: Run it.**
  - Run: `cargo test -p acid-lua --test game_tests file_manager`
  - Expected: `file_manager` (34), `file_manager_large` and `file_manager_resize` PASS.

- [ ] **Step 4: Commit.**

````bash
git add v3/apps/file_manager.lua v3/tools/test_file_manager.lua v3/crates/acid-lua/tests/game_tests.rs
git commit -m "File Manager: .spr files open in Sprite Paint

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
````

---

### Task 7: Golden frames (new `sprite_paint.ppm`, changed `menu.ppm`)

The Menu now lists Sprite Paint, so `menu.ppm` changes. A new golden shows Sprite Paint with the sample open. **The implementer produces the actual frames and stops. The controller gets the user's approval before any golden is written.**

**Files:**
- Modify: `v3/crates/acid-os/tests/golden.rs`
- Golden files (only after approval): `v3/crates/acid-os/tests/golden/sprite_paint.ppm`, `menu.ppm`

- [ ] **Step 1: Add the golden test.** In `golden.rs`, after `acid_spin_matches_golden`, add:

````rust
#[test]
fn sprite_paint_matches_golden() {
    let p = FakePlatform::new(FakePlatform::repo_root());
    let k = boot_with(p.clone(), Screen::DEFAULT);
    wait_for_desktop(&k, Screen::DEFAULT);
    let (w, h) = (360, 260); // the manifest's size
    let (x, y) = acid_kernel::placement::cascade_position(k.screen(), k.with_state(|st| st.windows.count()), w, h);
    let sprite = k
        .spawn_app(SpawnRequest {
            script_path: format!("{APPS_DIR}/sprite.lua"),
            x, y, w, h, closable: true,
            arg: Some("v3/fsroot/Home/acid_ship.spr".into()),
            libs: Some("lib/acid_sprite.lua, sprite/doc.lua, sprite/tools.lua, sprite/layout.lua, sprite/picker.lua".into()),
            force_cart: false,
        })
        .expect("Sprite Paint opens");
    k.activate_window(sprite);
    // Sprite Paint draws its border last, so a border means a whole frame.
    wait_for_border(&k, sprite, w, h);
    k.composite_frame();
    // Resizable, so the kernel draws a grip in the window's bottom-right 8x8.
    let (wx, wy, ww, wh) = k.with_state(|st| st.windows.by_task(sprite).map(|win| (win.x, win.y, win.w, win.h))).unwrap();
    let grip = ((wx + ww - 8) as usize, (wy + wh - 8) as usize, 8, 8);
    assert_matches_golden_masked(&p.display.last_frame().unwrap(), "sprite_paint.ppm", 640, &[clock_mask(Screen::DEFAULT), grip]);
}
````

- [ ] **Step 2: Produce the actual frames.**
  - Run: `cargo test -p acid-os --test golden`
  - Expected: two failures, with every other golden passing:
    - `sprite_paint_matches_golden` fails with "no golden sprite_paint.ppm yet";
    - the menu golden test fails with "pixels differ from golden menu.ppm".

  Both write `v3/target/actual-<name>.ppm`. Convert them for review:

````bash
magick v3/target/actual-sprite_paint.ppm v3/target/actual-sprite_paint.png
magick v3/target/actual-menu.ppm v3/target/actual-menu.png
````

- [ ] **Step 3: STOP. Report back with the two PNG paths.** Don't copy the files into `tests/golden/` and don't commit them. The controller shows the PNGs to the user.

- [ ] **Step 4: After approval** (the controller does this or dispatches it):

````bash
cp v3/target/actual-sprite_paint.ppm v3/crates/acid-os/tests/golden/sprite_paint.ppm
cp v3/target/actual-menu.ppm v3/crates/acid-os/tests/golden/menu.ppm
cargo test -p acid-os --test golden      # all PASS
git add v3/crates/acid-os/tests/golden.rs v3/crates/acid-os/tests/golden/sprite_paint.ppm v3/crates/acid-os/tests/golden/menu.ppm
git commit -m "Goldens: Sprite Paint with the sample ship; Menu lists Sprite Paint

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
````

---

### Task 8: Docs

**Files:**
- Modify: `docs/manual-v3/04-graphics.md`, `docs/manual-v3/09-api-reference.md`, `docs/manual-v3/01-getting-started.md`

- [ ] **Step 1: Chapter 4.** In `04-graphics.md`, at the end of §4.6 Sprites (just before `## 4.7 The wallpaper`), add:

````markdown
### Sprite files

Sprite Paint saves sprites as `.spr` text files, normally in `Home`. Any app
can load one with `AcidSprite.load` and draw its frames with
`AcidSprite.draw`, because each frame is a list of picture-strings and the
palette is keyed by the same characters:

```lua snippet
local ship = AcidSprite.load("v3/fsroot/Home/acid_ship.spr")
AcidSprite.draw(ship.frames[1], x, y, 2, ship.palette)
```

The format is plain text, one item per line:

```text
acid-sprite 1
size 16 16
fps 6
pal 1 ff00ff
frame
..11............
...
```

- The first line is `acid-sprite 1`. Blank lines and lines starting with `#`
  are ignored, anywhere in the file.
- `size W H` gives the width and height, each from 1 to 32. It comes before
  the first `frame`.
- `fps N` (1 to 30) is the animation speed. It is optional and defaults to 6.
- `pal K RRGGBB` gives one palette entry. K is one of `0`–`9` or `a`–`f`, so
  there are at most 16 entries, and they come before the first `frame`.
- `frame` starts a frame of exactly H rows of W characters. Each character
  is `.` (transparent) or a palette key. A file has 1 to 8 frames.

`AcidSprite.parse(text)` returns the same table from a string, and
`AcidSprite.serialize(sprite)` turns a table back into text. A bad file
makes `load` and `parse` return `nil` and a message such as
`"line 4: row is 3 wide, expected 16"`. They never raise an error.
````

Both fence tags, `lua snippet` and `text`, are in `manual.rs`'s `TAGS` list.

- [ ] **Step 2: Chapter 9.** In `09-api-reference.md`, in the `### AcidSprite` entry:
  - after the existing code block, add the three new calls to the block:

````lua
local s, err = AcidSprite.load(path)    -- read and parse a .spr file
local s, err = AcidSprite.parse(text)   -- parse .spr text
local text = AcidSprite.serialize(s)    -- .spr text for a sprite table
````

  - after the existing bullets, add one:

````markdown
- `load` and `parse` return a sprite table,
  `{ w, h, fps, palette, frames }`, where each of `frames` is a `rows` list
  you can pass to `draw` with `palette`. On a bad file they return `nil` and
  `"line N: reason"`. See [Sprite files](04-graphics.md#sprite-files).
````

- [ ] **Step 3: Chapter 1.** In `01-getting-started.md`, after the paragraph that starts "You can also work from inside Acid OS itself", add:

````markdown
**Sprite Paint** (Menu → Sprite Paint) draws pixel-art sprites, with
animation frames, and saves them as `.spr` files in `Home`. Your apps can
load them with `AcidSprite.load`
([§4.6](04-graphics.md#sprite-files)). Open `Home/acid_ship.spr` from the
File Manager to see one.
````

- [ ] **Step 4: Run the manual checks.**
  - Run: `cargo test -p acid-os --test manual`
  - Expected: PASS.

- [ ] **Step 5: Run the full suite.**
  - Run: `cargo test --workspace`
  - Expected: all PASS (the goldens were approved in Task 7).

- [ ] **Step 6: Commit.**

````bash
git add docs/manual-v3/01-getting-started.md docs/manual-v3/04-graphics.md docs/manual-v3/09-api-reference.md
git commit -m "Manual: Sprite Paint and .spr sprite files

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
````

