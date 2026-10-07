# 11. Music: Acid Tracker and the audio language

[← WASM carts](10-wasm-carts.md) · [Contents](README.md)

On top of the synthesiser from [chapter 5](05-sound.md), Acid OS has two
ways to make music:

- **Acid Tracker** is an app for writing songs. It saves them as `.trk` files.
- **The audio language** is a small scripting language just for sound. It
  saves its scripts as `.snd` files.

  A script can be a sound effect for a game, or an instrument the tracker
  plays. It can also be a conductor that starts a song and keeps time with it.

Songs and scripts both run inside the kernel's audio, 50 times a second
(PAL timing, as in GoatTracker). That keeps their timing exact whatever
your app is doing.

## 11.1 Acid Tracker

Open Acid Tracker from the File Manager's **Apps** folder, or any `.trk` file,
such as `Home/music/acid_groove.trk`. You can also type
`run acid tracker` in the Terminal. Like Sprite Paint, it isn't in the Menu.

A song has **eight tracks**, and each track plays one voice. A song is a
list of **patterns**, played in turn: the **order**. Each pattern holds all
eight tracks, so every track in it has the same length, up to 64 rows. A
song can have up to 64 patterns (00–3F), and the order up to 128 entries.

Each track's part of a row looks like this:

```text
C-4 01 4 22
```

Read left to right, the four fields are:
- a note
- an instrument
- a command
- its parameter

