# Acid Sound Engine Implementation Plan (plan 1 of 2)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build `acid-sound`, a new `no_std` crate holding the `.snd` audio language (compiler and VM), the `.trk` song format and player, and an engine the kernel ticks at 50 Hz. Then wire it into the kernel, `acid-api` and Lua, and pass function keys through to apps.

**Architecture:** `acid-sound` sits between `acid-kernel` and `acid-synth`.
- **Compiling.** The compiler turns `.snd` text into bytecode on the calling app's thread.
- **Running.** `Instance`s run that bytecode for at most 256 instructions per tick.
- **Songs.** A `Player` steps through a parsed `.trk` song, one row every `speed` ticks.
- **The engine.** The `Engine` owns one song, a preview player and up to 16 sound effects. It ticks them every 441 samples from inside `Kernel::render_audio`.
- **App calls.** `acid-api` exposes `acid_sound_*` and `acid_song_*` to Lua.

**Plan 2.** The Acid Tracker app is a separate plan, written once this one has landed, because it builds on this engine's real behaviour. That plan also covers File Manager and Terminal integration, the sample songs and the manual.

**Tech Stack:** Rust 2024 (`no_std` + `alloc`), the existing `acid-synth`, `mlua` for bindings, and the Lua 5.4 app library.

**Spec:** `docs/superpowers/specs/2026-10-06-acid-tracker-design.md`

## Global Constraints

- **Running commands.** Run every command from the repository top (`/home/norfolkh/acid-os-v3`), using `--manifest-path v3/Cargo.toml`.
- **`no_std`.** `acid-sound` begins with `#![cfg_attr(not(test), no_std)]` and `extern crate alloc;`. It depends only on `acid-synth`.
- **Integer maths only** in `acid-sound`, with no floats anywhere.
- **Fixed tick.** One engine tick is `TICK_SAMPLES = 441` samples (22050 Hz / 50).
- **Pitch.** It uses the synth's `ona` scale, 1..=88 with A0 = 1 and C-4 = 40. Fine pitch is 1/64 of a semitone.
- **Channels.** There are 4. Channel *n* (0-based) uses voices `2n` and `2n+1`.
- **No allocation in the tick path.** Vectors used per tick are created with their capacity up front and never grow past it.
- **Golden recordings.** Never regenerate one to make a test pass. `ACID_SOUND_BLESS=1` is only for a change in sound that is intended and has been listened to.
- **Commits.** Messages follow the repo style `Area: summary`, for example `acid-sound: lexer`, and end with:
  ```
  Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
  ```
- **Comments** follow house style: a `//!` header on every file, and short `///` docs that say *why* rather than restating the code.

## File map

| File | Responsibility |
|---|---|
| `v3/crates/acid-sound/Cargo.toml` | New crate manifest |
| `v3/crates/acid-sound/src/lib.rs` | Module list and `TICK_SAMPLES` |
| `v3/crates/acid-sound/src/pitch.rs` | Note names (`C-4`) and fine-pitch phase increments |
| `v3/crates/acid-sound/src/lexer.rs` | `.snd` tokens and `CompileError` |
| `v3/crates/acid-sound/src/program.rs` | Bytecode types: `Op`, `Cmd`, `Block`, `Program` |
| `v3/crates/acid-sound/src/compile.rs` | `.snd` to `Program` (one pass, no AST) |
| `v3/crates/acid-sound/src/vm.rs` | `Instance`: runs a block, ticks, budget, synth writes |
| `v3/crates/acid-sound/src/song.rs` | `.trk` model, `parse`, `write` |
| `v3/crates/acid-sound/src/player.rs` | `LoadedSong`, `Player` |
| `v3/crates/acid-sound/src/engine.rs` | `Engine`: render and tick, sounds, voice lending, song commands |
| `v3/crates/acid-sound/src/load.rs` | `load_song` and `load_program` (resolve the file references) |
| `v3/crates/acid-sound/tests/golden.rs` and `tests/golden/*` | Golden audio |
| `v3/crates/acid-sound/examples/to_wav.rs` | Turns a `.u8` recording into a `.wav` to listen to |
| `v3/crates/acid-kernel/src/audio.rs` | Engine inside `AudioState`, the new `audio_sound_*` and `audio_song_*` |
| `v3/crates/acid-api/src/lib.rs` | Trait methods, `SoundStore`, `KernelApi` impl |
| `v3/crates/acid-lua/src/lib.rs` | Lua bindings |
| `v3/crates/acid-platform/src/keys.rs`, `v3/crates/acid-hosted/src/keymap.rs`, `v3/apps/lib/acid_keys.lua` | F1 to F12 |

---

### Task 1: Crate scaffold, pitch helpers and spec touch-ups

**Files:**
- Modify: `v3/Cargo.toml`
- Create: `v3/crates/acid-sound/Cargo.toml`
- Create: `v3/crates/acid-sound/src/lib.rs`
- Create: `v3/crates/acid-sound/src/pitch.rs`
- Modify: `docs/superpowers/specs/2026-10-06-acid-tracker-design.md`

**Interfaces:**
- Produces:
  - `acid_sound::TICK_SAMPLES: u32`
  - Constants `pitch::{ONA_MIN, ONA_MAX, FINE_STEPS, FINE_MAX}`
  - `pitch::parse_note(&str) -> Option<i32>`
  - `pitch::note_name(i32) -> String`
  - `pitch::fine_pos(ona: i32) -> i32`
  - `pitch::increment_at(pos: i32) -> u32`
  - `pitch::phase_increment(ona: i32, fine: i32) -> u32`

- [ ] **Step 1: Bring the spec in line with this plan**

Edit `docs/superpowers/specs/2026-10-06-acid-tracker-design.md`:
- In §2.3, change `(0..87)` to `(1..88)`.
- In §3's row table, change the Note 2 row's form to `` `...` or a note (no note-off) ``.
- In §3's built-in field list, write the filter as `filter lp|bp|hp CUTOFF RES`.
- Replace §4's first bullet with:

```markdown
- Each tick:
  1. On the first tick of a row, every channel reads its row and starts new
     notes.
  2. Every channel advances its effects.
  3. Script instances run.
  4. The row tick counter moves on. After `speed` ticks, every channel steps
     to its next row.
- Pattern commands `1`–`4` (slides, glide, vibrato) act on built-in
  instruments. A script instrument controls its own pitch.
```

Replace the §5 table with:

```markdown
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
```

Add one line at the end of §5:

```markdown
Paths given to Lua calls are full paths, as with `acid_fs_read`. Paths inside files (`script "PATH"` in a `.trk`, `song "PATH"` in a `.snd`) are relative to fsroot.
```

- [ ] **Step 2: Register the crate**

Add to `[workspace.dependencies]` in `v3/Cargo.toml`, right after the `acid-synth` line:

```toml
acid-sound = { path = "crates/acid-sound" }
```

Create `v3/crates/acid-sound/Cargo.toml`:

```toml
[package]
name = "acid-sound"
version.workspace = true
edition.workspace = true

[dependencies]
acid-synth = { workspace = true }
```

Create `v3/crates/acid-sound/src/lib.rs`:

```rust
//! The Acid OS sound engine: the .snd audio language (compiler and VM),
//! the .trk song format and player, and the engine the kernel ticks at
//! 50 Hz inside the audio render. Integer-only and no_std.
#![cfg_attr(not(test), no_std)]

extern crate alloc;

pub mod pitch;

/// Samples per engine tick: 22050 Hz / 50 Hz, PAL timing as in GoatTracker.
pub const TICK_SAMPLES: u32 = acid_synth::SAMPLE_RATE / 50;
```

- [ ] **Step 3: Write the failing pitch tests**

Create `v3/crates/acid-sound/src/pitch.rs` containing just the tests for now:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use acid_synth::ONA_PHASE_INCREMENT;

    #[test]
    fn note_names_map_to_piano_keys() {
        assert_eq!(parse_note("A-0"), Some(1));
        assert_eq!(parse_note("C-4"), Some(40));
        assert_eq!(parse_note("C#4"), Some(41));
        assert_eq!(parse_note("C-8"), Some(88));
        assert_eq!(parse_note("G#0"), None, "below A0");
        assert_eq!(parse_note("C#8"), None, "above C8");
        assert_eq!(parse_note("H-4"), None);
        assert_eq!(parse_note("C-"), None);
    }

    #[test]
    fn names_round_trip() {
        for ona in ONA_MIN..=ONA_MAX {
            assert_eq!(parse_note(&note_name(ona)), Some(ona), "{ona}");
        }
    }

    #[test]
    fn whole_semitones_match_the_synth_table() {
        for ona in ONA_MIN..=ONA_MAX {
            assert_eq!(phase_increment(ona, 0), ONA_PHASE_INCREMENT[(ona - 1) as usize]);
        }
    }

    #[test]
    fn fine_steps_sit_between_semitones() {
        let (lo, mid, hi) = (phase_increment(40, 0), phase_increment(40, 32), phase_increment(41, 0));
        assert!(lo < mid && mid < hi);
        assert_eq!(phase_increment(40, 64), hi);
        assert_eq!(phase_increment(40, -64), phase_increment(39, 0));
    }

    #[test]
    fn out_of_range_clamps_to_the_keyboard() {
        assert_eq!(phase_increment(-5, 0), ONA_PHASE_INCREMENT[0]);
        assert_eq!(phase_increment(200, 0), ONA_PHASE_INCREMENT[87]);
        assert_eq!(increment_at(fine_pos(40) + 8), phase_increment(40, 8));
    }
}
```

- [ ] **Step 4: Run them to see them fail**

Run: `cargo test --manifest-path v3/Cargo.toml -p acid-sound`
Expected: compile errors, `cannot find function parse_note` and so on.

- [ ] **Step 5: Implement the pitch helpers**

Put this above the tests in `pitch.rs`:

```rust
//! Note names and pitch. Pitch is the synth's 88-key `ona` scale (A0 = 1,
//! C-4 = 40, C-8 = 88). Fine pitch is 1/64 of a semitone, linearly
//! interpolated between the synth's per-semitone phase increments.

use alloc::format;
use alloc::string::String;

use acid_synth::ONA_PHASE_INCREMENT;

pub const ONA_MIN: i32 = 1;
pub const ONA_MAX: i32 = 88;
/// Fine pitch steps per semitone.
pub const FINE_STEPS: i32 = 64;
/// The highest fine position (C-8).
pub const FINE_MAX: i32 = (ONA_MAX - 1) * FINE_STEPS;

const NAMES: [&str; 12] = ["C-", "C#", "D-", "D#", "E-", "F-", "F#", "G-", "G#", "A-", "A#", "B-"];

/// "C-4" -> 40. None for anything else, or a note off the keyboard.
pub fn parse_note(s: &str) -> Option<i32> {
    let b = s.as_bytes();
    if b.len() != 3 {
        return None;
    }
    let semi = NAMES.iter().position(|n| n.as_bytes() == &b[..2])? as i32;
    let oct = (b[2] as char).to_digit(10)? as i32;
    let ona = oct * 12 + semi - 8;
    (ONA_MIN..=ONA_MAX).contains(&ona).then_some(ona)
}

/// 40 -> "C-4". Out-of-range values are clamped onto the keyboard.
pub fn note_name(ona: i32) -> String {
    let n = ona.clamp(ONA_MIN, ONA_MAX) + 8;
    format!("{}{}", NAMES[(n % 12) as usize], n / 12)
}

/// A whole note's fine position; slides and vibrato add to it.
pub fn fine_pos(ona: i32) -> i32 {
    ona.saturating_sub(1).saturating_mul(FINE_STEPS)
}

/// The phase increment at a fine position, clamped to the keyboard.
pub fn increment_at(pos: i32) -> u32 {
    let p = pos.clamp(0, FINE_MAX);
    let i = (p / FINE_STEPS) as usize;
    let f = (p % FINE_STEPS) as u64;
    let lo = ONA_PHASE_INCREMENT[i] as u64;
    if f == 0 {
        return lo as u32;
    }
    let hi = ONA_PHASE_INCREMENT[i + 1] as u64;
    (lo + (hi - lo) * f / FINE_STEPS as u64) as u32
}

/// The phase increment for `ona` plus `fine` 1/64-semitone steps.
pub fn phase_increment(ona: i32, fine: i32) -> u32 {
    increment_at(fine_pos(ona).saturating_add(fine))
}
```

- [ ] **Step 6: Run the tests to see them pass**

Run: `cargo test --manifest-path v3/Cargo.toml -p acid-sound`
Expected: 5 passed.

- [ ] **Step 7: Commit**

```bash
git add v3/Cargo.toml v3/Cargo.lock v3/crates/acid-sound docs/superpowers/specs/2026-10-06-acid-tracker-design.md
git commit -m "acid-sound: new crate with note names and fine pitch; spec touch-ups

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 2: Lexer

**Files:**
- Create: `v3/crates/acid-sound/src/lexer.rs`
- Modify: `v3/crates/acid-sound/src/lib.rs` (add `pub mod lexer;` and `pub use lexer::CompileError;`)

**Interfaces:**
- Produces:
  - `CompileError { line: u32, col: u32, message: String }`, with `CompileError::new(line, col, msg)` and a `Display` that prints `"LINE:COL message"`
  - `Tok` with the variants `Ident(String)`, `Num(i32)`, `Str(String)`, `Sym(&'static str)`, `Newline`, `Eof`
  - `Token { tok, line, col }`
  - `lex(&str) -> Result<Vec<Token>, CompileError>`
- Behaviour:
  - `;` becomes `Tok::Newline`.
  - `#` starts a comment that runs to the end of the line.
  - Every line ends with a `Newline` token, and the token stream ends with `Eof`.

- [ ] **Step 1: Write the failing tests**

Create `v3/crates/acid-sound/src/lexer.rs` with:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;

    fn toks(src: &str) -> Vec<Tok> {
        lex(src).unwrap().into_iter().map(|t| t.tok).collect()
    }

    fn id(s: &str) -> Tok {
        Tok::Ident(s.into())
    }

    #[test]
    fn words_numbers_and_symbols() {
        assert_eq!(toks("duty +8"), vec![id("duty"), Tok::Sym("+"), Tok::Num(8), Tok::Newline, Tok::Eof]);
    }

    #[test]
    fn two_character_symbols_win() {
        assert_eq!(toks("a<=b")[1], Tok::Sym("<="));
        assert_eq!(toks("a == b")[1], Tok::Sym("=="));
        assert_eq!(toks("a = b")[1], Tok::Sym("="));
    }

    #[test]
    fn semicolons_split_and_hashes_comment() {
        assert_eq!(
            toks("gate on; wait 1 # done"),
            vec![id("gate"), id("on"), Tok::Newline, id("wait"), Tok::Num(1), Tok::Newline, Tok::Eof]
        );
    }

    #[test]
    fn strings_keep_their_text() {
        assert_eq!(toks("song \"music/a.trk\"")[1], Tok::Str("music/a.trk".into()));
    }

    #[test]
    fn positions_are_one_based() {
        let t = lex("wave saw\n  gate on").unwrap();
        assert_eq!((t[3].line, t[3].col), (2, 3));
    }

    #[test]
    fn errors_carry_line_and_column() {
        assert_eq!(lex("wait 1\n  @").unwrap_err().to_string(), "2:3 unexpected '@'");
        assert_eq!(lex("wait 99999999999").unwrap_err().to_string(), "1:6 number too big");
        assert_eq!(lex("song \"x").unwrap_err().to_string(), "1:6 string has no closing '\"'");
    }
}
```

Add `pub mod lexer;` and `pub use lexer::CompileError;` to `lib.rs`.

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test --manifest-path v3/Cargo.toml -p acid-sound lexer`
Expected: compile errors, `cannot find function lex` and `cannot find type Tok`.

- [ ] **Step 3: Implement the lexer**

Put this above the tests:

```rust
//! .snd tokens. Lines matter (a statement ends at a newline or `;`), so
//! the lexer emits a Newline token at the end of every line.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompileError {
    pub line: u32,
    pub col: u32,
    pub message: String,
}

impl CompileError {
    pub fn new(line: u32, col: u32, message: impl Into<String>) -> Self {
        Self { line, col, message: message.into() }
    }
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{} {}", self.line, self.col, self.message)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Tok {
    Ident(String),
    Num(i32),
    Str(String),
    Sym(&'static str),
    Newline,
    Eof,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Token {
    pub tok: Tok,
    pub line: u32,
    pub col: u32,
}

/// Longest first, so "<=" wins over "<".
const SYMS: [&str; 15] = ["==", "!=", "<=", ">=", "<", ">", "=", "+", "-", "*", "/", "%", "(", ")", ";"];

pub fn lex(src: &str) -> Result<Vec<Token>, CompileError> {
    let mut out = Vec::new();
    for (li, text) in src.lines().enumerate() {
        let line = li as u32 + 1;
        let b = text.as_bytes();
        let mut i = 0;
        while i < b.len() {
            let c = b[i];
            let col = i as u32 + 1;
            if c == b' ' || c == b'\t' {
                i += 1;
                continue;
            }
            if c == b'#' {
                break;
            }
            if c.is_ascii_digit() {
                let start = i;
                while i < b.len() && b[i].is_ascii_digit() {
                    i += 1;
                }
                let n = text[start..i].parse().map_err(|_| CompileError::new(line, col, "number too big"))?;
                out.push(Token { tok: Tok::Num(n), line, col });
                continue;
            }
            if c.is_ascii_alphabetic() || c == b'_' {
                let start = i;
                while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                    i += 1;
                }
                out.push(Token { tok: Tok::Ident(text[start..i].to_string()), line, col });
                continue;
            }
            if c == b'"' {
                let start = i + 1;
                let end = text[start..]
                    .find('"')
                    .ok_or_else(|| CompileError::new(line, col, "string has no closing '\"'"))?;
                out.push(Token { tok: Tok::Str(text[start..start + end].to_string()), line, col });
                i = start + end + 1;
                continue;
            }
            let Some(&s) = SYMS.iter().find(|s| text[i..].starts_with(**s)) else {
                let ch = text[i..].chars().next().unwrap_or('?');
                return Err(CompileError::new(line, col, format!("unexpected '{ch}'")));
            };
            out.push(Token { tok: if s == ";" { Tok::Newline } else { Tok::Sym(s) }, line, col });
            i += s.len();
        }
        out.push(Token { tok: Tok::Newline, line, col: b.len() as u32 + 1 });
    }
    let line = out.last().map_or(1, |t| t.line);
    out.push(Token { tok: Tok::Eof, line, col: 1 });
    Ok(out)
}
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test --manifest-path v3/Cargo.toml -p acid-sound lexer`
Expected: 6 passed.

- [ ] **Step 5: Commit**

```bash
git add v3/crates/acid-sound
git commit -m "acid-sound: .snd lexer with line:col errors

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 3: Bytecode and the compiler core (blocks, variables, control flow, expressions)

**Files:**
- Create: `v3/crates/acid-sound/src/program.rs`
- Create: `v3/crates/acid-sound/src/compile.rs`
- Modify: `v3/crates/acid-sound/src/lib.rs` (add `pub mod program; pub mod compile; pub use compile::compile;`)

**Interfaces:**
- Consumes: `lexer::{lex, CompileError, Tok, Token}`
- Produces:
  - `program::{MAX_VARS, Target, Builtin, BinOp, Cmd, Op, BlockKind, Block, SongRef, Program}`, exactly as written in Step 1
  - `compile::compile(&str) -> Result<Program, CompileError>`
  - `Program::block(&self, name) -> Option<usize>`

- [ ] **Step 1: Write `program.rs`**

All of the bytecode types are data, so they're written here in full. Task 4 compiles the `Cmd` variants.

```rust
//! .snd bytecode. A stack machine over i32: expressions push, commands pop
//! their arguments. Jump targets are indexes into a block's `code`.

use alloc::string::String;
use alloc::vec::Vec;

/// Variables per block, including the hidden counters `repeat` uses.
pub const MAX_VARS: usize = 64;

/// Which of an instance's two voices a command touches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    V1,
    V2,
    Both,
}

/// Read-only values a script can name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Builtin {
    Note,
    Note2,
    Tick,
    Row,
    Beat,
    Order,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}

/// Sound and song commands. The comment says what each pops.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cmd {
    /// Synth numbering: 0 pulse, 1 saw, 2 tri, 3 noise.
    Wave(i32),
    /// Pops 1. `rel` adds to the current value.
    Duty { rel: bool },
    /// Pops attack, decay, sustain, release.
    Adsr,
    Gate(bool),
    /// Pops 1 (semitones on the ona scale).
    Pitch { rel: bool },
    /// Pops 1 (1/64 semitones).
    Fine { rel: bool },
    Ring(bool),
    /// Pops n semitone offsets (1..=3).
    Arp(u8),
    ArpOff,
    /// Pops 1 (ms per arp step).
    ArpRate,
    Route(bool),
    /// Mode mask 1/2/4; pops cutoff, resonance.
    Filter(i32),
    /// Index into Program::song_paths.
    SongSelect(u8),
    /// Pops the order position.
    SongPlay,
    SongStop,
    /// Pops ticks per row.
    Tempo,
    /// Pops a 1-based channel; true mutes.
    Mute(bool),
    /// Pops the order position.
    Jump,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Push(i32),
    Load(u8),
    Store(u8),
    Get(Builtin),
    Bin(BinOp),
    Neg,
    Not,
    /// Pops n, pushes 0..n-1 (0 when n <= 0).
    Rand,
    Jump(u16),
    JumpIfZero(u16),
    /// Pops ticks; under 1 doesn't wait.
    Wait,
    WaitRow,
    WaitBeat,
    /// Gate off this instance's voices and finish.
    Stop,
    /// The end of a part: a sound finishes, an instrument's note holds.
    End,
    Cmd(Cmd, Target),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockKind {
    Sound,
    Instrument,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block {
    pub name: String,
    pub kind: BlockKind,
    pub code: Vec<Op>,
    /// Where `on release` starts in `code`, for an instrument that has one.
    pub release_pc: Option<u16>,
    pub vars: u8,
    /// Any `v2` or `both` command: a sound asks for two voices.
    pub uses_v2: bool,
}

/// A `song "PATH"` line, kept so a load failure can point at it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SongRef {
    pub path: String,
    pub line: u32,
    pub col: u32,
}

#[derive(Clone, Debug, Default)]
pub struct Program {
    pub blocks: Vec<Block>,
    /// Distinct `song` paths in first-use order; Cmd::SongSelect indexes this.
    pub song_paths: Vec<SongRef>,
}

impl Program {
    pub fn block(&self, name: &str) -> Option<usize> {
        self.blocks.iter().position(|b| b.name == name)
    }
}
```

- [ ] **Step 2: Write the failing compiler tests**

Create `v3/crates/acid-sound/src/compile.rs` containing these tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;

    fn code(src: &str) -> Vec<Op> {
        compile(src).unwrap().blocks[0].code.clone()
    }

    fn err(src: &str) -> String {
        compile(src).unwrap_err().to_string()
    }

    #[test]
    fn a_file_without_blocks_is_sound_main() {
        let p = compile("wait 2").unwrap();
        assert_eq!(p.blocks.len(), 1);
        assert_eq!(p.blocks[0].name, "main");
        assert_eq!(p.blocks[0].kind, BlockKind::Sound);
        assert_eq!(p.blocks[0].code, vec![Op::Push(2), Op::Wait, Op::End]);
    }

    #[test]
    fn named_blocks_and_on_release() {
        let p = compile("sound a\nwait 1\nend\n\ninstrument b\nwait 1\non release\nwait 2\nend\n").unwrap();
        assert_eq!(p.block("a"), Some(0));
        assert_eq!(p.block("b"), Some(1));
        let b = &p.blocks[1];
        assert_eq!(b.kind, BlockKind::Instrument);
        assert_eq!(b.code, vec![Op::Push(1), Op::Wait, Op::End, Op::Push(2), Op::Wait, Op::End]);
        assert_eq!(b.release_pc, Some(3));
    }

    #[test]
    fn precedence_and_variables() {
        assert_eq!(
            code("let x = 1 + 2 * 3\nx = x - 1"),
            vec![
                Op::Push(1), Op::Push(2), Op::Push(3), Op::Bin(BinOp::Mul), Op::Bin(BinOp::Add), Op::Store(0),
                Op::Load(0), Op::Push(1), Op::Bin(BinOp::Sub), Op::Store(0), Op::End,
            ]
        );
    }

    #[test]
    fn unary_logic_and_builtins() {
        assert_eq!(
            code("let a = not -note and rand 6 >= tick"),
            vec![
                Op::Get(Builtin::Note), Op::Neg, Op::Not,
                Op::Push(6), Op::Rand, Op::Get(Builtin::Tick), Op::Bin(BinOp::Ge),
                Op::Bin(BinOp::And), Op::Store(0), Op::End,
            ]
        );
    }

    #[test]
    fn if_else_jumps() {
        assert_eq!(
            code("if 1\nwait 1\nelse\nwait 2\nend"),
            vec![
                Op::Push(1), Op::JumpIfZero(5), Op::Push(1), Op::Wait, Op::Jump(7),
                Op::Push(2), Op::Wait, Op::End,
            ]
        );
    }

    #[test]
    fn repeat_counts_down_a_hidden_variable() {
        assert_eq!(
            code("repeat 3\nwait 1\nend"),
            vec![
                Op::Push(3), Op::Store(0),
                Op::Load(0), Op::Push(0), Op::Bin(BinOp::Gt), Op::JumpIfZero(13),
                Op::Push(1), Op::Wait,
                Op::Load(0), Op::Push(1), Op::Bin(BinOp::Sub), Op::Store(0), Op::Jump(2),
                Op::End,
            ]
        );
    }

    #[test]
    fn loop_and_wait_forms() {
        assert_eq!(
            code("loop\nwait row\nwait beat\nend"),
            vec![Op::WaitRow, Op::WaitBeat, Op::Jump(0), Op::End]
        );
    }

    #[test]
    fn error_messages() {
        assert_eq!(err("x = 1"), "1:1 unknown variable 'x' (use 'let')");
        assert_eq!(err("wait y"), "1:6 unknown name 'y'");
        assert_eq!(err("wav saw"), "1:1 unknown command 'wav'");
        assert_eq!(err("wait 1 2"), "1:8 unexpected '2'");
        assert_eq!(err("let let = 1"), "1:5 'let' is a reserved word");
        assert_eq!(err("wait (1"), "1:8 expected ')'");
        assert_eq!(err("sound a\nrepeat 2\nwait 1"), "2:1 'repeat' has no 'end'");
        assert_eq!(err("sound a\nwait 1"), "1:1 'sound' has no 'end'");
        assert_eq!(err("sound a\nend\nwait 1"), "3:1 statement outside a block");
        assert_eq!(err("sound a\nend\nsound a\nend"), "3:7 'a' is already defined");
        assert_eq!(err("sound a\non release\nend"), "2:1 only an instrument has 'on release'");
        assert_eq!(err("end"), "1:1 unexpected 'end'");
        assert_eq!(err("\n\n"), "2:1 nothing to play");
    }

    #[test]
    fn at_most_64_variables() {
        let src: String = (0..65).map(|i| alloc::format!("let v{i} = 0\n")).collect();
        assert_eq!(err(&src), "65:5 too many variables (64 max)");
    }
}
```

