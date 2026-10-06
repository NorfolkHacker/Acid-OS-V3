//! Note names and pitch. Pitch is the synth's 88-key `ona` scale (A0 = 1,
//! C-4 = 40, C-8 = 88). Fine pitch is 1/64 of a semitone, linearly
//! interpolated between the synth's per-semitone phase increments.

use alloc::format;
use alloc::string::String;

use acid_synth::ONA_PHASE_INCREMENT;

pub const ONA_MIN: i32 = 1;
pub const ONA_MAX: i32 = 88;
/// Fine pitch steps per semitone.
pub const FINE_STEPS: i32 = 64;
/// The highest fine position (C-8).
pub const FINE_MAX: i32 = (ONA_MAX - 1) * FINE_STEPS;

const NAMES: [&str; 12] = ["C-", "C#", "D-", "D#", "E-", "F-", "F#", "G-", "G#", "A-", "A#", "B-"];

/// "C-4" -> 40. None for anything else, or a note off the keyboard.
pub fn parse_note(s: &str) -> Option<i32> {
    let b = s.as_bytes();
    if b.len() != 3 {
        return None;
    }
    let semi = NAMES.iter().position(|n| n.as_bytes() == &b[..2])? as i32;
    let oct = (b[2] as char).to_digit(10)? as i32;
    let ona = oct * 12 + semi - 8;
    (ONA_MIN..=ONA_MAX).contains(&ona).then_some(ona)
}

/// 40 -> "C-4". Out-of-range values are clamped onto the keyboard.
pub fn note_name(ona: i32) -> String {
    let n = ona.clamp(ONA_MIN, ONA_MAX) + 8;
    format!("{}{}", NAMES[(n % 12) as usize], n / 12)
}

/// A whole note's fine position; slides and vibrato add to it.
pub fn fine_pos(ona: i32) -> i32 {
    ona.saturating_sub(1).saturating_mul(FINE_STEPS)
}

/// The phase increment at a fine position, clamped to the keyboard.
pub fn increment_at(pos: i32) -> u32 {
    let p = pos.clamp(0, FINE_MAX);
    let i = (p / FINE_STEPS) as usize;
    let f = (p % FINE_STEPS) as u64;
    let lo = ONA_PHASE_INCREMENT[i] as u64;
    if f == 0 {
        return lo as u32;
    }
    let hi = ONA_PHASE_INCREMENT[i + 1] as u64;
    (lo + (hi - lo) * f / FINE_STEPS as u64) as u32
}

/// The phase increment for `ona` plus `fine` 1/64-semitone steps.
pub fn phase_increment(ona: i32, fine: i32) -> u32 {
    increment_at(fine_pos(ona).saturating_add(fine))
}

#[cfg(test)]
mod tests {
    use super::*;
    use acid_synth::ONA_PHASE_INCREMENT;

    #[test]
    fn note_names_map_to_piano_keys() {
        assert_eq!(parse_note("A-0"), Some(1));
        assert_eq!(parse_note("C-4"), Some(40));
        assert_eq!(parse_note("C#4"), Some(41));
        assert_eq!(parse_note("C-8"), Some(88));
        assert_eq!(parse_note("G#0"), None, "below A0");
        assert_eq!(parse_note("C#8"), None, "above C8");
        assert_eq!(parse_note("H-4"), None);
        assert_eq!(parse_note("C-"), None);
    }

    #[test]
    fn names_round_trip() {
        for ona in ONA_MIN..=ONA_MAX {
            assert_eq!(parse_note(&note_name(ona)), Some(ona), "{ona}");
        }
    }

    #[test]
    fn whole_semitones_match_the_synth_table() {
        for ona in ONA_MIN..=ONA_MAX {
            assert_eq!(phase_increment(ona, 0), ONA_PHASE_INCREMENT[(ona - 1) as usize]);
        }
    }

    #[test]
    fn fine_steps_sit_between_semitones() {
        let (lo, mid, hi) = (phase_increment(40, 0), phase_increment(40, 32), phase_increment(41, 0));
        assert!(lo < mid && mid < hi);
        assert_eq!(phase_increment(40, 64), hi);
        assert_eq!(phase_increment(40, -64), phase_increment(39, 0));
    }

    #[test]
    fn out_of_range_clamps_to_the_keyboard() {
        assert_eq!(phase_increment(-5, 0), ONA_PHASE_INCREMENT[0]);
        assert_eq!(phase_increment(200, 0), ONA_PHASE_INCREMENT[87]);
        assert_eq!(increment_at(fine_pos(40) + 8), phase_increment(40, 8));
    }
}
