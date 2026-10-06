//! The engine the kernel ticks: one song, a preview player for the
//! tracker, and up to MAX_SOUNDS running .snd sounds, stepped every
//! TICK_SAMPLES samples while the synth renders. Owners are app task ids.
//! While a song plays the synth mixes with SONG_MIX_SHIFT bits of headroom,
//! because a song sums many full-scale voices and would otherwise clip.

//!
//! Voice choice for a sound: it takes the highest-numbered free voice,
//! avoiding voices apps play directly (`busy`) and voices other sounds hold.
//! While a song plays, every voice except the song's donor voice is
//! reserved for it. A voice a sound takes is lent to the song and preview
//! players, and handed back when the sound ends.

use alloc::sync::Arc;
use alloc::vec::Vec;

use acid_synth::{Synth, NUM_VOICES};

use crate::player::{LoadedSong, Player};
use crate::program::{BlockKind, Program};
use crate::song::CHANNELS;
use crate::vm::{Clock, Env, Instance, SongCmd, State, SONG_CMDS_MAX};
use crate::TICK_SAMPLES;

pub const MAX_SOUNDS: usize = 16;

/// Mixer headroom (divide by 4) while a song plays.
pub const SONG_MIX_SHIFT: i32 = 2;

struct Sound {
    id: u32,
    owner: u32,
    prog: Arc<Program>,
    inst: Instance,
}

struct SongSlot {
    owner: u32,
    /// The app's song handle; 0 for a song a script started.
    handle: u32,
    player: Player,
}

pub struct Engine {
    song: Option<SongSlot>,
    preview: Option<SongSlot>,
    sounds: Vec<Sound>,
    cmds: Vec<SongCmd>,
    until_tick: u32,
    ticks: u32,
    next_id: u32,
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

impl Engine {
    pub fn new() -> Self {
        Self {
            song: None,
            preview: None,
            sounds: Vec::with_capacity(MAX_SOUNDS),
            cmds: Vec::with_capacity(SONG_CMDS_MAX),
            until_tick: 0,
            ticks: 0,
            next_id: 1,
        }
    }

    pub fn ticks(&self) -> u32 {
        self.ticks
    }

    pub fn sound_count(&self) -> usize {
        self.sounds.len()
    }

    pub fn song_player(&self) -> Option<&Player> {
        self.song.as_ref().map(|s| &s.player)
    }

    pub fn song_owner(&self) -> Option<u32> {
        self.song.as_ref().map(|s| s.owner)
    }

    pub fn song_position(&self) -> Option<(i32, i32, i32)> {
        self.song.as_ref().filter(|s| s.player.playing).map(|s| s.player.position())
    }

    /// Renders `buf`, ticking at every TICK_SAMPLES boundary. The counter
    /// carries across calls, so tick timing doesn't depend on buffer sizes.
    pub fn render(&mut self, synth: &mut Synth, buf: &mut [u8]) {
        let mut done = 0;
        while done < buf.len() {
            if self.until_tick == 0 {
                self.tick(synth);
                self.until_tick = TICK_SAMPLES;
            }
            let n = (self.until_tick as usize).min(buf.len() - done);
            synth.render(&mut buf[done..done + n]);
            done += n;
            self.until_tick -= n as u32;
        }
    }

    /// Commands from song script instruments are dropped: they have no
    /// program context, so they can't start songs. Preview commands are
    /// discarded too.
    pub fn tick(&mut self, synth: &mut Synth) {
        self.ticks = self.ticks.wrapping_add(1);
        if let Some(s) = self.song.as_mut() {
            self.cmds.clear();
            s.player.tick(synth, &mut self.cmds);
            let owner = s.owner;
            self.apply_cmds(synth, owner, None);
        }
        if let Some(p) = self.preview.as_mut() {
            self.cmds.clear();
            p.player.tick(synth, &mut self.cmds);
        }
        let clock = self.song.as_ref().map_or(Clock::default(), |s| s.player.clock());
        let mut i = 0;
        while i < self.sounds.len() {
            self.cmds.clear();
            let prog = self.sounds[i].prog.clone();
            let owner = self.sounds[i].owner;
            {
                let inst = &mut self.sounds[i].inst;
                if let Some(b) = prog.blocks.get(inst.block) {
                    inst.tick(b, &mut Env { synth: &mut *synth, clock, song_cmds: &mut self.cmds });
                } else {
                    inst.state = State::Done;
                }
            }
            self.apply_cmds(synth, owner, Some(&prog));
            if self.sounds[i].inst.state == State::Done {
                let s = self.sounds.remove(i);
                self.give_back(&s);
            } else {
                i += 1;
            }
        }
    }