Add the modules to `lib.rs`:

```rust
pub mod program;
pub mod compile;

pub use compile::compile;
```

- [ ] **Step 3: Run them to see them fail**

Run: `cargo test --manifest-path v3/Cargo.toml -p acid-sound compile`
Expected: compile error, `cannot find function compile`.

- [ ] **Step 4: Implement the compiler core**

Put this above the tests in `compile.rs`:

```rust
//! The .snd compiler: tokens straight to bytecode in one pass, no AST.
//! Command arguments that sit side by side (`adsr 2 120 60 80`) are unary
//! expressions, so `-1` is an argument rather than a subtraction; anything
//! bigger goes in parentheses.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::lexer::{lex, CompileError, Tok, Token};
use crate::program::*;

/// Words that can't be variable names.
const RESERVED: &[&str] = &[
    "let", "if", "else", "end", "repeat", "loop", "wait", "stop", "sound", "instrument", "on", "off",
    "release", "and", "or", "not", "rand", "v1", "v2", "both", "song", "play", "tempo", "mute",
    "unmute", "jump", "filter", "res", "row", "beat", "note", "note2", "tick", "order", "wave",
    "duty", "adsr", "gate", "pitch", "fine", "ring", "arp", "arprate", "route",
];

pub fn compile(src: &str) -> Result<Program, CompileError> {
    let toks = lex(src)?;
    let mut p = Parser { toks, pos: 0, prog: Program::default() };
    p.file()?;
    Ok(p.prog)
}

fn builtin(w: &str) -> Option<Builtin> {
    Some(match w {
        "note" => Builtin::Note,
        "note2" => Builtin::Note2,
        "tick" => Builtin::Tick,
        "row" => Builtin::Row,
        "beat" => Builtin::Beat,
        "order" => Builtin::Order,
        _ => return None,
    })
}

fn describe(t: &Tok) -> String {
    match t {
        Tok::Ident(w) => format!("'{w}'"),
        Tok::Num(n) => format!("'{n}'"),
        Tok::Str(_) => String::from("a string"),
        Tok::Sym(s) => format!("'{s}'"),
        Tok::Newline => String::from("end of line"),
        Tok::Eof => String::from("end of file"),
    }
}

/// One block being compiled.
struct Cx {
    code: Vec<Op>,
    vars: Vec<String>,
    hidden: u32,
    uses_v2: bool,
}

impl Cx {
    fn new() -> Self {
        Self { code: Vec::new(), vars: Vec::new(), hidden: 0, uses_v2: false }
    }

    fn emit(&mut self, op: Op) -> usize {
        self.code.push(op);
        self.code.len() - 1
    }

    /// The next op's index. Clamped; `finish` rejects a block this long.
    fn here(&self) -> u16 {
        self.code.len().min(u16::MAX as usize) as u16
    }

    fn patch(&mut self, at: usize, to: u16) {
        self.code[at] = match self.code[at] {
            Op::Jump(_) => Op::Jump(to),
            Op::JumpIfZero(_) => Op::JumpIfZero(to),
            op => op,
        };
    }

    fn var(&self, name: &str) -> Option<u8> {
        self.vars.iter().position(|v| v == name).map(|i| i as u8)
    }

    fn new_var(&mut self, name: String, line: u32, col: u32) -> Result<u8, CompileError> {
        if let Some(i) = self.var(&name) {
            return Ok(i);
        }
        if self.vars.len() >= MAX_VARS {
            return Err(CompileError::new(line, col, format!("too many variables ({MAX_VARS} max)")));
        }
        self.vars.push(name);
        Ok((self.vars.len() - 1) as u8)
    }

    /// A counter for `repeat`; the leading space keeps it out of reach of names.
    fn hidden_var(&mut self, line: u32, col: u32) -> Result<u8, CompileError> {
        self.hidden += 1;
        self.new_var(format!(" repeat{}", self.hidden), line, col)
    }
}

struct Parser {
    toks: Vec<Token>,
    pos: usize,
    prog: Program,
}

impl Parser {
    fn peek(&self) -> &Tok {
        &self.toks[self.pos].tok
    }

    fn here(&self) -> (u32, u32) {
        let t = &self.toks[self.pos];
        (t.line, t.col)
    }

    fn err_here<T>(&self, msg: impl Into<String>) -> Result<T, CompileError> {
        let (l, c) = self.here();
        Err(CompileError::new(l, c, msg))
    }

    /// Takes the current token; Eof stays put.
    fn bump(&mut self) -> Tok {
        let t = self.toks[self.pos].tok.clone();
        if self.pos + 1 < self.toks.len() {
            self.pos += 1;
        }
        t
    }

    fn skip_newlines(&mut self) {
        while *self.peek() == Tok::Newline {
            self.bump();
        }
    }

    fn is_word(&self, w: &str) -> bool {
        matches!(self.peek(), Tok::Ident(s) if s == w)
    }

    fn eat_word(&mut self, w: &str) -> bool {
        let yes = self.is_word(w);
        if yes {
            self.bump();
        }
        yes
    }

    fn eat_sym(&mut self, s: &str) -> bool {
        let yes = matches!(self.peek(), Tok::Sym(x) if *x == s);
        if yes {
            self.bump();
        }
        yes
    }

    fn at_line_end(&self) -> bool {
        matches!(self.peek(), Tok::Newline | Tok::Eof)
    }

    fn ident(&mut self, what: &str) -> Result<String, CompileError> {
        if let Tok::Ident(s) = self.peek().clone() {
            self.bump();
            Ok(s)
        } else {
            self.err_here(format!("expected {what}"))
        }
    }

    fn end_of_statement(&mut self) -> Result<(), CompileError> {
        match self.peek() {
            Tok::Newline => {
                self.bump();
                Ok(())
            }
            Tok::Eof => Ok(()),
            t => {
                let d = describe(t);
                self.err_here(format!("unexpected {d}"))
            }
        }
    }

    fn file(&mut self) -> Result<(), CompileError> {
        self.skip_newlines();
        if *self.peek() == Tok::Eof {
            return self.err_here("nothing to play");
        }
        if !self.is_word("sound") && !self.is_word("instrument") {
            let mut cx = Cx::new();
            self.body(&mut cx, &[], ("", 0, 0))?;
            return self.finish(String::from("main"), BlockKind::Sound, cx, None, (1, 1));
        }
        loop {
            self.skip_newlines();
            if *self.peek() == Tok::Eof {
                return Ok(());
            }
            let (l, c) = self.here();
            let kind = if self.eat_word("sound") {
                BlockKind::Sound
            } else if self.eat_word("instrument") {
                BlockKind::Instrument
            } else {
                return self.err_here("statement outside a block");
            };
            let opener = if kind == BlockKind::Sound { "sound" } else { "instrument" };
            let (nl, nc) = self.here();
            let name = self.ident("a name")?;
            if self.prog.blocks.iter().any(|b| b.name == name) {
                return Err(CompileError::new(nl, nc, format!("'{name}' is already defined")));
            }
            self.end_of_statement()?;
            let mut cx = Cx::new();
            let mut release = None;
            if self.body(&mut cx, &["end", "on"], (opener, l, c))? == "on" {
                let (ol, oc) = self.here();
                self.bump();
                if kind != BlockKind::Instrument {
                    return Err(CompileError::new(ol, oc, "only an instrument has 'on release'"));
                }
                if !self.eat_word("release") {
                    return self.err_here("expected 'release'");
                }
                self.end_of_statement()?;
                cx.emit(Op::End);
                release = Some(cx.here());
                self.body(&mut cx, &["end"], ("on release", ol, oc))?;
            }
            self.bump(); // end
            self.end_of_statement()?;
            self.finish(name, kind, cx, release, (l, c))?;
        }
    }

    /// Statements up to one of `terms` (not consumed). Returns that word,
    /// or "" at the end of the file when `terms` is empty.
    fn body(&mut self, cx: &mut Cx, terms: &[&'static str], opener: (&str, u32, u32)) -> Result<&'static str, CompileError> {
        loop {
            self.skip_newlines();
            if *self.peek() == Tok::Eof {
                if terms.is_empty() {
                    return Ok("");
                }
                let (w, l, c) = opener;
                return Err(CompileError::new(l, c, format!("'{w}' has no 'end'")));
            }
            if let Some(t) = terms.iter().find(|t| self.is_word(t)) {
                return Ok(t);
            }
            self.statement(cx)?;
        }
    }

    fn finish(&mut self, name: String, kind: BlockKind, mut cx: Cx, release_pc: Option<u16>, at: (u32, u32)) -> Result<(), CompileError> {
        cx.emit(Op::End);
        if cx.code.len() > u16::MAX as usize {
            return Err(CompileError::new(at.0, at.1, "script too long"));
        }
        self.prog.blocks.push(Block { name, kind, code: cx.code, release_pc, vars: cx.vars.len() as u8, uses_v2: cx.uses_v2 });
        Ok(())
    }

    fn statement(&mut self, cx: &mut Cx) -> Result<(), CompileError> {
        let (l, c) = self.here();
        let word = match self.peek().clone() {
            Tok::Ident(w) => w,
            t => return self.err_here(format!("unexpected {}", describe(&t))),
        };
        self.bump();
        match word.as_str() {
            "let" => {
                let (nl, nc) = self.here();
                let name = self.ident("a variable name")?;
                if RESERVED.contains(&name.as_str()) {
                    return Err(CompileError::new(nl, nc, format!("'{name}' is a reserved word")));
                }
                if !self.eat_sym("=") {
                    return self.err_here("expected '='");
                }
                self.expr(cx)?;
                let slot = cx.new_var(name, nl, nc)?;
                cx.emit(Op::Store(slot));
            }
            "if" => self.if_stmt(cx, l, c)?,
            "repeat" => self.repeat_stmt(cx, l, c)?,
            "loop" => {
                self.end_of_statement()?;
                let top = cx.here();
                self.body(cx, &["end"], ("loop", l, c))?;
                self.bump();
                cx.emit(Op::Jump(top));
            }
            "wait" => {
                if self.eat_word("row") {
                    cx.emit(Op::WaitRow);
                } else if self.eat_word("beat") {
                    cx.emit(Op::WaitBeat);
                } else {
                    self.expr(cx)?;
                    cx.emit(Op::Wait);
                }
            }
            "stop" => {
                cx.emit(Op::Stop);
            }
            "end" | "else" | "on" => return Err(CompileError::new(l, c, format!("unexpected '{word}'"))),
            _ => {
                if let Some(slot) = cx.var(&word) {
                    if !self.eat_sym("=") {
                        return self.err_here("expected '='");
                    }
                    self.expr(cx)?;
                    cx.emit(Op::Store(slot));
                } else if matches!(self.peek(), Tok::Sym("=")) {
                    return Err(CompileError::new(l, c, format!("unknown variable '{word}' (use 'let')")));
                } else {
                    return Err(CompileError::new(l, c, format!("unknown command '{word}'")));
                }
            }
        }
        self.end_of_statement()
    }

    fn if_stmt(&mut self, cx: &mut Cx, l: u32, c: u32) -> Result<(), CompileError> {
        self.expr(cx)?;
        self.end_of_statement()?;
        let jz = cx.emit(Op::JumpIfZero(0));
        if self.body(cx, &["else", "end"], ("if", l, c))? == "else" {
            self.bump();
            self.end_of_statement()?;
            let j = cx.emit(Op::Jump(0));
            let at = cx.here();
            cx.patch(jz, at);
            self.body(cx, &["end"], ("if", l, c))?;
            let at = cx.here();
            cx.patch(j, at);
        } else {
            let at = cx.here();
            cx.patch(jz, at);
        }
        self.bump(); // end
        Ok(())
    }

    fn repeat_stmt(&mut self, cx: &mut Cx, l: u32, c: u32) -> Result<(), CompileError> {
        self.expr(cx)?;
        self.end_of_statement()?;
        let slot = cx.hidden_var(l, c)?;
        cx.emit(Op::Store(slot));
        let top = cx.here();
        cx.emit(Op::Load(slot));
        cx.emit(Op::Push(0));
        cx.emit(Op::Bin(BinOp::Gt));
        let jz = cx.emit(Op::JumpIfZero(0));
        self.body(cx, &["end"], ("repeat", l, c))?;
        self.bump(); // end
        cx.emit(Op::Load(slot));
        cx.emit(Op::Push(1));
        cx.emit(Op::Bin(BinOp::Sub));
        cx.emit(Op::Store(slot));
        cx.emit(Op::Jump(top));
        let at = cx.here();
        cx.patch(jz, at);
        Ok(())
    }

    fn expr(&mut self, cx: &mut Cx) -> Result<(), CompileError> {
        self.and_expr(cx)?;
        while self.eat_word("or") {
            self.and_expr(cx)?;
            cx.emit(Op::Bin(BinOp::Or));
        }
        Ok(())
    }

    fn and_expr(&mut self, cx: &mut Cx) -> Result<(), CompileError> {
        self.cmp_expr(cx)?;
        while self.eat_word("and") {
            self.cmp_expr(cx)?;
            cx.emit(Op::Bin(BinOp::And));
        }
        Ok(())
    }

    fn cmp_expr(&mut self, cx: &mut Cx) -> Result<(), CompileError> {
        self.add_expr(cx)?;
        let ops = [("==", BinOp::Eq), ("!=", BinOp::Ne), ("<=", BinOp::Le), (">=", BinOp::Ge), ("<", BinOp::Lt), (">", BinOp::Gt)];
        for (s, op) in ops {
            if self.eat_sym(s) {
                self.add_expr(cx)?;
                cx.emit(Op::Bin(op));
                break;
            }
        }
        Ok(())
    }

    fn add_expr(&mut self, cx: &mut Cx) -> Result<(), CompileError> {
        self.mul_expr(cx)?;
        loop {
            let op = if self.eat_sym("+") {
                BinOp::Add
            } else if self.eat_sym("-") {
                BinOp::Sub
            } else {
                return Ok(());
            };
            self.mul_expr(cx)?;
            cx.emit(Op::Bin(op));
        }
    }

    fn mul_expr(&mut self, cx: &mut Cx) -> Result<(), CompileError> {
        self.unary(cx)?;
        loop {
            let op = if self.eat_sym("*") {
                BinOp::Mul
            } else if self.eat_sym("/") {
                BinOp::Div
            } else if self.eat_sym("%") {
                BinOp::Mod
            } else {
                return Ok(());
            };
            self.unary(cx)?;
            cx.emit(Op::Bin(op));
        }
    }

    /// Also the form of a side-by-side command argument.
    fn unary(&mut self, cx: &mut Cx) -> Result<(), CompileError> {
        if self.eat_sym("-") {
            self.unary(cx)?;
            cx.emit(Op::Neg);
        } else if self.eat_word("not") {
            self.unary(cx)?;
            cx.emit(Op::Not);
        } else if self.eat_word("rand") {
            self.unary(cx)?;
            cx.emit(Op::Rand);
        } else {
            self.primary(cx)?;
        }
        Ok(())
    }

    fn primary(&mut self, cx: &mut Cx) -> Result<(), CompileError> {
        let (l, c) = self.here();
        match self.bump() {
            Tok::Num(n) => {
                cx.emit(Op::Push(n));
            }
            Tok::Sym("(") => {
                self.expr(cx)?;
                if !self.eat_sym(")") {
                    return self.err_here("expected ')'");
                }
            }
            Tok::Ident(w) => {
                if let Some(s) = cx.var(&w) {
                    cx.emit(Op::Load(s));
                } else if let Some(b) = builtin(&w) {
                    cx.emit(Op::Get(b));
                } else {
                    return Err(CompileError::new(l, c, format!("unknown name '{w}'")));
                }
            }
            t => return Err(CompileError::new(l, c, format!("expected a value, found {}", describe(&t)))),
        }
        Ok(())
    }
}
```

- [ ] **Step 5: Run the tests to see them pass**

Run: `cargo test --manifest-path v3/Cargo.toml -p acid-sound compile`
Expected: 9 passed.

- [ ] **Step 6: Commit**

```bash
git add v3/crates/acid-sound
git commit -m "acid-sound: bytecode and compiler core (blocks, variables, control flow)

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 4: Compiler sound and song commands

**Files:**
- Modify: `v3/crates/acid-sound/src/compile.rs`

**Interfaces:**
- Consumes: `program::{Cmd, Target, SongRef}` from Task 3
- Produces:
  - Statements that compile to `Op::Cmd(..)`:
    - Voice commands, with an optional `v1`/`v2`/`both` prefix: `wave`, `duty`, `adsr`, `gate`, `pitch`, `fine`, `ring`, `arp`, `arprate`, `route`
    - `filter`, `song "PATH"`, `play [E]`, `stop song`, `tempo`, `mute`, `unmute`, `jump`
  - `Block::uses_v2` is set when any `v2` or `both` prefix appears

- [ ] **Step 1: Write the failing tests**

Add these inside `compile.rs`'s `mod tests`:

```rust
    fn cmd(c: Cmd) -> Op {
        Op::Cmd(c, Target::V1)
    }

    #[test]
    fn voice_commands() {
        assert_eq!(code("wave saw"), vec![cmd(Cmd::Wave(1)), Op::End]);
        assert_eq!(code("gate on\nring off\nroute on"), vec![cmd(Cmd::Gate(true)), cmd(Cmd::Ring(false)), cmd(Cmd::Route(true)), Op::End]);
        assert_eq!(code("pitch -4"), vec![Op::Push(4), Op::Neg, cmd(Cmd::Pitch { rel: true }), Op::End]);
        assert_eq!(
            code("pitch note + 12"),
            vec![Op::Get(Builtin::Note), Op::Push(12), Op::Bin(BinOp::Add), cmd(Cmd::Pitch { rel: false }), Op::End]
        );
        assert_eq!(code("fine +6"), vec![Op::Push(6), cmd(Cmd::Fine { rel: true }), Op::End]);
        assert_eq!(code("arp 0 4 7"), vec![Op::Push(0), Op::Push(4), Op::Push(7), cmd(Cmd::Arp(3)), Op::End]);
        assert_eq!(code("arp off\narprate 30"), vec![cmd(Cmd::ArpOff), Op::Push(30), cmd(Cmd::ArpRate), Op::End]);
    }

    #[test]
    fn voice_prefixes() {
        let p = compile("v2 duty +8").unwrap();
        assert_eq!(p.blocks[0].code, vec![Op::Push(8), Op::Cmd(Cmd::Duty { rel: true }, Target::V2), Op::End]);
        assert!(p.blocks[0].uses_v2);
        assert!(!compile("v1 wave saw").unwrap().blocks[0].uses_v2);
        assert_eq!(
            code("both adsr 2 120 -1 80"),
            vec![Op::Push(2), Op::Push(120), Op::Push(1), Op::Neg, Op::Push(80), Op::Cmd(Cmd::Adsr, Target::Both), Op::End]
        );
    }

    #[test]
    fn filter_and_song_commands() {
        assert_eq!(code("filter lp 40 res 6"), vec![Op::Push(40), Op::Push(6), cmd(Cmd::Filter(1)), Op::End]);
        let p = compile("song \"a.trk\"\nplay\nsong \"b.trk\"\nsong \"a.trk\"\nplay 2\ntempo 3\nmute 2\nunmute 2\njump 1\nstop song\nstop").unwrap();
        let paths: Vec<&str> = p.song_paths.iter().map(|s| s.path.as_str()).collect();
        assert_eq!(paths, ["a.trk", "b.trk"]);
        assert_eq!((p.song_paths[1].line, p.song_paths[1].col), (3, 6));
        assert_eq!(
            p.blocks[0].code,
            vec![
                cmd(Cmd::SongSelect(0)), Op::Push(0), cmd(Cmd::SongPlay),
                cmd(Cmd::SongSelect(1)), cmd(Cmd::SongSelect(0)), Op::Push(2), cmd(Cmd::SongPlay),
                Op::Push(3), cmd(Cmd::Tempo), Op::Push(2), cmd(Cmd::Mute(true)), Op::Push(2), cmd(Cmd::Mute(false)),
                Op::Push(1), cmd(Cmd::Jump), cmd(Cmd::SongStop), Op::Stop, Op::End,
            ]
        );
    }

    #[test]
    fn command_errors() {
        assert_eq!(err("arp"), "1:4 arp needs 1 to 3 notes");
        assert_eq!(err("arp 1 2 3 4"), "1:11 arp takes at most 3 notes");
        assert_eq!(err("gate maybe"), "1:6 expected 'on' or 'off'");
        assert_eq!(err("wave square"), "1:6 unknown waveform 'square'");
        assert_eq!(err("v2 filter lp 1 res 1"), "1:4 'v2' can't go before 'filter'");
        assert_eq!(err("filter xx 1 res 1"), "1:8 unknown filter mode 'xx'");
        assert_eq!(err("filter lp 1 2"), "1:13 expected 'res'");
        assert_eq!(err("song a"), "1:6 expected a song path in quotes");
    }
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test --manifest-path v3/Cargo.toml -p acid-sound compile`
Expected: the 4 new tests FAIL, for example `unknown command 'wave'`.

- [ ] **Step 3: Implement**

In `compile.rs`, add a constant below `RESERVED`:

```rust
/// Commands that act on a voice, so may take a v1/v2/both prefix.
const VOICE_CMDS: &[&str] = &["wave", "duty", "adsr", "gate", "pitch", "fine", "ring", "arp", "arprate", "route"];
```

In `Parser::statement`, replace the `"stop" => { cx.emit(Op::Stop); }` arm with all of the following arms:

```rust
            "stop" => {
                if self.eat_word("song") {
                    cx.emit(Op::Cmd(Cmd::SongStop, Target::V1));
                } else {
                    cx.emit(Op::Stop);
                }
            }
            "v1" | "v2" | "both" => {
                let target = match word.as_str() {
                    "v1" => Target::V1,
                    "v2" => Target::V2,
                    _ => Target::Both,
                };
                let (cl, cc) = self.here();
                let name = self.ident("a command after the voice")?;
                if !VOICE_CMDS.contains(&name.as_str()) {
                    return Err(CompileError::new(cl, cc, format!("'{word}' can't go before '{name}'")));
                }
                if target != Target::V1 {
                    cx.uses_v2 = true;
                }
                self.voice_cmd(cx, &name, target)?;
            }
            w if VOICE_CMDS.contains(&w) => self.voice_cmd(cx, w, Target::V1)?,
            "filter" => {
                let (ml, mc) = self.here();
                let m = self.ident("lp, bp or hp")?;
                let mode = match m.as_str() {
                    "lp" => 1,
                    "bp" => 2,
                    "hp" => 4,
                    _ => return Err(CompileError::new(ml, mc, format!("unknown filter mode '{m}'"))),
                };
                self.unary(cx)?;
                if !self.eat_word("res") {
                    return self.err_here("expected 'res'");
                }
                self.unary(cx)?;
                cx.emit(Op::Cmd(Cmd::Filter(mode), Target::V1));
            }
            "song" => {
                let (pl, pc) = self.here();
                let Tok::Str(path) = self.peek().clone() else {
                    return self.err_here("expected a song path in quotes");
                };
                self.bump();
                let idx = match self.prog.song_paths.iter().position(|s| s.path == path) {
                    Some(i) => i,
                    None => {
                        self.prog.song_paths.push(SongRef { path, line: pl, col: pc });
                        self.prog.song_paths.len() - 1
                    }
                };
                if idx > u8::MAX as usize {
                    return Err(CompileError::new(pl, pc, "too many songs"));
                }
                cx.emit(Op::Cmd(Cmd::SongSelect(idx as u8), Target::V1));
            }
            "play" => {
                if self.at_line_end() {
                    cx.emit(Op::Push(0));
                } else {
                    self.expr(cx)?;
                }
                cx.emit(Op::Cmd(Cmd::SongPlay, Target::V1));
            }
            "tempo" => {
                self.expr(cx)?;
                cx.emit(Op::Cmd(Cmd::Tempo, Target::V1));
            }
            "mute" | "unmute" => {
                self.expr(cx)?;
                cx.emit(Op::Cmd(Cmd::Mute(word == "mute"), Target::V1));
            }
            "jump" => {
                self.expr(cx)?;
                cx.emit(Op::Cmd(Cmd::Jump, Target::V1));
            }
```

Add these methods to `impl Parser`:

```rust
    fn voice_cmd(&mut self, cx: &mut Cx, name: &str, t: Target) -> Result<(), CompileError> {
        let c = match name {
            "wave" => {
                let (l, c) = self.here();
                let w = self.ident("a waveform")?;
                Cmd::Wave(match w.as_str() {
                    "pulse" => 0,
                    "saw" => 1,
                    "tri" => 2,
                    "noise" => 3,
                    _ => return Err(CompileError::new(l, c, format!("unknown waveform '{w}'"))),
                })
            }
            "duty" => Cmd::Duty { rel: self.maybe_relative(cx)? },
            "pitch" => Cmd::Pitch { rel: self.maybe_relative(cx)? },
            "fine" => Cmd::Fine { rel: self.maybe_relative(cx)? },
            "adsr" => {
                for _ in 0..4 {
                    self.unary(cx)?;
                }
                Cmd::Adsr
            }
            "gate" => Cmd::Gate(self.on_off()?),
            "ring" => Cmd::Ring(self.on_off()?),
            "route" => Cmd::Route(self.on_off()?),
            "arp" => {
                if self.eat_word("off") {
                    Cmd::ArpOff
                } else {
                    let (l, c) = self.here();
                    let mut n = 0u8;
                    while !self.at_line_end() {
                        if n == 3 {
                            return self.err_here("arp takes at most 3 notes");
                        }
                        self.unary(cx)?;
                        n += 1;
                    }
                    if n == 0 {
                        return Err(CompileError::new(l, c, "arp needs 1 to 3 notes"));
                    }
                    Cmd::Arp(n)
                }
            }
            "arprate" => {
                self.expr(cx)?;
                Cmd::ArpRate
            }
            _ => unreachable!("VOICE_CMDS and voice_cmd disagree on '{name}'"),
        };
        cx.emit(Op::Cmd(c, t));
        Ok(())
    }

    /// A leading + or - makes the value relative to the current one.
    fn maybe_relative(&mut self, cx: &mut Cx) -> Result<bool, CompileError> {
        if self.eat_sym("+") {
            self.expr(cx)?;
            Ok(true)
        } else if self.eat_sym("-") {
            self.expr(cx)?;
            cx.emit(Op::Neg);
            Ok(true)
        } else {
            self.expr(cx)?;
            Ok(false)
        }
    }

    fn on_off(&mut self) -> Result<bool, CompileError> {
        if self.eat_word("on") {
            Ok(true)
        } else if self.eat_word("off") {
            Ok(false)
        } else {
            self.err_here("expected 'on' or 'off'")
        }
    }
