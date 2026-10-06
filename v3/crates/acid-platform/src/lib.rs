//! The seam between Acid OS and whatever it runs on. Everything above this
//! crate (gfx, kernel, api) talks to the outside world only through these
//! traits, so a bare-metal or MCU backend is a new implementation of them,
//! not a rewrite.
#![cfg_attr(not(test), no_std)]

extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

pub mod keys;
#[cfg(feature = "std")]
pub mod std_impl;
pub mod sync;

use alloc::{boxed::Box, string::String, sync::Arc, vec::Vec};

/// Level state of the single touch point (the mouse's left button when
/// hosted), polled once per router tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TouchState {
    pub x: i32,
    pub y: i32,
    pub pressed: bool,
}

/// A binary semaphore: `notify` sets it, `wait_timeout` consumes it.
pub trait Signal: Send + Sync {
    fn notify(&self);
    /// Returns true if notified within `ms`, false on timeout.
    fn wait_timeout(&self, ms: u32) -> bool;
}

pub trait Display: Send + Sync {
    /// Shows one full RGB565 frame of `width * height` pixels.
    fn present(&self, pixels: &[u16], width: usize, height: usize);
}

/// One key going down or coming back up. A release carries the same code
/// as its press, so an app tracking held keys sees them pair up even if
/// Shift changed in between. No auto-repeat: a held key is one press,
/// then one release.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyEvent {
    pub code: i32,
    pub pressed: bool,
}

pub trait Input: Send + Sync {
    fn poll_touch(&self) -> TouchState;
    /// Next queued key press or release (Shift already resolved), if any.
    fn poll_key(&self) -> Option<KeyEvent>;
    fn should_quit(&self) -> bool;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FsError {
    NotFound,
    Other(String),
}

pub trait Fs: Send + Sync {
    fn read(&self, path: &str) -> Result<Vec<u8>, FsError>;
    /// Entry names (not paths) in `dir`, sorted.
    fn list(&self, dir: &str) -> Result<Vec<String>, FsError>;
    /// Size in bytes of the file at path.
    fn size(&self, path: &str) -> Result<u64, FsError>;
    /// Create or replace the file at path with `data`.
    fn write(&self, path: &str, data: &[u8]) -> Result<(), FsError>;
    /// Move `from` to `to`.
    fn rename(&self, from: &str, to: &str) -> Result<(), FsError>;
    /// Remove the file at path; a directory is an error.
    fn delete(&self, path: &str) -> Result<(), FsError>;
    /// Whether `path` really lands under `dir` (both relative to the
    /// root), wherever symlinks on the way point. The default only
    /// compares spellings, so a fake file system with no links stays simple.
    /// A case-insensitive `Fs` (such as FAT) must override this, or the
    /// lock can be bypassed by spelling.
    fn resolves_into(&self, path: &str, dir: &str) -> bool {
        path == dir || path.strip_prefix(dir).is_some_and(|rest| rest.starts_with('/'))
    }
}

/// The OS's two file roots (spec §11.2, §13.2).
pub const FS_ROOTS: [&str; 2] = ["v3/apps", "v3/fsroot"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpawnError;

pub type TaskFn = Box<dyn FnOnce() + Send + 'static>;

/// Wall-clock local time (year/month/day/hour/min/sec). The Clock app reads
/// it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalTime {
    pub year: i32,
    pub month: i32,
    pub day: i32,
    pub hour: i32,
    pub min: i32,
    pub sec: i32,
}

/// Host name, IPv4 address and link state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkInfo {
    pub host: String,
    pub ip: String,
    pub connected: bool,
}

impl NetworkInfo {
    /// The fallback when no interface could be found.
    pub fn unknown() -> Self {
        Self { host: String::from("unknown"), ip: String::from("none"), connected: false }
    }
}

/// One thread's row for the task monitor. `state` is the scheduler's one-letter state and
/// `cpu_ms` the CPU time used so far.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadSample {
    pub id: u64,
    pub name: String,
    pub state: char,
    pub cpu_ms: u64,
}

/// What a host cart folder entry is (spec §14.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CartStat {
    pub is_dir: bool,
    pub size: u64,
}

/// The largest cart file the host folders will hand over.
pub const CART_MAX_BYTES: u64 = 256 * 1024;

pub trait Platform: Send + Sync {
    fn spawn(&self, name: &str, f: TaskFn) -> Result<(), SpawnError>;
    fn now_ms(&self) -> u64;
    fn sleep_ms(&self, ms: u32);
    fn new_signal(&self) -> Arc<dyn Signal>;
    fn display(&self) -> &dyn Display;
    fn input(&self) -> &dyn Input;
    fn fs(&self) -> &dyn Fs;

    /// Local wall-clock time. Backends without a clock report the epoch.
    fn local_time(&self) -> LocalTime {
        LocalTime { year: 1970, month: 1, day: 1, hour: 0, min: 0, sec: 0 }
    }

    /// Memory in use, in KiB ; -1 when unknown.
    fn mem_used_kb(&self) -> i64 {
        -1
    }

    fn network_info(&self) -> NetworkInfo {
        NetworkInfo::unknown()
    }

    fn thread_samples(&self) -> Vec<ThreadSample> {
        Vec::new()
    }

    /// Host folders that hold `.cart` programs (only those that exist). Default: none.
    fn cart_roots(&self) -> Vec<String> {
        Vec::new()
    }
    fn cart_list(&self, _dir: &str) -> Result<Vec<String>, FsError> {
        Err(FsError::NotFound)
    }
    fn cart_stat(&self, _path: &str) -> Result<CartStat, FsError> {
        Err(FsError::NotFound)
    }
    fn cart_read(&self, _path: &str) -> Result<Vec<u8>, FsError> {
        Err(FsError::NotFound)
    }
    /// Restart the whole OS from its boot screen. Returns false if this
    /// platform can't (the default); on success it doesn't return.
    fn restart(&self) -> bool {
        false
    }
}
