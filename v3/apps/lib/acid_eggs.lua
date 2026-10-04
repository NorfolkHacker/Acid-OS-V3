-- The terminal's three easter eggs (see apps/terminal.lua): `dave` flies a
-- superman across the screen, `joe` flies a teapot, `maximbady` bounces a
-- figure around and then shouts. All three draw on the kernel overlay --
-- over the wallpaper, the taskbar and every open window.
--
-- A module, not an app, and deliberately so: spawning an app for this would
-- cost a whole VM and its libs every time someone types `dave`. Instead the terminal drives
-- it from its own event loop (its idle hook).
--
-- Every entry point takes an optional now_ms (default acid_now_ms()) so the
-- whole animation can be stepped deterministically with no clock
-- (tools/test_acid_eggs.lua).
AcidEggs = {}

AcidEggs.SCREEN_W, AcidEggs.SCREEN_H = acid_screen_size()
AcidEggs.TICK_MS = 33     -- ~30fps
AcidEggs.SCALE = 3

-- Voices 0-4 belong to the games and 5 to the piano out of 8, so 6 is free.
AcidEggs.VOICE = 6
AcidEggs.NOTE_TICKS = 6

AcidEggs.FLY_SPEED = 7
AcidEggs.BOUNCE_SPEED = 5
AcidEggs.BOUNCE_LIMIT = 6
AcidEggs.WORD_FRAMES = 60

-- A bob table instead of math.sin: an integer pixel offset is what actually
-- gets drawn anyway. Accessed 0-based with + 1.
AcidEggs.BOB = { 0, 1, 2, 3, 3, 2, 1, 0, -1, -2, -3, -3, -2, -1 }

AcidEggs.PALETTE = {
  K = 0x101010,   -- hair, outline
  S = 0xE8B48A,   -- skin
  B = 0x2050E0,   -- superman blue / maximbady trousers
  R = 0xE01020,   -- cape / maximbady top
  Y = 0xFFD400,   -- chest emblem
  W = 0xE8E8F0,   -- teapot body
  G = 0x9AA4B0,   -- teapot shadow
  N = 0x18B830,   -- maximbady green
}

-- Flying right: cape trails to the LEFT, arms reach right. AcidSprite's
-- flip handles the other direction.
AcidEggs.DAVE = { "......KKKK....",
                  ".....KSSSSK...",
                  "RR...KSSSSK...",
                  "RRR..KSSSSK...",
                  "RRRRBBBBBBSSS.",
                  "RRRRBBYBBBSSSS",
                  "RRRRBBBBBBSSS.",
                  "RRR..BBBBB....",
                  "RR...BB..BB...",
                  ".....RR..RR..." }

-- A teapot flying right: knob and lid on top, handle loop trailing on the
-- left, spout leading on the right, outlined so it reads as a teapot.
AcidEggs.JOE = { "........KKKK........",
                 "........KWWK........",
                 "......KKKKKKKK......",
                 "..KK.KWWWWWWWWK...KK",
                 ".K..KWWWWWWWWWWK.KWK",
                 ".K.KWWWWWWWWWWWWKWK.",
                 ".K.KWWWWWWWWWWWWWK..",
                 "..KKWWWWWWWWWWWWWK..",
                 "....KGWWWWWWWWWGK...",
                 ".....KGGGGGGGGGK....",
                 "......KKKKKKKKK....." }

-- Blue trousers, red top, green head and arms, exactly as asked for.
AcidEggs.MAXIMBADY = { "..NNN..",
                       "..NNN..",
                       "N.RRR.N",
                       "NRRRRRN",
                       "NRRRRRN",
                       "..RRR..",
                       "..BBB..",
                       "..B.B..",
                       "..B.B.." }

-- The two glyphs "SOOOOOOOOO" needs, at 5x7. The built-in 6px font would
-- render the whole word 60px wide -- far too small for a screen-centre
-- gag, which is why the word is drawn as sprites like everything else.
AcidEggs.GLYPH_SCALE = 6
AcidEggs.GLYPH_GAP = 6
AcidEggs.FONT = {
  S = { ".YYYY",
        "Y....",
        "Y....",
        ".YYY.",
        "....Y",
        "....Y",
        "YYYY." },
  O = { ".YYY.",
        "Y...Y",
        "Y...Y",
        "Y...Y",
        "Y...Y",
        "Y...Y",
        ".YYY." },
}
AcidEggs.WORD = "SOOOOOOOOO"

