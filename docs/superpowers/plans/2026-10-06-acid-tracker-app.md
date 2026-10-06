# Acid Tracker App Implementation Plan (plan 2 of 2)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build Acid Tracker, the GoatTracker-style app that edits `.trk` songs and plays them through the `acid-sound` engine. Wire it into File Manager and Terminal, ship sample songs and sounds, and document all of it in the manual.

**Architecture:** The app follows Sprite Paint's pattern: a main `apps/tracker.lua` plus helper modules in `apps/tracker/`, loaded through the manifest's `libs`.
- **The song model lives in Lua.** `TrkSong` reads `.trk` text and writes the same canonical text as acid-sound's Rust writer.
- **The Rust parser is the reader of record.** Text always goes through `acid_song_parse` first.
- **Editing** happens in `TrkEdit`, which holds the cursor and every edit operation.
- **Layout** is computed in `TrkLayout`.
- **Commands** typed on the Esc command line are parsed by `TrkCmd`.
- **Playback.** Every change is sent to the kernel with `acid_song_update`, so edits are heard while the song plays. Preview uses `acid_song_preview`.

Everything except drawing is plain logic, tested headlessly by the existing Lua suite runner.

**Tech Stack:** Lua 5.4 apps (`AcidApp`), the `acid_song_*`/`acid_sound_*` calls from plan 1, the Rust test harnesses (`acid-lua/tests/game_tests.rs`, `acid-os/tests/{games,golden,manual}.rs`), and `acid-sound`'s loaders for the shipped-file tests.

**Spec:** `docs/superpowers/specs/2026-10-06-acid-tracker-design.md`, §7 in particular. Plan 1 (`docs/superpowers/plans/2026-10-06-acid-sound-engine.md`) is merged.

## Global Constraints

- **Running commands.** Run every command from the repository top (`/home/norfolkh/acid-os-v3`), using `--manifest-path v3/Cargo.toml`.
- **Commits.** Messages use the repo style `Area: summary` and end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- **The window.** It opens at 480×320, is resizable, and has a minimum of 420×240. The font is a fixed 6×8, so the app does not opt into Large font.
- **Channels.** There are 4, and a row in one channel reads like `C-4 01 4 22 E-4`: note, instrument, command, parameter, second note.
- **The `.trk` text the app writes must load in Rust.** Instrument values stay inside acid-sound's ranges:

  | Field | Range |
  |---|---|
  | adsr | 0 to 100000 (sustain 0 to 100) |
  | duty | 1 to 99 |
  | pwm | -50 to 50 |
  | vib | 0 to 15 |
  | arp | -48 to 48 |
  | detune | -768 to 768 |
  | cutoff | 0 to 255 |
  | res | 0 to 15 |
  | speed | 1 to 31 |
  | sfx-donor | 1 to 4 |
  | pattern length | 1 to 64 |
  | instrument number | 01 to 3F |
  | pattern number | 00 to 7F |

  Order transposes are held to ±48.
- **Overflow limits (standing rule from the user).** Every number that comes from a key, a command or a file is clamped or range-checked before it is used. Numbers too big to hold are refused.
- **Pinned assertion counts.** Lua test suites are registered in `v3/crates/acid-lua/tests/game_tests.rs` with an exact assertion count. Each task states its count.
- **Golden images.** Never regenerate a golden image to make a test pass.
- **Lua house style:**
  - classes are tables (`X = AcidApp:extend("X")`)
  - 2-space indent
  - a header comment on every file
  - short comments that say *why*
  - integers only, using `//` for division
- **Paths.** Inside `.trk` and `.snd` files, paths are relative to fsroot (`Home/sounds/bass.snd`). Lua calls take full paths (`v3/fsroot/Home/...`).

## Departures from spec §7, decided here

- **Manifest.** It sets `menu = false`, like Sprite Paint, so the Menu golden stays unchanged. Acid Tracker opens from File Manager (`App/tracker.app.toml` or any `.trk`) or with `run acid tracker` in Terminal.
- **Extra commands.** Besides `:w :o :new :speed :len :ins :q`, there are `:title`, `:name`, `:arp` and `:donor`.
- **Instrument panel.** Up/Down chooses a field, Left/Right changes it by 1, and `-`/`+` change it by 10. Arpeggios are set with `:arp`.
- **Orders panel.** Hex digits pick the pattern (an unused number creates an empty pattern), `+`/`-` transpose, Enter repeats the entry, Delete removes it, and `l` sets the loop point.

Task 8 updates the spec to match.

## File map

| File | Responsibility |
|---|---|
| `v3/apps/tracker/song.lua` | `TrkSong`: the model, note names, read and write `.trk` text |
| `v3/apps/tracker/edit.lua` | `TrkEdit`: the cursor, grid typing, orders and instrument panels |
| `v3/apps/tracker/layout.lua` | `TrkLayout`: every y position and the channel x positions |
| `v3/apps/tracker/cmd.lua` | `TrkCmd`: parsing the command line, number and path checks |
| `v3/apps/tracker.lua` | `TrackerApp`: drawing, keys, playback and files |
| `v3/apps/tracker.app.toml` | The manifest |
| `v3/fsroot/Home/music/acid_groove.trk` | The sample song |
| `v3/fsroot/Home/sounds/{bass,wobble,zap,sync}.snd` | Sample scripts |
| `v3/tools/game_test_env.lua` | Fakes for the `acid_song_*` and `acid_sound_*` calls |
| `v3/tools/test_trk_song.lua`, `test_trk_edit.lua`, `test_trk_layout_cmd.lua`, `test_tracker_app.lua` | Lua suites |
| `v3/crates/acid-lua/tests/game_tests.rs` | Suite registration |
| `v3/crates/acid-sound/tests/shipped.rs` | The shipped songs and sounds load cleanly |
| `v3/apps/file_manager.lua`, `v3/apps/terminal.lua` (and their tests) | `.trk`/`.snd` opening; `play` |
| `v3/crates/acid-os/tests/{games,golden}.rs`, `.../golden/acid_tracker.ppm` | Real-kernel boot and screenshot |
| `docs/manual-v3/11-music.md` and edits to the other chapters, `README.md` | Documentation |

---

### Task 1: Lua fakes for the sound calls, and the `TrkSong` model

**Files:**
- Modify: `v3/tools/game_test_env.lua`
- Create: `v3/apps/tracker/song.lua`
- Create: `v3/tools/test_trk_song.lua`
- Modify: `v3/crates/acid-lua/tests/game_tests.rs`

**Interfaces:**
- **Test fakes (globals):**
  - `SOUND_CALLS`, `SONGS`, `SONG_NEXT`, `SONG_PARSE_ERR` (fails the next parse only), `SONG_WARNINGS`, `SONG_POS`, `SOUND_PLAY_ID`
  - every `acid_song_*` and `acid_sound_*` function
  - calls are recorded as tables, for example `{ "parse", handle }`, `{ "update", h }`, `{ "free", h }`, `{ "play", h, order, row }`, `{ "stop" }`, `{ "mute", ch, on }`, `{ "preview", h, ch, note, inst }`, `{ "sound_load", id }`, `{ "sound_play", prog, name, note }`, `{ "sound_stop", id }`, `{ "sound_free", prog }`
- **The `TrkSong` model:**

  ```
  song = { title, speed, donor, instruments = { [n] = ins }, orders = { [1..4] = { entries = { { pattern, transpose } }, loop } }, patterns = { [n] = { rows } } }
  row  = { note, inst, cmd, param, note2 }
  ```

  - `note`: 0 means none, 255 means off, 1..88 is a note.
  - `cmd`: `""` means none, otherwise one letter of `"123489AF"`.
  - A built-in instrument is `{ name, kind = "builtin", wave = 0..3, adsr = {a,d,s,r}, duty, pwm, vib = {depth, speed}, arp = {...}, filter = nil | {mode 1|2|4, cutoff, res}, voice2 = "off"|"detune"|"octave"|"fifth"|"ring", detune }`.
  - A script instrument is `{ name, kind = "script", path, block }`.
- **`TrkSong` functions:**
  - `parse_note`, `note_name`, `clamp(v, what)`
  - `builtin(name)`, `script(name, path, block)`, `empty_rows(n)`, `new()`
  - `parse(text)`, which returns `song` or `nil, err`
  - `write(song)`, `row_text(row)`
- **`TrkSong` constants:** `CHANNELS`, `MAX_ROWS`, `MAX_PATTERN`, `MAX_INSTRUMENT`, `NOTE_NONE`, `NOTE_OFF`, `WAVES`, `COMMANDS`, `FILTER_MODES`, `DEFAULT_ROWS`, `RANGE`

- [ ] **Step 1: Add the sound-call fakes to the test environment**

In `v3/tools/game_test_env.lua`, insert this block directly after the line `function acid_active_voice_count() return VOICES end`:

```lua
-- Songs and sounds (acid-sound's calls). The kernel's parser is faked:
-- every text parses unless SONG_PARSE_ERR is set, which fails the next
-- parse only. Every call is recorded in SOUND_CALLS.
SOUND_CALLS = {}
SONGS = {}            -- handle -> text
SONG_NEXT = 0
SONG_PARSE_ERR = nil
SONG_WARNINGS = {}    -- what every parse and update warns
SONG_POS = nil        -- { order, row, tick } while "playing"
SOUND_NEXT = 0
SOUND_PLAY_ID = 1     -- what acid_sound_play answers (nil: no free voice)
local function copy_list(t) local c = {} for i, v in ipairs(t) do c[i] = v end return c end
function acid_song_parse(text)
  if SONG_PARSE_ERR then
    local e = SONG_PARSE_ERR
    SONG_PARSE_ERR = nil
    return nil, e
  end
  SONG_NEXT = SONG_NEXT + 1
  SONGS[SONG_NEXT] = text
  push(SOUND_CALLS, { "parse", SONG_NEXT })
  return SONG_NEXT, copy_list(SONG_WARNINGS)
end
function acid_song_load(path)
  local text = FS[path]
  if type(text) ~= "string" then return nil, "not found" end
  return acid_song_parse(text)
end
function acid_song_update(song, text)
  if not SONGS[song] then return nil, "no such song" end
  SONGS[song] = text
  push(SOUND_CALLS, { "update", song })
  return copy_list(SONG_WARNINGS)
end
function acid_song_free(song) SONGS[song] = nil; push(SOUND_CALLS, { "free", song }) end
function acid_song_play(song, order, row)
  push(SOUND_CALLS, { "play", song, order or 0, row or 0 })
  SONG_POS = { order or 0, row or 0, 0 }
end
function acid_song_stop() push(SOUND_CALLS, { "stop" }); SONG_POS = nil end
function acid_song_position()
  if SONG_POS then return SONG_POS[1], SONG_POS[2], SONG_POS[3] end
end
function acid_song_mute(ch, on) push(SOUND_CALLS, { "mute", ch, on }) end
function acid_song_preview(song, ch, note, inst) push(SOUND_CALLS, { "preview", song, ch, note, inst }) end
function acid_sound_load(src)
  SOUND_NEXT = SOUND_NEXT + 1
  push(SOUND_CALLS, { "sound_load", SOUND_NEXT })
  return SOUND_NEXT
end
function acid_sound_load_file(path)
  local text = FS[path]
  if type(text) ~= "string" then return nil, "not found" end
  return acid_sound_load(text)
end
function acid_sound_play(prog, name, note)
  push(SOUND_CALLS, { "sound_play", prog, name or "", note or 40 })
  return SOUND_PLAY_ID
end
function acid_sound_stop(id) push(SOUND_CALLS, { "sound_stop", id }) end
function acid_sound_free(prog) push(SOUND_CALLS, { "sound_free", prog }) end
```

- [ ] **Step 2: Write the failing `TrkSong` tests**

Create `v3/tools/test_trk_song.lua`:

```lua
-- TrkSong (apps/tracker/song.lua): note names, rows, the new-song
-- template, and reading then writing .trk text byte for byte -- checked
-- against acid-sound's own canonical files (TRK, from the prelude) so the
-- Lua and Rust writers can't drift apart.

group("note names")
eq({ TrkSong.parse_note("A-0"), TrkSong.parse_note("C-4"), TrkSong.parse_note("C#4"), TrkSong.parse_note("C-8") },
  { 1, 40, 41, 88 }, "note names map to piano keys")
eq({ TrkSong.parse_note("G#0"), TrkSong.parse_note("C#8"), TrkSong.parse_note("H-4"), TrkSong.parse_note("C-") },
  {}, "off-keyboard and malformed names are nil")
local bad = {}
for ona = 1, 88 do
  if TrkSong.parse_note(TrkSong.note_name(ona)) ~= ona then bad[#bad + 1] = ona end
end
eq(bad, {}, "every key's name reads back")

group("rows")
eq(TrkSong.row_text({ note = 0, inst = 0, cmd = "", param = 0, note2 = 0 }), "... .. . .. ...", "an empty row is dots")
eq(TrkSong.row_text({ note = 40, inst = 0x1F, cmd = "A", param = 0, note2 = 52 }), "C-4 1F A 00 C-5", "a full row")
eq(TrkSong.row_text({ note = 255, inst = 0, cmd = "", param = 0, note2 = 0 }), "=== .. . .. ...", "a note-off")

group("a new song")
local fresh = TrkSong.write(TrkSong.new())
local s = TrkSong.parse(fresh)
eq({ s.title, s.speed, s.donor, #s.orders[1].entries, #s.patterns[3] }, { "untitled", 6, 4, 1, 16 },
  "a new song: four channels, each on its own 16-row pattern")
eq(TrkSong.write(s), fresh, "and it round-trips")

group("canonical files round-trip")
for _, f in ipairs(TRK) do
  local song, err = TrkSong.parse(f[2])
  ok(song, f[1] .. " parses" .. (err and (": " .. err) or ""))
  eq(song and TrkSong.write(song), f[2], f[1] .. " writes back byte for byte")
end

group("everything the format has")
local ROUND = [[
acid-track 1
title Round Trip
speed 6
sfx-donor 4
instrument 01 "Lead"  wave saw  adsr 0 8 70 20  duty 50  pwm 2  vib 4 2  arp 4 7  filter lp 120 4  voice2 detune 6
instrument 02 "Bass"  script "Home/sounds/demo.snd" bass
instrument 03 "Hat"  wave noise  adsr 0 2 0 2  duty 50  voice2 octave
order 1  00 00+12 loop 1
order 2  01 01-5 loop 0
order 3  01 loop 0
order 4  01 loop 0

pattern 00 4
C-4 01 . .. ...
... .. 4 22 ...
=== .. . .. ...
D#3 02 F 03 G-3

pattern 01 2
A-0 03 9 20 ...
... .. A FF ...
]]
eq(TrkSong.write(TrkSong.parse(ROUND)), ROUND, "every field and row form round-trips")
local r = TrkSong.parse(ROUND)
eq({ r.instruments[1].voice2, r.instruments[1].detune, r.instruments[1].filter, r.instruments[1].arp },
  { "detune", 6, { 1, 120, 4 }, { 4, 7 } }, "built-in fields read into the model")
eq({ r.instruments[2].kind, r.instruments[2].path, r.instruments[2].block },
  { "script", "Home/sounds/demo.snd", "bass" }, "and script instruments")
eq({ r.orders[1].entries[2].transpose, r.orders[1].loop, r.patterns[0][4].cmd, r.patterns[0][4].param },
  { 12, 1, "F", 3 }, "orders, transposes and commands")

group("not a song")
eq({ TrkSong.parse("hello") }, { nil, "not an acid-track 1 file" }, "anything else is refused")
eq(TrkSong.clamp(500, "duty"), 99, "clamp holds a value inside its range")
```

