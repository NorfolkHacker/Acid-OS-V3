//! The songs and sounds shipped in v3/fsroot/Home load cleanly: every .snd
//! compiles, every .trk loads with no warnings and is canonical, and the
//! demo song plays, audibly and without clipping much.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use acid_sound::engine::Engine;
use acid_sound::load::{load_program, load_song};
use acid_sound::song::{parse, write};
use acid_synth::Synth;

fn fsroot() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fsroot")
}

/// Files named inside songs and scripts are fsroot-relative.
fn read(path: &str) -> Result<String, String> {
    if path.split('/').any(|s| s.is_empty() || s == "." || s == "..") {
        return Err("bad path".into());
    }
    std::fs::read_to_string(fsroot().join(path)).map_err(|_| "not found".to_string())
}

fn files(dir: &str, ext: &str) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(fsroot().join(dir))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == ext))
        .collect();
    v.sort();
    v
}

#[test]
fn every_shipped_sound_compiles() {
    let snd = files("Home/sounds", "snd");
    assert!(snd.len() >= 4, "bass, wobble, zap and sync are shipped");
    for p in snd {
        let src = std::fs::read_to_string(&p).unwrap();
        load_program(&src, &read).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
    }
}

#[test]
fn every_shipped_song_loads_cleanly_and_is_canonical() {
    let trk = files("Home/music", "trk");
    assert!(!trk.is_empty(), "acid_groove.trk is shipped");
    for p in trk {
        let text = std::fs::read_to_string(&p).unwrap();
        let (_, warnings) = load_song(&text, &read).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
        assert!(warnings.is_empty(), "{}: {warnings:?}", p.display());
        assert_eq!(write(&parse(&text).unwrap()), text, "{} is not in canonical form", p.display());
    }
}

#[test]
fn the_demo_song_plays_audibly_without_much_clipping() {
    let text = read("Home/music/acid_groove.trk").unwrap();
    let (song, _) = load_song(&text, &read).unwrap();
    let (mut synth, mut engine) = (Synth::new(), Engine::new());
    engine.play_song(&mut synth, 1, 1, Arc::new(song), 0, 0);
    let mut buf = vec![0u8; 22050 * 4];
    engine.render(&mut synth, &mut buf);
    let loud = buf.iter().filter(|&&b| b != 128).count();
    assert!(loud > buf.len() / 4, "only {loud} audible samples");
    let clipped = buf.iter().filter(|&&b| b == 0 || b == 255).count();
    assert!(clipped < buf.len() / 20, "{clipped} clipped samples");
}

#[test]
fn the_sync_script_starts_the_demo_song() {
    let src = read("Home/sounds/sync.snd").unwrap();
    let prog = Arc::new(load_program(&src, &read).unwrap());
    let (mut synth, mut engine) = (Synth::new(), Engine::new());
    engine.play_sound(&mut synth, 7, prog, 0, 40, 0).unwrap();
    let mut buf = vec![0u8; 441 * 3];
    engine.render(&mut synth, &mut buf);
    assert_eq!(engine.song_owner(), Some(7));
    assert!(engine.song_position().is_some());
}
