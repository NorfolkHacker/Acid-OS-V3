# Acid Tracker and the Audio Language — Design

**Date:** 2026-10-06
**Status:** approved in brainstorming, awaiting spec review

## Goal

Two new things that work together:

1. **An audio language (`.snd`).** A small, integer-only scripting language
   that only does sound. It's an OS-wide feature: any app, game, the Terminal
   or the tracker can run its scripts. Scripts are compiled to bytecode and
   run inside the audio thread at a steady 50 Hz tick, so their timing never
   depends on Lua frames.
2. **Acid Tracker.** A GoatTracker-style tracker app with 4 channels and up to
   2 voices per channel. It saves songs in its own `.trk` format. The tracker
   and the language are separate, but they connect. A tracker instrument can
   be a script, and a script (or any app) can load and drive a song.

Reference: GoatTracker 2 (https://github.com/leafo/goattracker2) for the
pattern and order-list model, the command set and the keyboard layout.

## Decisions

| Question | Choice |
|---|---|
| How the language relates to the tracker | They're separate. The tracker has its own `.trk` format, and `.snd` is a standalone language. They connect through a bridge. |
| Bridge, first version | (1) Script instruments in songs. (3) Scripts and apps load and drive songs (play, stop, tempo, mute, jump, read position). |
| Bridge, later | (2) An `S` pattern command that calls a script. (4) Scripts that generate pattern data. |
| Where the engine runs | A new `no_std` Rust crate, `acid-sound`, stepped inside the kernel's `render_audio`. The tracker UI is a Lua app on top. |
| Tick rate | 50 Hz, one tick every 441 samples at 22050 Hz (PAL timing, as in GoatTracker) |
| Channels and voices | 4 channels. Channel *n* (1-based) uses synth voices 2(n-1) and 2(n-1)+1, which is all 8. |
| What voice 2 is for | Both. Each row has an optional second note column. If it holds a note, voice 2 plays it. If it's empty, the instrument may use voice 2 (detune, octave, ring and so on). |
| Sound effects during a song | They take a free voice. If none is free, they borrow the donor channel's voice 2 (channel 4 by default, set with `sfx-donor`) and hand it back when they end. |
| Existing audio calls | They don't change. `acid_play_note` and the others write straight to voices, and last write wins. |
| Mixer headroom | `acid-synth` gains optional mixer headroom, `set_mix_shift` (default 0). The engine sets it to 2 while a song plays, because a song sums many full-scale voices. Apps' direct `acid_play_note` voices are about 12 dB quieter during a song. |
| Song file format | A versioned text format in the `.spr` house style |
| Window | 480×320, resizable, minimum 420×240. It doesn't opt into Large font. |
| Keys | GoatTracker-like piano keys, F1–F8 for transport and mutes (needs a platform change), an Esc command bar like the Editor's for file and song commands |

## Out of scope (this spec)

- The `S` pattern command that calls a script (phase 2 of the bridge). The
  command letter is reserved now.
- Scripts that write pattern data (bridge item 4).
- Exporting songs to WAV or other formats. Importing GoatTracker `.sng` files.
- More than one song playing at once.
- WASM carts calling the song and sound API. Only Lua apps get it first. The
  calls live in `acid-api`, so they can be wired into `acid-wasm` later.
- Changing `acid-synth`'s DSP, apart from the optional mixer headroom
  (`set_mix_shift`, default 0; see Decisions). Sub-semitone pitch writes the
  existing public `phase_increment` field.

## 1. Architecture

```
Acid Tracker (Lua UI)      games / Terminal / any Lua app
        │   acid_song_*, acid_sound_*   │
        └───────────────┬───────────────┘
                   acid-api  (+ acid-lua bindings)
                        │
     acid-sound (new, no_std + alloc)
       compiler · bytecode VM · song player · voice allocator
                        │  tick() every 441 samples
                   acid-kernel audio (AudioState)
                        │
                   acid-synth (8 voices + filter; adds mixer headroom)
```

- **`acid-sound`** is a new workspace crate. It depends on `acid-synth` and
  nothing else. It uses `alloc` for programs and song data, but the tick path
  never allocates.
- **The kernel's `AudioState`** gains an `acid_sound::Engine` next to the
  `Synth`. `render_audio` renders the buffer in pieces split at tick
  boundaries. A 441-sample counter carries over between buffers, so ticks
  stay exact whatever size buffers the platform asks for. At each boundary it
  calls `engine.tick(&mut synth)` and then keeps rendering.
- **Compiling never happens in the audio thread.** `acid_sound_load` and
  `acid_song_load` parse and compile on the calling app's thread. Only the
  finished `Program` or `Song` is moved into the engine, under the audio
  lock.
- **Ownership.** Every running sound and every song records the `TaskId`
  that started it. `audio_release_owner(task)` (an app exiting) also stops
  that task's sounds and its song, and releases their voices.

## 2. The audio language (`.snd`)

### 2.1 Execution model

A script runs until it reaches `wait n`, then sleeps for `n` ticks. Each
running script is a VM instance with its own program counter, variables and
wait counter. At each engine tick, every running instance whose wait has
finished runs until its next `wait`, `stop`, the end of its block, or its
instruction budget.

- **Instruction budget.** 256 instructions per instance per tick. Using up
  the budget acts like `wait 1`, so a loop with no `wait` keeps going on the
  next tick but can't stall the audio thread.
- **Variables.** At most 64 variables per block, and `repeat` counters count
  toward them. Every value is an `i32`, and arithmetic saturates.
- **Runtime errors don't exist.** Division or modulo by zero gives 0, and
  out-of-range arguments are clamped the way the synth setters already
  clamp.

### 2.2 File layout

A file is a list of blocks. Line comments start with `#`.

```
sound NAME          # a one-shot or looping sound, started by acid_sound_play
  ...statements
end

instrument NAME     # a tracker instrument; started on each note
  ...statements
on release          # optional: runs when the tracker sends note-off
  ...statements
end
```

Names are unique within a file. A file with no blocks at all is treated as
one implicit `sound main`, so short one-off scripts in the Terminal need no
wrapper.

### 2.3 Statements

Statements are separated by a newline or `;`. A voice
prefix (`v1`, `v2`, `both`) applies to one sound command, and no prefix means
`v1`.

| Statement | Meaning |
|---|---|
| `wave saw\|pulse\|tri\|noise` | Waveform |
| `duty N` / `duty +N` | Pulse width in percent, absolute or relative |
| `adsr A D S R` | Attack, decay and release in ms, sustain in percent |
| `gate on\|off` | Start the envelope or release it |
| `pitch N` / `pitch +N` / `pitch note` | Semitone pitch on the synth's 88-key `ona` scale (1..88), absolute, relative, or the tracker's note |
| `fine N` / `fine +N` | Fine pitch offset in 1/64 semitone. Lets you write vibrato and slides. |
| `ring on\|off` | Ring-modulate with the other voice in the channel |
| `arp A B C` / `arp off` / `arprate MS` | Synth arpeggio, semitone offsets from the current pitch, up to 3 offsets after the base note |
| `filter lp\|bp\|hp CUTOFF res RES` | Shared filter (cutoff 0..255, resonance 0..15) |
| `route on\|off` | Send this voice through the filter |
| `let X = EXPR` / `X = EXPR` | Integer variables |
| `if EXPR ... else ... end` | Conditional |
| `repeat EXPR ... end` / `loop ... end` | Counted and endless loops |
| `wait EXPR` / `wait row` / `wait beat` | Sleep for ticks, or until the next song row or beat |
| `stop` | End this instance and gate off its voices |
| `song "PATH"` / `play [ORDER]` / `stop song` | Load (compiled when the script loads) and drive the song |
| `tempo N` / `mute CH` / `unmute CH` / `jump ORDER` | Song control |

**Song commands and ownership.** `stop song`, `tempo`, `mute`, `unmute`,
`jump` and `play` with no song act only on a song the same app started.
`play` of a song the script loaded replaces any playing song, as
`acid_song_play` does. A song's own script instruments share the song's
owner, so these commands work from them too, but they can't start a song.

**Expressions:** integers, variables, `+ - * / %`, `== != < <= > >=`,
`and`, `or`, `not`, parentheses, and `rand N` (0..N-1, from a per-instance
LFSR seeded from the engine tick count).

**Read-only values:** `note`, `note2` (0 if none), `tick` (ticks since
this instance started), `row`, `beat`, `order` (song position, or -1 with no
song playing).

### 2.4 Voices for a running instance

- A **sound** started by `acid_sound_play` asks the allocator for voices (one
  or two, depending on whether its program uses `v2`/`both`) when it starts.
  `v1` and `v2` are mapped to the voices it gets.
- An **instrument** started by the player gets its channel's two voices. If
  the row's second note column is filled, the player starts a **second
  instance** for voice 2. In that instance `v1` and `both` both mean voice 2,
  and `v2` commands do nothing. While the second instance runs, the first
  instance's `v2` commands also do nothing, so each voice has one owner.

### 2.5 Errors

`compile(src)` returns `Result<Program, CompileError { line, col, message }>`.
The Lua API returns `nil, "LINE:COL message"`. Messages are short and
specific, such as `3:7 unknown command 'wav'` or `12:1 'repeat' has no 'end'`.

## 3. The song format (`.trk`)

```
acid-track 1
title Acid Groove
speed 6
sfx-donor 4
instrument 01 "Lead" wave saw  adsr 0 8 70 20  duty 50  vib 4 2  arp 0 4 7  voice2 detune 6
instrument 02 "Fat Bass" script "Home/sounds/bass.snd" fatbass
order 1  00 00 01 02 loop 0
order 2  03 03+5 03 04 loop 0
order 3  05 loop 0
order 4  06 loop 0
pattern 00 16
C-3 02 . .. ...
... .. . .. ...
C-3 02 3 20 E-3
=== .. . .. ...
```

- **Header lines.** `acid-track 1` (the version, required first),
  `title`, `speed` (ticks per row, 1..31), and `sfx-donor` (1..4).
- **Built-in instruments** (`01`..`3F`):
  - `wave` and `adsr`
  - `duty` and `pwm SPEED` (pulse sweep)
  - `vib DEPTH SPEED`
  - `arp A B C` (up to 3 offsets, plus the base note)
  - `filter lp|bp|hp CUTOFF RES` (applied when the note starts)
  - `voice2` mode: `off`, `detune N`, `octave`, `fifth`, `ring`
- **Script instruments.** `script "PATH" NAME`. The path is relative to
  fsroot.
- **Order lists.** One per channel. Each entry is a pattern number, with an
  optional `+N`/`-N` transpose. `loop N` sets the restart point, and every
  list must end with `loop`.
- **Patterns.** Each one is single-channel, `pattern NN LENGTH` with 1..64
  rows, numbered `00`..`7F`.
- **Rows.** Five fields:

  | Field | Form | Meaning |
  |---|---|---|
  | Note | `C-3`, `C#3` | Start a note |
  | | `===` | Note-off |
  | | `...` | Nothing |
  | Instrument | `..` or a hex number | Instrument |
  | Command | `.` or `1`/`2`/`3`/`4`/`8`/`9`/`A`/`F` | Command, `S` is reserved |
  | Parameter | `..` or hex | Command parameter |
  | Note 2 | `...` or a note (no note-off) | Second-voice note |

- **Pattern commands:**

  | Command | Effect |
  |---|---|
  | `1 XX` | Slide up |
  | `2 XX` | Slide down |
  | `3 XX` | Glide to note at speed XX |
  | `4 XY` | Vibrato, depth X and speed Y |
  | `8 0X` | Set waveform |
  | `9 XX` | Set duty |
  | `A XX` | Filter cutoff |
  | `F XX` | Speed |

- **Loading is strict.** It checks the version and fails with
  `LINE: message` on malformed lines, an unknown version, missing patterns,
  or out-of-range numbers. Out-of-range numbers are load errors, never
  silent clamps; for example `5: pwm must be -50 to 50`. A script instrument whose file is missing or fails
  to compile **doesn't** fail the load. The instrument plays silence, and the
  load returns the song plus a list of warnings.
- **Saving** writes the canonical form, so a file saved twice is identical
  byte for byte.

## 4. The player

- Each tick:
  1. On the first tick of a row, every channel reads its row and starts new
     notes.
  2. Every channel advances its effects.
  3. Script instances run.
  4. The row tick counter moves on. After `speed` ticks, every channel steps
     to its next row.
- Pattern commands `1`–`4` (slides, glide, vibrato) act on built-in
  instruments. A script instrument controls its own pitch.
- **When a row has a note**, the channel's previous note is silenced first
  (its gates off, its ring links cleared), then the channel's instrument (from
  the row, or the last one used on the channel) starts. A built-in instrument sets the
  oscillators, ADSR and filter and gates on. A script instrument starts its
  instance or instances (§2.4), and stops any instance already playing on
  that channel. Note 2 behaves as described in §2.4.