    fn apply_cmds(&mut self, synth: &mut Synth, owner: u32, prog: Option<&Arc<Program>>) {
        // Stop, tempo, mute and jump act only on a song this owner started.
        for k in 0..self.cmds.len() {
            // Re-read each time: an earlier command may have replaced the song.
            let own = self.song_owner() == Some(owner);
            match self.cmds[k] {
                SongCmd::Play { song: Some(i), order } => {
                    let found = prog.and_then(|p| p.songs.get(i as usize)).and_then(|s| s.clone());
                    if let Some(ls) = found {
                        self.play_song(synth, owner, 0, ls, order, 0);
                    }
                }
                SongCmd::Play { song: None, order } if own => {
                    if let Some(s) = self.song.as_mut() {
                        s.player.jump(order);
                    }
                }
                SongCmd::Stop if own => self.stop_song(synth),
                SongCmd::Tempo(n) if own => {
                    if let Some(s) = self.song.as_mut() {
                        s.player.set_speed(n);
                    }
                }
                SongCmd::Mute(ch, on) if own => self.mute(synth, ch, on),
                SongCmd::Jump(o) if own => {
                    if let Some(s) = self.song.as_mut() {
                        s.player.jump(o);
                    }
                }
                _ => {}
            }
        }
    }

    fn sound_voices(&self) -> u8 {
        let mut m = 0u8;
        for s in &self.sounds {
            for v in s.inst.voices.iter().flatten() {
                m |= 1 << v;
            }
        }
        m
    }

    fn give_back(&mut self, s: &Sound) {
        for v in s.inst.voices.iter().flatten() {
            for slot in [self.song.as_mut(), self.preview.as_mut()].into_iter().flatten() {
                slot.player.take_back(*v);
            }
        }
    }

    /// Starts a `sound` block on free voices (see the module docs); `busy`
    /// is voices apps are playing directly. None when nothing is free.
    pub fn play_sound(&mut self, synth: &mut Synth, owner: u32, prog: Arc<Program>, block: usize, note: i32, busy: u8) -> Option<u32> {
        let b = prog.blocks.get(block)?;
        if b.kind != BlockKind::Sound || self.sounds.len() >= MAX_SOUNDS {
            return None;
        }
        let mut taken = busy | self.sound_voices();
        if let Some(s) = self.song.as_ref().filter(|s| s.player.playing) {
            taken |= !(1u8 << s.player.donor_voice());
        }
        let mut free = (0..NUM_VOICES as u8).rev().filter(|v| taken & (1 << v) == 0);
        let v1 = free.next()?;
        let v2 = if b.uses_v2 { free.next() } else { None };
        for v in [Some(v1), v2].into_iter().flatten() {
            for slot in [self.song.as_mut(), self.preview.as_mut()].into_iter().flatten() {
                slot.player.lend(v);
            }
        }
        let id = self.next_id;
        // Ids wrap but skip 0.
        self.next_id = self.next_id.wrapping_add(1).max(1);
        let mut inst = Instance::new(block, [Some(v1), v2], note, 0, self.ticks ^ id.rotate_left(16));
        inst.start(synth);
        self.sounds.push(Sound { id, owner, prog, inst });
        Some(id)
    }

    pub fn stop_sound(&mut self, synth: &mut Synth, owner: u32, id: u32) {
        if let Some(i) = self.sounds.iter().position(|s| s.id == id && s.owner == owner) {
            let mut s = self.sounds.remove(i);
            s.inst.stop(synth);
            self.give_back(&s);
        }
    }

