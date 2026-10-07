//! The .trk song format: a versioned text file in the house style of
//! .spr. `parse` is strict and is the one reader of record (games and the
//! tracker both use it); `write` emits the canonical form.
//!
//! A song is one order list of patterns, ProTracker style. Every pattern
//! holds all TRACKS tracks, so its length is every track's length. Track
//! n plays synth voice n.

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt;

use crate::pitch::{note_name, parse_note};

pub const TRACKS: usize = 8;
pub const MAX_ROWS: usize = 64;
/// Patterns are numbered 00..=3F: 64 of them.
pub const MAX_PATTERN: u8 = 0x3F;
/// The longest order list.
pub const MAX_ORDER: usize = 128;
pub const MAX_INSTRUMENT: u8 = 0x3F;
pub const NOTE_NONE: u8 = 0;
pub const NOTE_OFF: u8 = 0xFF;
/// Pattern command letters. `S` is reserved for script calls.
pub const COMMANDS: &[u8] = b"123489AF";
/// Waveform names, in synth order.
pub const WAVES: [&str; 4] = ["pulse", "saw", "tri", "noise"];
/// What separates a row's tracks in the file.
const SEP: &str = " | ";

/// One track's part of a row. `note` is ona (or NOTE_NONE/NOTE_OFF);
/// `cmd` is 0 or the command's ASCII letter.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cell {
    pub note: u8,
    pub inst: u8,
    pub cmd: u8,
    pub param: u8,
}

/// One row of a pattern: a cell for every track.
pub type Row = [Cell; TRACKS];

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
}

