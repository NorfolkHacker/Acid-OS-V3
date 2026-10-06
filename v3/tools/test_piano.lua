-- Headless tests for Piano (apps/piano.lua): white/black key hit-testing, a
-- press plays ROOT_ONA + offset on VOICE at volume 45, holding the same key
-- doesn't retrigger, and only a release of a sounding key stops it.

local P = GAME

group("keys")
eq(P:hit_test(0), 0, "the left edge is the root (C)")
eq(P:hit_test(Piano.WINDOW_W - 1), 11, "the right edge is B")
eq(P:hit_test(P:black_key_x(0)), 1, "the first black key is C#")
eq(P:hit_test(-1), 0, "a drag off the left edge stays on C")
eq(P:hit_test(-100000), 0, "a drag far off the left edge stays on C")
eq(P:hit_test(Piano.WINDOW_W + 100000), 11, "a drag far off the right edge stays on B")

group("playing")
NOTES = {}
P:on_touch(5, 50, true)
eq(NOTES, { { "play", Piano.VOICE, Piano.ROOT_ONA, 45 } }, "a press plays the root note")
NOTES = {}
P:on_touch(6, 50, true)
eq(NOTES, {}, "holding the same key doesn't retrigger")
P:on_touch(6, 50, false)
eq(NOTES, { { "stop", Piano.VOICE } }, "release stops the note")
NOTES = {}
P:on_touch(6, 50, false)
eq(NOTES, {}, "a second release does nothing")
