//! The .trk song format: a versioned text file in the house style of
//! .spr. `parse` is strict and is the one reader of record (games and the
//! tracker both use it); `write` emits the canonical form.

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt;

use crate::pitch::{note_name, parse_note};

pub const CHANNELS: usize = 4;
pub const MAX_ROWS: usize = 64;
pub const MAX_PATTERN: u8 = 0x7F;
pub const MAX_INSTRUMENT: u8 = 0x3F;
pub const NOTE_NONE: u8 = 0;
pub const NOTE_OFF: u8 = 0xFF;
/// Pattern command letters. `S` is reserved for script calls.
pub const COMMANDS: &[u8] = b"123489AF";
/// Waveform names, in synth order.
pub const WAVES: [&str; 4] = ["pulse", "saw", "tri", "noise"];

/// One row of one channel. `note`/`note2` are ona (or NOTE_NONE/NOTE_OFF);
/// `cmd` is 0 or the command's ASCII letter.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Row {
    pub note: u8,
    pub inst: u8,
    pub cmd: u8,
    pub param: u8,
    pub note2: u8,
}

/// What a built-in instrument does with the channel's second voice when
/// the row has no second note.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Voice2 {
    Off,
    /// Fine steps (1/64 semitone).
    Detune(i32),
    Octave,
    Fifth,
    Ring,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuiltIn {
    pub wave: i32,
    pub adsr: [i32; 4],
    pub duty: i32,
    /// Duty change per tick, bouncing between 10 and 90.
    pub pwm: i32,
    /// Depth and speed, as pattern command 4.
    pub vib: (i32, i32),
    /// Semitone offsets stepped one per tick after the base note.
    pub arp: Vec<i32>,
    /// Mode mask, cutoff, resonance.
    pub filter: Option<(i32, i32, i32)>,
    pub voice2: Voice2,
}

