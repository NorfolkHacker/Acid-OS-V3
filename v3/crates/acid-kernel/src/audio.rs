//! Kernel audio. Apps issue commands; the platform's audio callback pulls
//! samples with render_audio. Commands apply directly under the audio leaf
//! lock, which render_audio holds for a whole buffer, so they take effect
//! between buffers (spec 9.4). Volume and voice count are atomics, so
//! reading them never waits on a render.

use core::sync::atomic::{AtomicI32, AtomicU32, Ordering};

use alloc::sync::Arc;

use acid_platform::sync::Mutex;
use acid_sound::engine::Engine;
use acid_sound::player::LoadedSong;
use acid_sound::program::Program;
use acid_synth::{ENV_FULL, EnvStage, NUM_VOICES, Synth};

use crate::TaskId;
use crate::kernel::Kernel;

pub struct AudioState {
    pub(crate) synth: Synth,
    /// Which app last started each voice.
    pub(crate) owners: [Option<TaskId>; NUM_VOICES],
    /// Songs and .snd sounds, ticked from render_audio.
    pub(crate) engine: Engine,
}

pub struct AudioRuntime {
    pub(crate) state: Mutex<AudioState>,
    master_volume: AtomicI32,
    active_voice_mask: AtomicU32,
}

impl Default for AudioRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioRuntime {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(AudioState { synth: Synth::new(), owners: [None; NUM_VOICES], engine: Engine::new() }),
            master_volume: AtomicI32::new(100),
            active_voice_mask: AtomicU32::new(0),
        }
    }
}

/// Bounds-checks the voice of every voice-addressed command, the osc and
/// ring commands included.
fn voice_index(voice: i32) -> Option<usize> {
    usize::try_from(voice).ok().filter(|&v| v < NUM_VOICES)
}

/// Voices apps are playing directly (acid_play_note), which sounds avoid.
fn busy_voices(a: &AudioState) -> u8 {
    (0..NUM_VOICES)
        .filter(|&v| a.owners[v].is_some() && a.synth.voice(v).envelope_stage != EnvStage::Off)
        .fold(0u8, |m, v| m | 1 << v)
}

impl Kernel {
    pub fn audio_note_on(&self, task: TaskId, voice: i32, ona: i32, volume: i32) {
        let Some(v) = voice_index(voice) else { return };
        let mut a = self.audio.state.lock();
        a.synth.set_ona(voice, ona);
        a.synth.voice_mut(v).sustain_level = (volume.clamp(0, 100) * ENV_FULL / 100) << 8;
        a.synth.gate_on(voice);
        a.owners[v] = Some(task);
    }

    /// Any app may stop any voice (the owner isn't checked).
    pub fn audio_note_off(&self, voice: i32) {
        let Some(v) = voice_index(voice) else { return };
        let mut a = self.audio.state.lock();
        a.synth.gate_off(voice);
        a.synth.voice_mut(v).arp_active = false;
        a.owners[v] = None;
    }

    pub fn audio_configure_voice(&self, voice: i32, filter_route: i32, attack_ms: i32, decay_ms: i32,
                                 sustain_percent: i32, release_ms: i32) {
        if voice_index(voice).is_none() {
            return;
        }
        let mut a = self.audio.state.lock();
        a.synth.set_voice_filter_route(voice, filter_route);
        a.synth.set_adsr(voice, attack_ms, decay_ms, sustain_percent, release_ms);
    }

    pub fn audio_configure_filter(&self, cutoff: i32, resonance: i32, mode: i32) {
        let mut a = self.audio.state.lock();
        a.synth.set_filter_cutoff(cutoff);
        a.synth.set_filter_resonance(resonance);
        a.synth.set_filter_mode(mode);
    }

    pub fn audio_trigger_arp(&self, voice: i32, notes: [i32; 4], count: i32, rate_ms: i32) {
        let Some(v) = voice_index(voice) else { return };
        let mut a = self.audio.state.lock();
        for (slot, note) in notes.iter().enumerate() {
            a.synth.set_arp_note(voice, slot as i32, *note);
        }
        a.synth.set_arp_rate(voice, rate_ms);
        a.synth.voice_mut(v).arp_step = 0;
        a.synth.voice_mut(v).arp_step_counter = 0;
        a.synth.arp_on(voice, count);
    }

