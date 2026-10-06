//! The .snd virtual machine. An `Instance` is one running sound or one
//! instrument note: it owns up to two synth voices and runs its block a
//! tick at a time, never more than BUDGET instructions per tick, and
//! nothing it does can panic or allocate.

use alloc::vec::Vec;

use acid_synth::Synth;

use crate::pitch;
use crate::program::*;

/// Instructions an instance may run per tick; running out acts as `wait 1`.
pub const BUDGET: u32 = 256;
/// Song commands one tick may queue; more are dropped, never allocated.
pub const SONG_CMDS_MAX: usize = 16;
const STACK: usize = 32;
const DEFAULT_ARP_MS: i32 = 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SongCmd {
    /// `song` indexes Program::song_paths; None restarts the current song.
    Play { song: Option<u8>, order: i32 },
    Stop,
    Tempo(i32),
    /// A 1-based channel.
    Mute(i32, bool),
    Jump(i32),
}

/// How the playing song looks this tick.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Clock {
    pub playing: bool,
    pub order: i32,
    pub row: i32,
    /// Counts rows started, so a wait can tell a new row from the same one.
    pub row_serial: u32,
}

pub struct Env<'a> {
    pub synth: &'a mut Synth,
    pub clock: Clock,
    pub song_cmds: &'a mut Vec<SongCmd>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Running,
    /// An instrument's main part ended; its note holds until release.
    Idle,
    Done,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Waiting {
    No,
    Ticks(i32),
    Row(u32),
    Beat(u32),
}

#[derive(Clone, Debug)]
pub struct Instance {
    pub block: usize,
    pub voices: [Option<u8>; 2],
    pub state: State,
    pc: usize,
    stack: [i32; STACK],
    sp: usize,
    vars: [i32; MAX_VARS],
    waiting: Waiting,
    pitch: [i32; 2],
    fine: [i32; 2],
    duty: [i32; 2],
    arp_ms: i32,
    note: i32,
    note2: i32,
    ticks: i32,
    rng: u32,
    song: Option<u8>,
}

fn bin(o: BinOp, l: i32, r: i32) -> i32 {
    match o {
        BinOp::Add => l.saturating_add(r),
        BinOp::Sub => l.saturating_sub(r),
        BinOp::Mul => l.saturating_mul(r),
        BinOp::Div => if r == 0 { 0 } else { l.saturating_div(r) },
        BinOp::Mod => if r == 0 { 0 } else { l.checked_rem(r).unwrap_or(0) },
        BinOp::Eq => (l == r) as i32,
        BinOp::Ne => (l != r) as i32,
        BinOp::Lt => (l < r) as i32,
        BinOp::Le => (l <= r) as i32,
        BinOp::Gt => (l > r) as i32,
        BinOp::Ge => (l >= r) as i32,
        BinOp::And => (l != 0 && r != 0) as i32,
        BinOp::Or => (l != 0 || r != 0) as i32,
    }
}

fn queue(env: &mut Env, c: SongCmd) {
    if env.song_cmds.len() < SONG_CMDS_MAX {
        env.song_cmds.push(c);
    }
}

impl Instance {
    /// Both voices start at `note`; `note2` is what the script's `note2` reads.
    pub fn new(block: usize, voices: [Option<u8>; 2], note: i32, note2: i32, seed: u32) -> Self {
        let note = note.clamp(pitch::ONA_MIN, pitch::ONA_MAX);
        Self {
            block,
            voices,
            state: State::Running,
            pc: 0,
            stack: [0; STACK],
            sp: 0,
            vars: [0; MAX_VARS],
            waiting: Waiting::No,
            pitch: [note; 2],
            fine: [0; 2],
            duty: [50; 2],
            arp_ms: DEFAULT_ARP_MS,
            note,
            note2,
            ticks: 0,
            rng: seed.wrapping_mul(0x9E37_79B9) | 1,
            song: None,
        }
    }

    /// Points the voices at the start note, so `gate on` sounds without a `pitch`.
    pub fn start(&mut self, synth: &mut Synth) {
        for s in 0..2 {
            self.apply_pitch(synth, s);
        }
    }