    /// Replaces any playing song. Voices sounds hold stay lent to them.
    pub fn play_song(&mut self, synth: &mut Synth, owner: u32, handle: u32, song: Arc<LoadedSong>, order: i32, row: i32) {
        self.stop_song(synth);
        let mut player = Player::new(song, order.max(0) as usize, row.max(0) as usize);
        let held = self.sound_voices();
        for v in 0..NUM_VOICES as u8 {
            if held & (1 << v) != 0 {
                player.lend(v);
            }
        }
        self.song = Some(SongSlot { owner, handle, player });
        synth.set_mix_shift(SONG_MIX_SHIFT);
    }

    pub fn stop_song(&mut self, synth: &mut Synth) {
        if let Some(mut s) = self.song.take() {
            s.player.stop(synth);
        }
        synth.set_mix_shift(0);
    }

    /// Swaps edited data into the playing song if it came from this handle.
    pub fn update_song(&mut self, owner: u32, handle: u32, song: Arc<LoadedSong>) {
        if let Some(s) = self.song.as_mut().filter(|s| s.owner == owner && s.handle == handle) {
            s.player.replace_song(song);
        }
    }

    /// `ch` is 1-based.
    pub fn mute(&mut self, synth: &mut Synth, ch: i32, on: bool) {
        let Ok(ch) = usize::try_from(ch.saturating_sub(1)) else { return };
        if let Some(s) = self.song.as_mut() {
            s.player.set_muted(synth, ch, on);
        }
    }

    /// Sounds one note on channel `ch` (1-based) of `song`; note 0 is note-off.
    pub fn preview(&mut self, synth: &mut Synth, owner: u32, song: Arc<LoadedSong>, ch: i32, note: i32, inst: i32) {
        let Ok(ch) = usize::try_from(ch.saturating_sub(1)) else { return };
        if ch >= CHANNELS {
            return;
        }
        let same = self.preview.as_ref().is_some_and(|p| p.owner == owner && Arc::ptr_eq(p.player.song(), &song));
        if !same {
            if let Some(mut p) = self.preview.take() {
                p.player.stop(synth);
            }
            let mut player = Player::preview(song);
            let held = self.sound_voices();
            for v in 0..NUM_VOICES as u8 {
                if held & (1 << v) != 0 {
                    player.lend(v);
                }
            }
            self.preview = Some(SongSlot { owner, handle: 0, player });
        }
        let Some(p) = self.preview.as_mut() else { return };
        if note <= 0 {
            p.player.note_off(synth, ch);
        } else {
            p.player.trigger(synth, ch, note, None, inst.clamp(0, 255) as u8);
        }
    }

