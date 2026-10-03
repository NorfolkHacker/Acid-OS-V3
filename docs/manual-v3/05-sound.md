# 5. Sound

[← Graphics](04-graphics.md) · [Contents](README.md) · [Next: Games →](06-games.md)

Acid OS has its own **8-voice synthesiser**, built into the kernel (the
`acid-synth` crate). It makes sound from scratch: there are no samples, no
audio files and no codecs. Every sound in the OS comes from it, including the
games' sound effects, the Piano app and the Terminal's easter eggs.

You control it with ten functions:

- `acid_play_note` and `acid_stop_note`
- `acid_configure_voice`, `acid_configure_osc` and `acid_configure_filter`
- `acid_set_ring_partner` and `acid_trigger_arp`
- `acid_set_volume`, `acid_get_volume` and `acid_active_voice_count`

It is modelled on the **SID** (the MOS 6581), the sound chip in the Commodore 64.
Like the SID, it has:

- pulse, saw, triangle and noise waveforms
- a volume envelope (ADSR) on each voice
- one shared filter, with a choice of which voices go through it
- ring modulation, built into the triangle wave
- an arpeggiator, so one voice can fake a chord

It works entirely in whole numbers, with no floating point. The output is mono,
unsigned 8-bit, at 22,050 Hz.

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

- **8 voices**, numbered **0–7**. Each one has its own waveform, pitch,
  envelope, duty cycle, ring-mod partner, arpeggio and filter routing.
- **One filter**, shared by every voice. Its cutoff, resonance and mode are
  global, so if you change them, you change them for everyone. What each voice
  chooses for itself is whether it goes through the filter.
- **One master volume**, 0–100, applied to the final mix. This is a system
  setting, and the Config app looks after it.

### Calls take effect between audio buffers

The synthesiser makes sound in small chunks called buffers. While it is
working on a buffer, your calls wait. So each call takes effect **between
buffers**: a fraction of a second later at most, and never halfway through a
sample. None of the audio functions tell you what the synth did.

Your calls are applied directly, so there is no queue that could overflow.
Even so, don't build something that falls apart if one `acid_stop_note` goes
missing. It is easy to design it so it doesn't need to care.

**Your calls happen in the order you make them.** So `acid_play_note` followed
by `acid_trigger_arp` on the same voice does what you expect.

### Voice ownership, and voice stealing

The kernel remembers which app last started a note on each voice (in synth
terms, which app "gated it on"). When your app ends, the kernel releases every
voice you own. This happens **however** your app ends, even if it crashes with a
Lua error, so you can't leave a note droning after your window closes.

If another app has since played a note on one of your voices, that voice is
theirs now. Your app ending leaves it alone.

What the kernel does **not** do is stop another app taking a voice you are
using. `acid_stop_note(3)` silences voice 3, whoever started it. You can't
reserve a voice. Instead, apps simply agree to use different ones:

| Voice | Used by |
|---|---|
| 0, 1 | Acid Blaster (hit, game over) |
| 2, 3, 4 | Breakout (brick, paddle, game over) |
| 5 | Piano |
| 6 | Tetris, and the Terminal's easter eggs |
| **7** | **free** |

If your app is the only thing making noise, use whatever voices you like. To be
a good neighbour, use 7. If you need more, pick the highest numbers you can and
note them in your source, the way the games do.

## 5.2 Pitch: the `ona` scale

Pitch is a whole number called `ona`. It counts the keys on a **standard
88-key piano**:

- `ona = 1` is **A0**, the lowest key (27.5 Hz)
- `ona = 49` is **A4**, concert pitch (440.0 Hz)
- `ona = 88` is **C8**, the top key (4186 Hz)
- `freq(n) = 440 × 2^((n − 49) / 12)`

Values outside 1–88 are ignored, and the note keeps the pitch it had.

If you already think in MIDI note numbers, `ona` is the MIDI number minus 20.

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

Music is built from intervals, so most code picks a starting note (the root) and
adds to it. The Piano app does exactly this:

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

Some chords, as offsets from the root:

- major: `{ 0, 4, 7 }`
- minor: `{ 0, 3, 7 }`
- diminished: `{ 0, 3, 6 }`
- major 7th: `{ 0, 4, 7, 11 }`
- minor 7th: `{ 0, 3, 7, 10 }`

If you'd like to use note names in your own app, here is a small helper:

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

`acid_play_note` does three things:

1. It sets the voice's pitch.
2. It sets the voice's **sustain level** from `volume`.
3. It starts the note ("gates it on"). The envelope restarts from zero, so if
   the voice was already sounding you hear the attack again.