-- Module state. egg == nil means inactive.
AcidEggs.egg = nil

function AcidEggs.names()
  return { "dave", "joe", "maximbady" }
end

function AcidEggs.active()
  return AcidEggs.egg ~= nil
end

function AcidEggs.start(name, now_ms)
  local E = AcidEggs
  if E.active() then return false end
  local known = false
  for _, n in ipairs(E.names()) do
    if n == name then known = true end
  end
  if not known then return false end
  if not acid_overlay_open() then return false end

  E.egg = name
  E.now = now_ms
  if E.now == nil then E.now = acid_now_ms() end
  -- Deliberately NOT backdated to now - TICK_MS. start() draws nothing
  -- itself, so the first visible frame always comes from the first step()
  -- call, which the terminal's event loop reaches within one poll interval
  -- regardless -- backdating would only have bought back that one frame
  -- (33ms) of latency, which is imperceptible. What backdating actually
  -- costs is real: it makes step()'s own elapsed-time guard vacuous, since
  -- any step() called after a backdated start already shows a full
  -- TICK_MS elapsed and fires immediately no matter how soon it is called.
  -- That guard exists to stop the animation free-running off however
  -- often the caller happens to poll, and a guard that can never withhold
  -- a frame is not a guard.
  E.last_ms = E.now
  E.frame = 0
  E.note_ticks = 0
  E.phase = "fly"

  acid_configure_voice(E.VOICE, 1, 4, 60, 40, 90)

  if name == "maximbady" then
    E.sprite = E.MAXIMBADY
    E.bounces = 0
    E.x = 40
    E.y = 40
    E.vx = E.BOUNCE_SPEED
    E.vy = E.BOUNCE_SPEED
    E.word_frames = 0
    E.flip = false -- nothing reads it on this path
  else
    E.sprite = (name == "dave") and E.DAVE or E.JOE
    local w = AcidSprite.width(E.sprite) * E.SCALE
    local h = AcidSprite.height(E.sprite) * E.SCALE
    -- Random height that keeps the whole sprite on screen even at the
    -- extremes of the bob, and a random direction each time.
    local bob = 3 * E.SCALE
    E.y = bob + math.random(0, E.SCREEN_H - h - 2 * bob - 1)
    E.flip = math.random(0, 1) == 0
    if E.flip then
      E.x = E.SCREEN_W
      E.vx = -E.FLY_SPEED
    else
      E.x = -w
      E.vx = E.FLY_SPEED
    end
    E.whoosh()
  end

  return true
end

function AcidEggs.step(now_ms)
  local E = AcidEggs
  if not E.active() then return end
  local now = now_ms
  if now == nil then now = acid_now_ms() end
  if now - E.last_ms < E.TICK_MS then return end
  E.last_ms = now
  E.frame = E.frame + 1

  E.tick_note()

  if E.phase == "word" then
    E.step_word()
  elseif E.egg == "maximbady" then
    E.step_bounce()
  else
    E.step_fly()
  end
end

function AcidEggs.abort()
  local E = AcidEggs
  if not E.active() then return end
  acid_stop_note(E.VOICE)
  acid_overlay_close()
  E.egg = nil
  E.phase = nil
end

-- ------------------------------------------------------------- phases

