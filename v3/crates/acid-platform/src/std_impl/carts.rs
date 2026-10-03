//! Read-only access to the host folders where `.cart` programs live (spec §14.4).

use alloc::{string::String, string::ToString, vec::Vec};
use std::io::Read;
use std::path::{Path, PathBuf};

use super::map_io;
use crate::{CART_MAX_BYTES, CartStat, FsError};

pub struct HostCarts {
    fs_root: PathBuf,
    roots: Vec<String>,
}

fn bad_path() -> FsError {
    FsError::Other("bad path".to_string())
}

impl HostCarts {
    pub fn with_roots(fs_root: impl Into<PathBuf>, roots: Vec<String>) -> HostCarts {
        HostCarts { fs_root: fs_root.into(), roots }
    }

    /// The standard folders: the repo's carts, the user's, and removable media.
    pub fn from_env(fs_root: impl Into<PathBuf>) -> HostCarts {
        let mut roots: Vec<String> = alloc::vec!["v3/carts".to_string()];
        if let Ok(home) = std::env::var("HOME") {
            if !home.is_empty() {
                roots.push(std::format!("{home}/carts"));
            }
        }
        roots.extend(["/media", "/mnt", "/run/media"].map(String::from));
        HostCarts::with_roots(fs_root, roots)
    }

    /// A relative path lands under `fs_root`; an absolute one is used as is.
    fn resolve(&self, path: &str) -> PathBuf {
        self.fs_root.join(path)
    }

    /// Only the folders that exist, in order.
    pub fn roots(&self) -> Vec<String> {
        self.roots.iter().filter(|r| self.resolve(r).is_dir()).cloned().collect()
    }

    /// Spec §14.4: `path` must not be a symlink and must really lie inside a root.
    fn inside(&self, path: &str) -> Result<PathBuf, FsError> {
        let full = self.resolve(path);
        match std::fs::symlink_metadata(&full) {
            // A configured root may itself be a symlink (/media on some distros);
            // containment is still checked by canonicalizing both sides.
            Ok(m) if m.file_type().is_symlink() && !self.roots.iter().any(|r| r == path) => {
                return Err(bad_path());
            }
            Ok(_) => {}
            Err(e) => {
                // A missing file is only "not found" if it would sit inside a root.
                let real = full.parent().and_then(|p| std::fs::canonicalize(p).ok()).ok_or_else(bad_path)?;
                return if self.in_a_root(&real) { Err(map_io(e)) } else { Err(bad_path()) };
            }
        }
        let real = std::fs::canonicalize(&full).map_err(map_io)?;
        if self.in_a_root(&real) { Ok(real) } else { Err(bad_path()) }
    }

    fn in_a_root(&self, real: &Path) -> bool {
        self.roots
            .iter()
            .any(|r| std::fs::canonicalize(self.resolve(r)).is_ok_and(|root| real.starts_with(&root)))
    }

    pub fn list(&self, dir: &str) -> Result<Vec<String>, FsError> {
        let real = self.inside(dir)?;
        let mut names: Vec<String> = std::fs::read_dir(real)
            .map_err(map_io)?
            .filter_map(|e| e.ok())
            // file_type() does not follow links: symlinks, FIFOs and devices drop out.
            .filter(|e| e.file_type().is_ok_and(|t| t.is_file() || t.is_dir()))
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        Ok(names)
    }

    pub fn stat(&self, path: &str) -> Result<CartStat, FsError> {
        let m = std::fs::metadata(self.inside(path)?).map_err(map_io)?;
        if !m.is_file() && !m.is_dir() {
            return Err(bad_path());
        }
        Ok(CartStat { is_dir: m.is_dir(), size: m.len() })
    }

