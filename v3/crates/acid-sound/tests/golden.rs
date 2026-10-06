//! The engine must render tests/golden/demo.trk, with demo.snd's
//! instruments and its zap effect fired mid-song, byte for byte as the
//! committed recording tests/golden/demo.u8. Never regenerate the
//! recording to make this test pass: ACID_SOUND_BLESS=1 is only for a
//! change in sound that is intended and has been listened to
//! (examples/to_wav.rs).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use acid_sound::engine::Engine;
use acid_sound::load::{load_program, load_song};
use acid_sound::song::{parse, write};
use acid_synth::Synth;

fn dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden")
}

fn read(path: &str) -> Result<String, String> {
    match path {
        "Home/sounds/demo.snd" => std::fs::read_to_string(dir().join("demo.snd")).map_err(|e| e.to_string()),
        _ => Err("not found".into()),
    }
}

/// Four seconds of the demo, in 512-sample buffers; the zap starts in
/// buffer 90 (about 2.1 s) and borrows channel 4's second voice.
fn render_demo() -> Vec<u8> {
    let text = std::fs::read_to_string(dir().join("demo.trk")).unwrap();
    let (song, warnings) = load_song(&text, &read).unwrap();
    assert!(warnings.is_empty(), "{warnings:?}");
    let sfx = Arc::new(load_program(&read("Home/sounds/demo.snd").unwrap(), &read).unwrap());
    let zap = sfx.block("zap").unwrap();
    let mut synth = Synth::new();
    let mut engine = Engine::new();
    engine.play_song(&mut synth, 1, 1, Arc::new(song), 0, 0);
    let mut out = vec![0u8; 22050 * 4];
    for (i, chunk) in out.chunks_mut(512).enumerate() {
        if i == 90 {
            assert!(engine.play_sound(&mut synth, 2, sfx.clone(), zap, 40, 0).is_some());
        }
        engine.render(&mut synth, chunk);
    }
    out
}

#[test]
fn demo_matches_the_reference_recording() {
    let actual = render_demo();
    let path = dir().join("demo.u8");
    if std::env::var_os("ACID_SOUND_BLESS").is_some() {
        std::fs::write(&path, &actual).unwrap();
    }
    let expected = std::fs::read(&path).expect("missing tests/golden/demo.u8 (bless it once with ACID_SOUND_BLESS=1)");
    assert_eq!(actual.len(), expected.len(), "total rendered length");
    if let Some(i) = (0..expected.len()).find(|&i| actual[i] != expected[i]) {
        let diffs = (0..expected.len()).filter(|&i| actual[i] != expected[i]).count();
        panic!("{diffs} bytes differ from the reference; first at byte {i}: expected {}, got {}", expected[i], actual[i]);
    }
}

#[test]
fn the_demo_is_not_trivially_silent() {
    let r = render_demo();
    let loud = r.iter().filter(|&&b| b != 128).count();
    assert!(loud > r.len() / 10, "the demo must actually make sound");
}

#[test]
fn rendering_is_deterministic() {
    assert_eq!(render_demo(), render_demo());
}

#[test]
fn the_demo_song_file_is_canonical() {
    let text = std::fs::read_to_string(dir().join("demo.trk")).unwrap();
    assert_eq!(write(&parse(&text).unwrap()), text);
}