That is 14 fixed assertions, plus 2 per file in `TRK`.

Register the suite in `v3/crates/acid-lua/tests/game_tests.rs`. Add it after the `sprite_app_commands` test:

```rust
/// The .trk files the Lua reader and writer must reproduce exactly:
/// acid-sound's golden demo and every song shipped under Home/music, as
/// long strings in a TRK table.
fn trk_prelude() -> (String, usize) {
    let mut files = vec![repo_root().join("v3/crates/acid-sound/tests/golden/demo.trk")];
    if let Ok(dir) = std::fs::read_dir(repo_root().join("v3/fsroot/Home/music")) {
        let mut music: Vec<PathBuf> = dir.map(|e| e.unwrap().path()).filter(|p| p.extension().is_some_and(|e| e == "trk")).collect();
        music.sort();
        files.extend(music);
    }
    let mut prelude = String::from("TRK = {}\n");
    for path in &files {
        let text = std::fs::read_to_string(path).unwrap();
        assert!(!text.contains("]==]"), "{} can't be quoted", path.display());
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        prelude.push_str(&format!("TRK[#TRK + 1] = {{ {name:?}, [==[\n{text}]==] }}\n"));
    }
    (prelude, files.len())
}

#[test]
fn trk_song() {
    let (prelude, files) = trk_prelude();
    run_suite_with(&prelude, &["v3/tools/game_test_env.lua", "v3/apps/tracker/song.lua", "v3/tools/test_trk_song.lua"], 14 + 2 * files);
}
```

- [ ] **Step 3: Run the suite and watch it fail**

Run: `cargo test --manifest-path v3/Cargo.toml -p acid-lua --test game_tests trk_song`
Expected: FAIL with `v3/apps/tracker/song.lua: No such file`.

- [ ] **Step 4: Implement `TrkSong`**

Create `v3/apps/tracker/song.lua`:

```lua
-- TrkSong: Acid Tracker's in-memory .trk song and its text form. The
-- reader of record is the kernel's (acid_song_parse): the tracker only
-- reads text that has already parsed there, and `write` emits exactly the
-- canonical text acid-sound's writer does, so files round-trip byte for
-- byte. See docs/superpowers/specs/2026-10-06-acid-tracker-design.md §3.

TrkSong = {}
TrkSong.CHANNELS = 4
TrkSong.MAX_ROWS = 64
TrkSong.MAX_PATTERN = 0x7F
TrkSong.MAX_INSTRUMENT = 0x3F
TrkSong.NOTE_NONE = 0
TrkSong.NOTE_OFF = 255
TrkSong.WAVES = { "pulse", "saw", "tri", "noise" }   -- [wave + 1]
TrkSong.COMMANDS = "123489AF"
TrkSong.FILTER_MODES = { [1] = "lp", [2] = "bp", [4] = "hp" }
TrkSong.DEFAULT_ROWS = 16
-- What acid-sound's parser accepts; edits stay inside these so a saved
-- file always loads.
TrkSong.RANGE = {
  adsr = { 0, 100000 }, sustain = { 0, 100 }, duty = { 1, 99 }, pwm = { -50, 50 }, vib = { 0, 15 },
  arp = { -48, 48 }, detune = { -768, 768 }, cutoff = { 0, 255 }, res = { 0, 15 },
  speed = { 1, 31 }, donor = { 1, 4 }, transpose = { -48, 48 },
}

local NAMES = { "C-", "C#", "D-", "D#", "E-", "F-", "F#", "G-", "G#", "A-", "A#", "B-" }

function TrkSong.clamp(v, what)
  local r = TrkSong.RANGE[what]
  return math.max(r[1], math.min(r[2], v))
end

-- "C-4" -> 40 (piano key, A0 = 1), or nil.
function TrkSong.parse_note(s)
  if #s ~= 3 or not s:sub(3, 3):match("%d") then return nil end
  local semi
  for i, n in ipairs(NAMES) do
    if n == s:sub(1, 2) then semi = i - 1 end
  end
  if not semi then return nil end
  local ona = tonumber(s:sub(3, 3)) * 12 + semi - 8
  if ona < 1 or ona > 88 then return nil end
  return ona
end

-- 40 -> "C-4"; held on the keyboard.
function TrkSong.note_name(ona)
  local n = math.max(1, math.min(88, ona)) + 8
  return NAMES[n % 12 + 1] .. (n // 12)
end

function TrkSong.builtin(name)
  return { name = name, kind = "builtin", wave = 0, adsr = { 2, 40, 80, 40 }, duty = 50, pwm = 0,
           vib = { 0, 0 }, arp = {}, filter = nil, voice2 = "off", detune = 0 }
end

function TrkSong.script(name, path, block)
  return { name = name, kind = "script", path = path, block = block }
end

function TrkSong.empty_rows(n)
  local rows = {}
  for i = 1, n do rows[i] = { note = 0, inst = 0, cmd = "", param = 0, note2 = 0 } end
  return rows
end

-- A new song: one lead instrument, each channel on its own empty pattern.
function TrkSong.new()
  local s = { title = "untitled", speed = 6, donor = 4, instruments = {}, orders = {}, patterns = {} }
  s.instruments[1] = TrkSong.builtin("Lead")
  for ch = 1, TrkSong.CHANNELS do
    s.patterns[ch - 1] = TrkSong.empty_rows(TrkSong.DEFAULT_ROWS)
    s.orders[ch] = { entries = { { pattern = ch - 1, transpose = 0 } }, loop = 0 }
  end
  return s
end

-- Reading ---------------------------------------------------------------

-- Whitespace-separated fields; a "quoted" field keeps its spaces.
local function split(line)
  local out, i = {}, 1
  while i <= #line do
    local c = line:sub(i, i)
    if c == " " or c == "\t" then
      i = i + 1
    elseif c == '"' then
      local j = line:find('"', i + 1, true)
      if not j then return nil end
      out[#out + 1] = { text = line:sub(i + 1, j - 1), quoted = true }
      i = j + 1
    else
      local j = line:find("[ \t]", i) or (#line + 1)
      out[#out + 1] = { text = line:sub(i, j - 1) }
      i = j
    end
  end
  return out
end

local function hex2(s)
  if type(s) == "string" and s:match("^%x%x$") then return tonumber(s, 16) end
end

-- A whole number field, or nil (quoted, missing, or too big to hold).
local function int(f)
  if f and not f.quoted and f.text:match("^[+-]?%d+$") then return math.tointeger(tonumber(f.text)) end
end

local function parse_row(line)
  local f = {}
  for w in line:gmatch("%S+") do f[#f + 1] = w end
  if #f ~= 5 then return nil end
  local function note(s, off_ok)
    if s == "..." then return 0 end
    if s == "===" and off_ok then return TrkSong.NOTE_OFF end
    return TrkSong.parse_note(s)
  end
  local r = { note = note(f[1], true), note2 = note(f[5], false) }
  r.inst = f[2] == ".." and 0 or hex2(f[2])
  if f[3] == "." then
    r.cmd = ""
  elseif #f[3] == 1 and TrkSong.COMMANDS:find(f[3], 1, true) then
    r.cmd = f[3]
  end
  r.param = f[4] == ".." and 0 or hex2(f[4])
  if r.note and r.note2 and r.inst and r.cmd and r.param then return r end
end

local function parse_instrument(f)
  local num = f[2] and not f[2].quoted and hex2(f[2].text)
  if not num or not f[3] or not f[3].quoted then return nil end
  local name = f[3].text
  if f[4] and not f[4].quoted and f[4].text == "script" then
    if not (f[5] and f[5].quoted and f[6] and not f[7]) then return nil end
    return num, TrkSong.script(name, f[5].text, f[6].text)
  end
  local b, i, bad = TrkSong.builtin(name), 4, false
  local function nxt()
    local v = int(f[i])
    i = i + 1
    if v == nil then bad = true end
    return v
  end
  local function word()
    local w = f[i] and f[i].text
    i = i + 1
    return w
  end
  while i <= #f do
    local key = word()
    if key == "wave" then
      local w = word()
      b.wave = nil
      for k, n in ipairs(TrkSong.WAVES) do
        if n == w then b.wave = k - 1 end
      end
      if not b.wave then return nil end
    elseif key == "adsr" then
      b.adsr = { nxt(), nxt(), nxt(), nxt() }
    elseif key == "duty" then
      b.duty = nxt()
    elseif key == "pwm" then
      b.pwm = nxt()
    elseif key == "vib" then
      b.vib = { nxt(), nxt() }
    elseif key == "arp" then
      b.arp = {}
      while #b.arp < 3 and int(f[i]) do b.arp[#b.arp + 1] = nxt() end
    elseif key == "filter" then
      local mode = ({ lp = 1, bp = 2, hp = 4 })[word()]
      if not mode then return nil end
      b.filter = { mode, nxt(), nxt() }
    elseif key == "voice2" then
      local m = word()
      if m == "detune" then
        b.voice2, b.detune = "detune", nxt()
      elseif m == "off" or m == "octave" or m == "fifth" or m == "ring" then
        b.voice2 = m
      else
        return nil
      end
    else
      return nil
    end
  end
  if bad then return nil end
  return num, b
end

local function parse_order(f)
  local ch = int(f[2])
  if not ch or ch < 1 or ch > TrkSong.CHANNELS then return nil end
  local entries, i = {}, 3
  while f[i] and f[i].text ~= "loop" do
    local p, t = f[i].text:match("^(%x%x)([+-]%d+)$")
    if not p then p, t = f[i].text:match("^(%x%x)$"), "0" end
    if not p then return nil end
    entries[#entries + 1] = { pattern = tonumber(p, 16), transpose = math.tointeger(tonumber(t)) }
    i = i + 1
  end
  local loop = int(f[i + 1])
  if #entries == 0 or not loop or f[i + 2] then return nil end
  return ch, { entries = entries, loop = loop }
end

-- Text acid_song_parse accepted -> a song, or nil and why.
function TrkSong.parse(text)
  local lines = {}
  for line in (text .. "\n"):gmatch("(.-)\r?\n") do lines[#lines + 1] = line end
  local s = { title = "", speed = 6, donor = 4, instruments = {}, orders = {}, patterns = {} }
  local i = 1
  while lines[i] and lines[i]:match("^%s*$") do i = i + 1 end
  if not lines[i] or lines[i]:match("^%s*(.-)%s*$") ~= "acid-track 1" then
    return nil, "not an acid-track 1 file"
  end
  i = i + 1
  while i <= #lines do
    local n = i
    local line = lines[i]:match("^%s*(.-)%s*$")
    i = i + 1
    local word = line:match("^(%S+)")
    if line == "" or line:sub(1, 1) == "#" then
      -- skip
    elseif word == "title" then
      s.title = line:sub(6):match("^%s*(.-)%s*$")
    else
      local f = split(line)
      if not f then return nil, n .. ": bad line" end
      if word == "speed" then
        s.speed = int(f[2])
      elseif word == "sfx-donor" then
        s.donor = int(f[2])
      elseif word == "instrument" then
        local num, ins = parse_instrument(f)
        if not num then return nil, n .. ": bad instrument" end
        s.instruments[num] = ins
      elseif word == "order" then
        local ch, o = parse_order(f)
        if not ch then return nil, n .. ": bad order" end
        s.orders[ch] = o
      elseif word == "pattern" then
        local num, len = hex2(f[2] and f[2].text), int(f[3])
        if not num or not len then return nil, n .. ": bad pattern" end
        local rows = {}
        for r = 1, len do
          local row = lines[i] and parse_row(lines[i])
          if not row then return nil, i .. ": bad row" end
          rows[r] = row
          i = i + 1
        end
        s.patterns[num] = rows
      else
        return nil, n .. ": unknown line"
      end
    end
  end
  if not s.speed or not s.donor then return nil, "bad header" end
  for ch = 1, TrkSong.CHANNELS do
    if not s.orders[ch] then return nil, "no order for channel " .. ch end
  end
  return s
end

-- Writing ---------------------------------------------------------------

local function sorted_keys(t)
  local k = {}
  for n in pairs(t) do k[#k + 1] = n end
  table.sort(k)
  return k
end

-- One row as the file writes it, e.g. "C-4 01 4 22 E-4".
function TrkSong.row_text(r)
  local function note(n)
    if n == TrkSong.NOTE_NONE then return "..." end
    if n == TrkSong.NOTE_OFF then return "===" end
    return TrkSong.note_name(n)
  end
  local inst = r.inst == 0 and ".." or string.format("%02X", r.inst)
  local cmd = r.cmd == "" and "." or r.cmd
  local param = (r.cmd == "" and r.param == 0) and ".." or string.format("%02X", r.param)
  return note(r.note) .. " " .. inst .. " " .. cmd .. " " .. param .. " " .. note(r.note2)
end

-- The canonical text, exactly as acid-sound's song::write produces it.
function TrkSong.write(s)
  local out = {}
  local function add(x) out[#out + 1] = x end
  add("acid-track 1\n")
  add("title " .. s.title .. "\n")
  add("speed " .. s.speed .. "\n")
  add("sfx-donor " .. s.donor .. "\n")
  for _, num in ipairs(sorted_keys(s.instruments)) do
    local ins = s.instruments[num]
    add(string.format('instrument %02X "%s"', num, (ins.name:gsub('"', "'"))))
    if ins.kind == "script" then
      add(string.format('  script "%s" %s', ins.path, ins.block))
    else
      add(string.format("  wave %s  adsr %d %d %d %d  duty %d", TrkSong.WAVES[ins.wave + 1],
        ins.adsr[1], ins.adsr[2], ins.adsr[3], ins.adsr[4], ins.duty))
      if ins.pwm ~= 0 then add("  pwm " .. ins.pwm) end
      if ins.vib[1] ~= 0 or ins.vib[2] ~= 0 then add("  vib " .. ins.vib[1] .. " " .. ins.vib[2]) end
      if #ins.arp > 0 then
        add("  arp")
        for _, a in ipairs(ins.arp) do add(" " .. a) end
      end
      if ins.filter then
        add(string.format("  filter %s %d %d", TrkSong.FILTER_MODES[ins.filter[1]], ins.filter[2], ins.filter[3]))
      end
      if ins.voice2 == "detune" then
        add("  voice2 detune " .. ins.detune)
      elseif ins.voice2 ~= "off" then
        add("  voice2 " .. ins.voice2)
      end
    end
    add("\n")
  end
  for ch = 1, TrkSong.CHANNELS do
    local o = s.orders[ch]
    add("order " .. ch .. " ")
    for _, e in ipairs(o.entries) do
      add(string.format(" %02X", e.pattern))
      if e.transpose > 0 then
        add("+" .. e.transpose)
      elseif e.transpose < 0 then
        add(tostring(e.transpose))
      end
    end
    add(" loop " .. o.loop .. "\n")
  end
  for _, num in ipairs(sorted_keys(s.patterns)) do
    local rows = s.patterns[num]
    add(string.format("\npattern %02X %d\n", num, #rows))
    for _, r in ipairs(rows) do add(TrkSong.row_text(r) .. "\n") end
  end
  return table.concat(out)
end
```