- **Note-off (`===`)** gates off a built-in instrument, or runs a script's
  `on release` part. If there's no release part, it gates off both voices.
- **When a pattern ends**, the channel moves to its next order entry and
  wraps at `loop`.
- **Mutes** stop a channel from changing its voices and gate them off. The
  song keeps its position. A muted channel still takes its instrument column
  and still applies `F` (speed), `A` (filter cutoff) and `3`'s glide speed,
  so it rejoins in step when unmuted.
- **The filter** belongs to the song while it plays. Sounds that use
  `filter` during a song do change it, and the song's next filter change
  overrides theirs.
- **Position.** `acid_song_position()` returns `order, row, tick` for channel
  1. A `beat` is 4 rows.

## 5. API (`acid-api`, bound in `acid-lua`)

| Call | Returns | Notes |
|---|---|---|
| `acid_sound_load(src)` / `acid_sound_load_file(path)` | `prog` or `nil, err` | Compile; `song "PATH"` files load now, so a missing song is an error here |
| `acid_sound_free(prog)` | — | Up to 16 programs per task |
| `acid_sound_play(prog[, name[, note]])` | `id` or `nil` | Start a `sound` block (default: the first); `note` defaults to 40 |
| `acid_sound_stop(id)` | — | Only the caller's own sounds |
| `acid_song_load(path)` / `acid_song_parse(text)` | `song, warnings` or `nil, err` | Up to 4 songs per task |
| `acid_song_update(song, text)` | `warnings` or `nil, err` | Re-parse into the same handle; a playing copy swaps in, keeping its position |
| `acid_song_free(song)` | — | |
| `acid_song_play(song[, order[, row]])` | — | Replaces any song already playing (0-based order and row) |
| `acid_song_stop()` / `acid_song_mute(ch, on)` | — | Only affect a song the caller started |
| `acid_song_position()` | `order, row, tick` or nothing | |
| `acid_song_preview(song, ch, note, inst)` | — | Sound one note on channel `ch` (1..4); `note` 0 is note-off |