    pub fn audio_configure_osc(&self, voice: i32, waveform: i32, duty_percent: i32) {
        if voice_index(voice).is_none() {
            return;
        }
        let mut a = self.audio.state.lock();
        a.synth.set_voice_waveform(voice, waveform);
        a.synth.set_duty(voice, duty_percent);
    }

    /// partner < 0 clears the ring partner.
    pub fn audio_set_ring_partner(&self, voice: i32, partner: i32) {
        if voice_index(voice).is_none() {
            return;
        }
        let mut a = self.audio.state.lock();
        if partner < 0 {
            a.synth.clear_ring_partner(voice);
        } else {
            a.synth.set_ring_partner(voice, partner);
        }
    }

    /// Gates off every voice `task` started. Runs from exit_app on every
    /// exit path.
    pub fn audio_release_owner(&self, task: TaskId) {
        let mut a = self.audio.state.lock();
        for v in 0..NUM_VOICES {
            if a.owners[v] == Some(task) {
                a.synth.gate_off(v as i32);
                a.synth.voice_mut(v).arp_active = false;
                a.owners[v] = None;
            }
        }
        let st = &mut *a;
        st.engine.release_owner(&mut st.synth, task.0);
    }

    /// Starts a .snd `sound` block on free voices; None when there are none.
    pub fn audio_sound_play(&self, task: TaskId, prog: Arc<Program>, block: usize, note: i32) -> Option<u32> {
        let mut a = self.audio.state.lock();
        let busy = busy_voices(&a);
        let st = &mut *a;
        st.engine.play_sound(&mut st.synth, task.0, prog, block, note, busy)
    }

    /// Stops one of `task`'s own sounds.
    pub fn audio_sound_stop(&self, task: TaskId, id: u32) {
        let mut a = self.audio.state.lock();
        let st = &mut *a;
        st.engine.stop_sound(&mut st.synth, task.0, id);
    }

    /// Plays `song` (replacing any song) from `order`/`row`. The i32 values
    /// go straight to the engine, which clamps them.
    pub fn audio_song_play(&self, task: TaskId, handle: u32, song: Arc<LoadedSong>, order: i32, row: i32) {
        let mut a = self.audio.state.lock();
        let st = &mut *a;
        st.engine.play_song(&mut st.synth, task.0, handle, song, order, row);
    }

    /// Stops the song, if `task` started it.
    pub fn audio_song_stop(&self, task: TaskId) {
        let mut a = self.audio.state.lock();
        let st = &mut *a;
        if st.engine.song_owner() == Some(task.0) {
            st.engine.stop_song(&mut st.synth);
        }
    }

    pub fn audio_song_update(&self, task: TaskId, handle: u32, song: Arc<LoadedSong>) {
        self.audio.state.lock().engine.update_song(task.0, handle, song);
    }

    /// Where the song is, if `task` started it: an app that lost the
    /// song to another sees None, so it stops following.
    pub fn audio_song_position(&self, task: TaskId) -> Option<(i32, i32, i32)> {
        let a = self.audio.state.lock();
        if a.engine.song_owner() != Some(task.0) {
            return None;
        }
        a.engine.song_position()
    }

    /// Mutes track `ch` (1-based) of the song, if `task` started it.
    pub fn audio_song_mute(&self, task: TaskId, ch: i32, on: bool) {
        let mut a = self.audio.state.lock();
        let st = &mut *a;
        if st.engine.song_owner() == Some(task.0) {
            st.engine.mute(&mut st.synth, ch, on);
        }
    }

    pub fn audio_song_preview(&self, task: TaskId, song: Arc<LoadedSong>, ch: i32, note: i32, inst: i32) {
        let mut a = self.audio.state.lock();
        let st = &mut *a;
        st.engine.preview(&mut st.synth, task.0, song, ch, note, inst);
    }

