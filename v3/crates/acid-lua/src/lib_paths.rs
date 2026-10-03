//! Which manifest `libs` entries an app may load. A manifest is
//! app-controlled data, so it must not be able to name a file outside
//! the apps directory.

pub fn lib_path_is_safe(path: &str) -> bool {
    if path.is_empty() || path.starts_with('/') || path.contains('\\') {
        return false;
    }
    if path.len() < 4 || !path.ends_with(".lua") {
        return false;
    }
    path.split('/').all(|seg| !seg.is_empty() && seg != "..")
}

#[cfg(test)]
mod tests {
    use super::lib_path_is_safe;

    #[test]
    fn accepts_plain_relative_lua_paths() {
        assert!(lib_path_is_safe("lib/acid_palette.lua"));
        assert!(lib_path_is_safe("editor/buffer.lua"));
        assert!(lib_path_is_safe("a.lua"));
    }

    #[test]
    fn rejects_absolute_escaping_and_malformed_paths() {
        for bad in ["", "/etc/x.lua", "lib\\x.lua", "lib/x.rb", "x.lu", "../x.lua", "lib/../../x.lua", "lib//x.lua", "lib/x.lua/"] {
            assert!(!lib_path_is_safe(bad), "{bad:?} should be rejected");
        }
    }

    #[test]
    fn bare_extension_is_accepted() {
        // ".lua" on its own is one non-".." segment ending in .lua, so the
        // rule accepts it; it is deliberately not tightened.
        assert!(lib_path_is_safe(".lua"));
    }
}
