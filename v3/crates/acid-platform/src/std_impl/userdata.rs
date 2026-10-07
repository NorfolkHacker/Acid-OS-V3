//! The user directory: where a user's own files live, outside the
//! repository, so that `git pull` or a fresh clone upgrades the OS without
//! touching them. StdFs::with_user_dir serves it as Home, Tmp and the
//! installed carts in Apps.
//!
//! The repository's `v3/fsroot/Home` is the starter set. `prepare` copies
//! each of its files into the user's Home once: on the first run that is
//! all of them (with whatever the user had saved there before this
//! existed), and after an upgrade it is only the new ones. A file the user
//! changed or deleted is never copied again. Carts installed into
//! `v3/apps` before this existed are copied the same way.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use std::collections::BTreeSet;
use std::io;
use std::path::{Path, PathBuf};

pub const HOME: &str = "Home";
pub const TMP: &str = "Tmp";
pub const APPS: &str = "apps";
/// What `prepare` has copied already, one path a line ("Home/notes.txt",
/// "apps/x.lua"), so it never copies a file twice.
pub const SEEDED: &str = ".seeded";

/// Where the user directory goes: `$ACID_OS_DATA`, else
/// `$XDG_DATA_HOME/acid-os`, else `~/.local/share/acid-os`.
pub fn default_dir() -> Option<PathBuf> {
    let var = |k: &str| std::env::var_os(k).filter(|v| !v.is_empty()).map(PathBuf::from);
    if let Some(d) = var("ACID_OS_DATA") {
        return Some(d);
    }
    if let Some(d) = var("XDG_DATA_HOME") {
        return Some(d.join("acid-os"));
    }
    var("HOME").map(|h| h.join(".local/share/acid-os"))
}

/// Every file under `dir`, as paths relative to it, symlinks left out.
fn files_under(dir: &Path, rel: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir.join(rel)) else { return };
    for e in rd.flatten() {
        let Ok(t) = e.file_type() else { continue };
        let r = rel.join(e.file_name());
        if t.is_dir() {
            files_under(dir, &r, out);
        } else if t.is_file() {
            out.push(r);
        }
    }
}

/// Whether a manifest is one Load Cart wrote (`source = cart`), read as
/// acid-kernel's manifest_says_cart reads it.
fn says_cart(text: &str) -> bool {
    text.lines().any(|l| {
        let mut kv = l.splitn(2, '=');
        kv.next().is_some_and(|k| k.trim().eq_ignore_ascii_case("source"))
            && kv.next().is_some_and(|v| v.trim().trim_matches(['"', '\'']).eq_ignore_ascii_case("cart"))
    })
}

/// Copies `from` to `to` (making its folder) unless `to` already exists.
fn copy_new(from: &Path, to: &Path) -> io::Result<()> {
    if std::fs::symlink_metadata(to).is_ok() {
        return Ok(());
    }
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::copy(from, to).map(|_| ())
}

