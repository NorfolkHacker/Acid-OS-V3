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
eq({ E.ch, E.slot_i }, { 4, 7 }, "left from channel 1's note wraps to channel 4's second note")
E:move_slot(1)
eq({ E.ch, E.slot_i }, { 1, 1 }, "and right comes back")
E:next_focus(); E:next_focus(); E:next_focus()
eq({ E.ch, E.focus }, { 4, "grid" }, "Tab steps through the channels")
E:next_focus()
eq(E.focus, "orders", "then to the orders panel")
E:next_focus()
eq(E.focus, "ins", "then to the instrument")
E:next_focus()
eq({ E.focus, E.ch }, { "grid", 1 }, "and back to channel 1")

group("piano keys")
eq({ E:piano_note(b("z")), E:piano_note(b("m")), E:piano_note(b("q")), E:piano_note(b("i")) },
  { 40, 51, 52, 64 }, "two octaves of keys from C-4")
E:set_octave(9)
eq(E.octave, 7, "the octave stops at 7")
eq(E:piano_note(b("i")), nil, "a key off the top of the keyboard plays nothing")
E:set_octave(-3)

group("typing into the grid")
local changed, note = E:grid_key(b("q"))
eq({ changed, note, P[0][1].note, P[0][1].inst, E.row }, { true, 52, 52, 1, 1 },
  "a note key writes the note and the current instrument, then steps down")
E:grid_key(b("`"))
eq(P[0][2].note, 255, "` writes a note-off")
E.row, E.slot_i = 4, 2
E:grid_key(b("1")); E:grid_key(b("f"))
eq({ P[0][5].inst, E.row, E.slot_i }, { 0x1F, 5, 2 }, "two hex digits set the instrument, then step down")
E.row, E.slot_i = 4, 2
E:grid_key(b("7")); E:grid_key(b("f"))
eq(P[0][5].inst, 0x3F, "an instrument past 3F is held at 3F")
E.row, E.slot_i = 6, 4
E:grid_key(b("a")); E:grid_key(b("3")); E:grid_key(b("c"))
eq({ P[0][7].cmd, P[0][7].param, E.row }, { "A", 0x3C, 7 }, "a command letter moves on to its parameter")
eq({ E:grid_key(b("x")) }, { false }, "a key that isn't a hex digit does nothing there")
E.row, E.slot_i = 6, 4
E:grid_key(K.DELETE)
eq({ P[0][7].cmd, P[0][7].param, E.row }, { "", 0, 7 }, "Delete on the command clears it and its parameter")
E.row, E.slot_i = 0, 7
local _, n2 = E:grid_key(b("e"))
eq({ P[0][1].note2, n2 }, { 56, 56 }, "the second note column takes notes")
E.row, E.slot_i = 0, 7
eq({ E:grid_key(b("`")) }, { false }, "but not note-offs")

group("pattern length")
E.row = 10
E:set_length(4)
eq({ #P[0], E.row }, { 4, 3 }, "shortening a pattern pulls the cursor in")
E:set_length(20)
eq({ #P[0], P[0][20].note }, { 20, 0 }, "lengthening it adds empty rows")

group("orders")
E.focus = "orders"
E:orders_key(b("0")); E:orders_key(b("7"))
eq({ E.song.orders[1].entries[1].pattern, #P[7] }, { 7, 16 }, "two hex digits pick a pattern, creating it empty")
E:orders_key(K.ENTER)
eq({ #E.song.orders[1].entries, E.ord_pos, E.song.orders[1].entries[2].pattern }, { 2, 1, 7 },
  "Enter repeats the entry after itself")
E:orders_key(b("+")); E:orders_key(b("+"))
eq(E.song.orders[1].entries[2].transpose, 2, "+ transposes it up")
for _ = 1, 60 do E:orders_key(b("-")) end
eq(E.song.orders[1].entries[2].transpose, -48, "transposes stop at 48 semitones")
E:orders_key(b("l"))
eq(E.song.orders[1].loop, 1, "l makes this entry the loop point")
E:orders_key(K.DELETE)
eq({ #E.song.orders[1].entries, E.song.orders[1].loop, E.ord_pos }, { 1, 0, 0 },
  "Delete removes it and pulls the loop point back")
eq(E:orders_key(K.DELETE), false, "a channel's last entry can't be deleted")

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
E.ins_field = 10
E:ins_key(K.RIGHT)
eq(ins.filter, { 1, 128, 0 }, "the filter turns on as a low-pass")
E.ins_field = 14
eq(E:ins_key(K.RIGHT), false, "detune does nothing until voice 2 detunes")
E.ins_field = 13
E:ins_key(K.RIGHT)
E.ins_field = 14
for _ = 1, 100 do E:ins_key(b("+")) end
eq({ ins.voice2, ins.detune }, { "detune", 768 }, "voice 2 detunes, held at 768")
E.inst = 9
eq(E:ins_key(K.RIGHT), false, "an empty instrument slot has no fields")
ok(TrkSong.parse(TrkSong.write(E.song)), "after all that the song still writes and reads")
