# 5. Sound

[← Graphics](04-graphics.md) · [Contents](README.md) · [Next: Games →](06-games.md)

Acid OS v3 has a **hand-built 8-voice subtractive synthesiser** running in the
kernel (the `acid-synth` crate). There are no samples, no audio files, no codecs.
Everything the OS makes a noise with (every game's sound effects, the Piano
app, the Terminal's easter eggs) comes out of ten functions: `acid_play_note`,
`acid_stop_note`, `acid_configure_voice`, `acid_configure_osc`,
`acid_configure_filter`, `acid_set_ring_partner`, `acid_trigger_arp`,
`acid_set_volume`, `acid_get_volume` and `acid_active_voice_count`.

The design is openly modelled on the **MOS 6581 SID**, the sound chip in the
Commodore 64: pulse/saw/triangle/noise oscillators, a per-voice ADSR envelope,
one shared resonant multi-mode filter with per-voice routing, ring modulation
wired into the triangle generator, and an arpeggiator for chords a single voice
can fake. The whole thing is integer arithmetic at 22,050 Hz, unsigned 8-bit
mono; no floating point is involved.

If you have ever written a SID tune, you already know this instrument.

## 5.1 The mental model

```text
   voice 0  ──┬─(route 0)──────────────────────────┐
   voice 1  ──┤                                    │
   voice 2  ──┤                                    ├─► master volume ─► speaker
   ...        │                                    │
   voice 7  ──┴─(route 1)──► shared filter ────────┘
                              cutoff / resonance / LP|BP|HP
```

- **8 voices**, numbered **0–7**. Each has its own waveform, pitch, envelope,
  duty cycle, ring-mod partner, arpeggio and filter routing.
- **One filter**, shared by every voice. Its cutoff, resonance and mode are
  *global* settings: changing them changes them for everyone. Which voices go
  through it is *per-voice*.
- **One master volume**, 0–100, applied to the final mix. A system setting; the
  Config app owns it.

### Calls take effect between audio buffers

Every audio function changes the synthesiser's state under a lock that the audio
callback holds while it renders a buffer, so a call lands **between buffers**:
a fraction of a second at most, never mid-sample. Nothing you call returns a
result about what the synth did.

(Commands are applied directly, so there is no queue to overflow. Even so, do
not design something that depends on a single `acid_stop_note` arriving, because
it is cheap to make the design not need it.)

**Ordering within your own app is preserved**, so `acid_play_note` followed by
`acid_trigger_arp` on the same voice does what you expect.

### Voice ownership, and voice stealing

The kernel records which task last gated on each voice, and releases every voice
your app owns when your app ends, on **every** exit path, including a Lua error.
You cannot leave a note droning after your window closes. (If another app has
taken a voice from you by playing a note on it, that voice is theirs now and
your exit leaves it alone.)

What the kernel does **not** do is stop another app taking a voice you are
using. `acid_stop_note(3)` silences voice 3 no matter who started it. There is no
allocator and no reservation. The convention is simply that apps pick different
voices:

| Voice | Used by |
|---|---|
| 0, 1 | Acid Blaster (hit, game over) |
| 2, 3, 4 | Breakout (brick, paddle, game over) |
| 5 | Piano |
| 6 | Tetris, and the Terminal's easter eggs |
| **7** | **free** |

If you are writing one app and nothing else will be making noise, use whatever
you like. If you want to be a good citizen, use 7, or pick the highest numbers
you need and document them in your source the way the games do.

## 5.2 Pitch: the `ona` scale

Pitch is an integer called `ona`, using **standard 88-key piano numbering**:

- `ona = 1` is **A0**, the lowest key (27.5 Hz)
- `ona = 49` is **A4**, concert pitch (440.0 Hz)
- `ona = 88` is **C8**, the top key (4186 Hz)
- `freq(n) = 440 × 2^((n − 49) / 12)`

Values outside 1–88 are ignored: the note simply does not change pitch.

`ona` is MIDI note number minus 20, if that is a scale you already think in.

### The table

|  | C | C♯ | D | D♯ | E | F | F♯ | G | G♯ | A | A♯ | B |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| **0** | – | – | – | – | – | – | – | – | – | **1** | 2 | 3 |
| **1** | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 | 14 | 15 |
| **2** | 16 | 17 | 18 | 19 | 20 | 21 | 22 | 23 | 24 | 25 | 26 | 27 |
| **3** | 28 | 29 | 30 | 31 | 32 | 33 | 34 | 35 | 36 | 37 | 38 | 39 |
| **4** | **40** | 41 | 42 | 43 | 44 | 45 | 46 | 47 | 48 | **49** | 50 | 51 |
| **5** | 52 | 53 | 54 | 55 | 56 | 57 | 58 | 59 | 60 | 61 | 62 | 63 |
| **6** | 64 | 65 | 66 | 67 | 68 | 69 | 70 | 71 | 72 | 73 | 74 | 75 |
| **7** | 76 | 77 | 78 | 79 | 80 | 81 | 82 | 83 | 84 | 85 | 86 | 87 |
| **8** | **88** | – | – | – | – | – | – | – | – | – | – | – |

Middle C (C4) is **40**. Each octave is 12.

### Working in semitones

Music is intervals, so most code picks a root and adds. Piano does exactly this:

```lua snippet
local ROOT_ONA = 40                                  -- C4
local WHITE_OFFSETS = { 0, 2, 4, 5, 7, 9, 11 }       -- C D E F G A B
local BLACK_OFFSETS = { 1, 3, 6, 8, 10 }             -- C# D# F# G# A#

acid_play_note(VOICE, ROOT_ONA + offset, 45)
```

Useful intervals, in semitones:

| Interval | Semitones | | Interval | Semitones |
|---|---|---|---|---|
| minor 2nd | 1 | | perfect 5th | 7 |
| major 2nd | 2 | | minor 6th | 8 |
| minor 3rd | 3 | | major 6th | 9 |
| major 3rd | 4 | | minor 7th | 10 |
| perfect 4th | 5 | | major 7th | 11 |
| tritone | 6 | | octave | 12 |

Chords, as offsets from a root: major `{ 0, 4, 7 }`, minor `{ 0, 3, 7 }`,
diminished `{ 0, 3, 6 }`, major 7th `{ 0, 4, 7, 11 }`, minor 7th `{ 0, 3, 7, 10 }`.

If you want a note-name helper in your own app:

```lua snippet
local Notes = {}
local NAMES = { C = 0, ["C#"] = 1, D = 2, ["D#"] = 3, E = 4, F = 5,
                ["F#"] = 6, G = 7, ["G#"] = 8, A = 9, ["A#"] = 10, B = 11 }

-- Notes.ona("A", 4) --> 49     Notes.ona("C", 4) --> 40
function Notes.ona(name, octave)
  return 12 * octave + NAMES[name] - 8
end
```

## 5.3 Playing a note

```lua snippet
acid_play_note(voice, ona, volume)
acid_stop_note(voice)
```

| Argument | Range | Meaning |
|---|---|---|
| `voice` | 0–7 | Which voice. Out of range is ignored. |
| `ona` | 1–88 | Pitch. Out of range leaves the pitch unchanged. |
| `volume` | 0–100 | How loud this note holds. Clamped. |

`acid_play_note` does three things: sets the voice's pitch, sets its **sustain
level** from `volume`, and gates the envelope on, restarting it from zero, so
retriggering a sounding voice replays the attack rather than continuing.

`acid_stop_note` gates the envelope off, which starts its release phase, and
stops any arpeggio the voice was running.

```lua snippet
acid_play_note(7, 49, 60)    -- A4, at 60
-- ... later ...
acid_stop_note(7)            -- release it
```

### `volume` *is* the sustain level

This trips people up. `acid_configure_voice` takes a `sustain_percent`, but
**every `acid_play_note` overwrites it** with that call's `volume`. So:

- `acid_configure_voice`'s `sustain_percent` only matters for a note gated on
  *without* going through `acid_play_note`, which from Lua never happens.
- Per-note dynamics are exactly what `volume` is for.

Pass something sensible to `acid_configure_voice` for readability, and treat
`volume` as the real control.

### Nothing stops a note but you

There is **no note length**. A gated-on voice sustains at `volume` until
something calls `acid_stop_note` on it. A decay to silence is only silent
because you set `sustain_percent`/`volume` such that the envelope falls to
zero, and even then the voice is still gated on.

This is the single most important thing to internalise about this synthesiser,
and [§6.4](06-games.md#64-the-sound-effect-lifecycle) is about the bug it
causes.

### A voice with no pitch

A voice that has never had a pitch set has a phase increment of zero: its
oscillator never advances, so it would output a constant, non-silent value, a
full-scale DC thump instead of a note. `acid_play_note` with an out-of-range
`ona` on a fresh voice therefore **silently does nothing** rather than clicking.
Always play a real note first; an arpeggiating voice is the one exception, since
its pitch comes from the pattern.

## 5.4 Shaping the voice: `acid_configure_voice`

```lua snippet
acid_configure_voice(voice, filter_route, attack_ms, decay_ms, sustain_percent, release_ms)
```

| Argument | Range | Meaning |
|---|---|---|
| `voice` | 0–7 | |
| `filter_route` | 0 or 1 | Non-zero routes this voice through the shared filter; 0 goes straight to the output. |
| `attack_ms` | ms | Time from silence to full level after gate-on. 0 (or less) is instant. |
| `decay_ms` | ms | Time from full level down to the sustain level. |
| `sustain_percent` | 0–100 | Held level, **overwritten by `acid_play_note`'s `volume`**. Clamped. |
| `release_ms` | ms | Time from the current level to silence after gate-off. |

Times are capped at 100,000 ms (100 seconds). This is a classic **ADSR** envelope:

```text
 level
   ^
   |     /\
   |    /  \______________            <- sustain (= volume)
   |   /                  \
   |  /                    \
   +--------------------------------> time
     A    D       S         R
     ^                      ^
     gate on              gate off
```

### The defaults are deliberately unusable

A fresh voice is a **raw 50% pulse wave with instant attack, instant decay,
instant release, unfiltered**. That is a hard, buzzy square-wave blip: correct
as a neutral starting point, wrong as a sound. Configure every voice you use in
`on_create`.

### Envelope recipes

```lua snippet
-- Percussive blip: a UI click, a brick breaking
acid_configure_voice(v, 1, 2, 30, 50, 50)

-- Sharper, shorter: a laser, a hit
acid_configure_voice(v, 1, 3, 40, 55, 70)

-- Sustained: a held piano key that fades on release
acid_configure_voice(v, 1, 5, 80, 60, 120)

-- Slow pad: swells in, hangs, fades out
acid_configure_voice(v, 1, 400, 300, 70, 600)

-- Long descending sting: a game-over
acid_configure_voice(v, 1, 8, 150, 35, 250)
```

Millisecond values are honoured to a genuinely fine resolution; the envelope
runs in a scaled-up fixed-point representation internally so that a 500 ms decay
really is 500 ms and not a badly quantised approximation.

## 5.5 The oscillator: `acid_configure_osc`

```lua snippet
acid_configure_osc(voice, waveform, duty_percent)
```

`AcidWaveform` is always loaded: you never list it in `libs`. It is just names
for the four numbers:

```lua snippet
AcidWaveform = {
  PULSE    = 0,
  SAW      = 1,
  TRIANGLE = 2,
  NOISE    = 3,
}
```

| Waveform | Character | Good for |
|---|---|---|
| `PULSE` | Hollow, reedy; timbre varies hugely with duty | Leads, basses, chiptune everything |
| `SAW` | Bright, buzzy, full of harmonics | Acid basslines, brass, anything through a resonant filter |
| `TRIANGLE` | Soft, flute-like, few harmonics | Mellow basses, bells (with ring mod) |
| `NOISE` | White noise | Drums, explosions, wind, hi-hats |

`duty_percent` is **clamped to 1–99** and is audible **only on `PULSE`**. The
default is 50%, a pure square. An unknown waveform number leaves the waveform as
it was.

| Duty | Sound |
|---|---|
| 50 | Square: hollow, woody |
| 25 / 75 | Brighter, reedier (25 and 75 are identical in timbre) |
| 12 or 88 | Thin, nasal, very bright |
| 5 or 95 | Buzzy and sharp, almost a saw |

```lua snippet
acid_configure_osc(v, AcidWaveform.PULSE, 12)     -- thin nasal lead
acid_configure_osc(v, AcidWaveform.SAW, 50)       -- duty ignored
acid_configure_osc(v, AcidWaveform.NOISE, 50)     -- duty ignored
```

Sweeping duty over time is the classic pulse-width-modulation sound and costs
you nothing but a call per frame:

```lua snippet
function MyGame:on_tick()
  self.pwm = (self.pwm + 2) % 90
  acid_configure_osc(MyGame.LEAD_VOICE, AcidWaveform.PULSE, 5 + self.pwm)
end
```

## 5.6 The filter: `acid_configure_filter`

```lua snippet
acid_configure_filter(cutoff, resonance, mode)
```

| Argument | Range | Meaning |
|---|---|---|
| `cutoff` | 0–255 | Corner frequency. Low is dark, high is bright. Clamped. |
| `resonance` | 0–15 | Emphasis at the corner. 0 is heavily damped; 15 is a sharp, whistling peak. Clamped. |
| `mode` | bitmask 0–7 | Which outputs to sum. |

There are no Lua names for the modes; declare them yourself, as Piano does with its `Piano.FILTER_MODE_LP` class field (these examples use locals):

```lua snippet
local FILTER_MODE_LP = 1   -- low-pass:  keeps lows, removes highs
local FILTER_MODE_BP = 2   -- band-pass: keeps a band around the cutoff
local FILTER_MODE_HP = 4   -- high-pass: keeps highs, removes lows
```

Modes combine: `FILTER_MODE_LP | FILTER_MODE_HP` is a notch, `0` mutes every
routed voice. Before anyone configures it the filter is a low-pass at cutoff 128
with resonance 0.

Under the hood this is a **Chamberlin state-variable filter**, the same
topology as the SID's, with a resonance sweep from Q ≈ 0.707 (damped) to Q = 8.0
(a sharp resonant peak).

### It is one filter, shared

There is exactly one filter for the whole system. `acid_configure_filter` is
global: change it and you change it for every routed voice in every app. What is
per-voice is only *whether a voice goes through it*, set by
`acid_configure_voice`'s `filter_route`.

In practice apps set a mild low-pass in `on_create` and leave it alone:

```lua snippet
acid_configure_filter(180, 3, FILTER_MODE_LP)
```

That single line is what turns the harsh default pulse into something warm, and
it is why every sound-making app in the tree opens with it.

Sweeping the cutoff is the *acid* sound. It is also the most antisocial thing
you can do to another app's audio, so sweep only while your app is the one
making noise:

```lua snippet
function MyGame:on_tick()
  if not self:focused() then return end
  self.cutoff = 40 + ((self.cutoff + 6) % 200)
  acid_configure_filter(self.cutoff, 12, FILTER_MODE_LP)
end
```

## 5.7 Ring modulation: `acid_set_ring_partner`

```lua snippet
acid_set_ring_partner(voice, partner)   -- partner 0-7, or negative to clear
```

Ring modulation multiplies one oscillator against another, producing clangorous,
inharmonic sum-and-difference tones: bells, gongs, metallic percussion, the
sound of a C64 game hitting something hard.

**Audible only on a `TRIANGLE` voice.** This is not a limitation of the binding;
it matches the real SID, whose ring modulator is wired directly into the triangle
generator's fold-direction bit rather than being a general-purpose effect. Set it
on a pulse or saw voice and nothing happens. A partner above 7 is ignored.

The *partner* voice contributes only its oscillator phase. It does not need to be
gated on, and you will not hear it separately unless you play it too.

```lua snippet
local BELL = 4
local MOD  = 5

function MyGame:on_create()
  acid_configure_osc(BELL, AcidWaveform.TRIANGLE, 50)
  acid_configure_voice(BELL, 1, 2, 400, 30, 500)
  acid_set_ring_partner(BELL, MOD)
  acid_play_note(MOD, 63, 0)      -- partner's pitch sets the modulation ratio
end

function MyGame:clang()
  acid_play_note(BELL, 52, 70)
end
```

Change the partner's pitch to change the character completely: an octave apart
is mild, a tritone apart is a harsh clang. Clear it with a negative partner.

## 5.8 The arpeggiator: `acid_trigger_arp`

```lua snippet
acid_trigger_arp(voice, note0, note1, note2, note3, count, rate_ms)
```

A single voice steps through **up to four pitches**, cycling upward with
wraparound, at `rate_ms` per step. This is how an 8-bit machine plays a chord
with one voice, and it is how most of this OS's sound effects get their
character.

| Argument | Range | Meaning |
|---|---|---|
| `voice` | 0–7 | |
| `note0`–`note3` | 1–88 | **Absolute `ona` values**, not offsets. All four arguments are required. |
| `count` | 2–4 | How many slots to use. Unused slots must still be valid values: pass `1`. |
| `rate_ms` | ms | Time per step. 0 or less becomes one audio sample; capped at 10,000 ms. |

A voice the kernel releases when its owner exits also drops its arpeggio, so
the next app to play it hears a plain note.

A `count` outside 2–4 is ignored entirely, and **1 is outside it**: a one-note
"arpeggio" is not an arpeggio, so use `acid_play_note`.

**Call it immediately after `acid_play_note` on the same voice.** `acid_play_note`
sets the volume, envelope and gate; `acid_trigger_arp` drives pitch-stepping on
top. The pattern always restarts at slot 0 on gate-on.

```lua snippet
-- A chord, fast enough to hear as one sound
acid_play_note(v, 40, 60)
acid_trigger_arp(v, 40, 44, 47, 52, 4, 30)     -- C major, 30ms per note

-- A two-note descending blip: a brick breaking
acid_play_note(v, 60, 45)
acid_trigger_arp(v, 60, 57, 1, 1, 2, 40)       -- only 2 slots used

-- A long descending game-over run
acid_play_note(v, 30, 50)
acid_trigger_arp(v, 30, 27, 23, 18, 4, 110)
```

### The arpeggio does not stop by itself

It cycles **forever** until `acid_stop_note` gates the voice off, and
`acid_stop_note` is also the only thing that clears it: a later plain
`acid_play_note` on the same voice keeps arpeggiating. A four-note run at 110 ms
is 440 ms for one pass, and then it starts again. If you meant "play this run
once", you must stop the note after one pass:

```lua snippet
local RATE_MS = 110
local COUNT = 4
local TICKS = (RATE_MS * COUNT + TICK_MS - 1) // TICK_MS   -- ticks for one pass, rounded up
```

See [§6.4](06-games.md#64-the-sound-effect-lifecycle).

## 5.9 Master volume and metering

```lua snippet
acid_set_volume(percent)       -- 0-100, system-wide
acid_get_volume()              -- => 0-100
acid_active_voice_count()      -- => 0-8
```

`acid_set_volume` is a **system setting**, not yours. The Config app owns it. It
is open to carts like any built-in app, but that is no reason to use it: do not
turn the user's volume down because your app is loud. Scale your own `volume`
arguments instead. A value outside 0–100 is clamped.

`acid_active_voice_count` reports how many voices have a sounding envelope as of
the last audio buffer. It is a diagnostic (the System Monitor shows it), and it
is genuinely useful while developing:

```lua snippet
acid_draw_text("VOICES " .. acid_active_voice_count(), 6, 20, TEXT_COLOR, BG_COLOR)
```

If that number never returns to zero after your sounds finish, you have left a
voice gated on.

## 5.10 Patch book

Complete, paste-ready settings. Each assumes you have already set a filter, or
sets its own.

### UI click

```lua snippet
acid_configure_osc(V, AcidWaveform.PULSE, 25)
acid_configure_voice(V, 1, 1, 20, 40, 30)
acid_play_note(V, 64, 35)     -- then stop after ~2 ticks
```

### Laser / shot

```lua snippet
acid_configure_osc(V, AcidWaveform.SAW, 50)
acid_configure_voice(V, 1, 2, 60, 20, 40)
acid_play_note(V, 70, 50)
acid_trigger_arp(V, 70, 58, 46, 34, 4, 15)    -- fast downward sweep
```

### Explosion

```lua snippet
acid_configure_osc(V, AcidWaveform.NOISE, 50)
acid_configure_voice(V, 1, 1, 250, 0, 300)
acid_configure_filter(90, 6, FILTER_MODE_LP)
acid_play_note(V, 20, 80)
```

### Hi-hat

```lua snippet
acid_configure_osc(V, AcidWaveform.NOISE, 50)
acid_configure_voice(V, 1, 1, 25, 0, 20)
acid_play_note(V, 80, 40)
```

### Acid bassline

The one the OS is named after. Saw wave, resonant low-pass, cutoff swept every
frame.

```lua snippet
local BASS = 7

function MyGame:on_create()
  acid_configure_osc(BASS, AcidWaveform.SAW, 50)
  acid_configure_voice(BASS, 1, 2, 120, 60, 80)
  self.cutoff = 30
  self.up = true
end

function MyGame:on_tick()
  self.cutoff = self.cutoff + (self.up and 5 or -5)
  if self.cutoff > 220 or self.cutoff < 30 then self.up = not self.up end
  acid_configure_filter(self.cutoff, 13, 1)      -- high resonance is the whole point
end

function MyGame:note(ona)
  acid_play_note(BASS, ona, 70)
end
```

### Bell

```lua snippet
acid_configure_osc(V, AcidWaveform.TRIANGLE, 50)
acid_configure_voice(V, 0, 2, 500, 20, 600)   -- unfiltered: keep the highs
acid_set_ring_partner(V, V + 1)
acid_play_note(V + 1, 63, 0)
acid_play_note(V, 52, 70)
```

### Warm pad

```lua snippet
acid_configure_osc(V, AcidWaveform.PULSE, 40)
acid_configure_voice(V, 1, 500, 400, 60, 800)
acid_configure_filter(120, 2, FILTER_MODE_LP)
acid_play_note(V, 40, 45)
```

## 5.11 Worked example: a drum-and-bass box

A complete app. Four pads across the bottom fire drum sounds; the top half is a
step sequencer for a bassline that runs on its own tick. Save the script as
`v3/apps/rhythm.lua` with the manifest below and restart the OS, and it is in
the Menu.

`v3/apps/rhythm.app.toml`:

```toml
name = Rhythm
w = 260
h = 180
desc = Drum pads and a step bassline
menu = true
```

`v3/apps/rhythm.lua`:

```lua app
-- w: 260
-- h: 180
local RhythmApp = AcidGame:extend("RhythmApp")

local WINDOW_W = 260
local WINDOW_H = 180
local TITLE_BAR_H = 16
RhythmApp.TICK_MS = 50
local TICK_MS = RhythmApp.TICK_MS

local BG = 0x050607
local MUTED = 0x9DAAA3
local ACCENT = 0x00FF66

-- Voices. 7 is the one no shipped app claims; the rest are borrowed
-- on the assumption nothing else is making noise while this runs.
local BASS_VOICE = 7
local KICK_VOICE = 4
local SNARE_VOICE = 3
local HAT_VOICE = 2
local STAB_VOICE = 1

local FILTER_LP = 1

local STEPS = 16
local STEP_TICKS = 4                 -- 4 ticks x 50ms = 200ms per step

-- Offsets from the root, one per step. false is a rest (a nil would leave a
-- hole in the sequence). The step number itself stays 0-based; the +1 is
-- added where PATTERN is indexed.
local PATTERN = { 0, false, 12, 0, false, 7, 0, false,
                  3, false, 10, 3, false, 7, 15, false }
local ROOT_ONA = 28                  -- C3

local PAD_H = 34
local PAD_Y = WINDOW_H - PAD_H - 6
local PAD_W = WINDOW_W // 4
local PAD_LABELS = { "KICK", "SNARE", "HAT", "STAB" }

local GRID_Y = TITLE_BAR_H + 26
local CELL_W = (WINDOW_W - 12) // STEPS
local CELL_H = 26

function RhythmApp:on_create()
  acid_configure_filter(150, 4, FILTER_LP)

  acid_configure_osc(BASS_VOICE, AcidWaveform.SAW, 50)
  acid_configure_voice(BASS_VOICE, 1, 2, 120, 60, 60)

  acid_configure_osc(KICK_VOICE, AcidWaveform.TRIANGLE, 50)
  acid_configure_voice(KICK_VOICE, 0, 1, 90, 0, 60)

  acid_configure_osc(SNARE_VOICE, AcidWaveform.NOISE, 50)
  acid_configure_voice(SNARE_VOICE, 1, 1, 90, 0, 60)

  acid_configure_osc(HAT_VOICE, AcidWaveform.NOISE, 50)
  acid_configure_voice(HAT_VOICE, 0, 1, 25, 0, 20)

  acid_configure_osc(STAB_VOICE, AcidWaveform.PULSE, 20)
  acid_configure_voice(STAB_VOICE, 1, 2, 60, 50, 70)

  self.step = 0
  self.step_counter = 0
  self.cutoff = 60
  self.cutoff_up = true
  self.touch_down = false
  self.sfx = {}
  self.drawn_step = nil
  self.needs_full = true
end

-- Every sounding voice is registered here with a tick countdown, and
-- tick_sfx is the ONLY place a note is ever stopped. See section 6.4.
function RhythmApp:fire(voice, ona, volume, ticks, arp, rate_ms)
  -- Drop any pending countdown for this voice first, or the older
  -- entry's stop would cut the new note short.
  for i = #self.sfx, 1, -1 do
    if self.sfx[i].voice == voice then table.remove(self.sfx, i) end
  end
  acid_play_note(voice, ona, volume)
  if arp then
    acid_trigger_arp(voice, arp[1], arp[2], arp[3], arp[4], #arp, rate_ms)
  end
  self.sfx[#self.sfx + 1] = { voice = voice, ticks = ticks }
end

function RhythmApp:tick_sfx()
  for i = #self.sfx, 1, -1 do
    local s = self.sfx[i]
    s.ticks = s.ticks - 1
    if s.ticks <= 0 then
      acid_stop_note(s.voice)
      table.remove(self.sfx, i)
    end
  end
end

function RhythmApp:stop_all_sfx()
  if not self.sfx then return end
  for _, s in ipairs(self.sfx) do acid_stop_note(s.voice) end
  self.sfx = {}
end

function RhythmApp:kick()  self:fire(KICK_VOICE, 16, 85, 3) end
function RhythmApp:snare() self:fire(SNARE_VOICE, 45, 60, 3) end
function RhythmApp:hat()   self:fire(HAT_VOICE, 80, 35, 1) end

function RhythmApp:stab()
  -- One pass of a four-note arp, then stopped: 4 notes x 40ms = 160ms,
  -- which at TICK_MS 50 is 4 ticks (rounded up from 3.2).
  self:fire(STAB_VOICE, 52, 55, 4, { 52, 56, 59, 64 }, 40)
end

function RhythmApp:on_tick()
  -- The bassline sequencer.
  self.step_counter = self.step_counter + 1
  if self.step_counter >= STEP_TICKS then
    self.step_counter = 0
    self.step = (self.step + 1) % STEPS
    local offset = PATTERN[self.step + 1]
    if offset then self:fire(BASS_VOICE, ROOT_ONA + offset, 70, STEP_TICKS - 1) end
  end

  -- The acid sweep.
  self.cutoff = self.cutoff + (self.cutoff_up and 4 or -4)
  if self.cutoff > 210 then self.cutoff_up = false end
  if self.cutoff < 40 then self.cutoff_up = true end
  acid_configure_filter(self.cutoff, 12, FILTER_LP)

  self:tick_sfx()
  if self:focused() then self:draw() end
end

function RhythmApp:on_touch(x, y, pressed)
  if not pressed then
    self.touch_down = false
    return
  end
  if self.touch_down then return end   -- one hit per press, not per frame
  self.touch_down = true
  if y < PAD_Y then return end

  local pad = math.max(0, math.min(3, x // PAD_W))
  if pad == 0 then self:kick()
  elseif pad == 1 then self:snare()
  elseif pad == 2 then self:hat()
  else self:stab() end
end

function RhythmApp:on_key(code, pressed)
  if not pressed then return end
  if code == AcidKeys.ESCAPE then
    self:stop_all_sfx()
    self.step = 0
    self.needs_full = true
  end
end

function RhythmApp:on_destroy()
  self:stop_all_sfx()
end

function RhythmApp:draw()
  if self.needs_full then
    acid_clear_user_area()
    acid_draw_window_frame(self:window_title())
    acid_draw_text("STEP BASSLINE", 6, TITLE_BAR_H + 8, MUTED, BG)
    self:draw_pads()
    acid_draw_window_border()
    self.needs_full = false
    self.drawn_step = nil
  end
  if self.drawn_step ~= self.step then self:draw_grid() end
end

function RhythmApp:draw_grid()
  self.drawn_step = self.step
  for i = 0, STEPS - 1 do
    local x = 6 + i * CELL_W
    local offset = PATTERN[i + 1]
    local colour
    if i == self.step then
      colour = ACCENT
    elseif offset then
      colour = AcidPalette.hue(offset * 18)
    else
      colour = 0x0B1712
    end
    acid_fill_rect(x, GRID_Y, CELL_W - 1, CELL_H, colour)
  end
end

function RhythmApp:draw_pads()
  for i = 0, 3 do
    local x = i * PAD_W
    local colour = AcidPalette.hue(i * 40 + 20)
    acid_fill_rect(x + 2, PAD_Y, PAD_W - 4, PAD_H, colour)
    acid_draw_text(PAD_LABELS[i + 1], x + 6, PAD_Y + PAD_H // 2 - 4, BG, colour)
  end
end

RhythmApp:new():start()
```

Things worth noticing in it:

- Every voice is configured in `on_create`, before anything plays.
- `fire` registers every note it starts; `tick_sfx` is the **only** place a note
  is stopped; `stop_all_sfx` exists for the reset path and for `on_destroy`.
- The stab's arp runs for exactly one pass, because its tick count was computed
  from `rate_ms × count`.
- The touch handler debounces with `touch_down`, so resting a finger on a pad
  hits it once.
- The filter sweep runs every tick: the acid.
- A rest in `PATTERN` is `false`, not `nil`. A `nil` in the middle of a Lua
  table makes `#` unreliable and `ipairs` stop early.

---

[← Graphics](04-graphics.md) · [Contents](README.md) · [Next: Games →](06-games.md)
