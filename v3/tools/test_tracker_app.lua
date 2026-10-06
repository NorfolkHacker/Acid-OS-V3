-- Acid Tracker (apps/tracker.lua) against the faked acid_song_* calls: a
-- new song, previewing, typing in edit mode, playback with live updates,
-- follow and mutes, the command line, saving and opening, script
-- instruments, and fitting the window at its default and smallest sizes.
local G = GAME
local K = AcidKeys
local function key(c) G:on_key(type(c) == "string" and c:byte() or c, true) end
local function up(c) G:on_key(type(c) == "string" and c:byte() or c, false) end
local function keys(s) for i = 1, #s do key(s:sub(i, i)) end end
local function command(s) key(K.ESCAPE); keys(s); key(K.ENTER) end
local function shown(text)
  for _, t in ipairs(TEXT_AT) do
    if t[1]:find(text, 1, true) then return true end
  end
  return false
end
local function fits(what)
  TEXT_AT, RECTS = {}, {}
  G:redraw()
  local a, why = drawn_inside_window()
  local b, why2 = drawn_text_clear()
  ok(a and b, what .. (why and (": " .. why) or "") .. (why2 and (": " .. why2) or ""))
end

group("a new song")
eq({ G.E.ch, G.E.row, G.E.edit, G.path == nil }, { 1, 0, false, true }, "cursor on channel 1, row 00, not editing, no file")
eq(SOUND_CALLS, { { "parse", 1 } }, "the new song was handed to the kernel")
fits("everything fits 480x320")
ok(shown("ORD 00/00  ROW 00  SPD 6  OCT 4  INS 01 Lead"), "the status line")

group("previewing")
SOUND_CALLS = {}
key("z")
eq(SOUND_CALLS, { { "preview", 1, 1, 40, 1 } }, "z previews C-4 on channel 1 with instrument 01")
up("z")
eq(SOUND_CALLS[2], { "preview", 1, 1, 0, 0 }, "letting go sends note-off")
eq(G.E.song.patterns[0][1].note, 0, "and nothing was written")

group("editing")
key(" ")
eq(G.E.edit, true, "space turns edit mode on")
SOUND_CALLS = {}
key("q")
eq(SOUND_CALLS, { { "update", 1 }, { "preview", 1, 1, 52, 1 } }, "a typed note goes to the kernel, then previews")
up("q")
eq({ G.E.song.patterns[0][1].note, G.E.row, G.E.dirty }, { 52, 1, true }, "C-5 written, the cursor down, the song unsaved")

group("playing")
SOUND_CALLS = {}
key(K.F1)
eq(SOUND_CALLS, { { "play", 1, 0, 0 } }, "F1 plays from the start")
key(K.F5)
eq(SOUND_CALLS[2], { "mute", 1, true }, "F5 mutes channel 1 while playing")
SOUND_CALLS = {}
key("w")
eq(SOUND_CALLS[1], { "update", 1 }, "an edit while playing is sent at once")
up("w")
SONG_POS = { 0, 7, 2 }
G:on_idle()
eq(G.play_pos, { 0, 7 }, "the grid follows the song")
TEXT_AT = {}
G:redraw()
ok(shown("ROW 07"), "and the status line shows the playing row")
eq(G:poll_timeout_ms(), TrackerApp.PLAY_MS, "it reads the position often while playing")
key(K.F4)
eq({ G.playing, SOUND_CALLS[#SOUND_CALLS] }, { false, { "stop" } }, "F4 stops")
key(K.F2)
eq(SOUND_CALLS[#SOUND_CALLS - 1], { "play", 1, 0, 2 }, "F2 plays from the cursor's row, and re-mutes")
SONG_POS = nil
G:on_idle()
eq(G.playing, false, "a song that stops by itself ends playing")

group("commands")
command("speed 9")
eq(G.E.song.speed, 9, ":speed sets the speed")
command("speed 99")
eq({ G.E.song.speed, G.message }, { 9, "usage: speed 1-31" }, "an out-of-range speed is refused")
command("len 8")
eq(#G.E.song.patterns[0], 8, ":len resizes the pattern under the cursor")
command("ins 2 script Home/sounds/bass.snd fatbass")
eq({ G.E.inst, G.E.song.instruments[2].kind, G.E.song.instruments[2].block }, { 2, "script", "fatbass" },
  ":ins makes a script instrument and selects it")
command("arp 4 7")
eq(G.message, "not a built-in instrument", ":arp needs a built-in instrument")
command("ins 1")
command("arp 4 99")
eq(G.message, "usage: arp [a [b [c]]] (-48 to 48)", "arp offsets are limited")
command("arp 4 7")
eq(G.E.song.instruments[1].arp, { 4, 7 }, ":arp sets the arpeggio")
command("title Night Drive")
eq(G.E.song.title, "Night Drive", ":title names the song")
command("frob")
eq(G.message, "unknown command: frob", "an unknown command is named")

group("saving and opening")
command("w groove")
eq({ FS["v3/fsroot/Home/groove.trk"], G.path, G.E.dirty }, { TrkSong.write(G.E.song), "v3/fsroot/Home/groove.trk", false },
  ":w saves under Home with .trk added")
command("o ../escape")
eq(G.message, "not a file name", "names can't climb out of Home")
command("speed 5")
command("o groove")
eq(G.message, "unsaved: o again to discard", "opening over unsaved changes asks for a second go")
command("o groove")
eq({ G.E.song.speed, G.E.dirty, G.handle }, { 9, false, 2 }, "the second go opens the saved song as a new handle")
eq(SOUND_CALLS[#SOUND_CALLS], { "free", 1 }, "and frees the old one")
SONG_PARSE_ERR = "1: not an acid-track file"
FS["v3/fsroot/Home/bad.trk"] = "nope"
command("o bad")
eq({ G.message, G.path }, { "can't open: 1: not an acid-track file", nil }, "a bad file leaves a new song and says why")

group("smallest window")
resize_app(420, 240)
local a, why = drawn_inside_window()
local b2, why2 = drawn_text_clear()
ok(a and b2, "everything fits 420x240" .. (why and (": " .. why) or "") .. (why2 and (": " .. why2) or ""))

group("script instruments and quitting")
command("ins 3 script Home/sounds/wobble.snd wobble")
G.E.focus = "ins"
CALLS = {}
key("e")
eq(CALLS[1], { "spawn", "v3/apps/editor.lua", 420, 280, "v3/fsroot/Home/sounds/wobble.snd" },
  "e opens a script instrument's file in the Editor")
command("q")
eq(G.message, "unsaved: q again to discard", "quitting an unsaved song asks for a second go")
command("q")
eq(G.running, false, "the second go quits")