`acid_stop_note` ends the note ("gates it off"). The envelope moves into its
release phase, and any arpeggio on the voice stops.

```lua snippet
acid_play_note(7, 49, 60)    -- A4, at 60
-- ... later ...
acid_stop_note(7)            -- release it
```

### `volume` *is* the sustain level

This catches people out. `acid_configure_voice` takes a `sustain_percent`, but
**every `acid_play_note` overwrites it** with that call's `volume`. So:

- The `sustain_percent` you give `acid_configure_voice` only matters for a note
  started some other way than `acid_play_note`. From Lua, that never happens.
- If you want some notes louder than others, that is what `volume` is for.

Give `acid_configure_voice` a sensible value so your code reads well, but treat
`volume` as the real control.

### Nothing stops a note but you

There is **no note length**. Once a note starts, it holds at `volume` until
something calls `acid_stop_note` on that voice.

A note can fade to silence, but only because its sustain level (`volume`) is
zero, so the envelope decays to nothing. Even then, the voice is still on.

This is the most important thing to understand about this synthesiser.
[§6.4](06-games.md#64-the-sound-effect-lifecycle) covers the bug it causes and
how to avoid it.

### A voice with no pitch

A voice that has never been given a pitch can't make a proper sound. Its
oscillator never moves, so instead of a note you would get a loud thump.

To avoid that, `acid_play_note` with an out-of-range `ona` on a fresh voice
**silently does nothing**. Always play a real note first. The one exception is
an arpeggiating voice, because it gets its pitch from the pattern.

## 5.4 Shaping the voice: `acid_configure_voice`

```lua snippet
acid_configure_voice(voice, filter_route, attack_ms, decay_ms, sustain_percent, release_ms)
```

| Argument | Range | Meaning |
|---|---|---|
| `voice` | 0–7 | |
| `filter_route` | 0 or 1 | Non-zero routes this voice through the shared filter; 0 goes straight to the output. |
| `attack_ms` | ms | Time from silence to full level once the note starts. 0 (or less) is instant. |
| `decay_ms` | ms | Time from full level down to the sustain level. |
| `sustain_percent` | 0–100 | The level the note holds at. **Overwritten by `acid_play_note`'s `volume`**. Clamped. |
| `release_ms` | ms | Time from the current level to silence once the note stops. |

Times are capped at 100,000 ms (100 seconds).

Together these make a classic **ADSR** envelope (attack, decay, sustain,
release). It shapes how a note's loudness changes over time:

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

A fresh voice is a raw 50% pulse wave, with instant attack, decay and release,
and no filter. That gives you a hard, buzzy blip. It is a neutral starting
point, not a sound you want to hear.

**Configure every voice you use in `on_create`.**

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

The timings are accurate to the millisecond. A 500 ms decay really is 500 ms,
not a rough approximation.

## 5.5 The oscillator: `acid_configure_osc`

```lua snippet
acid_configure_osc(voice, waveform, duty_percent)
```

`AcidWaveform` is always loaded, so you never need to list it in `libs`. It just
gives names to four numbers:

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

`duty_percent` sets how wide the pulse is. It is **clamped to 1–99**, and you
only hear it **on `PULSE`**. The default is 50%, a pure square wave.

An unknown waveform number leaves the waveform as it was.

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

If you change the duty a little every frame, you get the classic
pulse-width-modulation sound. It only costs one call per frame:

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

The modes don't have built-in names, so declare your own. Piano uses a class
field, `Piano.FILTER_MODE_LP`. These examples use locals:

```lua snippet
local FILTER_MODE_LP = 1   -- low-pass:  keeps lows, removes highs
local FILTER_MODE_BP = 2   -- band-pass: keeps a band around the cutoff
local FILTER_MODE_HP = 4   -- high-pass: keeps highs, removes lows
```

You can combine modes. `FILTER_MODE_LP | FILTER_MODE_HP` is a notch filter, and
`0` mutes every voice that goes through the filter.

Until someone configures it, the filter is a low-pass at cutoff 128 with
resonance 0.

If you're curious, it is a Chamberlin state-variable filter, the same design as
the SID's. Resonance runs from Q ≈ 0.707 (damped) to Q = 8.0 (a sharp peak).

### It is one filter, shared

There is exactly one filter for the whole system. When you call
`acid_configure_filter`, you change it for every filtered voice in every app.
The only thing each voice decides is whether it goes through the filter, using
the `filter_route` argument to `acid_configure_voice`.

In practice, apps set a gentle low-pass in `on_create` and leave it alone:

```lua snippet
acid_configure_filter(180, 3, FILTER_MODE_LP)
```

That one line turns the harsh default pulse into something warm. It's why every
app that makes sound starts with it.

Sweeping the cutoff up and down is the *acid* sound. It is also the rudest thing
you can do to another app's audio, so only sweep while your app is the one
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

Ring modulation multiplies one oscillator by another. The result is a clanging,
metallic tone: bells, gongs, metal percussion, the sound of a C64 game hitting
something hard.

**You only hear it on a `TRIANGLE` voice.** That's how the real SID works too:
its ring modulator is built into the triangle wave rather than being a general
effect. Set it on a pulse or saw voice and nothing happens. A partner above 7 is
ignored.

The partner voice only lends its oscillator. It doesn't need to be playing, and
you won't hear it on its own unless you play it too.

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

Change the partner's pitch and the character changes completely. An octave
apart is mild; a tritone apart is a harsh clang. To clear it, pass a negative
partner.

## 5.8 The arpeggiator: `acid_trigger_arp`

```lua snippet
acid_trigger_arp(voice, note0, note1, note2, note3, count, rate_ms)
```

An arpeggio makes one voice step through **up to four pitches**, one after
another, then go back to the first and repeat. Each step lasts `rate_ms`. This
is how 8-bit machines play a chord with a single voice, and it gives most of
Acid OS's sound effects their character.

| Argument | Range | Meaning |
|---|---|---|
| `voice` | 0–7 | |
| `note0`–`note3` | 1–88 | **Actual `ona` values**, not offsets. You must pass all four. |
| `count` | 2–4 | How many of the four notes to use. Pass `1` for the unused ones, since they must still be valid. |
| `rate_ms` | ms | Time per step. 0 or less becomes one audio sample; capped at 10,000 ms. |

A `count` outside 2–4 is ignored completely, and **1 counts as outside**. For a
single note, just use `acid_play_note`.

**Call it straight after `acid_play_note` on the same voice.** `acid_play_note`
sets the volume and envelope and starts the note. `acid_trigger_arp` then steps
the pitch on top. Each time the note starts, the pattern begins again from the
first note.

When your app ends and the kernel releases its voices, their arpeggios are
cleared too, so the next app to use the voice hears a plain note.

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

It repeats **forever** until you call `acid_stop_note`. That is also the only
thing that clears it: if you call `acid_play_note` again on the same voice, it
keeps arpeggiating.

A four-note run at 110 ms takes 440 ms, and then it starts again. If you want it
to play just once, stop the note after one pass. Work out how many ticks that
takes, rounding up:

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

`acid_set_volume` is a **system setting that belongs to the user**, and the
Config app looks after it. Carts can call it just like built-in apps can, but
please don't. If your app is too loud, turn down your own `volume` arguments
instead of the user's volume. A value outside 0–100 is clamped.

`acid_active_voice_count` tells you how many voices were making sound in the
last audio buffer. The System Monitor shows it, and it's very handy while you're
developing:

```lua snippet
acid_draw_text("VOICES " .. acid_active_voice_count(), 6, 20, TEXT_COLOR, BG_COLOR)
```

If that number doesn't drop back to zero after your sounds finish, you've left a
voice playing.

## 5.10 Patch book

Complete settings you can paste straight in. Each one either sets its own
filter or assumes you've already set one.

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

The sound the OS is named after: a saw wave through a resonant low-pass filter,
with the cutoff swept every frame.

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

Here is a complete app. Four pads along the bottom play drum sounds. The top
half is a step sequencer that plays a bassline by itself.

Save the script as `v3/apps/rhythm.lua`, add the manifest below, and restart the
OS. You'll find it in the Menu.

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

Things to notice:

- Every voice is configured in `on_create`, before anything plays.
- `fire` keeps track of every note it starts. `tick_sfx` is the **only** place
  a note is stopped. `stop_all_sfx` is there for resetting and for `on_destroy`.
- The stab's arpeggio plays exactly once, because its tick count comes from
  `rate_ms × count`.
- The touch handler uses `touch_down` so that resting a finger on a pad only
  hits it once.
- The filter sweeps every tick. That's the acid.
- A rest in `PATTERN` is `false`, not `nil`. A `nil` in the middle of a Lua
  table makes `#` unreliable and stops `ipairs` early.

---

[← Graphics](04-graphics.md) · [Contents](README.md) · [Next: Games →](06-games.md)
