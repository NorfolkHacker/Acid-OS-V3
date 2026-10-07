//! Plays a parsed song on the synth. The song steps through its order
//! list one row every `speed` ticks; every track reads its cell of the
//! same row. Track n plays voice n; a voice lent to a sound effect is left
//! alone until it comes back.

use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use alloc::vec::Vec;

use acid_synth::Synth;

use crate::pitch::{self, fine_pos, increment_at, FINE_MAX, FINE_STEPS};
use crate::program::Program;
use crate::song::{Kind, Song, NOTE_NONE, NOTE_OFF, TRACKS};
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
struct Track {
    /// The instrument column's last value.
    inst: u8,
    /// The instrument the sounding note started with.
    playing: u8,
    /// A built-in note is running its effects.
    sounding: bool,
    /// The note's fine position, after slides and glides.
    pos: i32,
    glide_to: Option<i32>,
    glide_speed: i32,
    fx: RowFx,
    vib_phase: i32,
    arp_step: usize,
    duty: i32,
    pwm_dir: i32,
    script: Option<Arc<Program>>,
    instance: Option<Instance>,
    muted: bool,
}

pub struct Player {
    song: Arc<LoadedSong>,
    /// False for a preview: no rows are read, only notes it is handed.
    pub playing: bool,
    speed: u8,
    tick: u8,
    order_pos: usize,
    row: usize,
    tracks: [Track; TRACKS],
    row_serial: u32,
    borrowed: u8,
    seed: u32,
}

