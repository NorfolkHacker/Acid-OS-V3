//! .snd bytecode. A stack machine over i32: expressions push, commands pop
//! their arguments. Jump targets are indexes into a block's `code`.

use alloc::string::String;
use alloc::sync::Arc;
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
    /// Loaded songs, parallel to `song_paths`; filled by `load::load_program`.
    /// None (or missing) means `play` on that song does nothing.
    pub songs: Vec<Option<Arc<crate::player::LoadedSong>>>,
}

impl Program {
    pub fn block(&self, name: &str) -> Option<usize> {
        self.blocks.iter().position(|b| b.name == name)
    }
}
