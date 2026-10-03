-- Headless tests for AcidSprite (apps/lib/acid_sprite.lua).
-- game_test_env.lua's acid_overlay_fill_rect records into RECTS.

local PAL = { R = 0xFF0000, B = 0x0000FF }

group("AcidSprite: geometry")
eq(AcidSprite.width({ "..RR", "RRRR" }), 4, "width is the row length")
eq(AcidSprite.height({ "..RR", "RRRR" }), 2, "height is the row count")

group("AcidSprite: drawing")
RECTS = {}
AcidSprite.draw({ "R" }, 10, 20, 1, PAL)
eq(RECTS, { { 10, 20, 1, 1, 0xFF0000 } }, "a single pixel is one 1x1 rect at the origin")
RECTS = {}
AcidSprite.draw({ "R" }, 10, 20, 3, PAL)
eq(RECTS, { { 10, 20, 3, 3, 0xFF0000 } }, "scale 3 makes each pixel a 3x3 block")
RECTS = {}
AcidSprite.draw({ ".R" }, 10, 20, 3, PAL)
eq(RECTS, { { 13, 20, 3, 3, 0xFF0000 } }, "'.' is transparent and shifts the next pixel")
RECTS = {}
AcidSprite.draw({ "R", "B" }, 10, 20, 2, PAL)
eq(RECTS, { { 10, 20, 2, 2, 0xFF0000 }, { 10, 22, 2, 2, 0x0000FF } }, "rows advance by scale in y")
RECTS = {}
AcidSprite.draw({ "..." }, 10, 20, 2, PAL)
eq(RECTS, {}, "an all-transparent row draws nothing")
RECTS = {}
AcidSprite.draw({ "X" }, 10, 20, 2, PAL)
eq(RECTS, {}, "a character missing from the palette draws nothing")

group("AcidSprite: horizontal run merging")
RECTS = {}
AcidSprite.draw({ "RRR" }, 0, 0, 2, PAL)
eq(RECTS, { { 0, 0, 6, 2, 0xFF0000 } }, "three same-colour pixels become one wide rect")
RECTS = {}
AcidSprite.draw({ "RRBB" }, 0, 0, 1, PAL)
eq(RECTS, { { 0, 0, 2, 1, 0xFF0000 }, { 2, 0, 2, 1, 0x0000FF } }, "a colour change ends the run")
RECTS = {}
AcidSprite.draw({ "RR.R" }, 0, 0, 1, PAL)
eq(RECTS, { { 0, 0, 2, 1, 0xFF0000 }, { 3, 0, 1, 1, 0xFF0000 } }, "a transparent gap ends the run")

group("AcidSprite: flip")
RECTS = {}
AcidSprite.draw({ ".R" }, 0, 0, 1, PAL, true)
eq(RECTS, { { 0, 0, 1, 1, 0xFF0000 } }, "flip mirrors the row horizontally")
RECTS = {}
AcidSprite.draw({ "RRB" }, 0, 0, 1, PAL, true)
eq(RECTS, { { 0, 0, 1, 1, 0x0000FF }, { 1, 0, 2, 1, 0xFF0000 } }, "flip preserves run merging in mirrored order")
