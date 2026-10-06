//! The Acid OS kernel: window registry, input router, compositor and app
//! spawning. VM-independent: apps are started through an AppRunner the
//! boot code installs (acid-lua today).
#![cfg_attr(not(test), no_std)]

extern crate alloc;

pub mod audio;
pub mod event;
pub mod fs_path;
mod kernel;
pub mod launcher;
pub mod layout;
pub mod manifest;
pub mod overlay;
pub mod placement;
pub mod router;
#[cfg(test)]
mod test_support;
pub mod tasks;
pub mod theme;
pub mod window;
mod windows_api;

pub use windows_api::WindowInfo;
pub use kernel::{AppContext, AppRunner, Kernel, SpawnRequest, app_is_cart, manifest_says_cart};

/// The kernel's own id for one spawned app.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TaskId(pub u32);
