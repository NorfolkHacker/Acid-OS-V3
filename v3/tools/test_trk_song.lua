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

group("rows")
eq(TrkSong.row_text({ note = 0, inst = 0, cmd = "", param = 0, note2 = 0 }), "... .. . .. ...", "an empty row is dots")
eq(TrkSong.row_text({ note = 40, inst = 0x1F, cmd = "A", param = 0, note2 = 52 }), "C-4 1F A 00 C-5", "a full row")
eq(TrkSong.row_text({ note = 255, inst = 0, cmd = "", param = 0, note2 = 0 }), "=== .. . .. ...", "a note-off")

group("a new song")
local fresh = TrkSong.write(TrkSong.new())
local s = TrkSong.parse(fresh)
eq({ s.title, s.speed, s.donor, #s.orders[1].entries, #s.patterns[3] }, { "untitled", 6, 4, 1, 16 },
  "a new song: four channels, each on its own 16-row pattern")
eq(TrkSong.write(s), fresh, "and it round-trips")

group("canonical files round-trip")
for _, f in ipairs(TRK) do
  local song, err = TrkSong.parse(f[2])
  ok(song, f[1] .. " parses" .. (err and (": " .. err) or ""))
  eq(song and TrkSong.write(song), f[2], f[1] .. " writes back byte for byte")
end

group("everything the format has")
local ROUND = [[
acid-track 1
title Round Trip
speed 6
sfx-donor 4
instrument 01 "Lead"  wave saw  adsr 0 8 70 20  duty 50  pwm 2  vib 4 2  arp 4 7  filter lp 120 4  voice2 detune 6
instrument 02 "Bass"  script "Home/sounds/demo.snd" bass
instrument 03 "Hat"  wave noise  adsr 0 2 0 2  duty 50  voice2 octave
order 1  00 00+12 loop 1
order 2  01 01-5 loop 0
order 3  01 loop 0
order 4  01 loop 0

pattern 00 4
C-4 01 . .. ...
... .. 4 22 ...
=== .. . .. ...
D#3 02 F 03 G-3

pattern 01 2
A-0 03 9 20 ...
... .. A FF ...
]]
eq(TrkSong.write(TrkSong.parse(ROUND)), ROUND, "every field and row form round-trips")
local r = TrkSong.parse(ROUND)
eq({ r.instruments[1].voice2, r.instruments[1].detune, r.instruments[1].filter, r.instruments[1].arp },
  { "detune", 6, { 1, 120, 4 }, { 4, 7 } }, "built-in fields read into the model")
eq({ r.instruments[2].kind, r.instruments[2].path, r.instruments[2].block },
  { "script", "Home/sounds/demo.snd", "bass" }, "and script instruments")
eq({ r.orders[1].entries[2].transpose, r.orders[1].loop, r.patterns[0][4].cmd, r.patterns[0][4].param },
  { 12, 1, "F", 3 }, "orders, transposes and commands")

group("not a song")
eq({ TrkSong.parse("hello") }, { nil, "not an acid-track 1 file" }, "anything else is refused")
eq(TrkSong.clamp(500, "duty"), 99, "clamp holds a value inside its range")