impl Default for BuiltIn {
    fn default() -> Self {
        Self { wave: 0, adsr: [2, 40, 80, 40], duty: 50, pwm: 0, vib: (0, 0), arp: Vec::new(), filter: None }
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Song {
    pub title: String,
    pub speed: u8,
    pub instruments: BTreeMap<u8, Instrument>,
    /// Pattern numbers, played in turn; after the last, play goes back to `loop_to`.
    pub order: Vec<u8>,
    pub loop_to: usize,
    pub patterns: BTreeMap<u8, Vec<Row>>,
}

impl Song {
    /// The tracks that play a note somewhere in the order: a bit per track.
    /// The others' voices are free for sound effects while the song plays.
    pub fn used_tracks(&self) -> u8 {
        let mut m = 0u8;
        for p in &self.order {
            for row in self.patterns.get(p).into_iter().flatten() {
                for (t, c) in row.iter().enumerate() {
                    if c.note != NOTE_NONE {
                        m |= 1 << t;
                    }
                }
            }
        }
        m
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SongError {
    pub line: u32,
    pub message: String,
}

impl SongError {
    pub(crate) fn new(line: usize, message: impl Into<String>) -> Self {
        Self { line: line as u32, message: message.into() }
    }
}

impl fmt::Display for SongError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.line, self.message)
    }
}

pub(crate) fn hex2(s: &str) -> Option<u8> {
    if s.len() != 2 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    u8::from_str_radix(s, 16).ok()
}

pub(crate) struct Field {
    pub(crate) text: String,
    pub(crate) quoted: bool,
}

/// Splits on whitespace; "..." is one field (quotes removed).
pub(crate) fn fields(line: &str) -> Result<Vec<Field>, String> {
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

    /// A number that must lie in `lo..=hi`; out of range is a load error, not a clamp.
    fn ranged(&mut self, field: &str, lo: i32, hi: i32) -> Result<i32, String> {
        let n = self.int(field)?;
        if (lo..=hi).contains(&n) {
            Ok(n)
        } else {
            Err(format!("{field} must be {lo} to {hi}"))
        }
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

pub(crate) fn parse_instrument(f: &[Field]) -> Result<(u8, Instrument), String> {
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
                    b.adsr[k] = c.ranged("adsr", 0, 100_000)?;
                }
            }
            "duty" => b.duty = c.ranged("duty", 1, 99)?,
            "pwm" => b.pwm = c.ranged("pwm", -50, 50)?,
            "vib" => b.vib = (c.ranged("vib", 0, 15)?, c.ranged("vib", 0, 15)?),
            "arp" => {
                b.arp.clear();
                while b.arp.len() < 3 && c.next_is_int() {
                    b.arp.push(c.ranged("arp", -48, 48)?);
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
            "script" => return Err("'script' must come right after the name".into()),
            k => return Err(format!("unknown instrument field '{k}'")),
        }
    }
    Ok((num, Instrument { name, kind: Kind::BuiltIn(b) }))
}

/// `order 00 01 00 loop 0`
fn parse_order(f: &[Field]) -> Result<(Vec<u8>, usize), String> {
    let mut order = Vec::new();
    let mut i = 1;
    while i < f.len() && f[i].text != "loop" {
        let p = hex2(&f[i].text)
            .filter(|n| *n <= MAX_PATTERN)
            .ok_or_else(|| format!("bad order entry '{}'", f[i].text))?;
        order.push(p);
        i += 1;
    }
    if order.is_empty() {
        return Err("the order is empty".into());
    }
    if order.len() > MAX_ORDER {
        return Err(format!("the order has more than {MAX_ORDER} entries"));
    }
    if i + 2 != f.len() {
        return Err("the order must end with 'loop N'".into());
    }
    let loop_to: usize = f[i + 1].text.parse().map_err(|_| String::from("the order must end with 'loop N'"))?;
    if loop_to >= order.len() {
        return Err(format!("loop {loop_to} is past the end of the order"));
    }
    Ok((order, loop_to))
}

/// One track's cell, e.g. "C-4 01 4 22".
pub(crate) fn parse_cell(text: &str) -> Result<Cell, String> {
    let f: Vec<&str> = text.split_whitespace().collect();
    if f.len() != 4 {
        return Err("expected a cell like 'C-4 01 . ..'".into());
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
    Ok(Cell { note, inst, cmd, param })
}

/// A row: TRACKS cells separated by '|'.
fn parse_row(line: &str) -> Result<Row, String> {
    let parts: Vec<&str> = line.split('|').collect();
    if parts.len() != TRACKS {
        return Err(format!("a row has {TRACKS} tracks split by '|', not {}", parts.len()));
    }
    let mut row = [Cell::default(); TRACKS];
    for (cell, text) in row.iter_mut().zip(parts) {
        *cell = parse_cell(text)?;
    }
    Ok(row)
}

/// A single integer field `f[i]`, the last on its line, in lo..=hi.
pub(crate) fn last_int(f: &[Field], i: usize, lo: i32, hi: i32) -> Option<i32> {
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
        Some("2") => {}
        Some("1") => return crate::song_v1::upgrade(text),
        Some(v) => return Err(SongError::new(i + 1, format!("unsupported version {}", v.trim()))),
        None => return Err(SongError::new(i + 1, "not an acid-track file")),
    }
    i += 1;
    let mut song = Song {
        title: String::new(),
        speed: 6,
        instruments: BTreeMap::new(),
        order: Vec::new(),
        loop_to: 0,
        patterns: BTreeMap::new(),
    };
    // The line the order came from; 0 = none yet.
    let mut order_line = 0usize;
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
            "instrument" => {
                let (num, inst) = parse_instrument(&f).map_err(err)?;
                if song.instruments.insert(num, inst).is_some() {
                    return Err(err(format!("instrument {num:02X} defined twice")));
                }
            }
            "order" => {
                if order_line != 0 {
                    return Err(err("the order is defined twice".into()));
                }
                (song.order, song.loop_to) = parse_order(&f).map_err(err)?;
                order_line = n;
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
    if order_line == 0 {
        return Err(SongError::new(lines.len().max(1), "the song has no order"));
    }
    if let Some(p) = song.order.iter().find(|p| !song.patterns.contains_key(p)) {
        return Err(SongError::new(order_line, format!("the order uses missing pattern {p:02X}")));
    }
    Ok(song)
}

/// One track's cell as the file writes it, e.g. "C-4 01 4 22".
pub fn cell_text(c: &Cell) -> String {
    let note = match c.note {
        NOTE_NONE => String::from("..."),
        NOTE_OFF => String::from("==="),
        n => note_name(n as i32),
    };
    let inst = if c.inst == 0 { String::from("..") } else { format!("{:02X}", c.inst) };
    let cmd = if c.cmd == 0 { '.' } else { c.cmd as char };
    let param = if c.cmd == 0 && c.param == 0 { String::from("..") } else { format!("{:02X}", c.param) };
    format!("{note} {inst} {cmd} {param}")
}

/// One row as the file writes it: every track's cell, split by " | ".
pub fn row_text(r: &Row) -> String {
    let cells: Vec<String> = r.iter().map(cell_text).collect();
    cells.join(SEP)
}

/// The canonical text: parse(write(s)) == s, and write(parse(t)) == t
/// for any canonical t.
pub fn write(song: &Song) -> String {
    let mut s = String::new();
    s += "acid-track 2\n";
    s += &format!("title {}\n", song.title);
    s += &format!("speed {}\n", song.speed);
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
            }
        }
        s.push('\n');
    }
    s += "order";
    for p in &song.order {
        s += &format!(" {p:02X}");
    }
    s += &format!(" loop {}\n", song.loop_to);
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

    /// Seven empty cells: what follows track 1 in a row.
    pub(crate) const REST: &str = " | ... .. . .. | ... .. . .. | ... .. . .. | ... .. . .. | ... .. . .. | ... .. . .. | ... .. . ..";

    /// Line numbers: 4 instrument, 5 order, 7 pattern, 8-9 rows.
    fn min() -> String {
        format!(
            "acid-track 2\ntitle t\nspeed 6\ninstrument 01 \"Lead\"  wave saw  adsr 0 8 70 20  duty 50\norder 00 loop 0\n\npattern 00 2\nC-4 01 . ..{REST}\n=== .. 4 22 | E-4 01 . ..{}\n",
            &REST[14..]
        )
    }

    fn err(text: &str) -> String {
        parse(text).unwrap_err().to_string()
    }

    #[test]
    fn parses_the_minimal_song() {
        let s = parse(&min()).unwrap();
        assert_eq!((s.title.as_str(), s.speed), ("t", 6));
        let lead = &s.instruments[&1];
        assert_eq!(lead.name, "Lead");
        let Kind::BuiltIn(b) = &lead.kind else { panic!("built-in") };
        assert_eq!((b.wave, b.adsr, b.duty), (1, [0, 8, 70, 20], 50));
        assert_eq!((s.order.as_slice(), s.loop_to), (&[0u8][..], 0));
        let rows = &s.patterns[&0];
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0][0], Cell { note: 40, inst: 1, cmd: 0, param: 0 });
        assert_eq!(rows[1][0], Cell { note: NOTE_OFF, inst: 0, cmd: b'4', param: 0x22 });
        assert_eq!(rows[1][1], Cell { note: 44, inst: 1, cmd: 0, param: 0 });
        assert_eq!(rows[1][7], Cell::default());
    }

    #[test]
    fn every_instrument_field() {
        let text = min().replace(
            "instrument 01 \"Lead\"  wave saw  adsr 0 8 70 20  duty 50\n",
            "instrument 01 \"Pad Two\"  wave tri  adsr 1 2 3 4  duty 30  pwm 2  vib 4 3  arp 4 7  filter bp 100 5\ninstrument 02 \"S\"  script \"Home/s.snd\" bass\n",
        );
        let s = parse(&text).unwrap();
        assert_eq!(s.instruments[&1].name, "Pad Two");
        assert_eq!(
            s.instruments[&1].kind,
            Kind::BuiltIn(BuiltIn {
                wave: 2, adsr: [1, 2, 3, 4], duty: 30, pwm: 2, vib: (4, 3), arp: alloc::vec![4, 7],
                filter: Some((2, 100, 5)),
            })
        );
        assert_eq!(s.instruments[&2].kind, Kind::Script { path: "Home/s.snd".into(), block: "bass".into() });
    }

    #[test]
    fn the_order_loops() {
        let s = parse(&min().replace("order 00 loop 0", "order 00 00 00 loop 2")).unwrap();
        assert_eq!((s.order.len(), s.loop_to), (3, 2));
    }

    #[test]
    fn errors_name_the_line() {
        let m = min();
        assert_eq!(err("hello"), "1: not an acid-track file");
        assert_eq!(err("acid-track 3"), "1: unsupported version 3");
        assert_eq!(err("acid-track 1\nsfx-donor 5"), "2: sfx-donor must be 1 to 4");
        assert_eq!(err(&m.replace("speed 6", "speed 0")), "3: speed must be 1 to 31");
        assert_eq!(err(&m.replace("duty 50", "duty 50  wobble 3")), "4: unknown instrument field 'wobble'");
        assert_eq!(err(&m.replace("duty 50", "duty 50  voice2 octave")), "4: unknown instrument field 'voice2'");
        assert_eq!(err(&m.replace("order 00 loop 0", "order 00 loop 1")), "5: loop 1 is past the end of the order");
        assert_eq!(err(&m.replace("order 00 loop 0", "order 00 05 loop 0")), "5: the order uses missing pattern 05");
        assert_eq!(err(&m.replace("order 00 loop 0", "order 40 loop 0")), "5: bad order entry '40'");
        assert_eq!(err(&m.replace("order 00 loop 0", "order 00+5 loop 0")), "5: bad order entry '00+5'");
        assert_eq!(err(&m.replace("order 00 loop 0", "order loop 0")), "5: the order is empty");
        let long = format!("order{} loop 0", " 00".repeat(MAX_ORDER + 1));
        assert_eq!(err(&m.replace("order 00 loop 0", &long)), "5: the order has more than 128 entries");
        assert_eq!(err(&m.replace("order 00 loop 0\n", "")), "8: the song has no order");
        assert_eq!(err(&m.replace("pattern 00 2", "pattern 40 2")), "7: bad pattern number '40'");
        assert_eq!(err(&m.replace("pattern 00 2", "pattern 00 3")), "7: pattern 00 has 2 rows, expected 3");
        assert_eq!(err(&m.replace("pattern 00 2", "pattern 00 65")), "7: pattern length must be 1 to 64");
        assert_eq!(err(&m.replace("C-4 01", "C-9 01")), "8: bad note 'C-9'");
        assert_eq!(err(&m.replace("=== .. 4", "=== .. S")), "9: command S is reserved");
        assert_eq!(err(&m.replace("C-4 01 . .. | ", "C-4 01 . .. ")), "8: a row has 8 tracks split by '|', not 7");
        assert_eq!(err(&m.replace("C-4 01 . ..", "C-4 01 .")), "8: expected a cell like 'C-4 01 . ..'");
        assert_eq!(err(&m.replace("C-4 01 . ..", "C-4 01 . .. | ... .. . ..")), "8: a row has 8 tracks split by '|', not 9");
        assert_eq!(err(&format!("{m}tempo 4\n")), "10: unknown line 'tempo'");
    }

    fn round() -> String {
        let row = |cells: &[&str]| {
            let mut all: Vec<&str> = cells.to_vec();
            all.resize(TRACKS, "... .. . ..");
            all.join(" | ")
        };
        let mut s = String::from(
            "acid-track 2
title Round Trip
speed 6
instrument 01 \"Lead\"  wave saw  adsr 0 8 70 20  duty 50  pwm 2  vib 4 2  arp 4 7  filter lp 120 4
instrument 02 \"Bass\"  script \"Home/sounds/demo.snd\" bass
instrument 03 \"Hat\"  wave noise  adsr 0 2 0 2  duty 50
order 00 01 00 loop 1

pattern 00 4
",
        );
        for r in [
            row(&["C-4 01 . ..", "A-0 03 9 20"]),
            row(&["... .. 4 22"]),
            row(&["=== .. . ..", "... .. . ..", "... .. . ..", "... .. . ..", "... .. . ..", "... .. . ..", "... .. . ..", "C-8 02 1 00"]),
            row(&["D#3 02 F 03"]),
        ] {
            s += &r;
            s.push('\n');
        }
        s += "\npattern 3F 2\n";
        s += &row(&["... .. A FF"]);
        s += "\n";
        s += &row(&[]);
        s += "\n";
        s.replace("order 00 01 00", "order 00 3F 00")
    }

    #[test]
    fn canonical_text_round_trips_byte_for_byte() {
        assert_eq!(write(&parse(&round()).unwrap()), round());
        assert_eq!(write(&parse(&min()).unwrap()), min());
    }

    #[test]
    fn a_written_song_parses_back_the_same() {
        let mut s = parse(&round()).unwrap();
        s.title = "Edited".into();
        s.patterns.get_mut(&0x3F).unwrap()[1][5] = Cell { note: 88, inst: 1, cmd: b'1', param: 0 };
        assert_eq!(parse(&write(&s)).unwrap(), s);
        for t in ["a \"quoted\" one", "say \"hi"] {
            s.title = t.into();
            assert_eq!(parse(&write(&s)).unwrap(), s);
        }
    }

    #[test]
    fn cells_and_rows_show_empty_fields_as_dots() {
        assert_eq!(cell_text(&Cell::default()), "... .. . ..");
        assert_eq!(cell_text(&Cell { note: 40, inst: 0x1F, cmd: b'A', param: 0 }), "C-4 1F A 00");
        let mut r = [Cell::default(); TRACKS];
        r[0].note = NOTE_OFF;
        assert_eq!(row_text(&r), format!("=== .. . ..{REST}"));
    }

    #[test]
    fn used_tracks_are_those_with_notes_in_the_order() {
        let s = parse(&round()).unwrap();
        assert_eq!(s.used_tracks(), 0b1000_0011);
        let only_00 = parse(&round().replace("order 00 3F 00 loop 1", "order 00 loop 0")).unwrap();
        assert_eq!(only_00.used_tracks(), 0b1000_0011, "pattern 3F has no notes anyway");
        let m = parse(&min()).unwrap();
        assert_eq!(m.used_tracks(), 0b11);
    }

    #[test]
    fn a_title_is_free_text() {
        let m = min();
        assert_eq!(parse(&m.replace("title t", "title a \"quoted\" one")).unwrap().title, "a \"quoted\" one");
        assert_eq!(parse(&m.replace("title t", "title say \"hi")).unwrap().title, "say \"hi");
        assert_eq!(parse(&m.replace("title t", "title")).unwrap().title, "");
    }

    #[test]
    fn out_of_range_instrument_numbers_are_errors() {
        let cases = [
            ("adsr -5 8 70 20", "4: adsr must be 0 to 100000"),
            ("adsr 0 200000 70 20", "4: adsr must be 0 to 100000"),
            ("adsr 0 8 70 2147483647", "4: adsr must be 0 to 100000"),
            ("adsr 0 8 70 20  pwm 51", "4: pwm must be -50 to 50"),
            ("adsr 0 8 70 20  pwm -2147483648", "4: pwm must be -50 to 50"),
            ("adsr 0 8 70 20  vib 16 1", "4: vib must be 0 to 15"),
            ("adsr 0 8 70 20  vib 1 -1", "4: vib must be 0 to 15"),
            ("adsr 0 8 70 20  arp 4 49", "4: arp must be -48 to 48"),
            ("adsr 0 8 70 20  arp -49", "4: arp must be -48 to 48"),
            ("adsr 0 8 70 20  duty 0", "4: duty must be 1 to 99"),
            ("adsr 0 8 70 20  duty 100", "4: duty must be 1 to 99"),
        ];
        for (fields, want) in cases {
            assert_eq!(err(&min().replace("adsr 0 8 70 20  duty 50", fields)), want, "{fields}");
        }
    }

    #[test]
    fn instrument_numbers_at_their_limits_load() {
        let text = min().replace("adsr 0 8 70 20  duty 50", "adsr 0 100000 50 3  vib 15 0  pwm 50  arp 48 -48  duty 99");
        let s = parse(&text).unwrap();
        let Kind::BuiltIn(b) = &s.instruments[&1].kind else { panic!("built-in") };
        assert_eq!(b.adsr, [0, 100_000, 50, 3]);
        assert_eq!((b.vib, b.pwm, b.duty), ((15, 0), 50, 99));
        assert_eq!(b.arp, alloc::vec![48, -48]);
        let low = min().replace("adsr 0 8 70 20  duty 50", "adsr 0 8 70 20  pwm -50  duty 1");
        let Kind::BuiltIn(b) = &parse(&low).unwrap().instruments[&1].kind else { panic!("built-in") };
        assert_eq!((b.pwm, b.duty), (-50, 1));
    }
}
