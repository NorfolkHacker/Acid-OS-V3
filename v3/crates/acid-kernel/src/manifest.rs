//! `.app.toml` manifests -- not real TOML, just `key = value` lines: trim
//! each line, skip blank and `#` lines, split at the first `=`, trim both
//! sides; a later key wins.

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};

pub fn parse_manifest(text: &str) -> BTreeMap<String, String> {
    let mut fields = BTreeMap::new();
    for line in text.split('\n') {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some(eq) = line.find('=') else { continue };
        fields.insert(line[..eq].trim().to_string(), line[eq + 1..].trim().to_string());
    }
    fields
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_like_desktop_rb() {
        let m = parse_manifest(
            "# comment\nname = Hello Acid\n  w = 200 \nh=150\n\nlibs = lib/a.lua, lib/b.lua\nbroken line\nsource = cart\r\n",
        );
        assert_eq!(m.get("name").map(String::as_str), Some("Hello Acid"));
        assert_eq!(m.get("w").map(String::as_str), Some("200"));
        assert_eq!(m.get("h").map(String::as_str), Some("150"));
        assert_eq!(m.get("libs").map(String::as_str), Some("lib/a.lua, lib/b.lua"));
        assert_eq!(m.get("source").map(String::as_str), Some("cart"));
        assert_eq!(m.len(), 5);
    }

    #[test]
    fn value_may_contain_equals() {
        let m = parse_manifest("desc = a = b");
        assert_eq!(m.get("desc").map(String::as_str), Some("a = b"));
    }
}
