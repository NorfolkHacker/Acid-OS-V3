//! Plays a parsed song on the synth. Each channel steps through its own
//! order list, one row every `speed` ticks. Channel n plays voices 2n and
//! 2n+1; a voice lent to a sound effect is left alone until it comes back.

use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use alloc::vec::Vec;

use acid_synth::Synth;

use crate::pitch::{self, fine_pos, increment_at, FINE_MAX, FINE_STEPS};
use crate::program::Program;
use crate::song::{BuiltIn, Kind, Song, Voice2, CHANNELS, NOTE_NONE, NOTE_OFF};
use crate::vm::{Clock, Env, Instance, SongCmd, State};

/// A song ready to play. `scripts` maps an instrument number to the
/// program and instrument block of each script instrument that loaded.
#[derive(Debug)]
pub struct LoadedSong {
    pub song: Song,
    pub scripts: BTreeMap<u8, (Arc<Program>, usize)>,
}

impl LoadedSong {
    /// A song with no script instruments loaded (they play silence).
    pub fn plain(song: Song) -> Self {
        Self { song, scripts: BTreeMap::new() }
    }
}

/// What a row's command asks of that row only.
#[derive(Clone, Copy, Default)]
struct RowFx {
    slide: i32,
    vib_depth: i32,
    vib_speed: i32,
}

#[derive(Default)]
struct Chan {
    order_pos: usize,
    row: usize,
    /// The instrument column's last value.
    inst: u8,
    /// The instrument the sounding note started with.
    playing: u8,
    /// A built-in note is running its effects.
    sounding: bool,
    /// The started note's fine position, and where slides have taken it.
    base: i32,
    pos: i32,
    note2: Option<i32>,
    glide_to: Option<i32>,
    glide_speed: i32,
    fx: RowFx,
    vib_phase: i32,
    arp_step: usize,
    duty: i32,
    pwm_dir: i32,
    script: Option<Arc<Program>>,
    instances: [Option<Instance>; 2],
    muted: bool,
}

pub struct Player {
    song: Arc<LoadedSong>,
    /// False for a preview: no rows are read, only notes it is handed.
    pub playing: bool,
    speed: u8,
    tick: u8,
    chans: [Chan; CHANNELS],
    row_serial: u32,
    borrowed: u8,
    seed: u32,
}

fn pattern_len(song: &Song, ch: usize, order_pos: usize) -> usize {
    song.orders[ch]
        .entries
        .get(order_pos)
        .and_then(|e| song.patterns.get(&e.pattern))
        .map_or(1, |p| p.len())
}

/// A triangle LFO, -16..=16 over 64 steps.
fn tri(phase: i32) -> i32 {
    let p = phase & 63;
    if p < 16 {
        p
    } else if p < 48 {
        32 - p
    } else {
        p - 64
    }
}

/// Where a built-in's two voices sit; `wobble` is vibrato plus arp.
fn positions(c: &Chan, b: &BuiltIn, wobble: i32) -> (i32, Option<i32>) {
    let p1 = c.pos + wobble;
    let p2 = match (c.note2, b.voice2) {
        (Some(n2), _) => Some(n2 + (c.pos - c.base) + wobble),
        (None, Voice2::Detune(n)) => Some(p1 + n),
        (None, Voice2::Octave) => Some(p1 + 12 * FINE_STEPS),
        (None, Voice2::Fifth) => Some(p1 + 7 * FINE_STEPS),
        (None, Voice2::Ring) => Some(p1),
        (None, Voice2::Off) => None,
    };
    (p1, p2)
}

impl Player {
    pub fn new(song: Arc<LoadedSong>, order: usize, row: usize) -> Self {
        let speed = song.song.speed.max(1);
        let mut p = Self { song, playing: true, speed, tick: 0, chans: Default::default(), row_serial: 0, borrowed: 0, seed: 1 };
        p.seek(order, row);
        p
    }

    /// A player that only sounds the notes it is handed (the tracker's preview).
    pub fn preview(song: Arc<LoadedSong>) -> Self {
        let mut p = Self::new(song, 0, 0);
        p.playing = false;
        p
    }

    pub fn song(&self) -> &Arc<LoadedSong> {
        &self.song
    }

    pub fn clock(&self) -> Clock {
        Clock {
            playing: self.playing,
            order: self.chans[0].order_pos as i32,
            row: self.chans[0].row as i32,
            row_serial: self.row_serial,
        }
    }