    pub fn var(&self, i: usize) -> i32 {
        self.vars[i]
    }

    /// The voice was lent to a sound effect: stop touching it.
    pub fn drop_voice(&mut self, v: u8) {
        for slot in &mut self.voices {
            if *slot == Some(v) {
                *slot = None;
            }
        }
    }

    /// Note-off: run `on release`, or gate off and finish.
    pub fn release(&mut self, b: &Block, synth: &mut Synth) {
        if self.state == State::Done {
            return;
        }
        match b.release_pc {
            Some(pc) => {
                self.pc = pc as usize;
                self.sp = 0;
                self.waiting = Waiting::No;
                self.state = State::Running;
            }
            None => self.stop(synth),
        }
    }

    pub fn stop(&mut self, synth: &mut Synth) {
        for v in self.voices.iter().flatten() {
            synth.gate_off(*v as i32);
        }
        self.state = State::Done;
    }

    pub fn tick(&mut self, b: &Block, env: &mut Env) {
        if self.state == State::Running {
            self.step(b, env);
        }
        self.ticks = self.ticks.saturating_add(1);
    }

    fn resume(&mut self, clock: Clock) -> bool {
        let go = match self.waiting {
            Waiting::No => true,
            Waiting::Ticks(n) if n > 1 => {
                self.waiting = Waiting::Ticks(n - 1);
                false
            }
            Waiting::Ticks(_) => true,
            Waiting::Row(s) => !clock.playing || clock.row_serial != s,
            Waiting::Beat(s) => !clock.playing || (clock.row_serial != s && clock.row % 4 == 0),
        };
        if go {
            self.waiting = Waiting::No;
        }
        go
    }

    fn step(&mut self, b: &Block, env: &mut Env) {
        if !self.resume(env.clock) {
            return;
        }
        for _ in 0..BUDGET {
            let Some(&op) = b.code.get(self.pc) else {
                self.finish(b, env.synth);
                return;
            };
            self.pc += 1;
            match op {
                Op::Push(n) => self.push(n),
                Op::Load(s) => {
                    let v = self.vars[s as usize];
                    self.push(v);
                }
                Op::Store(s) => {
                    let v = self.pop();
                    self.vars[s as usize] = v;
                }
                Op::Get(g) => {
                    let v = self.get(g, env.clock);
                    self.push(v);
                }
                Op::Bin(o) => {
                    let r = self.pop();
                    let l = self.pop();
                    self.push(bin(o, l, r));
                }
                Op::Neg => {
                    let v = self.pop();
                    self.push(v.saturating_neg());
                }
                Op::Not => {
                    let v = self.pop();
                    self.push((v == 0) as i32);
                }
                Op::Rand => {
                    let n = self.pop();
                    let r = self.next_rand();
                    self.push(if n <= 0 { 0 } else { (r % n as u32) as i32 });
                }
                Op::Jump(t) => self.pc = t as usize,
                Op::JumpIfZero(t) => {
                    if self.pop() == 0 {
                        self.pc = t as usize;
                    }
                }
                Op::Wait => {
                    let n = self.pop();
                    if n >= 1 {
                        self.waiting = Waiting::Ticks(n);
                        return;
                    }
                }
                Op::WaitRow => {
                    self.waiting = Waiting::Row(env.clock.row_serial);
                    return;
                }
                Op::WaitBeat => {
                    self.waiting = Waiting::Beat(env.clock.row_serial);
                    return;
                }
                Op::Stop => {
                    self.stop(env.synth);
                    return;
                }
                Op::End => {
                    self.finish(b, env.synth);
                    return;
                }
                Op::Cmd(c, t) => self.command(c, t, env),
            }
        }
    }

    /// A sound is over (gate off). An instrument's note holds (Idle) until
    /// note-off. A finished `on release` is Done and leaves the gate alone.
    fn finish(&mut self, b: &Block, synth: &mut Synth) {
        let in_release = b.release_pc.is_some_and(|r| self.pc > r as usize);
        match b.kind {
            BlockKind::Sound => self.stop(synth),
            BlockKind::Instrument if in_release => self.state = State::Done,
            BlockKind::Instrument => self.state = State::Idle,
        }
    }