```

The `"v1" | "v2" | "both"` arm and the `w if VOICE_CMDS...` arm must come **before** the `"end" | "else" | "on"` arm and the `_` fallback.

- [ ] **Step 4: Run all `acid-sound` tests**

Run: `cargo test --manifest-path v3/Cargo.toml -p acid-sound`
Expected: everything passes, 13 compile tests in all.

- [ ] **Step 5: Commit**

```bash
git add v3/crates/acid-sound
git commit -m "acid-sound: compile voice, filter and song commands

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 5: The VM (`Instance`)

**Files:**
- Create: `v3/crates/acid-sound/src/vm.rs`
- Modify: `v3/crates/acid-sound/src/lib.rs` (add `pub mod vm;`)

**Interfaces:**
- Consumes:
  - `program::*`
  - `pitch::{phase_increment, ONA_MIN, ONA_MAX, FINE_MAX}`
  - `acid_synth::Synth` setters: `set_voice_waveform`, `set_duty`, `set_adsr`, `gate_on`, `gate_off`, `set_ring_partner`, `clear_ring_partner`, `set_voice_filter_route`, `set_arp_note`, `set_arp_rate`, `arp_on`, `set_filter_mode`, `set_filter_cutoff`, `set_filter_resonance`, `voice_mut`
- Produces:
  - `vm::BUDGET: u32 = 256` and `vm::SONG_CMDS_MAX: usize = 16`
  - `vm::SongCmd` with the variants `Play { song: Option<u8>, order: i32 }`, `Stop`, `Tempo(i32)`, `Mute(i32, bool)`, `Jump(i32)`
  - `vm::Clock { playing: bool, order: i32, row: i32, row_serial: u32 }`, which is `Default` and `Copy`
  - `vm::Env<'a> { synth: &'a mut Synth, clock: Clock, song_cmds: &'a mut Vec<SongCmd> }`
  - `vm::State` with the variants `Running`, `Idle`, `Done`
  - `vm::Instance`, with the public fields `block: usize`, `voices: [Option<u8>; 2]` and `state: State`, and these methods:
    - `new(block, voices, note, note2, seed) -> Instance`
    - `start(&mut self, &mut Synth)`
    - `tick(&mut self, &Block, &mut Env)`
    - `release(&mut self, &Block, &mut Synth)`
    - `stop(&mut self, &mut Synth)`
    - `drop_voice(&mut self, u8)`
    - `var(&self, usize) -> i32`

- [ ] **Step 1: Write the failing tests**

Create `v3/crates/acid-sound/src/vm.rs` with:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::compile::compile;
    use acid_synth::{EnvStage, Waveform, ONA_PHASE_INCREMENT};

    struct Rig {
        prog: Program,
        inst: Instance,
        synth: Synth,
        cmds: Vec<SongCmd>,
        clock: Clock,
    }

    fn rig(src: &str, voices: [Option<u8>; 2]) -> Rig {
        let prog = compile(src).unwrap();
        let mut synth = Synth::new();
        let mut inst = Instance::new(0, voices, 40, 0, 7);
        inst.start(&mut synth);
        Rig { prog, inst, synth, cmds: Vec::with_capacity(SONG_CMDS_MAX), clock: Clock::default() }
    }

    impl Rig {
        fn tick(&mut self, n: usize) {
            for _ in 0..n {
                let mut env = Env { synth: &mut self.synth, clock: self.clock, song_cmds: &mut self.cmds };
                self.inst.tick(&self.prog.blocks[self.inst.block], &mut env);
            }
        }
    }

    const ONE: [Option<u8>; 2] = [Some(0), None];
    const TWO: [Option<u8>; 2] = [Some(0), Some(1)];

    #[test]
    fn waits_sleep_whole_ticks() {
        let mut r = rig("let t = 0\nloop\nt = t + 1\nwait 2\nend", ONE);
        r.tick(1);
        assert_eq!(r.inst.var(0), 1);
        r.tick(1);
        assert_eq!(r.inst.var(0), 1);
        r.tick(1);
        assert_eq!(r.inst.var(0), 2);
    }

    #[test]
    fn a_busy_loop_yields_at_the_budget() {
        // 2 ops of setup, then 5 per pass: 256 ops leave x at 51.
        let mut r = rig("let x = 0\nloop\nx = x + 1\nend", ONE);
        r.tick(1);
        assert_eq!(r.inst.var(0), 51);
        assert_eq!(r.inst.state, State::Running);
        r.tick(1);
        assert_eq!(r.inst.var(0), 102);
    }

    #[test]
    fn arithmetic_saturates_and_never_fails() {
        let mut r = rig("let a = 2147483647 + 1\nlet b = 5 / 0\nlet c = 5 % 0\nlet d = 0 - 2147483647 - 10\nlet e = 7 / 2", ONE);
        r.tick(1);
        assert_eq!([r.inst.var(0), r.inst.var(1), r.inst.var(2), r.inst.var(3), r.inst.var(4)], [i32::MAX, 0, 0, i32::MIN, 3]);
    }

    #[test]
    fn comparisons_and_logic_give_one_or_zero() {
        let mut r = rig("let a = 3 > 2 and not 0\nlet b = 1 == 2 or 0", ONE);
        r.tick(1);
        assert_eq!((r.inst.var(0), r.inst.var(1)), (1, 0));
    }

    #[test]
    fn rand_is_in_range_and_repeatable() {
        let prog = compile("let r = rand 6").unwrap();
        let mut seen = alloc::collections::BTreeSet::new();
        for seed in 0..50 {
            let roll = |seed| {
                let mut s = Synth::new();
                let mut i = Instance::new(0, ONE, 40, 0, seed);
                let mut cmds = Vec::new();
                i.tick(&prog.blocks[0], &mut Env { synth: &mut s, clock: Clock::default(), song_cmds: &mut cmds });
                i.var(0)
            };
            let v = roll(seed);
            assert!((0..6).contains(&v));
            assert_eq!(v, roll(seed), "same seed, same roll");
            seen.insert(v);
        }
        assert!(seen.len() >= 3, "rand should vary with the seed: {seen:?}");
    }

    #[test]
    fn values_a_script_can_read() {
        let mut r = rig("let a = note\nlet b = note2\nlet c = row\nlet d = tick\nwait 1\nd = tick", ONE);
        r.tick(1);
        assert_eq!((r.inst.var(0), r.inst.var(1), r.inst.var(2), r.inst.var(3)), (40, 0, -1, 0));
        r.tick(1);
        assert_eq!(r.inst.var(3), 1);
    }

    #[test]
    fn gate_on_sounds_the_voice() {
        let mut r = rig("wave saw\npitch 44\ngate on", [Some(3), None]);
        r.tick(1);
        let v = r.synth.voice(3);
        assert_eq!(v.waveform, Waveform::Saw);
        assert_eq!(v.envelope_stage, EnvStage::Release, "a sound running off its end gates off");
        assert_eq!(v.phase_increment, ONA_PHASE_INCREMENT[43]);
    }

    #[test]
    fn gate_on_without_pitch_uses_the_start_note() {
        let mut r = rig("gate on\nwait 5", ONE);
        r.tick(1);
        assert_eq!(r.synth.voice(0).envelope_stage, EnvStage::Attack);
        assert_eq!(r.synth.voice(0).phase_increment, ONA_PHASE_INCREMENT[39]);
    }

    #[test]
    fn relative_pitch_and_fine() {
        let mut r = rig("pitch +2\nfine 32\nwait 5", ONE);
        r.tick(1);
        assert_eq!(r.synth.voice(0).phase_increment, crate::pitch::phase_increment(42, 32));
    }

    #[test]
    fn a_missing_voice_is_skipped_but_its_arguments_are_used_up() {
        let mut r = rig("v2 adsr 1 2 3 4\nlet x = 9\nv2 wave noise\nwave saw\nwait 5", ONE);
        r.tick(1);
        assert_eq!(r.inst.var(0), 9);
        assert_eq!(r.synth.voice(0).waveform, Waveform::Saw);
        assert_eq!(r.synth.voice(1).waveform, Waveform::Pulse, "voice 1 isn't this instance's");
    }

    #[test]
    fn both_targets_two_voices() {
        let mut r = rig("both wave tri\nwait 5", TWO);
        r.tick(1);
        assert_eq!(r.synth.voice(0).waveform, Waveform::Triangle);
        assert_eq!(r.synth.voice(1).waveform, Waveform::Triangle);
    }

    #[test]
    fn stop_gates_off_and_finishes() {
        let mut r = rig("gate on\nstop\nwait 1", ONE);
        r.tick(1);
        assert_eq!(r.synth.voice(0).envelope_stage, EnvStage::Release);
        assert_eq!(r.inst.state, State::Done);
    }

    #[test]
    fn an_instrument_holds_its_note_until_release() {
        let mut r = rig("instrument a\ngate on\nend", ONE);
        r.tick(1);
        assert_eq!(r.inst.state, State::Idle);
        assert_eq!(r.synth.voice(0).envelope_stage, EnvStage::Attack);
        r.inst.release(&r.prog.blocks[0], &mut r.synth);
        assert_eq!(r.synth.voice(0).envelope_stage, EnvStage::Release);
        assert_eq!(r.inst.state, State::Done);
    }

    #[test]
    fn on_release_runs_after_note_off() {
        let mut r = rig("instrument a\ngate on\non release\nwave noise\nend", ONE);
        r.tick(1);
        r.inst.release(&r.prog.blocks[0], &mut r.synth);
        assert_eq!(r.inst.state, State::Running);
        r.tick(1);
        assert_eq!(r.synth.voice(0).waveform, Waveform::Noise);
        assert_eq!(r.inst.state, State::Done);
        assert_ne!(r.synth.voice(0).envelope_stage, EnvStage::Release, "release code decides the gate");
    }

    #[test]
    fn wait_row_follows_the_song() {
        let mut r = rig("let n = 0\nloop\nwait row\nn = n + 1\nend", ONE);
        r.clock = Clock { playing: true, order: 0, row: 0, row_serial: 0 };
        r.tick(2);
        assert_eq!(r.inst.var(0), 0);
        r.clock.row_serial = 1;
        r.tick(1);
        assert_eq!(r.inst.var(0), 1);
    }

    #[test]
    fn wait_beat_waits_for_a_row_on_the_beat() {
        let mut r = rig("let n = 0\nloop\nwait beat\nn = n + 1\nend", ONE);
        r.clock = Clock { playing: true, order: 0, row: 3, row_serial: 0 };
        r.tick(1);
        r.clock = Clock { playing: true, order: 0, row: 3, row_serial: 1 };
        r.tick(1);
        assert_eq!(r.inst.var(0), 0, "row 3 isn't a beat");
        r.clock = Clock { playing: true, order: 0, row: 4, row_serial: 2 };
        r.tick(1);
        assert_eq!(r.inst.var(0), 1);
    }

    #[test]
    fn without_a_song_wait_row_is_one_tick() {
        let mut r = rig("let n = 0\nloop\nwait row\nn = n + 1\nend", ONE);
        r.tick(3);
        assert_eq!(r.inst.var(0), 2);
    }

    #[test]
    fn song_commands_queue_for_the_engine() {
        let mut r = rig("song \"a.trk\"\nplay 3\ntempo 4\nmute 2\nunmute 1\njump 5\nstop song", ONE);
        r.tick(1);
        assert_eq!(
            r.cmds,
            vec![
                SongCmd::Play { song: Some(0), order: 3 }, SongCmd::Tempo(4), SongCmd::Mute(2, true),
                SongCmd::Mute(1, false), SongCmd::Jump(5), SongCmd::Stop,
            ]
        );
    }

    #[test]
    fn the_song_command_queue_never_grows() {
        let mut r = rig("loop\ntempo 1\nend", ONE);
        r.tick(1);
        assert_eq!(r.cmds.len(), SONG_CMDS_MAX);
        assert_eq!(r.cmds.capacity(), SONG_CMDS_MAX);
    }

    #[test]
    fn arp_offsets_from_the_current_pitch() {
        let mut r = rig("pitch 40\narp 4 7\nwait 5", ONE);
        r.tick(1);
        let v = r.synth.voice(0);
        assert!(v.arp_active);
        assert_eq!(v.arp_count, 3);
        assert_eq!(&v.arp_notes[..3], &[40, 44, 47]);
    }

    #[test]
    fn a_dropped_voice_is_left_alone() {
        let mut r = rig("wave saw\nwait 5", ONE);
        r.inst.drop_voice(0);
        r.tick(1);
        assert_eq!(r.synth.voice(0).waveform, Waveform::Pulse);
    }
}
```

Add `pub mod vm;` to `lib.rs`.

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test --manifest-path v3/Cargo.toml -p acid-sound vm`
Expected: compile errors, `cannot find struct Instance` and so on.

- [ ] **Step 3: Implement the VM**

Put this above the tests in `vm.rs`:

```rust
//! The .snd virtual machine. An `Instance` is one running sound or one
//! instrument note: it owns up to two synth voices and runs its block a
//! tick at a time, never more than BUDGET instructions per tick, and
//! nothing it does can panic or allocate.

use alloc::vec::Vec;

use acid_synth::Synth;

use crate::pitch;
use crate::program::*;

/// Instructions an instance may run per tick; running out acts as `wait 1`.
pub const BUDGET: u32 = 256;
/// Song commands one tick may queue; more are dropped, never allocated.
pub const SONG_CMDS_MAX: usize = 16;
const STACK: usize = 32;
const DEFAULT_ARP_MS: i32 = 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SongCmd {
    /// `song` indexes Program::song_paths; None restarts the current song.
    Play { song: Option<u8>, order: i32 },
    Stop,
    Tempo(i32),
    /// A 1-based channel.
    Mute(i32, bool),
    Jump(i32),
}

/// How the playing song looks this tick.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Clock {
    pub playing: bool,
    pub order: i32,
    pub row: i32,
    /// Counts rows started, so a wait can tell a new row from the same one.
    pub row_serial: u32,
}

pub struct Env<'a> {
    pub synth: &'a mut Synth,
    pub clock: Clock,
    pub song_cmds: &'a mut Vec<SongCmd>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Running,
    /// An instrument's main part ended; its note holds until release.
    Idle,
    Done,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Waiting {
    No,
    Ticks(i32),
    Row(u32),
    Beat(u32),
}

#[derive(Clone, Debug)]
pub struct Instance {
    pub block: usize,
    pub voices: [Option<u8>; 2],
    pub state: State,
    pc: usize,
    stack: [i32; STACK],
    sp: usize,
    vars: [i32; MAX_VARS],
    waiting: Waiting,
    pitch: [i32; 2],
    fine: [i32; 2],
    duty: [i32; 2],
    arp_ms: i32,
    note: i32,
    note2: i32,
    ticks: i32,
    rng: u32,
    song: Option<u8>,
}

fn bin(o: BinOp, l: i32, r: i32) -> i32 {
    match o {
        BinOp::Add => l.saturating_add(r),
        BinOp::Sub => l.saturating_sub(r),
        BinOp::Mul => l.saturating_mul(r),
        BinOp::Div => if r == 0 { 0 } else { l.saturating_div(r) },
        BinOp::Mod => if r == 0 { 0 } else { l.checked_rem(r).unwrap_or(0) },
        BinOp::Eq => (l == r) as i32,
        BinOp::Ne => (l != r) as i32,
        BinOp::Lt => (l < r) as i32,
        BinOp::Le => (l <= r) as i32,
        BinOp::Gt => (l > r) as i32,
        BinOp::Ge => (l >= r) as i32,
        BinOp::And => (l != 0 && r != 0) as i32,
        BinOp::Or => (l != 0 || r != 0) as i32,
    }
}

fn queue(env: &mut Env, c: SongCmd) {
    if env.song_cmds.len() < SONG_CMDS_MAX {
        env.song_cmds.push(c);
    }
}

impl Instance {
    /// Both voices start at `note`; `note2` is what the script's `note2` reads.
    pub fn new(block: usize, voices: [Option<u8>; 2], note: i32, note2: i32, seed: u32) -> Self {
        let note = note.clamp(pitch::ONA_MIN, pitch::ONA_MAX);
        Self {
            block,
            voices,
            state: State::Running,
            pc: 0,
            stack: [0; STACK],
            sp: 0,
            vars: [0; MAX_VARS],
            waiting: Waiting::No,
            pitch: [note; 2],
            fine: [0; 2],
            duty: [50; 2],
            arp_ms: DEFAULT_ARP_MS,
            note,
            note2,
            ticks: 0,
            rng: seed.wrapping_mul(0x9E37_79B9) | 1,
            song: None,
        }
    }

    /// Points the voices at the start note, so `gate on` sounds without a `pitch`.
    pub fn start(&mut self, synth: &mut Synth) {
        for s in 0..2 {
            self.apply_pitch(synth, s);
        }
    }

    pub fn var(&self, i: usize) -> i32 {
        self.vars[i]
    }

    /// The voice was lent to a sound effect: stop touching it.
    pub fn drop_voice(&mut self, v: u8) {
        for slot in &mut self.voices {
            if *slot == Some(v) {
                *slot = None;
            }
        }
    }

    /// Note-off: run `on release`, or gate off and finish.
    pub fn release(&mut self, b: &Block, synth: &mut Synth) {
        if self.state == State::Done {
            return;
        }
        match b.release_pc {
            Some(pc) => {
                self.pc = pc as usize;
                self.sp = 0;
                self.waiting = Waiting::No;
                self.state = State::Running;
            }
            None => self.stop(synth),
        }
    }

    pub fn stop(&mut self, synth: &mut Synth) {
        for v in self.voices.iter().flatten() {
            synth.gate_off(*v as i32);
        }
        self.state = State::Done;
    }

    pub fn tick(&mut self, b: &Block, env: &mut Env) {
        if self.state == State::Running {
            self.step(b, env);
        }
        self.ticks = self.ticks.saturating_add(1);
    }

    fn resume(&mut self, clock: Clock) -> bool {
        let go = match self.waiting {
            Waiting::No => true,
            Waiting::Ticks(n) if n > 1 => {
                self.waiting = Waiting::Ticks(n - 1);
                false
            }
            Waiting::Ticks(_) => true,
            Waiting::Row(s) => !clock.playing || clock.row_serial != s,
            Waiting::Beat(s) => !clock.playing || (clock.row_serial != s && clock.row % 4 == 0),
        };
        if go {
            self.waiting = Waiting::No;
        }
        go
    }

    fn step(&mut self, b: &Block, env: &mut Env) {
        if !self.resume(env.clock) {
            return;
        }
        for _ in 0..BUDGET {
            let Some(&op) = b.code.get(self.pc) else {
                self.finish(b, env.synth);
                return;
            };
            self.pc += 1;
            match op {
                Op::Push(n) => self.push(n),
                Op::Load(s) => {
                    let v = self.vars[s as usize];
                    self.push(v);
                }
                Op::Store(s) => {
                    let v = self.pop();
                    self.vars[s as usize] = v;
                }
                Op::Get(g) => {
                    let v = self.get(g, env.clock);
                    self.push(v);
                }
                Op::Bin(o) => {
                    let r = self.pop();
                    let l = self.pop();
                    self.push(bin(o, l, r));
                }
                Op::Neg => {
                    let v = self.pop();
                    self.push(v.saturating_neg());
                }
                Op::Not => {
                    let v = self.pop();
                    self.push((v == 0) as i32);
                }
                Op::Rand => {
                    let n = self.pop();
                    let r = self.next_rand();
                    self.push(if n <= 0 { 0 } else { (r % n as u32) as i32 });
                }
                Op::Jump(t) => self.pc = t as usize,
                Op::JumpIfZero(t) => {
                    if self.pop() == 0 {
                        self.pc = t as usize;
                    }
                }
                Op::Wait => {
                    let n = self.pop();
                    if n >= 1 {
                        self.waiting = Waiting::Ticks(n);
                        return;
                    }
                }
                Op::WaitRow => {
                    self.waiting = Waiting::Row(env.clock.row_serial);
                    return;
                }
                Op::WaitBeat => {
                    self.waiting = Waiting::Beat(env.clock.row_serial);
                    return;
                }
                Op::Stop => {
                    self.stop(env.synth);
                    return;
                }
                Op::End => {
                    self.finish(b, env.synth);
                    return;
                }
                Op::Cmd(c, t) => self.command(c, t, env),
            }
        }
    }

    /// A sound is over (gate off). An instrument's note holds (Idle) until
    /// note-off. A finished `on release` is Done and leaves the gate alone.
    fn finish(&mut self, b: &Block, synth: &mut Synth) {
        let in_release = b.release_pc.is_some_and(|r| self.pc > r as usize);
        match b.kind {
            BlockKind::Sound => self.stop(synth),
            BlockKind::Instrument if in_release => self.state = State::Done,
            BlockKind::Instrument => self.state = State::Idle,
        }
    }

    fn push(&mut self, v: i32) {
        if self.sp < STACK {
            self.stack[self.sp] = v;
            self.sp += 1;
        }
    }

    fn pop(&mut self) -> i32 {
        if self.sp == 0 {
            return 0;
        }
        self.sp -= 1;
        self.stack[self.sp]
    }

    fn next_rand(&mut self) -> u32 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        x
    }

    fn get(&self, g: Builtin, clock: Clock) -> i32 {
        let song = |v: i32| if clock.playing { v } else { -1 };
        match g {
            Builtin::Note => self.note,
            Builtin::Note2 => self.note2,
            Builtin::Tick => self.ticks,
            Builtin::Row => song(clock.row),
            Builtin::Beat => song(clock.row / 4),
            Builtin::Order => song(clock.order),
        }
    }

    fn voice(&self, slot: usize) -> Option<i32> {
        self.voices[slot].map(i32::from)
    }

    /// The synth's arp drives the pitch while it runs, so leave it be.
    fn apply_pitch(&self, synth: &mut Synth, s: usize) {
        if let Some(v) = self.voices[s] {
            let vo = synth.voice_mut(v as usize);
            if !vo.arp_active {
                vo.phase_increment = pitch::phase_increment(self.pitch[s], self.fine[s]);
            }
        }
    }

    fn command(&mut self, c: Cmd, t: Target, env: &mut Env) {
        let slots: &[usize] = match t {
            Target::V1 => &[0],
            Target::V2 => &[1],
            Target::Both => &[0, 1],
        };
        match c {
            Cmd::Wave(w) => {
                for &s in slots {
                    if let Some(v) = self.voice(s) {
                        env.synth.set_voice_waveform(v, w);
                    }
                }
            }
            Cmd::Duty { rel } => {
                let n = self.pop();
                for &s in slots {
                    let d = if rel { self.duty[s].saturating_add(n) } else { n }.clamp(1, 99);
                    self.duty[s] = d;
                    if let Some(v) = self.voice(s) {
                        env.synth.set_duty(v, d);
                    }
                }
            }
            Cmd::Adsr => {
                let r = self.pop();
                let su = self.pop();
                let d = self.pop();
                let a = self.pop();
                for &s in slots {
                    if let Some(v) = self.voice(s) {
                        env.synth.set_adsr(v, a, d, su, r);
                    }
                }
            }
            Cmd::Gate(on) => {
                for &s in slots {
                    if let Some(v) = self.voice(s) {
                        if on { env.synth.gate_on(v) } else { env.synth.gate_off(v) }
                    }
                }
            }
            Cmd::Pitch { rel } => {
                let n = self.pop();
                for &s in slots {
                    let p = if rel { self.pitch[s].saturating_add(n) } else { n };
                    self.pitch[s] = p.clamp(pitch::ONA_MIN, pitch::ONA_MAX);
                    self.apply_pitch(env.synth, s);
                }
            }
            Cmd::Fine { rel } => {
                let n = self.pop();
                for &s in slots {
                    let f = if rel { self.fine[s].saturating_add(n) } else { n };
                    self.fine[s] = f.clamp(-pitch::FINE_MAX, pitch::FINE_MAX);
                    self.apply_pitch(env.synth, s);
                }
            }
            Cmd::Ring(on) => {
                for &s in slots {
                    if let Some(v) = self.voice(s) {
                        match (on, self.voice(1 - s)) {
                            (true, Some(p)) => env.synth.set_ring_partner(v, p),
                            (true, None) => {}
                            (false, _) => env.synth.clear_ring_partner(v),
                        }
                    }
                }
            }
            Cmd::Route(on) => {
                for &s in slots {
                    if let Some(v) = self.voice(s) {
                        env.synth.set_voice_filter_route(v, on as i32);
                    }
                }
            }
            Cmd::Arp(n) => {
                let mut offs = [0i32; 3];
                for k in (0..n as usize).rev() {
                    offs[k] = self.pop();
                }
                for &s in slots {
                    if let Some(v) = self.voice(s) {
                        let base = self.pitch[s];
                        env.synth.set_arp_note(v, 0, base);
                        for (k, off) in offs.iter().enumerate().take(n as usize) {
                            let note = base.saturating_add(*off).clamp(pitch::ONA_MIN, pitch::ONA_MAX);
                            env.synth.set_arp_note(v, k as i32 + 1, note);
                        }
                        env.synth.set_arp_rate(v, self.arp_ms);
                        let vo = env.synth.voice_mut(v as usize);
                        vo.arp_step = 0;
                        vo.arp_step_counter = 0;
                        env.synth.arp_on(v, n as i32 + 1);
                    }
                }
            }
            Cmd::ArpOff => {
                for &s in slots {
                    if let Some(v) = self.voice(s) {
                        env.synth.voice_mut(v as usize).arp_active = false;
                    }
                    self.apply_pitch(env.synth, s);
                }
            }
            Cmd::ArpRate => {
                self.arp_ms = self.pop().clamp(1, 10_000);
                for &s in slots {
                    if let Some(v) = self.voice(s) {
                        env.synth.set_arp_rate(v, self.arp_ms);
                    }
                }
            }
            Cmd::Filter(mode) => {
                let res = self.pop();
                let cut = self.pop();
                env.synth.set_filter_mode(mode);
                env.synth.set_filter_cutoff(cut);
                env.synth.set_filter_resonance(res);
            }
            Cmd::SongSelect(i) => self.song = Some(i),
            Cmd::SongPlay => {
                let order = self.pop();
                let song = self.song;
                queue(env, SongCmd::Play { song, order });
            }
            Cmd::SongStop => queue(env, SongCmd::Stop),
            Cmd::Tempo => {
                let n = self.pop();
                queue(env, SongCmd::Tempo(n));
            }
            Cmd::Mute(on) => {
                let ch = self.pop();
                queue(env, SongCmd::Mute(ch, on));
            }
            Cmd::Jump => {
                let o = self.pop();
                queue(env, SongCmd::Jump(o));
            }
        }
    }
}
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test --manifest-path v3/Cargo.toml -p acid-sound vm`
Expected: 21 passed.