    pub fn read(&self, path: &str) -> Result<Vec<u8>, FsError> {
        let real = self.inside(path)?;
        let m = std::fs::metadata(&real).map_err(map_io)?;
        // A FIFO or device reports length 0 and would block or never end.
        if !m.is_file() {
            return Err(bad_path());
        }
        if m.len() > CART_MAX_BYTES {
            return Err(FsError::Other("too big".to_string()));
        }
        // Cap the read itself too, so a growing file can't exceed the limit.
        let mut data = Vec::new();
        std::fs::File::open(real)
            .map_err(map_io)?
            .take(CART_MAX_BYTES + 1)
            .read_to_end(&mut data)
            .map_err(map_io)?;
        if data.len() as u64 > CART_MAX_BYTES {
            return Err(FsError::Other("too big".to_string()));
        }
        Ok(data)
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::FsError;

    fn tree(tag: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!("acid-carts-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("v3/carts/games")).unwrap();
        std::fs::create_dir_all(root.join("elsewhere")).unwrap();
        std::fs::write(root.join("v3/carts/b.cart"), b"-- name: B\n").unwrap();
        std::fs::write(root.join("v3/carts/a.cart"), b"-- name: A\n").unwrap();
        std::fs::write(root.join("v3/carts/big.cart"), vec![b'-'; 257 * 1024]).unwrap();
        std::fs::write(root.join("elsewhere/secret.cart"), b"x").unwrap();
        std::os::unix::fs::symlink(root.join("elsewhere"), root.join("v3/carts/escape")).unwrap();
        root
    }

    #[test]
    fn host_cart_folders_stay_inside_their_roots() {
        let root = tree("a");
        let c = HostCarts::with_roots(&root, vec!["v3/carts".into(), "/no/such/dir".into()]);
        assert_eq!(c.roots(), vec!["v3/carts".to_string()], "only existing folders");
        assert_eq!(c.list("v3/carts").unwrap(), vec!["a.cart", "b.cart", "big.cart", "games"], "sorted, symlink left out");
        assert_eq!(c.stat("v3/carts/a.cart").unwrap(), CartStat { is_dir: false, size: 11 });
        assert_eq!(c.stat("v3/carts/games").unwrap().is_dir, true);
        assert_eq!(c.read("v3/carts/a.cart").unwrap(), b"-- name: A\n");
        let bad = Err(FsError::Other("bad path".into()));
        assert_eq!(c.read("v3/carts/escape/secret.cart"), bad.clone(), "through a symlink");
        assert_eq!(c.stat("v3/carts/escape"), Err(FsError::Other("bad path".into())), "the link itself");
        assert_eq!(c.read("v3/carts/../elsewhere/secret.cart"), bad.clone(), ".. out of the root");
        assert_eq!(c.list("elsewhere"), Err(FsError::Other("bad path".into())), "not a root");
        assert_eq!(c.read("v3/carts/big.cart"), Err(FsError::Other("too big".into())));
        assert_eq!(c.read("v3/carts/missing.cart"), Err(FsError::NotFound));
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn in_root_symlinks_are_refused_and_symlinked_roots_work() {
        let root = tree("b");
        std::os::unix::fs::symlink(root.join("v3/carts/games"), root.join("v3/carts/inlink")).unwrap();
        std::os::unix::fs::symlink(root.join("v3/carts"), root.join("rootlink")).unwrap();
        let c = HostCarts::with_roots(&root, vec!["v3/carts".into(), "rootlink".into()]);
        let bad = FsError::Other("bad path".into());
        assert_eq!(c.stat("v3/carts/inlink"), Err(bad.clone()), "link inside a root");
        assert_eq!(c.list("v3/carts/inlink"), Err(bad.clone()));
        assert_eq!(c.roots(), vec!["v3/carts".to_string(), "rootlink".to_string()]);
        assert_eq!(c.list("rootlink").unwrap(), vec!["a.cart", "b.cart", "big.cart", "games"]);
        assert_eq!(c.read("rootlink/a.cart").unwrap(), b"-- name: A\n");
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn fifos_are_not_readable_or_listed() {
        let root = tree("c");
        let fifo = root.join("v3/carts/pipe.cart");
        assert!(std::process::Command::new("mkfifo").arg(&fifo).status().unwrap().success());
        let c = HostCarts::with_roots(&root, vec!["v3/carts".into()]);
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let bad = FsError::Other("bad path".into());
            let r = (c.read("v3/carts/pipe.cart"), c.stat("v3/carts/pipe.cart"), c.list("v3/carts").unwrap());
            let _ = tx.send((r.0 == Err(bad.clone()), r.1 == Err(bad), r.2));
        });
        let (read_bad, stat_bad, names) = rx.recv_timeout(std::time::Duration::from_secs(5)).expect("hung on a FIFO");
        assert!(read_bad && stat_bad);
        assert!(!names.contains(&"pipe.cart".to_string()), "special files left out");
        std::fs::remove_dir_all(&root).unwrap();
    }
}