Paths given to Lua calls are full paths, as with `acid_fs_read`. Paths inside files (`script "PATH"` in a `.trk`, `song "PATH"` in a `.snd`) are relative to fsroot.

## 6. Platform change: function keys

`acid-platform` gains `KEY_F1`..`KEY_F12` (266..277). `acid-hosted`'s
`translate_key` maps `KeyCode::F1..F12` to them. The keymap test that expects
F1 to be ignored changes to expect `KEY_F1`. The Lua key constants gain the
same names, following the path the other `KEY_*` constants take.

## 7. Acid Tracker app

### 7.1 Files and manifest

- `apps/tracker.lua` with `apps/tracker.app.toml` (name "Acid Tracker",
  480×320, resizable, minimum 420×240, `menu = false` like Sprite Paint).
  Helpers are split by job under `apps/tracker/`:
  - `song.lua`: the Lua song model, plus `.trk` read and write
  - `edit.lua`: the cursor and every edit
  - `layout.lua`: where each part of the window goes
  - `cmd.lua`: the Esc command line's parser
- **The `.trk` model lives in Lua** so the tracker can edit and save. To play
  or update, it serialises to text and calls `acid_song_parse`. There's one
  parser of record (Rust), which is the one games use.
- The file is loaded and validated through the same Rust parser:
  `acid_song_parse` returns errors, and the Lua reader only runs after a clean
  parse.