    /// Channel 1's order position and row, and the tick within the row.
    pub fn position(&self) -> (i32, i32, i32) {
        (self.chans[0].order_pos as i32, self.chans[0].row as i32, self.tick as i32)
    }

    pub fn borrowed(&self) -> u8 {
        self.borrowed
    }

    /// The voice sound effects borrow: the donor channel's second voice.
    pub fn donor_voice(&self) -> u8 {
        (self.song.song.sfx_donor.clamp(1, CHANNELS as u8) - 1) * 2 + 1
    }

    pub fn muted(&self, ch: usize) -> bool {
        self.chans.get(ch).is_some_and(|c| c.muted)
    }

    fn seek(&mut self, order: usize, row: usize) {
        let song = self.song.clone();
        for (ch, c) in self.chans.iter_mut().enumerate() {
            c.order_pos = order.min(song.song.orders[ch].entries.len().saturating_sub(1));
            c.row = if row < pattern_len(&song.song, ch, c.order_pos) { row } else { 0 };
        }
        self.tick = 0;
    }

    /// Every channel to order position `order`, row 0, playing.
    pub fn jump(&mut self, order: i32) {
        self.seek(order.max(0) as usize, 0);
        self.playing = true;
    }

    pub fn set_speed(&mut self, s: i32) {
        if (1..=31).contains(&s) {
            self.speed = s as u8;
        }
    }

    /// Swaps in edited song data, keeping each channel's place where it still exists.
    pub fn replace_song(&mut self, song: Arc<LoadedSong>) {
        self.speed = song.song.speed.max(1);
        for (ch, c) in self.chans.iter_mut().enumerate() {
            c.order_pos = c.order_pos.min(song.song.orders[ch].entries.len().saturating_sub(1));
            if c.row >= pattern_len(&song.song, ch, c.order_pos) {
                c.row = 0;
            }
        }
        self.song = song;
        if self.tick >= self.speed {
            self.tick = 0;
        }
    }

    fn voice(&self, ch: usize, k: usize) -> Option<i32> {
        let v = ch * 2 + k;
        (self.borrowed & (1 << v) == 0).then_some(v as i32)
    }

    /// Lends voice `v` to a sound effect: the player and its scripts stop touching it.
    pub fn lend(&mut self, v: u8) {
        if v as usize >= CHANNELS * 2 {
            return;
        }
        self.borrowed |= 1 << v;
        for c in &mut self.chans {
            for i in c.instances.iter_mut().flatten() {
                i.drop_voice(v);
            }
        }
    }

    /// Takes voice `v` back; it sounds again from its channel's next note.
    pub fn take_back(&mut self, v: u8) {
        if (v as usize) < CHANNELS * 2 {
            self.borrowed &= !(1 << v);
        }
    }

    pub fn tick(&mut self, synth: &mut Synth, cmds: &mut Vec<SongCmd>) {
        if self.playing && self.tick == 0 {
            for ch in 0..CHANNELS {
                self.read_row(synth, ch);
            }
            self.row_serial = self.row_serial.wrapping_add(1);
        }
        for ch in 0..CHANNELS {
            self.effects(synth, ch);
        }
        let clock = self.clock();
        for c in &mut self.chans {
            let Some(prog) = c.script.clone() else { continue };
            for slot in &mut c.instances {
                let Some(inst) = slot else { continue };
                let Some(block) = prog.blocks.get(inst.block) else {
                    *slot = None;
                    continue;
                };
                inst.tick(block, &mut Env { synth: &mut *synth, clock, song_cmds: &mut *cmds });
                if inst.state == State::Done {
                    *slot = None;
                }
            }
        }
        if self.playing {
            self.tick += 1;
            if self.tick >= self.speed {
                self.tick = 0;
                for ch in 0..CHANNELS {
                    self.advance(ch);
                }
            }
        }
    }

