//! The std-backed pieces shared by every hosted Platform (acid-hosted and
//! acid-testkit), so neither has its own copy.

use alloc::{string::String, string::ToString, vec::Vec};
use std::path::PathBuf;
use std::sync::{Condvar, Mutex};
use std::time::Duration;

pub mod carts;
pub mod sysinfo;

use crate::{Fs, FsError, Signal, SpawnError, TaskFn};

#[derive(Default)]
pub struct StdSignal {
    flag: Mutex<bool>,
    cv: Condvar,
}

impl StdSignal {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Signal for StdSignal {
    fn notify(&self) {
        *self.flag.lock().unwrap() = true;
        self.cv.notify_one();
    }

    fn wait_timeout(&self, ms: u32) -> bool {
        let guard = self.flag.lock().unwrap();
        let (mut guard, _) = self
            .cv
            .wait_timeout_while(guard, Duration::from_millis(ms as u64), |set| !*set)
            .unwrap();
        let was_set = *guard;
        *guard = false;
        was_set
    }
}

/// Files under `root`, addressed by repo-relative paths.
pub struct StdFs {
    root: PathBuf,
}

impl StdFs {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Where `path` really lives: the path itself when it exists (symlinks
    /// resolved), otherwise its parent directory resolved plus the final name.
    fn real_location(&self, path: &str) -> Option<PathBuf> {
        let full = self.root.join(path);
        if let Ok(p) = std::fs::canonicalize(&full) {
            return Some(p);
        }
        // A dangling symlink exists; writing through it would create its target.
        if std::fs::symlink_metadata(&full).is_ok() {
            return None;
        }
        let parent = std::fs::canonicalize(full.parent()?).ok()?;
        Some(parent.join(full.file_name()?))
    }

    /// Spec §13.2: a write, rename or delete must really land inside one of
    /// the two roots, wherever a symlink on the way points.
    fn may_change(&self, path: &str) -> Result<(), FsError> {
        let real = self.real_location(path).ok_or_else(|| FsError::Other("bad path".into()))?;
        let inside = crate::FS_ROOTS
            .iter()
            .any(|r| std::fs::canonicalize(self.root.join(r)).is_ok_and(|root| real.starts_with(&root)));
        if inside { Ok(()) } else { Err(FsError::Other("bad path".into())) }
    }
}

pub(crate) fn map_io(e: std::io::Error) -> FsError {
    if e.kind() == std::io::ErrorKind::NotFound {
        FsError::NotFound
    } else {
        FsError::Other(e.to_string())
    }
}

impl Fs for StdFs {
    fn resolves_into(&self, path: &str, dir: &str) -> bool {
        let (Some(real), Ok(target)) = (self.real_location(path), std::fs::canonicalize(self.root.join(dir))) else { return false };
        real.starts_with(&target)
    }

    fn read(&self, path: &str) -> Result<Vec<u8>, FsError> {
        std::fs::read(self.root.join(path)).map_err(map_io)
    }

    fn list(&self, dir: &str) -> Result<Vec<String>, FsError> {
        let mut names: Vec<String> = std::fs::read_dir(self.root.join(dir))
            .map_err(map_io)?
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        Ok(names)
    }

    fn size(&self, path: &str) -> Result<u64, FsError> {
        std::fs::metadata(self.root.join(path)).map(|m| m.len()).map_err(map_io)
    }

    fn write(&self, path: &str, data: &[u8]) -> Result<(), FsError> {
        self.may_change(path)?;
        std::fs::write(self.root.join(path), data).map_err(map_io)
    }

    fn rename(&self, from: &str, to: &str) -> Result<(), FsError> {
        let bad = || FsError::Other("bad path".into());
        if crate::FS_ROOTS.contains(&from) || crate::FS_ROOTS.contains(&to) {
            return Err(bad());
        }
        self.may_change(from)?;
        self.may_change(to)?;
        // The check resolved a link's target but rename moves (or replaces)
        // the link itself, on either side.
        for p in [from, to] {
            if std::fs::symlink_metadata(self.root.join(p)).is_ok_and(|m| m.file_type().is_symlink()) {
                return Err(bad());
            }
        }
        // A root under another spelling (a link to it, say) is still a root.
        let roots: Vec<PathBuf> =
            crate::FS_ROOTS.iter().filter_map(|r| std::fs::canonicalize(self.root.join(r)).ok()).collect();
        for p in [from, to] {
            if self.real_location(p).is_some_and(|real| roots.contains(&real)) {
                return Err(bad());
            }
        }
        std::fs::rename(self.root.join(from), self.root.join(to)).map_err(map_io)
    }

    fn delete(&self, path: &str) -> Result<(), FsError> {
        self.may_change(path)?;
        let full = self.root.join(path);
        if std::fs::metadata(&full).map_err(map_io)?.is_dir() {
            return Err(FsError::Other("is a directory".into()));
        }
        std::fs::remove_file(&full).map_err(map_io)
    }
}

pub fn std_spawn(name: &str, f: TaskFn) -> Result<(), SpawnError> {
    std::thread::Builder::new()
        .name(name.to_string())
        .spawn(f)
        .map(|_| ())
        .map_err(|_| SpawnError)
}

#[cfg(test)]
mod fs_tests {
    use super::*;

