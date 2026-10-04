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