    /// The platform's audio pull: 22050 Hz unsigned 8-bit mono. Master
    /// volume scales each sample about 128 after rendering, outside the
    /// lock.
    pub fn render_audio(&self, buf: &mut [u8]) {
        let mask = {
            let mut a = self.audio.state.lock();
            let st = &mut *a;
            st.engine.render(&mut st.synth, buf);
            (0..NUM_VOICES)
                .filter(|&v| st.synth.voice(v).envelope_stage != EnvStage::Off)
                .fold(0u32, |m, v| m | (1 << v))
        };
        let volume = self.audio.master_volume.load(Ordering::Relaxed);
        if volume != 100 {
            for b in buf.iter_mut() {
                let s = ((*b as i32 - 128) * volume / 100).clamp(-128, 127);
                *b = (s + 128) as u8;
            }
        }
        self.audio.active_voice_mask.store(mask, Ordering::Relaxed);
    }

    /// A voice's oscillator step: which pitch it plays, for tests that
    /// check what a song or sound did. None for a voice that doesn't exist.
    pub fn audio_voice_increment(&self, voice: usize) -> Option<u32> {
        (voice < NUM_VOICES).then(|| self.audio.state.lock().synth.voice(voice).phase_increment)
    }

    pub fn set_master_volume(&self, percent: i32) {
        self.audio.master_volume.store(percent.clamp(0, 100), Ordering::Relaxed);
    }

    pub fn master_volume(&self) -> i32 {
        self.audio.master_volume.load(Ordering::Relaxed)
    }