- [ ] **Step 5: Commit**

```bash
git add v3/crates/acid-sound
git commit -m "acid-sound: VM with tick waits, instruction budget and synth commands

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 6: The `.trk` song model and parser

**Files:**
- Create: `v3/crates/acid-sound/src/song.rs`
- Modify: `v3/crates/acid-sound/src/lib.rs` (add `pub mod song;`)

**Interfaces:**
- Consumes: `pitch::{parse_note, note_name}`
- Produces:
  - Constants: `song::{CHANNELS = 4, MAX_ROWS = 64, MAX_PATTERN = 0x7F, MAX_INSTRUMENT = 0x3F, NOTE_NONE = 0, NOTE_OFF = 0xFF, COMMANDS = b"123489AF", WAVES}`
  - `Row { note: u8, inst: u8, cmd: u8, param: u8, note2: u8 }`. `cmd` is 0 or the ASCII letter.
  - `Voice2` with the variants `Off`, `Detune(i32)`, `Octave`, `Fifth`, `Ring`
  - `BuiltIn { wave, adsr: [i32; 4], duty, pwm, vib: (i32, i32), arp: Vec<i32>, filter: Option<(i32, i32, i32)>, voice2 }`, which implements `Default`
  - `Kind` with the variants `BuiltIn(BuiltIn)` and `Script { path: String, block: String }`
  - `Instrument { name, kind }`
  - `OrderEntry { pattern: u8, transpose: i8 }`
  - `OrderList { entries: Vec<OrderEntry>, loop_to: usize }`
  - `Song { title, speed: u8, sfx_donor: u8, instruments: BTreeMap<u8, Instrument>, orders: [OrderList; 4], patterns: BTreeMap<u8, Vec<Row>> }`
  - `SongError { line: u32, message }`, whose `Display` prints `"LINE: message"`
  - `parse(&str) -> Result<Song, SongError>`

- [ ] **Step 1: Write the failing tests**

Create `v3/crates/acid-sound/src/song.rs` with:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;

    /// Line numbers: 5 instrument, 6-9 orders, 11 pattern, 12-13 rows.
    pub(crate) const MIN: &str = "acid-track 1\ntitle t\nspeed 6\nsfx-donor 4\ninstrument 01 \"Lead\"  wave saw  adsr 0 8 70 20  duty 50\norder 1  00 loop 0\norder 2  00 loop 0\norder 3  00 loop 0\norder 4  00 loop 0\n\npattern 00 2\nC-4 01 . .. ...\n=== .. 4 22 E-4\n";

    fn err(text: &str) -> String {
        parse(text).unwrap_err().to_string()
    }

    #[test]
    fn parses_the_minimal_song() {
        let s = parse(MIN).unwrap();
        assert_eq!((s.title.as_str(), s.speed, s.sfx_donor), ("t", 6, 4));
        let lead = &s.instruments[&1];
        assert_eq!(lead.name, "Lead");
        let Kind::BuiltIn(b) = &lead.kind else { panic!("built-in") };
        assert_eq!((b.wave, b.adsr, b.duty), (1, [0, 8, 70, 20], 50));
        assert_eq!(s.orders[0].entries, vec![OrderEntry { pattern: 0, transpose: 0 }]);
        assert_eq!(
            s.patterns[&0],
            vec![
                Row { note: 40, inst: 1, cmd: 0, param: 0, note2: 0 },
                Row { note: NOTE_OFF, inst: 0, cmd: b'4', param: 0x22, note2: 44 },
            ]
        );
    }

    #[test]
    fn every_instrument_field() {
        let text = MIN.replace(
            "instrument 01 \"Lead\"  wave saw  adsr 0 8 70 20  duty 50\n",
            "instrument 01 \"Pad Two\"  wave tri  adsr 1 2 3 4  duty 30  pwm 2  vib 4 3  arp 4 7  filter bp 100 5  voice2 detune -6\ninstrument 02 \"S\"  script \"Home/s.snd\" bass\n",
        );
        let s = parse(&text).unwrap();
        assert_eq!(s.instruments[&1].name, "Pad Two");
        assert_eq!(
            s.instruments[&1].kind,
            Kind::BuiltIn(BuiltIn {
                wave: 2, adsr: [1, 2, 3, 4], duty: 30, pwm: 2, vib: (4, 3), arp: alloc::vec![4, 7],
                filter: Some((2, 100, 5)), voice2: Voice2::Detune(-6),
            })
        );
        assert_eq!(s.instruments[&2].kind, Kind::Script { path: "Home/s.snd".into(), block: "bass".into() });
    }

    #[test]
    fn order_entries_transpose() {
        let s = parse(&MIN.replace("order 1  00 loop 0", "order 1  00 00+12 00-3 loop 2")).unwrap();
        let t: Vec<i8> = s.orders[0].entries.iter().map(|e| e.transpose).collect();
        assert_eq!(t, [0, 12, -3]);
        assert_eq!(s.orders[0].loop_to, 2);
    }

    #[test]
    fn errors_name_the_line() {
        assert_eq!(err("hello"), "1: not an acid-track file");
        assert_eq!(err("acid-track 2"), "1: unsupported version 2");
        assert_eq!(err(&MIN.replace("speed 6", "speed 0")), "3: speed must be 1 to 31");
        assert_eq!(err(&MIN.replace("sfx-donor 4", "sfx-donor 5")), "4: sfx-donor must be 1 to 4");
        assert_eq!(err(&MIN.replace("duty 50", "duty 50  wobble 3")), "5: unknown instrument field 'wobble'");
        assert_eq!(err(&MIN.replace("order 1  00 loop 0", "order 1  00 loop 1")), "6: loop 1 is past the end of order 1");
        assert_eq!(err(&MIN.replace("order 2  00", "order 2  05")), "7: order 2 uses missing pattern 05");
        assert_eq!(err(&MIN.replace("order 4  00 loop 0\n", "")), "12: no order for channel 4");
        assert_eq!(err(&MIN.replace("pattern 00 2", "pattern 00 3")), "11: pattern 00 has 2 rows, expected 3");
        assert_eq!(err(&MIN.replace("C-4 01", "C-9 01")), "12: bad note 'C-9'");
        assert_eq!(err(&MIN.replace("=== .. 4", "=== .. S")), "13: command S is reserved");
        assert_eq!(err(&MIN.replace("22 E-4", "22 ===")), "13: bad note '==='");
        assert_eq!(err(&alloc::format!("{MIN}tempo 4\n")), "14: unknown line 'tempo'");
    }
}
```

Add `pub mod song;` to `lib.rs`.

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test --manifest-path v3/Cargo.toml -p acid-sound song`
Expected: compile errors, `cannot find function parse`.

- [ ] **Step 3: Implement the model and the parser**

Put this above the tests in `song.rs`:

```rust
//! The .trk song format: a versioned text file in the house style of
//! .spr. `parse` is strict and is the one reader of record (games and the
//! tracker both use it); `write` emits the canonical form.

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt;

use crate::pitch::{note_name, parse_note};

pub const CHANNELS: usize = 4;
pub const MAX_ROWS: usize = 64;
pub const MAX_PATTERN: u8 = 0x7F;
pub const MAX_INSTRUMENT: u8 = 0x3F;
pub const NOTE_NONE: u8 = 0;
pub const NOTE_OFF: u8 = 0xFF;
/// Pattern command letters. `S` is reserved for script calls.
pub const COMMANDS: &[u8] = b"123489AF";
/// Waveform names, in synth order.
pub const WAVES: [&str; 4] = ["pulse", "saw", "tri", "noise"];

/// One row of one channel. `note`/`note2` are ona (or NOTE_NONE/NOTE_OFF);
/// `cmd` is 0 or the command's ASCII letter.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Row {
    pub note: u8,
    pub inst: u8,
    pub cmd: u8,
    pub param: u8,
    pub note2: u8,
}

/// What a built-in instrument does with the channel's second voice when
/// the row has no second note.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Voice2 {
    Off,
    /// Fine steps (1/64 semitone).
    Detune(i32),
    Octave,
    Fifth,
    Ring,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuiltIn {
    pub wave: i32,
    pub adsr: [i32; 4],
    pub duty: i32,
    /// Duty change per tick, bouncing between 10 and 90.
    pub pwm: i32,
    /// Depth and speed, as pattern command 4.
    pub vib: (i32, i32),
    /// Semitone offsets stepped one per tick after the base note.
    pub arp: Vec<i32>,
    /// Mode mask, cutoff, resonance.
    pub filter: Option<(i32, i32, i32)>,
    pub voice2: Voice2,
}