    #[test]
    fn std_fs_size_is_the_file_length() {
        let dir = std::env::temp_dir().join(format!("acid-fs-size-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("f.txt"), b"hello").unwrap();
        let fs = StdFs::new(&dir);
        assert_eq!(fs.size("f.txt"), Ok(5));
        assert_eq!(fs.size("missing.txt"), Err(FsError::NotFound));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[cfg(unix)]
    fn os_tree(tag: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!("acid-fs-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("v3/apps")).unwrap();
        std::fs::create_dir_all(root.join("v3/fsroot/Home")).unwrap();
        std::fs::create_dir_all(root.join("outside")).unwrap();
        std::os::unix::fs::symlink("../apps", root.join("v3/fsroot/Source")).unwrap();
        std::os::unix::fs::symlink(root.join("outside"), root.join("v3/fsroot/Out")).unwrap();
        root
    }

    #[cfg(unix)]
    #[test]
    fn writes_follow_links_into_the_roots_and_refuse_links_out() {
        let root = os_tree("links");
        let fs = StdFs::new(&root);
        assert_eq!(fs.write("v3/fsroot/Home/new.txt", b"hi"), Ok(()), "a new file in a real directory");
        assert_eq!(std::fs::read(root.join("v3/fsroot/Home/new.txt")).unwrap(), b"hi");
        assert_eq!(fs.write("v3/fsroot/Source/x.lua", b"-- x"), Ok(()), "through the Source link into the apps root");
        assert!(root.join("v3/apps/x.lua").exists());
        let bad = Err(FsError::Other("bad path".into()));
        assert_eq!(fs.write("v3/fsroot/Out/evil.txt", b"x"), bad, "a link out of the roots is refused");
        assert!(!root.join("outside/evil.txt").exists());
        std::fs::write(root.join("outside/keep.txt"), b"k").unwrap();
        assert_eq!(fs.rename("v3/fsroot/Home/new.txt", "v3/fsroot/Out/moved.txt"), bad, "rename out is refused");
        assert_eq!(fs.delete("v3/fsroot/Out/keep.txt"), bad, "delete through a link out is refused");
        assert!(root.join("outside/keep.txt").exists());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn resolves_into_follows_links() {
        let root = os_tree("resolves");
        let fs = StdFs::new(&root);
        std::fs::write(root.join("v3/apps/a.lua"), b"x").unwrap();
        assert!(fs.resolves_into("v3/fsroot/Source/a.lua", "v3/apps"), "a file through a link");
        assert!(fs.resolves_into("v3/fsroot/Source/new.lua", "v3/apps"), "a file not there yet");
        assert!(fs.resolves_into("v3/apps/a.lua", "v3/apps"));
        assert!(!fs.resolves_into("v3/fsroot/Home/a.lua", "v3/apps"), "outside it");
        assert!(!fs.resolves_into("v3/fsroot/Out/a.lua", "v3/apps"), "a link elsewhere");
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn dangling_links_roots_and_links_are_refused() {
        let root = os_tree("fix1");
        let fs = StdFs::new(&root);
        let bad = Err(FsError::Other("bad path".into()));
        std::os::unix::fs::symlink(root.join("outside/new.txt"), root.join("v3/fsroot/Home/dl")).unwrap();
        assert_eq!(fs.write("v3/fsroot/Home/dl", b"x"), bad, "dangling link out");
        assert!(!root.join("outside/new.txt").exists());
        assert_eq!(fs.rename("v3/apps", "v3/fsroot/Home/x"), bad, "a root cannot be moved");
        assert_eq!(fs.rename("v3/fsroot", "v3/fsroot/Home/y"), bad);
        assert_eq!(fs.rename("v3/fsroot/Home", "v3/apps"), bad, "nor replaced");
        assert!(root.join("v3/apps").is_dir() && root.join("v3/fsroot").is_dir());
        assert_eq!(fs.rename("v3/fsroot/Source", "v3/fsroot/Home/app"), bad, "a link is not moved");
        assert!(root.join("v3/fsroot/Source").exists());
        fs.write("v3/fsroot/Home/x", b"x").unwrap();
        assert_eq!(fs.rename("v3/fsroot/Home/x", "v3/fsroot/Source"), bad, "a link is not replaced");
        assert!(std::fs::symlink_metadata(root.join("v3/fsroot/Source")).unwrap().file_type().is_symlink());
        assert!(root.join("v3/fsroot/Home/x").exists(), "the source stays put");
        std::os::unix::fs::symlink(root.join("v3/apps"), root.join("v3/fsroot/Home/appsroot")).unwrap();
        assert_eq!(fs.rename("v3/fsroot/Home/x", "v3/fsroot/Home/appsroot/."), bad, "a root under another spelling");
        assert!(root.join("v3/fsroot/Home/x").exists());
        std::fs::write(root.join("v3/apps/y.lua"), b"y").unwrap();
        std::os::unix::fs::symlink(root.join("v3/apps/y.lua"), root.join("v3/fsroot/Home/ylink")).unwrap();
        assert_eq!(fs.rename("v3/fsroot/Home/x", "v3/fsroot/Home/ylink"), bad, "a link to a file is not replaced");
        assert!(std::fs::symlink_metadata(root.join("v3/fsroot/Home/ylink")).unwrap().file_type().is_symlink());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn rename_and_delete_inside_the_roots() {
        let root = os_tree("ops");
        let fs = StdFs::new(&root);
        fs.write("v3/fsroot/Home/a.txt", b"a").unwrap();
        assert_eq!(fs.rename("v3/fsroot/Home/a.txt", "v3/fsroot/Home/b.txt"), Ok(()));
        assert_eq!(fs.read("v3/fsroot/Home/b.txt"), Ok(b"a".to_vec()));
        assert_eq!(fs.delete("v3/fsroot/Home/b.txt"), Ok(()));
        assert_eq!(fs.delete("v3/fsroot/Home/b.txt"), Err(FsError::NotFound));
        assert_eq!(fs.delete("v3/fsroot/Home"), Err(FsError::Other("is a directory".into())));
        assert_eq!(fs.write("v3/elsewhere.txt", b"x"), Err(FsError::Other("bad path".into())), "outside both roots");
        std::fs::remove_dir_all(&root).unwrap();
    }
}
