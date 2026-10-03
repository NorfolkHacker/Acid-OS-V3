//! The launcher registry -- the Menu's list of launchable apps, which
//! desktop.lua fills from the .app.toml manifests at boot.

use alloc::string::String;
use alloc::vec::Vec;

/// The most apps the launcher can hold.
pub const LAUNCHER_MAX: usize = 48;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LaunchableApp {
    pub path: String,
    pub name: String,
    pub w: i32,
    pub h: i32,
    /// May run more than one window at once (manifest `multi = true`).
    pub multi: bool,
    pub libs: Option<String>,
}

#[derive(Default)]
pub struct Launcher {
    apps: Vec<LaunchableApp>,
}

impl Launcher {
    pub fn new() -> Self {
        Self::default()
    }

    /// False once LAUNCHER_MAX apps are registered.
    pub fn register(&mut self, app: LaunchableApp) -> bool {
        if self.apps.len() >= LAUNCHER_MAX {
            return false;
        }
        self.apps.push(app);
        true
    }

    pub fn count(&self) -> usize {
        self.apps.len()
    }

    pub fn get(&self, i: usize) -> Option<&LaunchableApp> {
        self.apps.get(i)
    }

    /// First entry with exactly this path.
    pub fn by_path(&self, path: &str) -> Option<&LaunchableApp> {
        self.apps.iter().find(|a| a.path == path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(path: &str, multi: bool) -> LaunchableApp {
        LaunchableApp { path: path.into(), name: "N".into(), w: 100, h: 80, multi, libs: None }
    }

    #[test]
    fn registers_up_to_the_cap() {
        let mut l = Launcher::new();
        for i in 0..LAUNCHER_MAX {
            assert!(l.register(app(&alloc::format!("a{i}"), false)));
        }
        assert!(!l.register(app("one-too-many", false)));
        assert_eq!(l.count(), 48);
        assert_eq!(l.get(0).unwrap().path, "a0");
        assert!(l.get(48).is_none());
    }

    #[test]
    fn by_path_finds_the_first_match() {
        let mut l = Launcher::new();
        l.register(app("x", true));
        l.register(app("x", false));
        assert!(l.by_path("x").unwrap().multi);
        assert!(l.by_path("y").is_none());
    }
}