fn pattern_len(song: &Song, order_pos: usize) -> usize {
    song.order.get(order_pos).and_then(|p| song.patterns.get(p)).map_or(1, |p| p.len())
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

impl Player {
    pub fn new(song: Arc<LoadedSong>, order: usize, row: usize) -> Self {
        let speed = song.song.speed.max(1);
        let mut p = Self {
            song,
            playing: true,
            speed,
            tick: 0,
            order_pos: 0,
            row: 0,
            tracks: Default::default(),
            row_serial: 0,
            borrowed: 0,
            seed: 1,
        };
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
        Clock { playing: self.playing, order: self.order_pos as i32, row: self.row as i32, row_serial: self.row_serial }
    }

    /// The order position and row, and the tick within the row.
    pub fn position(&self) -> (i32, i32, i32) {
        (self.order_pos as i32, self.row as i32, self.tick as i32)
    }

    pub fn borrowed(&self) -> u8 {
        self.borrowed
    }

    /// Voices the song never plays (a bit per voice): sound effects may take these.
    pub fn free_voices(&self) -> u8 {
        !self.song.song.used_tracks()
    }

    pub fn muted(&self, t: usize) -> bool {
        self.tracks.get(t).is_some_and(|c| c.muted)
    }

    fn seek(&mut self, order: usize, row: usize) {
        self.order_pos = order.min(self.song.song.order.len().saturating_sub(1));
        self.row = if row < pattern_len(&self.song.song, self.order_pos) { row } else { 0 };
        self.tick = 0;
    }

    /// To order position `order`, row 0, playing.
    pub fn jump(&mut self, order: i32) {
        self.seek(order.max(0) as usize, 0);
        self.playing = true;
    }

    pub fn set_speed(&mut self, s: i32) {
        if (1..=31).contains(&s) {
            self.speed = s as u8;
        }
    }

    /// Swaps in edited song data, keeping the place where it still exists.
    pub fn replace_song(&mut self, song: Arc<LoadedSong>) {
        self.speed = song.song.speed.max(1);
        self.order_pos = self.order_pos.min(song.song.order.len().saturating_sub(1));
        if self.row >= pattern_len(&song.song, self.order_pos) {
            self.row = 0;
        }
        self.song = song;
        if self.tick >= self.speed {
            self.tick = 0;
        }
    }

    fn voice(&self, t: usize) -> Option<i32> {
        (self.borrowed & (1 << t) == 0).then_some(t as i32)
    }

    /// Lends voice `v` to a sound effect: the player and its scripts stop touching it.
    pub fn lend(&mut self, v: u8) {
        if v as usize >= TRACKS {
            return;
        }
        self.borrowed |= 1 << v;
        for t in &mut self.tracks {
            if let Some(i) = t.instance.as_mut() {
                i.drop_voice(v);
            }
        }
    }

    /// Takes voice `v` back; it sounds again from its track's next note.
    pub fn take_back(&mut self, v: u8) {
        if (v as usize) < TRACKS {
            self.borrowed &= !(1 << v);
        }
    }

    pub fn tick(&mut self, synth: &mut Synth, cmds: &mut Vec<SongCmd>) {
        if self.playing && self.tick == 0 {
            for t in 0..TRACKS {
                self.read_cell(synth, t);
            }
            self.row_serial = self.row_serial.wrapping_add(1);
        }
        for t in 0..TRACKS {
            self.effects(synth, t);
        }
        let clock = self.clock();
        for tr in &mut self.tracks {
            let Some(prog) = tr.script.clone() else { continue };
            let Some(inst) = tr.instance.as_mut() else { continue };
            let Some(block) = prog.blocks.get(inst.block) else {
                tr.instance = None;
                continue;
            };
            inst.tick(block, &mut Env { synth: &mut *synth, clock, song_cmds: &mut *cmds });
            if inst.state == State::Done {
                tr.instance = None;
            }
        }
        if self.playing {
            self.tick += 1;
            if self.tick >= self.speed {
                self.tick = 0;
                self.advance();
            }
        }
    }

    fn read_cell(&mut self, synth: &mut Synth, t: usize) {
        let song = self.song.clone();
        let Some(pat) = song.song.order.get(self.order_pos) else { return };
        let Some(cell) = song.song.patterns.get(pat).and_then(|p| p.get(self.row)).map(|r| r[t]) else { return };
        self.tracks[t].fx = RowFx::default();
        if cell.inst != 0 {
            self.tracks[t].inst = cell.inst;
        }
        // A mute only stops the track changing its voice; song commands still run.
        if self.tracks[t].muted {
            self.command(synth, t, cell.cmd, cell.param);
            return;
        }
        match cell.note {
            NOTE_NONE => {}
            NOTE_OFF => self.note_off(synth, t),
            n if cell.cmd == b'3' => self.tracks[t].glide_to = Some(fine_pos(n as i32)),
            n => {
                let inst = self.tracks[t].inst;
                self.trigger(synth, t, n as i32, inst);
            }
        }
        self.command(synth, t, cell.cmd, cell.param);
    }

    fn command(&mut self, synth: &mut Synth, t: usize, cmd: u8, param: u8) {
        let p = param as i32;
        if self.tracks[t].muted && matches!(cmd, b'1' | b'2' | b'4' | b'8' | b'9') {
            return;
        }
        match cmd {
            b'1' => self.tracks[t].fx.slide = p,
            b'2' => self.tracks[t].fx.slide = -p,
            b'3' => self.tracks[t].glide_speed = p.max(1),
            b'4' => {
                let fx = &mut self.tracks[t].fx;
                fx.vib_depth = p >> 4;
                fx.vib_speed = p & 15;
            }
            b'8' => {
                if let Some(v) = self.voice(t) {
                    synth.set_voice_waveform(v, p & 3);
                }
            }
            b'9' => {
                let d = p.clamp(1, 99);
                self.tracks[t].duty = d;
                if let Some(v) = self.voice(t) {
                    synth.set_duty(v, d);
                }
            }
            b'A' => synth.set_filter_cutoff(p),
            b'F' => self.set_speed(p),
            _ => {}
        }
    }

    /// Starts `ona` on track `t` with instrument `inst`. A missing
    /// instrument plays silence.
    pub fn trigger(&mut self, synth: &mut Synth, t: usize, ona: i32, inst: u8) {
        if t >= TRACKS {
            return;
        }
        // `ona` can come from an app call: keep every position in range.
        let ona = ona.clamp(pitch::ONA_MIN, pitch::ONA_MAX);
        let song = self.song.clone();
        self.seed = self.seed.wrapping_add(1);
        let seed = self.seed;
        let v = self.voice(t);
        let tr = &mut self.tracks[t];
        tr.instance = None;
        tr.script = None;
        tr.sounding = false;
        if tr.muted {
            return;
        }
        // A new note stops whatever the track was playing, even if it then
        // plays nothing (a missing instrument is silence).
        if let Some(v) = v {
            synth.gate_off(v);
            synth.clear_ring_partner(v);
        }
        tr.playing = inst;
        tr.pos = fine_pos(ona);
        tr.glide_to = None;
        tr.vib_phase = 0;
        tr.arp_step = 0;
        tr.pwm_dir = 1;
        let Some(instrument) = song.song.instruments.get(&inst) else { return };
        match &instrument.kind {
            Kind::BuiltIn(b) => {
                tr.duty = b.duty;
                tr.sounding = true;
                let Some(v) = v else { return };
                synth.set_voice_waveform(v, b.wave);
                synth.set_adsr(v, b.adsr[0], b.adsr[1], b.adsr[2], b.adsr[3]);
                synth.set_duty(v, b.duty);
                synth.set_voice_filter_route(v, b.filter.is_some() as i32);
                let vo = synth.voice_mut(v as usize);
                vo.arp_active = false;
                vo.phase_increment = increment_at(tr.pos);
                if let Some((mode, cut, res)) = b.filter {
                    synth.set_filter_mode(mode);
                    synth.set_filter_cutoff(cut);
                    synth.set_filter_resonance(res);
                }
                synth.gate_on(v);
            }
            Kind::Script { .. } => {
                let Some((prog, block)) = song.scripts.get(&inst) else { return };
                let mut i = Instance::new(*block, [v.map(|v| v as u8), None], ona, 0, seed);
                i.start(synth);
                tr.instance = Some(i);
                tr.script = Some(prog.clone());
            }
        }
    }

    /// Note-off: a built-in releases its gate; a script runs `on release`.
    pub fn note_off(&mut self, synth: &mut Synth, t: usize) {
        if t >= TRACKS {
            return;
        }
        let v = self.voice(t);
        let tr = &mut self.tracks[t];
        if let Some(prog) = tr.script.clone() {
            if let Some(inst) = tr.instance.as_mut()
                && let Some(block) = prog.blocks.get(inst.block)
            {
                inst.release(block, synth);
            }
        } else if let Some(v) = v {
            synth.gate_off(v);
        }
    }

    /// Built-in instruments' per-tick work: slides, glide, vibrato, arp, PWM.
    fn effects(&mut self, synth: &mut Synth, t: usize) {
        let song = self.song.clone();
        let v = self.voice(t);
        let tr = &mut self.tracks[t];
        if tr.muted || !tr.sounding {
            return;
        }
        let Some(Kind::BuiltIn(b)) = song.song.instruments.get(&tr.playing).map(|i| &i.kind) else { return };
        tr.pos = (tr.pos + tr.fx.slide).clamp(0, FINE_MAX);
        if let Some(to) = tr.glide_to {
            let step = tr.glide_speed.max(1);
            tr.pos = if tr.pos < to { (tr.pos + step).min(to) } else { (tr.pos - step).max(to) };
            if tr.pos == to {
                tr.glide_to = None;
            }
        }
        let (depth, speed) = if tr.fx.vib_depth > 0 { (tr.fx.vib_depth, tr.fx.vib_speed) } else { b.vib };
        tr.vib_phase = (tr.vib_phase + speed) & 63;
        let vib = depth * tri(tr.vib_phase) / 4;
        let arp = if b.arp.is_empty() {
            0
        } else {
            let k = tr.arp_step % (b.arp.len() + 1);
            tr.arp_step = tr.arp_step.wrapping_add(1);
            if k == 0 { 0 } else { b.arp[k - 1] * FINE_STEPS }
        };
        if b.pwm != 0 {
            tr.duty += b.pwm * tr.pwm_dir;
            if tr.duty >= 90 {
                tr.duty = 90;
                tr.pwm_dir = -1;
            } else if tr.duty <= 10 {
                tr.duty = 10;
                tr.pwm_dir = 1;
            }
            if let Some(v) = v {
                synth.set_duty(v, tr.duty);
            }
        }
        if let Some(v) = v {
            synth.voice_mut(v as usize).phase_increment = increment_at(tr.pos + vib + arp);
        }
    }

    fn advance(&mut self) {
        let song = &self.song.song;
        self.row += 1;
        if self.row >= pattern_len(song, self.order_pos) {
            self.row = 0;
            self.order_pos += 1;
            if self.order_pos >= song.order.len() {
                self.order_pos = song.loop_to;
            }
        }
    }

    fn silence(&mut self, synth: &mut Synth, t: usize) {
        if let Some(v) = self.voice(t) {
            synth.gate_off(v);
        }
        let tr = &mut self.tracks[t];
        tr.instance = None;
        tr.script = None;
        tr.sounding = false;
    }

    /// Silences every voice the player holds and stops playing.
    pub fn stop(&mut self, synth: &mut Synth) {
        for t in 0..TRACKS {
            self.silence(synth, t);
        }
        self.playing = false;
    }

    /// A muted track keeps its place but leaves its voice silent.
    pub fn set_muted(&mut self, synth: &mut Synth, t: usize, on: bool) {
        if t >= TRACKS {
            return;
        }
        if on {
            self.silence(synth, t);
        }
        self.tracks[t].muted = on;
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
    const EMPTY: &str = "... .. . ..";

    /// A row: `cells` on the first tracks, the rest empty.
    fn row(cells: &[&str]) -> String {
        (0..TRACKS).map(|t| cells.get(t).copied().unwrap_or(EMPTY)).collect::<Vec<_>>().join(" | ")
    }

    /// Track 1 plays `rows` in pattern 00; every other track is empty.
    fn one_track(inst: &str, rows: &[&str]) -> String {
        song_text(inst, "order 00 loop 0", &[rows])
    }

    /// Patterns 00, 01, ... with track 1's cells given; `order` is the order line.
    fn song_text(inst: &str, order: &str, patterns: &[&[&str]]) -> String {
        let mut s = format!("acid-track 2\ntitle t\nspeed 2\n{inst}\n{order}\n");
        for (n, rows) in patterns.iter().enumerate() {
            s += &format!("\npattern {n:02X} {}\n", rows.len());
            for r in *rows {
                s += &row(&[r]);
                s += "\n";
            }
        }
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

    #[test]
    fn the_first_tick_starts_row_zero() {
        let (mut p, mut s) = player(&one_track(LEAD, &["C-4 01 . ..", EMPTY]));
        run(&mut p, &mut s, 1);
        assert_eq!(s.voice(0).waveform, Waveform::Saw);
        assert_eq!(s.voice(0).envelope_stage, EnvStage::Attack);
        assert_eq!(s.voice(0).phase_increment, ONA_PHASE_INCREMENT[39]);
        assert_eq!(s.voice(1).envelope_stage, EnvStage::Off, "track 2 is empty");
        assert_eq!(p.position(), (0, 0, 1));
    }

    #[test]
    fn every_track_plays_its_own_voice() {
        let text = format!(
            "acid-track 2\ntitle t\nspeed 2\n{LEAD}\norder 00 loop 0\n\npattern 00 1\n{}\n",
            row(&["C-4 01 . ..", EMPTY, EMPTY, EMPTY, EMPTY, EMPTY, EMPTY, "G-4 01 8 03"])
        );
        let (mut p, mut s) = player(&text);
        run(&mut p, &mut s, 1);
        assert_eq!(s.voice(0).phase_increment, ONA_PHASE_INCREMENT[39]);
        assert_eq!(s.voice(7).phase_increment, ONA_PHASE_INCREMENT[46]);
        assert_eq!((s.voice(0).waveform, s.voice(7).waveform), (Waveform::Saw, Waveform::Noise), "commands act on their own track");
        assert_eq!(s.voice(3).envelope_stage, EnvStage::Off);
    }

    #[test]
    fn each_row_lasts_speed_ticks() {
        let (mut p, mut s) = player(&one_track(LEAD, &["C-4 01 . ..", EMPTY, "=== .. . ..", EMPTY]));
        run(&mut p, &mut s, 2);
        assert_eq!(p.position(), (0, 1, 0));
        run(&mut p, &mut s, 2);
        assert_ne!(s.voice(0).envelope_stage, EnvStage::Release);
        run(&mut p, &mut s, 1);
        assert_eq!(s.voice(0).envelope_stage, EnvStage::Release, "row 2 is a note-off");
    }

    #[test]
    fn the_order_steps_through_patterns_and_loops() {
        let text = song_text(LEAD, "order 00 01 loop 1", &[&["C-4 01 . .."], &["C-5 01 . ..", EMPTY]]);
        let (mut p, mut s) = player(&text);
        run(&mut p, &mut s, 1);
        assert_eq!(s.voice(0).phase_increment, ONA_PHASE_INCREMENT[39]);
        run(&mut p, &mut s, 2);
        assert_eq!(p.position(), (1, 0, 1));
        assert_eq!(s.voice(0).phase_increment, ONA_PHASE_INCREMENT[51]);
        run(&mut p, &mut s, 2);
        assert_eq!(p.position(), (1, 1, 1), "pattern 01 is two rows long");
        run(&mut p, &mut s, 2);
        assert_eq!(p.position(), (1, 0, 1), "wraps to the loop point");
    }

    #[test]
    fn slide_moves_the_pitch_every_tick() {
        let (mut p, mut s) = player(&one_track(LEAD, &["C-4 01 1 08"]));
        run(&mut p, &mut s, 1);
        assert_eq!(s.voice(0).phase_increment, increment_at(fine_pos(40) + 8));
        run(&mut p, &mut s, 1);
        assert_eq!(s.voice(0).phase_increment, increment_at(fine_pos(40) + 16));
    }

    #[test]
    fn vibrato_command_wobbles_the_pitch() {
        let (mut p, mut s) = player(&one_track(LEAD, &["C-4 01 4 F4"]));
        run(&mut p, &mut s, 1);
        // Phase 4 of the triangle is +4; depth 15 * 4 / 4 = 15 fine steps.
        assert_eq!(s.voice(0).phase_increment, increment_at(fine_pos(40) + 15));
    }

    #[test]
    fn glide_heads_for_the_new_note_without_retriggering() {
        let (mut p, mut s) = player(&one_track(LEAD, &["C-4 01 . ..", "E-4 .. 3 10"]));
        run(&mut p, &mut s, 3);
        assert_eq!(s.voice(0).phase_increment, increment_at(fine_pos(40) + 16));
    }

    #[test]
    fn speed_command_changes_the_row_length() {
        let (mut p, mut s) = player(&one_track(LEAD, &["C-4 01 F 03", EMPTY]));
        run(&mut p, &mut s, 2);
        assert_eq!(p.position(), (0, 0, 2));
        run(&mut p, &mut s, 1);
        assert_eq!(p.position(), (0, 1, 0));
    }

    #[test]
    fn waveform_command() {
        let (mut p, mut s) = player(&one_track(LEAD, &["C-4 01 8 03"]));
        run(&mut p, &mut s, 1);
        assert_eq!(s.voice(0).waveform, Waveform::Noise);
    }

    #[test]
    fn built_in_arp_steps_each_tick() {
        let (mut p, mut s) = player(&one_track(&format!("{LEAD}  arp 4 7"), &["C-4 01 . ..", EMPTY]));
        let mut seen = Vec::new();
        for _ in 0..4 {
            run(&mut p, &mut s, 1);
            seen.push(s.voice(0).phase_increment);
        }
        assert_eq!(seen, [39, 43, 46, 39].map(|i| ONA_PHASE_INCREMENT[i]));
    }

    #[test]
    fn a_filter_field_routes_the_voice() {
        let (mut p, mut s) = player(&one_track(&format!("{LEAD}  filter lp 40 6"), &["C-4 01 . .."]));
        run(&mut p, &mut s, 1);
        assert!(s.voice(0).filter_route);
    }

    const BLIP: &str = "instrument blip\nwave tri\ngate on\nloop\npitch +1\nwait 1\nend\non release\ngate off\nend";

    fn scripted(src: &str, rows: &[&str]) -> (Player, Synth) {
        let text = one_track("instrument 01 \"S\"  script \"x.snd\" blip", rows);
        let mut ls = LoadedSong::plain(parse(&text).unwrap());
        ls.scripts.insert(1, (Arc::new(compile(src).unwrap()), 0));
        (Player::new(Arc::new(ls), 0, 0), Synth::new())
    }

    #[test]
    fn a_script_instrument_plays_and_releases() {
        let (mut p, mut s) = scripted(BLIP, &["C-4 01 . ..", EMPTY, "=== .. . ..", EMPTY]);
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
    fn a_script_has_only_its_tracks_voice() {
        let (mut p, mut s) = scripted("instrument p\ngate on\nv2 wave noise\nwave saw\nend", &["C-4 01 . .."]);
        s.set_voice_waveform(1, 2);
        run(&mut p, &mut s, 1);
        assert_eq!(s.voice(0).waveform, Waveform::Saw);
        assert_eq!(s.voice(1).waveform, Waveform::Triangle, "v2 commands do nothing in a song");
    }

    #[test]
    fn a_script_that_failed_to_load_is_silent() {
        let text = one_track("instrument 01 \"S\"  script \"x.snd\" blip", &["C-4 01 . .."]);
        let (mut p, mut s) = player(&text);
        run(&mut p, &mut s, 1);
        assert_eq!(s.voice(0).envelope_stage, EnvStage::Off);
    }

    #[test]
    fn script_song_commands_reach_the_caller() {
        let (mut p, mut s) = scripted("instrument t\ntempo 3\nend", &["C-4 01 . .."]);
        assert_eq!(run(&mut p, &mut s, 1), [SongCmd::Tempo(3)]);
    }

    #[test]
    fn a_muted_track_stays_silent() {
        let (mut p, mut s) = player(&one_track(LEAD, &["C-4 01 . .."]));
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
        let (mut p, mut s) = player(&one_track(LEAD, &["C-4 01 . .."]));
        p.lend(0);
        s.set_voice_waveform(0, 3);
        run(&mut p, &mut s, 1);
        assert_eq!(s.voice(0).waveform, Waveform::Noise);
        assert_eq!(p.borrowed(), 0b1);
        p.take_back(0);
        run(&mut p, &mut s, 2);
        assert_eq!(s.voice(0).waveform, Waveform::Saw, "the next note uses it again");
    }

    #[test]
    fn free_voices_are_the_tracks_with_no_notes() {
        let (p, _) = player(&one_track(LEAD, &["C-4 01 . .."]));
        assert_eq!(p.free_voices(), 0b1111_1110);
        let (p, _) = player(&one_track(LEAD, &[EMPTY]));
        assert_eq!(p.free_voices(), 0xFF);
    }

    #[test]
    fn replace_song_keeps_the_place() {
        let rows = ["C-4 01 . ..", EMPTY, EMPTY, EMPTY];
        let (mut p, mut s) = player(&one_track(LEAD, &rows));
        run(&mut p, &mut s, 3);
        assert_eq!(p.position(), (0, 1, 1));
        let edited = one_track(LEAD, &["D-4 01 . ..", EMPTY, EMPTY, EMPTY]);
        p.replace_song(Arc::new(LoadedSong::plain(parse(&edited).unwrap())));
        assert_eq!(p.position(), (0, 1, 1));
        let shorter = one_track(LEAD, &[EMPTY]);
        p.replace_song(Arc::new(LoadedSong::plain(parse(&shorter).unwrap())));
        assert_eq!(p.position(), (0, 0, 1), "row 1 no longer exists");
    }

    #[test]
    fn extreme_notes_from_an_app_call_are_clamped() {
        let text = one_track(&format!("{LEAD}  vib 15 15"), &[EMPTY]);
        let mut p = Player::preview(Arc::new(LoadedSong::plain(parse(&text).unwrap())));
        let mut s = Synth::new();
        p.trigger(&mut s, 0, i32::MAX, 1);
        assert_eq!(s.voice(0).phase_increment, ONA_PHASE_INCREMENT[87]);
        run(&mut p, &mut s, 1);
        p.trigger(&mut s, 0, i32::MIN, 1);
        assert_eq!(s.voice(0).phase_increment, ONA_PHASE_INCREMENT[0]);
        run(&mut p, &mut s, 1);
    }

    #[test]
    fn a_muted_track_still_runs_speed_commands() {
        let (mut p, mut s) = player(&one_track(LEAD, &["C-4 01 F 03", EMPTY]));
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
        let (mut p, mut s) = player(&one_track(&two, &["... 02 . ..", "C-4 .. . .."]));
        p.set_muted(&mut s, 0, true);
        run(&mut p, &mut s, 1);
        p.set_muted(&mut s, 0, false);
        run(&mut p, &mut s, 2);
        assert_eq!(s.voice(0).waveform, Waveform::Triangle);
    }

    #[test]
    fn a_note_on_a_missing_instrument_silences_the_old_one() {
        let (mut p, mut s) = player(&one_track(LEAD, &[EMPTY]));
        p.trigger(&mut s, 0, 40, 1);
        assert_eq!(s.voice(0).envelope_stage, EnvStage::Attack);
        p.trigger(&mut s, 0, 41, 9);
        assert_eq!(s.voice(0).envelope_stage, EnvStage::Release);
    }

    #[test]
    fn a_bad_script_block_index_does_not_panic() {
        let text = one_track("instrument 01 \"S\"  script \"x.snd\" blip", &["C-4 01 . .."]);
        let mut ls = LoadedSong::plain(parse(&text).unwrap());
        ls.scripts.insert(1, (Arc::new(compile(BLIP).unwrap()), 99));
        let (mut p, mut s) = (Player::new(Arc::new(ls), 0, 0), Synth::new());
        run(&mut p, &mut s, 2);
        p.note_off(&mut s, 0);
    }

    #[test]
    fn out_of_range_tracks_are_ignored() {
        let (mut p, mut s) = player(&one_track(LEAD, &[EMPTY]));
        p.trigger(&mut s, TRACKS, 40, 1);
        p.note_off(&mut s, usize::MAX);
        p.set_muted(&mut s, TRACKS, true);
        p.lend(200);
        p.take_back(200);
        assert!(!p.muted(TRACKS));
    }

    #[test]
    fn stop_gates_off_and_preview_only_plays_what_it_is_given() {
        let (mut p, mut s) = player(&one_track(LEAD, &["C-4 01 . .."]));
        run(&mut p, &mut s, 1);
        p.stop(&mut s);
        assert_eq!(s.voice(0).envelope_stage, EnvStage::Release);
        assert!(!p.playing);

        let text = one_track(LEAD, &["C-4 01 . .."]);
        let mut pv = Player::preview(Arc::new(LoadedSong::plain(parse(&text).unwrap())));
        let mut s = Synth::new();
        run(&mut pv, &mut s, 3);
        assert_eq!(s.voice(0).envelope_stage, EnvStage::Off, "a preview reads no rows");
        pv.trigger(&mut s, 6, 52, 1);
        assert_eq!(s.voice(6).envelope_stage, EnvStage::Attack);
        pv.note_off(&mut s, 6);
        assert_eq!(s.voice(6).envelope_stage, EnvStage::Release);
    }
}