- [ ] **Step 5: Run the suite and watch it pass**

Run: `cargo test --manifest-path v3/Cargo.toml -p acid-lua --test game_tests trk_song`
Expected: PASS (16 assertions: 14, plus 2 for `demo.trk`).

If the demo round-trip fails, the two writers disagree. Compare the strings line by line and fix the Lua writer. Never change `demo.trk`.

Then run the whole Lua harness: `cargo test --manifest-path v3/Cargo.toml -p acid-lua`. Every suite must still pass with the new fakes.

- [ ] **Step 6: Commit**

```bash
git add v3/tools/game_test_env.lua v3/apps/tracker/song.lua v3/tools/test_trk_song.lua v3/crates/acid-lua/tests/game_tests.rs
git commit -m "Tracker: TrkSong reads and writes canonical .trk text; sound-call fakes for Lua tests

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 2: The shipped songs and sounds

**Files:**
- Create: `v3/fsroot/Home/sounds/bass.snd`, `wobble.snd`, `zap.snd`, `sync.snd`
- Create: `v3/fsroot/Home/music/acid_groove.trk`
- Create: `v3/crates/acid-sound/tests/shipped.rs`

**Interfaces:**
- Consumes: `acid_sound::load::{load_song, load_program}`, `acid_sound::song::{parse, write}`, `acid_sound::engine::Engine`
- Produces: the sample files that Tasks 5 to 8 open. The Lua `trk_song` suite picks up `Home/music/*.trk` by itself, so its count grows by 2.

- [ ] **Step 1: Write the failing test**

Create `v3/crates/acid-sound/tests/shipped.rs`:

```rust
//! The songs and sounds shipped in v3/fsroot/Home load cleanly: every .snd
//! compiles, every .trk loads with no warnings and is canonical, and the
//! demo song plays, audibly and without clipping much.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use acid_sound::engine::Engine;
use acid_sound::load::{load_program, load_song};
use acid_sound::song::{parse, write};
use acid_synth::Synth;

fn fsroot() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fsroot")
}

/// Files named inside songs and scripts are fsroot-relative.
fn read(path: &str) -> Result<String, String> {
    if path.split('/').any(|s| s.is_empty() || s == "." || s == "..") {
        return Err("bad path".into());
    }
    std::fs::read_to_string(fsroot().join(path)).map_err(|_| "not found".to_string())
}

fn files(dir: &str, ext: &str) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(fsroot().join(dir))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == ext))
        .collect();
    v.sort();
    v
}

#[test]
fn every_shipped_sound_compiles() {
    let snd = files("Home/sounds", "snd");
    assert!(snd.len() >= 4, "bass, wobble, zap and sync are shipped");
    for p in snd {
        let src = std::fs::read_to_string(&p).unwrap();
        load_program(&src, &read).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
    }
}

#[test]
fn every_shipped_song_loads_cleanly_and_is_canonical() {
    let trk = files("Home/music", "trk");
    assert!(!trk.is_empty(), "acid_groove.trk is shipped");
    for p in trk {
        let text = std::fs::read_to_string(&p).unwrap();
        let (_, warnings) = load_song(&text, &read).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
        assert!(warnings.is_empty(), "{}: {warnings:?}", p.display());
        assert_eq!(write(&parse(&text).unwrap()), text, "{} is not in canonical form", p.display());
    }
}

#[test]
fn the_demo_song_plays_audibly_without_much_clipping() {
    let text = read("Home/music/acid_groove.trk").unwrap();
    let (song, _) = load_song(&text, &read).unwrap();
    let (mut synth, mut engine) = (Synth::new(), Engine::new());
    engine.play_song(&mut synth, 1, 1, Arc::new(song), 0, 0);
    let mut buf = vec![0u8; 22050 * 4];
    engine.render(&mut synth, &mut buf);
    let loud = buf.iter().filter(|&&b| b != 128).count();
    assert!(loud > buf.len() / 4, "only {loud} audible samples");
    let clipped = buf.iter().filter(|&&b| b == 0 || b == 255).count();
    assert!(clipped < buf.len() / 20, "{clipped} clipped samples");
}

#[test]
fn the_sync_script_starts_the_demo_song() {
    let src = read("Home/sounds/sync.snd").unwrap();
    let prog = Arc::new(load_program(&src, &read).unwrap());
    let (mut synth, mut engine) = (Synth::new(), Engine::new());
    engine.play_sound(&mut synth, 7, prog, 0, 40, 0).unwrap();
    let mut buf = vec![0u8; 441 * 3];
    engine.render(&mut synth, &mut buf);
    assert_eq!(engine.song_owner(), Some(7));
    assert!(engine.song_position().is_some());
}
```

Run: `cargo test --manifest-path v3/Cargo.toml -p acid-sound --test shipped`
Expected: FAIL, because the `Home/sounds` directory doesn't exist yet.

- [ ] **Step 2: Write the sample sounds**

`v3/fsroot/Home/sounds/bass.snd`:

```text
# A fat two-voice bass for Acid Tracker: a saw and a detuned pulse,
# filtered, with the pulse width sweeping while the note holds.
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

`v3/fsroot/Home/sounds/wobble.snd`:

```text
# A wobble pad: the filter cutoff swings up and down while the note holds.
instrument wobble
  wave saw
  adsr 5 0 100 120
  route on
  gate on
  let cut = 20
  let step = 12
  loop
    filter lp cut res 10
    cut = cut + step
    if cut > 200 or cut < 20
      step = 0 - step
    end
    wait 1
  end
on release
  gate off
end
```

`v3/fsroot/Home/sounds/zap.snd`:

```text
# A laser zap for games:
#   local zap = acid_sound_load_file("v3/fsroot/Home/sounds/zap.snd")
#   acid_sound_play(zap)
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
```

`v3/fsroot/Home/sounds/sync.snd`:

```text
# Plays the demo song and ticks a hi-hat on every beat, in time with it.
# Runs until stopped: Terminal's `play` on its own stops it.
sound main
  song "Home/music/acid_groove.trk"
  play
  wave noise
  adsr 0 20 0 10
  pitch 80
  loop
    wait beat
    gate on
    wait 1
    gate off
  end
end
```

- [ ] **Step 3: Write the sample song**

`v3/fsroot/Home/music/acid_groove.trk` must be in canonical form exactly as written here: no trailing spaces, a blank line before each pattern, and one final newline.

```text
acid-track 1
title Acid Groove
speed 6
sfx-donor 4
instrument 01 "Lead"  wave pulse  adsr 1 30 60 60  duty 40  pwm 2  vib 2 3  voice2 detune 5
instrument 02 "Fat Bass"  script "Home/sounds/bass.snd" fatbass
instrument 03 "Wobble"  script "Home/sounds/wobble.snd" wobble
instrument 04 "Pluck"  wave tri  adsr 0 60 0 60  duty 50  arp 12 7
instrument 05 "Hat"  wave noise  adsr 0 3 0 3  duty 50
order 1  00 00 01 00 loop 0
order 2  02 02+5 02 02+7 loop 0
order 3  03 loop 0
order 4  04 loop 0

pattern 00 16
A-4 01 . .. ...
... .. . .. ...
C-5 .. . .. ...
... .. . .. ...
E-5 .. . .. ...
... .. 4 23 ...
D-5 .. . .. ...
=== .. . .. ...
C-5 .. . .. ...
... .. . .. ...
A-4 .. . .. ...
... .. . .. ...
G-4 .. 3 20 ...
... .. . .. ...
E-4 .. . .. ...
=== .. . .. ...

pattern 01 16
A-4 01 . .. ...
... .. . .. ...
A-4 .. . .. ...
C-5 .. . .. ...
... .. . .. ...
E-5 .. 1 04 ...
... .. . .. ...
=== .. . .. ...
G-5 .. . .. ...
... .. . .. ...
E-5 .. . .. ...
... .. . .. ...
D-5 .. 4 34 ...
... .. . .. ...
C-5 .. . .. ...
=== .. . .. ...

pattern 02 16
A-1 02 . .. ...
... .. . .. ...
=== .. . .. ...
A-1 .. . .. ...
A-2 .. . .. ...
=== .. . .. ...
A-1 .. . .. ...
... .. . .. ...
C-2 .. . .. ...
=== .. . .. ...
C-2 .. . .. ...
D-2 .. . .. ...
E-2 .. . .. ...
... .. . .. ...
=== .. . .. ...
G-1 .. . .. ...

pattern 03 16
A-3 03 . .. ...
... .. . .. ...
... .. . .. ...
... .. . .. ...
... .. . .. ...
... .. . .. ...
... .. . .. ...
=== .. . .. ...
E-4 04 . .. A-4
... .. . .. ...
C-4 .. . .. E-4
... .. . .. ...
D-4 .. . .. F-4
... .. . .. ...
E-4 .. . .. G#4
=== .. . .. ...

pattern 04 16
C-6 05 . .. ...
... .. . .. ...
C-6 .. . .. ...
... .. . .. ...
C-6 .. . .. ...
... .. . .. ...
C-6 .. . .. ...
... .. . .. ...
C-6 .. . .. ...
... .. . .. ...
C-6 .. . .. ...
... .. . .. ...
C-6 .. . .. ...
... .. . .. ...
C-6 .. . .. ...
C-6 .. . .. ...
```

- [ ] **Step 4: Run both test sets**

Run: `cargo test --manifest-path v3/Cargo.toml -p acid-sound --test shipped`
Expected: 4 passed.

If `the_demo_song_plays_audibly_without_much_clipping` fails on clipping, lower the instruments' sustain levels:
- built-in instruments: the third `adsr` number;
- script instruments: their `adsr` sustain.

Keep doing that until clipping is under 5%, and report the numbers. Don't change the threshold.

Run: `cargo test --manifest-path v3/Cargo.toml -p acid-lua --test game_tests trk_song`
Expected: PASS. The count is now 14 + 2×2 = 18, because `acid_groove.trk` is picked up automatically and must round-trip through the Lua writer too.

- [ ] **Step 5: Commit**

```bash
git add v3/fsroot/Home/sounds v3/fsroot/Home/music v3/crates/acid-sound/tests/shipped.rs
git commit -m "Samples: Acid Groove song and bass, wobble, zap and sync sounds

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 3: `TrkEdit`: the cursor, grid typing and the panels

**Files:**
- Create: `v3/apps/tracker/edit.lua`
- Create: `v3/tools/test_trk_edit.lua`
- Modify: `v3/crates/acid-lua/tests/game_tests.rs`

**Interfaces:**
- **Consumes:** `TrkSong` (Task 1) and `AcidKeys`, which includes `F1`..`F12`.
- **Produces:** `TrkEdit.new(song)`, which returns an editor `E` with these fields:
  - `song`, `ch` (1..4), `row` (0-based), `slot_i` (1..7)
  - `order` (the view's order position, 0-based), `octave` (0..7), `inst` (1..63), `edit` (bool)
  - `focus` (`"grid"`, `"orders"` or `"ins"`)
  - `ord_ch`, `ord_pos`, `ord_digit`, `ins_field`, `dirty`
- **`E` methods:**
  - Looking things up: `entry(ch)`, `rows(ch)`, `max_rows()`, `order_count()`, `slot()` (which returns `{ col, digit }`), `cell()`
  - Moving: `move_row(d)`, `move_slot(d)`, `next_focus()`, `set_octave(d)`, `step_inst(d)`, `set_order(o)`
  - Typing: `piano_note(code)`, `grid_key(code)` (which returns `changed, note`), `orders_key(code)` and `ins_key(code)` (each returns `changed`)
  - Patterns: `set_length(n)`, `ensure_pattern(n)`
- **Statics:** `TrkEdit.SLOTS`, `TrkEdit.PIANO`, `TrkEdit.INS_FIELDS`, `TrkEdit.FIELD_LABELS`, `TrkEdit.field_text(ins, id)`, `TrkEdit.adjust(ins, id, d)`

- [ ] **Step 1: Write the failing tests**

Create `v3/tools/test_trk_edit.lua`:

```lua
-- TrkEdit (apps/tracker/edit.lua): moving the cursor, typing into the
-- grid, the orders panel and the instrument fields, on a TrkSong model.
local K = AcidKeys
local function b(c) return c:byte() end
local E = TrkEdit.new(TrkSong.new())
local P = E.song.patterns

group("moving")
E:move_row(-1)
eq(E.row, 15, "up from row 00 wraps to the pattern's last row")
E:move_row(1)
E:move_slot(-1)
eq({ E.ch, E.slot_i }, { 4, 7 }, "left from channel 1's note wraps to channel 4's second note")
E:move_slot(1)
eq({ E.ch, E.slot_i }, { 1, 1 }, "and right comes back")
E:next_focus(); E:next_focus(); E:next_focus()
eq({ E.ch, E.focus }, { 4, "grid" }, "Tab steps through the channels")
E:next_focus()
eq(E.focus, "orders", "then to the orders panel")
E:next_focus()
eq(E.focus, "ins", "then to the instrument")
E:next_focus()
eq({ E.focus, E.ch }, { "grid", 1 }, "and back to channel 1")

group("piano keys")
eq({ E:piano_note(b("z")), E:piano_note(b("m")), E:piano_note(b("q")), E:piano_note(b("i")) },
  { 40, 51, 52, 64 }, "two octaves of keys from C-4")
E:set_octave(9)
eq(E.octave, 7, "the octave stops at 7")
eq(E:piano_note(b("i")), nil, "a key off the top of the keyboard plays nothing")
E:set_octave(-3)

group("typing into the grid")
local changed, note = E:grid_key(b("q"))
eq({ changed, note, P[0][1].note, P[0][1].inst, E.row }, { true, 52, 52, 1, 1 },
  "a note key writes the note and the current instrument, then steps down")
E:grid_key(b("`"))
eq(P[0][2].note, 255, "` writes a note-off")
E.row, E.slot_i = 4, 2
E:grid_key(b("1")); E:grid_key(b("f"))
eq({ P[0][5].inst, E.row, E.slot_i }, { 0x1F, 5, 2 }, "two hex digits set the instrument, then step down")
E.row, E.slot_i = 4, 2
E:grid_key(b("7")); E:grid_key(b("f"))
eq(P[0][5].inst, 0x3F, "an instrument past 3F is held at 3F")
E.row, E.slot_i = 6, 4
E:grid_key(b("a")); E:grid_key(b("3")); E:grid_key(b("c"))
eq({ P[0][7].cmd, P[0][7].param, E.row }, { "A", 0x3C, 7 }, "a command letter moves on to its parameter")
eq({ E:grid_key(b("x")) }, { false }, "a key that isn't a hex digit does nothing there")
E.row, E.slot_i = 6, 4
E:grid_key(K.DELETE)
eq({ P[0][7].cmd, P[0][7].param, E.row }, { "", 0, 7 }, "Delete on the command clears it and its parameter")
E.row, E.slot_i = 0, 7
local _, n2 = E:grid_key(b("e"))
eq({ P[0][1].note2, n2 }, { 56, 56 }, "the second note column takes notes")
E.row, E.slot_i = 0, 7
eq({ E:grid_key(b("`")) }, { false }, "but not note-offs")

group("pattern length")
E.row = 10
E:set_length(4)
eq({ #P[0], E.row }, { 4, 3 }, "shortening a pattern pulls the cursor in")
E:set_length(20)
eq({ #P[0], P[0][20].note }, { 20, 0 }, "lengthening it adds empty rows")

group("orders")
E.focus = "orders"
E:orders_key(b("0")); E:orders_key(b("7"))
eq({ E.song.orders[1].entries[1].pattern, #P[7] }, { 7, 16 }, "two hex digits pick a pattern, creating it empty")
E:orders_key(K.ENTER)
eq({ #E.song.orders[1].entries, E.ord_pos, E.song.orders[1].entries[2].pattern }, { 2, 1, 7 },
  "Enter repeats the entry after itself")
E:orders_key(b("+")); E:orders_key(b("+"))
eq(E.song.orders[1].entries[2].transpose, 2, "+ transposes it up")
for _ = 1, 60 do E:orders_key(b("-")) end
eq(E.song.orders[1].entries[2].transpose, -48, "transposes stop at 48 semitones")
E:orders_key(b("l"))
eq(E.song.orders[1].loop, 1, "l makes this entry the loop point")
E:orders_key(K.DELETE)
eq({ #E.song.orders[1].entries, E.song.orders[1].loop, E.ord_pos }, { 1, 0, 0 },
  "Delete removes it and pulls the loop point back")
eq(E:orders_key(K.DELETE), false, "a channel's last entry can't be deleted")

group("instrument fields")
E.focus = "ins"
local ins = E.song.instruments[1]
E.ins_field = 1
E:ins_key(K.RIGHT)
eq(ins.wave, 1, "Right steps the waveform")
E:ins_key(K.DOWN)
E:ins_key(b("+"))
eq(ins.adsr[1], 12, "+ adds 10 to the attack")
for _ = 1, 20 do E:ins_key(b("-")) end
eq(ins.adsr[1], 0, "and it never goes below 0")
E.ins_field = 10
E:ins_key(K.RIGHT)
eq(ins.filter, { 1, 128, 0 }, "the filter turns on as a low-pass")
E.ins_field = 14
eq(E:ins_key(K.RIGHT), false, "detune does nothing until voice 2 detunes")
E.ins_field = 13
E:ins_key(K.RIGHT)
E.ins_field = 14
for _ = 1, 100 do E:ins_key(b("+")) end
eq({ ins.voice2, ins.detune }, { "detune", 768 }, "voice 2 detunes, held at 768")
E.inst = 9
eq(E:ins_key(K.RIGHT), false, "an empty instrument slot has no fields")
ok(TrkSong.parse(TrkSong.write(E.song)), "after all that the song still writes and reads")
```

That is 36 assertions.

Register the suite in `game_tests.rs`:

```rust
#[test]
fn trk_edit() {
    run_suite(&with_libs(&["v3/tools/game_test_env.lua", "v3/apps/tracker/song.lua", "v3/apps/tracker/edit.lua", "v3/tools/test_trk_edit.lua"]), 36);
}
```

Run: `cargo test --manifest-path v3/Cargo.toml -p acid-lua --test game_tests trk_edit`
Expected: FAIL with `v3/apps/tracker/edit.lua: No such file`.

- [ ] **Step 2: Implement `TrkEdit`**

Create `v3/apps/tracker/edit.lua`:

```lua
-- TrkEdit: Acid Tracker's cursor and every edit, on a TrkSong. No drawing
-- here, so it is tested on its own (tools/test_trk_edit.lua). The grid
-- shows each channel's pattern at the view's order position; typing goes
-- into the cursor channel's pattern, which every order entry using that
-- pattern shares (tracker semantics).

TrkEdit = {}
-- Cursor slots in one channel's row "C-4 01 4 22 E-4": { column, hex digit }.
TrkEdit.SLOTS = { { "note" }, { "inst", 0 }, { "inst", 1 }, { "cmd" }, { "param", 0 }, { "param", 1 }, { "note2" } }
-- Two piano rows, GoatTracker style: semitones from C of the octave.
TrkEdit.PIANO = {
  z = 0, s = 1, x = 2, d = 3, c = 4, v = 5, g = 6, b = 7, h = 8, n = 9, j = 10, m = 11,
  q = 12, ["2"] = 13, w = 14, ["3"] = 15, e = 16, r = 17, ["5"] = 18, t = 19, ["6"] = 20,
  y = 21, ["7"] = 22, u = 23, i = 24,
}
TrkEdit.MAX_ENTRIES = 64
TrkEdit.INS_FIELDS = { "wave", "a", "d", "s", "r", "duty", "pwm", "vd", "vs", "flt", "cut", "res", "v2", "det" }
TrkEdit.FIELD_LABELS = {
  wave = "wave", a = "a", d = "d", s = "s", r = "r", duty = "duty", pwm = "pwm", vd = "vib", vs = "spd",
  flt = "flt", cut = "cut", res = "res", v2 = "v2", det = "det",
}
local FILTERS = { "off", "lp", "bp", "hp" }
local FILTER_MASK = { lp = 1, bp = 2, hp = 4 }
local VOICE2 = { "off", "detune", "octave", "fifth", "ring" }

function TrkEdit.new(song)
  return setmetatable({
    song = song, ch = 1, row = 0, slot_i = 1, order = 0, octave = 4, inst = 1, edit = false,
    focus = "grid", ord_ch = 1, ord_pos = 0, ord_digit = 0, ins_field = 1, dirty = false,
  }, { __index = TrkEdit })
end

-- The order entry a channel shows at the view's position (held to its list).
function TrkEdit:entry(ch)
  local es = self.song.orders[ch].entries
  return es[math.min(self.order, #es - 1) + 1]
end

function TrkEdit:rows(ch) return self.song.patterns[self:entry(ch).pattern] end

function TrkEdit:max_rows()
  local m = 0
  for ch = 1, TrkSong.CHANNELS do m = math.max(m, #self:rows(ch)) end
  return m
end

function TrkEdit:order_count()
  local m = 0
  for ch = 1, TrkSong.CHANNELS do m = math.max(m, #self.song.orders[ch].entries) end
  return m
end

function TrkEdit:slot()
  local s = TrkEdit.SLOTS[self.slot_i]
  return { col = s[1], digit = s[2] }
end

-- The cursor's row, after pulling the cursor inside a shorter pattern.
function TrkEdit:cell()
  local rows = self:rows(self.ch)
  if self.row >= #rows then self.row = #rows - 1 end
  return rows[self.row + 1]
end

function TrkEdit:move_row(d)
  self.row = (self.row + d) % #self:rows(self.ch)
end

function TrkEdit:move_slot(d)
  self.slot_i = self.slot_i + d
  if self.slot_i < 1 then
    self.slot_i = #TrkEdit.SLOTS
    self.ch = (self.ch - 2) % TrkSong.CHANNELS + 1
  elseif self.slot_i > #TrkEdit.SLOTS then
    self.slot_i = 1
    self.ch = self.ch % TrkSong.CHANNELS + 1
  end
  self:cell()
end

-- Tab: the channels, then the orders panel, then the instrument.
function TrkEdit:next_focus()
  if self.focus == "grid" then
    if self.ch < TrkSong.CHANNELS then
      self.ch = self.ch + 1
      self:cell()
    else
      self.focus = "orders"
    end
  elseif self.focus == "orders" then
    self.focus = "ins"
  else
    self.focus = "grid"
    self.ch = 1
    self:cell()
  end
end

function TrkEdit:set_octave(d) self.octave = math.max(0, math.min(7, self.octave + d)) end
function TrkEdit:step_inst(d) self.inst = math.max(1, math.min(TrkSong.MAX_INSTRUMENT, self.inst + d)) end

function TrkEdit:set_order(o)
  self.order = math.max(0, math.min(self:order_count() - 1, o))
  self:cell()
end

-- The note a piano key plays at the current octave, or nil.
function TrkEdit:piano_note(code)
  if code < 32 or code > 126 then return nil end
  local semi = TrkEdit.PIANO[string.char(code)]
  if not semi then return nil end
  local ona = self.octave * 12 + semi - 8
  if ona < 1 or ona > 88 then return nil end
  return ona
end

local function hex_digit(code)
  if code >= 48 and code <= 57 then return code - 48 end
  if code >= 65 and code <= 70 then return code - 55 end
  if code >= 97 and code <= 102 then return code - 87 end
end

-- One key typed into the grid in edit mode: whether the song changed, and
-- a note worth previewing.
function TrkEdit:grid_key(code)
  local cell, slot = self:cell(), self:slot()
  if code == string.byte(".") or code == AcidKeys.DELETE then
    if slot.col == "note" then cell.note = 0
    elseif slot.col == "inst" then cell.inst = 0
    elseif slot.col == "cmd" then cell.cmd, cell.param = "", 0
    elseif slot.col == "param" then cell.param = 0
    else cell.note2 = 0 end
    self:move_row(1)
    return true
  end
  if slot.col == "note" or slot.col == "note2" then
    if slot.col == "note" and code == string.byte("`") then
      cell.note = TrkSong.NOTE_OFF
      self:move_row(1)
      return true
    end
    local ona = self:piano_note(code)
    if not ona then return false end
    if slot.col == "note" then
      cell.note = ona
      if cell.inst == 0 then cell.inst = self.inst end
    else
      cell.note2 = ona
    end
    self:move_row(1)
    return true, ona
  end
  if slot.col == "cmd" then
    local ch = code >= 32 and code <= 126 and string.char(code):upper()
    if not ch or not TrkSong.COMMANDS:find(ch, 1, true) then return false end
    cell.cmd = ch
    self.slot_i = self.slot_i + 1
    return true
  end
  local v = hex_digit(code)
  if not v then return false end
  local old = cell[slot.col]
  local new = slot.digit == 0 and (v * 16 + old % 16) or (old - old % 16 + v)
  if slot.col == "inst" then new = math.min(new, TrkSong.MAX_INSTRUMENT) end
  cell[slot.col] = new
  if slot.digit == 0 then
    self.slot_i = self.slot_i + 1
  else
    self.slot_i = self.slot_i - 1
    self:move_row(1)
  end
  return true
end

function TrkEdit:set_length(n)
  local rows = self:rows(self.ch)
  while #rows < n do rows[#rows + 1] = { note = 0, inst = 0, cmd = "", param = 0, note2 = 0 } end
  while #rows > n do rows[#rows] = nil end
  self:cell()
end

function TrkEdit:ensure_pattern(n)
  if not self.song.patterns[n] then self.song.patterns[n] = TrkSong.empty_rows(TrkSong.DEFAULT_ROWS) end
end

function TrkEdit:clamp_ord()
  self.ord_pos = math.min(self.ord_pos, #self.song.orders[self.ord_ch].entries - 1)
  self.ord_digit = 0
end

-- One key in the orders panel: whether the song changed.
function TrkEdit:orders_key(code)
  local K = AcidKeys
  local o = self.song.orders[self.ord_ch]
  local e = o.entries[self.ord_pos + 1]
  if code == K.UP then
    self.ord_ch = (self.ord_ch - 2) % TrkSong.CHANNELS + 1
    self:clamp_ord()
    return false
  elseif code == K.DOWN then
    self.ord_ch = self.ord_ch % TrkSong.CHANNELS + 1
    self:clamp_ord()
    return false
  elseif code == K.LEFT or code == K.RIGHT then
    local d = code == K.LEFT and -1 or 1
    self.ord_pos = math.max(0, math.min(#o.entries - 1, self.ord_pos + d))
    self.ord_digit = 0
    self:set_order(self.ord_pos)
    return false
  elseif code == string.byte("+") or code == string.byte("-") then
    local d = code == string.byte("+") and 1 or -1
    e.transpose = TrkSong.clamp(e.transpose + d, "transpose")
    return true
  elseif code == string.byte("l") then
    o.loop = self.ord_pos
    return true
  elseif code == K.ENTER then
    if #o.entries >= TrkEdit.MAX_ENTRIES then return false end
    table.insert(o.entries, self.ord_pos + 2, { pattern = e.pattern, transpose = e.transpose })
    self.ord_pos = self.ord_pos + 1
    return true
  elseif code == K.DELETE then
    if #o.entries == 1 then return false end
    table.remove(o.entries, self.ord_pos + 1)
    if o.loop >= #o.entries then o.loop = #o.entries - 1 end
    self:clamp_ord()
    return true
  end
  local v = hex_digit(code)
  if not v then return false end
  local p = self.ord_digit == 0 and (v * 16 + e.pattern % 16) or (e.pattern - e.pattern % 16 + v)
  e.pattern = math.min(p, TrkSong.MAX_PATTERN)
  self:ensure_pattern(e.pattern)
  self.ord_digit = 1 - self.ord_digit
  return true
end

local function cycle(list, cur, d)
  local i = 1
  for k, v in ipairs(list) do
    if v == cur then i = k end
  end
  return list[(i - 1 + d) % #list + 1]
end

function TrkEdit.field_text(ins, id)
  if id == "wave" then return TrkSong.WAVES[ins.wave + 1] end
  if id == "a" then return tostring(ins.adsr[1]) end
  if id == "d" then return tostring(ins.adsr[2]) end
  if id == "s" then return tostring(ins.adsr[3]) end
  if id == "r" then return tostring(ins.adsr[4]) end
  if id == "duty" then return tostring(ins.duty) end
  if id == "pwm" then return tostring(ins.pwm) end
  if id == "vd" then return tostring(ins.vib[1]) end
  if id == "vs" then return tostring(ins.vib[2]) end
  if id == "flt" then return ins.filter and TrkSong.FILTER_MODES[ins.filter[1]] or "off" end
  if id == "cut" then return ins.filter and tostring(ins.filter[2]) or "-" end
  if id == "res" then return ins.filter and tostring(ins.filter[3]) or "-" end
  if id == "v2" then return ins.voice2 end
  if id == "det" then return ins.voice2 == "detune" and tostring(ins.detune) or "-" end
end

-- Changes one field of a built-in by d, inside acid-sound's ranges. False
-- when the field doesn't apply (cutoff with no filter, detune unless
-- voice 2 detunes).
function TrkEdit.adjust(ins, id, d)
  local step = d > 0 and 1 or -1
  if id == "wave" then
    ins.wave = (ins.wave + step) % #TrkSong.WAVES
  elseif id == "a" or id == "d" or id == "r" then
    local k = ({ a = 1, d = 2, r = 4 })[id]
    ins.adsr[k] = TrkSong.clamp(ins.adsr[k] + d, "adsr")
  elseif id == "s" then
    ins.adsr[3] = TrkSong.clamp(ins.adsr[3] + d, "sustain")
  elseif id == "duty" then
    ins.duty = TrkSong.clamp(ins.duty + d, "duty")
  elseif id == "pwm" then
    ins.pwm = TrkSong.clamp(ins.pwm + d, "pwm")
  elseif id == "vd" then
    ins.vib[1] = TrkSong.clamp(ins.vib[1] + d, "vib")
  elseif id == "vs" then
    ins.vib[2] = TrkSong.clamp(ins.vib[2] + d, "vib")
  elseif id == "flt" then
    local m = cycle(FILTERS, ins.filter and TrkSong.FILTER_MODES[ins.filter[1]] or "off", step)
    if m == "off" then
      ins.filter = nil
    else
      ins.filter = { FILTER_MASK[m], ins.filter and ins.filter[2] or 128, ins.filter and ins.filter[3] or 0 }
    end
  elseif id == "cut" or id == "res" then
    if not ins.filter then return false end
    local k = id == "cut" and 2 or 3
    ins.filter[k] = TrkSong.clamp(ins.filter[k] + d, id == "cut" and "cutoff" or "res")
  elseif id == "v2" then
    ins.voice2 = cycle(VOICE2, ins.voice2, step)
  elseif id == "det" then
    if ins.voice2 ~= "detune" then return false end
    ins.detune = TrkSong.clamp(ins.detune + d, "detune")
  end
  return true
end

-- One key in the instrument panel: Up/Down choose a field, Left/Right
-- change it by 1, -/+ by 10. Whether the song changed.
function TrkEdit:ins_key(code)
  local K = AcidKeys
  local ins = self.song.instruments[self.inst]
  if not ins or ins.kind ~= "builtin" then return false end
  local n = #TrkEdit.INS_FIELDS
  if code == K.UP then
    self.ins_field = (self.ins_field - 2) % n + 1
    return false
  elseif code == K.DOWN then
    self.ins_field = self.ins_field % n + 1
    return false
  end
  local d = (code == K.RIGHT and 1) or (code == K.LEFT and -1)
    or (code == string.byte("+") and 10) or (code == string.byte("-") and -10)
  if not d then return false end
  return TrkEdit.adjust(ins, TrkEdit.INS_FIELDS[self.ins_field], d)
end
```

- [ ] **Step 3: Run the suite and watch it pass**

Run: `cargo test --manifest-path v3/Cargo.toml -p acid-lua --test game_tests trk_edit`
Expected: PASS, with 36 assertions.

- [ ] **Step 4: Commit**

```bash
git add v3/apps/tracker/edit.lua v3/tools/test_trk_edit.lua v3/crates/acid-lua/tests/game_tests.rs
git commit -m "Tracker: TrkEdit cursor, grid typing, orders and instrument fields

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 4: `TrkLayout` and `TrkCmd`

**Files:**
- Create: `v3/apps/tracker/layout.lua`
- Create: `v3/apps/tracker/cmd.lua`
- Create: `v3/tools/test_trk_layout_cmd.lua`
- Modify: `v3/crates/acid-lua/tests/game_tests.rs`

**Interfaces:**
- **`TrkLayout.compute(w, h)`** returns `L`. Its fields:
  - `w`, `h`, `x`
  - the y positions `status_y`, `head_y`, `grid_y`, `ord_y`, `ins_y`, `msg_y`
  - `rows`, the number of grid rows that fit
  - `ch_x[1..4]`, the channel x positions
  - `text_cols`, the number of text columns
- **`TrkLayout` constants:**
  - `TITLE_H = 16`, `CH_W = 6`, `CH_H = 8`, `MARGIN = 4`
  - `CELL_CHARS = 15`, `COL_CHARS = 16`, `ROWNUM_CHARS = 3`
  - `ORDER_LINES = 4`, `INS_LINES = 3`
  - `SLOT_CHARS`, `SLOT_W`
- **`TrkCmd`:**
  - `TrkCmd.parse(line)` returns `{ name, args, rest }`, or `nil, err`. An empty line gives `err` as `""`.
  - Helpers: `TrkCmd.int(s)`, `TrkCmd.hex(s)`, `TrkCmd.home_path(name)`
  - `TrkCmd.USAGE`

- [ ] **Step 1: Write the failing tests**

Create `v3/tools/test_trk_layout_cmd.lua`:

```lua
-- TrkLayout (apps/tracker/layout.lua) and TrkCmd (apps/tracker/cmd.lua).

group("layout at the default size")
local L = TrkLayout.compute(480, 320)
eq({ L.status_y, L.head_y, L.grid_y }, { 19, 29, 38 }, "the status line, channel headings, then the grid")
eq({ L.ord_y, L.ins_y, L.msg_y }, { 250, 284, 310 }, "orders, instrument and message lines along the bottom")
eq(L.rows, 26, "26 pattern rows show")
eq(L.ch_x, { 23, 119, 215, 311 }, "four channels, 16 characters apart")
ok(L.ch_x[4] + TrkLayout.CELL_CHARS * TrkLayout.CH_W <= 420 - L.x, "the four channels fit even the smallest window")
local S = TrkLayout.compute(420, 240)
eq({ S.rows, S.ord_y }, { 16, 170 }, "the smallest window still shows 16 rows")

group("commands")
eq(TrkCmd.parse("speed 9"), { name = "speed", args = { "9" }, rest = "9" }, "a command and its argument")
eq(TrkCmd.parse("title  My   Song "), { name = "title", args = { "My", "Song" }, rest = "My   Song" },
  "rest keeps the text as typed")
eq({ TrkCmd.parse("zap") }, { nil, "unknown command: zap" }, "an unknown command is named")
eq({ TrkCmd.parse("speed") }, { nil, "usage: speed 1-31" }, "a missing argument shows the usage")
eq({ TrkCmd.parse("  ") }, { nil, "" }, "an empty line is nothing")
eq({ TrkCmd.int("12"), TrkCmd.int("-3"), TrkCmd.int("x"), TrkCmd.int("99999999999999999999") }, { 12, -3 },
  "whole numbers only, and none too big to hold")
eq({ TrkCmd.hex("2"), TrkCmd.hex("3F"), TrkCmd.hex("G1"), TrkCmd.hex("123") }, { 2, 63 },
  "instrument numbers are one or two hex digits")
eq({ TrkCmd.home_path("groove"), TrkCmd.home_path("music/a.trk") },
  { "v3/fsroot/Home/groove.trk", "v3/fsroot/Home/music/a.trk" }, "song names live under Home, .trk added")
eq({ TrkCmd.home_path("../x"), TrkCmd.home_path("/etc/x"), TrkCmd.home_path("a//b"), TrkCmd.home_path("a/") }, {},
  "and can't climb out of it")
```

That is 15 assertions.

Register the suite:

```rust
#[test]
fn trk_layout_and_commands() {
    run_suite(&["v3/tools/game_test_env.lua", "v3/apps/tracker/layout.lua", "v3/apps/tracker/cmd.lua", "v3/tools/test_trk_layout_cmd.lua"], 15);
}
```

Run: `cargo test --manifest-path v3/Cargo.toml -p acid-lua --test game_tests trk_layout`
Expected: FAIL, because the files are missing.

- [ ] **Step 2: Implement both modules**

Create `v3/apps/tracker/layout.lua`:

```lua
-- TrkLayout: where Acid Tracker draws, for a window size. Top to bottom:
-- the status line, channel headings, the pattern grid, the orders panel
-- (one line per channel), the instrument panel and the message / command
-- line. Text is always 6x8 (the app isn't font-scalable).

TrkLayout = {}
TrkLayout.TITLE_H = 16
TrkLayout.CH_W, TrkLayout.CH_H = 6, 8
TrkLayout.MARGIN = 4
TrkLayout.CELL_CHARS = 15        -- "C-4 01 4 22 E-4"
TrkLayout.COL_CHARS = 16         -- a cell and a gap
TrkLayout.ROWNUM_CHARS = 3       -- "0A "
TrkLayout.ORDER_LINES = 4
TrkLayout.INS_LINES = 3
-- Where each cursor column sits inside a cell, and how wide it is.
TrkLayout.SLOT_CHARS = { note = 0, inst = 4, cmd = 7, param = 9, note2 = 12 }
TrkLayout.SLOT_W = { note = 3, inst = 2, cmd = 1, param = 2, note2 = 3 }

function TrkLayout.compute(w, h)
  local S = TrkLayout
  local L = { w = w, h = h }
  L.x = 1 + S.MARGIN
  L.status_y = S.TITLE_H + 3
  L.head_y = L.status_y + S.CH_H + 2
  L.grid_y = L.head_y + S.CH_H + 1
  L.msg_y = h - 2 - S.CH_H
  L.ins_y = L.msg_y - 2 - S.INS_LINES * S.CH_H
  L.ord_y = L.ins_y - 2 - S.ORDER_LINES * S.CH_H
  L.rows = math.max(1, (L.ord_y - 2 - L.grid_y) // S.CH_H)
  L.ch_x = {}
  for ch = 1, 4 do L.ch_x[ch] = L.x + (S.ROWNUM_CHARS + (ch - 1) * S.COL_CHARS) * S.CH_W end
  L.text_cols = (w - 2 * L.x) // S.CH_W
  return L
end
```

Create `v3/apps/tracker/cmd.lua`:

```lua
-- TrkCmd: Acid Tracker's Esc command line. Parsing only; TrackerApp runs
-- the commands. Numbers are whole and must fit, song names stay under
-- Home.

TrkCmd = {}
TrkCmd.USAGE = {
  w = "w [name]", o = "o name", new = "new", speed = "speed 1-31", len = "len 1-64",
  title = "title text", ins = "ins N  or  ins N script PATH NAME", name = "name text",
  arp = "arp [a [b [c]]]", donor = "donor 1-4", q = "q",
}
-- Fewest and most arguments each command takes.
local ARGS = {
  w = { 0, 1 }, o = { 1, 1 }, new = { 0, 0 }, speed = { 1, 1 }, len = { 1, 1 }, title = { 0, 99 },
  ins = { 1, 4 }, name = { 0, 99 }, arp = { 0, 3 }, donor = { 1, 1 }, q = { 0, 0 },
}

-- A typed line -> { name, args, rest } (rest is everything after the
-- name, as typed), or nil and a message ("" for an empty line).
function TrkCmd.parse(line)
  local words = {}
  for w in line:gmatch("%S+") do words[#words + 1] = w end
  local name = table.remove(words, 1)
  if not name then return nil, "" end
  local n = ARGS[name]
  if not n then return nil, "unknown command: " .. name end
  if #words < n[1] or #words > n[2] then return nil, "usage: " .. TrkCmd.USAGE[name] end
  return { name = name, args = words, rest = line:match("^%s*%S+%s*(.-)%s*$") }
end

-- A whole number, or nil (not a number, or too big to hold).
function TrkCmd.int(s)
  if s and s:match("^[+-]?%d+$") then return math.tointeger(tonumber(s)) end
end

-- One or two hex digits, or nil.
function TrkCmd.hex(s)
  if s and s:match("^%x%x?$") then return tonumber(s, 16) end
end

-- A song name -> its path under Home with .trk added, or nil if it would
-- leave Home or isn't a plain name.
function TrkCmd.home_path(name)
  if name == "" or name:sub(1, 1) == "/" or name:sub(-1) == "/" or name:find("//", 1, true) then return nil end
  for seg in name:gmatch("[^/]+") do
    if seg == "." or seg == ".." then return nil end
  end
  if name:sub(-4) ~= ".trk" then name = name .. ".trk" end
  return "v3/fsroot/Home/" .. name
end
```

- [ ] **Step 3: Run the suite and watch it pass**

Run: `cargo test --manifest-path v3/Cargo.toml -p acid-lua --test game_tests trk_layout`
Expected: PASS, with 15 assertions.

- [ ] **Step 4: Commit**

```bash
git add v3/apps/tracker/layout.lua v3/apps/tracker/cmd.lua v3/tools/test_trk_layout_cmd.lua v3/crates/acid-lua/tests/game_tests.rs
git commit -m "Tracker: layout and command-line parsing

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 5: The Acid Tracker app

**Files:**
- Create: `v3/apps/tracker.app.toml`
- Create: `v3/apps/tracker.lua`
- Create: `v3/tools/test_tracker_app.lua`
- Modify: `v3/crates/acid-lua/tests/game_tests.rs`
- Modify: `v3/crates/acid-os/tests/games.rs`

**Interfaces:**
- **Consumes:**
  - `TrkSong`, `TrkEdit`, `TrkLayout` and `TrkCmd` (Tasks 1, 3, 4)
  - `AcidKeys.F1` to `F8`
  - the calls `acid_song_parse`, `acid_song_update`, `acid_song_free`, `acid_song_play`, `acid_song_stop`, `acid_song_position`, `acid_song_mute`, `acid_song_preview`
  - the file calls `acid_fs_read`, `acid_fs_write`, and `acid_spawn_app`
- **Produces:** `TrackerApp` (the global `GAME` in tests). Its fields:
  - `E`, the `TrkEdit`
  - `L`, the layout
  - `handle`, `path`, `playing`, `play_pos`, `muted`, `cmd`, `message`, `synced`, `armed`

  Its methods: `open_path(path)`, `new_song()`, `sync()`, `changed()`, `play(order, row)`, `stop()`, `run_command(line)`, `redraw()`.
- **The manifest:** the window is 480×320, resizable with a minimum of 420×240. `menu = false`, and libs is `tracker/song.lua, tracker/edit.lua, tracker/layout.lua, tracker/cmd.lua`.

- [ ] **Step 1: Write the failing app tests**

Create `v3/tools/test_tracker_app.lua`:

```lua
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
```

That is 38 assertions.

Register the suite in `game_tests.rs`:

```rust
const TRACKER_APP: [&str; 6] = [
    "v3/tools/game_test_env.lua",
    "v3/apps/tracker/song.lua",
    "v3/apps/tracker/edit.lua",
    "v3/apps/tracker/layout.lua",
    "v3/apps/tracker/cmd.lua",
    "v3/apps/tracker.lua",
];

#[test]
fn tracker_app() {
    let files: Vec<&str> = TRACKER_APP.iter().copied().chain(["v3/tools/test_tracker_app.lua"]).collect();
    run_suite_with("WIN_W, WIN_H = 480, 320", &with_libs(&files), 38);
}
```

Then add this after `sprite_paint` in `v3/crates/acid-os/tests/games.rs`, and add "Acid Tracker" to the list in its `//!` header:

```rust
#[test]
fn acid_tracker() { boots_and_draws("tracker"); }
```

Run: `cargo test --manifest-path v3/Cargo.toml -p acid-lua --test game_tests tracker_app`
Expected: FAIL, because `v3/apps/tracker.lua` is missing.

- [ ] **Step 2: Write the manifest**

Create `v3/apps/tracker.app.toml`:

```toml
name = Acid Tracker
w = 480
h = 320
desc = 4-channel tracker for .trk songs
multi = true
libs = tracker/song.lua, tracker/edit.lua, tracker/layout.lua, tracker/cmd.lua
resizable = true
min_w = 420
min_h = 240
menu = false
```

- [ ] **Step 3: Write the app**

Create `v3/apps/tracker.lua`:

```lua
-- Acid Tracker: a GoatTracker-style tracker for .trk songs, four channels
-- of two voices. The song is a TrkSong (tracker/song.lua), the cursor and
-- every edit a TrkEdit (tracker/edit.lua), positions TrkLayout
-- (tracker/layout.lua) and the Esc command line TrkCmd (tracker/cmd.lua).
-- The kernel plays the song: each change goes over with acid_song_update,
-- so edits are heard while it plays. See
-- docs/superpowers/specs/2026-10-06-acid-tracker-design.md §7.

TrackerApp = AcidApp:extend("TrackerApp")
TrackerApp.BG = 0x050607         -- THEME_BG
TrackerApp.PANEL = 0x0B1712      -- THEME_PANEL
TrackerApp.SEL_BG = 0x123322     -- THEME_PANEL's hover shade
TrackerApp.TEXT = 0xD4E6DB       -- THEME_TEXT
TrackerApp.MUTED = 0x9DAAA3      -- THEME_MUTED
TrackerApp.HARD = 0x00FF66       -- THEME_HARD
TrackerApp.DIM = 0x4A5650        -- a muted channel's notes
TrackerApp.BEAT = 0x0E1E18       -- every fourth row
TrackerApp.PLAY_MS = 40          -- how often the play position is read
TrackerApp.EDITOR_PATH = "v3/apps/editor.lua"
TrackerApp.EDITOR_W, TrackerApp.EDITOR_H = 420, 280   -- editor.app.toml's size
TrackerApp.HELP = "F1 play  F2 from row  F4 stop  F5-8 mute  Esc command  Space edit"

function TrackerApp:window_title() return "Acid Tracker" end

function TrackerApp:on_create()
  self.handle = nil
  self.path = nil
  self.playing = false
  self.play_pos = nil      -- { order, row } while playing
  self.muted = { false, false, false, false }
  self.cmd = nil           -- the command line's text while it's open
  self.armed = nil         -- "new" / "o" / "q" after one go on an unsaved song
  self.message = nil
  self.held = {}           -- key code -> channel, so its release ends the preview
  local arg = acid_launch_arg()
  if arg ~= nil and arg ~= "" then self:open_path(arg) else self:new_song() end
  self:layout(acid_window_size())
end

function TrackerApp:layout(w, h) self.L = TrkLayout.compute(w, h) end
function TrackerApp:on_resize(w, h) self:layout(w, h) end

function TrackerApp:on_destroy()
  if self.playing then acid_song_stop() end
  if self.handle then acid_song_free(self.handle) end
end

-- Songs ------------------------------------------------------------------

-- Text goes to the kernel's parser first (the reader of record); only then
-- is the Lua model read from it.
function TrackerApp:load_text(text)
  local handle, warnings = acid_song_parse(text)
  if not handle then return nil, warnings end
  local song, err = TrkSong.parse(text)
  if not song then
    acid_song_free(handle)
    return nil, err
  end
  return handle, song, warnings
end

function TrackerApp:adopt(handle, song, warnings)
  if self.playing then self:stop() end
  if self.handle then acid_song_free(self.handle) end
  self.handle = handle
  self.E = TrkEdit.new(song)
  self.synced = true
  self.armed = nil
  self.message = type(warnings) == "table" and warnings[1] or nil
end

function TrackerApp:new_song()
  local song = TrkSong.new()
  local handle, warnings = acid_song_parse(TrkSong.write(song))
  self:adopt(handle, song, warnings)
  self.path = nil
end

-- A file that won't load leaves a new song, and says why.
function TrackerApp:open_path(path)
  local text, err = acid_fs_read(path)
  if text then
    local handle, song, warnings = self:load_text(text)
    if handle then
      self:adopt(handle, song, warnings)
      self.path = path
      return
    end
    err = song
  end
  self:new_song()
  self.message = "can't open: " .. tostring(err)
end

-- Sends the edited song to the kernel; a playing copy keeps its place.
function TrackerApp:sync()
  if self.synced or not self.handle then return end
  self.synced = true
  local warnings, err = acid_song_update(self.handle, TrkSong.write(self.E.song))
  if not warnings then
    self.message = "song error: " .. tostring(err)
  elseif warnings[1] then
    self.message = warnings[1]
  end
end

-- After any edit: unsaved, and heard at once while the song plays.
function TrackerApp:changed()
  self.E.dirty = true
  self.synced = false
  self.armed = nil
  if self.playing then self:sync() end
end

-- new, o and q on an unsaved song need a second go.
function TrackerApp:confirmed(id)
  if not self.E.dirty or self.armed == id then
    self.armed = nil
    return true
  end
  self.armed = id
  self.message = "unsaved: " .. id .. " again to discard"
  return false
end

function TrackerApp:save(name)
  local path = self.path
  if name then
    path = TrkCmd.home_path(name)
    if not path then
      self.message = "not a file name"
      return
    end
  end
  if not path then
    self.message = "usage: " .. TrkCmd.USAGE.w
    return
  end
  local ok, err = acid_fs_write(path, TrkSong.write(self.E.song))
  if ok then
    self.path = path
    self.E.dirty = false
    self.message = "saved " .. path:match("[^/]*$")
  else
    self.message = "save failed: " .. tostring(err)
  end
end

-- Playback ---------------------------------------------------------------

function TrackerApp:play(order, row)
  if not self.handle then return end
  self:sync()
  acid_song_play(self.handle, order, row)
  self.playing = true
  self.play_pos = { order, row }
  for ch = 1, 4 do
    if self.muted[ch] then acid_song_mute(ch, true) end
  end
end

function TrackerApp:stop()
  acid_song_stop()
  self.playing = false
  self.play_pos = nil
end

function TrackerApp:toggle_mute(ch)
  self.muted[ch] = not self.muted[ch]
  if self.playing then acid_song_mute(ch, self.muted[ch]) end
end

function TrackerApp:preview(ch, note)
  if not self.handle then return end
  self:sync()
  acid_song_preview(self.handle, ch, note, self.E.inst)
end

function TrackerApp:poll_timeout_ms()
  return self.playing and self.PLAY_MS or 200
end

-- While playing, the grid follows the song (channel 1's place).
function TrackerApp:on_idle()
  if not self.playing then return end
  local o, r = acid_song_position()
  if not o then
    self.playing = false
    self.play_pos = nil
  elseif self.play_pos[1] ~= o or self.play_pos[2] ~= r then
    self.play_pos = { o, r }
    self.E:set_order(o)
  else
    return
  end
  self:redraw()
end

function TrackerApp:edit_script()
  local ins = self.E.song.instruments[self.E.inst]
  if ins and ins.kind == "script" then
    acid_spawn_app(self.EDITOR_PATH, self.EDITOR_W, self.EDITOR_H, "v3/fsroot/" .. ins.path)
  else
    self.message = "not a script instrument"
  end
end

-- Keys -------------------------------------------------------------------

function TrackerApp:on_key(code, pressed)
  if not pressed then
    local ch = self.held[code]
    if ch then
      self.held[code] = nil
      if self.handle then acid_song_preview(self.handle, ch, 0, 0) end
    end
    return
  end
  if self.cmd then
    self:cmd_key(code)
    return
  end
  local K, E = AcidKeys, self.E
  if code == K.ESCAPE then
    self.cmd = ""
  elseif code == K.F1 then
    self:play(0, 0)
  elseif code == K.F2 then
    self:play(E.order, E.row)
  elseif code == K.F4 then
    self:stop()
  elseif code >= K.F5 and code <= K.F8 then
    self:toggle_mute(code - K.F5 + 1)
  elseif code == K.TAB then
    E:next_focus()
  elseif code == string.byte(" ") then
    E.edit = not E.edit
  elseif code == string.byte("<") or code == string.byte(">") then
    E:set_octave(code == string.byte("<") and -1 or 1)
  elseif code == string.byte("[") or code == string.byte("]") then
    E:step_inst(code == string.byte("[") and -1 or 1)
  else
    self:focus_key(code)
    return
  end
  self:redraw()
end

-- A key for whichever part has focus: the grid, the orders or the instrument.
function TrackerApp:focus_key(code)
  local E, K = self.E, AcidKeys
  local changed, note = false, nil
  if E.focus == "grid" then
    if code == K.UP then E:move_row(-1)
    elseif code == K.DOWN then E:move_row(1)
    elseif code == K.LEFT then E:move_slot(-1)
    elseif code == K.RIGHT then E:move_slot(1)
    elseif E.edit then changed, note = E:grid_key(code)
    else note = E:piano_note(code) end
  elseif E.focus == "orders" then
    changed = E:orders_key(code)
  elseif code == string.byte("e") then
    self:edit_script()
  elseif code == string.byte("r") then
    self.synced = false
    self:sync()
  else
    changed = E:ins_key(code)
  end
  if changed then self:changed() end
  if note then
    self.held[code] = E.ch
    self:preview(E.ch, note)
  end
  self:redraw()
end

function TrackerApp:cmd_key(code)
  local K = AcidKeys
  if code == K.ESCAPE then
    self.cmd = nil
  elseif code == K.ENTER then
    local line = self.cmd
    self.cmd = nil
    self:run_command(line)
  elseif code == K.BACKSPACE then
    self.cmd = self.cmd:sub(1, -2)
  elseif code >= 32 and code <= 126 and #self.cmd < self.L.text_cols - 2 then
    self.cmd = self.cmd .. string.char(code)
  end
  self:redraw()
end

function TrackerApp:usage(name) self.message = "usage: " .. TrkCmd.USAGE[name] end

function TrackerApp:run_command(line)
  local c, err = TrkCmd.parse(line)
  self.message = nil
  if not c then
    if err ~= "" then self.message = err end
    return
  end
  if c.name ~= "new" and c.name ~= "o" and c.name ~= "q" then self.armed = nil end
  local E, a = self.E, c.args
  local ins = E.song.instruments[E.inst]
  if c.name == "w" then
    self:save(a[1])
  elseif c.name == "o" then
    local path = TrkCmd.home_path(a[1])
    if not path then
      self.message = "not a file name"
    elseif self:confirmed("o") then
      self:open_path(path)
    end
  elseif c.name == "new" then
    if self:confirmed("new") then self:new_song() end
  elseif c.name == "q" then
    if self:confirmed("q") then self:quit() end
  elseif c.name == "speed" or c.name == "donor" or c.name == "len" then
    local n = TrkCmd.int(a[1])
    local hi = ({ speed = 31, donor = 4, len = TrkSong.MAX_ROWS })[c.name]
    if not n or n < 1 or n > hi then return self:usage(c.name) end
    if c.name == "speed" then
      E.song.speed = n
    elseif c.name == "donor" then
      E.song.donor = n
    else
      E:set_length(n)
    end
    self:changed()
  elseif c.name == "title" then
    E.song.title = c.rest
    self:changed()
  elseif c.name == "name" then
    if not ins then
      self.message = string.format("no instrument %02X", E.inst)
      return
    end
    ins.name = c.rest
    self:changed()
  elseif c.name == "arp" then
    if not ins or ins.kind ~= "builtin" then
      self.message = "not a built-in instrument"
      return
    end
    local arp = {}
    for i, s in ipairs(a) do
      local n = TrkCmd.int(s)
      if not n or n < -48 or n > 48 then
        self.message = "usage: " .. TrkCmd.USAGE.arp .. " (-48 to 48)"
        return
      end
      arp[i] = n
    end
    ins.arp = arp
    self:changed()
  elseif c.name == "ins" then
    local n = TrkCmd.hex(a[1])
    if not n or n < 1 or n > TrkSong.MAX_INSTRUMENT then return self:usage("ins") end
    if a[2] == "script" and #a == 4 then
      local old = E.song.instruments[n]
      E.song.instruments[n] = TrkSong.script(old and old.name or "Script", a[3], a[4])
      self:changed()
    elseif #a ~= 1 then
      return self:usage("ins")
    elseif not E.song.instruments[n] then
      E.song.instruments[n] = TrkSong.builtin(string.format("Inst %02X", n))
      self:changed()
    end
    E.inst = n
  end
end

-- Drawing ----------------------------------------------------------------

function TrackerApp:redraw()
  acid_begin_frame()
  acid_clear_user_area()
  acid_draw_window_frame(self:window_title())
  self:draw_status()
  self:draw_grid()
  self:draw_orders()
  self:draw_instrument()
  self:draw_message()
  -- Last, so a finished border means a finished frame (the golden test
  -- waits for it).
  acid_draw_window_border()
  acid_end_frame()
end

-- A full-width line of text, cut to fit.
function TrackerApp:line(text, y, fg, bg)
  acid_draw_text(text:sub(1, self.L.text_cols), self.L.x, y, fg, bg)
end

function TrackerApp:status_text()
  local E = self.E
  local pos = self.play_pos or { E.order, E.row }
  local ins = E.song.instruments[E.inst]
  local tag = self.playing and "  [PLAY]" or (E.edit and "  [EDIT]" or "")
  return string.format("ORD %02X/%02X  ROW %02X  SPD %d  OCT %d  INS %02X %s%s",
    pos[1], E:order_count() - 1, pos[2], E.song.speed, E.octave, E.inst,
    ins and ins.name:sub(1, 12) or "--", tag)
end

function TrackerApp:message_text()
  if self.cmd then return ":" .. self.cmd .. "_" end
  return self.message or self.HELP
end

function TrackerApp:draw_status()
  self:line(self:status_text(), self.L.status_y, self.E.edit and self.HARD or self.TEXT, self.BG)
end

function TrackerApp:draw_message()
  self:line(self:message_text(), self.L.msg_y, self.cmd and self.HARD or self.MUTED, self.BG)
end

-- One channel's row. On the cursor's cell the field under the cursor is
-- drawn inverted, as its own piece, so no two texts overlap.
function TrackerApp:draw_row_text(text, x, y, fg, bg, off, w, hl)
  if not off then
    acid_draw_text(text, x, y, fg, bg)
    return
  end
  local cw = TrkLayout.CH_W
  if off > 0 then acid_draw_text(text:sub(1, off), x, y, fg, bg) end
  acid_draw_text(text:sub(off + 1, off + w), x + off * cw, y, self.BG, hl)
  if off + w < #text then acid_draw_text(text:sub(off + w + 1), x + (off + w) * cw, y, fg, bg) end
end

-- The rows scroll past a fixed middle line: the cursor's row, or the
-- playing row while the song plays here.
function TrackerApp:draw_grid()
  local L, E, S = self.L, self.E, TrkLayout
  for ch = 1, 4 do
    local m = self.muted[ch]
    acid_draw_text(m and (ch .. " muted") or tostring(ch), L.ch_x[ch], L.head_y, m and self.DIM or self.MUTED, self.BG)
  end
  local following = self.play_pos ~= nil and self.play_pos[1] == E.order
  local center = following and self.play_pos[2] or E.row
  local top = center - L.rows // 2
  local width = (S.ROWNUM_CHARS + 4 * S.COL_CHARS - 1) * S.CH_W
  local slot = E:slot()
  local max_rows = E:max_rows()
  for i = 0, L.rows - 1 do
    local r = top + i
    if r >= 0 and r < max_rows then
      local y = L.grid_y + i * S.CH_H
      local bg = (following and r == self.play_pos[2]) and self.SEL_BG or (r % 4 == 0 and self.BEAT or self.BG)
      acid_fill_rect(L.x, y, width, S.CH_H, bg)
      acid_draw_text(string.format("%02X", r), L.x, y, self.MUTED, bg)
      for ch = 1, 4 do
        local row = E:rows(ch)[r + 1]
        if row then
          local off, w
          if E.focus == "grid" and ch == E.ch and r == E.row then
            off, w = S.SLOT_CHARS[slot.col], S.SLOT_W[slot.col]
            if slot.digit then off, w = off + slot.digit, 1 end
          end
          local fg = self.muted[ch] and self.DIM or self.TEXT
          self:draw_row_text(TrkSong.row_text(row), L.ch_x[ch], y, fg, bg, off, w, E.edit and self.HARD or self.MUTED)
        end
      end
    end
  end
end

-- One line per channel: its loop point, then the entries around the
-- view's order position ("03+5" is pattern 03 transposed up 5).
function TrackerApp:draw_orders()
  local L, E, S = self.L, self.E, TrkLayout
  acid_fill_rect(L.x, L.ord_y - 1, L.w - 2 * L.x, S.ORDER_LINES * S.CH_H + 1, self.PANEL)
  local per = math.max(1, (L.text_cols - 6) // 6)
  for ch = 1, 4 do
    local o = E.song.orders[ch]
    local y = L.ord_y + (ch - 1) * S.CH_H
    acid_draw_text(string.format("%d L%02X", ch, o.loop), L.x, y, self.MUTED, self.PANEL)
    local first = math.max(0, math.min(E.order - per // 2, #o.entries - per))
    for i = first, math.min(#o.entries - 1, first + per - 1) do
      local e = o.entries[i + 1]
      local label = string.format("%02X", e.pattern) .. (e.transpose ~= 0 and string.format("%+d", e.transpose) or "")
      local sel = E.focus == "orders" and E.ord_ch == ch and E.ord_pos == i
      local fg = sel and self.BG or (i == E.order and self.HARD or self.TEXT)
      acid_draw_text(label, L.x + (6 + (i - first) * 6) * S.CH_W, y, fg, sel and self.HARD or self.PANEL)
    end
  end
end

-- The current instrument: its name, then its fields (a built-in) or its
-- script (a script instrument).
function TrackerApp:draw_instrument()
  local L, E, S = self.L, self.E, TrkLayout
  acid_fill_rect(L.x, L.ins_y - 1, L.w - 2 * L.x, S.INS_LINES * S.CH_H + 1, self.PANEL)
  local ins = E.song.instruments[E.inst]
  local head = string.format("INS %02X ", E.inst)
  if not ins then
    self:line(head .. string.format("empty: :ins %02X makes one", E.inst), L.ins_y, self.MUTED, self.PANEL)
    return
  end
  self:line(head .. ins.name .. (ins.kind == "script" and "  (script)" or ""), L.ins_y, self.TEXT, self.PANEL)
  if ins.kind == "script" then
    self:line(ins.path .. " : " .. ins.block .. "   e edit  r reload", L.ins_y + S.CH_H, self.MUTED, self.PANEL)
    return
  end
  local x, line = 0, 1
  for i, id in ipairs(TrkEdit.INS_FIELDS) do
    local text = TrkEdit.FIELD_LABELS[id] .. " " .. TrkEdit.field_text(ins, id)
    if x + #text > L.text_cols then x, line = 0, line + 1 end
    if line >= S.INS_LINES then break end
    local sel = E.focus == "ins" and E.ins_field == i
    acid_draw_text(text, L.x + x * S.CH_W, L.ins_y + line * S.CH_H, sel and self.BG or self.TEXT, sel and self.HARD or self.PANEL)
    x = x + #text + 2
  end
end

TrackerApp:new():start()
```

- [ ] **Step 4: Run the suites**

Run: `cargo test --manifest-path v3/Cargo.toml -p acid-lua --test game_tests tracker_app`
Expected: PASS, with 38 assertions.

Run: `cargo test --manifest-path v3/Cargo.toml -p acid-os --test games acid_tracker`
Expected: PASS. This test boots the app under the real kernel with the real Rust song parser. If it fails, check that the new-song text loads with `acid_song_parse`. A Lua error ends the app before it draws.

Run: `cargo test --manifest-path v3/Cargo.toml -p acid-os --test games boot_discovers_every_manifest`
Expected: PASS. The new manifest is discovered with the rest.

- [ ] **Step 5: Commit**

```bash
git add v3/apps/tracker.app.toml v3/apps/tracker.lua v3/tools/test_tracker_app.lua v3/crates/acid-lua/tests/game_tests.rs v3/crates/acid-os/tests/games.rs
git commit -m "Acid Tracker: the app (grid, orders, instrument, preview, playback, commands)

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 6: File Manager opens songs and sounds; Terminal's `play`

**Files:**
- Modify: `v3/apps/file_manager.lua`
- Modify: `v3/tools/test_file_manager.lua`
- Modify: `v3/apps/terminal.lua`
- Modify: `v3/tools/test_terminal.lua`
- Modify: `v3/crates/acid-lua/tests/game_tests.rs`, to change the counts: `file_manager` 34 → 36 and `terminal` 28 → 33

**Interfaces:**
- **Consumes:** the `acid_song_load/play/stop/free` and `acid_sound_load_file/play/stop/free` calls, and the fakes from Task 1.
- **Produces:**
  - File Manager: `.trk` → `acid_spawn_app("v3/apps/tracker.lua", 480, 320, path)`, and `.snd` → the Editor.
  - Terminal: `play FILE` and `play`, plus a new help line.

- [ ] **Step 1: Write the failing tests**

Append to `v3/tools/test_file_manager.lua`:

```lua
group("song and sound files")
FS["v3/fsroot/Music"] = { "a.trk", "b.snd" }
FS["v3/fsroot/Music/a.trk"] = "acid-track 1\n"
FS["v3/fsroot/Music/b.snd"] = "gate on\n"
G.dir = "v3/fsroot/Music"
G:scan_dir()
local function pick(name)
  for i, e in ipairs(G.entries) do
    if e.name == name then G.selected = i - 1 end
  end
end
pick("a.trk")
CALLS = {}
G:activate_selected()
eq(CALLS[1], { "spawn", "v3/apps/tracker.lua", 480, 320, "v3/fsroot/Music/a.trk" }, "a .trk opens in Acid Tracker with its path")
pick("b.snd")
CALLS = {}
G:activate_selected()
eq(CALLS[1], { "spawn", "v3/apps/editor.lua", 420, 280, "v3/fsroot/Music/b.snd" }, "a .snd opens in the Editor")
```

In `v3/tools/test_terminal.lua`, change the help assertion to:

```lua
eq(last(), "help, clear, pwd, cd, ls, cat, echo, run <app>, play <file>", "help lists the commands")
```

Then append:

```lua
group("play")
FS["v3/fsroot/Home/a.trk"] = "acid-track 1\n"
SOUND_CALLS = {}
type_line("play Home/a.trk")
eq(SOUND_CALLS, { { "parse", 1 }, { "play", 1, 0, 0 } }, "play loads a .trk and plays it from the start")
FS["v3/fsroot/Home/z.snd"] = "gate on\n"
SOUND_CALLS = {}
type_line("play Home/z.snd")
eq(SOUND_CALLS, { { "stop" }, { "free", 1 }, { "sound_load", 1 }, { "sound_play", 1, "", 40 } },
  "playing a .snd stops the song first, then starts the sound")
SOUND_CALLS = {}
type_line("play")
eq(SOUND_CALLS, { { "sound_stop", 1 }, { "sound_free", 1 } }, "play on its own stops everything")
type_line("play Home/missing.trk")
eq(last(), "play: not found", "a missing file is reported")
type_line("play notes.txt")
eq(last(), "play: not a .trk or .snd file", "other files are refused")
```

In `game_tests.rs`, change the `file_manager` count from 34 to 36, and the `terminal` count from 28 to 33.

Run: `cargo test --manifest-path v3/Cargo.toml -p acid-lua --test game_tests file_manager terminal`
Expected: both FAIL.

- [ ] **Step 2: Implement File Manager's file types**

In `v3/apps/file_manager.lua`:
1. Add `.trk` files in Acid Tracker, and `.snd` files in Editor, to the header comment.
2. Add these after the `SPRITE_*` constants:

```lua
FileManagerApp.TRACKER_PATH = "v3/apps/tracker.lua"
FileManagerApp.TRACKER_W = 480                -- tracker.app.toml's size
FileManagerApp.TRACKER_H = 320
```

3. In `activate_selected`, replace the `.lua` branch with these two branches:

```lua
  elseif ends_with(entry.name, ".lua") or ends_with(entry.name, ".snd") then
    acid_spawn_app(self.EDITOR_PATH, self.EDITOR_W, self.EDITOR_H, self.dir .. "/" .. entry.name)
  elseif ends_with(entry.name, ".trk") then
    acid_spawn_app(self.TRACKER_PATH, self.TRACKER_W, self.TRACKER_H, self.dir .. "/" .. entry.name)
```

- [ ] **Step 3: Implement Terminal's `play`**

In `v3/apps/terminal.lua`:
1. Add `elseif cmd == "play" then self:cmd_play(args)` to the command dispatch, before the `else`.
2. Change `cmd_help`'s line to `"help, clear, pwd, cd, ls, cat, echo, run <app>, play <file>"`.
3. Add these after `cmd_run`:

```lua
-- play FILE plays a .trk song or a .snd file's first sound; play on its
-- own stops them. One of each at a time: a new play stops the last.
function TerminalApp:cmd_play(args)
  self:stop_playing()
  if #args == 0 then return end
  local path = self:resolve_path(table.concat(args, " "))
  local function say(s) self.lines[#self.lines + 1] = "play: " .. s end
  if path:sub(-4) == ".trk" then
    local song, warnings = acid_song_load(path)
    if not song then return say(tostring(warnings)) end
    for _, w in ipairs(warnings) do say(w) end
    self.song = song
    acid_song_play(song, 0, 0)
  elseif path:sub(-4) == ".snd" then
    local prog, err = acid_sound_load_file(path)
    if not prog then return say(tostring(err)) end
    self.prog = prog
    self.sound = acid_sound_play(prog)
    if not self.sound then say("no free voice") end
  else
    say("not a .trk or .snd file")
  end
end

function TerminalApp:stop_playing()
  if self.song then
    acid_song_stop()
    acid_song_free(self.song)
    self.song = nil
  end
  if self.sound then
    acid_sound_stop(self.sound)
    self.sound = nil
  end
  if self.prog then
    acid_sound_free(self.prog)
    self.prog = nil
  end
end
```

4. Call `self:stop_playing()` at the start of `TerminalApp:on_destroy()`.

- [ ] **Step 4: Run every Lua suite**

Run: `cargo test --manifest-path v3/Cargo.toml -p acid-lua`
Expected: all pass, including `file_manager` (36), `terminal` (33), the resize, scroll and large-font variants, and `trk_*`/`tracker_app`.

- [ ] **Step 5: Commit**

```bash
git add v3/apps/file_manager.lua v3/apps/terminal.lua v3/tools/test_file_manager.lua v3/tools/test_terminal.lua v3/crates/acid-lua/tests/game_tests.rs
git commit -m "File Manager opens .trk in Acid Tracker and .snd in Editor; Terminal plays them

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 7: The tracker's golden screenshot

**Files:**
- Modify: `v3/crates/acid-os/tests/golden.rs`
- Create: `v3/crates/acid-os/tests/golden/acid_tracker.ppm`, approved once in Step 3

**Interfaces:**
- **Consumes:** the app (Task 5) and `Home/music/acid_groove.trk` (Task 2).

- [ ] **Step 1: Add the test**

Add this after `sprite_paint_matches_golden` in `v3/crates/acid-os/tests/golden.rs`:

```rust
#[test]
fn acid_tracker_matches_golden() {
    let p = FakePlatform::new(FakePlatform::repo_root());
    let k = boot_with(p.clone(), Screen::DEFAULT);
    wait_for_desktop(&k, Screen::DEFAULT);
    let (w, h) = (480, 320); // the manifest's size
    let (x, y) = acid_kernel::placement::cascade_position(k.screen(), k.with_state(|st| st.windows.count()), w, h);
    let tracker = k
        .spawn_app(SpawnRequest {
            script_path: format!("{APPS_DIR}/tracker.lua"),
            x, y, w, h, closable: true,
            arg: Some("v3/fsroot/Home/music/acid_groove.trk".into()),
            libs: Some("tracker/song.lua, tracker/edit.lua, tracker/layout.lua, tracker/cmd.lua".into()),
            force_cart: false,
        })
        .expect("Acid Tracker opens");
    k.activate_window(tracker);
    // Acid Tracker draws its border last, so a border means a whole frame.
    wait_for_border(&k, tracker, w, h);
    k.composite_frame();
    // Resizable, so the kernel draws a grip in the window's bottom-right 8x8.
    let (wx, wy, ww, wh) = k.with_state(|st| st.windows.by_task(tracker).map(|win| (win.x, win.y, win.w, win.h))).unwrap();
    let grip = ((wx + ww - 8) as usize, (wy + wh - 8) as usize, 8, 8);
    assert_matches_golden_masked(&p.display.last_frame().unwrap(), "acid_tracker.ppm", 640, &[clock_mask(Screen::DEFAULT), grip]);
}
```

- [ ] **Step 2: Run it and capture the first frame**

Run: `cargo test --manifest-path v3/Cargo.toml -p acid-os --test golden acid_tracker`
Expected: FAIL with `no golden acid_tracker.ppm yet: review .../v3/target/actual-acid_tracker.ppm`.

- [ ] **Step 3: Look at the frame, then approve it**

Convert the frame to PNG with this throwaway script. Write it in the session scratchpad, not the repo:

```python
import sys, zlib, struct
data = open(sys.argv[1], "rb").read()
magic, size, maxval, px = data.split(b"\n", 3)
w, h = map(int, size.split())
raw = b"".join(b"\x00" + px[y * w * 3:(y + 1) * w * 3] for y in range(h))
def chunk(t, d):
    c = struct.pack(">I", len(d)) + t + d
    return c + struct.pack(">I", zlib.crc32(t + d) & 0xffffffff)
png = (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0))
       + chunk(b"IDAT", zlib.compress(raw)) + chunk(b"IEND", b""))
open(sys.argv[2], "wb").write(png)
```

Run it as `python3 <script> v3/target/actual-acid_tracker.ppm <scratchpad>/acid_tracker.png`, then view the PNG with the Read tool. Approve the frame only if all of these hold:
- **Title:** the window title is "Acid Tracker".
- **Status line:** it reads `ORD 00/03  ROW 00  SPD 6  OCT 4  INS 01 Lead`.
- **Grid:** four channel columns with readable rows. Channel 1 starts `A-4 01 . .. ...`, channel 2 `A-1 02 . .. ...`, channel 3 `A-3 03 . .. ...`, and channel 4 `C-6 05 . .. ...`.
- **Panels:** the orders panel shows four lines, the instrument panel shows Lead's fields, and the bottom line shows the help text.
- **Fit:** nothing is cut off at the window's edges.

If anything is wrong, fix the app (back in Task 5's code) and repeat. When it's right, copy the frame into place:

```bash
cp v3/target/actual-acid_tracker.ppm v3/crates/acid-os/tests/golden/acid_tracker.ppm
```

Run: `cargo test --manifest-path v3/Cargo.toml -p acid-os --test golden`
Expected: all pass, and the other goldens are unchanged.

- [ ] **Step 4: Commit**

```bash
git add v3/crates/acid-os/tests/golden.rs v3/crates/acid-os/tests/golden/acid_tracker.ppm
git commit -m "Goldens: Acid Tracker with the Acid Groove demo song

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 8: The manual, the READMEs and the spec

**Files:**
- Create: `docs/manual-v3/11-music.md`
- Modify:
  - `docs/manual-v3/README.md`, the contents table
  - `docs/manual-v3/10-wasm-carts.md`, its "Next" link
  - `docs/manual-v3/01-getting-started.md`
  - `docs/manual-v3/05-sound.md`
  - `docs/manual-v3/09-api-reference.md`
- Modify: `v3/crates/acid-os/tests/manual.rs`, the `the_manual_is_complete` list
- Modify: `README.md`, the repository root
- Modify: `docs/superpowers/specs/2026-10-06-acid-tracker-design.md`, §7

**Interfaces:**
- **Consumes:** every name and behaviour from plan 1 and Tasks 1 to 7.
- **Checked by:** the manual tests in `v3/crates/acid-os/tests/manual.rs`:
  - Every fence is tagged, and every `lua snippet` parses.
  - Every `acid_*` name mentioned exists as a live global.
  - Every link resolves, and the chapter list is complete.

- [ ] **Step 1: Make the manual test expect chapter 11**

In `v3/crates/acid-os/tests/manual.rs`, add `"11-music.md"` to the `want` list in `the_manual_is_complete`, after `"10-wasm-carts.md"`.

Run: `cargo test --manifest-path v3/Cargo.toml -p acid-os --test manual the_manual_is_complete`
Expected: FAIL, because the chapter is missing.

- [ ] **Step 2: Write chapter 11**

Create `docs/manual-v3/11-music.md`:

````markdown
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

Open Acid Tracker from the File Manager: either `App/tracker.app.toml`, or
any `.trk` file such as `Home/music/acid_groove.trk`. You can also type
`run acid tracker` in the Terminal. Like Sprite Paint, it isn't in the Menu.

A song has **four channels**, and each channel has **two voices**. Each row
of a channel looks like this:

```text
C-4 01 4 22 E-4
```

Read left to right, the five fields are:
- a note
- an instrument
- a command and its parameter
- a second note

The second note plays on the channel's second voice. When a row leaves it
empty, the instrument is free to use that voice itself, for example for a
detuned double.

The window has four parts, from top to bottom:
- **The status line**, showing the order position, row, speed, octave,
  current instrument and mode.
- **The pattern grid**, showing each channel's pattern at the current order
  position. The rows scroll past a fixed middle line. While the song plays,
  the grid follows it.
- **The orders panel**, with one line per channel. Each line shows the
  loop point (`L00`), then the patterns the channel plays in turn. `03+5`
  means pattern 03, transposed up 5 semitones.
- **The instrument panel**, showing the current instrument's fields.

### Keys

| Key | What it does |
|---|---|
| `z s x d c v g b h n j m` | Notes from C to B in the current octave |
| `q 2 w 3 e r 5 t 6 y 7 u i` | The octave above |
| Space | Edit mode on or off. Off, the note keys only play the note. |
| `` ` `` | Note-off (edit mode, note column) |
| `.` or Delete | Clear the field under the cursor |
| `0`–`9`, `a`–`f` | Instrument, command parameter and pattern numbers |
| Arrow keys | Move |
| Tab | Next channel, then the orders panel, then the instrument |
| `<` `>` | Octave down, up |
| `[` `]` | Previous, next instrument |
| F1 / F2 / F4 | Play from the start / play from the cursor's row / stop |
| F5–F8 | Mute or unmute channels 1–4 |
| Esc | The command line |

In the orders panel:
- hex digits choose the pattern, and a new number makes an empty pattern;
- `+` and `-` transpose the entry;
- Enter repeats the entry after itself, and Delete removes it;
- `l` makes the entry the loop point.

In the instrument panel:
- Up and Down choose a field;
- Left and Right change it by 1, and `-` and `+` by 10;
- on a script instrument, `e` opens its `.snd` file in the Editor and `r`
  reloads it.

Every change is heard at once, even while the song plays.

### Commands

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

## 11.2 The song format (`.trk`)

A `.trk` file is plain text, so you can read it or write it by hand:

```text
acid-track 1
title Acid Groove
speed 6
sfx-donor 4
instrument 01 "Lead"  wave pulse  adsr 1 30 60 60  duty 40  pwm 2  vib 2 3  voice2 detune 5
instrument 02 "Fat Bass"  script "Home/sounds/bass.snd" fatbass
order 1  00 00 01 00 loop 0
order 2  02 02+5 02 02+7 loop 0
order 3  03 loop 0
order 4  04 loop 0

pattern 00 16
A-4 01 . .. ...
... .. . .. ...
```

Each channel has its own **order list**, the patterns it plays in turn
with an optional transpose. Each **pattern** belongs to one channel.

A **built-in instrument** has these fields:

| Field | Values |
|---|---|
| `wave` | `pulse`, `saw`, `tri` or `noise` |
| `adsr` | Attack, decay and release in ms (0–100000), and sustain in percent |
| `duty` | Pulse width, 1–99 |
| `pwm` | Duty change per tick, –50 to 50 |
| `vib` | Vibrato depth and speed, 0–15 each |
| `arp` | Up to 3 semitone offsets, –48 to 48, stepped one per tick |
| `filter` | `lp`, `bp` or `hp`, then the cutoff (0–255) and the resonance (0–15) |
| `voice2` | What the second voice does: `off`, `detune N` (–768 to 768, in 1/64 semitones), `octave`, `fifth` or `ring` |

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
| `song "PATH"`, `play [N]`, `stop song` | Load a song (when the script loads), play it from order N, stop it |
| `tempo N`, `mute CH`, `unmute CH`, `jump N` | Steer the song |

**Voices.** A script has two voices. Put `v1`, `v2` or `both` before a
command to choose; with none, it means `v1`. A leading `+` or `-` makes a
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
- It can have at most 64 variables, and can nest at most 64 deep.
- Arithmetic saturates rather than overflowing, and dividing by zero gives 0.
- A script can't crash the OS. A mistake is a compile error with its line
  and column, such as `3:7 unknown command 'wav'`.

The song commands in a script only steer a song that the same app started.
`play "song"` replaces whatever is playing, just as `acid_song_play` does.

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
| `acid_sound_play(prog[, name[, note]])`, `acid_sound_stop(id)`, `acid_sound_free(prog)` | A sound id, or `nil` |
| `acid_song_load(path)`, `acid_song_parse(text)` | A song and a table of warnings, or `nil, "LINE: message"` |
| `acid_song_update(song, text)` | The warnings, or `nil, err`. A playing copy keeps its place. |
| `acid_song_play(song[, order[, row]])`, `acid_song_stop()`, `acid_song_mute(ch, on)` | Nothing |
| `acid_song_position()` | `order, row, tick`, or nothing when no song plays |
| `acid_song_preview(song, ch, note, inst)`, `acid_song_free(song)` | Nothing |

Each app can hold up to 16 programs and 4 songs. Free the ones you're done
with. Paths you pass to these calls are full paths, like `acid_fs_read`'s.
Only one song plays at a time, and stopping or muting it only works for
the app that started it.

## 11.5 How songs and sounds share the voices

A playing song uses all eight voices: channel 1 uses voices 0 and 1, and
channel 4 uses voices 6 and 7.

A sound effect takes a free voice if there is one. During a song, it
borrows the **donor** channel's second voice, set with `sfx-donor` and
channel 4 by default. The song gets the voice back when the sound ends.
Notes you start yourself with `acid_play_note` still work as before, and
the last thing to write to a voice wins.

While a song plays, the whole mix is turned down about 12 dB, so that
eight voices together don't clip. Your own notes are quieter then too.

---

[← WASM carts](10-wasm-carts.md) · [Contents](README.md)
````

- [ ] **Step 3: Update the other chapters and the READMEs**

In `docs/manual-v3/README.md`, add a row to the contents table after chapter 10:

```markdown
| **[11. Music](11-music.md)** | Acid Tracker, the `.trk` song format, the `.snd` audio language, and playing sounds and songs from your app. |
```

In `docs/manual-v3/10-wasm-carts.md`, change the navigation lines at the top and the bottom to end with ` · [Next: Music →](11-music.md)`.

In `docs/manual-v3/01-getting-started.md`, add this paragraph after the Sprite Paint paragraph:

```markdown
**Acid Tracker** writes music: four channels of two voices, saved as `.trk`
files. Open `App/tracker.app.toml` or any `.trk` file in the File Manager,
such as `Home/music/acid_groove.trk`, and press F1 to play it. You can also
play a song or a `.snd` sound from the Terminal with `play Home/music/acid_groove.trk`.
See [chapter 11](11-music.md).
```

In `docs/manual-v3/05-sound.md`, add this paragraph straight after the paragraph that ends "including the games' sound effects, the Piano app and the Terminal's easter eggs.":

```markdown
This chapter is about playing the synthesiser note by note. For songs, and
for scripted sound effects that run with exact timing inside the kernel,
see [chapter 11](11-music.md).
```

In `docs/manual-v3/09-api-reference.md`:
- In "Index by area", add this line after the **Audio** line:

```markdown
**Songs and sounds** — [`acid_sound_load`](#acid_sound_load) · [`acid_sound_load_file`](#acid_sound_load_file) · [`acid_sound_play`](#acid_sound_play) · [`acid_sound_stop`](#acid_sound_stop) · [`acid_sound_free`](#acid_sound_free) · [`acid_song_load`](#acid_song_load) · [`acid_song_parse`](#acid_song_parse) · [`acid_song_update`](#acid_song_update) · [`acid_song_free`](#acid_song_free) · [`acid_song_play`](#acid_song_play) · [`acid_song_stop`](#acid_song_stop) · [`acid_song_position`](#acid_song_position) · [`acid_song_mute`](#acid_song_mute) · [`acid_song_preview`](#acid_song_preview)
```

- Under "Functions, alphabetically", add these 14 entries, each at its alphabetical place. They all go after `acid_skipped_frames` and before `acid_spawn_app`.

````markdown
### `acid_song_free`

```lua snippet
acid_song_free(song)
```

Forgets a song handle. A song that's still playing keeps playing. See [§11.4](11-music.md).

### `acid_song_load`

```lua snippet
local song, warnings = acid_song_load(path)
```

Reads and parses a `.trk` file, and loads its script instruments. Returns a handle and a table of
warnings, one for each script instrument that didn't load, or `nil, "LINE: message"`. Up to 4 songs
per app.

### `acid_song_mute`

```lua snippet
acid_song_mute(ch, on)
```

Mutes or unmutes channel `ch` (1–4) of the playing song, if your app started it.

### `acid_song_parse`

```lua snippet
local song, warnings = acid_song_parse(text)
```

As `acid_song_load`, but for `.trk` text you already have.

### `acid_song_play`

```lua snippet
acid_song_play(song, order, row)
```

Plays a song from order position `order` and row `row` (both default to 0), replacing any song
already playing.

### `acid_song_position`

```lua snippet
local order, row, tick = acid_song_position()
```

Where the playing song is, going by channel 1. Returns nothing when no song plays.

### `acid_song_preview`

```lua snippet
acid_song_preview(song, ch, note, inst)
```

Sounds one note (1–88) on channel `ch` with instrument `inst`. A `note` of 0 is a note-off. Acid
Tracker uses it for the keys you play while editing.

### `acid_song_stop`

```lua snippet
acid_song_stop()
```

Stops the playing song, if your app started it.

### `acid_song_update`

```lua snippet
local warnings, err = acid_song_update(song, text)
```

Parses new text into an existing handle. If that song is playing, it carries on from the same place
with the new notes.

### `acid_sound_free`

```lua snippet
acid_sound_free(prog)
```

Forgets a compiled program. Up to 16 programs per app.

### `acid_sound_load`

```lua snippet
local prog, err = acid_sound_load(src)
```

Compiles `.snd` source text. Any `song "PATH"` lines load their songs now. Returns a program, or
`nil, "LINE:COL message"`. See [§11.3](11-music.md).

### `acid_sound_load_file`

```lua snippet
local prog, err = acid_sound_load_file(path)
```

As `acid_sound_load`, reading the source from a file.

### `acid_sound_play`

```lua snippet
local id = acid_sound_play(prog, name, note)
```

Starts the `sound` block called `name` (by default, the first), with `note` as the script's `note`
(default 40, C-4). It takes a free voice, or the song's donor voice. Returns an id, or `nil` when no
voice is free.

### `acid_sound_stop`

```lua snippet
acid_sound_stop(id)
```

Stops one of your sounds and releases its voices.
````

In the repository root `README.md`:
- Under "Out of the box you get", add this after the Sprite Paint line:

```markdown
- Acid Tracker, a four-channel tracker for `.trk` songs, with its own audio
  scripting language (`.snd`);
```

- Change "Games, demos and Sprite Paint open from the **File Manager**" to "Games, demos, Sprite Paint and Acid Tracker open from the **File Manager**".
- In the `--app` names table, add `tracker` (Acid Tracker) to the Toys row.

- [ ] **Step 4: Bring spec §7 in line**

In `docs/superpowers/specs/2026-10-06-acid-tracker-design.md`:
- In §7.1, change the manifest note to: `name "Acid Tracker", 480×320, resizable, minimum 420×240, menu = false (like Sprite Paint)`. Also replace the module list with `song.lua`, `edit.lua`, `layout.lua` and `cmd.lua`.
- Replace the §7.3 key table with the table in manual §11.1, keys and orders and instrument panels included.
- Add the commands table from manual §11.1.
- In §7.4, note that the samples are `Home/music/acid_groove.trk` and `Home/sounds/{bass,wobble,zap,sync}.snd`, and that File Manager opens `.snd` in the Editor.

- [ ] **Step 5: Run the manual tests and the whole workspace**

Run: `cargo test --manifest-path v3/Cargo.toml -p acid-os --test manual`
Expected: all pass. That means:
- `every_snippet_parses`
- `every_acid_name_in_the_lua_chapters_exists`, which shows the 14 new names are real globals
- `every_manual_link_resolves`
- `the_manual_is_complete`

If a link fails, fix the text. Don't relax the test.

Run: `cargo test --manifest-path v3/Cargo.toml`
Expected: everything passes.

- [ ] **Step 6: Commit**

```bash
git add docs/manual-v3 README.md v3/crates/acid-os/tests/manual.rs docs/superpowers/specs/2026-10-06-acid-tracker-design.md
git commit -m "Manual: chapter 11, music (Acid Tracker, .trk, the .snd language, song and sound calls)

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

## Self-review against the spec

| Spec | Covered by |
|---|---|
| §7.1 files and manifest | Tasks 1, 3, 4 and 5 |
| §7.1 model in Lua, Rust reader of record | Tasks 1 and 5 (`load_text` parses in Rust before Lua) |
| §7.2 layout: status, grid, orders, instrument, follow, play bar, muted dimming | Tasks 4 and 5 |
| §7.3 keys | Tasks 3 and 5, as amended and recorded in Task 8 |
| §7.4 File Manager, Terminal `play`, samples, manual | Tasks 2, 6 and 8 |
| §8 Lua tests | Tasks 1 and 3 to 6 |
| §8 window goldens | Task 7 |
| Shipped files load cleanly | Task 2 |

**Checks I made:**
- **Names line up between tasks.** `TrkEdit:grid_key` returns `changed, note`, which `TrackerApp:focus_key` consumes in that order. `TrkLayout.SLOT_CHARS` and `SLOT_W` are used by `draw_grid`. `TrkCmd.USAGE.arp` is the message text the app test expects.
- **The assertion counts:**

  | Suite | Count |
  |---|---|
  | `trk_song` | 14 + 2 per file (16 in Task 1, 18 after Task 2) |
  | `trk_edit` | 36 |
  | `trk_layout_and_commands` | 15 |
  | `tracker_app` | 38 |
  | `file_manager` | 36 |
  | `terminal` | 33 |
