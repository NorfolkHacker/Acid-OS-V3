//! acid-synth must render tests/golden/script.txt byte for byte as the
//! committed reference recording (tests/golden/script.u8) does. Never
//! regenerate the recording to make this test pass.

use std::path::Path;

use acid_synth::Synth;

fn play(script: &str) -> Vec<u8> {
    let mut s = Synth::new();
    let mut out = Vec::new();
    for (lineno, line) in script.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut words = line.split_whitespace();
        let cmd = words.next().unwrap();
        let n: Vec<i32> = words.map(|w| w.parse().expect("integer argument")).collect();
        match (cmd, n.as_slice()) {
            ("waveform", [v, w]) => s.set_voice_waveform(*v, *w),
            ("duty", [v, p]) => s.set_duty(*v, *p),
            ("ona", [v, o]) => s.set_ona(*v, *o),
            ("adsr", [v, a, d, su, r]) => s.set_adsr(*v, *a, *d, *su, *r),
            ("gate_on", [v]) => s.gate_on(*v),
            ("gate_off", [v]) => s.gate_off(*v),
            ("ring", [v, p]) => s.set_ring_partner(*v, *p),
            ("ring_clear", [v]) => s.clear_ring_partner(*v),
            ("route", [v, r]) => s.set_voice_filter_route(*v, *r),
            ("arp_note", [v, slot, note]) => s.set_arp_note(*v, *slot, *note),
            ("arp_on", [v, c]) => s.arp_on(*v, *c),
            ("arp_rate", [v, ms]) => s.set_arp_rate(*v, *ms),
            ("cutoff", [c]) => s.set_filter_cutoff(*c),
            ("resonance", [r]) => s.set_filter_resonance(*r),
            ("mode", [m]) => s.set_filter_mode(*m),
            ("render", [len]) => {
                let len = usize::try_from(*len).unwrap_or_else(|_| {
                    panic!("script line {}: negative render length {len}", lineno + 1)
                });
                let mut buf = vec![0u8; len];
                s.render(&mut buf);
                out.extend_from_slice(&buf);
            }
            _ => panic!("script line {}: bad command {line:?}", lineno + 1),
        }
    }
    out
}

#[test]
fn matches_reference_recording_byte_for_byte() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    let script = std::fs::read_to_string(dir.join("script.txt")).unwrap();
    let expected = std::fs::read(dir.join("script.u8"))
        .expect("missing committed reference recording tests/golden/script.u8");
    let actual = play(&script);
    assert_eq!(actual.len(), expected.len(), "total rendered length");
    if let Some(i) = (0..expected.len()).find(|&i| actual[i] != expected[i]) {
        let diffs = (0..expected.len()).filter(|&i| actual[i] != expected[i]).count();
        panic!("{diffs} bytes differ from the reference; first at byte {i}: expected {}, got {}", expected[i], actual[i]);
    }
}

#[test]
fn the_golden_is_not_trivially_silent() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    let expected = std::fs::read(dir.join("script.u8")).unwrap();
    let loud = expected.iter().filter(|&&b| b != 128).count();
    assert!(loud > expected.len() / 10, "the script must actually make sound");
}