impl Default for BuiltIn {
    fn default() -> Self {
        Self { wave: 0, adsr: [2, 40, 80, 40], duty: 50, pwm: 0, vib: (0, 0), arp: Vec::new(), filter: None, voice2: Voice2::Off }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    BuiltIn(BuiltIn),
    /// An instrument block in a .snd file; the path is fsroot-relative.
    Script { path: String, block: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Instrument {
    pub name: String,
    pub kind: Kind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OrderEntry {
    pub pattern: u8,
    pub transpose: i8,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OrderList {
    pub entries: Vec<OrderEntry>,
    pub loop_to: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Song {
    pub title: String,
    pub speed: u8,
    pub sfx_donor: u8,
    pub instruments: BTreeMap<u8, Instrument>,
    pub orders: [OrderList; CHANNELS],
    pub patterns: BTreeMap<u8, Vec<Row>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SongError {
    pub line: u32,
    pub message: String,
}

impl SongError {
    fn new(line: usize, message: impl Into<String>) -> Self {
        Self { line: line as u32, message: message.into() }
    }
}

impl fmt::Display for SongError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.line, self.message)
    }
}

fn hex2(s: &str) -> Option<u8> {
    if s.len() != 2 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    u8::from_str_radix(s, 16).ok()
}

struct Field {
    text: String,
    quoted: bool,
}

/// Splits on whitespace; "..." is one field (quotes removed).
fn fields(line: &str) -> Result<Vec<Field>, String> {
    let mut out = Vec::new();
    let mut rest = line.trim_start();
    while !rest.is_empty() {
        if let Some(r) = rest.strip_prefix('"') {
            let end = r.find('"').ok_or("a quote has no closing '\"'")?;
            out.push(Field { text: r[..end].to_string(), quoted: true });
            rest = r[end + 1..].trim_start();
        } else {
            let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
            out.push(Field { text: rest[..end].to_string(), quoted: false });
            rest = rest[end..].trim_start();
        }
    }
    Ok(out)
}

/// Walks an instrument line's fields.
struct Cur<'a> {
    f: &'a [Field],
    i: usize,
}

impl<'a> Cur<'a> {
    fn word(&mut self) -> Option<&'a str> {
        let x = self.f.get(self.i).filter(|x| !x.quoted)?;
        self.i += 1;
        Some(&x.text)
    }

    fn int(&mut self, after: &str) -> Result<i32, String> {
        let n = self
            .f
            .get(self.i)
            .filter(|x| !x.quoted)
            .and_then(|x| x.text.parse().ok())
            .ok_or_else(|| format!("expected a number after '{after}'"))?;
        self.i += 1;
        Ok(n)
    }

    fn next_is_int(&self) -> bool {
        self.f.get(self.i).is_some_and(|x| !x.quoted && x.text.parse::<i32>().is_ok())
    }

    fn quoted(&mut self, what: &str) -> Result<String, String> {
        match self.f.get(self.i) {
            Some(x) if x.quoted => {
                self.i += 1;
                Ok(x.text.clone())
            }
            _ => Err(format!("expected {what} in quotes")),
        }
    }

    fn done(&self) -> bool {
        self.i >= self.f.len()
    }
}

fn parse_instrument(f: &[Field]) -> Result<(u8, Instrument), String> {
    let mut c = Cur { f, i: 1 };
    let num = c.word().and_then(hex2).filter(|n| (1..=MAX_INSTRUMENT).contains(n)).ok_or("bad instrument number")?;
    let name = c.quoted("the instrument's name")?;
    if c.f.get(c.i).is_some_and(|x| !x.quoted && x.text == "script") {
        c.i += 1;
        let path = c.quoted("the script's path")?;
        let block = c.word().ok_or("expected the script's instrument name")?.to_string();
        if !c.done() {
            return Err("a script instrument has no other fields".into());
        }
        return Ok((num, Instrument { name, kind: Kind::Script { path, block } }));
    }
    let mut b = BuiltIn::default();
    while !c.done() {
        let key = c.word().ok_or("expected an instrument field")?;
        match key {
            "wave" => {
                let w = c.word().unwrap_or("");
                b.wave = WAVES.iter().position(|x| *x == w).ok_or_else(|| format!("unknown waveform '{w}'"))? as i32;
            }
            "adsr" => {
                for k in 0..4 {
                    b.adsr[k] = c.int("adsr")?;
                }
            }
            "duty" => b.duty = c.int("duty")?.clamp(1, 99),
            "pwm" => b.pwm = c.int("pwm")?,
            "vib" => b.vib = (c.int("vib")?, c.int("vib")?),
            "arp" => {
                b.arp.clear();
                while b.arp.len() < 3 && c.next_is_int() {
                    b.arp.push(c.int("arp")?);
                }
                if b.arp.is_empty() {
                    return Err("expected a number after 'arp'".into());
                }
            }
            "filter" => {
                let m = c.word().unwrap_or("");
                let mode = match m {
                    "lp" => 1,
                    "bp" => 2,
                    "hp" => 4,
                    _ => return Err(format!("unknown filter mode '{m}'")),
                };
                let (cut, res) = (c.int("filter")?, c.int("filter")?);
                if !(0..=255).contains(&cut) || !(0..=15).contains(&res) {
                    return Err("filter cutoff is 0 to 255 and resonance 0 to 15".into());
                }
                b.filter = Some((mode, cut, res));
            }
            "voice2" => {
                b.voice2 = match c.word().unwrap_or("") {
                    "off" => Voice2::Off,
                    "detune" => Voice2::Detune(c.int("detune")?),
                    "octave" => Voice2::Octave,
                    "fifth" => Voice2::Fifth,
                    "ring" => Voice2::Ring,
                    m => return Err(format!("unknown voice2 mode '{m}'")),
                };
            }
            "script" => return Err("'script' must come right after the name".into()),
            k => return Err(format!("unknown instrument field '{k}'")),
        }
    }
    Ok((num, Instrument { name, kind: Kind::BuiltIn(b) }))
}

fn parse_entry(s: &str) -> Option<OrderEntry> {
    let (p, transpose) = match s.find(['+', '-']) {
        Some(k) => (&s[..k], s[k..].parse::<i8>().ok()?),
        None => (s, 0),
    };
    Some(OrderEntry { pattern: hex2(p).filter(|n| *n <= MAX_PATTERN)?, transpose })
}

fn parse_order(f: &[Field]) -> Result<(usize, OrderList), String> {
    let ch: usize = f
        .get(1)
        .and_then(|x| x.text.parse().ok())
        .filter(|c| (1..=CHANNELS).contains(c))
        .ok_or("order channel must be 1 to 4")?;
    let mut entries = Vec::new();
    let mut i = 2;
    while i < f.len() && f[i].text != "loop" {
        entries.push(parse_entry(&f[i].text).ok_or_else(|| format!("bad order entry '{}'", f[i].text))?);
        i += 1;
    }
    if entries.is_empty() {
        return Err(format!("order {ch} is empty"));
    }
    if i + 2 != f.len() {
        return Err(format!("order {ch} must end with 'loop N'"));
    }
    let loop_to: usize = f[i + 1].text.parse().map_err(|_| format!("order {ch} must end with 'loop N'"))?;
    if loop_to >= entries.len() {
        return Err(format!("loop {loop_to} is past the end of order {ch}"));
    }
    Ok((ch - 1, OrderList { entries, loop_to }))
}

fn parse_row(line: &str) -> Result<Row, String> {
    let f: Vec<&str> = line.split_whitespace().collect();
    if f.len() != 5 {
        return Err("expected a row like 'C-4 01 . .. ...'".into());
    }
    let note = match f[0] {
        "..." => NOTE_NONE,
        "===" => NOTE_OFF,
        n => parse_note(n).ok_or_else(|| format!("bad note '{n}'"))? as u8,
    };
    let inst = match f[1] {
        ".." => 0,
        s => hex2(s).filter(|n| (1..=MAX_INSTRUMENT).contains(n)).ok_or_else(|| format!("bad instrument '{s}'"))?,
    };
    let cmd = match f[2] {
        "." => 0,
        "S" => return Err("command S is reserved".into()),
        s if s.len() == 1 && COMMANDS.contains(&s.as_bytes()[0]) => s.as_bytes()[0],
        s => return Err(format!("unknown command '{s}'")),
    };
    let param = match f[3] {
        ".." => 0,
        s => hex2(s).ok_or_else(|| format!("bad parameter '{s}'"))?,
    };
    let note2 = match f[4] {
        "..." => NOTE_NONE,
        n => parse_note(n).ok_or_else(|| format!("bad note '{n}'"))? as u8,
    };
    Ok(Row { note, inst, cmd, param, note2 })
}

/// A single integer field `f[i]`, the last on its line, in lo..=hi.
fn last_int(f: &[Field], i: usize, lo: i32, hi: i32) -> Option<i32> {
    if f.len() != i + 1 {
        return None;
    }
    f[i].text.parse().ok().filter(|n| (lo..=hi).contains(n))
}

pub fn parse(text: &str) -> Result<Song, SongError> {
    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0;
    while i < lines.len() && lines[i].trim().is_empty() {
        i += 1;
    }
    match lines.get(i).map(|l| l.trim()).unwrap_or("").strip_prefix("acid-track ") {
        Some("1") => {}
        Some(v) => return Err(SongError::new(i + 1, format!("unsupported version {}", v.trim()))),
        None => return Err(SongError::new(i + 1, "not an acid-track file")),
    }
    i += 1;
    let mut song = Song {
        title: String::new(),
        speed: 6,
        sfx_donor: 4,
        instruments: BTreeMap::new(),
        orders: Default::default(),
        patterns: BTreeMap::new(),
    };
    // The line each channel's order came from; 0 = none yet.
    let mut order_line = [0usize; CHANNELS];
    while i < lines.len() {
        let n = i + 1;
        let line = lines[i].trim();
        i += 1;
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let err = |m: String| SongError::new(n, m);
        let f = fields(line).map_err(err)?;
        match f[0].text.as_str() {
            "title" => song.title = line["title".len()..].trim().to_string(),
            "speed" => song.speed = last_int(&f, 1, 1, 31).ok_or_else(|| err("speed must be 1 to 31".into()))? as u8,
            "sfx-donor" => song.sfx_donor = last_int(&f, 1, 1, 4).ok_or_else(|| err("sfx-donor must be 1 to 4".into()))? as u8,
            "instrument" => {
                let (num, inst) = parse_instrument(&f).map_err(err)?;
                if song.instruments.insert(num, inst).is_some() {
                    return Err(err(format!("instrument {num:02X} defined twice")));
                }
            }
            "order" => {
                let (ch, list) = parse_order(&f).map_err(err)?;
                if order_line[ch] != 0 {
                    return Err(err(format!("order {} defined twice", ch + 1)));
                }
                order_line[ch] = n;
                song.orders[ch] = list;
            }
            "pattern" => {
                if f.len() != 3 {
                    return Err(err("expected 'pattern NN LENGTH'".into()));
                }
                let num = hex2(&f[1].text)
                    .filter(|p| *p <= MAX_PATTERN)
                    .ok_or_else(|| err(format!("bad pattern number '{}'", f[1].text)))?;
                let len: usize = f[2]
                    .text
                    .parse()
                    .ok()
                    .filter(|l| (1..=MAX_ROWS).contains(l))
                    .ok_or_else(|| err(format!("pattern length must be 1 to {MAX_ROWS}")))?;
                let mut rows = Vec::with_capacity(len);
                while rows.len() < len {
                    let Some(row_line) = lines.get(i) else {
                        return Err(err(format!("pattern {num:02X} has {} rows, expected {len}", rows.len())));
                    };
                    rows.push(parse_row(row_line).map_err(|m| SongError::new(i + 1, m))?);
                    i += 1;
                }
                if song.patterns.insert(num, rows).is_some() {
                    return Err(err(format!("pattern {num:02X} defined twice")));
                }
            }
            w => return Err(err(format!("unknown line '{w}'"))),
        }
    }
    let last = lines.len().max(1);
    for ch in 0..CHANNELS {
        if order_line[ch] == 0 {
            return Err(SongError::new(last, format!("no order for channel {}", ch + 1)));
        }
        for e in &song.orders[ch].entries {
            if !song.patterns.contains_key(&e.pattern) {
                return Err(SongError::new(order_line[ch], format!("order {} uses missing pattern {:02X}", ch + 1, e.pattern)));
            }
        }
    }
    Ok(song)
}
```

`note_name` gets used by `write` in Task 7. Until then the compiler may warn that it's an unused import. That's expected.

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test --manifest-path v3/Cargo.toml -p acid-sound song`
Expected: 4 passed.

- [ ] **Step 5: Commit**

```bash
git add v3/crates/acid-sound
git commit -m "acid-sound: .trk song model and strict parser

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 7: The `.trk` writer

**Files:**
- Modify: `v3/crates/acid-sound/src/song.rs`

**Interfaces:**
- Produces:
  - `song::write(&Song) -> String`, which writes the canonical text
  - `song::row_text(&Row) -> String`, for example `"C-4 01 4 22 E-4"`. The tracker app uses it in plan 2.
- **The canonical form, exactly:**
  - Header lines: `acid-track 1`, `title …`, `speed N`, `sfx-donor N`.
  - Instrument lines are written in number order. Each is `instrument NN "name"` followed by fields, with every field prefixed by two spaces.
  - A built-in always writes `wave`, `adsr` and `duty`. It writes `pwm`, `vib`, `arp`, `filter` and `voice2` only when they aren't the default.
  - Order lines are `order N  E E loop L`.
  - Each pattern is preceded by a blank line, then `pattern NN LEN` and its rows.

- [ ] **Step 1: Write the failing tests**

Add these inside `song.rs`'s `mod tests`:

```rust
    const ROUND: &str = "acid-track 1
title Round Trip
speed 6
sfx-donor 4
instrument 01 \"Lead\"  wave saw  adsr 0 8 70 20  duty 50  pwm 2  vib 4 2  arp 4 7  filter lp 120 4  voice2 detune 6
instrument 02 \"Bass\"  script \"Home/sounds/demo.snd\" bass
instrument 03 \"Hat\"  wave noise  adsr 0 2 0 2  duty 50  voice2 octave
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
";

    #[test]
    fn canonical_text_round_trips_byte_for_byte() {
        assert_eq!(write(&parse(ROUND).unwrap()), ROUND);
        assert_eq!(write(&parse(MIN).unwrap()), MIN);
    }

    #[test]
    fn a_written_song_parses_back_the_same() {
        let mut s = parse(ROUND).unwrap();
        s.title = "Edited".into();
        s.patterns.get_mut(&1).unwrap()[1] = Row { note: 88, inst: 1, cmd: b'1', param: 0, note2: 1 };
        assert_eq!(parse(&write(&s)).unwrap(), s);
    }

    #[test]
    fn row_text_shows_empty_fields_as_dots() {
        assert_eq!(row_text(&Row::default()), "... .. . .. ...");
        assert_eq!(row_text(&Row { note: 40, inst: 0x1F, cmd: b'A', param: 0, note2: 52 }), "C-4 1F A 00 C-5");
    }
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test --manifest-path v3/Cargo.toml -p acid-sound song`
Expected: compile errors, `cannot find function write`.

- [ ] **Step 3: Implement the writer**

Add this to `song.rs`, above the tests:

```rust
/// One row as the file writes it, e.g. "C-4 01 4 22 E-4".
pub fn row_text(r: &Row) -> String {
    let note = |n: u8| match n {
        NOTE_NONE => String::from("..."),
        NOTE_OFF => String::from("==="),
        n => note_name(n as i32),
    };
    let inst = if r.inst == 0 { String::from("..") } else { format!("{:02X}", r.inst) };
    let cmd = if r.cmd == 0 { '.' } else { r.cmd as char };
    let param = if r.cmd == 0 && r.param == 0 { String::from("..") } else { format!("{:02X}", r.param) };
    format!("{} {} {} {} {}", note(r.note), inst, cmd, param, note(r.note2))
}

/// The canonical text: parse(write(s)) == s, and write(parse(t)) == t
/// for any canonical t.
pub fn write(song: &Song) -> String {
    let mut s = String::new();
    s += "acid-track 1\n";
    s += &format!("title {}\n", song.title);
    s += &format!("speed {}\n", song.speed);
    s += &format!("sfx-donor {}\n", song.sfx_donor);
    for (num, inst) in &song.instruments {
        s += &format!("instrument {num:02X} \"{}\"", inst.name.replace('"', "'"));
        match &inst.kind {
            Kind::Script { path, block } => s += &format!("  script \"{path}\" {block}"),
            Kind::BuiltIn(b) => {
                s += &format!(
                    "  wave {}  adsr {} {} {} {}  duty {}",
                    WAVES[b.wave.clamp(0, 3) as usize], b.adsr[0], b.adsr[1], b.adsr[2], b.adsr[3], b.duty
                );
                if b.pwm != 0 {
                    s += &format!("  pwm {}", b.pwm);
                }
                if b.vib != (0, 0) {
                    s += &format!("  vib {} {}", b.vib.0, b.vib.1);
                }
                if !b.arp.is_empty() {
                    s += "  arp";
                    for a in &b.arp {
                        s += &format!(" {a}");
                    }
                }
                if let Some((m, c, r)) = b.filter {
                    let mode = match m {
                        2 => "bp",
                        4 => "hp",
                        _ => "lp",
                    };
                    s += &format!("  filter {mode} {c} {r}");
                }
                match b.voice2 {
                    Voice2::Off => {}
                    Voice2::Detune(n) => s += &format!("  voice2 detune {n}"),
                    Voice2::Octave => s += "  voice2 octave",
                    Voice2::Fifth => s += "  voice2 fifth",
                    Voice2::Ring => s += "  voice2 ring",
                }
            }
        }
        s.push('\n');
    }
    for (ch, o) in song.orders.iter().enumerate() {
        s += &format!("order {} ", ch + 1);
        for e in &o.entries {
            s += &format!(" {:02X}", e.pattern);
            if e.transpose != 0 {
                s += &format!("{:+}", e.transpose);
            }
        }
        s += &format!(" loop {}\n", o.loop_to);
    }
    for (num, rows) in &song.patterns {
        s += &format!("\npattern {num:02X} {}\n", rows.len());
        for r in rows {
            s += &row_text(r);
            s.push('\n');
        }
    }
    s
}
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test --manifest-path v3/Cargo.toml -p acid-sound song`
Expected: 7 passed.

- [ ] **Step 5: Commit**

```bash
git add v3/crates/acid-sound
git commit -m "acid-sound: canonical .trk writer

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 8: The song player

**Files:**
- Create: `v3/crates/acid-sound/src/player.rs`
- Modify: `v3/crates/acid-sound/src/lib.rs` (add `pub mod player;`)

**Interfaces:**
- Consumes:
  - `song::*` (Task 6)
  - `vm::{Clock, Env, Instance, SongCmd, State}` (Task 5)
  - `pitch::{fine_pos, increment_at, FINE_MAX, FINE_STEPS, ONA_MIN, ONA_MAX}`
- Produces:
  - `player::LoadedSong { song: Song, scripts: BTreeMap<u8, (Arc<Program>, usize)> }`, which implements `Debug`, with `LoadedSong::plain(Song)`
  - `player::Player`, with the public field `playing: bool` and these methods:
    - `new(Arc<LoadedSong>, order: usize, row: usize)` and `preview(Arc<LoadedSong>)`
    - `song() -> &Arc<LoadedSong>`, `clock() -> Clock`, `position() -> (i32, i32, i32)`
    - `tick(&mut Synth, &mut Vec<SongCmd>)`
    - `trigger(&mut Synth, ch: usize, ona: i32, note2: Option<i32>, inst: u8)`, `note_off(&mut Synth, ch)`
    - `stop(&mut Synth)`, `set_muted(&mut Synth, ch, on)`, `muted(ch) -> bool`
    - `set_speed(i32)`, `jump(i32)`, `replace_song(Arc<LoadedSong>)`
    - `lend(u8)`, `take_back(u8)`, `borrowed() -> u8`, `donor_voice() -> u8`
- Behaviour:
  - Each tick does four things in order:
    1. If playing and on tick 0, read a row on every channel.
    2. Run the built-in effects.
    3. Run the script instances.
    4. If playing, count the tick, and advance a row every `speed` ticks.
  - `position()` returns channel 1's `(order_pos, row, tick)`.

- [ ] **Step 1: Write the failing tests**

Create `v3/crates/acid-sound/src/player.rs` with:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::compile::compile;
    use crate::song::parse;
    use alloc::format;
    use alloc::string::String;
    use acid_synth::{EnvStage, Waveform, ONA_PHASE_INCREMENT};

    const LEAD: &str = "instrument 01 \"Lead\"  wave saw  adsr 0 0 100 0  duty 50";

    /// Channel 1 plays `rows` (pattern 00); channels 2-4 sit on an empty pattern.
    fn one_channel(inst: &str, rows: &[&str]) -> String {
        let mut s = String::from("acid-track 1\ntitle t\nspeed 2\nsfx-donor 4\n");
        s += inst;
        s += "\norder 1  00 loop 0\norder 2  01 loop 0\norder 3  01 loop 0\norder 4  01 loop 0\n";
        s += &format!("\npattern 00 {}\n", rows.len());
        for r in rows {
            s += r;
            s += "\n";
        }
        s += "\npattern 01 1\n... .. . .. ...\n";
        s
    }

    fn player(text: &str) -> (Player, Synth) {
        (Player::new(Arc::new(LoadedSong::plain(parse(text).unwrap())), 0, 0), Synth::new())
    }

    fn run(p: &mut Player, s: &mut Synth, n: usize) -> Vec<SongCmd> {
        let mut cmds = Vec::new();
        for _ in 0..n {
            p.tick(s, &mut cmds);
        }
        cmds
    }

    const EMPTY: &str = "... .. . .. ...";

    #[test]
    fn the_first_tick_starts_row_zero() {
        let (mut p, mut s) = player(&one_channel(LEAD, &["C-4 01 . .. ...", EMPTY]));
        run(&mut p, &mut s, 1);
        assert_eq!(s.voice(0).waveform, Waveform::Saw);
        assert_eq!(s.voice(0).envelope_stage, EnvStage::Attack);
        assert_eq!(s.voice(0).phase_increment, ONA_PHASE_INCREMENT[39]);
        assert_eq!(s.voice(1).envelope_stage, EnvStage::Off, "voice2 off and no second note");
        assert_eq!(p.position(), (0, 0, 1));
    }

    #[test]
    fn each_row_lasts_speed_ticks() {
        let (mut p, mut s) = player(&one_channel(LEAD, &["C-4 01 . .. ...", EMPTY, "=== .. . .. ...", EMPTY]));
        run(&mut p, &mut s, 2);
        assert_eq!(p.position(), (0, 1, 0));
        run(&mut p, &mut s, 2);
        assert_ne!(s.voice(0).envelope_stage, EnvStage::Release);
        run(&mut p, &mut s, 1);
        assert_eq!(s.voice(0).envelope_stage, EnvStage::Release, "row 2 is a note-off");
    }

    #[test]
    fn order_lists_transpose_and_loop() {
        let text = one_channel(LEAD, &["C-4 01 . .. ..."]).replace("order 1  00 loop 0", "order 1  00 00+12 loop 1");
        let (mut p, mut s) = player(&text);
        run(&mut p, &mut s, 1);
        assert_eq!(s.voice(0).phase_increment, ONA_PHASE_INCREMENT[39]);
        run(&mut p, &mut s, 2);
        assert_eq!(p.position(), (1, 0, 1));
        assert_eq!(s.voice(0).phase_increment, ONA_PHASE_INCREMENT[51]);
        run(&mut p, &mut s, 2);
        assert_eq!(p.position(), (1, 0, 1), "wraps to the loop point");
    }

    #[test]
    fn slide_moves_the_pitch_every_tick() {
        let (mut p, mut s) = player(&one_channel(LEAD, &["C-4 01 1 08 ..."]));
        run(&mut p, &mut s, 1);
        assert_eq!(s.voice(0).phase_increment, increment_at(fine_pos(40) + 8));
        run(&mut p, &mut s, 1);
        assert_eq!(s.voice(0).phase_increment, increment_at(fine_pos(40) + 16));
    }

    #[test]
    fn vibrato_command_wobbles_the_pitch() {
        let (mut p, mut s) = player(&one_channel(LEAD, &["C-4 01 4 F4 ..."]));
        run(&mut p, &mut s, 1);
        // Phase 4 of the triangle is +4; depth 15 * 4 / 4 = 15 fine steps.
        assert_eq!(s.voice(0).phase_increment, increment_at(fine_pos(40) + 15));
    }

    #[test]
    fn glide_heads_for_the_new_note_without_retriggering() {
        let (mut p, mut s) = player(&one_channel(LEAD, &["C-4 01 . .. ...", "E-4 .. 3 10 ..."]));
        run(&mut p, &mut s, 3);
        assert_eq!(s.voice(0).phase_increment, increment_at(fine_pos(40) + 16));
    }

    #[test]
    fn speed_command_changes_the_row_length() {
        let (mut p, mut s) = player(&one_channel(LEAD, &["C-4 01 F 03 ...", EMPTY]));
        run(&mut p, &mut s, 2);
        assert_eq!(p.position(), (0, 0, 2));
        run(&mut p, &mut s, 1);
        assert_eq!(p.position(), (0, 1, 0));
    }

    #[test]
    fn waveform_command() {
        let (mut p, mut s) = player(&one_channel(LEAD, &["C-4 01 8 03 ..."]));
        run(&mut p, &mut s, 1);
        assert_eq!(s.voice(0).waveform, Waveform::Noise);
    }

    #[test]
    fn voice2_detune_doubles_the_note() {
        let (mut p, mut s) = player(&one_channel(&format!("{LEAD}  voice2 detune 6"), &["C-4 01 . .. ..."]));
        run(&mut p, &mut s, 1);
        assert_eq!(s.voice(1).envelope_stage, EnvStage::Attack);
        assert_eq!(s.voice(1).phase_increment, increment_at(fine_pos(40) + 6));
    }

    #[test]
    fn the_second_note_column_plays_on_voice_two() {
        let (mut p, mut s) = player(&one_channel(LEAD, &["C-4 01 . .. E-4"]));
        run(&mut p, &mut s, 1);
        assert_eq!(s.voice(1).envelope_stage, EnvStage::Attack);
        assert_eq!(s.voice(1).phase_increment, ONA_PHASE_INCREMENT[43]);
    }

    #[test]
    fn built_in_arp_steps_each_tick() {
        let (mut p, mut s) = player(&one_channel(&format!("{LEAD}  arp 4 7"), &["C-4 01 . .. ...", EMPTY]));
        let mut seen = Vec::new();
        for _ in 0..4 {
            run(&mut p, &mut s, 1);
            seen.push(s.voice(0).phase_increment);
        }
        assert_eq!(seen, [39, 43, 46, 39].map(|i| ONA_PHASE_INCREMENT[i]));
    }

    #[test]
    fn a_filter_field_routes_the_voice() {
        let (mut p, mut s) = player(&one_channel(&format!("{LEAD}  filter lp 40 6"), &["C-4 01 . .. ..."]));
        run(&mut p, &mut s, 1);
        assert!(s.voice(0).filter_route);
    }

    const BLIP: &str = "instrument blip\nwave tri\ngate on\nloop\npitch +1\nwait 1\nend\non release\ngate off\nend";

    fn scripted(src: &str, rows: &[&str]) -> (Player, Synth) {
        let text = one_channel("instrument 01 \"S\"  script \"x.snd\" blip", rows);
        let mut ls = LoadedSong::plain(parse(&text).unwrap());
        ls.scripts.insert(1, (Arc::new(compile(src).unwrap()), 0));
        (Player::new(Arc::new(ls), 0, 0), Synth::new())
    }

    #[test]
    fn a_script_instrument_plays_and_releases() {
        let (mut p, mut s) = scripted(BLIP, &["C-4 01 . .. ...", EMPTY, "=== .. . .. ...", EMPTY]);
        run(&mut p, &mut s, 1);
        assert_eq!(s.voice(0).waveform, Waveform::Triangle);
        assert_eq!(s.voice(0).envelope_stage, EnvStage::Attack);
        assert_eq!(s.voice(0).phase_increment, ONA_PHASE_INCREMENT[40]);
        run(&mut p, &mut s, 1);
        assert_eq!(s.voice(0).phase_increment, ONA_PHASE_INCREMENT[41]);
        run(&mut p, &mut s, 3);
        assert_eq!(s.voice(0).envelope_stage, EnvStage::Release);
    }

    #[test]
    fn a_second_note_runs_a_second_instance_on_voice_two() {
        let (mut p, mut s) = scripted("instrument p\ngate on\nv2 wave noise\nwave saw\nend", &["C-4 01 . .. G-4"]);
        run(&mut p, &mut s, 1);
        assert_eq!(s.voice(0).waveform, Waveform::Saw);
        assert_eq!(s.voice(1).waveform, Waveform::Saw, "v1 of the second instance is voice 2");
        assert_eq!(s.voice(1).phase_increment, ONA_PHASE_INCREMENT[46]);
    }

    #[test]
    fn a_script_that_failed_to_load_is_silent() {
        let text = one_channel("instrument 01 \"S\"  script \"x.snd\" blip", &["C-4 01 . .. ..."]);
        let (mut p, mut s) = player(&text);
        run(&mut p, &mut s, 1);
        assert_eq!(s.voice(0).envelope_stage, EnvStage::Off);
    }

    #[test]
    fn script_song_commands_reach_the_caller() {
        let (mut p, mut s) = scripted("instrument t\ntempo 3\nend", &["C-4 01 . .. ..."]);
        assert_eq!(run(&mut p, &mut s, 1), [SongCmd::Tempo(3)]);
    }

    #[test]
    fn a_muted_channel_stays_silent() {
        let (mut p, mut s) = player(&one_channel(LEAD, &["C-4 01 . .. ..."]));
        p.set_muted(&mut s, 0, true);
        run(&mut p, &mut s, 3);
        assert_eq!(s.voice(0).envelope_stage, EnvStage::Off);
        assert!(p.muted(0));
        p.set_muted(&mut s, 0, false);
        run(&mut p, &mut s, 2);
        assert_eq!(s.voice(0).envelope_stage, EnvStage::Attack);
    }

    #[test]
    fn a_lent_voice_is_left_alone_until_taken_back() {
        let (mut p, mut s) = player(&one_channel(&format!("{LEAD}  voice2 detune 6"), &["C-4 01 . .. ..."]));
        p.lend(1);
        s.set_voice_waveform(1, 3);
        run(&mut p, &mut s, 1);
        assert_eq!(s.voice(1).waveform, Waveform::Noise);
        assert_eq!(p.borrowed(), 0b10);
        p.take_back(1);
        run(&mut p, &mut s, 2);
        assert_eq!(s.voice(1).waveform, Waveform::Saw, "the next note uses it again");
    }

    #[test]
    fn the_donor_voice_follows_sfx_donor() {
        let (p, _) = player(&one_channel(LEAD, &[EMPTY]));
        assert_eq!(p.donor_voice(), 7);
        let (p, _) = player(&one_channel(LEAD, &[EMPTY]).replace("sfx-donor 4", "sfx-donor 1"));
        assert_eq!(p.donor_voice(), 1);
    }

    #[test]
    fn replace_song_keeps_the_place() {
        let rows = ["C-4 01 . .. ...", EMPTY, EMPTY, EMPTY];
        let (mut p, mut s) = player(&one_channel(LEAD, &rows));
        run(&mut p, &mut s, 3);
        assert_eq!(p.position(), (0, 1, 1));
        let edited = one_channel(LEAD, &["D-4 01 . .. ...", EMPTY, EMPTY, EMPTY]);
        p.replace_song(Arc::new(LoadedSong::plain(parse(&edited).unwrap())));
        assert_eq!(p.position(), (0, 1, 1));
        let shorter = one_channel(LEAD, &[EMPTY]);
        p.replace_song(Arc::new(LoadedSong::plain(parse(&shorter).unwrap())));
        assert_eq!(p.position(), (0, 0, 1), "row 1 no longer exists");
    }

    #[test]
    fn stop_gates_off_and_preview_only_plays_what_it_is_given() {
        let (mut p, mut s) = player(&one_channel(LEAD, &["C-4 01 . .. ..."]));
        run(&mut p, &mut s, 1);
        p.stop(&mut s);
        assert_eq!(s.voice(0).envelope_stage, EnvStage::Release);
        assert!(!p.playing);

        let text = one_channel(LEAD, &["C-4 01 . .. ..."]);
        let mut pv = Player::preview(Arc::new(LoadedSong::plain(parse(&text).unwrap())));
        let mut s = Synth::new();
        run(&mut pv, &mut s, 3);
        assert_eq!(s.voice(0).envelope_stage, EnvStage::Off, "a preview reads no rows");
        pv.trigger(&mut s, 2, 52, None, 1);
        assert_eq!(s.voice(4).envelope_stage, EnvStage::Attack);
        pv.note_off(&mut s, 2);
        assert_eq!(s.voice(4).envelope_stage, EnvStage::Release);
    }
}
```

Add `pub mod player;` to `lib.rs`.

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test --manifest-path v3/Cargo.toml -p acid-sound player`
Expected: compile errors, `cannot find struct Player`.

- [ ] **Step 3: Implement the player**

Put this above the tests in `player.rs`:

```rust
//! Plays a parsed song on the synth. Each channel steps through its own
//! order list, one row every `speed` ticks. Channel n plays voices 2n and
//! 2n+1; a voice lent to a sound effect is left alone until it comes back.

use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use alloc::vec::Vec;

use acid_synth::Synth;

use crate::pitch::{self, fine_pos, increment_at, FINE_MAX, FINE_STEPS};
use crate::program::Program;
use crate::song::{BuiltIn, Kind, Song, Voice2, CHANNELS, NOTE_NONE, NOTE_OFF};
use crate::vm::{Clock, Env, Instance, SongCmd, State};

/// A song ready to play. `scripts` maps an instrument number to the
/// program and instrument block of each script instrument that loaded.
#[derive(Debug)]
pub struct LoadedSong {
    pub song: Song,
    pub scripts: BTreeMap<u8, (Arc<Program>, usize)>,
}

impl LoadedSong {
    /// A song with no script instruments loaded (they play silence).
    pub fn plain(song: Song) -> Self {
        Self { song, scripts: BTreeMap::new() }
    }
}

/// What a row's command asks of that row only.
#[derive(Clone, Copy, Default)]
struct RowFx {
    slide: i32,
    vib_depth: i32,
    vib_speed: i32,
}

#[derive(Default)]
struct Chan {
    order_pos: usize,
    row: usize,
    /// The instrument column's last value.
    inst: u8,
    /// The instrument the sounding note started with.
    playing: u8,
    /// A built-in note is running its effects.
    sounding: bool,
    /// The started note's fine position, and where slides have taken it.
    base: i32,
    pos: i32,
    note2: Option<i32>,
    glide_to: Option<i32>,
    glide_speed: i32,
    fx: RowFx,
    vib_phase: i32,
    arp_step: usize,
    duty: i32,
    pwm_dir: i32,
    script: Option<Arc<Program>>,
    instances: [Option<Instance>; 2],
    muted: bool,
}

pub struct Player {
    song: Arc<LoadedSong>,
    /// False for a preview: no rows are read, only notes it is handed.
    pub playing: bool,
    speed: u8,
    tick: u8,
    chans: [Chan; CHANNELS],
    row_serial: u32,
    borrowed: u8,
    seed: u32,
}

fn pattern_len(song: &Song, ch: usize, order_pos: usize) -> usize {
    song.orders[ch]
        .entries
        .get(order_pos)
        .and_then(|e| song.patterns.get(&e.pattern))
        .map_or(1, |p| p.len())
}

/// A triangle LFO, -16..=16 over 64 steps.
fn tri(phase: i32) -> i32 {
    let p = phase & 63;
    if p < 16 {
        p
    } else if p < 48 {
        32 - p
    } else {
        p - 64
    }
}

/// Where a built-in's two voices sit; `wobble` is vibrato plus arp.
fn positions(c: &Chan, b: &BuiltIn, wobble: i32) -> (i32, Option<i32>) {
    let p1 = c.pos + wobble;
    let p2 = match (c.note2, b.voice2) {
        (Some(n2), _) => Some(n2 + (c.pos - c.base) + wobble),
        (None, Voice2::Detune(n)) => Some(p1 + n),
        (None, Voice2::Octave) => Some(p1 + 12 * FINE_STEPS),
        (None, Voice2::Fifth) => Some(p1 + 7 * FINE_STEPS),
        (None, Voice2::Ring) => Some(p1),
        (None, Voice2::Off) => None,
    };
    (p1, p2)
}

impl Player {
    pub fn new(song: Arc<LoadedSong>, order: usize, row: usize) -> Self {
        let speed = song.song.speed.max(1);
        let mut p = Self { song, playing: true, speed, tick: 0, chans: Default::default(), row_serial: 0, borrowed: 0, seed: 1 };
        p.seek(order, row);
        p
    }

    /// A player that only sounds the notes it is handed (the tracker's preview).
    pub fn preview(song: Arc<LoadedSong>) -> Self {
        let mut p = Self::new(song, 0, 0);
        p.playing = false;
        p
    }

    pub fn song(&self) -> &Arc<LoadedSong> {
        &self.song
    }

    pub fn clock(&self) -> Clock {
        Clock {
            playing: self.playing,
            order: self.chans[0].order_pos as i32,
            row: self.chans[0].row as i32,
            row_serial: self.row_serial,
        }
    }

    /// Channel 1's order position and row, and the tick within the row.
    pub fn position(&self) -> (i32, i32, i32) {
        (self.chans[0].order_pos as i32, self.chans[0].row as i32, self.tick as i32)
    }

    pub fn borrowed(&self) -> u8 {
        self.borrowed
    }

    /// The voice sound effects borrow: the donor channel's second voice.
    pub fn donor_voice(&self) -> u8 {
        (self.song.song.sfx_donor.clamp(1, CHANNELS as u8) - 1) * 2 + 1
    }

    pub fn muted(&self, ch: usize) -> bool {
        self.chans.get(ch).is_some_and(|c| c.muted)
    }

    fn seek(&mut self, order: usize, row: usize) {
        let song = self.song.clone();
        for (ch, c) in self.chans.iter_mut().enumerate() {
            c.order_pos = order.min(song.song.orders[ch].entries.len().saturating_sub(1));
            c.row = if row < pattern_len(&song.song, ch, c.order_pos) { row } else { 0 };
        }
        self.tick = 0;
    }

    /// Every channel to order position `order`, row 0, playing.
    pub fn jump(&mut self, order: i32) {
        self.seek(order.max(0) as usize, 0);
        self.playing = true;
    }

    pub fn set_speed(&mut self, s: i32) {
        if (1..=31).contains(&s) {
            self.speed = s as u8;
        }
    }

    /// Swaps in edited song data, keeping each channel's place where it still exists.
    pub fn replace_song(&mut self, song: Arc<LoadedSong>) {
        self.speed = song.song.speed.max(1);
        for (ch, c) in self.chans.iter_mut().enumerate() {
            c.order_pos = c.order_pos.min(song.song.orders[ch].entries.len().saturating_sub(1));
            if c.row >= pattern_len(&song.song, ch, c.order_pos) {
                c.row = 0;
            }
        }
        self.song = song;
        if self.tick >= self.speed {
            self.tick = 0;
        }
    }

    fn voice(&self, ch: usize, k: usize) -> Option<i32> {
        let v = ch * 2 + k;
        (self.borrowed & (1 << v) == 0).then_some(v as i32)
    }

    /// Lends voice `v` to a sound effect: the player and its scripts stop touching it.
    pub fn lend(&mut self, v: u8) {
        if v as usize >= CHANNELS * 2 {
            return;
        }
        self.borrowed |= 1 << v;
        for c in &mut self.chans {
            for i in c.instances.iter_mut().flatten() {
                i.drop_voice(v);
            }
        }
    }

    /// Takes voice `v` back; it sounds again from its channel's next note.
    pub fn take_back(&mut self, v: u8) {
        if (v as usize) < CHANNELS * 2 {
            self.borrowed &= !(1 << v);
        }
    }

    pub fn tick(&mut self, synth: &mut Synth, cmds: &mut Vec<SongCmd>) {
        if self.playing && self.tick == 0 {
            for ch in 0..CHANNELS {
                self.read_row(synth, ch);
            }
            self.row_serial = self.row_serial.wrapping_add(1);
        }
        for ch in 0..CHANNELS {
            self.effects(synth, ch);
        }
        let clock = self.clock();
        for c in &mut self.chans {
            let Some(prog) = c.script.clone() else { continue };
            for slot in &mut c.instances {
                let Some(inst) = slot else { continue };
                inst.tick(&prog.blocks[inst.block], &mut Env { synth: &mut *synth, clock, song_cmds: &mut *cmds });
                if inst.state == State::Done {
                    *slot = None;
                }
            }
        }
        if self.playing {
            self.tick += 1;
            if self.tick >= self.speed {
                self.tick = 0;
                for ch in 0..CHANNELS {
                    self.advance(ch);
                }
            }
        }
    }

    fn read_row(&mut self, synth: &mut Synth, ch: usize) {
        let song = self.song.clone();
        let (order_pos, row_i) = (self.chans[ch].order_pos, self.chans[ch].row);
        let Some(entry) = song.song.orders[ch].entries.get(order_pos).copied() else { return };
        let Some(row) = song.song.patterns.get(&entry.pattern).and_then(|p| p.get(row_i)).copied() else { return };
        self.chans[ch].fx = RowFx::default();
        if self.chans[ch].muted {
            return;
        }
        if row.inst != 0 {
            self.chans[ch].inst = row.inst;
        }
        let shift = |n: u8| (n as i32 + entry.transpose as i32).clamp(pitch::ONA_MIN, pitch::ONA_MAX);
        match row.note {
            NOTE_NONE => {}
            NOTE_OFF => self.note_off(synth, ch),
            n if row.cmd == b'3' => self.chans[ch].glide_to = Some(fine_pos(shift(n))),
            n => {
                let note2 = (row.note2 != NOTE_NONE).then(|| shift(row.note2));
                let inst = self.chans[ch].inst;
                self.trigger(synth, ch, shift(n), note2, inst);
            }
        }
        self.command(synth, ch, row.cmd, row.param);
    }

    fn command(&mut self, synth: &mut Synth, ch: usize, cmd: u8, param: u8) {
        let p = param as i32;
        match cmd {
            b'1' => self.chans[ch].fx.slide = p,
            b'2' => self.chans[ch].fx.slide = -p,
            b'3' => self.chans[ch].glide_speed = p.max(1),
            b'4' => {
                let fx = &mut self.chans[ch].fx;
                fx.vib_depth = p >> 4;
                fx.vib_speed = p & 15;
            }
            b'8' => {
                for k in 0..2 {
                    if let Some(v) = self.voice(ch, k) {
                        synth.set_voice_waveform(v, p & 3);
                    }
                }
            }
            b'9' => {
                let d = p.clamp(1, 99);
                self.chans[ch].duty = d;
                for k in 0..2 {
                    if let Some(v) = self.voice(ch, k) {
                        synth.set_duty(v, d);
                    }
                }
            }
            b'A' => synth.set_filter_cutoff(p),
            b'F' => self.set_speed(p),
            _ => {}
        }
    }

    /// Starts `ona` (and `note2`, from the second column) on channel `ch`
    /// with instrument `inst`. A missing instrument plays silence.
    pub fn trigger(&mut self, synth: &mut Synth, ch: usize, ona: i32, note2: Option<i32>, inst: u8) {
        if ch >= CHANNELS {
            return;
        }
        let song = self.song.clone();
        self.seed = self.seed.wrapping_add(1);
        let seed = self.seed;
        let (v1, v2) = (self.voice(ch, 0), self.voice(ch, 1));
        let c = &mut self.chans[ch];
        c.instances = [None, None];
        c.script = None;
        c.sounding = false;
        if c.muted {
            return;
        }
        c.playing = inst;
        c.base = fine_pos(ona);
        c.pos = c.base;
        c.note2 = note2.map(fine_pos);
        c.glide_to = None;
        c.vib_phase = 0;
        c.arp_step = 0;
        c.pwm_dir = 1;
        let Some(instrument) = song.song.instruments.get(&inst) else { return };
        match &instrument.kind {
            Kind::BuiltIn(b) => {
                c.duty = b.duty;
                c.sounding = true;
                let (p1, p2) = positions(c, b, 0);
                for (v, p) in [(v1, Some(p1)), (v2, p2)] {
                    let Some(v) = v else { continue };
                    let Some(p) = p else {
                        synth.gate_off(v);
                        continue;
                    };
                    synth.set_voice_waveform(v, b.wave);
                    synth.set_adsr(v, b.adsr[0], b.adsr[1], b.adsr[2], b.adsr[3]);
                    synth.set_duty(v, b.duty);
                    synth.set_voice_filter_route(v, b.filter.is_some() as i32);
                    synth.clear_ring_partner(v);
                    let vo = synth.voice_mut(v as usize);
                    vo.arp_active = false;
                    vo.phase_increment = increment_at(p);
                }
                if let (Voice2::Ring, None, Some(a), Some(bv)) = (b.voice2, note2, v1, v2) {
                    synth.set_ring_partner(a, bv);
                }
                if let Some((mode, cut, res)) = b.filter {
                    synth.set_filter_mode(mode);
                    synth.set_filter_cutoff(cut);
                    synth.set_filter_resonance(res);
                }
                for (v, p) in [(v1, Some(p1)), (v2, p2)] {
                    if let (Some(v), Some(_)) = (v, p) {
                        synth.gate_on(v);
                    }
                }
            }
            Kind::Script { .. } => {
                let Some((prog, block)) = song.scripts.get(&inst) else { return };
                let as_u8 = |v: Option<i32>| v.map(|v| v as u8);
                let mut start = |voices: [Option<u8>; 2], note: i32, n2: i32, seed: u32| {
                    let mut i = Instance::new(*block, voices, note, n2, seed);
                    i.start(synth);
                    i
                };
                c.instances = match note2 {
                    Some(n2) => [
                        Some(start([as_u8(v1), None], ona, n2, seed)),
                        Some(start([as_u8(v2), None], n2, n2, seed ^ 0x5A5A)),
                    ],
                    None => [Some(start([as_u8(v1), as_u8(v2)], ona, 0, seed)), None],
                };
                c.script = Some(prog.clone());
            }
        }
    }

    /// Note-off: a built-in releases its gates; a script runs `on release`.
    pub fn note_off(&mut self, synth: &mut Synth, ch: usize) {
        if ch >= CHANNELS {
            return;
        }
        let (v1, v2) = (self.voice(ch, 0), self.voice(ch, 1));
        let c = &mut self.chans[ch];
        if let Some(prog) = c.script.clone() {
            for inst in c.instances.iter_mut().flatten() {
                inst.release(&prog.blocks[inst.block], synth);
            }
        } else {
            for v in [v1, v2].into_iter().flatten() {
                synth.gate_off(v);
            }
        }
    }

    /// Built-in instruments' per-tick work: slides, glide, vibrato, arp, PWM.
    fn effects(&mut self, synth: &mut Synth, ch: usize) {
        let song = self.song.clone();
        let (v1, v2) = (self.voice(ch, 0), self.voice(ch, 1));
        let c = &mut self.chans[ch];
        if c.muted || !c.sounding {
            return;
        }
        let Some(Kind::BuiltIn(b)) = song.song.instruments.get(&c.playing).map(|i| &i.kind) else { return };
        c.pos = (c.pos + c.fx.slide).clamp(0, FINE_MAX);
        if let Some(t) = c.glide_to {
            let step = c.glide_speed.max(1);
            c.pos = if c.pos < t { (c.pos + step).min(t) } else { (c.pos - step).max(t) };
            if c.pos == t {
                c.glide_to = None;
            }
        }
        let (depth, speed) = if c.fx.vib_depth > 0 { (c.fx.vib_depth, c.fx.vib_speed) } else { b.vib };
        c.vib_phase = (c.vib_phase + speed) & 63;
        let vib = depth * tri(c.vib_phase) / 4;
        let arp = if b.arp.is_empty() {
            0
        } else {
            let k = c.arp_step % (b.arp.len() + 1);
            c.arp_step += 1;
            if k == 0 { 0 } else { b.arp[k - 1] * FINE_STEPS }
        };
        if b.pwm != 0 {
            c.duty += b.pwm * c.pwm_dir;
            if c.duty >= 90 {
                c.duty = 90;
                c.pwm_dir = -1;
            } else if c.duty <= 10 {
                c.duty = 10;
                c.pwm_dir = 1;
            }
            for v in [v1, v2].into_iter().flatten() {
                synth.set_duty(v, c.duty);
            }
        }
        let (p1, p2) = positions(c, b, vib + arp);
        if let Some(v) = v1 {
            synth.voice_mut(v as usize).phase_increment = increment_at(p1);
        }
        if let (Some(v), Some(p)) = (v2, p2) {
            synth.voice_mut(v as usize).phase_increment = increment_at(p);
        }
    }

    fn advance(&mut self, ch: usize) {
        let song = &self.song.song;
        let c = &mut self.chans[ch];
        c.row += 1;
        if c.row >= pattern_len(song, ch, c.order_pos) {
            c.row = 0;
            c.order_pos += 1;
            if c.order_pos >= song.orders[ch].entries.len() {
                c.order_pos = song.orders[ch].loop_to;
            }
        }
    }

    /// Silences every voice the player holds and stops playing.
    pub fn stop(&mut self, synth: &mut Synth) {
        for ch in 0..CHANNELS {
            for k in 0..2 {
                if let Some(v) = self.voice(ch, k) {
                    synth.gate_off(v);
                }
            }
            let c = &mut self.chans[ch];
            c.instances = [None, None];
            c.script = None;
            c.sounding = false;
        }
        self.playing = false;
    }

    /// A muted channel keeps its place but leaves its voices silent.
    pub fn set_muted(&mut self, synth: &mut Synth, ch: usize, on: bool) {
        if ch >= CHANNELS {
            return;
        }
        if on {
            for k in 0..2 {
                if let Some(v) = self.voice(ch, k) {
                    synth.gate_off(v);
                }
            }
            let c = &mut self.chans[ch];
            c.instances = [None, None];
            c.script = None;
            c.sounding = false;
        }
        self.chans[ch].muted = on;
    }
}
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test --manifest-path v3/Cargo.toml -p acid-sound player`
Expected: 21 passed.

If `a_muted_channel_stays_silent` fails after unmuting, check the tick count first. Speed is 2 and the pattern is 1 row, so a fresh row is read at ticks 1, 3, 5 and so on. After 3 muted ticks, 2 more ticks reach tick 5, which reads the row and triggers the note. Fix the code, not the expected values, unless that walk-through shows the test is wrong.

- [ ] **Step 5: Commit**

```bash
git add v3/crates/acid-sound
git commit -m "acid-sound: song player (built-in and script instruments, note 2, mutes, lending)

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 9: The engine

**Files:**
- Create: `v3/crates/acid-sound/src/engine.rs`
- Modify: `v3/crates/acid-sound/src/program.rs` (add the `songs` field)
- Modify: `v3/crates/acid-sound/src/lib.rs` (add `pub mod engine;`)

**Interfaces:**
- Consumes:
  - `player::{LoadedSong, Player}`
  - `vm::{Clock, Env, Instance, SongCmd, State, SONG_CMDS_MAX}`
  - `program::{BlockKind, Program}`
  - `TICK_SAMPLES`
- Produces:
  - `Program::songs: Vec<Option<Arc<LoadedSong>>>`, which runs parallel to `song_paths` and is filled by the loader in Task 10
  - `engine::MAX_SOUNDS = 16`
  - `engine::Engine` with these methods (`owner` is the app's task id as a `u32`, and `handle` is the app's song handle, 0 for songs started by scripts):
    - `new()`, `ticks() -> u32`, `sound_count() -> usize`
    - `render(&mut Synth, &mut [u8])`, `tick(&mut Synth)`
    - `play_sound(&mut Synth, owner, Arc<Program>, block, note, busy: u8) -> Option<u32>`
    - `stop_sound(&mut Synth, owner, id)`
    - `play_song(&mut Synth, owner, handle, Arc<LoadedSong>, order: i32, row: i32)`, `stop_song(&mut Synth)`
    - `update_song(owner, handle, Arc<LoadedSong>)`
    - `song_owner() -> Option<u32>`, `song_position() -> Option<(i32, i32, i32)>`, `song_player() -> Option<&Player>`
    - `mute(&mut Synth, ch: i32 (1-based), on)`
    - `preview(&mut Synth, owner, Arc<LoadedSong>, ch: i32 (1-based), note: i32, inst: i32)`
    - `release_owner(&mut Synth, owner)`
- **Voice choice for a sound.** Take the highest-numbered voice not in `busy`, not held by another sound, and not reserved by a playing song. A playing song reserves every voice except its donor voice. A voice the sound takes is lent to the song player and the preview player, and handed back when the sound ends.

- [ ] **Step 1: Add the `songs` field to `Program`**

In `program.rs`, add `use alloc::sync::Arc;`. Then add this field to `Program`, after `song_paths`:

```rust
    /// Loaded songs, parallel to `song_paths`; filled by `load::load_program`.
    /// None (or missing) means `play` on that song does nothing.
    pub songs: Vec<Option<Arc<crate::player::LoadedSong>>>,
```

`Program` derives `Default` and `Debug`, and both still work because `LoadedSong` is `Debug`.

- [ ] **Step 2: Write the failing tests**

Create `v3/crates/acid-sound/src/engine.rs` with:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::compile::compile;
    use crate::song::parse;
    use alloc::format;
    use alloc::string::String;
    use alloc::vec;
    use acid_synth::EnvStage;

    fn prog(src: &str) -> Arc<Program> {
        Arc::new(compile(src).unwrap())
    }

    /// All four channels play a detuned two-voice C-4 for two rows.
    fn four(donor: u8) -> String {
        format!(
            "acid-track 1\ntitle t\nspeed 2\nsfx-donor {donor}\ninstrument 01 \"Lead\"  wave saw  adsr 0 0 100 0  duty 50  voice2 detune 6\norder 1  00 loop 0\norder 2  00 loop 0\norder 3  00 loop 0\norder 4  00 loop 0\n\npattern 00 2\nC-4 01 . .. ...\n... .. . .. ...\n"
        )
    }

    fn song(donor: u8) -> Arc<LoadedSong> {
        Arc::new(LoadedSong::plain(parse(&four(donor)).unwrap()))
    }

    fn render(e: &mut Engine, s: &mut Synth, n: usize) {
        let mut buf = vec![0u8; n];
        e.render(s, &mut buf);
    }

    const T: usize = TICK_SAMPLES as usize;

    #[test]
    fn an_idle_engine_renders_like_the_bare_synth() {
        let (mut a, mut b) = (Synth::new(), Synth::new());
        for s in [&mut a, &mut b] {
            s.set_ona(0, 40);
            s.gate_on(0);
        }
        let mut e = Engine::new();
        let mut got = Vec::new();
        for n in [1, 100, 441, 1000, 7] {
            let mut buf = vec![0u8; n];
            e.render(&mut a, &mut buf);
            got.extend_from_slice(&buf);
        }
        let mut want = vec![0u8; got.len()];
        b.render(&mut want);
        assert_eq!(got, want);
    }

    #[test]
    fn ticks_land_every_441_samples_whatever_the_buffer() {
        let (mut e1, mut e2, mut s) = (Engine::new(), Engine::new(), Synth::new());
        render(&mut e1, &mut s, 1900);
        for _ in 0..9 {
            render(&mut e2, &mut s, 100);
        }
        render(&mut e2, &mut s, 1000);
        assert_eq!((e1.ticks(), e2.ticks()), (5, 5), "ticks at samples 0, 441, 882, 1323, 1764");
    }

    #[test]
    fn sounds_take_the_highest_free_voice() {
        let (mut e, mut s) = (Engine::new(), Synth::new());
        let p = prog("gate on\nwait 10");
        assert!(e.play_sound(&mut s, 1, p.clone(), 0, 40, 0).is_some());
        assert!(e.play_sound(&mut s, 1, p.clone(), 0, 40, 0).is_some());
        assert_eq!(e.play_sound(&mut s, 1, p.clone(), 0, 40, 0b0011_1111), None, "7 and 6 are taken, the rest busy");
        render(&mut e, &mut s, T);
        assert_eq!(s.voice(7).envelope_stage, EnvStage::Attack);
        assert_eq!(s.voice(6).envelope_stage, EnvStage::Attack);
        assert_eq!(s.voice(5).envelope_stage, EnvStage::Off);
    }

    #[test]
    fn a_two_voice_sound_takes_two() {
        let (mut e, mut s) = (Engine::new(), Synth::new());
        e.play_sound(&mut s, 1, prog("both gate on\nwait 10"), 0, 40, 0).unwrap();
        render(&mut e, &mut s, T);
        assert_eq!(s.voice(7).envelope_stage, EnvStage::Attack);
        assert_eq!(s.voice(6).envelope_stage, EnvStage::Attack);
    }

    #[test]
    fn only_sound_blocks_play_as_sounds() {
        let (mut e, mut s) = (Engine::new(), Synth::new());
        assert_eq!(e.play_sound(&mut s, 1, prog("instrument i\ngate on\nend"), 0, 40, 0), None);
        assert_eq!(e.play_sound(&mut s, 1, prog("wait 1"), 3, 40, 0), None, "no such block");
    }

    #[test]
    fn a_finished_sound_frees_its_voice() {
        let (mut e, mut s) = (Engine::new(), Synth::new());
        e.play_sound(&mut s, 1, prog("gate on\nwait 2"), 0, 40, 0).unwrap();
        render(&mut e, &mut s, 3 * T);
        assert_eq!(e.sound_count(), 0);
        e.play_sound(&mut s, 1, prog("gate on\nwait 2"), 0, 52, 0).unwrap();
        render(&mut e, &mut s, T);
        assert_eq!(s.voice(7).phase_increment, acid_synth::ONA_PHASE_INCREMENT[51], "voice 7 again");
    }

    #[test]
    fn during_a_song_sounds_borrow_only_the_donor_voice() {
        let (mut e, mut s) = (Engine::new(), Synth::new());
        e.play_song(&mut s, 1, 1, song(4), 0, 0);
        render(&mut e, &mut s, T);
        let p = prog("gate on\nwait 2");
        assert!(e.play_sound(&mut s, 2, p.clone(), 0, 40, 0).is_some());
        assert_eq!(e.song_player().unwrap().borrowed(), 1 << 7);
        assert_eq!(e.play_sound(&mut s, 2, p, 0, 40, 0), None, "everything else belongs to the song");
        render(&mut e, &mut s, 4 * T);
        assert_eq!(e.song_player().unwrap().borrowed(), 0, "handed back when the sound ends");
    }

    #[test]
    fn sfx_donor_moves_the_borrowed_voice() {
        let (mut e, mut s) = (Engine::new(), Synth::new());
        e.play_song(&mut s, 1, 1, song(1), 0, 0);
        e.play_sound(&mut s, 2, prog("gate on\nwait 2"), 0, 40, 0).unwrap();
        assert_eq!(e.song_player().unwrap().borrowed(), 1 << 1);
    }

    #[test]
    fn a_script_can_start_a_song() {
        let (mut e, mut s) = (Engine::new(), Synth::new());
        let mut p = compile("song \"s.trk\"\nplay").unwrap();
        p.songs = vec![Some(song(4))];
        e.play_sound(&mut s, 3, Arc::new(p), 0, 40, 0).unwrap();
        render(&mut e, &mut s, 2 * T);
        assert_eq!(e.song_position(), Some((0, 0, 1)));
        assert_eq!(e.song_owner(), Some(3));
    }

    #[test]
    fn a_script_can_mute_a_channel() {
        let (mut e, mut s) = (Engine::new(), Synth::new());
        e.play_song(&mut s, 1, 1, song(4), 0, 0);
        render(&mut e, &mut s, T);
        e.play_sound(&mut s, 1, prog("mute 1"), 0, 40, 0).unwrap();
        render(&mut e, &mut s, T);
        assert!(e.song_player().unwrap().muted(0));
        assert_eq!(s.voice(0).envelope_stage, EnvStage::Release);
    }

    #[test]
    fn release_owner_stops_its_song_and_sounds() {
        let (mut e, mut s) = (Engine::new(), Synth::new());
        e.play_song(&mut s, 1, 1, song(4), 0, 0);
        e.play_sound(&mut s, 1, prog("gate on\nwait 50"), 0, 40, 0).unwrap();
        e.stop_song(&mut s);
        e.play_sound(&mut s, 2, prog("gate on\nwait 50"), 0, 40, 0).unwrap();
        e.play_song(&mut s, 1, 1, song(4), 0, 0);
        render(&mut e, &mut s, T);
        e.release_owner(&mut s, 1);
        assert_eq!(e.song_position(), None);
        assert_eq!(e.sound_count(), 1);
    }

    #[test]
    fn update_song_swaps_data_and_keeps_the_place() {
        let (mut e, mut s) = (Engine::new(), Synth::new());
        e.play_song(&mut s, 1, 5, song(4), 0, 0);
        render(&mut e, &mut s, 3 * T);
        assert_eq!(e.song_position(), Some((0, 1, 1)));
        let b = song(4);
        e.update_song(1, 5, b.clone());
        assert!(Arc::ptr_eq(e.song_player().unwrap().song(), &b));
        assert_eq!(e.song_position(), Some((0, 1, 1)));
        e.update_song(1, 6, song(4));
        assert!(Arc::ptr_eq(e.song_player().unwrap().song(), &b), "another handle's update is ignored");
    }

    #[test]
    fn preview_sounds_a_note_while_stopped() {
        let (mut e, mut s) = (Engine::new(), Synth::new());
        e.preview(&mut s, 1, song(4), 1, 40, 1);
        assert_eq!(s.voice(0).envelope_stage, EnvStage::Attack);
        e.preview(&mut s, 1, song(4), 1, 0, 0);
        assert_eq!(s.voice(0).envelope_stage, EnvStage::Release);
        e.preview(&mut s, 1, song(4), 9, 40, 1);
    }
}
```

The last `preview` call checks that an out-of-range channel is ignored rather than panicking.

Add `pub mod engine;` to `lib.rs`.

- [ ] **Step 3: Run them to see them fail**

Run: `cargo test --manifest-path v3/Cargo.toml -p acid-sound engine`
Expected: compile errors, `cannot find struct Engine`.

- [ ] **Step 4: Implement the engine**

Put this above the tests in `engine.rs`:

```rust
//! The engine the kernel ticks: one song, a preview player for the
//! tracker, and up to MAX_SOUNDS running .snd sounds, stepped every
//! TICK_SAMPLES samples while the synth renders. Owners are app task ids.

use alloc::sync::Arc;
use alloc::vec::Vec;

use acid_synth::{Synth, NUM_VOICES};

use crate::player::{LoadedSong, Player};
use crate::program::{BlockKind, Program};
use crate::song::CHANNELS;
use crate::vm::{Clock, Env, Instance, SongCmd, State, SONG_CMDS_MAX};
use crate::TICK_SAMPLES;

pub const MAX_SOUNDS: usize = 16;

struct Sound {
    id: u32,
    owner: u32,
    prog: Arc<Program>,
    inst: Instance,
}

struct SongSlot {
    owner: u32,
    /// The app's song handle; 0 for a song a script started.
    handle: u32,
    player: Player,
}

pub struct Engine {
    song: Option<SongSlot>,
    preview: Option<SongSlot>,
    sounds: Vec<Sound>,
    cmds: Vec<SongCmd>,
    until_tick: u32,
    ticks: u32,
    next_id: u32,
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

impl Engine {
    pub fn new() -> Self {
        Self {
            song: None,
            preview: None,
            sounds: Vec::with_capacity(MAX_SOUNDS),
            cmds: Vec::with_capacity(SONG_CMDS_MAX),
            until_tick: 0,
            ticks: 0,
            next_id: 1,
        }
    }

    pub fn ticks(&self) -> u32 {
        self.ticks
    }

    pub fn sound_count(&self) -> usize {
        self.sounds.len()
    }

    pub fn song_player(&self) -> Option<&Player> {
        self.song.as_ref().map(|s| &s.player)
    }

    pub fn song_owner(&self) -> Option<u32> {
        self.song.as_ref().map(|s| s.owner)
    }

    pub fn song_position(&self) -> Option<(i32, i32, i32)> {
        self.song.as_ref().filter(|s| s.player.playing).map(|s| s.player.position())
    }

    /// Renders `buf`, ticking at every TICK_SAMPLES boundary. The counter
    /// carries across calls, so tick timing doesn't depend on buffer sizes.
    pub fn render(&mut self, synth: &mut Synth, buf: &mut [u8]) {
        let mut done = 0;
        while done < buf.len() {
            if self.until_tick == 0 {
                self.tick(synth);
                self.until_tick = TICK_SAMPLES;
            }
            let n = (self.until_tick as usize).min(buf.len() - done);
            synth.render(&mut buf[done..done + n]);
            done += n;
            self.until_tick -= n as u32;
        }
    }

    pub fn tick(&mut self, synth: &mut Synth) {
        self.ticks = self.ticks.wrapping_add(1);
        if let Some(s) = self.song.as_mut() {
            self.cmds.clear();
            s.player.tick(synth, &mut self.cmds);
            let owner = s.owner;
            self.apply_cmds(synth, owner, None);
        }
        if let Some(p) = self.preview.as_mut() {
            self.cmds.clear();
            p.player.tick(synth, &mut self.cmds);
        }
        let clock = self.song.as_ref().map_or(Clock::default(), |s| s.player.clock());
        let mut i = 0;
        while i < self.sounds.len() {
            self.cmds.clear();
            let prog = self.sounds[i].prog.clone();
            let owner = self.sounds[i].owner;
            {
                let inst = &mut self.sounds[i].inst;
                inst.tick(&prog.blocks[inst.block], &mut Env { synth: &mut *synth, clock, song_cmds: &mut self.cmds });
            }
            self.apply_cmds(synth, owner, Some(&prog));
            if self.sounds[i].inst.state == State::Done {
                let s = self.sounds.remove(i);
                self.give_back(&s);
            } else {
                i += 1;
            }
        }
    }

    fn apply_cmds(&mut self, synth: &mut Synth, owner: u32, prog: Option<&Arc<Program>>) {
        for k in 0..self.cmds.len() {
            match self.cmds[k] {
                SongCmd::Play { song: Some(i), order } => {
                    let found = prog.and_then(|p| p.songs.get(i as usize)).and_then(|s| s.clone());
                    if let Some(ls) = found {
                        self.play_song(synth, owner, 0, ls, order, 0);
                    }
                }
                SongCmd::Play { song: None, order } => {
                    if let Some(s) = self.song.as_mut() {
                        s.player.jump(order);
                    }
                }
                SongCmd::Stop => self.stop_song(synth),
                SongCmd::Tempo(n) => {
                    if let Some(s) = self.song.as_mut() {
                        s.player.set_speed(n);
                    }
                }
                SongCmd::Mute(ch, on) => self.mute(synth, ch, on),
                SongCmd::Jump(o) => {
                    if let Some(s) = self.song.as_mut() {
                        s.player.jump(o);
                    }
                }
            }
        }
    }

    fn sound_voices(&self) -> u8 {
        let mut m = 0u8;
        for s in &self.sounds {
            for v in s.inst.voices.iter().flatten() {
                m |= 1 << v;
            }
        }
        m
    }

    fn give_back(&mut self, s: &Sound) {
        for v in s.inst.voices.iter().flatten() {
            for slot in [self.song.as_mut(), self.preview.as_mut()].into_iter().flatten() {
                slot.player.take_back(*v);
            }
        }
    }

    /// Starts a `sound` block on free voices (see the module docs); `busy`
    /// is voices apps are playing directly. None when nothing is free.
    pub fn play_sound(&mut self, synth: &mut Synth, owner: u32, prog: Arc<Program>, block: usize, note: i32, busy: u8) -> Option<u32> {
        let b = prog.blocks.get(block)?;
        if b.kind != BlockKind::Sound || self.sounds.len() >= MAX_SOUNDS {
            return None;
        }
        let mut taken = busy | self.sound_voices();
        if let Some(s) = self.song.as_ref().filter(|s| s.player.playing) {
            taken |= !(1u8 << s.player.donor_voice());
        }
        let mut free = (0..NUM_VOICES as u8).rev().filter(|v| taken & (1 << v) == 0);
        let v1 = free.next()?;
        let v2 = if b.uses_v2 { free.next() } else { None };
        for v in [Some(v1), v2].into_iter().flatten() {
            for slot in [self.song.as_mut(), self.preview.as_mut()].into_iter().flatten() {
                slot.player.lend(v);
            }
        }
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1).max(1);
        let mut inst = Instance::new(block, [Some(v1), v2], note, 0, self.ticks ^ id.rotate_left(16));
        inst.start(synth);
        self.sounds.push(Sound { id, owner, prog, inst });
        Some(id)
    }

    pub fn stop_sound(&mut self, synth: &mut Synth, owner: u32, id: u32) {
        if let Some(i) = self.sounds.iter().position(|s| s.id == id && s.owner == owner) {
            let mut s = self.sounds.remove(i);
            s.inst.stop(synth);
            self.give_back(&s);
        }
    }

    /// Replaces any playing song. Voices sounds hold stay lent to them.
    pub fn play_song(&mut self, synth: &mut Synth, owner: u32, handle: u32, song: Arc<LoadedSong>, order: i32, row: i32) {
        self.stop_song(synth);
        let mut player = Player::new(song, order.max(0) as usize, row.max(0) as usize);
        let held = self.sound_voices();
        for v in 0..NUM_VOICES as u8 {
            if held & (1 << v) != 0 {
                player.lend(v);
            }
        }
        self.song = Some(SongSlot { owner, handle, player });
    }

    pub fn stop_song(&mut self, synth: &mut Synth) {
        if let Some(mut s) = self.song.take() {
            s.player.stop(synth);
        }
    }

    /// Swaps edited data into the playing song if it came from this handle.
    pub fn update_song(&mut self, owner: u32, handle: u32, song: Arc<LoadedSong>) {
        if let Some(s) = self.song.as_mut().filter(|s| s.owner == owner && s.handle == handle) {
            s.player.replace_song(song);
        }
    }

    /// `ch` is 1-based.
    pub fn mute(&mut self, synth: &mut Synth, ch: i32, on: bool) {
        let Ok(ch) = usize::try_from(ch.saturating_sub(1)) else { return };
        if let Some(s) = self.song.as_mut() {
            s.player.set_muted(synth, ch, on);
        }
    }

    /// Sounds one note on channel `ch` (1-based) of `song`; note 0 is note-off.
    pub fn preview(&mut self, synth: &mut Synth, owner: u32, song: Arc<LoadedSong>, ch: i32, note: i32, inst: i32) {
        let Ok(ch) = usize::try_from(ch.saturating_sub(1)) else { return };
        if ch >= CHANNELS {
            return;
        }
        let same = self.preview.as_ref().is_some_and(|p| p.owner == owner && Arc::ptr_eq(p.player.song(), &song));
        if !same {
            if let Some(mut p) = self.preview.take() {
                p.player.stop(synth);
            }
            let mut player = Player::preview(song);
            let held = self.sound_voices();
            for v in 0..NUM_VOICES as u8 {
                if held & (1 << v) != 0 {
                    player.lend(v);
                }
            }
            self.preview = Some(SongSlot { owner, handle: 0, player });
        }
        let Some(p) = self.preview.as_mut() else { return };
        if note <= 0 {
            p.player.note_off(synth, ch);
        } else {
            p.player.trigger(synth, ch, note, None, inst.clamp(0, 255) as u8);
        }
    }

    /// An app exited: its sounds, its song and its preview stop.
    pub fn release_owner(&mut self, synth: &mut Synth, owner: u32) {
        let mut i = 0;
        while i < self.sounds.len() {
            if self.sounds[i].owner == owner {
                let mut s = self.sounds.remove(i);
                s.inst.stop(synth);
                self.give_back(&s);
            } else {
                i += 1;
            }
        }
        if self.song_owner() == Some(owner) {
            self.stop_song(synth);
        }
        if self.preview.as_ref().is_some_and(|p| p.owner == owner) {
            if let Some(mut p) = self.preview.take() {
                p.player.stop(synth);
            }
        }
    }
}
```

In `tick`, the block that holds `inst` is scoped so that borrow ends before `apply_cmds` takes `&mut self`. `Player::trigger` clamps `note` itself through `fine_pos` and `increment_at`.

- [ ] **Step 5: Run every `acid-sound` test**

Run: `cargo test --manifest-path v3/Cargo.toml -p acid-sound`
Expected: all pass, 13 of them engine tests.

`during_a_song_sounds_borrow_only_the_donor_voice` renders 4 ticks after the sound starts. The sound's first tick gates it on and starts `wait 2`. It finishes 2 ticks later, when the engine removes it and gives the voice back. 4 ticks leaves one spare.

- [ ] **Step 6: Commit**

```bash
git add v3/crates/acid-sound
git commit -m "acid-sound: engine (tick-exact render, sound voices, donor lending, song commands, preview)

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 10: Loading files, and the golden audio test

**Files:**
- Create: `v3/crates/acid-sound/src/load.rs`
- Modify: `v3/crates/acid-sound/src/lib.rs` (add `pub mod load;`)
- Create: `v3/crates/acid-sound/tests/golden.rs`
- Create: `v3/crates/acid-sound/tests/golden/demo.trk` and `v3/crates/acid-sound/tests/golden/demo.snd`
- Create: `v3/crates/acid-sound/tests/golden/demo.u8` (blessed once, in Step 7)
- Create: `v3/crates/acid-sound/examples/to_wav.rs`

**Interfaces:**
- Consumes: `compile`, `song::parse`, `player::LoadedSong`, `engine::Engine`
- Produces:
  - `load::load_song(text: &str, read: &dyn Fn(&str) -> Result<String, String>) -> Result<(LoadedSong, Vec<String>), SongError>`, which returns the song plus one warning per script instrument that didn't load. Each warning reads `"instrument NN: PATH: reason"`.
  - `load::load_program(src: &str, read: &dyn Fn(&str) -> Result<String, String>) -> Result<Program, CompileError>`. A song that won't load is an error at its `song` line: `"L:C song \"PATH\": reason"`.
- `read` takes a path relative to fsroot. Scripts inside a song are compiled but their own `song` lines aren't loaded, so there's no recursion.

- [ ] **Step 1: Write the failing loader tests**

Create `v3/crates/acid-sound/src/load.rs` with:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;

    const SONG: &str = "acid-track 1\ntitle t\nspeed 6\nsfx-donor 4\ninstrument 01 \"A\"  script \"Home/good.snd\" bass\ninstrument 02 \"B\"  script \"Home/bad.snd\" bass\ninstrument 03 \"C\"  script \"Home/missing.snd\" bass\ninstrument 04 \"D\"  script \"Home/good.snd\" zap\norder 1  00 loop 0\norder 2  00 loop 0\norder 3  00 loop 0\norder 4  00 loop 0\n\npattern 00 1\n... .. . .. ...\n";

    fn files(path: &str) -> Result<String, String> {
        match path {
            "Home/good.snd" => Ok("instrument bass\ngate on\nend\nsound zap\ngate on\nend".into()),
            "Home/bad.snd" => Ok("wav saw".into()),
            "Home/s.trk" => Ok(SONG.into()),
            _ => Err("not found".into()),
        }
    }

    #[test]
    fn script_instruments_load_or_warn() {
        let (song, warnings) = load_song(SONG, &files).unwrap();
        assert_eq!(
            warnings,
            [
                "instrument 02: Home/bad.snd: 1:1 unknown command 'wav'",
                "instrument 03: Home/missing.snd: not found",
                "instrument 04: Home/good.snd: no instrument 'zap'",
            ]
        );
        assert_eq!(song.scripts.keys().copied().collect::<Vec<u8>>(), [1]);
        assert_eq!(song.scripts[&1].1, 0, "block 0 is 'bass'");
    }

    #[test]
    fn a_bad_song_file_is_an_error_not_a_warning() {
        assert_eq!(load_song("nope", &files).unwrap_err().to_string(), "1: not an acid-track file");
    }

    #[test]
    fn programs_load_the_songs_they_name() {
        let p = load_program("song \"Home/s.trk\"\nplay", &files).unwrap();
        assert_eq!(p.songs.len(), 1);
        assert!(p.songs[0].is_some());
    }

    #[test]
    fn a_song_that_wont_load_points_at_its_line() {
        assert_eq!(load_program("wait 1\nsong \"Home/x.trk\"", &files).unwrap_err().to_string(), "2:6 song \"Home/x.trk\": not found");
        assert_eq!(
            load_program("song \"Home/bad.snd\"", &files).unwrap_err().to_string(),
            "1:6 song \"Home/bad.snd\": 1: not an acid-track file"
        );
    }
}
```

Add `pub mod load;` to `lib.rs`.

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test --manifest-path v3/Cargo.toml -p acid-sound load`
Expected: compile errors, `cannot find function load_song`.

- [ ] **Step 3: Implement the loader**

Put this above the tests in `load.rs`:

```rust
//! Files that name other files: a song's script instruments, a program's
//! `song "PATH"` lines. `read` fetches text by fsroot-relative path, so
//! this crate never touches a filesystem itself.

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::compile::compile;
use crate::lexer::CompileError;
use crate::player::LoadedSong;
use crate::program::{BlockKind, Program};
use crate::song::{parse, Kind, SongError};

pub type Read<'a> = &'a dyn Fn(&str) -> Result<String, String>;

/// Parses a song and compiles its script instruments. A script that won't
/// load leaves its instrument silent and adds a warning; a bad song is an error.
pub fn load_song(text: &str, read: Read) -> Result<(LoadedSong, Vec<String>), SongError> {
    let song = parse(text)?;
    let mut cache: BTreeMap<String, Result<Arc<Program>, String>> = BTreeMap::new();
    let mut scripts = BTreeMap::new();
    let mut warnings = Vec::new();
    for (num, inst) in &song.instruments {
        let Kind::Script { path, block } = &inst.kind else { continue };
        let prog = cache
            .entry(path.clone())
            .or_insert_with(|| read(path).and_then(|src| compile(&src).map(Arc::new).map_err(|e| e.to_string())))
            .clone();
        match prog {
            Err(e) => warnings.push(format!("instrument {num:02X}: {path}: {e}")),
            Ok(p) => match p.blocks.iter().position(|b| b.name == *block && b.kind == BlockKind::Instrument) {
                Some(i) => {
                    scripts.insert(*num, (p, i));
                }
                None => warnings.push(format!("instrument {num:02X}: {path}: no instrument '{block}'")),
            },
        }
    }
    Ok((LoadedSong { song, scripts }, warnings))
}

/// Compiles a program and loads every song it names.
pub fn load_program(src: &str, read: Read) -> Result<Program, CompileError> {
    let mut prog = compile(src)?;
    let mut songs = Vec::with_capacity(prog.song_paths.len());
    for r in &prog.song_paths {
        let fail = |e: String| CompileError::new(r.line, r.col, format!("song \"{}\": {e}", r.path));
        let text = read(&r.path).map_err(fail)?;
        let (song, _warnings) = load_song(&text, read).map_err(|e| fail(e.to_string()))?;
        songs.push(Some(Arc::new(song)));
    }
    prog.songs = songs;
    Ok(prog)
}
```

- [ ] **Step 4: Run the loader tests to see them pass**

Run: `cargo test --manifest-path v3/Cargo.toml -p acid-sound load`
Expected: 4 passed.

- [ ] **Step 5: Add the golden demo files**

Create `v3/crates/acid-sound/tests/golden/demo.snd`:

```
# The golden demo's script instruments and its sound effect.
instrument bass
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

instrument pluck
  wave tri
  adsr 0 30 0 40
  arp 12 7
  gate on
end

sound zap
  wave saw
  adsr 0 40 0 80
  pitch 70
  gate on
  repeat 12
    pitch -3
    wait 1
  end
  gate off
end
```

Create `v3/crates/acid-sound/tests/golden/demo.trk`, which must be canonical and end with a single newline:

```
acid-track 1
title Golden Demo
speed 5
sfx-donor 4
instrument 01 "Lead"  wave pulse  adsr 1 20 60 40  duty 40  pwm 2  vib 3 3  voice2 detune 5
instrument 02 "Bass"  script "Home/sounds/demo.snd" bass
instrument 03 "Pluck"  script "Home/sounds/demo.snd" pluck
instrument 04 "Hat"  wave noise  adsr 0 3 0 3  duty 50
order 1  00 00+5 loop 0
order 2  01 loop 0
order 3  02 loop 0
order 4  03 loop 0

pattern 00 8
C-4 01 . .. ...
... .. . .. ...
E-4 .. . .. ...
... .. 4 34 ...
G-4 .. . .. ...
=== .. . .. ...
C-5 .. 2 04 ...
... .. . .. ...

pattern 01 8
C-2 02 . .. ...
... .. . .. ...
... .. . .. ...
=== .. . .. ...
G-1 02 . .. ...
... .. . .. ...
... .. . .. ...
=== .. . .. ...

pattern 02 4
C-4 03 . .. E-4
... .. . .. ...
G-3 03 . .. B-3
... .. . .. ...

pattern 03 2
C-6 04 . .. ...
... .. . .. ...
```

- [ ] **Step 6: Write the golden test and the listening tool**

Create `v3/crates/acid-sound/tests/golden.rs`:

```rust
//! The engine must render tests/golden/demo.trk, with demo.snd's
//! instruments and its zap effect fired mid-song, byte for byte as the
//! committed recording tests/golden/demo.u8. Never regenerate the
//! recording to make this test pass: ACID_SOUND_BLESS=1 is only for a
//! change in sound that is intended and has been listened to
//! (examples/to_wav.rs).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use acid_sound::engine::Engine;
use acid_sound::load::{load_program, load_song};
use acid_sound::song::{parse, write};
use acid_synth::Synth;

fn dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden")
}

