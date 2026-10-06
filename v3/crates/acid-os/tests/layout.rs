//! The shipped v3/fsroot layout: Source is a link into v3/apps, so the
//! OS's file tree shows the one real copy of the apps and their libs
//! rather than a second folder.

use acid_platform::Platform;
use acid_testkit::FakePlatform;

#[cfg(unix)]
#[test]
fn source_is_a_link_into_the_apps_root() {
    let root = FakePlatform::repo_root();
    for (link, target) in [("v3/fsroot/Source", "../apps")] {
        let meta = std::fs::symlink_metadata(root.join(link)).unwrap_or_else(|e| panic!("{link}: {e}"));
        assert!(meta.file_type().is_symlink(), "{link} must be a link, not a folder");
        assert_eq!(std::fs::read_link(root.join(link)).unwrap(), std::path::Path::new(target), "{link}");
    }
    for gone in ["v3/fsroot/App", "v3/fsroot/Lib"] {
        assert!(std::fs::symlink_metadata(root.join(gone)).is_err(), "{gone} must not exist");
    }
}

#[test]
fn the_libs_are_browsable_through_source() {
    let p = FakePlatform::new(FakePlatform::repo_root());
    let names = p.fs().list("v3/fsroot/Source/lib").expect("v3/fsroot/Source/lib lists");
    assert!(names.iter().any(|n| n == "acid_app.lua"), "{names:?}");
    assert!(!names.iter().any(|n| n == "README.txt"), "the old placeholder is gone: {names:?}");
}