impl Default for BuiltIn {
    fn default() -> Self {
        Self { wave: 0, adsr: [2, 40, 80, 40], duty: 50, pwm: 0, vib: (0, 0), arp: Vec::new(), filter: None, voice2: Voice2::Off }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    BuiltIn(BuiltIn),
    /// An instrument block in a .snd file; the path is fsroot-relative.
    Script { path: String, block: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Instrument {
    pub name: String,
    pub kind: Kind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OrderEntry {
    pub pattern: u8,
    pub transpose: i8,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OrderList {
    pub entries: Vec<OrderEntry>,
    pub loop_to: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Song {
    pub title: String,
    pub speed: u8,
    pub sfx_donor: u8,
    pub instruments: BTreeMap<u8, Instrument>,
    pub orders: [OrderList; CHANNELS],
    pub patterns: BTreeMap<u8, Vec<Row>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SongError {
    pub line: u32,
    pub message: String,
}

impl SongError {
    fn new(line: usize, message: impl Into<String>) -> Self {
        Self { line: line as u32, message: message.into() }
    }
}

impl fmt::Display for SongError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.line, self.message)
    }
}

fn hex2(s: &str) -> Option<u8> {
    if s.len() != 2 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    u8::from_str_radix(s, 16).ok()
}

struct Field {
    text: String,
    quoted: bool,
}

/// Splits on whitespace; "..." is one field (quotes removed).
fn fields(line: &str) -> Result<Vec<Field>, String> {
    let mut out = Vec::new();
    let mut rest = line.trim_start();
    while !rest.is_empty() {
        if let Some(r) = rest.strip_prefix('"') {
            let end = r.find('"').ok_or("a quote has no closing '\"'")?;
            out.push(Field { text: r[..end].to_string(), quoted: true });
            rest = r[end + 1..].trim_start();
        } else {
            let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
            out.push(Field { text: rest[..end].to_string(), quoted: false });
            rest = rest[end..].trim_start();
        }
    }
    Ok(out)
}

/// Walks an instrument line's fields.
struct Cur<'a> {
    f: &'a [Field],
    i: usize,
}

impl<'a> Cur<'a> {
    fn word(&mut self) -> Option<&'a str> {
        let x = self.f.get(self.i).filter(|x| !x.quoted)?;
        self.i += 1;
        Some(&x.text)
    }

    fn int(&mut self, after: &str) -> Result<i32, String> {
        let n = self
            .f
            .get(self.i)
            .filter(|x| !x.quoted)
            .and_then(|x| x.text.parse().ok())
            .ok_or_else(|| format!("expected a number after '{after}'"))?;
        self.i += 1;
        Ok(n)
    }

    fn next_is_int(&self) -> bool {
        self.f.get(self.i).is_some_and(|x| !x.quoted && x.text.parse::<i32>().is_ok())
    }

    fn quoted(&mut self, what: &str) -> Result<String, String> {
        match self.f.get(self.i) {
            Some(x) if x.quoted => {
                self.i += 1;
                Ok(x.text.clone())
            }
            _ => Err(format!("expected {what} in quotes")),
        }
    }

    fn done(&self) -> bool {
        self.i >= self.f.len()
    }
}

fn parse_instrument(f: &[Field]) -> Result<(u8, Instrument), String> {
    let mut c = Cur { f, i: 1 };
    let num = c.word().and_then(hex2).filter(|n| (1..=MAX_INSTRUMENT).contains(n)).ok_or("bad instrument number")?;
    let name = c.quoted("the instrument's name")?;
    if c.f.get(c.i).is_some_and(|x| !x.quoted && x.text == "script") {
        c.i += 1;
        let path = c.quoted("the script's path")?;
        let block = c.word().ok_or("expected the script's instrument name")?.to_string();
        if !c.done() {
            return Err("a script instrument has no other fields".into());
        }
        return Ok((num, Instrument { name, kind: Kind::Script { path, block } }));
    }
    let mut b = BuiltIn::default();
    while !c.done() {
        let key = c.word().ok_or("expected an instrument field")?;
        match key {
            "wave" => {
                let w = c.word().unwrap_or("");
                b.wave = WAVES.iter().position(|x| *x == w).ok_or_else(|| format!("unknown waveform '{w}'"))? as i32;
            }
            "adsr" => {
                for k in 0..4 {
                    b.adsr[k] = c.int("adsr")?.clamp(0, 100_000);
                }
            }
            "duty" => b.duty = c.int("duty")?.clamp(1, 99),
            "pwm" => b.pwm = c.int("pwm")?.clamp(-50, 50),
            "vib" => b.vib = (c.int("vib")?.clamp(0, 15), c.int("vib")?.clamp(0, 15)),
            "arp" => {
                b.arp.clear();
                while b.arp.len() < 3 && c.next_is_int() {
                    b.arp.push(c.int("arp")?.clamp(-48, 48));
                }
                if b.arp.is_empty() {
                    return Err("expected a number after 'arp'".into());
                }
            }
            "filter" => {
                let m = c.word().unwrap_or("");
                let mode = match m {
                    "lp" => 1,
                    "bp" => 2,
                    "hp" => 4,
                    _ => return Err(format!("unknown filter mode '{m}'")),
                };
                let (cut, res) = (c.int("filter")?, c.int("filter")?);
                if !(0..=255).contains(&cut) || !(0..=15).contains(&res) {
                    return Err("filter cutoff is 0 to 255 and resonance 0 to 15".into());
                }
                b.filter = Some((mode, cut, res));
            }
            "voice2" => {
                b.voice2 = match c.word().unwrap_or("") {
                    "off" => Voice2::Off,
                    "detune" => Voice2::Detune(c.int("detune")?.clamp(-768, 768)),
                    "octave" => Voice2::Octave,
                    "fifth" => Voice2::Fifth,
                    "ring" => Voice2::Ring,
                    m => return Err(format!("unknown voice2 mode '{m}'")),
                };
            }
            "script" => return Err("'script' must come right after the name".into()),
            k => return Err(format!("unknown instrument field '{k}'")),
        }
    }
    Ok((num, Instrument { name, kind: Kind::BuiltIn(b) }))
}

fn parse_entry(s: &str) -> Option<OrderEntry> {
    let (p, transpose) = match s.find(['+', '-']) {
        Some(k) => (&s[..k], s[k..].parse::<i8>().ok()?),
        None => (s, 0),
    };
    Some(OrderEntry { pattern: hex2(p).filter(|n| *n <= MAX_PATTERN)?, transpose })
}

fn parse_order(f: &[Field]) -> Result<(usize, OrderList), String> {
    let ch: usize = f
        .get(1)
        .and_then(|x| x.text.parse().ok())
        .filter(|c| (1..=CHANNELS).contains(c))
        .ok_or("order channel must be 1 to 4")?;
    let mut entries = Vec::new();
    let mut i = 2;
    while i < f.len() && f[i].text != "loop" {
        entries.push(parse_entry(&f[i].text).ok_or_else(|| format!("bad order entry '{}'", f[i].text))?);
        i += 1;
    }
    if entries.is_empty() {
        return Err(format!("order {ch} is empty"));
    }
    if i + 2 != f.len() {
        return Err(format!("order {ch} must end with 'loop N'"));
    }
    let loop_to: usize = f[i + 1].text.parse().map_err(|_| format!("order {ch} must end with 'loop N'"))?;
    if loop_to >= entries.len() {
        return Err(format!("loop {loop_to} is past the end of order {ch}"));
    }
    Ok((ch - 1, OrderList { entries, loop_to }))
}

fn parse_row(line: &str) -> Result<Row, String> {
    let f: Vec<&str> = line.split_whitespace().collect();
    if f.len() != 5 {
        return Err("expected a row like 'C-4 01 . .. ...'".into());
    }
    let note = match f[0] {
        "..." => NOTE_NONE,
        "===" => NOTE_OFF,
        n => parse_note(n).ok_or_else(|| format!("bad note '{n}'"))? as u8,
    };
    let inst = match f[1] {
        ".." => 0,
        s => hex2(s).filter(|n| (1..=MAX_INSTRUMENT).contains(n)).ok_or_else(|| format!("bad instrument '{s}'"))?,
    };
    let cmd = match f[2] {
        "." => 0,
        "S" => return Err("command S is reserved".into()),
        s if s.len() == 1 && COMMANDS.contains(&s.as_bytes()[0]) => s.as_bytes()[0],
        s => return Err(format!("unknown command '{s}'")),
    };
    let param = match f[3] {
        ".." => 0,
        s => hex2(s).ok_or_else(|| format!("bad parameter '{s}'"))?,
    };
    let note2 = match f[4] {
        "..." => NOTE_NONE,
        n => parse_note(n).ok_or_else(|| format!("bad note '{n}'"))? as u8,
    };
    Ok(Row { note, inst, cmd, param, note2 })
}

/// A single integer field `f[i]`, the last on its line, in lo..=hi.
fn last_int(f: &[Field], i: usize, lo: i32, hi: i32) -> Option<i32> {
    if f.len() != i + 1 {
        return None;
    }
    f[i].text.parse().ok().filter(|n| (lo..=hi).contains(n))
}

pub fn parse(text: &str) -> Result<Song, SongError> {
    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0;
    while i < lines.len() && lines[i].trim().is_empty() {
        i += 1;
    }
    match lines.get(i).map(|l| l.trim()).unwrap_or("").strip_prefix("acid-track ") {
        Some("1") => {}
        Some(v) => return Err(SongError::new(i + 1, format!("unsupported version {}", v.trim()))),
        None => return Err(SongError::new(i + 1, "not an acid-track file")),
    }
    i += 1;
    let mut song = Song {
        title: String::new(),
        speed: 6,
        sfx_donor: 4,
        instruments: BTreeMap::new(),
        orders: Default::default(),
        patterns: BTreeMap::new(),
    };
    // The line each channel's order came from; 0 = none yet.
    let mut order_line = [0usize; CHANNELS];
    while i < lines.len() {
        let n = i + 1;
        let line = lines[i].trim();
        i += 1;
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let err = |m: String| SongError::new(n, m);
        // A title is free text, so it is taken before `fields` can reject its quotes.
        if line.split_whitespace().next() == Some("title") {
            song.title = line["title".len()..].trim().to_string();
            continue;
        }
        let f = fields(line).map_err(err)?;
        match f[0].text.as_str() {
            "speed" => song.speed = last_int(&f, 1, 1, 31).ok_or_else(|| err("speed must be 1 to 31".into()))? as u8,
            "sfx-donor" => song.sfx_donor = last_int(&f, 1, 1, 4).ok_or_else(|| err("sfx-donor must be 1 to 4".into()))? as u8,
            "instrument" => {
                let (num, inst) = parse_instrument(&f).map_err(err)?;
                if song.instruments.insert(num, inst).is_some() {
                    return Err(err(format!("instrument {num:02X} defined twice")));
                }
            }
            "order" => {
                let (ch, list) = parse_order(&f).map_err(err)?;
                if order_line[ch] != 0 {
                    return Err(err(format!("order {} defined twice", ch + 1)));
                }
                order_line[ch] = n;
                song.orders[ch] = list;
            }
            "pattern" => {
                if f.len() != 3 {
                    return Err(err("expected 'pattern NN LENGTH'".into()));
                }
                let num = hex2(&f[1].text)
                    .filter(|p| *p <= MAX_PATTERN)
                    .ok_or_else(|| err(format!("bad pattern number '{}'", f[1].text)))?;
                let len: usize = f[2]
                    .text
                    .parse()
                    .ok()
                    .filter(|l| (1..=MAX_ROWS).contains(l))
                    .ok_or_else(|| err(format!("pattern length must be 1 to {MAX_ROWS}")))?;
                let mut rows = Vec::with_capacity(len);
                while rows.len() < len {
                    let Some(row_line) = lines.get(i) else {
                        return Err(err(format!("pattern {num:02X} has {} rows, expected {len}", rows.len())));
                    };
                    rows.push(parse_row(row_line).map_err(|m| SongError::new(i + 1, m))?);
                    i += 1;
                }
                if song.patterns.insert(num, rows).is_some() {
                    return Err(err(format!("pattern {num:02X} defined twice")));
                }
            }
            w => return Err(err(format!("unknown line '{w}'"))),
        }
    }
    let last = lines.len().max(1);
    for ch in 0..CHANNELS {
        if order_line[ch] == 0 {
            return Err(SongError::new(last, format!("no order for channel {}", ch + 1)));
        }
        for e in &song.orders[ch].entries {
            if !song.patterns.contains_key(&e.pattern) {
                return Err(SongError::new(order_line[ch], format!("order {} uses missing pattern {:02X}", ch + 1, e.pattern)));
            }
        }
    }
    Ok(song)
}

/// One row as the file writes it, e.g. "C-4 01 4 22 E-4".
pub fn row_text(r: &Row) -> String {
    let note = |n: u8| match n {
        NOTE_NONE => String::from("..."),
        NOTE_OFF => String::from("==="),
        n => note_name(n as i32),
    };
    let inst = if r.inst == 0 { String::from("..") } else { format!("{:02X}", r.inst) };
    let cmd = if r.cmd == 0 { '.' } else { r.cmd as char };
    let param = if r.cmd == 0 && r.param == 0 { String::from("..") } else { format!("{:02X}", r.param) };
    format!("{} {} {} {} {}", note(r.note), inst, cmd, param, note(r.note2))
}

/// The canonical text: parse(write(s)) == s, and write(parse(t)) == t
/// for any canonical t.
pub fn write(song: &Song) -> String {
    let mut s = String::new();
    s += "acid-track 1\n";
    s += &format!("title {}\n", song.title);
    s += &format!("speed {}\n", song.speed);
    s += &format!("sfx-donor {}\n", song.sfx_donor);
    for (num, inst) in &song.instruments {
        s += &format!("instrument {num:02X} \"{}\"", inst.name.replace('"', "'"));
        match &inst.kind {
            Kind::Script { path, block } => s += &format!("  script \"{path}\" {block}"),
            Kind::BuiltIn(b) => {
                s += &format!(
                    "  wave {}  adsr {} {} {} {}  duty {}",
                    WAVES[b.wave.clamp(0, 3) as usize], b.adsr[0], b.adsr[1], b.adsr[2], b.adsr[3], b.duty
                );
                if b.pwm != 0 {
                    s += &format!("  pwm {}", b.pwm);
                }
                if b.vib != (0, 0) {
                    s += &format!("  vib {} {}", b.vib.0, b.vib.1);
                }
                if !b.arp.is_empty() {
                    s += "  arp";
                    for a in &b.arp {
                        s += &format!(" {a}");
                    }
                }
                if let Some((m, c, r)) = b.filter {
                    let mode = match m {
                        2 => "bp",
                        4 => "hp",
                        _ => "lp",
                    };
                    s += &format!("  filter {mode} {c} {r}");
                }
                match b.voice2 {
                    Voice2::Off => {}
                    Voice2::Detune(n) => s += &format!("  voice2 detune {n}"),
                    Voice2::Octave => s += "  voice2 octave",
                    Voice2::Fifth => s += "  voice2 fifth",
                    Voice2::Ring => s += "  voice2 ring",
                }
            }
        }
        s.push('\n');
    }
    for (ch, o) in song.orders.iter().enumerate() {
        s += &format!("order {} ", ch + 1);
        for e in &o.entries {
            s += &format!(" {:02X}", e.pattern);
            if e.transpose != 0 {
                s += &format!("{:+}", e.transpose);
            }
        }
        s += &format!(" loop {}\n", o.loop_to);
    }
    for (num, rows) in &song.patterns {
        s += &format!("\npattern {num:02X} {}\n", rows.len());
        for r in rows {
            s += &row_text(r);
            s.push('\n');
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;

    /// Line numbers: 5 instrument, 6-9 orders, 11 pattern, 12-13 rows.
    pub(crate) const MIN: &str = "acid-track 1\ntitle t\nspeed 6\nsfx-donor 4\ninstrument 01 \"Lead\"  wave saw  adsr 0 8 70 20  duty 50\norder 1  00 loop 0\norder 2  00 loop 0\norder 3  00 loop 0\norder 4  00 loop 0\n\npattern 00 2\nC-4 01 . .. ...\n=== .. 4 22 E-4\n";

    fn err(text: &str) -> String {
        parse(text).unwrap_err().to_string()
    }

    #[test]
    fn parses_the_minimal_song() {
        let s = parse(MIN).unwrap();
        assert_eq!((s.title.as_str(), s.speed, s.sfx_donor), ("t", 6, 4));
        let lead = &s.instruments[&1];
        assert_eq!(lead.name, "Lead");
        let Kind::BuiltIn(b) = &lead.kind else { panic!("built-in") };
        assert_eq!((b.wave, b.adsr, b.duty), (1, [0, 8, 70, 20], 50));
        assert_eq!(s.orders[0].entries, vec![OrderEntry { pattern: 0, transpose: 0 }]);
        assert_eq!(
            s.patterns[&0],
            vec![
                Row { note: 40, inst: 1, cmd: 0, param: 0, note2: 0 },
                Row { note: NOTE_OFF, inst: 0, cmd: b'4', param: 0x22, note2: 44 },
            ]
        );
    }

    #[test]
    fn every_instrument_field() {
        let text = MIN.replace(
            "instrument 01 \"Lead\"  wave saw  adsr 0 8 70 20  duty 50\n",
            "instrument 01 \"Pad Two\"  wave tri  adsr 1 2 3 4  duty 30  pwm 2  vib 4 3  arp 4 7  filter bp 100 5  voice2 detune -6\ninstrument 02 \"S\"  script \"Home/s.snd\" bass\n",
        );
        let s = parse(&text).unwrap();
        assert_eq!(s.instruments[&1].name, "Pad Two");
        assert_eq!(
            s.instruments[&1].kind,
            Kind::BuiltIn(BuiltIn {
                wave: 2, adsr: [1, 2, 3, 4], duty: 30, pwm: 2, vib: (4, 3), arp: alloc::vec![4, 7],
                filter: Some((2, 100, 5)), voice2: Voice2::Detune(-6),
            })
        );
        assert_eq!(s.instruments[&2].kind, Kind::Script { path: "Home/s.snd".into(), block: "bass".into() });
    }

    #[test]
    fn order_entries_transpose() {
        let s = parse(&MIN.replace("order 1  00 loop 0", "order 1  00 00+12 00-3 loop 2")).unwrap();
        let t: Vec<i8> = s.orders[0].entries.iter().map(|e| e.transpose).collect();
        assert_eq!(t, [0, 12, -3]);
        assert_eq!(s.orders[0].loop_to, 2);
    }

    #[test]
    fn errors_name_the_line() {
        assert_eq!(err("hello"), "1: not an acid-track file");
        assert_eq!(err("acid-track 2"), "1: unsupported version 2");
        assert_eq!(err(&MIN.replace("speed 6", "speed 0")), "3: speed must be 1 to 31");
        assert_eq!(err(&MIN.replace("sfx-donor 4", "sfx-donor 5")), "4: sfx-donor must be 1 to 4");
        assert_eq!(err(&MIN.replace("duty 50", "duty 50  wobble 3")), "5: unknown instrument field 'wobble'");
        assert_eq!(err(&MIN.replace("order 1  00 loop 0", "order 1  00 loop 1")), "6: loop 1 is past the end of order 1");
        assert_eq!(err(&MIN.replace("order 2  00", "order 2  05")), "7: order 2 uses missing pattern 05");
        assert_eq!(err(&MIN.replace("order 4  00 loop 0\n", "")), "12: no order for channel 4");
        assert_eq!(err(&MIN.replace("pattern 00 2", "pattern 00 3")), "11: pattern 00 has 2 rows, expected 3");
        assert_eq!(err(&MIN.replace("C-4 01", "C-9 01")), "12: bad note 'C-9'");
        assert_eq!(err(&MIN.replace("=== .. 4", "=== .. S")), "13: command S is reserved");
        assert_eq!(err(&MIN.replace("22 E-4", "22 ===")), "13: bad note '==='");
        assert_eq!(err(&alloc::format!("{MIN}tempo 4\n")), "14: unknown line 'tempo'");
    }

    const ROUND: &str = "acid-track 1
title Round Trip
speed 6
sfx-donor 4
instrument 01 \"Lead\"  wave saw  adsr 0 8 70 20  duty 50  pwm 2  vib 4 2  arp 4 7  filter lp 120 4  voice2 detune 6
instrument 02 \"Bass\"  script \"Home/sounds/demo.snd\" bass
instrument 03 \"Hat\"  wave noise  adsr 0 2 0 2  duty 50  voice2 octave
order 1  00 00+12 loop 1
order 2  01 01-5 loop 0
order 3  01 loop 0
order 4  01 loop 0

pattern 00 4
C-4 01 . .. ...
... .. 4 22 ...
=== .. . .. ...
D#3 02 F 03 G-3

pattern 01 2
A-0 03 9 20 ...
... .. A FF ...
";

    #[test]
    fn canonical_text_round_trips_byte_for_byte() {
        assert_eq!(write(&parse(ROUND).unwrap()), ROUND);
        assert_eq!(write(&parse(MIN).unwrap()), MIN);
    }

    #[test]
    fn a_written_song_parses_back_the_same() {
        let mut s = parse(ROUND).unwrap();
        s.title = "Edited".into();
        s.patterns.get_mut(&1).unwrap()[1] = Row { note: 88, inst: 1, cmd: b'1', param: 0, note2: 1 };
        assert_eq!(parse(&write(&s)).unwrap(), s);
        for t in ["a \"quoted\" one", "say \"hi"] {
            s.title = t.into();
            assert_eq!(parse(&write(&s)).unwrap(), s);
        }
    }

    #[test]
    fn row_text_shows_empty_fields_as_dots() {
        assert_eq!(row_text(&Row::default()), "... .. . .. ...");
        assert_eq!(row_text(&Row { note: 40, inst: 0x1F, cmd: b'A', param: 0, note2: 52 }), "C-4 1F A 00 C-5");
    }

    #[test]
    fn a_title_is_free_text() {
        assert_eq!(parse(&MIN.replace("title t", "title a \"quoted\" one")).unwrap().title, "a \"quoted\" one");
        assert_eq!(parse(&MIN.replace("title t", "title say \"hi")).unwrap().title, "say \"hi");
        assert_eq!(parse(&MIN.replace("title t", "title")).unwrap().title, "");
    }

    #[test]
    fn instrument_numbers_are_clamped() {
        let text = MIN.replace(
            "adsr 0 8 70 20  duty 50",
            "adsr -5 200000 50 3  vib 99 -1  pwm 1000  arp 100 -100  voice2 detune 99999",
        );
        let s = parse(&text).unwrap();
        let Kind::BuiltIn(b) = &s.instruments[&1].kind else { panic!("built-in") };
        assert_eq!(b.adsr, [0, 100_000, 50, 3]);
        assert_eq!((b.vib, b.pwm), ((15, 0), 50));
        assert_eq!(b.arp, alloc::vec![48, -48]);
        assert_eq!(b.voice2, Voice2::Detune(768));
        let low = MIN.replace("adsr 0 8 70 20  duty 50", "adsr 0 8 70 20  pwm -1000  voice2 detune -99999");
        let Kind::BuiltIn(b) = &parse(&low).unwrap().instruments[&1].kind else { panic!("built-in") };
        assert_eq!((b.pwm, b.voice2), (-50, Voice2::Detune(-768)));
    }
}