fn read(path: &str) -> Result<String, String> {
    match path {
        "Home/sounds/demo.snd" => std::fs::read_to_string(dir().join("demo.snd")).map_err(|e| e.to_string()),
        _ => Err("not found".into()),
    }
}

/// Four seconds of the demo, in 512-sample buffers; the zap starts in
/// buffer 90 (about 2.1 s) and borrows channel 4's second voice.
fn render_demo() -> Vec<u8> {
    let text = std::fs::read_to_string(dir().join("demo.trk")).unwrap();
    let (song, warnings) = load_song(&text, &read).unwrap();
    assert!(warnings.is_empty(), "{warnings:?}");
    let sfx = Arc::new(load_program(&read("Home/sounds/demo.snd").unwrap(), &read).unwrap());
    let zap = sfx.block("zap").unwrap();
    let mut synth = Synth::new();
    let mut engine = Engine::new();
    engine.play_song(&mut synth, 1, 1, Arc::new(song), 0, 0);
    let mut out = vec![0u8; 22050 * 4];
    for (i, chunk) in out.chunks_mut(512).enumerate() {
        if i == 90 {
            assert!(engine.play_sound(&mut synth, 2, sfx.clone(), zap, 40, 0).is_some());
        }
        engine.render(&mut synth, chunk);
    }
    out
}