    fn read_row(&mut self, synth: &mut Synth, ch: usize) {
        let song = self.song.clone();
        let (order_pos, row_i) = (self.chans[ch].order_pos, self.chans[ch].row);
        let Some(entry) = song.song.orders[ch].entries.get(order_pos).copied() else { return };
        let Some(row) = song.song.patterns.get(&entry.pattern).and_then(|p| p.get(row_i)).copied() else { return };
        self.chans[ch].fx = RowFx::default();
        if row.inst != 0 {
            self.chans[ch].inst = row.inst;
        }
        // A mute only stops the channel changing its voices; song commands still run.
        if self.chans[ch].muted {
            self.command(synth, ch, row.cmd, row.param);
            return;
        }
        let shift = |n: u8| (n as i32 + entry.transpose as i32).clamp(pitch::ONA_MIN, pitch::ONA_MAX);
        match row.note {
            NOTE_NONE => {}
            NOTE_OFF => self.note_off(synth, ch),
            n if row.cmd == b'3' => self.chans[ch].glide_to = Some(fine_pos(shift(n))),
            n => {
                let note2 = (row.note2 != NOTE_NONE).then(|| shift(row.note2));
                let inst = self.chans[ch].inst;
                self.trigger(synth, ch, shift(n), note2, inst);
            }
        }
        self.command(synth, ch, row.cmd, row.param);
    }

    fn command(&mut self, synth: &mut Synth, ch: usize, cmd: u8, param: u8) {
        let p = param as i32;
        if self.chans[ch].muted && matches!(cmd, b'1' | b'2' | b'4' | b'8' | b'9') {
            return;
        }
        match cmd {
            b'1' => self.chans[ch].fx.slide = p,
            b'2' => self.chans[ch].fx.slide = -p,
            b'3' => self.chans[ch].glide_speed = p.max(1),
            b'4' => {
                let fx = &mut self.chans[ch].fx;
                fx.vib_depth = p >> 4;
                fx.vib_speed = p & 15;
            }
            b'8' => {
                for k in 0..2 {
                    if let Some(v) = self.voice(ch, k) {
                        synth.set_voice_waveform(v, p & 3);
                    }
                }
            }
            b'9' => {
                let d = p.clamp(1, 99);
                self.chans[ch].duty = d;
                for k in 0..2 {
                    if let Some(v) = self.voice(ch, k) {
                        synth.set_duty(v, d);
                    }
                }
            }
            b'A' => synth.set_filter_cutoff(p),
            b'F' => self.set_speed(p),
            _ => {}
        }
    }

    /// Starts `ona` (and `note2`, from the second column) on channel `ch`
    /// with instrument `inst`. A missing instrument plays silence.
    pub fn trigger(&mut self, synth: &mut Synth, ch: usize, ona: i32, note2: Option<i32>, inst: u8) {
        if ch >= CHANNELS {
            return;
        }
        // `ona` can come from an app call: keep every position in range.
        let ona = ona.clamp(pitch::ONA_MIN, pitch::ONA_MAX);
        let note2 = note2.map(|n| n.clamp(pitch::ONA_MIN, pitch::ONA_MAX));
        let song = self.song.clone();
        self.seed = self.seed.wrapping_add(1);
        let seed = self.seed;
        let (v1, v2) = (self.voice(ch, 0), self.voice(ch, 1));
        let c = &mut self.chans[ch];
        c.instances = [None, None];
        c.script = None;
        c.sounding = false;
        if c.muted {
            return;
        }
        // A new note stops whatever the channel was playing, even if it
        // then plays nothing (a missing instrument is silence).
        for v in [v1, v2].into_iter().flatten() {
            synth.gate_off(v);
            synth.clear_ring_partner(v);
        }
        c.playing = inst;
        c.base = fine_pos(ona);
        c.pos = c.base;
        c.note2 = note2.map(fine_pos);
        c.glide_to = None;
        c.vib_phase = 0;
        c.arp_step = 0;
        c.pwm_dir = 1;
        let Some(instrument) = song.song.instruments.get(&inst) else { return };
        match &instrument.kind {
            Kind::BuiltIn(b) => {
                c.duty = b.duty;
                c.sounding = true;
                let (p1, p2) = positions(c, b, 0);
                for (v, p) in [(v1, Some(p1)), (v2, p2)] {
                    let Some(v) = v else { continue };
                    let Some(p) = p else {
                        synth.gate_off(v);
                        continue;
                    };
                    synth.set_voice_waveform(v, b.wave);
                    synth.set_adsr(v, b.adsr[0], b.adsr[1], b.adsr[2], b.adsr[3]);
                    synth.set_duty(v, b.duty);
                    synth.set_voice_filter_route(v, b.filter.is_some() as i32);
                    synth.clear_ring_partner(v);
                    let vo = synth.voice_mut(v as usize);
                    vo.arp_active = false;
                    vo.phase_increment = increment_at(p);
                }
                if let (Voice2::Ring, None, Some(a), Some(bv)) = (b.voice2, note2, v1, v2) {
                    synth.set_ring_partner(a, bv);
                }
                if let Some((mode, cut, res)) = b.filter {
                    synth.set_filter_mode(mode);
                    synth.set_filter_cutoff(cut);
                    synth.set_filter_resonance(res);
                }
                for (v, p) in [(v1, Some(p1)), (v2, p2)] {
                    if let (Some(v), Some(_)) = (v, p) {
                        synth.gate_on(v);
                    }
                }
            }
            Kind::Script { .. } => {
                let Some((prog, block)) = song.scripts.get(&inst) else { return };
                let as_u8 = |v: Option<i32>| v.map(|v| v as u8);
                let mut start = |voices: [Option<u8>; 2], note: i32, n2: i32, seed: u32| {
                    let mut i = Instance::new(*block, voices, note, n2, seed);
                    i.start(synth);
                    i
                };
                c.instances = match note2 {
                    Some(n2) => [
                        Some(start([as_u8(v1), None], ona, n2, seed)),
                        Some(start([as_u8(v2), None], n2, n2, seed ^ 0x5A5A)),
                    ],
                    None => [Some(start([as_u8(v1), as_u8(v2)], ona, 0, seed)), None],
                };
                c.script = Some(prog.clone());
            }
        }
    }