    /// Voices not yet back to ENV_OFF as of the last render (sysmon's meter).
    pub fn active_voice_count(&self) -> u32 {
        self.audio.active_voice_mask.load(Ordering::Relaxed).count_ones()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::*;
    use acid_testkit::FakePlatform;
    use alloc::sync::Arc;

    fn kernel() -> Arc<Kernel> {
        Kernel::new(FakePlatform::new("."))
    }

    fn render(k: &Kernel, n: usize) -> Vec<u8> {
        let mut buf = vec![0u8; n];
        k.render_audio(&mut buf);
        buf
    }

    #[test]
    fn a_voice_taken_over_survives_the_first_owners_exit() {
        // From the Phase 3 review: B re-notes A's voice; A exiting must not
        // stop B's note (note_on overwrites the owner).
        let k = kernel();
        k.audio_note_on(TaskId(1), 0, 40, 80);
        k.audio_note_on(TaskId(2), 0, 44, 80);
        k.audio_release_owner(TaskId(1));
        let st = k.audio.state.lock();
        assert_eq!(st.owners[0], Some(TaskId(2)));
        assert_eq!(st.synth.voice(0).envelope_stage, EnvStage::Attack);
    }

    #[test]
    fn a_released_voice_drops_its_arpeggio() {
        // A arps voice 0 and exits; B's plain note on it must not resume A's arp.
        let k = kernel();
        k.audio_note_on(TaskId(1), 0, 40, 80);
        k.audio_trigger_arp(0, [40, 44, 47, 52], 4, 30);
        k.audio_release_owner(TaskId(1));
        k.audio_note_on(TaskId(2), 0, 49, 80);
        assert!(!k.audio.state.lock().synth.voice(0).arp_active);
    }

    #[test]
    fn note_on_sounds_records_the_owner_and_counts() {
        let k = kernel();
        k.audio_note_on(TaskId(1), 0, 49, 100);
        assert!(render(&k, 512).iter().any(|&b| b != 128));
        assert_eq!(k.active_voice_count(), 1);
        assert_eq!(k.audio.state.lock().owners[0], Some(TaskId(1)));
    }

    #[test]
    fn note_on_volume_sets_sustain_level() {
        let k = kernel();
        k.audio_note_on(TaskId(1), 2, 40, 50);
        assert_eq!(k.audio.state.lock().synth.voice(2).sustain_level, (50 * ENV_FULL / 100) << 8);
        k.audio_note_on(TaskId(1), 3, 40, 250);
        assert_eq!(k.audio.state.lock().synth.voice(3).sustain_level, (100 * ENV_FULL / 100) << 8);
    }

    #[test]
    fn note_off_clears_owner_and_arp() {
        let k = kernel();
        k.audio_trigger_arp(0, [40, 44, 47, 52], 4, 30);
        k.audio_note_on(TaskId(1), 0, 40, 80);
        k.audio_note_off(0);
        let st = k.audio.state.lock();
        assert!(!st.synth.voice(0).arp_active);
        assert_eq!(st.owners[0], None);
        assert_eq!(st.synth.voice(0).envelope_stage, EnvStage::Release);
    }

    #[test]
    fn release_owner_gates_off_only_that_tasks_voices() {
        let k = kernel();
        k.audio_note_on(TaskId(1), 0, 40, 80);
        k.audio_note_on(TaskId(2), 1, 44, 80);
        k.audio_release_owner(TaskId(1));
        let st = k.audio.state.lock();
        assert_eq!(st.synth.voice(0).envelope_stage, EnvStage::Release);
        assert_eq!(st.synth.voice(1).envelope_stage, EnvStage::Attack);
        assert_eq!((st.owners[0], st.owners[1]), (None, Some(TaskId(2))));
    }

    #[test]
    fn an_exiting_app_releases_its_voices() {
        let (_p, k, rx) = setup();
        let t = k.spawn_app(req(0, 30, 10, 10)).unwrap();
        recv(&rx);
        k.audio_note_on(t, 0, 40, 80);
        k.close_window(t);
        wait_until(|| k.audio.state.lock().owners[0].is_none());
        assert_eq!(k.audio.state.lock().synth.voice(0).envelope_stage, EnvStage::Release);
    }

    #[test]
    fn commands_match_the_bare_synth() {
        // Each kernel command must equal calling the synth directly.
        let k = kernel();
        k.audio_configure_osc(0, 1, 50);
        k.audio_configure_voice(0, 1, 5, 20, 60, 30);
        k.audio_configure_filter(80, 6, 1);
        k.audio_note_on(TaskId(1), 0, 45, 100);
        let got = render(&k, 2048);
        let mut s = Synth::new();
        s.set_voice_waveform(0, 1);
        s.set_duty(0, 50);
        s.set_voice_filter_route(0, 1);
        s.set_adsr(0, 5, 20, 60, 30);
        s.set_filter_cutoff(80);
        s.set_filter_resonance(6);
        s.set_filter_mode(1);
        s.set_ona(0, 45);
        s.voice_mut(0).sustain_level = (100 * ENV_FULL / 100) << 8;
        s.gate_on(0);
        let mut want = vec![0u8; 2048];
        s.render(&mut want);
        assert_eq!(got, want);
    }

    #[test]
    fn master_volume_scales_output_and_clamps() {
        let k = kernel();
        assert_eq!(k.master_volume(), 100);
        k.set_master_volume(150);
        assert_eq!(k.master_volume(), 100);
        k.set_master_volume(-5);
        assert_eq!(k.master_volume(), 0);
        k.audio_note_on(TaskId(1), 0, 49, 100);
        assert!(render(&k, 256).iter().all(|&b| b == 128), "volume 0 is silence");

        let k = kernel();
        k.set_master_volume(50);
        k.audio_note_on(TaskId(1), 0, 49, 100);
        let half = render(&k, 512);
        let k_full = kernel();
        k_full.audio_note_on(TaskId(1), 0, 49, 100);
        let full = render(&k_full, 512);
        for (h, f) in half.iter().zip(&full) {
            let want = ((*f as i32 - 128) * 50 / 100).clamp(-128, 127) + 128;
            assert_eq!(*h as i32, want);
        }
    }

    #[test]
    fn active_count_falls_to_zero_after_release() {
        let k = kernel();
        k.audio_note_on(TaskId(1), 0, 49, 100);
        k.audio_note_on(TaskId(1), 1, 52, 100);
        render(&k, 64);
        assert_eq!(k.active_voice_count(), 2);
        k.audio_note_off(0);
        k.audio_note_off(1);
        render(&k, 64);
        assert_eq!(k.active_voice_count(), 0, "default release is instant");
    }

    #[test]
    fn out_of_range_voices_are_ignored() {
        let k = kernel();
        for v in [-1, 8, 99] {
            k.audio_note_on(TaskId(1), v, 40, 80);
            k.audio_note_off(v);
            k.audio_configure_voice(v, 1, 1, 1, 50, 1);
            k.audio_trigger_arp(v, [40, 44, 47, 52], 4, 30);
            k.audio_configure_osc(v, 1, 50);
            k.audio_set_ring_partner(v, 1);
        }
        assert!(render(&k, 128).iter().all(|&b| b == 128));
        assert_eq!(k.active_voice_count(), 0);
    }

    #[test]
    fn ring_partner_negative_clears() {
        let k = kernel();
        k.audio_set_ring_partner(2, 3);
        assert_eq!(k.audio.state.lock().synth.voice(2).ring_partner, 3);
        k.audio_set_ring_partner(2, -1);
        assert_eq!(k.audio.state.lock().synth.voice(2).ring_partner, -1);
    }

    #[test]
    fn arp_retrigger_in_app_call_order_matches_the_bare_synth() {
        // Apps call play_note then trigger_arp, later stop_note, then the
        // same again. The second trigger must restart the arp from step 0.
        // 1000 samples is not a multiple of the 30 ms arp step (661 samples),
        // so the arp is mid-step when the note stops.
        let k = kernel();
        k.audio_note_on(TaskId(1), 0, 45, 90);
        k.audio_trigger_arp(0, [40, 44, 47, 52], 4, 30);
        let mut got = render(&k, 1000);
        k.audio_note_off(0);
        got.extend(render(&k, 500));
        k.audio_note_on(TaskId(1), 0, 45, 90);
        k.audio_trigger_arp(0, [40, 44, 47, 52], 4, 30);
        got.extend(render(&k, 1500));

        let mut s = Synth::new();
        let mut want = Vec::new();
        let mut chunk = |s: &mut Synth, n: usize| {
            let mut b = vec![0u8; n];
            s.render(&mut b);
            want.extend(b);
        };
        let play_and_arp = |s: &mut Synth| {
            s.set_ona(0, 45);
            s.voice_mut(0).sustain_level = (90 * ENV_FULL / 100) << 8;
            s.gate_on(0);
            for (i, n) in [40, 44, 47, 52].iter().enumerate() {
                s.set_arp_note(0, i as i32, *n);
            }
            s.set_arp_rate(0, 30);
            s.voice_mut(0).arp_step = 0;
            s.voice_mut(0).arp_step_counter = 0;
            s.arp_on(0, 4);
        };
        play_and_arp(&mut s);
        chunk(&mut s, 1000);
        s.gate_off(0);
        s.voice_mut(0).arp_active = false;
        chunk(&mut s, 500);
        play_and_arp(&mut s);
        chunk(&mut s, 1500);
        assert_eq!(got, want);
    }

    const FOUR: &str = "acid-track 2\ntitle t\nspeed 2\ninstrument 01 \"Lead\"  wave saw  adsr 0 0 100 0  duty 50\norder 00 loop 0\n\npattern 00 2\nC-4 01 . .. | ... .. . .. | ... .. . .. | ... .. . .. | ... .. . .. | ... .. . .. | ... .. . .. | ... .. . ..\n=== .. . .. | ... .. . .. | ... .. . .. | ... .. . .. | ... .. . .. | ... .. . .. | ... .. . .. | ... .. . ..\n";

    fn song() -> Arc<acid_sound::player::LoadedSong> {
        Arc::new(acid_sound::player::LoadedSong::plain(acid_sound::song::parse(FOUR).unwrap()))
    }

    #[test]
    fn a_song_sounds_the_same_whatever_the_buffer_size() {
        let (a, b) = (kernel(), kernel());
        a.audio_song_play(TaskId(1), 1, song(), 0, 0);
        b.audio_song_play(TaskId(1), 1, song(), 0, 0);
        let whole = render(&a, 1900);
        let mut parts = Vec::new();
        for _ in 0..9 {
            parts.extend(render(&b, 100));
        }
        parts.extend(render(&b, 1000));
        assert!(whole.iter().any(|&s| s != 128), "the song must be audible");
        assert_eq!(whole, parts);
    }

    #[test]
    fn an_exiting_app_stops_its_song_and_sounds() {
        let k = kernel();
        k.audio_song_play(TaskId(1), 1, song(), 0, 0);
        let prog = Arc::new(acid_sound::compile("gate on\nwait 100").unwrap());
        k.audio_sound_play(TaskId(1), prog, 0, 40).unwrap();
        render(&k, 441);
        k.audio_release_owner(TaskId(1));
        assert_eq!(k.audio_song_position(TaskId(1)), None);
        assert_eq!(k.audio.state.lock().engine.sound_count(), 0);
    }

    #[test]
    fn only_the_owner_stops_or_mutes_its_song() {
        let k = kernel();
        k.audio_song_play(TaskId(1), 1, song(), 0, 0);
        render(&k, 441);
        k.audio_song_mute(TaskId(2), 1, true);
        k.audio_song_stop(TaskId(2));
        assert!(k.audio_song_position(TaskId(1)).is_some());
        assert!(!k.audio.state.lock().engine.song_player().unwrap().muted(0));
        k.audio_song_stop(TaskId(1));
        assert_eq!(k.audio_song_position(TaskId(1)), None);
    }

    #[test]
    fn only_the_owner_sees_its_song_position() {
        let k = kernel();
        k.audio_song_play(TaskId(1), 1, song(), 0, 0);
        render(&k, 441);
        assert!(k.audio_song_position(TaskId(1)).is_some());
        assert_eq!(k.audio_song_position(TaskId(2)), None, "another app's song is not ours to follow");
    }

    #[test]
    fn sounds_avoid_voices_apps_are_playing() {
        let k = kernel();
        // The default envelope is instant, so use a 2 s attack to keep the
        // voices in Attack after one tick of samples.
        for v in 0..NUM_VOICES as i32 {
            k.audio_configure_voice(v, 0, 2000, 0, 100, 0);
        }
        k.audio_note_on(TaskId(1), 7, 40, 80);
        let prog = Arc::new(acid_sound::compile("gate on\nwait 100").unwrap());
        k.audio_sound_play(TaskId(2), prog, 0, 52).unwrap();
        render(&k, 441);
        let st = k.audio.state.lock();
        assert_eq!(st.synth.voice(6).envelope_stage, EnvStage::Attack);
        assert_eq!(st.synth.voice(6).phase_increment, acid_synth::ONA_PHASE_INCREMENT[51]);
    }

    #[test]
    fn preview_plays_through_the_kernel() {
        let k = kernel();
        k.audio_song_preview(TaskId(1), song(), 2, 40, 1);
        assert_eq!(k.audio.state.lock().synth.voice(1).envelope_stage, EnvStage::Attack, "track 2 is voice 1");
        k.audio_song_preview(TaskId(1), song(), 8, 40, 1);
        assert_eq!(k.audio.state.lock().synth.voice(7).envelope_stage, EnvStage::Attack, "track 8 is voice 7");
    }

    #[test]
    fn extreme_song_values_are_safe() {
        let k = kernel();
        let prog = Arc::new(acid_sound::compile("gate on\nwait 100").unwrap());
        k.audio_song_play(TaskId(1), 1, song(), i32::MIN, i32::MAX);
        k.audio_song_mute(TaskId(1), i32::MIN, true);
        k.audio_song_preview(TaskId(1), song(), i32::MAX, i32::MIN, i32::MAX);
        let _ = k.audio_sound_play(TaskId(1), prog, 0, i32::MAX);
        render(&k, 441);
    }
}