#[test]
fn demo_matches_the_reference_recording() {
    let actual = render_demo();
    let path = dir().join("demo.u8");
    if std::env::var_os("ACID_SOUND_BLESS").is_some() {
        std::fs::write(&path, &actual).unwrap();
    }
    let expected = std::fs::read(&path).expect("missing tests/golden/demo.u8 (bless it once with ACID_SOUND_BLESS=1)");
    assert_eq!(actual.len(), expected.len(), "total rendered length");
    if let Some(i) = (0..expected.len()).find(|&i| actual[i] != expected[i]) {
        let diffs = (0..expected.len()).filter(|&i| actual[i] != expected[i]).count();
        panic!("{diffs} bytes differ from the reference; first at byte {i}: expected {}, got {}", expected[i], actual[i]);
    }
}

#[test]
fn the_demo_is_not_trivially_silent() {
    let r = render_demo();
    let loud = r.iter().filter(|&&b| b != 128).count();
    assert!(loud > r.len() / 10, "the demo must actually make sound");
}

#[test]
fn rendering_is_deterministic() {
    assert_eq!(render_demo(), render_demo());
}

#[test]
fn the_demo_song_file_is_canonical() {
    let text = std::fs::read_to_string(dir().join("demo.trk")).unwrap();
    assert_eq!(write(&parse(&text).unwrap()), text);
}
```

Create `v3/crates/acid-sound/examples/to_wav.rs`:

```rust
//! Writes a .u8 recording (22050 Hz unsigned 8-bit mono) as a .wav, so a
//! golden can be listened to before it's blessed:
//!   cargo run --manifest-path v3/Cargo.toml -p acid-sound --example to_wav -- IN.u8 OUT.wav

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let [_, input, output] = args.as_slice() else {
        eprintln!("usage: to_wav IN.u8 OUT.wav");
        std::process::exit(2);
    };
    let data = std::fs::read(input).expect("read input");
    let rate: u32 = 22050;
    let mut w = Vec::with_capacity(44 + data.len());
    w.extend_from_slice(b"RIFF");
    w.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
    w.extend_from_slice(b"WAVEfmt ");
    w.extend_from_slice(&16u32.to_le_bytes());
    w.extend_from_slice(&1u16.to_le_bytes()); // PCM
    w.extend_from_slice(&1u16.to_le_bytes()); // mono
    w.extend_from_slice(&rate.to_le_bytes());
    w.extend_from_slice(&rate.to_le_bytes()); // bytes per second
    w.extend_from_slice(&1u16.to_le_bytes()); // block align
    w.extend_from_slice(&8u16.to_le_bytes()); // bits
    w.extend_from_slice(b"data");
    w.extend_from_slice(&(data.len() as u32).to_le_bytes());
    w.extend_from_slice(&data);
    std::fs::write(output, w).expect("write output");
}
```

- [ ] **Step 7: Bless the recording once, and have a person listen**

Run: `ACID_SOUND_BLESS=1 cargo test --manifest-path v3/Cargo.toml -p acid-sound --test golden`
Expected: 4 passed, and `tests/golden/demo.u8` now exists at 88200 bytes.

Run: `cargo run --manifest-path v3/Cargo.toml -p acid-sound --example to_wav -- v3/crates/acid-sound/tests/golden/demo.u8 /tmp/claude-demo.wav`

Ask your human partner to play `/tmp/claude-demo.wav` with `pw-play` or `aplay`. Here's what they should hear:
- a detuned pulse lead with vibrato
- a filtered saw bass
- triangle plucks with arpeggios and a second-column harmony
- a noise hat
- a falling saw "zap" at about 2.1 seconds

Don't commit the recording until they confirm it sounds right. If it doesn't, find the cause (`superpowers:systematic-debugging`), fix it, and bless again.

Then run the golden tests without the variable: `cargo test --manifest-path v3/Cargo.toml -p acid-sound --test golden`. Expected: 4 passed.

- [ ] **Step 8: Commit**

```bash
git add v3/crates/acid-sound
git commit -m "acid-sound: song and program loading; golden demo recording

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 11: The engine inside the kernel

**Files:**
- Modify: `v3/crates/acid-kernel/Cargo.toml` (add `acid-sound = { workspace = true }` to `[dependencies]`)
- Modify: `v3/crates/acid-kernel/src/audio.rs`

**Interfaces:**
- Consumes: the `acid_sound::engine::Engine` API (Task 9), `acid_sound::player::LoadedSong`, `acid_sound::program::Program`
- Produces these `Kernel` methods, which `acid-api` uses in Task 12:
  - `audio_sound_play(&self, task: TaskId, prog: Arc<Program>, block: usize, note: i32) -> Option<u32>`
  - `audio_sound_stop(&self, task: TaskId, id: u32)`
  - `audio_song_play(&self, task: TaskId, handle: u32, song: Arc<LoadedSong>, order: i32, row: i32)`
  - `audio_song_stop(&self, task: TaskId)` and `audio_song_mute(&self, task: TaskId, ch: i32, on: bool)`. Both only act on the caller's own song.
  - `audio_song_update(&self, task: TaskId, handle: u32, song: Arc<LoadedSong>)`
  - `audio_song_position(&self) -> Option<(i32, i32, i32)>`
  - `audio_song_preview(&self, task: TaskId, song: Arc<LoadedSong>, ch: i32, note: i32, inst: i32)`

- [ ] **Step 1: Write the failing kernel tests**

Add these to the end of `mod tests` in `v3/crates/acid-kernel/src/audio.rs`:

```rust
    const FOUR: &str = "acid-track 1\ntitle t\nspeed 2\nsfx-donor 4\ninstrument 01 \"Lead\"  wave saw  adsr 0 0 100 0  duty 50  voice2 detune 6\norder 1  00 loop 0\norder 2  00 loop 0\norder 3  00 loop 0\norder 4  00 loop 0\n\npattern 00 2\nC-4 01 . .. ...\n=== .. . .. ...\n";

    fn song() -> Arc<acid_sound::player::LoadedSong> {
        Arc::new(acid_sound::player::LoadedSong::plain(acid_sound::song::parse(FOUR).unwrap()))
    }

    #[test]
    fn a_song_sounds_the_same_whatever_the_buffer_size() {
        let (a, b) = (kernel(), kernel());
        a.audio_song_play(TaskId(1), 1, song(), 0, 0);
        b.audio_song_play(TaskId(1), 1, song(), 0, 0);
        let whole = render(&a, 1900);
        let mut parts = Vec::new();
        for _ in 0..9 {
            parts.extend(render(&b, 100));
        }
        parts.extend(render(&b, 1000));
        assert!(whole.iter().any(|&s| s != 128), "the song must be audible");
        assert_eq!(whole, parts);
    }

    #[test]
    fn an_exiting_app_stops_its_song_and_sounds() {
        let k = kernel();
        k.audio_song_play(TaskId(1), 1, song(), 0, 0);
        let prog = Arc::new(acid_sound::compile("gate on\nwait 100").unwrap());
        k.audio_sound_play(TaskId(1), prog, 0, 40).unwrap();
        render(&k, 441);
        k.audio_release_owner(TaskId(1));
        assert_eq!(k.audio_song_position(), None);
        assert_eq!(k.audio.state.lock().engine.sound_count(), 0);
    }

    #[test]
    fn only_the_owner_stops_or_mutes_its_song() {
        let k = kernel();
        k.audio_song_play(TaskId(1), 1, song(), 0, 0);
        render(&k, 441);
        k.audio_song_mute(TaskId(2), 1, true);
        k.audio_song_stop(TaskId(2));
        assert!(k.audio_song_position().is_some());
        assert!(!k.audio.state.lock().engine.song_player().unwrap().muted(0));
        k.audio_song_stop(TaskId(1));
        assert_eq!(k.audio_song_position(), None);
    }

    #[test]
    fn sounds_avoid_voices_apps_are_playing() {
        let k = kernel();
        k.audio_note_on(TaskId(1), 7, 40, 80);
        let prog = Arc::new(acid_sound::compile("gate on\nwait 100").unwrap());
        k.audio_sound_play(TaskId(2), prog, 0, 52).unwrap();
        render(&k, 441);
        let st = k.audio.state.lock();
        assert_eq!(st.synth.voice(6).envelope_stage, EnvStage::Attack);
        assert_eq!(st.synth.voice(6).phase_increment, acid_synth::ONA_PHASE_INCREMENT[51]);
    }

    #[test]
    fn preview_plays_through_the_kernel() {
        let k = kernel();
        k.audio_song_preview(TaskId(1), song(), 2, 40, 1);
        assert_eq!(k.audio.state.lock().synth.voice(2).envelope_stage, EnvStage::Attack);
    }
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test --manifest-path v3/Cargo.toml -p acid-kernel audio`
Expected: compile errors, `no method named audio_song_play`.

- [ ] **Step 3: Wire the engine in**

1. Add `acid-sound = { workspace = true }` to `[dependencies]` in `v3/crates/acid-kernel/Cargo.toml`.

2. In `audio.rs`, add these imports:

```rust
use alloc::sync::Arc;

use acid_sound::engine::Engine;
use acid_sound::player::LoadedSong;
use acid_sound::program::Program;
```

3. Extend `AudioState`, and its constructor in `AudioRuntime::new`:

```rust
pub struct AudioState {
    pub(crate) synth: Synth,
    /// Which app last started each voice.
    pub(crate) owners: [Option<TaskId>; NUM_VOICES],
    /// Songs and .snd sounds, ticked from render_audio.
    pub(crate) engine: Engine,
}
```

```rust
            state: Mutex::new(AudioState { synth: Synth::new(), owners: [None; NUM_VOICES], engine: Engine::new() }),
```

4. Add this helper below `voice_index`:

```rust
/// Voices apps are playing directly (acid_play_note), which sounds avoid.
fn busy_voices(a: &AudioState) -> u8 {
    (0..NUM_VOICES)
        .filter(|&v| a.owners[v].is_some() && a.synth.voice(v).envelope_stage != EnvStage::Off)
        .fold(0u8, |m, v| m | 1 << v)
}
```

5. In `audio_release_owner`, add this after the `for` loop, still holding the lock:

```rust
        let st = &mut *a;
        st.engine.release_owner(&mut st.synth, task.0);
```

6. In `render_audio`, render through the engine. Replace the locked block with:

```rust
        let mask = {
            let mut a = self.audio.state.lock();
            let st = &mut *a;
            st.engine.render(&mut st.synth, buf);
            (0..NUM_VOICES)
                .filter(|&v| st.synth.voice(v).envelope_stage != EnvStage::Off)
                .fold(0u32, |m, v| m | (1 << v))
        };
```

7. Add these methods to `impl Kernel`, after `audio_release_owner`:

```rust
    /// Starts a .snd `sound` block on free voices; None when there are none.
    pub fn audio_sound_play(&self, task: TaskId, prog: Arc<Program>, block: usize, note: i32) -> Option<u32> {
        let mut a = self.audio.state.lock();
        let busy = busy_voices(&a);
        let st = &mut *a;
        st.engine.play_sound(&mut st.synth, task.0, prog, block, note, busy)
    }

    /// Stops one of `task`'s own sounds.
    pub fn audio_sound_stop(&self, task: TaskId, id: u32) {
        let mut a = self.audio.state.lock();
        let st = &mut *a;
        st.engine.stop_sound(&mut st.synth, task.0, id);
    }

    /// Plays `song` (replacing any song) from `order`/`row`.
    pub fn audio_song_play(&self, task: TaskId, handle: u32, song: Arc<LoadedSong>, order: i32, row: i32) {
        let mut a = self.audio.state.lock();
        let st = &mut *a;
        st.engine.play_song(&mut st.synth, task.0, handle, song, order, row);
    }

    /// Stops the song, if `task` started it.
    pub fn audio_song_stop(&self, task: TaskId) {
        let mut a = self.audio.state.lock();
        let st = &mut *a;
        if st.engine.song_owner() == Some(task.0) {
            st.engine.stop_song(&mut st.synth);
        }
    }

    pub fn audio_song_update(&self, task: TaskId, handle: u32, song: Arc<LoadedSong>) {
        self.audio.state.lock().engine.update_song(task.0, handle, song);
    }

    pub fn audio_song_position(&self) -> Option<(i32, i32, i32)> {
        self.audio.state.lock().engine.song_position()
    }

    /// Mutes channel `ch` (1-based) of the song, if `task` started it.
    pub fn audio_song_mute(&self, task: TaskId, ch: i32, on: bool) {
        let mut a = self.audio.state.lock();
        let st = &mut *a;
        if st.engine.song_owner() == Some(task.0) {
            st.engine.mute(&mut st.synth, ch, on);
        }
    }

    pub fn audio_song_preview(&self, task: TaskId, song: Arc<LoadedSong>, ch: i32, note: i32, inst: i32) {
        let mut a = self.audio.state.lock();
        let st = &mut *a;
        st.engine.preview(&mut st.synth, task.0, song, ch, note, inst);
    }
```

The tests module already does `use alloc::sync::Arc;`. Remove that line if the compiler warns that it's now a duplicate import through `super::*`.

- [ ] **Step 4: Run the kernel tests**

Run: `cargo test --manifest-path v3/Cargo.toml -p acid-kernel`
Expected: all pass. That includes the existing `commands_match_the_bare_synth`, because an idle engine renders exactly like the bare synth.

- [ ] **Step 5: Commit**