### 7.2 Layout (6×8 font)

```
┌ Acid Tracker ─ groove.trk ───────────────────────────────┐
│ ORD 03/0C  ROW 0A  SPD 6  OCT 4  INS 02 Fat Bass   [EDIT] │
├──┬───────────────┬───────────────┬───────────────┬────────┤
│08│C-3 02 . .. ...│--- .. . .. ...│E-4 01 4 22 G-4│...     │
│0A│C-3 02 3 20 ...│G-2 05 . .. ...│=== .. . .. ...│C-5 03  │ ← play/edit row
├──┴──────┬────────┴───────────────┴───────────────┴────────┤
│ ORDERS  │ 1: 00 00 01 02 L0   2: 03 03+5 03 04 L0  …      │
│ INSTR   │ 02 Fat Bass  script bass.snd:fatbass            │
└─────────┴─────────────────────────────────────────────────┘
```

- **Status line.** Order, row, speed, octave, the current instrument, the
  edit or play mode, and the last message or error.
- **Pattern grid.** The cursor row is fixed in the middle and the rows scroll
  past it. Muted channels are dimmed. While playing, the grid follows
  `acid_song_position()`, and the row being played gets a hue-cycling bar.
- **Order panel and instrument panel.** These sit below the grid, and their
  height grows with the window. Built-in instruments are edited as fields. A
  script instrument shows its path and name, and gets keys to open the file
  in the Editor (`e`) or recompile it (`r`).