function AcidEggs.step_fly()
  local E = AcidEggs
  E.x = E.x + E.vx
  local w = AcidSprite.width(E.sprite) * E.SCALE
  if E.x > E.SCREEN_W or E.x + w < 0 then
    E.abort()
    return
  end

  local bob = E.BOB[E.frame % #E.BOB + 1] * ((E.egg == "dave") and 1 or 0)
  local wobble = 0
  if E.egg == "joe" then wobble = E.BOB[(E.frame // 2) % #E.BOB + 1] // 2 end
  acid_overlay_clear()
  AcidSprite.draw(E.sprite, E.x, E.y + bob + wobble, E.SCALE, E.PALETTE, E.flip)
end

function AcidEggs.step_bounce()
  local E = AcidEggs
  local w = AcidSprite.width(E.sprite) * E.SCALE
  local h = AcidSprite.height(E.sprite) * E.SCALE

  E.x = E.x + E.vx
  E.y = E.y + E.vy

  local bounced = false
  if E.x <= 0 then
    E.x = 0
    E.vx = -E.vx
    bounced = true
  elseif E.x + w >= E.SCREEN_W then
    E.x = E.SCREEN_W - w
    E.vx = -E.vx
    bounced = true
  end
  if E.y <= 0 then
    E.y = 0
    E.vy = -E.vy
    bounced = true
  elseif E.y + h >= E.SCREEN_H then
    E.y = E.SCREEN_H - h
    E.vy = -E.vy
    bounced = true
  end

  if bounced then
    E.bounces = E.bounces + 1
    E.boing()
  end

  if E.bounces >= E.BOUNCE_LIMIT then
    E.phase = "word"
    E.word_frames = 0
    E.slide()
    acid_overlay_clear()
    return
  end

  acid_overlay_clear()
  AcidSprite.draw(E.sprite, E.x, E.y, E.SCALE, E.PALETTE)
end

function AcidEggs.step_word()
  local E = AcidEggs
  E.word_frames = E.word_frames + 1
  if E.word_frames > E.WORD_FRAMES then
    E.abort()
    return
  end

  -- Redrawn every frame rather than drawn once: the overlay canvas is not
  -- persistent across an app's other drawing, and a single clear+draw per
  -- frame is the same path every other phase uses.
  acid_overlay_clear()
  E.draw_word()
end

-- { x, y, w, h } of the space the whole word occupies, centred on screen.
-- Separate from draw_word because the glyphs have blank columns in some of
-- their rows, so the drawn pixels' own bounding box is narrower than this
-- and is the wrong thing for anything (a test, a future backdrop) to
-- measure centring against.
function AcidEggs.word_box()
  local E = AcidEggs
  local glyph_w = 5 * E.GLYPH_SCALE
  local glyph_h = 7 * E.GLYPH_SCALE
  local total = #E.WORD * glyph_w + (#E.WORD - 1) * E.GLYPH_GAP
  return { (E.SCREEN_W - total) // 2, (E.SCREEN_H - glyph_h) // 2, total, glyph_h }
end

function AcidEggs.draw_word()
  local E = AcidEggs
  local box = E.word_box()
  local glyph_w = 5 * E.GLYPH_SCALE
  local i = 0
  while i < #E.WORD do
    AcidSprite.draw(E.FONT[E.WORD:sub(i + 1, i + 1)], box[1] + i * (glyph_w + E.GLYPH_GAP), box[2],
                    E.GLYPH_SCALE, E.PALETTE)
    i = i + 1
  end
end

-- -------------------------------------------------------------- sound

function AcidEggs.whoosh()
  local E = AcidEggs
  if E.egg == "dave" then
    acid_play_note(E.VOICE, 28, 40)
    acid_trigger_arp(E.VOICE, 28, 35, 40, 47, 4, 60)
  else
    acid_play_note(E.VOICE, 52, 35)
    acid_trigger_arp(E.VOICE, 52, 56, 52, 56, 2, 90)
  end
  E.note_ticks = E.NOTE_TICKS * 2
end

function AcidEggs.boing()
  local E = AcidEggs
  acid_play_note(E.VOICE, 40 + math.random(0, 7), 45)
  E.note_ticks = E.NOTE_TICKS
end

function AcidEggs.slide()
  local E = AcidEggs
  acid_play_note(E.VOICE, 50, 55)
  acid_trigger_arp(E.VOICE, 50, 45, 38, 31, 4, 110)
  E.note_ticks = E.NOTE_TICKS * 4
end

function AcidEggs.tick_note()
  local E = AcidEggs
  if E.note_ticks <= 0 then return end
  E.note_ticks = E.note_ticks - 1
  if E.note_ticks == 0 then acid_stop_note(E.VOICE) end
end