/// Makes `user` ready: its folders, then the starter files and installed
/// carts from the repository at `root` that it hasn't had yet.
pub fn prepare(root: &Path, user: &Path) -> io::Result<()> {
    for d in [HOME, TMP, APPS] {
        std::fs::create_dir_all(user.join(d))?;
    }
    let record = user.join(SEEDED);
    let mut seeded: BTreeSet<String> = match std::fs::read_to_string(&record) {
        Ok(t) => t.lines().map(String::from).collect(),
        Err(e) if e.kind() == io::ErrorKind::NotFound => BTreeSet::new(),
        Err(e) => return Err(e),
    };
    let before = seeded.len();
    let home = root.join("v3/fsroot/Home");
    let mut files = Vec::new();
    files_under(&home, Path::new(""), &mut files);
    for rel in files {
        let key = format!("{HOME}/{}", rel.to_string_lossy());
        if seeded.insert(key) {
            copy_new(&home.join(&rel), &user.join(HOME).join(&rel))?;
        }
    }
    let apps = root.join("v3/apps");
    if let Ok(rd) = std::fs::read_dir(&apps) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            let Some(stem) = name.strip_suffix(".app.toml") else { continue };
            if !std::fs::read_to_string(e.path()).is_ok_and(|t| says_cart(&t)) {
                continue;
            }
            for file in [name.clone(), format!("{stem}.lua"), format!("{stem}.wasm")] {
                let from = apps.join(&file);
                if from.is_file() && seeded.insert(format!("{APPS}/{file}")) {
                    copy_new(&from, &user.join(APPS).join(&file))?;
                }
            }
        }
    }
    if seeded.len() != before {
        let mut text: String = seeded.iter().map(|s| format!("{s}\n")).collect();
        if text.is_empty() {
            text.push('\n');
        }
        std::fs::write(record, text)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(tag: &str) -> (PathBuf, PathBuf) {
        let base = std::env::temp_dir().join(format!("acid-userdata-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let root = base.join("repo");
        std::fs::create_dir_all(root.join("v3/fsroot/Home/music")).unwrap();
        std::fs::create_dir_all(root.join("v3/apps")).unwrap();
        std::fs::write(root.join("v3/fsroot/Home/notes.txt"), "shipped").unwrap();
        std::fs::write(root.join("v3/fsroot/Home/music/a.trk"), "song").unwrap();
        std::fs::write(root.join("v3/apps/editor.app.toml"), "name = Editor\n").unwrap();
        std::fs::write(root.join("v3/apps/editor.lua"), "-- editor").unwrap();
        std::fs::write(root.join("v3/apps/mine.app.toml"), "name = Mine\nsource = cart\n").unwrap();
        std::fs::write(root.join("v3/apps/mine.lua"), "-- mine").unwrap();
        (root, base.join("user"))
    }

    fn read(p: PathBuf) -> String {
        std::fs::read_to_string(p).unwrap()
    }

    #[test]
    fn the_first_run_copies_the_starter_files_and_installed_carts() {
        let (root, user) = tree("first");
        prepare(&root, &user).unwrap();
        assert_eq!(read(user.join("Home/notes.txt")), "shipped");
        assert_eq!(read(user.join("Home/music/a.trk")), "song");
        assert!(user.join("Tmp").is_dir());
        assert_eq!(read(user.join("apps/mine.lua")), "-- mine", "an installed cart moves with the user");
        assert!(!user.join("apps/editor.lua").exists(), "the OS's own apps stay in the repository");
        let _ = std::fs::remove_dir_all(user.parent().unwrap());
    }

    #[test]
    fn later_runs_add_only_new_starter_files() {
        let (root, user) = tree("later");
        prepare(&root, &user).unwrap();
        std::fs::write(user.join("Home/notes.txt"), "my notes").unwrap();
        std::fs::remove_file(user.join("Home/music/a.trk")).unwrap();
        std::fs::write(root.join("v3/fsroot/Home/notes.txt"), "upgraded").unwrap();
        std::fs::write(root.join("v3/fsroot/Home/new.txt"), "new in this version").unwrap();
        prepare(&root, &user).unwrap();
        assert_eq!(read(user.join("Home/notes.txt")), "my notes", "an edited file is never replaced");
        assert!(!user.join("Home/music/a.trk").exists(), "a deleted file stays deleted");
        assert_eq!(read(user.join("Home/new.txt")), "new in this version", "a new starter file arrives");
        let _ = std::fs::remove_dir_all(user.parent().unwrap());
    }

    #[test]
    fn a_file_the_user_already_has_is_kept() {
        let (root, user) = tree("kept");
        std::fs::create_dir_all(user.join("Home")).unwrap();
        std::fs::write(user.join("Home/notes.txt"), "mine first").unwrap();
        prepare(&root, &user).unwrap();
        assert_eq!(read(user.join("Home/notes.txt")), "mine first");
        let _ = std::fs::remove_dir_all(user.parent().unwrap());
    }

    #[test]
    fn manifests_say_cart_only_with_source_cart() {
        assert!(says_cart("name = X\nsource = cart\n"));
        assert!(says_cart("source=\"cart\""));
        assert!(!says_cart("name = cart\n"));
        assert!(!says_cart("source = system\n"));
    }
}
