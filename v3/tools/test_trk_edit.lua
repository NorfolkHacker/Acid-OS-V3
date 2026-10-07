-- TrkEdit (apps/tracker/edit.lua): moving the cursor, typing into the
-- grid, the orders panel and the instrument fields, on a TrkSong model.
local K = AcidKeys
local function b(c) return c:byte() end
local E = TrkEdit.new(TrkSong.new())
local P = E.song.patterns

group("moving")
E:move_row(-1)
eq(E.row, 15, "up from row 00 wraps to the pattern's last row")
E:move_row(1)
E:move_slot(-1)
eq({ E.ch, E.slot_i }, { 8, 6 }, "left from track 1's note wraps to track 8's last digit")
E:move_slot(1)
eq({ E.ch, E.slot_i }, { 1, 1 }, "and right comes back")
for _ = 1, 7 do E:next_focus() end
eq({ E.ch, E.focus }, { 8, "grid" }, "Tab steps through the eight tracks")
E:next_focus()
eq(E.focus, "orders", "then to the orders panel")
E:next_focus()
eq(E.focus, "ins", "then to the instrument")
E:next_focus()
eq({ E.focus, E.ch }, { "grid", 1 }, "and back to track 1")

group("piano keys")
eq({ E:piano_note(b("z")), E:piano_note(b("m")), E:piano_note(b("q")), E:piano_note(b("i")) },
  { 40, 51, 52, 64 }, "two octaves of keys from C-4")
E:set_octave(9)
eq(E.octave, 7, "the octave stops at 7")
eq(E:piano_note(b("i")), nil, "a key off the top of the keyboard plays nothing")
E:set_octave(-3)

group("typing into the grid")
local changed, note = E:grid_key(b("q"))
eq({ changed, note, P[0][1][1].note, P[0][1][1].inst, E.row }, { true, 52, 52, 1, 1 },
  "a note key writes the note and the current instrument, then steps down")
E:grid_key(b("`"))
eq(P[0][2][1].note, 255, "` writes a note-off")
E.row, E.slot_i = 4, 2
E:grid_key(b("1")); E:grid_key(b("f"))
eq({ P[0][5][1].inst, E.row, E.slot_i }, { 0x1F, 5, 2 }, "two hex digits set the instrument, then step down")
E.row, E.slot_i = 4, 2
E:grid_key(b("7")); E:grid_key(b("f"))
eq(P[0][5][1].inst, 0x3F, "an instrument past 3F is held at 3F")
E.row, E.slot_i = 6, 4
E:grid_key(b("a")); E:grid_key(b("3")); E:grid_key(b("c"))
eq({ P[0][7][1].cmd, P[0][7][1].param, E.row }, { "A", 0x3C, 7 }, "a command letter moves on to its parameter")
eq({ E:grid_key(b("x")) }, { false }, "a key that isn't a hex digit does nothing there")
E.row, E.slot_i = 6, 4
E:grid_key(K.DELETE)
eq({ P[0][7][1].cmd, P[0][7][1].param, E.row }, { "", 0, 7 }, "Delete on the command clears it and its parameter")
E.ch, E.row, E.slot_i = 8, 0, 1
E:grid_key(b("e"))
eq({ P[0][1][8].note, P[0][1][1].note }, { 56, 52 }, "track 8 has its own cells")
E.ch = 1

group("pattern length")
E.row = 10
E:set_length(4)
eq({ #P[0], E.row }, { 4, 3 }, "shortening a pattern pulls the cursor in")
eq(P[0][1][8].note, 56, "and keeps every track's rows")
E:set_length(20)
eq({ #P[0], #P[0][20], P[0][20][5].note }, { 20, 8, 0 }, "lengthening it adds empty rows across all eight tracks")
E:set_length(99)
eq(#P[0], 64, "a pattern is at most 64 rows")
E:set_length(16)

group("orders")
E.focus = "orders"
local O = E.song.order
local _, msg = E:orders_key(b("n"))
eq({ O, E.ord_pos, E.order, #P[1], msg }, { { 0, 1 }, 1, 1, 16, "pattern 01" },
  "n makes a new empty pattern after the entry and shows it")
P[1][1][3].note = 40
E:orders_key(b("p"))
eq({ O, P[2][1][3].note, P[2] ~= P[1] }, { { 0, 1, 2 }, 40, true }, "p copies the entry's pattern into a new one")
E:orders_key(b("0")); E:orders_key(b("7"))
eq({ O[3], #P[7] }, { 7, 16 }, "two hex digits pick a pattern, creating it empty")
E:orders_key(b("4")); E:orders_key(b("F"))
eq(O[3], 0x3F, "pattern numbers stop at 3F")
E:orders_key(b("+"))
eq({ O[3], P[0] ~= nil }, { 0, true }, "+ steps to the next pattern, wrapping round")
E:orders_key(b("-"))
eq(O[3], 0x3F, "- steps back")
E:orders_key(K.ENTER)
eq({ O, E.ord_pos }, { { 0, 1, 0x3F, 0x3F }, 3 }, "Enter repeats the entry after itself")
E:orders_key(K.LEFT)
E:orders_key(b("l"))
eq(E.song.loop, 2, "l makes this entry the loop point")
E:orders_key(K.LEFT); E:orders_key(K.LEFT)
E:orders_key(K.DELETE)
eq({ O, E.song.loop, E.ord_pos }, { { 1, 0x3F, 0x3F }, 1, 0 }, "Delete removes it and keeps the loop on its entry")
E:orders_key(K.DELETE); E:orders_key(K.DELETE)
eq({ E:orders_key(K.DELETE), #O, E.song.loop }, { false, 1, 0 }, "the last entry can't be deleted")
for n = 0, TrkSong.MAX_PATTERN do P[n] = P[n] or TrkSong.empty_rows(1) end
eq({ E:orders_key(b("n")) }, { false, "all 64 patterns are in use (:clean drops unused ones)" }, "there are 64 patterns at most")
while #O < TrkSong.MAX_ORDER do O[#O + 1] = 0 end
eq({ E:orders_key(K.ENTER) }, { false, "the order is full (128 entries)" }, "and 128 order entries")
while #O > 1 do O[#O] = nil end

group("instrument fields")
E.focus = "ins"
local ins = E.song.instruments[1]
E.ins_field = 1
E:ins_key(K.RIGHT)
eq(ins.wave, 1, "Right steps the waveform")
E:ins_key(K.DOWN)
E:ins_key(b("+"))
eq(ins.adsr[1], 12, "+ adds 10 to the attack")
for _ = 1, 20 do E:ins_key(b("-")) end
eq(ins.adsr[1], 0, "and it never goes below 0")
E.ins_field = 11
eq(E:ins_key(K.RIGHT), false, "cutoff does nothing until the filter is on")
E.ins_field = 10
E:ins_key(K.RIGHT)
eq(ins.filter, { 1, 128, 0 }, "the filter turns on as a low-pass")
E.inst = 9
eq(E:ins_key(K.RIGHT), false, "an empty instrument slot has no fields")
ok(TrkSong.parse(TrkSong.write(E.song)), "after all that the song still writes and reads")