```bash
git add v3/crates/acid-kernel v3/Cargo.lock
git commit -m "Kernel: the sound engine renders inside render_audio; song and sound calls

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 12: App calls (`acid-api`) and Lua bindings

**Files:**
- Modify: `v3/crates/acid-api/Cargo.toml` (add `acid-sound = { workspace = true }` to `[dependencies]`)
- Modify: `v3/crates/acid-api/src/lib.rs`
- Modify: `v3/crates/acid-lua/src/lib.rs`
- Modify: `v3/crates/acid-lua/tests/lua_app.rs`

**Interfaces:**
- Consumes: the Task 11 `Kernel::audio_*` methods, and `acid_sound::load::{load_program, load_song}`
- Produces these `AcidApi` trait methods. All of them have default bodies, so the test fakes in `acid-lua` and `acid-wasm` keep compiling. The defaults return `Err("unsupported")`, `None` or do nothing.
  - `sound_load(&self, src: &str) -> Result<i32, String>`
  - `sound_load_file(&self, path: &str) -> Result<i32, String>`
  - `sound_free(&self, prog: i32)`
  - `sound_play(&self, prog: i32, name: &str, note: i32) -> Option<i32>`, where an empty `name` means the first sound block
  - `sound_stop(&self, id: i32)`
  - `song_load(&self, path: &str) -> Result<(i32, Vec<String>), String>`
  - `song_parse(&self, text: &str) -> Result<(i32, Vec<String>), String>`
  - `song_update(&self, song: i32, text: &str) -> Result<Vec<String>, String>`
  - `song_free(&self, song: i32)`
  - `song_play(&self, song: i32, order: i32, row: i32)`
  - `song_stop(&self)`
  - `song_position(&self) -> Option<(i32, i32, i32)>`
  - `song_mute(&self, ch: i32, on: bool)`
  - `song_preview(&self, song: i32, ch: i32, note: i32, inst: i32)`
- Constants: `SOUND_PROGRAM_MAX = 16` and `SONG_MAX = 4`, both per app.
- Lua globals:
  - `acid_sound_load`, `acid_sound_load_file`, `acid_sound_free`, `acid_sound_play`, `acid_sound_stop`
  - `acid_song_load`, `acid_song_parse`, `acid_song_update`, `acid_song_free`, `acid_song_play`, `acid_song_stop`, `acid_song_position`, `acid_song_mute`, `acid_song_preview`

- [ ] **Step 1: Write the failing `acid-api` tests**

Add these to `mod tests` in `v3/crates/acid-api/src/lib.rs`, next to the audio tests:

```rust
    const FOUR_SONG: &str = "acid-track 1\ntitle t\nspeed 2\nsfx-donor 4\ninstrument 01 \"Lead\"  wave saw  adsr 0 0 100 0  duty 50\norder 1  00 loop 0\norder 2  00 loop 0\norder 3  00 loop 0\norder 4  00 loop 0\n\npattern 00 2\nC-4 01 . .. ...\n... .. . .. ...\n";

    #[test]
    fn sound_load_reports_compile_errors() {
        let (k, rx) = kernel_with_parked_apps();
        let a = api_at(&k, &rx, 0, 30, 10, 10);
        assert_eq!(a.sound_load("wav saw"), Err(String::from("1:1 unknown command 'wav'")));
    }

    #[test]
    fn a_loaded_sound_plays_on_a_voice() {
        let (k, rx) = kernel_with_parked_apps();
        let a = api_at(&k, &rx, 0, 30, 10, 10);
        let p = a.sound_load("sound zap\ngate on\nwait 50\nend\ninstrument i\nend").unwrap();
        assert_eq!(a.sound_play(p, "i", 40), None, "instruments aren't sounds");
        assert_eq!(a.sound_play(p, "nope", 40), None);
        assert_eq!(a.sound_play(99, "", 40), None, "no such program");
        assert!(a.sound_play(p, "", 40).is_some());
        k.render_audio(&mut [0u8; 441]);
        assert_eq!(a.active_voice_count(), 1);
    }

    #[test]
    fn programs_and_songs_are_capped_per_app() {
        let (k, rx) = kernel_with_parked_apps();
        let a = api_at(&k, &rx, 0, 30, 10, 10);
        let ids: Vec<i32> = (0..SOUND_PROGRAM_MAX).map(|_| a.sound_load("wait 1").unwrap()).collect();
        assert_eq!(a.sound_load("wait 1"), Err(String::from("too many")));
        a.sound_free(ids[0]);
        assert!(a.sound_load("wait 1").is_ok());
        for _ in 0..SONG_MAX {
            a.song_parse(FOUR_SONG).unwrap();
        }
        assert_eq!(a.song_parse(FOUR_SONG), Err(String::from("too many")));
    }

    #[test]
    fn a_song_plays_reports_its_place_and_stops() {
        let (k, rx) = kernel_with_parked_apps();
        let a = api_at(&k, &rx, 0, 30, 10, 10);
        let (s, warnings) = a.song_parse(FOUR_SONG).unwrap();
        assert!(warnings.is_empty());
        a.song_play(s, 0, 0);
        k.render_audio(&mut [0u8; 441]);
        assert_eq!(a.song_position(), Some((0, 0, 1)));
        assert_eq!(a.song_update(s, &FOUR_SONG.replace("C-4", "D-4")), Ok(vec![]));
        assert_eq!(a.song_position(), Some((0, 0, 1)), "an update keeps the place");
        a.song_stop();
        assert_eq!(a.song_position(), None);
        assert_eq!(a.song_update(77, FOUR_SONG), Err(String::from("no such song")));
    }

    #[test]
    fn a_missing_script_instrument_is_a_warning() {
        let (k, rx) = kernel_with_parked_apps();
        let a = api_at(&k, &rx, 0, 30, 10, 10);
        let text = FOUR_SONG.replace("instrument 01 \"Lead\"  wave saw  adsr 0 0 100 0  duty 50", "instrument 01 \"S\"  script \"Home/nope.snd\" bass");
        let (_, warnings) = a.song_parse(&text).unwrap();
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].starts_with("instrument 01: Home/nope.snd: "), "{warnings:?}");
    }

    #[test]
    fn song_parse_errors_name_the_line() {
        let (k, rx) = kernel_with_parked_apps();
        let a = api_at(&k, &rx, 0, 30, 10, 10);
        assert_eq!(a.song_parse("nope"), Err(String::from("1: not an acid-track file")));
    }
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test --manifest-path v3/Cargo.toml -p acid-api`
Expected: compile errors, `no method named sound_load` and `cannot find value SOUND_PROGRAM_MAX`.

- [ ] **Step 3: Add the trait methods, the store and the implementation**

1. Add `acid-sound = { workspace = true }` to `[dependencies]` in `v3/crates/acid-api/Cargo.toml`.

2. Add these imports to `lib.rs`, skipping any line that's already there.

```rust
use alloc::format;
use alloc::sync::Arc;

use acid_sound::load::{load_program, load_song};
use acid_sound::player::LoadedSong;
use acid_sound::program::{BlockKind, Program};
```

3. Add this next to `MESH_MAX`:

```rust
/// .snd programs alive per app.
pub const SOUND_PROGRAM_MAX: usize = 16;
/// Songs alive per app.
pub const SONG_MAX: usize = 4;

/// One app's compiled programs and loaded songs. Ids start at 1, are
/// shared between the two maps, and are never reused.
struct SoundStore {
    progs: BTreeMap<i32, Arc<Program>>,
    songs: BTreeMap<i32, Arc<LoadedSong>>,
    next_id: i32,
}

impl SoundStore {
    fn new() -> Self {
        Self { progs: BTreeMap::new(), songs: BTreeMap::new(), next_id: 1 }
    }

    fn take_id(&mut self) -> Result<i32, String> {
        let id = self.next_id;
        self.next_id = id.checked_add(1).ok_or_else(|| String::from("too many"))?;
        Ok(id)
    }
}
```

4. Add these to `trait AcidApi`, directly after `fn configure_osc`:

```rust
    /// Compiles .snd source; `song "PATH"` files load now (spec §5).
    fn sound_load(&self, _src: &str) -> Result<i32, String> { Err(String::from("unsupported")) }
    fn sound_load_file(&self, _path: &str) -> Result<i32, String> { Err(String::from("unsupported")) }
    fn sound_free(&self, _prog: i32) {}
    /// Starts a `sound` block ("" = the first); None when no voice is free.
    fn sound_play(&self, _prog: i32, _name: &str, _note: i32) -> Option<i32> { None }
    fn sound_stop(&self, _id: i32) {}
    /// A song handle plus one warning per script instrument that didn't load.
    fn song_load(&self, _path: &str) -> Result<(i32, Vec<String>), String> { Err(String::from("unsupported")) }
    fn song_parse(&self, _text: &str) -> Result<(i32, Vec<String>), String> { Err(String::from("unsupported")) }
    /// Re-parses into the same handle; a playing copy keeps its place.
    fn song_update(&self, _song: i32, _text: &str) -> Result<Vec<String>, String> { Err(String::from("unsupported")) }
    fn song_free(&self, _song: i32) {}
    fn song_play(&self, _song: i32, _order: i32, _row: i32) {}
    fn song_stop(&self) {}
    fn song_position(&self) -> Option<(i32, i32, i32)> { None }
    /// `ch` is 1-based.
    fn song_mute(&self, _ch: i32, _on: bool) {}
    /// Sounds `note` on channel `ch` (1-based); note 0 is note-off.
    fn song_preview(&self, _song: i32, _ch: i32, _note: i32, _inst: i32) {}
```

5. Add a field to `KernelApi`:

```rust
    /// This app's .snd programs and songs.
    sound: Mutex<SoundStore>,
```

In `KernelApi::new`, add `sound: Mutex::new(SoundStore::new()),`.

6. Add these to `impl KernelApi`, next to `cart_allowed`:

```rust
    /// Reads a file named inside a .trk or .snd: paths there are fsroot-relative.
    fn read_fsroot(&self, path: &str) -> Result<String, String> {
        self.read_text(&format!("v3/fsroot/{path}"))
    }

    fn read_text(&self, path: &str) -> Result<String, String> {
        String::from_utf8(self.fs_read(path)?).map_err(|_| String::from("not text"))
    }

    /// Parses and loads a song's scripts, then stores it under a new handle.
    fn store_song(&self, text: &str) -> Result<(i32, Vec<String>), String> {
        if self.sound.lock().songs.len() >= SONG_MAX {
            return Err(String::from("too many"));
        }
        let (song, warnings) = load_song(text, &|p| self.read_fsroot(p)).map_err(|e| e.to_string())?;
        let mut st = self.sound.lock();
        if st.songs.len() >= SONG_MAX {
            return Err(String::from("too many"));
        }
        let id = st.take_id()?;
        st.songs.insert(id, Arc::new(song));
        Ok((id, warnings))
    }
```

`to_string()` needs `use alloc::string::ToString;`. Add it to the imports if it isn't there.

7. Add these to `impl AcidApi for KernelApi`, directly after `fn configure_osc`:

```rust
    fn sound_load(&self, src: &str) -> Result<i32, String> {
        if self.sound.lock().progs.len() >= SOUND_PROGRAM_MAX {
            return Err(String::from("too many"));
        }
        let prog = load_program(src, &|p| self.read_fsroot(p)).map_err(|e| e.to_string())?;
        let mut st = self.sound.lock();
        if st.progs.len() >= SOUND_PROGRAM_MAX {
            return Err(String::from("too many"));
        }
        let id = st.take_id()?;
        st.progs.insert(id, Arc::new(prog));
        Ok(id)
    }

    fn sound_load_file(&self, path: &str) -> Result<i32, String> {
        let src = self.read_text(path)?;
        self.sound_load(&src)
    }

    fn sound_free(&self, prog: i32) {
        self.sound.lock().progs.remove(&prog);
    }

    fn sound_play(&self, prog: i32, name: &str, note: i32) -> Option<i32> {
        let p = self.sound.lock().progs.get(&prog)?.clone();
        let block = if name.is_empty() {
            p.blocks.iter().position(|b| b.kind == BlockKind::Sound)?
        } else {
            p.block(name)?
        };
        self.ctx.kernel.audio_sound_play(self.ctx.task, p, block, note).map(|id| id as i32)
    }

    fn sound_stop(&self, id: i32) {
        if let Ok(id) = u32::try_from(id) {
            self.ctx.kernel.audio_sound_stop(self.ctx.task, id);
        }
    }

    fn song_load(&self, path: &str) -> Result<(i32, Vec<String>), String> {
        let text = self.read_text(path)?;
        self.store_song(&text)
    }

    fn song_parse(&self, text: &str) -> Result<(i32, Vec<String>), String> {
        self.store_song(text)
    }

    fn song_update(&self, song: i32, text: &str) -> Result<Vec<String>, String> {
        if !self.sound.lock().songs.contains_key(&song) {
            return Err(String::from("no such song"));
        }
        let (loaded, warnings) = load_song(text, &|p| self.read_fsroot(p)).map_err(|e| e.to_string())?;
        let loaded = Arc::new(loaded);
        self.sound.lock().songs.insert(song, loaded.clone());
        self.ctx.kernel.audio_song_update(self.ctx.task, song as u32, loaded);
        Ok(warnings)
    }

    fn song_free(&self, song: i32) {
        self.sound.lock().songs.remove(&song);
    }

    fn song_play(&self, song: i32, order: i32, row: i32) {
        let Some(s) = self.sound.lock().songs.get(&song).cloned() else { return };
        self.ctx.kernel.audio_song_play(self.ctx.task, song as u32, s, order, row);
    }

    fn song_stop(&self) {
        self.ctx.kernel.audio_song_stop(self.ctx.task);
    }

    fn song_position(&self) -> Option<(i32, i32, i32)> {
        self.ctx.kernel.audio_song_position()
    }

    fn song_mute(&self, ch: i32, on: bool) {
        self.ctx.kernel.audio_song_mute(self.ctx.task, ch, on);
    }

    fn song_preview(&self, song: i32, ch: i32, note: i32, inst: i32) {
        let Some(s) = self.sound.lock().songs.get(&song).cloned() else { return };
        self.ctx.kernel.audio_song_preview(self.ctx.task, s, ch, note, inst);
    }
```

`load_program` and `load_song` take `&dyn Fn`. The closures borrow `self` immutably while no store lock is held. The store is locked only before and after loading, never during. Loading reads files, so it must never run while holding the lock.

- [ ] **Step 4: Run the `acid-api` tests**

Run: `cargo test --manifest-path v3/Cargo.toml -p acid-api`
Expected: all pass, 6 of them new.

- [ ] **Step 5: Write the failing Lua binding test**

In `v3/crates/acid-lua/tests/lua_app.rs`, add these overrides inside `impl AcidApi for FakeApi`:

```rust
    fn sound_load(&self, src: &str) -> Result<i32, String> {
        if src == "bad" { Err("1:1 unknown command 'bad'".into()) } else { Ok(3) }
    }
    fn sound_play(&self, prog: i32, name: &str, note: i32) -> Option<i32> {
        self.log(format!("sound_play {prog} {name} {note}"));
        Some(9)
    }
    fn song_parse(&self, _text: &str) -> Result<(i32, Vec<String>), String> {
        Ok((2, vec!["instrument 01: x: not found".into()]))
    }
    fn song_play(&self, song: i32, order: i32, row: i32) {
        self.log(format!("song_play {song} {order} {row}"));
    }
    fn song_position(&self) -> Option<(i32, i32, i32)> {
        Some((1, 2, 3))
    }
```

Add this test:

```rust
#[test]
fn sound_and_song_calls_reach_lua() {
    let api = FakeApi::with_events(vec![]);
    let lua = state(api.clone());
    run(&lua, r#"
        local id, err = acid_sound_load("bad")
        acid_draw_text(tostring(id) .. " " .. err, 0, 0, 0, 0)
        local p = acid_sound_load("gate on")
        acid_draw_text(tostring(acid_sound_play(p)), 0, 0, 0, 0)
        acid_sound_play(p, "zap", 52)
        local s, w = acid_song_parse("x")
        acid_draw_text(s .. " " .. #w .. " " .. w[1], 0, 0, 0, 0)
        acid_song_play(s)
        acid_song_play(s, 4, 8)
        local o, r, t = acid_song_position()
        acid_draw_text(o .. r .. t, 0, 0, 0, 0)
    "#);
    assert_eq!(api.texts(), ["nil 1:1 unknown command 'bad'", "9", "2 1 instrument 01: x: not found", "123"]);
    let calls = api.calls();
    for want in ["sound_play 3  40", "sound_play 3 zap 52", "song_play 2 0 0", "song_play 2 4 8"] {
        assert!(calls.contains(&want.to_string()), "missing {want:?} in {calls:?}");
    }
}
```

Run: `cargo test --manifest-path v3/Cargo.toml -p acid-lua --test lua_app sound_and_song`
Expected: FAIL, because `acid_sound_load` is a nil global.

- [ ] **Step 6: Add the Lua bindings**

In `v3/crates/acid-lua/src/lib.rs`, add the following just before `let a = api.clone(); g.set("acid_now_ms", …`:

```rust
    let a = api.clone();
    g.set("acid_sound_load", lua.create_function(move |lua, src: mlua::String| {
        match a.sound_load(&src.to_string_lossy()) {
            Ok(id) => id.into_lua_multi(lua),
            Err(e) => (Value::Nil, e).into_lua_multi(lua),
        }
    })?)?;
    let a = api.clone();
    g.set("acid_sound_load_file", lua.create_function(move |lua, path: mlua::String| {
        match a.sound_load_file(&path.to_string_lossy()) {
            Ok(id) => id.into_lua_multi(lua),
            Err(e) => (Value::Nil, e).into_lua_multi(lua),
        }
    })?)?;
    let a = api.clone();
    g.set("acid_sound_free", lua.create_function(move |_, p: i32| { a.sound_free(p); Ok(()) })?)?;
    let a = api.clone();
    g.set("acid_sound_play", lua.create_function(
        move |_, (p, name, note): (i32, Option<mlua::String>, Option<i32>)| {
            Ok(a.sound_play(p, &opt_str(name), note.unwrap_or(40)))
        },
    )?)?;
    let a = api.clone();
    g.set("acid_sound_stop", lua.create_function(move |_, id: i32| { a.sound_stop(id); Ok(()) })?)?;
    let a = api.clone();
    g.set("acid_song_load", lua.create_function(move |lua, path: mlua::String| {
        match a.song_load(&path.to_string_lossy()) {
            Ok((id, w)) => (id, lua.create_sequence_from(w)?).into_lua_multi(lua),
            Err(e) => (Value::Nil, e).into_lua_multi(lua),
        }
    })?)?;
    let a = api.clone();
    g.set("acid_song_parse", lua.create_function(move |lua, text: mlua::String| {
        match a.song_parse(&text.to_string_lossy()) {
            Ok((id, w)) => (id, lua.create_sequence_from(w)?).into_lua_multi(lua),
            Err(e) => (Value::Nil, e).into_lua_multi(lua),
        }
    })?)?;
    let a = api.clone();
    g.set("acid_song_update", lua.create_function(move |lua, (s, text): (i32, mlua::String)| {
        match a.song_update(s, &text.to_string_lossy()) {
            Ok(w) => lua.create_sequence_from(w)?.into_lua_multi(lua),
            Err(e) => (Value::Nil, e).into_lua_multi(lua),
        }
    })?)?;
    let a = api.clone();
    g.set("acid_song_free", lua.create_function(move |_, s: i32| { a.song_free(s); Ok(()) })?)?;
    let a = api.clone();
    g.set("acid_song_play", lua.create_function(move |_, (s, o, r): (i32, Option<i32>, Option<i32>)| {
        a.song_play(s, o.unwrap_or(0), r.unwrap_or(0));
        Ok(())
    })?)?;
    let a = api.clone();
    g.set("acid_song_stop", lua.create_function(move |_, ()| { a.song_stop(); Ok(()) })?)?;
    let a = api.clone();
    g.set("acid_song_position", lua.create_function(move |lua, ()| match a.song_position() {
        Some(pos) => pos.into_lua_multi(lua),
        None => Ok(MultiValue::new()),
    })?)?;
    let a = api.clone();
    g.set("acid_song_mute", lua.create_function(move |_, (ch, on): (i32, bool)| { a.song_mute(ch, on); Ok(()) })?)?;
    let a = api.clone();
    g.set("acid_song_preview", lua.create_function(move |_, (s, ch, n, i): (i32, i32, i32, i32)| {
        a.song_preview(s, ch, n, i);
        Ok(())
    })?)?;
```

`opt_str`, `Value`, `MultiValue` and `IntoLuaMulti` are already in scope in this file. `acid_spawn_app` and `acid_task_info` use them.

- [ ] **Step 7: Run the binding test and the whole workspace**

Run: `cargo test --manifest-path v3/Cargo.toml -p acid-lua --test lua_app sound_and_song`
Expected: PASS.

Run: `cargo test --manifest-path v3/Cargo.toml`
Expected: every crate passes, and the existing Lua app suites are unchanged.

- [ ] **Step 8: Commit**

```bash
git add v3/crates/acid-api v3/crates/acid-lua v3/Cargo.lock
git commit -m "API: acid_sound_* and acid_song_* calls, bound into Lua

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 13: Function keys F1 to F12

**Files:**
- Modify: `v3/crates/acid-platform/src/keys.rs`
- Modify: `v3/crates/acid-hosted/src/keymap.rs`
- Modify: `v3/apps/lib/acid_keys.lua`

**Interfaces:**
- Produces: `KEY_F1` to `KEY_F12` (266 to 277) in `acid_platform::keys`, and `AcidKeys.F1` to `AcidKeys.F12` in Lua. The tracker in plan 2 uses them for play, stop and mutes.

- [ ] **Step 1: Write the failing tests**

In `keys.rs` tests, add:

```rust
    #[test]
    fn function_keys_follow_the_named_keys() {
        assert_eq!([KEY_F1, KEY_F2, KEY_F12], [266, 267, 277]);
    }
```

In `keymap.rs`:
- Change `unmapped_keys_are_ignored` to use `KeyCode::F13` instead of `KeyCode::F1`.
- Change `unmapped_keys_never_press_or_release` the same way, in both lines.
- Add this test:

```rust
    #[test]
    fn function_keys_map_with_or_without_shift() {
        assert_eq!(translate_key(KeyCode::F1, false), Some(KEY_F1));
        assert_eq!(translate_key(KeyCode::F12, true), Some(KEY_F12));
        let mut h = HeldKeys::default();
        assert_eq!(h.press(KeyCode::F5, false), Some(KeyEvent { code: KEY_F5, pressed: true }));
        assert_eq!(h.release(KeyCode::F5), Some(KeyEvent { code: KEY_F5, pressed: false }));
    }
```

Run: `cargo test --manifest-path v3/Cargo.toml -p acid-platform -p acid-hosted`
Expected: compile error, `cannot find value KEY_F1`.

- [ ] **Step 2: Implement**

In `keys.rs`, after `KEY_RIGHT`, add:

```rust
pub const KEY_F1: i32 = 266;
pub const KEY_F2: i32 = 267;
pub const KEY_F3: i32 = 268;
pub const KEY_F4: i32 = 269;
pub const KEY_F5: i32 = 270;
pub const KEY_F6: i32 = 271;
pub const KEY_F7: i32 = 272;
pub const KEY_F8: i32 = 273;
pub const KEY_F9: i32 = 274;
pub const KEY_F10: i32 = 275;
pub const KEY_F11: i32 = 276;
pub const KEY_F12: i32 = 277;
```

In `keymap.rs`'s `translate_key` match, before `_ => None`, add:

```rust
        KeyCode::F1 => Some(KEY_F1), KeyCode::F2 => Some(KEY_F2), KeyCode::F3 => Some(KEY_F3),
        KeyCode::F4 => Some(KEY_F4), KeyCode::F5 => Some(KEY_F5), KeyCode::F6 => Some(KEY_F6),
        KeyCode::F7 => Some(KEY_F7), KeyCode::F8 => Some(KEY_F8), KeyCode::F9 => Some(KEY_F9),
        KeyCode::F10 => Some(KEY_F10), KeyCode::F11 => Some(KEY_F11), KeyCode::F12 => Some(KEY_F12),
```

Update that file's `//!` header to say "Anything unmapped (F13 and up, Ctrl/Alt) is ignored."

In `v3/apps/lib/acid_keys.lua`, after `RIGHT = 265,`, add:

```lua
  F1        = 266,
  F2        = 267,
  F3        = 268,
  F4        = 269,
  F5        = 270,
  F6        = 271,
  F7        = 272,
  F8        = 273,
  F9        = 274,
  F10       = 275,
  F11       = 276,
  F12       = 277,
```

- [ ] **Step 3: Run the tests and the whole workspace**

Run: `cargo test --manifest-path v3/Cargo.toml`
Expected: everything passes.

- [ ] **Step 4: Commit**

```bash
git add v3/crates/acid-platform v3/crates/acid-hosted v3/apps/lib/acid_keys.lua
git commit -m "Keys: F1-F12 reach apps (KEY_F1..KEY_F12, AcidKeys.F1..F12)

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 14: Check that it works end to end

**Files:** none changed. This task only verifies.

- [ ] **Step 1: Run the full test suite and clippy**

Run: `cargo test --manifest-path v3/Cargo.toml && cargo clippy --manifest-path v3/Cargo.toml --all-targets`
Expected: every test passes. Clippy should report no new warnings in `acid-sound`, `acid-kernel` or `acid-api`; fix any it reports in those crates.

- [ ] **Step 2: Hear it in the real OS**

Write `v3/fsroot/Tmp/beep.lua`:

```lua
local p = assert(acid_sound_load("wave saw; adsr 0 40 0 80; pitch 70; gate on; repeat 12; pitch -3; wait 1; end; gate off"))
acid_sound_play(p)
```

Run the OS with `cargo run --release --manifest-path v3/Cargo.toml -p acid-os -- --screen 640x480`. Run the script from the Terminal if Terminal can run a Lua file. If it can't, temporarily add the two lines to `v3/apps/hello_acid.lua`'s `on_create` and open Hello Acid.

Expected: a falling saw zap.

Remove the temporary file and any temporary edit afterwards, and don't commit them. Report what you heard to your human partner. If it was silent, use `superpowers:systematic-debugging`.

---

## Self-review against the spec

**Covered by this plan:**

| Spec section | Task |
|---|---|
| §1 Architecture | Tasks 9 and 11 |
| §2.1 Execution model | Task 5 |
| §2.2 File layout | Tasks 3 and 4 |
| §2.3 Statements | Tasks 4 and 5 |
| §2.4 Voices for an instance | Tasks 5 and 8 |
| §2.5 Errors | Tasks 2 and 3 |
| §3 Song format | Tasks 6 and 7 |
| §4 Player | Task 8 |
| §5 API | Task 12 |
| §6 Function keys | Task 13 |
| §8 Testing: units, round trip, golden, kernel | Tasks 1–12 |

**Left for plan 2** (the Acid Tracker app):
- §7, the app itself, plus File Manager `.trk`/`.snd` handling, Terminal `play`, the sample `music/` and `sounds/`, and the manual chapter
- §8's Lua tests and window goldens for the tracker

**Consistency checks:**
- `Player::trigger(synth, ch, ona, Option<i32>, u8)` is used the same way by the engine's preview.
- `Engine::play_sound(.., busy: u8)` is fed by the kernel's `busy_voices`.
- `Program::songs` is filled by `load_program` and read by `Engine::apply_cmds`.
- Song handles are `i32` in the API and `u32` in the kernel and engine; the conversion is `song as u32`.