    /// Note-off: a built-in releases its gates; a script runs `on release`.
    pub fn note_off(&mut self, synth: &mut Synth, ch: usize) {
        if ch >= CHANNELS {
            return;
        }
        let (v1, v2) = (self.voice(ch, 0), self.voice(ch, 1));
        let c = &mut self.chans[ch];
        if let Some(prog) = c.script.clone() {
            for inst in c.instances.iter_mut().flatten() {
                if let Some(block) = prog.blocks.get(inst.block) {
                    inst.release(block, synth);
                }
            }
        } else {
            for v in [v1, v2].into_iter().flatten() {
                synth.gate_off(v);
            }
        }
    }

    /// Built-in instruments' per-tick work: slides, glide, vibrato, arp, PWM.
    fn effects(&mut self, synth: &mut Synth, ch: usize) {
        let song = self.song.clone();
        let (v1, v2) = (self.voice(ch, 0), self.voice(ch, 1));
        let c = &mut self.chans[ch];
        if c.muted || !c.sounding {
            return;
        }
        let Some(Kind::BuiltIn(b)) = song.song.instruments.get(&c.playing).map(|i| &i.kind) else { return };
        c.pos = (c.pos + c.fx.slide).clamp(0, FINE_MAX);
        if let Some(t) = c.glide_to {
            let step = c.glide_speed.max(1);
            c.pos = if c.pos < t { (c.pos + step).min(t) } else { (c.pos - step).max(t) };
            if c.pos == t {
                c.glide_to = None;
            }
        }
        let (depth, speed) = if c.fx.vib_depth > 0 { (c.fx.vib_depth, c.fx.vib_speed) } else { b.vib };
        c.vib_phase = (c.vib_phase + speed) & 63;
        let vib = depth * tri(c.vib_phase) / 4;
        let arp = if b.arp.is_empty() {
            0
        } else {
            let k = c.arp_step % (b.arp.len() + 1);
            c.arp_step = c.arp_step.wrapping_add(1);
            if k == 0 { 0 } else { b.arp[k - 1] * FINE_STEPS }
        };
        if b.pwm != 0 {
            c.duty += b.pwm * c.pwm_dir;
            if c.duty >= 90 {
                c.duty = 90;
                c.pwm_dir = -1;
            } else if c.duty <= 10 {
                c.duty = 10;
                c.pwm_dir = 1;
            }
            for v in [v1, v2].into_iter().flatten() {
                synth.set_duty(v, c.duty);
            }
        }
        let (p1, p2) = positions(c, b, vib + arp);
        if let Some(v) = v1 {
            synth.voice_mut(v as usize).phase_increment = increment_at(p1);
        }
        if let (Some(v), Some(p)) = (v2, p2) {
            synth.voice_mut(v as usize).phase_increment = increment_at(p);
        }
    }

    fn advance(&mut self, ch: usize) {
        let song = &self.song.song;
        let c = &mut self.chans[ch];
        c.row += 1;
        if c.row >= pattern_len(song, ch, c.order_pos) {
            c.row = 0;
            c.order_pos += 1;
            if c.order_pos >= song.orders[ch].entries.len() {
                c.order_pos = song.orders[ch].loop_to;
            }
        }
    }

