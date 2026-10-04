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
