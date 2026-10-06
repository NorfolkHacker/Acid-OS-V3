//! The Acid OS sound engine: the .snd audio language (compiler and VM),
//! the .trk song format and player, and the engine the kernel ticks at
//! 50 Hz inside the audio render. Integer-only and no_std.
#![cfg_attr(not(test), no_std)]

extern crate alloc;

pub mod lexer;
pub mod pitch;

pub use lexer::CompileError;

/// Samples per engine tick: 22050 Hz / 50 Hz, PAL timing as in GoatTracker.
pub const TICK_SAMPLES: u32 = acid_synth::SAMPLE_RATE / 50;

pub mod compile;
pub mod program;

pub use compile::compile;

pub mod vm;
pub mod song;
