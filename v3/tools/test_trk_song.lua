-- TrkSong (apps/tracker/song.lua): note names, rows, the new-song
-- template, and reading then writing .trk text byte for byte -- checked
-- against acid-sound's own canonical files (TRK, from the prelude) so the
-- Lua and Rust writers can't drift apart.

group("note names")
eq({ TrkSong.parse_note("A-0"), TrkSong.parse_note("C-4"), TrkSong.parse_note("C#4"), TrkSong.parse_note("C-8") },
  { 1, 40, 41, 88 }, "note names map to piano keys")
eq({ TrkSong.parse_note("G#0"), TrkSong.parse_note("C#8"), TrkSong.parse_note("H-4"), TrkSong.parse_note("C-") },
  {}, "off-keyboard and malformed names are nil")
local bad = {}
for ona = 1, 88 do
  if TrkSong.parse_note(TrkSong.note_name(ona)) ~= ona then bad[#bad + 1] = ona end
end
eq(bad, {}, "every key's name reads back")

group("cells and rows")
local blank = TrkSong.empty_row()
eq(TrkSong.cell_text(TrkSong.empty_cell()), "... .. . ..", "an empty cell is dots")
eq(TrkSong.cell_text({ note = 40, inst = 0x1F, cmd = "A", param = 0 }), "C-4 1F A 00", "a full cell")
eq(TrkSong.cell_text({ note = 255, inst = 0, cmd = "", param = 0 }), "=== .. . ..", "a note-off")
blank[8].note = 255
eq(TrkSong.row_text(blank), string.rep("... .. . .. | ", 7) .. "=== .. . ..", "a row is eight cells split by |")

group("a new song")
local fresh = TrkSong.write(TrkSong.new())
local s = TrkSong.parse(fresh)
eq({ s.title, s.speed, s.order, s.loop, #s.patterns[0], #s.patterns[0][1] }, { "untitled", 6, { 0 }, 0, 16, 8 },
  "a new song: one 16-row pattern of eight tracks")
eq(TrkSong.write(s), fresh, "and it round-trips")
eq(TrkSong.free_pattern(s), 1, "the next free pattern number")

group("canonical files round-trip")
for _, f in ipairs(TRK) do
  local song, err = TrkSong.parse(f[2])
  ok(song, f[1] .. " parses" .. (err and (": " .. err) or ""))
  eq(song and TrkSong.write(song), f[2], f[1] .. " writes back byte for byte")
end

group("everything the format has")
local function row(...)
  local cells = { ... }
  for t = #cells + 1, 8 do cells[t] = "... .. . .." end
  return table.concat(cells, " | ") .. "\n"
end
local ROUND = [[
acid-track 2
title Round Trip
speed 6
instrument 01 "Lead"  wave saw  adsr 0 8 70 20  duty 50  pwm 2  vib 4 2  arp 4 7  filter lp 120 4
instrument 02 "Bass"  script "Home/sounds/demo.snd" bass
instrument 03 "Hat"  wave noise  adsr 0 2 0 2  duty 50
order 00 3F 00 loop 1

pattern 00 4
]] .. row("C-4 01 . ..", "A-0 03 9 20") .. row("... .. 4 22") .. row("=== .. . ..") ..
  row("D#3 02 F 03", "... .. . ..", "... .. . ..", "... .. . ..", "... .. . ..", "... .. . ..", "... .. . ..", "C-8 02 1 00") ..
  "\npattern 3F 2\n" .. row("... .. A FF") .. row()
eq(TrkSong.write(TrkSong.parse(ROUND)), ROUND, "every field and row form round-trips")
local r = TrkSong.parse(ROUND)
eq({ r.instruments[1].filter, r.instruments[1].arp }, { { 1, 120, 4 }, { 4, 7 } }, "built-in fields read into the model")
eq({ r.instruments[2].kind, r.instruments[2].path, r.instruments[2].block },
  { "script", "Home/sounds/demo.snd", "bass" }, "and script instruments")
eq({ r.order, r.loop, r.patterns[0][4][1].cmd, r.patterns[0][4][1].param, r.patterns[0][4][8].note, r.patterns[0][1][2].inst },
  { { 0, 0x3F, 0 }, 1, "F", 3, 88, 3 }, "the order, commands and every track's cells")

group("not a song")
eq({ TrkSong.parse("hello") }, { nil, "not an acid-track 2 file" }, "anything else is refused")
eq(({ TrkSong.parse("acid-track 1\n") })[2], "not an acid-track 2 file", "so are 4-channel songs")
eq(TrkSong.clamp(500, "duty"), 99, "clamp holds a value inside its range")

group("out of range numbers are refused")
local function variant(from, to) return (ROUND:gsub(from, to, 1)) end
local function pat_len(n) return (ROUND:gsub("pattern 3F 2", "pattern 3F " .. n, 1)) end
eq({ TrkSong.parse(variant("speed 6", "speed 0")), TrkSong.parse(variant("speed 6", "speed 9223372036854775807")),
     TrkSong.parse(variant("order 00 3F 00", "order 00 40 00")),
     TrkSong.parse(variant("adsr 0 8 70 20", "adsr -5 0 0 0")), TrkSong.parse(variant("pwm 2", "pwm 51")),
     TrkSong.parse(pat_len(65)), (TrkSong.parse(variant("loop 1", "loop 3"))),
     (TrkSong.parse(variant("order 00 3F 00", "order" .. string.rep(" 00", 129)))),
     (TrkSong.parse(variant("C%-4 01 . .. | ", "C-4 01 . .. "))) },
  {}, "speed, a pattern past 3F, adsr, pwm, pattern length, loop, a long order and a short row are refused")
local edge = variant("speed 6", "speed 31"):gsub("pwm 2", "pwm -50")
  :gsub("adsr 0 8 70 20", "adsr 100000 100000 100000 100000")
ok(TrkSong.parse(edge), "boundary values still parse")