    /// Silences every voice the player holds and stops playing.
    pub fn stop(&mut self, synth: &mut Synth) {
        for ch in 0..CHANNELS {
            for k in 0..2 {
                if let Some(v) = self.voice(ch, k) {
                    synth.gate_off(v);
                }
            }
            let c = &mut self.chans[ch];
            c.instances = [None, None];
            c.script = None;
            c.sounding = false;
        }
        self.playing = false;
    }

    /// A muted channel keeps its place but leaves its voices silent.
    pub fn set_muted(&mut self, synth: &mut Synth, ch: usize, on: bool) {
        if ch >= CHANNELS {
            return;
        }
        if on {
            for k in 0..2 {
                if let Some(v) = self.voice(ch, k) {
                    synth.gate_off(v);
                }
            }
            let c = &mut self.chans[ch];
            c.instances = [None, None];
            c.script = None;
            c.sounding = false;
        }
        self.chans[ch].muted = on;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compile::compile;
    use crate::song::parse;
    use alloc::format;
    use alloc::string::String;
    use acid_synth::{EnvStage, Waveform, ONA_PHASE_INCREMENT};

    const LEAD: &str = "instrument 01 \"Lead\"  wave saw  adsr 0 0 100 0  duty 50";

    /// Channel 1 plays `rows` (pattern 00); channels 2-4 sit on an empty pattern.
    fn one_channel(inst: &str, rows: &[&str]) -> String {
        let mut s = String::from("acid-track 1\ntitle t\nspeed 2\nsfx-donor 4\n");
        s += inst;
        s += "\norder 1  00 loop 0\norder 2  01 loop 0\norder 3  01 loop 0\norder 4  01 loop 0\n";
        s += &format!("\npattern 00 {}\n", rows.len());
        for r in rows {
            s += r;
            s += "\n";
        }
        s += "\npattern 01 1\n... .. . .. ...\n";
        s
    }

    fn player(text: &str) -> (Player, Synth) {
        (Player::new(Arc::new(LoadedSong::plain(parse(text).unwrap())), 0, 0), Synth::new())
    }

    fn run(p: &mut Player, s: &mut Synth, n: usize) -> Vec<SongCmd> {
        let mut cmds = Vec::new();
        for _ in 0..n {
            p.tick(s, &mut cmds);
        }
        cmds
    }

    const EMPTY: &str = "... .. . .. ...";

    #[test]
    fn the_first_tick_starts_row_zero() {
        let (mut p, mut s) = player(&one_channel(LEAD, &["C-4 01 . .. ...", EMPTY]));
        run(&mut p, &mut s, 1);
        assert_eq!(s.voice(0).waveform, Waveform::Saw);
        assert_eq!(s.voice(0).envelope_stage, EnvStage::Attack);
        assert_eq!(s.voice(0).phase_increment, ONA_PHASE_INCREMENT[39]);
        assert_eq!(s.voice(1).envelope_stage, EnvStage::Off, "voice2 off and no second note");
        assert_eq!(p.position(), (0, 0, 1));
    }

    #[test]
    fn each_row_lasts_speed_ticks() {
        let (mut p, mut s) = player(&one_channel(LEAD, &["C-4 01 . .. ...", EMPTY, "=== .. . .. ...", EMPTY]));
        run(&mut p, &mut s, 2);
        assert_eq!(p.position(), (0, 1, 0));
        run(&mut p, &mut s, 2);
        assert_ne!(s.voice(0).envelope_stage, EnvStage::Release);
        run(&mut p, &mut s, 1);
        assert_eq!(s.voice(0).envelope_stage, EnvStage::Release, "row 2 is a note-off");
    }

    #[test]
    fn order_lists_transpose_and_loop() {
        let text = one_channel(LEAD, &["C-4 01 . .. ..."]).replace("order 1  00 loop 0", "order 1  00 00+12 loop 1");
        let (mut p, mut s) = player(&text);
        run(&mut p, &mut s, 1);
        assert_eq!(s.voice(0).phase_increment, ONA_PHASE_INCREMENT[39]);
        run(&mut p, &mut s, 2);
        assert_eq!(p.position(), (1, 0, 1));
        assert_eq!(s.voice(0).phase_increment, ONA_PHASE_INCREMENT[51]);
        run(&mut p, &mut s, 2);
        assert_eq!(p.position(), (1, 0, 1), "wraps to the loop point");
    }

    #[test]
    fn slide_moves_the_pitch_every_tick() {
        let (mut p, mut s) = player(&one_channel(LEAD, &["C-4 01 1 08 ..."]));
        run(&mut p, &mut s, 1);
        assert_eq!(s.voice(0).phase_increment, increment_at(fine_pos(40) + 8));
        run(&mut p, &mut s, 1);
        assert_eq!(s.voice(0).phase_increment, increment_at(fine_pos(40) + 16));
    }

    #[test]
    fn vibrato_command_wobbles_the_pitch() {
        let (mut p, mut s) = player(&one_channel(LEAD, &["C-4 01 4 F4 ..."]));
        run(&mut p, &mut s, 1);
        // Phase 4 of the triangle is +4; depth 15 * 4 / 4 = 15 fine steps.
        assert_eq!(s.voice(0).phase_increment, increment_at(fine_pos(40) + 15));
    }

    #[test]
    fn glide_heads_for_the_new_note_without_retriggering() {
        let (mut p, mut s) = player(&one_channel(LEAD, &["C-4 01 . .. ...", "E-4 .. 3 10 ..."]));
        run(&mut p, &mut s, 3);
        assert_eq!(s.voice(0).phase_increment, increment_at(fine_pos(40) + 16));
    }

    #[test]
    fn speed_command_changes_the_row_length() {
        let (mut p, mut s) = player(&one_channel(LEAD, &["C-4 01 F 03 ...", EMPTY]));
        run(&mut p, &mut s, 2);
        assert_eq!(p.position(), (0, 0, 2));
        run(&mut p, &mut s, 1);
        assert_eq!(p.position(), (0, 1, 0));
    }

    #[test]
    fn waveform_command() {
        let (mut p, mut s) = player(&one_channel(LEAD, &["C-4 01 8 03 ..."]));
        run(&mut p, &mut s, 1);
        assert_eq!(s.voice(0).waveform, Waveform::Noise);
    }

    #[test]
    fn voice2_detune_doubles_the_note() {
        let (mut p, mut s) = player(&one_channel(&format!("{LEAD}  voice2 detune 6"), &["C-4 01 . .. ..."]));
        run(&mut p, &mut s, 1);
        assert_eq!(s.voice(1).envelope_stage, EnvStage::Attack);
        assert_eq!(s.voice(1).phase_increment, increment_at(fine_pos(40) + 6));
    }

    #[test]
    fn the_second_note_column_plays_on_voice_two() {
        let (mut p, mut s) = player(&one_channel(LEAD, &["C-4 01 . .. E-4"]));
        run(&mut p, &mut s, 1);
        assert_eq!(s.voice(1).envelope_stage, EnvStage::Attack);
        assert_eq!(s.voice(1).phase_increment, ONA_PHASE_INCREMENT[43]);
    }

    #[test]
    fn built_in_arp_steps_each_tick() {
        let (mut p, mut s) = player(&one_channel(&format!("{LEAD}  arp 4 7"), &["C-4 01 . .. ...", EMPTY]));
        let mut seen = Vec::new();
        for _ in 0..4 {
            run(&mut p, &mut s, 1);
            seen.push(s.voice(0).phase_increment);
        }
        assert_eq!(seen, [39, 43, 46, 39].map(|i| ONA_PHASE_INCREMENT[i]));
    }

    #[test]
    fn a_filter_field_routes_the_voice() {
        let (mut p, mut s) = player(&one_channel(&format!("{LEAD}  filter lp 40 6"), &["C-4 01 . .. ..."]));
        run(&mut p, &mut s, 1);
        assert!(s.voice(0).filter_route);
    }

    const BLIP: &str = "instrument blip\nwave tri\ngate on\nloop\npitch +1\nwait 1\nend\non release\ngate off\nend";

    fn scripted(src: &str, rows: &[&str]) -> (Player, Synth) {
        let text = one_channel("instrument 01 \"S\"  script \"x.snd\" blip", rows);
        let mut ls = LoadedSong::plain(parse(&text).unwrap());
        ls.scripts.insert(1, (Arc::new(compile(src).unwrap()), 0));
        (Player::new(Arc::new(ls), 0, 0), Synth::new())
    }

    #[test]
    fn a_script_instrument_plays_and_releases() {
        let (mut p, mut s) = scripted(BLIP, &["C-4 01 . .. ...", EMPTY, "=== .. . .. ...", EMPTY]);
        run(&mut p, &mut s, 1);
        assert_eq!(s.voice(0).waveform, Waveform::Triangle);
        assert_eq!(s.voice(0).envelope_stage, EnvStage::Attack);
        assert_eq!(s.voice(0).phase_increment, ONA_PHASE_INCREMENT[40]);
        run(&mut p, &mut s, 1);
        assert_eq!(s.voice(0).phase_increment, ONA_PHASE_INCREMENT[41]);
        run(&mut p, &mut s, 3);
        assert_eq!(s.voice(0).envelope_stage, EnvStage::Release);
    }

    #[test]
    fn a_second_note_runs_a_second_instance_on_voice_two() {
        let (mut p, mut s) = scripted("instrument p\ngate on\nv2 wave noise\nwave saw\nend", &["C-4 01 . .. G-4"]);
        run(&mut p, &mut s, 1);
        assert_eq!(s.voice(0).waveform, Waveform::Saw);
        assert_eq!(s.voice(1).waveform, Waveform::Saw, "v1 of the second instance is voice 2");
        assert_eq!(s.voice(1).phase_increment, ONA_PHASE_INCREMENT[46]);
    }

    #[test]
    fn a_script_that_failed_to_load_is_silent() {
        let text = one_channel("instrument 01 \"S\"  script \"x.snd\" blip", &["C-4 01 . .. ..."]);
        let (mut p, mut s) = player(&text);
        run(&mut p, &mut s, 1);
        assert_eq!(s.voice(0).envelope_stage, EnvStage::Off);
    }

    #[test]
    fn script_song_commands_reach_the_caller() {
        let (mut p, mut s) = scripted("instrument t\ntempo 3\nend", &["C-4 01 . .. ..."]);
        assert_eq!(run(&mut p, &mut s, 1), [SongCmd::Tempo(3)]);
    }

    #[test]
    fn a_muted_channel_stays_silent() {
        let (mut p, mut s) = player(&one_channel(LEAD, &["C-4 01 . .. ..."]));
        p.set_muted(&mut s, 0, true);
        run(&mut p, &mut s, 3);
        assert_eq!(s.voice(0).envelope_stage, EnvStage::Off);
        assert!(p.muted(0));
        p.set_muted(&mut s, 0, false);
        run(&mut p, &mut s, 2);
        assert_eq!(s.voice(0).envelope_stage, EnvStage::Attack);
    }

    #[test]
    fn a_lent_voice_is_left_alone_until_taken_back() {
        let (mut p, mut s) = player(&one_channel(&format!("{LEAD}  voice2 detune 6"), &["C-4 01 . .. ..."]));
        p.lend(1);
        s.set_voice_waveform(1, 3);
        run(&mut p, &mut s, 1);
        assert_eq!(s.voice(1).waveform, Waveform::Noise);
        assert_eq!(p.borrowed(), 0b10);
        p.take_back(1);
        run(&mut p, &mut s, 2);
        assert_eq!(s.voice(1).waveform, Waveform::Saw, "the next note uses it again");
    }

    #[test]
    fn the_donor_voice_follows_sfx_donor() {
        let (p, _) = player(&one_channel(LEAD, &[EMPTY]));
        assert_eq!(p.donor_voice(), 7);
        let (p, _) = player(&one_channel(LEAD, &[EMPTY]).replace("sfx-donor 4", "sfx-donor 1"));
        assert_eq!(p.donor_voice(), 1);
    }

    #[test]
    fn replace_song_keeps_the_place() {
        let rows = ["C-4 01 . .. ...", EMPTY, EMPTY, EMPTY];
        let (mut p, mut s) = player(&one_channel(LEAD, &rows));
        run(&mut p, &mut s, 3);
        assert_eq!(p.position(), (0, 1, 1));
        let edited = one_channel(LEAD, &["D-4 01 . .. ...", EMPTY, EMPTY, EMPTY]);
        p.replace_song(Arc::new(LoadedSong::plain(parse(&edited).unwrap())));
        assert_eq!(p.position(), (0, 1, 1));
        let shorter = one_channel(LEAD, &[EMPTY]);
        p.replace_song(Arc::new(LoadedSong::plain(parse(&shorter).unwrap())));
        assert_eq!(p.position(), (0, 0, 1), "row 1 no longer exists");
    }

    #[test]
    fn extreme_notes_from_an_app_call_are_clamped() {
        let text = one_channel(&format!("{LEAD}  voice2 detune 6"), &[EMPTY]);
        let mut p = Player::preview(Arc::new(LoadedSong::plain(parse(&text).unwrap())));
        let mut s = Synth::new();
        p.trigger(&mut s, 0, i32::MAX, Some(i32::MIN), 1);
        assert_eq!(s.voice(0).phase_increment, ONA_PHASE_INCREMENT[87]);
        assert_eq!(s.voice(1).phase_increment, ONA_PHASE_INCREMENT[0]);
        // Without the clamp `p1 + 6` overflows here.
        p.trigger(&mut s, 0, i32::MAX, None, 1);
        run(&mut p, &mut s, 1);
        assert_eq!(s.voice(0).phase_increment, ONA_PHASE_INCREMENT[87]);
        assert_eq!(s.voice(1).phase_increment, increment_at(fine_pos(88) + 6));
        p.trigger(&mut s, 0, i32::MIN, None, 1);
        assert_eq!(s.voice(0).phase_increment, ONA_PHASE_INCREMENT[0]);
    }

    #[test]
    fn a_muted_channel_still_runs_speed_commands() {
        let (mut p, mut s) = player(&one_channel(LEAD, &["C-4 01 F 03 ...", EMPTY]));
        p.set_muted(&mut s, 0, true);
        run(&mut p, &mut s, 2);
        assert_eq!(p.position(), (0, 0, 2));
        run(&mut p, &mut s, 1);
        assert_eq!(p.position(), (0, 1, 0));
        assert_eq!(s.voice(0).envelope_stage, EnvStage::Off);
    }

    #[test]
    fn an_instrument_change_while_muted_is_used_after_unmute() {
        let two = format!("{LEAD}\ninstrument 02 \"T\"  wave tri  adsr 0 0 100 0  duty 50");
        let (mut p, mut s) = player(&one_channel(&two, &["... 02 . .. ...", "C-4 .. . .. ..."]));
        p.set_muted(&mut s, 0, true);
        run(&mut p, &mut s, 1);
        p.set_muted(&mut s, 0, false);
        run(&mut p, &mut s, 2);
        assert_eq!(s.voice(0).waveform, Waveform::Triangle);
    }

    #[test]
    fn a_note_on_a_missing_instrument_silences_the_old_one() {
        let (mut p, mut s) = player(&one_channel(LEAD, &[EMPTY]));
        p.trigger(&mut s, 0, 40, None, 1);
        assert_eq!(s.voice(0).envelope_stage, EnvStage::Attack);
        p.trigger(&mut s, 0, 41, None, 9);
        assert_eq!(s.voice(0).envelope_stage, EnvStage::Release);
    }

    #[test]
    fn a_bad_script_block_index_does_not_panic() {
        let text = one_channel("instrument 01 \"S\"  script \"x.snd\" blip", &["C-4 01 . .. ..."]);
        let mut ls = LoadedSong::plain(parse(&text).unwrap());
        ls.scripts.insert(1, (Arc::new(compile(BLIP).unwrap()), 99));
        let (mut p, mut s) = (Player::new(Arc::new(ls), 0, 0), Synth::new());
        run(&mut p, &mut s, 2);
        p.note_off(&mut s, 0);
    }

    #[test]
    fn stop_gates_off_and_preview_only_plays_what_it_is_given() {
        let (mut p, mut s) = player(&one_channel(LEAD, &["C-4 01 . .. ..."]));
        run(&mut p, &mut s, 1);
        p.stop(&mut s);
        assert_eq!(s.voice(0).envelope_stage, EnvStage::Release);
        assert!(!p.playing);

        let text = one_channel(LEAD, &["C-4 01 . .. ..."]);
        let mut pv = Player::preview(Arc::new(LoadedSong::plain(parse(&text).unwrap())));
        let mut s = Synth::new();
        run(&mut pv, &mut s, 3);
        assert_eq!(s.voice(0).envelope_stage, EnvStage::Off, "a preview reads no rows");
        pv.trigger(&mut s, 2, 52, None, 1);
        assert_eq!(s.voice(4).envelope_stage, EnvStage::Attack);
        pv.note_off(&mut s, 2);
        assert_eq!(s.voice(4).envelope_stage, EnvStage::Release);
    }
}