    /// An app exited: its sounds, its song and its preview stop.
    pub fn release_owner(&mut self, synth: &mut Synth, owner: u32) {
        let mut i = 0;
        while i < self.sounds.len() {
            if self.sounds[i].owner == owner {
                let mut s = self.sounds.remove(i);
                s.inst.stop(synth);
                self.give_back(&s);
            } else {
                i += 1;
            }
        }
        if self.song_owner() == Some(owner) {
            self.stop_song(synth);
        }
        if self.preview.as_ref().is_some_and(|p| p.owner == owner) {
            if let Some(mut p) = self.preview.take() {
                p.player.stop(synth);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compile::compile;
    use crate::song::parse;
    use alloc::format;
    use alloc::string::String;
    use alloc::vec;
    use acid_synth::EnvStage;

    fn prog(src: &str) -> Arc<Program> {
        Arc::new(compile(src).unwrap())
    }

    /// All four channels play a detuned two-voice C-4 for two rows.
    fn four(donor: u8) -> String {
        format!(
            "acid-track 1\ntitle t\nspeed 2\nsfx-donor {donor}\ninstrument 01 \"Lead\"  wave saw  adsr 0 0 100 0  duty 50  voice2 detune 6\norder 1  00 loop 0\norder 2  00 loop 0\norder 3  00 loop 0\norder 4  00 loop 0\n\npattern 00 2\nC-4 01 . .. ...\n... .. . .. ...\n"
        )
    }

    fn song(donor: u8) -> Arc<LoadedSong> {
        Arc::new(LoadedSong::plain(parse(&four(donor)).unwrap()))
    }

    fn render(e: &mut Engine, s: &mut Synth, n: usize) {
        let mut buf = vec![0u8; n];
        e.render(s, &mut buf);
    }

    const T: usize = TICK_SAMPLES as usize;

    /// The synth's default envelope is instant (0 ms attack), so one tick of
    /// samples carries a gated voice past Attack; a 2 s attack keeps it there.
    fn slow_attack(s: &mut Synth) {
        for v in 0..NUM_VOICES as i32 {
            s.set_adsr(v, 2000, 0, 100, 0);
        }
    }

    #[test]
    fn an_idle_engine_renders_like_the_bare_synth() {
        let (mut a, mut b) = (Synth::new(), Synth::new());
        for s in [&mut a, &mut b] {
            s.set_ona(0, 40);
            s.gate_on(0);
        }
        let mut e = Engine::new();
        let mut got = Vec::new();
        for n in [1, 100, 441, 1000, 7] {
            let mut buf = vec![0u8; n];
            e.render(&mut a, &mut buf);
            got.extend_from_slice(&buf);
        }
        let mut want = vec![0u8; got.len()];
        b.render(&mut want);
        assert_eq!(got, want);
    }

    #[test]
    fn ticks_land_every_441_samples_whatever_the_buffer() {
        let (mut e1, mut e2, mut s) = (Engine::new(), Engine::new(), Synth::new());
        render(&mut e1, &mut s, 1900);
        for _ in 0..9 {
            render(&mut e2, &mut s, 100);
        }
        render(&mut e2, &mut s, 1000);
        assert_eq!((e1.ticks(), e2.ticks()), (5, 5), "ticks at samples 0, 441, 882, 1323, 1764");
    }

    #[test]
    fn sounds_take_the_highest_free_voice() {
        let (mut e, mut s) = (Engine::new(), Synth::new());
        slow_attack(&mut s);
        let p = prog("gate on\nwait 10");
        assert!(e.play_sound(&mut s, 1, p.clone(), 0, 40, 0).is_some());
        assert!(e.play_sound(&mut s, 1, p.clone(), 0, 40, 0).is_some());
        assert_eq!(e.play_sound(&mut s, 1, p.clone(), 0, 40, 0b0011_1111), None, "7 and 6 are taken, the rest busy");
        render(&mut e, &mut s, T);
        assert_eq!(s.voice(7).envelope_stage, EnvStage::Attack);
        assert_eq!(s.voice(6).envelope_stage, EnvStage::Attack);
        assert_eq!(s.voice(5).envelope_stage, EnvStage::Off);
    }

    #[test]
    fn a_two_voice_sound_takes_two() {
        let (mut e, mut s) = (Engine::new(), Synth::new());
        slow_attack(&mut s);
        e.play_sound(&mut s, 1, prog("both gate on\nwait 10"), 0, 40, 0).unwrap();
        render(&mut e, &mut s, T);
        assert_eq!(s.voice(7).envelope_stage, EnvStage::Attack);
        assert_eq!(s.voice(6).envelope_stage, EnvStage::Attack);
    }

    #[test]
    fn only_sound_blocks_play_as_sounds() {
        let (mut e, mut s) = (Engine::new(), Synth::new());
        assert_eq!(e.play_sound(&mut s, 1, prog("instrument i\ngate on\nend"), 0, 40, 0), None);
        assert_eq!(e.play_sound(&mut s, 1, prog("wait 1"), 3, 40, 0), None, "no such block");
    }

    #[test]
    fn a_finished_sound_frees_its_voice() {
        let (mut e, mut s) = (Engine::new(), Synth::new());
        e.play_sound(&mut s, 1, prog("gate on\nwait 2"), 0, 40, 0).unwrap();
        render(&mut e, &mut s, 3 * T);
        assert_eq!(e.sound_count(), 0);
        e.play_sound(&mut s, 1, prog("gate on\nwait 2"), 0, 52, 0).unwrap();
        render(&mut e, &mut s, T);
        assert_eq!(s.voice(7).phase_increment, acid_synth::ONA_PHASE_INCREMENT[51], "voice 7 again");
    }

    #[test]
    fn during_a_song_sounds_borrow_only_the_donor_voice() {
        let (mut e, mut s) = (Engine::new(), Synth::new());
        e.play_song(&mut s, 1, 1, song(4), 0, 0);
        render(&mut e, &mut s, T);
        let p = prog("gate on\nwait 2");
        assert!(e.play_sound(&mut s, 2, p.clone(), 0, 40, 0).is_some());
        assert_eq!(e.song_player().unwrap().borrowed(), 1 << 7);
        assert_eq!(e.play_sound(&mut s, 2, p, 0, 40, 0), None, "everything else belongs to the song");
        render(&mut e, &mut s, 4 * T);
        assert_eq!(e.song_player().unwrap().borrowed(), 0, "handed back when the sound ends");
    }

    #[test]
    fn sfx_donor_moves_the_borrowed_voice() {
        let (mut e, mut s) = (Engine::new(), Synth::new());
        e.play_song(&mut s, 1, 1, song(1), 0, 0);
        e.play_sound(&mut s, 2, prog("gate on\nwait 2"), 0, 40, 0).unwrap();
        assert_eq!(e.song_player().unwrap().borrowed(), 1 << 1);
    }

    #[test]
    fn a_script_can_start_a_song() {
        let (mut e, mut s) = (Engine::new(), Synth::new());
        let mut p = compile("song \"s.trk\"\nplay").unwrap();
        p.songs = vec![Some(song(4))];
        e.play_sound(&mut s, 3, Arc::new(p), 0, 40, 0).unwrap();
        render(&mut e, &mut s, 2 * T);
        assert_eq!(e.song_position(), Some((0, 0, 1)));
        assert_eq!(e.song_owner(), Some(3));
    }

    #[test]
    fn a_script_can_mute_a_channel() {
        let (mut e, mut s) = (Engine::new(), Synth::new());
        // A 2 s release keeps the muted voice in Release for the tick's samples.
        let slow_release = Arc::new(LoadedSong::plain(parse(&four(4).replace("adsr 0 0 100 0", "adsr 0 0 100 2000")).unwrap()));
        e.play_song(&mut s, 1, 1, slow_release, 0, 0);
        render(&mut e, &mut s, T);
        e.play_sound(&mut s, 1, prog("mute 1"), 0, 40, 0).unwrap();
        render(&mut e, &mut s, T);
        assert!(e.song_player().unwrap().muted(0));
        assert_eq!(s.voice(0).envelope_stage, EnvStage::Release);
    }

    #[test]
    fn release_owner_stops_its_song_and_sounds() {
        let (mut e, mut s) = (Engine::new(), Synth::new());
        e.play_song(&mut s, 1, 1, song(4), 0, 0);
        e.play_sound(&mut s, 1, prog("gate on\nwait 50"), 0, 40, 0).unwrap();
        e.stop_song(&mut s);
        e.play_sound(&mut s, 2, prog("gate on\nwait 50"), 0, 40, 0).unwrap();
        e.play_song(&mut s, 1, 1, song(4), 0, 0);
        render(&mut e, &mut s, T);
        e.release_owner(&mut s, 1);
        assert_eq!(e.song_position(), None);
        assert_eq!(e.sound_count(), 1);
    }

    #[test]
    fn update_song_swaps_data_and_keeps_the_place() {
        let (mut e, mut s) = (Engine::new(), Synth::new());
        e.play_song(&mut s, 1, 5, song(4), 0, 0);
        render(&mut e, &mut s, 3 * T);
        assert_eq!(e.song_position(), Some((0, 1, 1)));
        let b = song(4);
        e.update_song(1, 5, b.clone());
        assert!(Arc::ptr_eq(e.song_player().unwrap().song(), &b));
        assert_eq!(e.song_position(), Some((0, 1, 1)));
        e.update_song(1, 6, song(4));
        assert!(Arc::ptr_eq(e.song_player().unwrap().song(), &b), "another handle's update is ignored");
    }

    #[test]
    fn preview_sounds_a_note_while_stopped() {
        let (mut e, mut s) = (Engine::new(), Synth::new());
        e.preview(&mut s, 1, song(4), 1, 40, 1);
        assert_eq!(s.voice(0).envelope_stage, EnvStage::Attack);
        e.preview(&mut s, 1, song(4), 1, 0, 0);
        assert_eq!(s.voice(0).envelope_stage, EnvStage::Release);
        e.preview(&mut s, 1, song(4), 9, 40, 1);
    }

    #[test]
    fn extreme_api_values_do_not_panic() {
        let (mut e, mut s) = (Engine::new(), Synth::new());
        let ex = [i32::MIN, i32::MAX];
        for &o in &ex {
            for &r in &ex {
                e.play_song(&mut s, 1, 1, song(4), o, r);
            }
        }
        for &c in &ex {
            e.mute(&mut s, c, true);
            e.mute(&mut s, c, false);
            for &n in &ex {
                for &i in &ex {
                    e.preview(&mut s, 1, song(4), c, n, i);
                }
            }
        }
        for &n in &ex {
            let _ = e.play_sound(&mut s, 1, prog("gate on\nwait 2"), 0, n, 0);
        }
        e.play_song(&mut s, 1, 1, song(4), i32::MAX, i32::MAX);
        render(&mut e, &mut s, T);
    }

    #[test]
    fn script_song_commands_only_touch_the_callers_own_song() {
        let (mut e, mut s) = (Engine::new(), Synth::new());
        e.play_song(&mut s, 1, 1, song(4), 0, 0);
        render(&mut e, &mut s, T);
        e.play_sound(&mut s, 2, prog("mute 1"), 0, 40, 0).unwrap();
        render(&mut e, &mut s, T);
        assert!(!e.song_player().unwrap().muted(0), "owner 2 can't mute owner 1's song");
        e.play_sound(&mut s, 2, prog("stop song"), 0, 40, 0).unwrap();
        render(&mut e, &mut s, T);
        assert!(e.song_position().is_some(), "owner 2 can't stop owner 1's song");
        e.play_sound(&mut s, 1, prog("mute 1"), 0, 40, 0).unwrap();
        render(&mut e, &mut s, T);
        assert!(e.song_player().unwrap().muted(0), "the owner still can");
        e.play_sound(&mut s, 1, prog("stop song"), 0, 40, 0).unwrap();
        render(&mut e, &mut s, T);
        assert_eq!(e.song_position(), None);
    }

    #[test]
    fn stop_sound_hands_the_voice_back() {
        let (mut e, mut s) = (Engine::new(), Synth::new());
        e.play_song(&mut s, 1, 1, song(4), 0, 0);
        let id = e.play_sound(&mut s, 2, prog("gate on\nwait 50"), 0, 40, 0).unwrap();
        assert_eq!(e.song_player().unwrap().borrowed(), 1 << 7);
        e.stop_sound(&mut s, 2, id);
        assert_eq!(e.song_player().unwrap().borrowed(), 0);
    }

    #[test]
    fn songs_play_with_headroom() {
        let (mut e, mut s) = (Engine::new(), Synth::new());
        assert_eq!(s.mix_shift(), 0);
        e.play_song(&mut s, 1, 1, song(4), 0, 0);
        assert_eq!(s.mix_shift(), SONG_MIX_SHIFT as u32);
        e.stop_song(&mut s);
        assert_eq!(s.mix_shift(), 0);
        e.play_song(&mut s, 1, 1, song(4), 0, 0);
        e.release_owner(&mut s, 1);
        assert_eq!(s.mix_shift(), 0);
    }

    #[test]
    fn release_owner_of_a_sound_hands_back_a_song_it_does_not_own() {
        let (mut e, mut s) = (Engine::new(), Synth::new());
        e.play_song(&mut s, 1, 1, song(4), 0, 0);
        e.play_sound(&mut s, 2, prog("gate on\nwait 50"), 0, 40, 0).unwrap();
        assert_eq!(e.song_player().unwrap().borrowed(), 1 << 7);
        e.release_owner(&mut s, 2);
        assert_eq!(e.song_player().unwrap().borrowed(), 0);
        assert_eq!(e.song_owner(), Some(1));
    }

    #[test]
    fn a_song_started_during_a_sound_starts_with_its_voice_lent() {
        let (mut e, mut s) = (Engine::new(), Synth::new());
        e.play_sound(&mut s, 2, prog("gate on\nwait 50"), 0, 40, 0).unwrap();
        e.play_song(&mut s, 1, 1, song(4), 0, 0);
        assert_eq!(e.song_player().unwrap().borrowed(), 1 << 7);
    }
}
