-- Headless tests for AcidEggs (apps/lib/acid_eggs.lua) -- the state machine
-- and geometry of the three terminal easter eggs. Every OS call it makes is
-- stubbed below, and every time-dependent entry point takes an explicit
-- now_ms, so the whole animation can be stepped deterministically with no OS
-- and no clock underneath it.

RECTS_E = {}
CLEARS = 0
OPENS = 0
CLOSES = 0
OPEN_RESULT = true
NOTES_E = {}

-- These replace the env's recording fakes (globals resolve at call time).
function acid_overlay_fill_rect(x, y, w, h, color)
  RECTS_E[#RECTS_E + 1] = { x, y, w, h, color }
end

function acid_overlay_clear()
  CLEARS = CLEARS + 1
end

function acid_overlay_open()
  OPENS = OPENS + 1
  return OPEN_RESULT
end

function acid_overlay_close()
  CLOSES = CLOSES + 1
end

function acid_play_note(voice, ona, volume)
  NOTES_E[#NOTES_E + 1] = { "play", voice, ona, volume }
end

function acid_trigger_arp(voice, n0, n1, n2, n3, count, rate_ms)
  NOTES_E[#NOTES_E + 1] = { "arp", voice, n0, n1, n2, n3, count, rate_ms }
end

function acid_stop_note(voice)
  NOTES_E[#NOTES_E + 1] = { "stop", voice }
end

function acid_configure_voice(voice, filter_route, attack_ms, decay_ms, sustain_percent, release_ms)
  NOTES_E[#NOTES_E + 1] = { "configure", voice }
end

local function reset()
  RECTS_E = {}
  CLEARS = 0
  OPENS = 0
  CLOSES = 0
  OPEN_RESULT = true
  NOTES_E = {}
  AcidEggs.abort()
  CLOSES = 0
end

-- Min / max of f(r) over a list of rects.
local function min_of(list, f)
  local m = f(list[1])
  for i = 2, #list do
    local v = f(list[i])
    if v < m then m = v end
  end
  return m
end

local function max_of(list, f)
  local m = f(list[1])
  for i = 2, #list do
    local v = f(list[i])
    if v > m then m = v end
  end
  return m
end

group("AcidEggs: starting and stopping")

reset()
eq(AcidEggs.active(), false, "inactive before anything starts")
eq(AcidEggs.start("dave", 1000), true, "dave starts")
eq(AcidEggs.active(), true, "active once started")
eq(OPENS, 1, "start opens the overlay")

reset()
eq(AcidEggs.start("nope", 1000), false, "an unknown name does not start")
eq(AcidEggs.active(), false, "and leaves the eggs inactive")
eq(OPENS, 0, "and never opens the overlay")

reset()
OPEN_RESULT = false
eq(AcidEggs.start("dave", 1000), false, "a failed overlay open does not start")
eq(AcidEggs.active(), false, "and leaves the eggs inactive")

reset()
AcidEggs.start("dave", 1000)
AcidEggs.abort()
eq(AcidEggs.active(), false, "abort stops the animation")
eq(CLOSES, 1, "abort closes the overlay")

reset()
AcidEggs.start("dave", 1000)
eq(AcidEggs.start("joe", 1000), false, "a second egg is refused while one is in flight")

group("AcidEggs: the tick guard")

reset()
AcidEggs.start("dave", 1000)
local before = CLEARS
AcidEggs.step(1000 + AcidEggs.TICK_MS - 1)
eq(CLEARS, before, "no frame is drawn before TICK_MS has elapsed")
AcidEggs.step(1000 + AcidEggs.TICK_MS)
eq(CLEARS, before + 1, "a frame is drawn once TICK_MS has elapsed")

group("AcidEggs: dave and joe fly and leave")

for _, name in ipairs({ "dave", "joe" }) do
  reset()
  AcidEggs.start(name, 0)
  local t = 0
  local frames = 0
  while AcidEggs.active() and frames < 500 do
    t = t + AcidEggs.TICK_MS
    AcidEggs.step(t)
    frames = frames + 1
  end
  eq(AcidEggs.active(), false, name .. " finishes on its own")
  eq(frames < 500, true, name .. " finishes within 500 frames (took " .. frames .. ")")
  eq(CLOSES, 1, name .. " closes the overlay exactly once")
  eq(#RECTS_E > 0, true, name .. " actually drew something")
end

group("AcidEggs: dave and joe stay on screen vertically")

-- Run each egg many times: the height and direction are random per run, and
-- a sprite half off the top or bottom of the screen is the bug this catches.
-- Both names are run, not just dave: joe has a different sprite height (11
-- rows vs dave's 10) and moves with a wobble rather than a bob, so running
-- only dave would leave joe's vertical geometry completely unexercised.
for _, name in ipairs({ "dave", "joe" }) do
  local i = 0
  while i < 40 do
    reset()
    AcidEggs.start(name, 0)
    local t = 0
    while AcidEggs.active() and t < 20000 do
      t = t + AcidEggs.TICK_MS
      AcidEggs.step(t)
    end
    local top = min_of(RECTS_E, function(r) return r[2] end)
    -- The bottom edge, not just the top: a rect's stored y is its top, so
    -- checking only that against SCREEN_H would let a sprite whose bottom
    -- row hangs off the screen still pass.
    local bottom = max_of(RECTS_E, function(r) return r[2] + r[4] end)
    eq(top >= 0, true, name .. " never draws above the top of the screen")
    eq(bottom <= AcidEggs.SCREEN_H, true, name .. " never draws below the bottom")
    i = i + 1
  end
end

group("AcidEggs: maximbady bounces then shouts")

reset()
AcidEggs.start("maximbady", 0)
-- start() calls acid_configure_voice before the animation runs a single
-- tick, and that already appends a "configure" entry to NOTES_E. Without
-- clearing here, an assertion that merely checks NOTES_E is non-empty would
-- pass even against an implementation that never bounced -- or made a sound
-- -- at all. Clearing means only sounds made DURING the animation count.
NOTES_E = {}
local t = 0
while AcidEggs.active() and t < 60000 do
  t = t + AcidEggs.TICK_MS
  AcidEggs.step(t)
end
eq(AcidEggs.active(), false, "maximbady finishes on its own")
eq(CLOSES, 1, "maximbady closes the overlay exactly once")
-- One boing per bounce (BOUNCE_LIMIT bounces) plus one slide when the word
-- phase begins, and both go through acid_play_note -- so at least
-- BOUNCE_LIMIT "play" events is the real signature of "a sound on every
-- bounce", not just "a sound was made at some point".
local plays = {}
for _, n in ipairs(NOTES_E) do
  if n[1] == "play" then plays[#plays + 1] = n end
end
eq(#plays >= AcidEggs.BOUNCE_LIMIT, true,
   "maximbady makes a sound on every bounce (got " .. #plays .. " plays)")

-- Centring is asserted against the layout box, not against the drawn pixels:
-- both glyphs have blank columns in some of their rows, so the ink's own
-- bounding box is narrower than the space the word occupies and is the wrong
-- thing to measure.
local box = AcidEggs.word_box()
eq(box[1], (AcidEggs.SCREEN_W - box[3]) // 2, "the word box is horizontally centred")
eq(box[2], (AcidEggs.SCREEN_H - box[4]) // 2, "the word box is vertically centred")
eq(box[1] >= 0 and box[1] + box[3] <= AcidEggs.SCREEN_W, true,
   "the word fits across the screen")
eq(box[2] >= 0 and box[2] + box[4] <= AcidEggs.SCREEN_H, true,
   "the word fits down the screen")

-- ...and the ink actually lands inside that box.
reset()
AcidEggs.start("maximbady", 0)
t = 0
local word_rects = {}
while AcidEggs.active() and t < 60000 do
  t = t + AcidEggs.TICK_MS
  local before_n = #RECTS_E
  AcidEggs.step(t)
  local frame = {}
  for i = before_n + 1, #RECTS_E do frame[#frame + 1] = RECTS_E[i] end
  -- The word frame is the one with far more rects than a single small figure.
  if #frame > #word_rects then word_rects = frame end
end
eq(#word_rects > 0, true, "the word phase drew something")
eq(min_of(word_rects, function(r) return r[1] end) >= box[1], true, "no glyph ink starts left of the box")
eq(max_of(word_rects, function(r) return r[1] + r[3] end) <= box[1] + box[3], true,
   "no glyph ink runs past the right of the box")
eq(min_of(word_rects, function(r) return r[2] end) >= box[2], true, "no glyph ink starts above the box")
eq(max_of(word_rects, function(r) return r[2] + r[4] end) <= box[2] + box[4], true,
   "no glyph ink runs below the box")