### 7.3 Keys and commands

| Key | What it does |
|---|---|
| `z s x d c v g b h n j m` | Notes from C to B in the current octave |
| `q 2 w 3 e r 5 t 6 y 7 u i` | The octave above |
| Space | Edit mode on or off. Off, the note keys only play the note. |
| `` ` `` | Note-off (edit mode, note column) |
| `.` or Delete | Clear the field under the cursor |
| `0`–`9`, `a`–`f` | Instrument (01–3F) and parameter digits in the grid, and pattern numbers |
| `1 2 3 4 8 9 a f` | In the command column, the command |
| Arrow keys | Move |
| Tab | Next channel, then the orders panel, then the instrument |
| `<` `>` | Octave down, up |
| `[` `]` | Previous, next instrument |
| F1 / F2 / F4 | Play from the start / play from the cursor's row / stop |
| F5–F8 | Mute or unmute channels 1–4 |
| Esc | The command line: type a command and press Enter. Esc again cancels. |

In the orders panel:
- Up and Down choose the channel, and Left and Right choose the entry;
- hex digits choose the pattern (00–7F), and a new number makes an empty pattern;
- `+` and `-` transpose the entry;
- Enter repeats the entry after itself, and Delete removes it;
- `l` makes the entry the loop point.

In the instrument panel:
- Up and Down choose a field;
- Left and Right change it by 1, and `-` and `+` by 10;
- on a script instrument, `e` opens its `.snd` file in the Editor and `r`
  reloads it.

Every change is heard at once, even while the song plays. The row being
played gets a bar that cycles through hues.

Esc opens the command line (type a command, Enter runs it):

| Command | What it does |
|---|---|
| `w [name]` | Save, or save as `Home/<name>.trk` |
| `o name` | Open `Home/<name>.trk` |
| `new` | A new song |
| `speed N` | Ticks per row, 1–31 |
| `len N` | The current pattern's length, 1–64 rows |
| `title text` | The song's title |
| `ins N` | Make (if needed) and select built-in instrument `N` (hex) |
| `ins N script PATH NAME` | Make instrument `N` the instrument block `NAME` in `PATH` |
| `name text` | The current instrument's name |
| `arp a b c` | The current built-in's arpeggio, up to 3 offsets of –48 to 48 |
| `donor N` | Which channel's second voice sound effects borrow, 1–4 |
| `q` | Quit |

`new`, `o` and `q` on an unsaved song need typing twice.

When not in edit mode, the note keys play a preview of the current
instrument through `acid_song_preview` on the cursor's channel, and releasing
the key sends note-off. This uses the existing key-release events.

### 7.4 Integration

- **File Manager** opens `.trk` files in Acid Tracker, the same way it maps
  `.spr` to Sprite Paint. `.snd` files open in the Editor.
- **Terminal** gains `play FILE`. It plays a `.snd` file's first sound, or a
  `.trk` song, and `play` on its own stops.
- **Samples** are shipped in `fsroot/Home`:
  - `music/acid_groove.trk`, a demo song that uses both built-in and script
    instruments, the second note column, and the donor channel
  - `sounds/` with a few scripts: `bass.snd`, `zap.snd`,
    `wobble.snd`, and `sync.snd` (a script that plays the song and pulses to
    `wait beat`)
  - In short: `Home/music/acid_groove.trk` and
    `Home/sounds/{bass,wobble,zap,sync}.snd`. File Manager opens `.snd`
    in the Editor.
- **Manual.** A new chapter covers the tracker and the audio language, and
  the app list, File Manager and Terminal chapters are updated.

## 8. Testing

- **`acid-sound` unit tests.**
  - Compiler: every statement, plus each error message with its line and
    column.
  - VM: waits, the budget, saturation, divide by zero, `rand` being
    deterministic.
  - Allocator: free voice, donor borrowing and giving back, release on
    owner exit.
  - Player: row timing, order wrap and transpose, note-off, note 2, mutes,
    each command.
- **`.trk` round trip.** Parse, then write, gives the same bytes for the
  sample song. Every malformed-input case returns its `LINE: message`.
- **Golden audio.** `acid-sound/tests/golden.rs` renders the sample song for
  a fixed number of samples and checks it byte for byte against a committed
  recording, the same pattern `acid-synth` uses. The single demo golden also
  covers a sound effect borrowing the donor voice mid-song, and a test checks
  that the effect is audible in it.
- **Kernel tests.**
  - The tick boundary stays exact across odd buffer sizes, for example
    100-sample and 1000-sample renders giving the same output.
  - An app exiting stops its song and sounds.
  - Old `acid_play_note` calls still match the bare synth, which keeps the
    existing `commands_match_the_bare_synth` test passing.
- **Lua tests** under `tools/`: the tracker's song model round trip, and the
  cursor and editing logic.
- **Goldens of the tracker window** with the sample song open, following the
  existing app screenshot tests.

## 9. Build order

1. **`acid-sound` language.** Lexer, compiler, VM, with unit tests.
2. **`acid-sound` player.** `.trk` parser and writer, player, allocator, and
   the golden audio tests.
3. **Kernel and API.** The engine inside `render_audio`, the `acid_sound_*`
   and `acid_song_*` calls, owner cleanup, kernel tests.
4. **Function keys.**
5. **Acid Tracker app.**
6. **Integration.** File Manager, the Terminal's `play`, samples, the manual.
