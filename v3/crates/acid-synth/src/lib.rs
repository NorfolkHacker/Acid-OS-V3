//! The Acid OS synthesiser -- a SID-style synth. Integer-only, 22050 Hz,
//! 8 voices, unsigned 8-bit mono. Every setter range-guards its arguments,
//! and the output is checked byte for byte against a committed reference
//! recording (tests/golden.rs).
#![cfg_attr(not(test), no_std)]

mod tables;

pub use tables::{FILTER_F_COEFF, FILTER_Q_COEFF, ONA_PHASE_INCREMENT};

pub const SAMPLE_RATE: u32 = 22050;
pub const NUM_VOICES: usize = 8;
/// Full envelope level; envelope state is this in Q8.
pub const ENV_FULL: i32 = 32768;
pub const ARP_NOTES: usize = 4;
pub const FILTER_MODE_LP: i32 = 1;
pub const FILTER_MODE_BP: i32 = 2;
pub const FILTER_MODE_HP: i32 = 4;
pub const FILTER_STATE_MAX: i32 = 65536;
/// How far back the filter looks for a repeating state while its input
/// is silent, to break limit cycles.
const SILENCE_HISTORY_LEN: usize = 512;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Waveform {
    Pulse = 0,
    Saw = 1,
    Triangle = 2,
    Noise = 3,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnvStage {
    Off,
    Attack,
    Decay,
    Sustain,
    Release,
}

#[derive(Clone, Debug)]
pub struct Voice {
    pub waveform: Waveform,
    pub phase_accum: u32,
    pub phase_increment: u32,
    pub duty_threshold: u32,
    pub noise_lfsr: u32,
    pub envelope_stage: EnvStage,
    /// Q8.
    pub envelope_level: i32,
    pub attack_rate: i32,
    pub decay_rate: i32,
    pub sustain_level: i32,
    pub release_rate: i32,
    /// Another voice index, or -1 for none.
    pub ring_partner: i32,
    pub filter_route: bool,
    pub arp_notes: [i32; ARP_NOTES],
    pub arp_count: i32,
    pub arp_active: bool,
    pub arp_step: i32,
    pub arp_step_rate: i32,
    pub arp_step_counter: i32,
}

impl Voice {
    /// synth_init's per-voice state.
    fn new(index: usize) -> Self {
        Self {
            waveform: Waveform::Pulse,
            phase_accum: 0,
            phase_increment: 0,
            duty_threshold: 128,
            noise_lfsr: 0xACE1 + index as u32,
            envelope_stage: EnvStage::Off,
            envelope_level: 0,
            attack_rate: ENV_FULL << 8,
            decay_rate: ENV_FULL << 8,
            sustain_level: ENV_FULL << 8,
            release_rate: ENV_FULL << 8,
            ring_partner: -1,
            filter_route: false,
            arp_notes: [1; ARP_NOTES],
            arp_count: 0,
            arp_active: false,
            arp_step: 0,
            arp_step_rate: 1,
            arp_step_counter: 0,
        }
    }
}

pub struct Synth {
    voices: [Voice; NUM_VOICES],
    filter_lp: i32,
    filter_bp: i32,
    hist_lp: [i32; SILENCE_HISTORY_LEN],
    hist_bp: [i32; SILENCE_HISTORY_LEN],
    /// u32 rather than C's int: identical until 2^31 silent samples, and
    /// can't overflow-panic (or go negative) after that.
    hist_count: u32,
    cutoff_index: usize,
    res_index: usize,
    mode_mask: i32,
    mix_shift: u32,
}

impl Default for Synth {
    fn default() -> Self {
        Self::new()
    }
}

fn voice_idx(voice: i32) -> Option<usize> {
    if (0..NUM_VOICES as i32).contains(&voice) { Some(voice as usize) } else { None }
}

fn calc_rate(duration_ms: i32) -> i32 {
    if duration_ms <= 0 {
        return ENV_FULL << 8;
    }
    let ms = duration_ms.min(100_000) as u32;
    let samples = ms * SAMPLE_RATE / 1000;
    if samples == 0 {
        return ENV_FULL << 8;
    }
    ((ENV_FULL << 8) / samples as i32).max(1)
}

fn calc_arp_step_samples(ms: i32) -> i32 {
    if ms <= 0 {
        return 1;
    }
    let samples = ms.min(10_000) as u32 * SAMPLE_RATE / 1000;
    if samples == 0 { 1 } else { samples as i32 }
}

fn noise_advance(lfsr: &mut u32) {
    let mut v = *lfsr & 0xFFFF;
    let lsb = v & 1;
    v >>= 1;
    if lsb != 0 {
        v ^= 0xB400;
    }
    *lfsr = v;
}

/// One oscillator sample, -128..=127.
pub fn osc_sample(
    wave: Waveform,
    phase_accum: u32,
    duty_threshold: u32,
    noise_lfsr: &mut u32,
    phase_wrapped: bool,
    ring_active: bool,
    ring_partner_phase_accum: u32,
) -> i32 {
    let pos8 = (phase_accum >> 24) & 0xFF;
    match wave {
        Waveform::Saw => pos8 as i32 - 128,
        Waveform::Triangle => {
            let mut msb = (pos8 >> 7) & 1;
            let lower7 = pos8 & 0x7F;
            if ring_active {
                let partner_pos8 = (ring_partner_phase_accum >> 24) & 0xFF;
                msb ^= (partner_pos8 >> 7) & 1;
            }
            let tri_pos = if msb != 0 { 127 - lower7 } else { lower7 };
            (tri_pos * 2) as i32 - 128
        }
        Waveform::Pulse => {
            if pos8 < duty_threshold { 127 } else { -128 }
        }
        Waveform::Noise => {
            if phase_wrapped {
                noise_advance(noise_lfsr);
            }
            (*noise_lfsr & 0xFF) as i32 - 128
        }
    }
}

/// Advances one envelope sample and returns the level >> 8 (0..=32768).
pub fn envelope_advance_sample(v: &mut Voice) -> i32 {
    match v.envelope_stage {
        EnvStage::Off => v.envelope_level = 0,
        EnvStage::Attack => {
            v.envelope_level += v.attack_rate;
            if v.envelope_level >= ENV_FULL << 8 {
                v.envelope_level = ENV_FULL << 8;
                v.envelope_stage = EnvStage::Decay;
            }
        }
        EnvStage::Decay => {
            if v.envelope_level > v.sustain_level {
                v.envelope_level -= v.decay_rate;
                if v.envelope_level <= v.sustain_level {
                    v.envelope_level = v.sustain_level;
                    v.envelope_stage = EnvStage::Sustain;
                }
            } else {
                v.envelope_stage = EnvStage::Sustain;
            }
        }
        EnvStage::Sustain => v.envelope_level = v.sustain_level,
        EnvStage::Release => {
            if v.envelope_level > v.release_rate {
                v.envelope_level -= v.release_rate;
            } else {
                v.envelope_level = 0;
                v.envelope_stage = EnvStage::Off;
            }
        }
    }
    v.envelope_level >> 8
}

impl Synth {
    /// synth_init.
    pub fn new() -> Self {
        Self {
            voices: core::array::from_fn(Voice::new),
            filter_lp: 0,
            filter_bp: 0,
            hist_lp: [0; SILENCE_HISTORY_LEN],
            hist_bp: [0; SILENCE_HISTORY_LEN],
            hist_count: 0,
            cutoff_index: 128,
            res_index: 0,
            mode_mask: FILTER_MODE_LP,
            mix_shift: 0,
        }
    }

    /// Headroom: the voice mix is divided by 2^shift before clipping.
    pub fn set_mix_shift(&mut self, shift: i32) {
        self.mix_shift = shift.clamp(0, 3) as u32;
    }

    pub fn mix_shift(&self) -> u32 {
        self.mix_shift
    }

    pub fn voice(&self, v: usize) -> &Voice {
        &self.voices[v]
    }

    /// For the kernel's note-on sustain and arp reset, which write
    /// straight into the voice.
    pub fn voice_mut(&mut self, v: usize) -> &mut Voice {
        &mut self.voices[v]
    }

    pub fn set_voice_waveform(&mut self, voice: i32, wave: i32) {
        let Some(v) = voice_idx(voice) else { return };
        self.voices[v].waveform = match wave {
            0 => Waveform::Pulse,
            1 => Waveform::Saw,
            2 => Waveform::Triangle,
            3 => Waveform::Noise,
            _ => return,
        };
    }

    pub fn set_duty(&mut self, voice: i32, duty_percent: i32) {
        let Some(v) = voice_idx(voice) else { return };
        let d = duty_percent.clamp(1, 99) as u32;
        self.voices[v].duty_threshold = d * 256 / 100;
    }

    pub fn set_ona(&mut self, voice: i32, ona: i32) {
        let Some(v) = voice_idx(voice) else { return };
        if !(1..=88).contains(&ona) {
            return;
        }
        self.voices[v].phase_increment = ONA_PHASE_INCREMENT[(ona - 1) as usize];
    }

    pub fn set_ring_partner(&mut self, voice: i32, partner: i32) {
        let Some(v) = voice_idx(voice) else { return };
        if !(0..=7).contains(&partner) {
            return;
        }
        self.voices[v].ring_partner = partner;
    }

    pub fn clear_ring_partner(&mut self, voice: i32) {
        if let Some(v) = voice_idx(voice) {
            self.voices[v].ring_partner = -1;
        }
    }

    pub fn set_voice_filter_route(&mut self, voice: i32, routed: i32) {
        if let Some(v) = voice_idx(voice) {
            self.voices[v].filter_route = routed != 0;
        }
    }

    pub fn set_arp_note(&mut self, voice: i32, slot: i32, note: i32) {
        let Some(v) = voice_idx(voice) else { return };
        if !(0..ARP_NOTES as i32).contains(&slot) || !(1..=88).contains(&note) {
            return;
        }
        self.voices[v].arp_notes[slot as usize] = note;
    }

    pub fn arp_on(&mut self, voice: i32, count: i32) {
        let Some(v) = voice_idx(voice) else { return };
        if !(2..=ARP_NOTES as i32).contains(&count) {
            return;
        }
        self.voices[v].arp_count = count;
        self.voices[v].arp_active = true;
    }

    pub fn set_arp_rate(&mut self, voice: i32, ms: i32) {
        if let Some(v) = voice_idx(voice) {
            self.voices[v].arp_step_rate = calc_arp_step_samples(ms);
        }
    }

    pub fn set_adsr(&mut self, voice: i32, attack_ms: i32, decay_ms: i32, sustain_percent: i32, release_ms: i32) {
        let Some(v) = voice_idx(voice) else { return };
        let s = sustain_percent.clamp(0, 100);
        let voice = &mut self.voices[v];
        voice.attack_rate = calc_rate(attack_ms);
        voice.decay_rate = calc_rate(decay_ms);
        voice.sustain_level = ((s * ENV_FULL) / 100) << 8;
        voice.release_rate = calc_rate(release_ms);
    }

    pub fn gate_on(&mut self, voice: i32) {
        let Some(v) = voice_idx(voice) else { return };
        let voice = &mut self.voices[v];
        if voice.arp_active {
            voice.arp_step = 0;
            voice.arp_step_counter = 0;
            voice.phase_increment = ONA_PHASE_INCREMENT[(voice.arp_notes[0] - 1) as usize];
        }
        if voice.phase_increment == 0 {
            return;
        }
        voice.envelope_level = 0;
        voice.envelope_stage = EnvStage::Attack;
    }

    pub fn gate_off(&mut self, voice: i32) {
        let Some(v) = voice_idx(voice) else { return };
        if self.voices[v].envelope_stage != EnvStage::Off {
            self.voices[v].envelope_stage = EnvStage::Release;
        }
    }

    pub fn set_filter_cutoff(&mut self, cutoff: i32) {
        self.cutoff_index = cutoff.clamp(0, 255) as usize;
    }

    pub fn set_filter_resonance(&mut self, resonance: i32) {
        self.res_index = resonance.clamp(0, 15) as usize;
    }

    pub fn set_filter_mode(&mut self, mode_mask: i32) {
        self.mode_mask = mode_mask.clamp(0, 7);
    }

    /// One state-variable filter sample, including the silence-history
    /// limit-cycle breaker and the state clamp.
    fn filter_process_sample(&mut self, input: i32, f_coeff: i32, q_coeff: i32, mode_mask: i32) -> i32 {
        let hp = input - self.filter_lp - ((q_coeff as i64 * self.filter_bp as i64) >> 14) as i32;
        let mut bp_new = self.filter_bp + ((f_coeff as i64 * hp as i64) >> 14) as i32;
        let mut lp_new = self.filter_lp + ((f_coeff as i64 * bp_new as i64) >> 14) as i32;
        if input != 0 {
            self.hist_count = 0;
        } else {
            let seen = (self.hist_count as usize).min(SILENCE_HISTORY_LEN);
            let matched = (0..seen).any(|k| self.hist_lp[k] == lp_new && self.hist_bp[k] == bp_new);
            if matched {
                lp_new = 0;
                bp_new = 0;
                self.hist_count = 0;
            } else {
                let slot = self.hist_count as usize % SILENCE_HISTORY_LEN;
                self.hist_lp[slot] = lp_new;
                self.hist_bp[slot] = bp_new;
                self.hist_count = self.hist_count.wrapping_add(1);
            }
        }
        lp_new = lp_new.clamp(-FILTER_STATE_MAX, FILTER_STATE_MAX);
        bp_new = bp_new.clamp(-FILTER_STATE_MAX, FILTER_STATE_MAX);
        self.filter_lp = lp_new;
        self.filter_bp = bp_new;
        let mut out = 0;
        if mode_mask & FILTER_MODE_LP != 0 {
            out += lp_new;
        }
        if mode_mask & FILTER_MODE_BP != 0 {
            out += bp_new;
        }
        if mode_mask & FILTER_MODE_HP != 0 {
            out += hp;
        }
        out
    }

    /// Renders `buf.len()` samples. Voices advance in place, in index
    /// order, so a ring partner with a lower index has already advanced
    /// this sample and a higher one hasn't.
    pub fn render(&mut self, buf: &mut [u8]) {
        for out in buf.iter_mut() {
            let mut filtered_sum = 0i32;
            let mut bypass_sum = 0i32;
            for v in 0..NUM_VOICES {
                let partner = self.voices[v].ring_partner;
                let ring_active = partner >= 0;
                let ring_partner_accum = if ring_active { self.voices[partner as usize].phase_accum } else { 0 };
                let voice = &mut self.voices[v];
                let old_accum = voice.phase_accum;
                let new_accum = old_accum.wrapping_add(voice.phase_increment);
                let wrapped = new_accum < old_accum;
                let osc = osc_sample(
                    voice.waveform, old_accum, voice.duty_threshold, &mut voice.noise_lfsr,
                    wrapped, ring_active, ring_partner_accum,
                );
                let level = envelope_advance_sample(voice);
                let contribution = (osc * level) >> 15;
                if voice.filter_route {
                    filtered_sum += contribution;
                } else {
                    bypass_sum += contribution;
                }
                voice.phase_accum = new_accum;
                if voice.arp_active {
                    voice.arp_step_counter += 1;
                    if voice.arp_step_counter >= voice.arp_step_rate {
                        voice.arp_step_counter = 0;
                        voice.arp_step = (voice.arp_step + 1) % voice.arp_count;
                        voice.phase_increment =
                            ONA_PHASE_INCREMENT[(voice.arp_notes[voice.arp_step as usize] - 1) as usize];
                    }
                }
            }
            let filtered_out = self.filter_process_sample(
                filtered_sum,
                FILTER_F_COEFF[self.cutoff_index],
                FILTER_Q_COEFF[self.res_index],
                self.mode_mask,
            );
            let sum = ((filtered_out + bypass_sum) >> self.mix_shift).clamp(-128, 127);
            *out = (sum + 128) as u8;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn silent(buf: &[u8]) -> bool {
        buf.iter().all(|&b| b == 128)
    }

    #[test]
    fn a_new_synth_is_silent() {
        let mut s = Synth::new();
        let mut buf = [0u8; 256];
        s.render(&mut buf);
        assert!(silent(&buf));
    }

    #[test]
    fn note_then_release_to_silence() {
        let mut s = Synth::new();
        s.set_voice_waveform(0, 0);
        s.set_ona(0, 49);
        s.gate_on(0);
        let mut buf = [0u8; 512];
        s.render(&mut buf);
        assert!(!silent(&buf));
        s.gate_off(0);
        let mut buf2 = [0u8; 4096];
        s.render(&mut buf2);
        assert!(silent(&buf2), "default release is one sample: silent over the whole buffer");
        assert_eq!(s.voice(0).envelope_stage, EnvStage::Off);
    }

    #[test]
    fn tables_match_spot_values() {
        assert_eq!(ONA_PHASE_INCREMENT[0], 5_356_535);
        assert_eq!(ONA_PHASE_INCREMENT[87], 815_363_807);
        assert_eq!((FILTER_F_COEFF[0], FILTER_F_COEFF[255]), (93, 13_583));
        assert_eq!((FILTER_Q_COEFF[0], FILTER_Q_COEFF[15]), (23_174, 2_048));
    }

    #[test]
    fn range_guards_ignore_bad_arguments() {
        let mut s = Synth::new();
        s.set_ona(0, 0);
        s.set_ona(0, 89);
        s.set_ona(8, 40);
        s.set_ona(-1, 40);
        assert_eq!(s.voice(0).phase_increment, 0);
        s.set_voice_waveform(0, 7);
        assert_eq!(s.voice(0).waveform, Waveform::Pulse);
        s.set_ring_partner(0, 8);
        assert_eq!(s.voice(0).ring_partner, -1);
        s.arp_on(0, 1);
        s.arp_on(0, 5);
        assert!(!s.voice(0).arp_active);
        s.gate_on(0);
        assert_eq!(s.voice(0).envelope_stage, EnvStage::Off, "no pitch, no gate");
    }

    #[test]
    fn duty_clamps_to_1_to_99_percent() {
        let mut s = Synth::new();
        s.set_duty(0, 0);
        assert_eq!(s.voice(0).duty_threshold, 2); // 1 * 256 / 100
        s.set_duty(0, 150);
        assert_eq!(s.voice(0).duty_threshold, 253); // 99 * 256 / 100
    }

    #[test]
    fn adsr_rates_and_sustain_in_q8() {
        let mut s = Synth::new();
        s.set_adsr(0, 0, 1000, 50, 100_000_000);
        let v = s.voice(0);
        assert_eq!(v.attack_rate, ENV_FULL << 8, "0 ms is instant");
        assert_eq!(v.decay_rate, (ENV_FULL << 8) / 22_050);
        assert_eq!(v.sustain_level, (50 * ENV_FULL / 100) << 8);
        assert_eq!(v.release_rate, ((ENV_FULL << 8) / 2_205_000).max(1), "clamped to 100 s");
    }

    #[test]
    fn noise_lfsr_advances_by_its_feedback_taps() {
        let mut lfsr = 0xACE1u32;
        // 0xACE1 >> 1 = 0x5670, the lsb was 1, so xor 0xB400 -> 0xE270.
        let out = osc_sample(Waveform::Noise, 0, 128, &mut lfsr, true, false, 0);
        assert_eq!(lfsr, 0xE270);
        assert_eq!(out, 0x70 - 128);
        let again = osc_sample(Waveform::Noise, 0, 128, &mut lfsr, false, false, 0);
        assert_eq!((lfsr, again), (0xE270, 0x70 - 128), "only advances when the phase wraps");
    }

    #[test]
    fn oscillator_shapes() {
        let mut l = 1;
        assert_eq!(osc_sample(Waveform::Saw, 0x8000_0000, 0, &mut l, false, false, 0), 0);
        assert_eq!(osc_sample(Waveform::Pulse, 0x1000_0000, 0x20, &mut l, false, false, 0), 127);
        assert_eq!(osc_sample(Waveform::Pulse, 0x3000_0000, 0x20, &mut l, false, false, 0), -128);
        // Triangle at pos 0x40: rising half -> 0x40 * 2 - 128 = 0.
        assert_eq!(osc_sample(Waveform::Triangle, 0x4000_0000, 0, &mut l, false, false, 0), 0);
        // Ring mod flips the half when the partner's msb is set.
        assert_eq!(osc_sample(Waveform::Triangle, 0x4000_0000, 0, &mut l, false, true, 0x8000_0000), 126 - 128);
    }

    #[test]
    fn envelope_walks_attack_decay_sustain_release_off() {
        let mut s = Synth::new();
        s.set_adsr(0, 1, 1, 50, 1);
        s.set_ona(0, 40);
        s.gate_on(0);
        let mut v = s.voice(0).clone();
        let mut stages = vec![Some(EnvStage::Attack)];
        for _ in 0..200 {
            envelope_advance_sample(&mut v);
            note(&mut stages, v.envelope_stage);
        }
        v.envelope_stage = EnvStage::Release;
        for _ in 0..200 {
            envelope_advance_sample(&mut v);
            note(&mut stages, v.envelope_stage);
        }
        assert_eq!(stages, [EnvStage::Attack, EnvStage::Decay, EnvStage::Sustain, EnvStage::Release, EnvStage::Off]
            .iter().map(|s| Some(*s)).collect::<Vec<_>>());
    }

    fn note(stages: &mut Vec<Option<EnvStage>>, s: EnvStage) {
        if stages.last() != Some(&Some(s)) {
            stages.push(Some(s));
        }
    }

    #[test]
    fn filter_state_is_clamped() {
        let mut s = Synth::new();
        for _ in 0..1000 {
            s.filter_process_sample(30_000, FILTER_F_COEFF[255], FILTER_Q_COEFF[15], FILTER_MODE_LP);
        }
        assert!(s.filter_lp.abs() <= FILTER_STATE_MAX && s.filter_bp.abs() <= FILTER_STATE_MAX);
    }

    #[test]
    fn arp_steps_through_its_notes() {
        let mut s = Synth::new();
        for (slot, n) in [40, 44, 47, 52].iter().enumerate() {
            s.set_arp_note(0, slot as i32, *n);
        }
        s.set_arp_rate(0, 1); // 22 samples per step
        s.arp_on(0, 4);
        s.gate_on(0);
        assert_eq!(s.voice(0).phase_increment, ONA_PHASE_INCREMENT[39]);
        let mut buf = [0u8; 22];
        s.render(&mut buf);
        assert_eq!(s.voice(0).phase_increment, ONA_PHASE_INCREMENT[43]);
    }

    fn four_saws(shift: i32, spread: u32) -> [u8; 2048] {
        let mut s = Synth::new();
        s.set_mix_shift(shift);
        for v in 0..4 {
            s.set_voice_waveform(v, 1);
            s.set_adsr(v, 0, 0, 100, 0);
            s.set_ona(v, 40);
            s.set_voice_filter_route(v, 0);
            s.gate_on(v);
            s.voice_mut(v as usize).phase_accum = v as u32 * spread;
        }
        let mut buf = [0u8; 2048];
        s.render(&mut buf);
        buf
    }

    #[test]
    fn mix_shift_gives_headroom() {
        let hot = four_saws(0, 0);
        assert!(hot.iter().any(|&b| b == 0 || b == 255), "four full saws clip without headroom");
        let cool = four_saws(2, 0x1000_0000);
        assert!(cool.iter().all(|&b| b != 0 && b != 255), "shift 2 leaves four saws unclipped");
        assert!(cool.iter().any(|&b| b != 128));
    }

    #[test]
    fn mix_shift_is_clamped() {
        let mut s = Synth::new();
        assert_eq!(s.mix_shift(), 0);
        s.set_mix_shift(i32::MAX);
        assert_eq!(s.mix_shift(), 3);
        s.set_mix_shift(i32::MIN);
        assert_eq!(s.mix_shift(), 0);
    }
}