The window has five parts, from top to bottom:
- **The status line**, showing the order position, the pattern, the row
  (and the pattern's last row), speed, octave, current instrument and mode.
- **The pattern grid**, showing the pattern at the current order position.
  A pattern that fits is shown from row 00. The grid scrolls only when the
  cursor, or the playing row, would leave the screen, and then keeps it
  near the middle. While the song plays, the grid follows it.
- **The tracks' scroll bar.** The default window fits six of the eight
  tracks. Moving the cursor onto a track scrolls the grid sideways to show
  it, and you can also press or drag the bar under the grid. Widen the
  window to see all eight.
- **The orders panel**, showing the order: the pattern numbers in turn.
  The loop point is underlined.
- **The instrument panel**, showing the current instrument's fields.

### Keys

| Key | What it does |
|---|---|
| `z s x d c v g b h n j m` | Notes from C to B in the current octave |
| `q 2 w 3 e r 5 t 6 y 7 u i` | The octave above |
| Space | Edit mode on or off. Off, the note keys only play the note. |
| `` ` `` | Note-off (edit mode, note column) |
| `.` or Delete | In edit mode, clear the field under the cursor and move down a row |
| `0`–`9`, `a`–`f` | Instrument (01–3F) and parameter digits in the grid, and pattern numbers |
| `1 2 3 4 8 9 a f` | In the command column, the command |
| Arrow keys | Move |
| Tab | Next track, then the orders panel, then the instrument |
| `<` `>` | Octave down, up |
| `[` `]` | Previous, next instrument |
| F1 / F2 / F4 | Play from the start / play from the cursor's row / stop |
| F3 | Go to the orders panel, or back to the grid |
| F5–F12 | Mute or unmute tracks 1–8 |
| Esc | The command line: type a command and press Enter. Esc again cancels. |

In the orders panel:
- Left and Right choose the entry, and the grid shows its pattern;
- `n` makes a new, empty pattern after the entry, as long as the entry's
  pattern;
- `p` copies the entry's pattern into a new pattern after the entry;
- hex digits type the entry's pattern number (00–3F), and `+` and `-` step
  it. A number that isn't used yet makes an empty pattern;
- Enter repeats the entry after itself, and Delete removes it;
- `l` makes the entry the loop point, where the song goes back to after
  its last entry.

In the instrument panel:
- Up and Down choose a field;
- Left and Right change it by 1, and `-` and `+` by 10;
- on a script instrument, `e` opens its `.snd` file in the Editor and `r`
  reloads it.

Every change is heard at once, even while the song plays.

### Commands

| Command | What it does |
|---|---|
| `w [name]` | Save, or save as `Home/<name>.trk` (names live under `Home`) |
| `o name` | Open `Home/<name>.trk` (names live under `Home`) |
| `new` | A new song |
| `speed N` | Ticks per row, 1–31 |
| `len N` | The current pattern's length, 1–64 rows. It changes every track in the pattern. |
| `clean` | Drop the patterns the order doesn't play, so their numbers are free again |
| `title text` | The song's title |
| `ins N` | Make (if needed) and select built-in instrument `N` (hex) |
| `ins N script PATH NAME` | Make instrument `N` the instrument block `NAME` in `PATH` |
| `name text` | The current instrument's name |
| `arp a b c` | The current built-in's arpeggio, up to 3 offsets of –48 to 48 |
| `q` | Quit |

`new`, `o` and `q` on an unsaved song need typing twice.

## 11.2 The song format (`.trk`)

A `.trk` file is plain text, so you can read it or write it by hand. This
is an excerpt, and doesn't load on its own: a real file needs a row for
every row of each pattern.

```text
acid-track 2
title Acid Groove
speed 6
instrument 01 "Lead"  wave pulse  adsr 1 30 60 60  duty 40  pwm 2  vib 2 3
instrument 02 "Fat Bass"  script "Home/sounds/bass.snd" fatbass
order 00 01 02 03 loop 0

pattern 00 16
A-4 01 . .. | A-1 02 . .. | A-3 03 . .. | C-6 05 . .. | ... .. . .. | ... .. . .. | ... .. . .. | ... .. . ..
... .. . .. | ... .. . .. | ... .. . .. | ... .. . .. | ... .. . .. | ... .. . .. | ... .. . .. | ... .. . ..
```

The **order** lists the patterns the song plays in turn, and `loop N`
says which entry (counting from 0) to go back to after the last. A
**pattern** can appear in the order any number of times, and editing it
changes it everywhere it is used. Each row of a pattern has all eight
tracks, split by `|`.

Songs from before tracks (`acid-track 1`, four channels of two voices)
still load: they're turned into tracks as they open. Channel *n* goes on
track *n* and its second-note column on track 4+*n*, order transposes are
worked into the notes, and `voice2` settings are dropped. Acid Tracker
says so when it opens one; save it with `:w` to keep the new form.

A **built-in instrument** has these fields:

| Field | Values |
|---|---|
| `wave` | `pulse`, `saw`, `tri` or `noise` |
| `adsr` | Attack, decay and release in ms (0–100000), and sustain as a percentage (0–100 in the tracker) |
| `duty` | Pulse width, 1–99 |
| `pwm` | Duty change per tick, –50 to 50 |
| `vib` | Vibrato depth and speed, 0–15 each |
| `arp` | Up to 3 semitone offsets, –48 to 48, stepped one per tick |
| `filter` | `lp`, `bp` or `hp`, then the cutoff (0–255) and the resonance (0–15) |

A **script instrument** names an `instrument` block in a `.snd` file. Paths
inside song files are relative to the file system's root, such as
`Home/sounds/bass.snd`.

These are the pattern commands:

| Command | Effect (on built-in instruments) |
|---|---|
| `1 XX` | Slide up |
| `2 XX` | Slide down |
| `3 XX` | Glide to the row's note |
| `4 XY` | Vibrato, depth X and speed Y |
| `8 0X` | Waveform (0 pulse, 1 saw, 2 tri, 3 noise) |
| `9 XX` | Duty |
| `A XX` | Filter cutoff |
| `F XX` | Speed |

These ranges hold:
- pattern numbers 00–3F, instrument numbers 01–3F, pattern length 1–64;
- an order of 1–128 entries;
- `speed` 1–31.

A file with a number out of range, or anything malformed, fails to load
with `LINE: message`. A script instrument whose file is missing or broken
doesn't stop the song from loading: it plays silence, and you get a
warning.

## 11.3 The audio language (`.snd`)

A script runs until it reaches `wait`, then sleeps for that many ticks.
At 50 ticks a second, `wait 1` is 20 ms. A file holds `sound` blocks and
`instrument` blocks. An instrument can also have an `on release` part,
which runs at note-off.

```text
sound zap
  wave saw
  adsr 0 40 100 80
  pitch 70
  gate on
  repeat 12
    pitch -3
    wait 1
  end
  gate off
end

instrument fatbass
  wave saw
  v2 wave pulse; v2 duty 30; v2 fine +6
  both adsr 2 120 60 80
  filter lp 60 res 6
  both route on
  both gate on
  loop
    v2 duty +2
    wait 1
  end
on release
  both gate off
end
```

A file with no blocks at all is one sound, so a short script needs no
`sound … end` around it. Statements end at a new line or a `;`.

| Statement | What it does |
|---|---|
| `wave saw` (`pulse`, `tri`, `noise`) | The waveform |
| `duty N`, `duty +N` | Pulse width in percent, absolute or relative |
| `adsr A D S R` | The envelope: ms, ms, percent, ms |
| `gate on`, `gate off` | Start the note, release it |
| `pitch N`, `pitch +N`, `pitch note` | Semitones on the 88-key scale (1–88, C-4 is 40). `note` is the tracker's note. |
| `fine N`, `fine +N` | Fine pitch in 1/64 semitones, for slides and vibrato |
| `arp A B C`, `arp off`, `arprate MS` | The synth's arpeggio, offsets from the current pitch |
| `ring on/off`, `route on/off` | Ring-modulate with the other voice; send the voice through the filter |
| `filter lp CUT res RES` | The shared filter (`lp`, `bp` or `hp`) |
| `let x = …`, `x = …` | Whole-number variables |
| `if … else … end`, `repeat N … end`, `loop … end` | Control flow |
| `wait N`, `wait row`, `wait beat` | Sleep for ticks, or until the song's next row or beat |
| `stop` | End now, releasing the voices |
| `song "PATH"`, `play [N]`, `stop song` | `song` loads a song (when the script loads). `play` plays it from order N. A bare `play`, with no `song` line before it, restarts the app's own song from order N. `stop song` stops it |
| `tempo N`, `mute CH`, `unmute CH`, `jump N` | Steer the song |

**Voices.** A sound has two voices. Put `v1`, `v2` or `both` before a
command to choose; with none, it means `v1`. A tracker instrument has only
its track's voice, so in a song its `v2` commands do nothing and its
`note2` is 0. A leading `+` or `-` makes a
value relative to the current one. The sign applies to everything after
it, so `fine -4 + 2` moves by –6.

**Expressions.** These are whole numbers only:
- the arithmetic operators `+ - * / %`
- the comparisons `== != < <= > >=`
- `and`, `or` and `not`
- `rand N`, which gives 0 to N–1

A script can read `note`, `note2`, `tick` and the song's `row`, `beat` and
`order`. Each of the last three is –1 with no song playing.

**Limits.**
- A script runs at most 256 instructions a tick. A busy loop just carries
  on next tick, and can't stall the sound.
- It can have at most 64 variables, and each `repeat` counts as one of them.
  It can nest at most 64 deep, and nested expressions count too.
- Arithmetic saturates rather than overflowing, and dividing by zero gives 0.
- A script can't crash the OS. A mistake is a compile error with its line
  and column, such as `3:7 unknown command 'wav'`.

The song commands in a script only steer a song that the same app started.
`song "PATH"` followed by `play` replaces whatever is playing, just as
`acid_song_play` does.

## 11.4 Sounds and songs from an app

Load a script once and play its sounds whenever you need them:

```lua snippet
local sfx = assert(acid_sound_load_file("v3/fsroot/Home/sounds/zap.snd"))
local id = acid_sound_play(sfx, "zap")      -- nil if no voice is free
-- later, if it needs cutting short:
if id then acid_sound_stop(id) end
```

You can also compile source text directly with `acid_sound_load`. Pass a
note as the third argument to `acid_sound_play` to set the script's `note`.

Songs work the same way:

```lua snippet
local song, warnings = acid_song_load("v3/fsroot/Home/music/acid_groove.trk")
if song then
  acid_song_play(song)                  -- from the start
  local order, row, tick = acid_song_position()
end
```

| Call | Returns |
|---|---|
| `acid_sound_load(src)`, `acid_sound_load_file(path)` | A program, or `nil, "LINE:COL message"` |
| `acid_sound_play(prog[, name[, note]])` | A sound id, or `nil` when no voice is free, the name isn't a `sound` block, or 16 sounds are already running |
| `acid_sound_stop(id)`, `acid_sound_free(prog)` | Nothing |
| `acid_song_load(path)`, `acid_song_parse(text)` | A song and a table of warnings, or `nil, "LINE: message"` |
| `acid_song_text(song)` | The song as `.trk` text in today's format |
| `acid_song_update(song, text)` | The warnings, or `nil, err`. A playing copy keeps its place. |
| `acid_song_play(song[, order[, row]])`, `acid_song_stop()`, `acid_song_mute(ch, on)` | Nothing |
| `acid_song_position()` | `order, row, tick`, or nothing when your app's song isn't playing |
| `acid_song_preview(song, ch, note, inst)`, `acid_song_free(song)` | Nothing |

Each app can hold up to 16 programs and 4 songs. Free the ones you're done
with. Paths you pass to these calls are full paths, like `acid_fs_read`'s.
Only one song plays at a time. Stopping it, muting it and reading its
position only work for the app that started it.

## 11.5 How songs and sounds share the voices

Track 1 of a song plays voice 0, and track 8 plays voice 7.

A sound effect takes a free voice if there is one, starting from voice 7.
During a song, the voices of tracks that have notes belong to the song,
so a sound only gets the voice of a track the song leaves empty. A song
that uses five tracks leaves three voices for sound effects; a song that
uses all eight leaves none, and `acid_sound_play` returns `nil` until it
stops. A sound that uses `v2` or `both` takes two voices when two are
free, or one when only one is.
Notes you start yourself with `acid_play_note` still work as before, and
the last thing to write to a voice wins.

While a song plays, the whole mix is turned down about 12 dB, so that
eight voices together don't clip. Your own notes are quieter then too.

---

[← WASM carts](10-wasm-carts.md) · [Contents](README.md)