    fn push(&mut self, v: i32) {
        if self.sp < STACK {
            self.stack[self.sp] = v;
            self.sp += 1;
        }
    }

    fn pop(&mut self) -> i32 {
        if self.sp == 0 {
            return 0;
        }
        self.sp -= 1;
        self.stack[self.sp]
    }

    fn next_rand(&mut self) -> u32 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        x
    }

    fn get(&self, g: Builtin, clock: Clock) -> i32 {
        let song = |v: i32| if clock.playing { v } else { -1 };
        match g {
            Builtin::Note => self.note,
            Builtin::Note2 => self.note2,
            Builtin::Tick => self.ticks,
            Builtin::Row => song(clock.row),
            Builtin::Beat => song(clock.row / 4),
            Builtin::Order => song(clock.order),
        }
    }

    fn voice(&self, slot: usize) -> Option<i32> {
        self.voices[slot].map(i32::from)
    }

    /// The synth's arp drives the pitch while it runs, so leave it be.
    fn apply_pitch(&self, synth: &mut Synth, s: usize) {
        if let Some(v) = self.voices[s] {
            let vo = synth.voice_mut(v as usize);
            if !vo.arp_active {
                vo.phase_increment = pitch::phase_increment(self.pitch[s], self.fine[s]);
            }
        }
    }

    fn command(&mut self, c: Cmd, t: Target, env: &mut Env) {
        let slots: &[usize] = match t {
            Target::V1 => &[0],
            Target::V2 => &[1],
            Target::Both => &[0, 1],
        };
        match c {
            Cmd::Wave(w) => {
                for &s in slots {
                    if let Some(v) = self.voice(s) {
                        env.synth.set_voice_waveform(v, w);
                    }
                }
            }
            Cmd::Duty { rel } => {
                let n = self.pop();
                for &s in slots {
                    let d = if rel { self.duty[s].saturating_add(n) } else { n }.clamp(1, 99);
                    self.duty[s] = d;
                    if let Some(v) = self.voice(s) {
                        env.synth.set_duty(v, d);
                    }
                }
            }
            Cmd::Adsr => {
                let r = self.pop();
                let su = self.pop();
                let d = self.pop();
                let a = self.pop();
                for &s in slots {
                    if let Some(v) = self.voice(s) {
                        env.synth.set_adsr(v, a, d, su, r);
                    }
                }
            }
            Cmd::Gate(on) => {
                for &s in slots {
                    if let Some(v) = self.voice(s) {
                        if on { env.synth.gate_on(v) } else { env.synth.gate_off(v) }
                    }
                }
            }
            Cmd::Pitch { rel } => {
                let n = self.pop();
                for &s in slots {
                    let p = if rel { self.pitch[s].saturating_add(n) } else { n };
                    self.pitch[s] = p.clamp(pitch::ONA_MIN, pitch::ONA_MAX);
                    self.apply_pitch(env.synth, s);
                }
            }
            Cmd::Fine { rel } => {
                let n = self.pop();
                for &s in slots {
                    let f = if rel { self.fine[s].saturating_add(n) } else { n };
                    self.fine[s] = f.clamp(-pitch::FINE_MAX, pitch::FINE_MAX);
                    self.apply_pitch(env.synth, s);
                }
            }
            Cmd::Ring(on) => {
                for &s in slots {
                    if let Some(v) = self.voice(s) {
                        match (on, self.voice(1 - s)) {
                            (true, Some(p)) => env.synth.set_ring_partner(v, p),
                            (true, None) => {}
                            (false, _) => env.synth.clear_ring_partner(v),
                        }
                    }
                }
            }
            Cmd::Route(on) => {
                for &s in slots {
                    if let Some(v) = self.voice(s) {
                        env.synth.set_voice_filter_route(v, on as i32);
                    }
                }
            }
            Cmd::Arp(n) => {
                let mut offs = [0i32; 3];
                for k in (0..n as usize).rev() {
                    offs[k] = self.pop();
                }
                for &s in slots {
                    if let Some(v) = self.voice(s) {
                        let base = self.pitch[s];
                        env.synth.set_arp_note(v, 0, base);
                        for (k, off) in offs.iter().enumerate().take(n as usize) {
                            let note = base.saturating_add(*off).clamp(pitch::ONA_MIN, pitch::ONA_MAX);
                            env.synth.set_arp_note(v, k as i32 + 1, note);
                        }
                        env.synth.set_arp_rate(v, self.arp_ms);
                        let vo = env.synth.voice_mut(v as usize);
                        vo.arp_step = 0;
                        vo.arp_step_counter = 0;
                        env.synth.arp_on(v, n as i32 + 1);
                    }
                }
            }
            Cmd::ArpOff => {
                for &s in slots {
                    if let Some(v) = self.voice(s) {
                        env.synth.voice_mut(v as usize).arp_active = false;
                    }
                    self.apply_pitch(env.synth, s);
                }
            }
            Cmd::ArpRate => {
                self.arp_ms = self.pop().clamp(1, 10_000);
                for &s in slots {
                    if let Some(v) = self.voice(s) {
                        env.synth.set_arp_rate(v, self.arp_ms);
                    }
                }
            }
            Cmd::Filter(mode) => {
                let res = self.pop();
                let cut = self.pop();
                env.synth.set_filter_mode(mode);
                env.synth.set_filter_cutoff(cut);
                env.synth.set_filter_resonance(res);
            }
            Cmd::SongSelect(i) => self.song = Some(i),
            Cmd::SongPlay => {
                let order = self.pop();
                let song = self.song;
                queue(env, SongCmd::Play { song, order });
            }
            Cmd::SongStop => queue(env, SongCmd::Stop),
            Cmd::Tempo => {
                let n = self.pop();
                queue(env, SongCmd::Tempo(n));
            }
            Cmd::Mute(on) => {
                let ch = self.pop();
                queue(env, SongCmd::Mute(ch, on));
            }
            Cmd::Jump => {
                let o = self.pop();
                queue(env, SongCmd::Jump(o));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compile::compile;
    use acid_synth::{EnvStage, Waveform, ONA_PHASE_INCREMENT};

    struct Rig {
        prog: Program,
        inst: Instance,
        synth: Synth,
        cmds: Vec<SongCmd>,
        clock: Clock,
    }

    fn rig(src: &str, voices: [Option<u8>; 2]) -> Rig {
        let prog = compile(src).unwrap();
        let mut synth = Synth::new();
        let mut inst = Instance::new(0, voices, 40, 0, 7);
        inst.start(&mut synth);
        Rig { prog, inst, synth, cmds: Vec::with_capacity(SONG_CMDS_MAX), clock: Clock::default() }
    }

    impl Rig {
        fn tick(&mut self, n: usize) {
            for _ in 0..n {
                let mut env = Env { synth: &mut self.synth, clock: self.clock, song_cmds: &mut self.cmds };
                self.inst.tick(&self.prog.blocks[self.inst.block], &mut env);
            }
        }
    }

    const ONE: [Option<u8>; 2] = [Some(0), None];
    const TWO: [Option<u8>; 2] = [Some(0), Some(1)];

    #[test]
    fn waits_sleep_whole_ticks() {
        let mut r = rig("let t = 0\nloop\nt = t + 1\nwait 2\nend", ONE);
        r.tick(1);
        assert_eq!(r.inst.var(0), 1);
        r.tick(1);
        assert_eq!(r.inst.var(0), 1);
        r.tick(1);
        assert_eq!(r.inst.var(0), 2);
    }

    #[test]
    fn a_busy_loop_yields_at_the_budget() {
        // 2 ops of setup, then 5 per pass: 256 ops leave x at 51.
        let mut r = rig("let x = 0\nloop\nx = x + 1\nend", ONE);
        r.tick(1);
        assert_eq!(r.inst.var(0), 51);
        assert_eq!(r.inst.state, State::Running);
        r.tick(1);
        assert_eq!(r.inst.var(0), 102);
    }

    #[test]
    fn arithmetic_saturates_and_never_fails() {
        let mut r = rig("let a = 2147483647 + 1\nlet b = 5 / 0\nlet c = 5 % 0\nlet d = 0 - 2147483647 - 10\nlet e = 7 / 2", ONE);
        r.tick(1);
        assert_eq!([r.inst.var(0), r.inst.var(1), r.inst.var(2), r.inst.var(3), r.inst.var(4)], [i32::MAX, 0, 0, i32::MIN, 3]);
    }

    #[test]
    fn comparisons_and_logic_give_one_or_zero() {
        let mut r = rig("let a = 3 > 2 and not 0\nlet b = 1 == 2 or 0", ONE);
        r.tick(1);
        assert_eq!((r.inst.var(0), r.inst.var(1)), (1, 0));
    }

    #[test]
    fn rand_is_in_range_and_repeatable() {
        let prog = compile("let r = rand 6").unwrap();
        let mut seen = alloc::collections::BTreeSet::new();
        for seed in 0..50 {
            let roll = |seed| {
                let mut s = Synth::new();
                let mut i = Instance::new(0, ONE, 40, 0, seed);
                let mut cmds = Vec::new();
                i.tick(&prog.blocks[0], &mut Env { synth: &mut s, clock: Clock::default(), song_cmds: &mut cmds });
                i.var(0)
            };
            let v = roll(seed);
            assert!((0..6).contains(&v));
            assert_eq!(v, roll(seed), "same seed, same roll");
            seen.insert(v);
        }
        assert!(seen.len() >= 3, "rand should vary with the seed: {seen:?}");
    }

    #[test]
    fn values_a_script_can_read() {
        let mut r = rig("let a = note\nlet b = note2\nlet c = row\nlet d = tick\nwait 1\nd = tick", ONE);
        r.tick(1);
        assert_eq!((r.inst.var(0), r.inst.var(1), r.inst.var(2), r.inst.var(3)), (40, 0, -1, 0));
        r.tick(1);
        assert_eq!(r.inst.var(3), 1);
    }

    #[test]
    fn gate_on_sounds_the_voice() {
        let mut r = rig("wave saw\npitch 44\ngate on", [Some(3), None]);
        r.tick(1);
        let v = r.synth.voice(3);
        assert_eq!(v.waveform, Waveform::Saw);
        assert_eq!(v.envelope_stage, EnvStage::Release, "a sound running off its end gates off");
        assert_eq!(v.phase_increment, ONA_PHASE_INCREMENT[43]);
    }

    #[test]
    fn gate_on_without_pitch_uses_the_start_note() {
        let mut r = rig("gate on\nwait 5", ONE);
        r.tick(1);
        assert_eq!(r.synth.voice(0).envelope_stage, EnvStage::Attack);
        assert_eq!(r.synth.voice(0).phase_increment, ONA_PHASE_INCREMENT[39]);
    }

    #[test]
    fn relative_pitch_and_fine() {
        let mut r = rig("pitch +2\nfine 32\nwait 5", ONE);
        r.tick(1);
        assert_eq!(r.synth.voice(0).phase_increment, crate::pitch::phase_increment(42, 32));
    }

    #[test]
    fn a_missing_voice_is_skipped_but_its_arguments_are_used_up() {
        let mut r = rig("v2 adsr 1 2 3 4\nlet x = 9\nv2 wave noise\nwave saw\nwait 5", ONE);
        r.tick(1);
        assert_eq!(r.inst.var(0), 9);
        assert_eq!(r.synth.voice(0).waveform, Waveform::Saw);
        assert_eq!(r.synth.voice(1).waveform, Waveform::Pulse, "voice 1 isn't this instance's");
    }

    #[test]
    fn both_targets_two_voices() {
        let mut r = rig("both wave tri\nwait 5", TWO);
        r.tick(1);
        assert_eq!(r.synth.voice(0).waveform, Waveform::Triangle);
        assert_eq!(r.synth.voice(1).waveform, Waveform::Triangle);
    }

    #[test]
    fn stop_gates_off_and_finishes() {
        let mut r = rig("gate on\nstop\nwait 1", ONE);
        r.tick(1);
        assert_eq!(r.synth.voice(0).envelope_stage, EnvStage::Release);
        assert_eq!(r.inst.state, State::Done);
    }

    #[test]
    fn an_instrument_holds_its_note_until_release() {
        let mut r = rig("instrument a\ngate on\nend", ONE);
        r.tick(1);
        assert_eq!(r.inst.state, State::Idle);
        assert_eq!(r.synth.voice(0).envelope_stage, EnvStage::Attack);
        r.inst.release(&r.prog.blocks[0], &mut r.synth);
        assert_eq!(r.synth.voice(0).envelope_stage, EnvStage::Release);
        assert_eq!(r.inst.state, State::Done);
    }

    #[test]
    fn on_release_runs_after_note_off() {
        let mut r = rig("instrument a\ngate on\non release\nwave noise\nend", ONE);
        r.tick(1);
        r.inst.release(&r.prog.blocks[0], &mut r.synth);
        assert_eq!(r.inst.state, State::Running);
        r.tick(1);
        assert_eq!(r.synth.voice(0).waveform, Waveform::Noise);
        assert_eq!(r.inst.state, State::Done);
        assert_ne!(r.synth.voice(0).envelope_stage, EnvStage::Release, "release code decides the gate");
    }

    #[test]
    fn wait_row_follows_the_song() {
        let mut r = rig("let n = 0\nloop\nwait row\nn = n + 1\nend", ONE);
        r.clock = Clock { playing: true, order: 0, row: 0, row_serial: 0 };
        r.tick(2);
        assert_eq!(r.inst.var(0), 0);
        r.clock.row_serial = 1;
        r.tick(1);
        assert_eq!(r.inst.var(0), 1);
    }

    #[test]
    fn wait_beat_waits_for_a_row_on_the_beat() {
        let mut r = rig("let n = 0\nloop\nwait beat\nn = n + 1\nend", ONE);
        r.clock = Clock { playing: true, order: 0, row: 3, row_serial: 0 };
        r.tick(1);
        r.clock = Clock { playing: true, order: 0, row: 3, row_serial: 1 };
        r.tick(1);
        assert_eq!(r.inst.var(0), 0, "row 3 isn't a beat");
        r.clock = Clock { playing: true, order: 0, row: 4, row_serial: 2 };
        r.tick(1);
        assert_eq!(r.inst.var(0), 1);
    }

    #[test]
    fn without_a_song_wait_row_is_one_tick() {
        let mut r = rig("let n = 0\nloop\nwait row\nn = n + 1\nend", ONE);
        r.tick(3);
        assert_eq!(r.inst.var(0), 2);
    }

    #[test]
    fn song_commands_queue_for_the_engine() {
        let mut r = rig("song \"a.trk\"\nplay 3\ntempo 4\nmute 2\nunmute 1\njump 5\nstop song", ONE);
        r.tick(1);
        assert_eq!(
            r.cmds,
            vec![
                SongCmd::Play { song: Some(0), order: 3 }, SongCmd::Tempo(4), SongCmd::Mute(2, true),
                SongCmd::Mute(1, false), SongCmd::Jump(5), SongCmd::Stop,
            ]
        );
    }

    #[test]
    fn the_song_command_queue_never_grows() {
        let mut r = rig("loop\ntempo 1\nend", ONE);
        r.tick(1);
        assert_eq!(r.cmds.len(), SONG_CMDS_MAX);
        assert_eq!(r.cmds.capacity(), SONG_CMDS_MAX);
    }

    #[test]
    fn arp_offsets_from_the_current_pitch() {
        let mut r = rig("pitch 40\narp 4 7\nwait 5", ONE);
        r.tick(1);
        let v = r.synth.voice(0);
        assert!(v.arp_active);
        assert_eq!(v.arp_count, 3);
        assert_eq!(&v.arp_notes[..3], &[40, 44, 47]);
    }

    #[test]
    fn a_dropped_voice_is_left_alone() {
        let mut r = rig("wave saw\nwait 5", ONE);
        r.inst.drop_voice(0);
        r.tick(1);
        assert_eq!(r.synth.voice(0).waveform, Waveform::Pulse);
    }
}
