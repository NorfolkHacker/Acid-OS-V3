//! Path guard for the Lua fs calls: the Phase 1 "StdFs paths" item. Apps may
//! only read under the two roots, with no `..` or `.` segments, empty segments, absolute
//! paths, NULs or backslashes. 5b's write calls reuse this check.

/// The only directory trees apps may touch.
pub use acid_platform::FS_ROOTS;

pub fn fs_path_is_allowed(path: &str) -> bool {
    if path.is_empty() || path.starts_with('/') || path.contains('\0') || path.contains('\\') {
        return false;
    }
    if path.split('/').any(|seg| seg.is_empty() || seg == ".." || seg == ".") {
        return false;
    }
    FS_ROOTS.iter().any(|r| {
        path == *r || path.strip_prefix(r).is_some_and(|rest| rest.starts_with('/'))
    })
}

#[cfg(test)]
mod tests {
    use super::fs_path_is_allowed;

    #[test]
    fn both_roots_and_paths_under_them_are_allowed() {
        for ok in ["v3/apps", "v3/apps/desktop.lua", "v3/apps/lib/acid_app.lua", "v3/fsroot", "v3/fsroot/Help/about.txt"] {
            assert!(fs_path_is_allowed(ok), "{ok:?} should be allowed");
        }
    }

    #[test]
    fn everything_else_is_rejected() {
        for bad in [
            "", "/v3/apps", "v3/apps/../crates/x", "..", "v3/apps//x", "v3/apps/", "v3/appsx",
            "v3/fsroot2/x", "v3", "v3/crates/acid-os", "v3/apps/a\0b", "v3\\apps", "./v3/apps",
            "v3/apps/./x.lua", "v3/fsroot/Source/.",
        ] {
            assert!(!fs_path_is_allowed(bad), "{bad:?} should be rejected");
        }
    }
}
