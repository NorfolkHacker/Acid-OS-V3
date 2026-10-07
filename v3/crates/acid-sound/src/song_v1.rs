//! Old songs: `acid-track 1`, four channels of two voices, each channel
//! with its own order list. `upgrade` reads one as strictly as the old
//! reader did and turns it into an eight-track song, so songs saved before
//! tracks still open (and save back in the new form).
//!
//! Channel c plays on track c, and its second-note column on track 4+c.
//! Order transposes are worked into the notes. The channel order lists are
//! played out side by side, row by row, until they all repeat together,
//! then cut into patterns; patterns that come out the same are shared.
//! `voice2` instrument modes have no track of their own and are dropped.

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::pitch::{parse_note, ONA_MAX, ONA_MIN};
use crate::song::{
    fields, hex2, last_int, parse_cell, parse_instrument, Cell, Field, Row, Song, SongError, MAX_ORDER, MAX_PATTERN, MAX_ROWS,
    NOTE_NONE, NOTE_OFF, TRACKS,
};

const CHANNELS: usize = 4;
/// Old pattern numbers ran to 7F.
const OLD_MAX_PATTERN: u8 = 0x7F;

/// One channel's row: a track cell and the second note.
#[derive(Clone, Copy)]
struct OldRow {
    cell: Cell,
    note2: u8,
}

struct OldOrder {
    /// Pattern and transpose.
    entries: Vec<(u8, i8)>,
    loop_to: usize,
}

fn parse_order(f: &[Field]) -> Result<(usize, OldOrder), String> {
    let ch: usize = f
        .get(1)
        .and_then(|x| x.text.parse().ok())
        .filter(|c| (1..=CHANNELS).contains(c))
        .ok_or("order channel must be 1 to 4")?;
    let mut entries = Vec::new();
    let mut i = 2;
    while i < f.len() && f[i].text != "loop" {
        let t = &f[i].text;
        let (p, tr) = match t.find(['+', '-']) {
            Some(k) => (&t[..k], t[k..].parse::<i8>().ok()),
            None => (t.as_str(), Some(0)),
        };
        match (hex2(p).filter(|n| *n <= OLD_MAX_PATTERN), tr) {
            (Some(p), Some(tr)) => entries.push((p, tr)),
            _ => return Err(format!("bad order entry '{t}'")),
        }
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
    Ok((ch - 1, OldOrder { entries, loop_to }))
}

fn parse_row(line: &str) -> Result<OldRow, String> {
    let f: Vec<&str> = line.split_whitespace().collect();
    if f.len() != 5 {
        return Err("expected a row like 'C-4 01 . .. ...'".into());
    }
    let cell = parse_cell(&f[..4].join(" "))?;
    let note2 = match f[4] {
        "..." => NOTE_NONE,
        n => parse_note(n).ok_or_else(|| format!("bad note '{n}'"))? as u8,
    };
    Ok(OldRow { cell, note2 })
}

/// An instrument line without its `voice2` field, which tracks don't have.
fn without_voice2(f: Vec<Field>) -> Result<Vec<Field>, String> {
    let mut out = Vec::with_capacity(f.len());
    let mut it = f.into_iter().peekable();
    while let Some(x) = it.next() {
        if x.quoted || x.text != "voice2" {
            out.push(x);
            continue;
        }
        match it.next().map(|m| m.text) {
            Some(m) if m == "detune" => {
                if it.next_if(|n| n.text.parse::<i32>().is_ok_and(|v| (-768..=768).contains(&v))).is_none() {
                    return Err("detune must be -768 to 768".into());
                }
            }
            Some(m) if ["off", "octave", "fifth", "ring"].contains(&m.as_str()) => {}
            m => return Err(format!("unknown voice2 mode '{}'", m.unwrap_or_default())),
        }
    }
    Ok(out)
}

struct Old {
    title: String,
    speed: u8,
    instruments: BTreeMap<u8, crate::song::Instrument>,
    orders: Vec<OldOrder>,
    patterns: BTreeMap<u8, Vec<OldRow>>,
}

/// The old reader, line for line: the same checks and messages.
fn read(text: &str) -> Result<Old, SongError> {
    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0;
    while i < lines.len() && lines[i].trim().is_empty() {
        i += 1;
    }
    i += 1;
    let mut old = Old { title: String::new(), speed: 6, instruments: BTreeMap::new(), orders: Vec::new(), patterns: BTreeMap::new() };
    let mut orders: [Option<(usize, OldOrder)>; CHANNELS] = Default::default();
    while i < lines.len() {
        let n = i + 1;
        let line = lines[i].trim();
        i += 1;
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let err = |m: String| SongError::new(n, m);
        if line.split_whitespace().next() == Some("title") {
            old.title = line["title".len()..].trim().to_string();
            continue;
        }
        let f = fields(line).map_err(err)?;
        match f[0].text.as_str() {
            "speed" => old.speed = last_int(&f, 1, 1, 31).ok_or_else(|| err("speed must be 1 to 31".into()))? as u8,
            "sfx-donor" => {
                last_int(&f, 1, 1, 4).ok_or_else(|| err("sfx-donor must be 1 to 4".into()))?;
            }
            "instrument" => {
                let (num, inst) = parse_instrument(&without_voice2(f).map_err(err)?).map_err(err)?;
                if old.instruments.insert(num, inst).is_some() {
                    return Err(err(format!("instrument {num:02X} defined twice")));
                }
            }
            "order" => {
                let (ch, list) = parse_order(&f).map_err(err)?;
                if orders[ch].is_some() {
                    return Err(err(format!("order {} defined twice", ch + 1)));
                }
                orders[ch] = Some((n, list));
            }
            "pattern" => {
                if f.len() != 3 {
                    return Err(err("expected 'pattern NN LENGTH'".into()));
                }
                let num = hex2(&f[1].text)
                    .filter(|p| *p <= OLD_MAX_PATTERN)
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
                if old.patterns.insert(num, rows).is_some() {
                    return Err(err(format!("pattern {num:02X} defined twice")));
                }
            }
            w => return Err(err(format!("unknown line '{w}'"))),
        }
    }
    let last = lines.len().max(1);
    for (ch, o) in orders.into_iter().enumerate() {
        let Some((line_no, list)) = o else {
            return Err(SongError::new(last, format!("no order for channel {}", ch + 1)));
        };
        if let Some((p, _)) = list.entries.iter().find(|(p, _)| !old.patterns.contains_key(p)) {
            return Err(SongError::new(line_no, format!("order {} uses missing pattern {p:02X}", ch + 1)));
        }
        old.orders.push(list);
    }
    Ok(old)
}

fn gcd(a: usize, b: usize) -> usize {
    if b == 0 { a } else { gcd(b, a % b) }
}

/// Reads an old song and returns it as tracks.
pub fn upgrade(text: &str) -> Result<Song, SongError> {
    let old = read(text)?;
    let last = text.lines().count().max(1);
    let too_long = || SongError::new(last, "this 4-channel song is too long to turn into tracks");
    // Each channel played out row by row: the rows before its loop point, then the loop.
    let mut streams: Vec<(Vec<(OldRow, i8)>, usize)> = Vec::with_capacity(CHANNELS);
    for o in &old.orders {
        let mut rows = Vec::new();
        let mut intro = 0;
        for (k, (p, tr)) in o.entries.iter().enumerate() {
            if k == o.loop_to {
                intro = rows.len();
            }
            rows.extend(old.patterns[p].iter().map(|r| (*r, *tr)));
        }
        streams.push((rows, intro));
    }
    let limit = MAX_ROWS * MAX_ORDER;
    let intro = streams.iter().map(|(_, i)| *i).max().unwrap_or(0);
    let mut cycle = 1usize;
    for (rows, i) in &streams {
        let c = rows.len() - i;
        cycle = cycle / gcd(cycle, c) * c;
        if cycle > limit {
            return Err(too_long());
        }
    }
    let total = intro + cycle;
    if total > limit {
        return Err(too_long());
    }
    // Pattern length: as long as the old patterns, and dividing both the
    // intro and the loop, so the loop starts on a pattern.
    let longest = old.patterns.values().map(Vec::len).max().unwrap_or(1).min(MAX_ROWS);
    let g = gcd(intro, cycle);
    let len = (1..=longest).rev().find(|d| g.is_multiple_of(*d)).unwrap_or(1);

    let mut grid: Vec<Row> = alloc::vec![[Cell::default(); TRACKS]; total];
    for (ch, (rows, ch_intro)) in streams.iter().enumerate() {
        let ch_cycle = rows.len() - ch_intro;
        let (mut inst, mut v2_on) = (0u8, false);
        for (r, out) in grid.iter_mut().enumerate() {
            let k = if r < *ch_intro { r } else { ch_intro + (r - ch_intro) % ch_cycle };
            let (row, tr) = rows[k];
            let shift = |n: u8| (n as i32 + tr as i32).clamp(ONA_MIN, ONA_MAX) as u8;
            let mut cell = row.cell;
            if cell.inst != 0 {
                inst = cell.inst;
            }
            if !matches!(cell.note, NOTE_NONE | NOTE_OFF) {
                cell.note = shift(cell.note);
            }
            out[ch] = cell;
            let second = &mut out[CHANNELS + ch];
            match row.cell.note {
                NOTE_NONE => {}
                // A glide doesn't start a note, so it leaves voice 2 alone.
                _ if row.cell.cmd == b'3' => {}
                NOTE_OFF => {
                    if v2_on {
                        *second = Cell { note: NOTE_OFF, ..Cell::default() };
                        v2_on = false;
                    }
                }
                _ if row.note2 != NOTE_NONE => {
                    *second = Cell { note: shift(row.note2), inst, ..Cell::default() };
                    v2_on = true;
                }
                // A new note without a second one silenced voice 2.
                _ if v2_on => {
                    *second = Cell { note: NOTE_OFF, ..Cell::default() };
                    v2_on = false;
                }
                _ => {}
            }
        }
    }

    let mut patterns: BTreeMap<u8, Vec<Row>> = BTreeMap::new();
    let mut order = Vec::new();
    for chunk in grid.chunks(len) {
        let num = match patterns.iter().find(|(_, p)| p.as_slice() == chunk) {
            Some((n, _)) => *n,
            None => {
                let n = patterns.len();
                if n > MAX_PATTERN as usize {
                    return Err(SongError::new(last, "this 4-channel song needs more than 64 patterns as tracks"));
                }
                patterns.insert(n as u8, chunk.to_vec());
                n as u8
            }
        };
        order.push(num);
    }
    if order.len() > MAX_ORDER {
        return Err(too_long());
    }
    Ok(Song { title: old.title, speed: old.speed, instruments: old.instruments, order, loop_to: intro / len, patterns })
}

#[cfg(test)]
mod tests {
    use crate::song::{parse, write};

    const E: &str = "... .. . ..";

    fn row(cells: &[&str]) -> alloc::string::String {
        (0..8).map(|t| cells.get(t).copied().unwrap_or(E)).collect::<alloc::vec::Vec<_>>().join(" | ")
    }

    const OLD: &str = "acid-track 1
title Old One
speed 5
sfx-donor 4
instrument 01 \"Lead\"  wave saw  adsr 0 8 70 20  duty 50  voice2 detune 6
instrument 02 \"Pad\"  wave tri  adsr 0 8 70 20  duty 50
order 1  00 00+12 loop 0
order 2  01 loop 0
order 3  02 loop 0
order 4  02 loop 0

pattern 00 2
C-4 01 . .. ...
=== .. 4 22 ...

pattern 01 4
C-2 02 . .. E-2
... .. . .. ...
D-2 .. . .. ...
=== .. . .. ...

pattern 02 1
... .. . .. ...
";

    #[test]
    fn an_old_song_becomes_tracks() {
        let s = parse(OLD).unwrap();
        assert_eq!((s.title.as_str(), s.speed), ("Old One", 5));
        assert_eq!((s.order.as_slice(), s.loop_to), (&[0u8][..], 0), "one 4-row pattern: lcm(4, 4, 1, 1) rows");
        let want = alloc::format!(
            "acid-track 2
title Old One
speed 5
instrument 01 \"Lead\"  wave saw  adsr 0 8 70 20  duty 50
instrument 02 \"Pad\"  wave tri  adsr 0 8 70 20  duty 50
order 00 loop 0

pattern 00 4
{}
{}
{}
{}
",
            row(&["C-4 01 . ..", "C-2 02 . ..", E, E, E, "E-2 02 . .."]),
            row(&["=== .. 4 22"]),
            row(&["C-5 01 . ..", "D-2 .. . ..", E, E, E, "=== .. . .."]),
            row(&["=== .. 4 22", "=== .. . .."]),
        );
        assert_eq!(write(&s), want, "the transpose is worked in, the second note has its own track, voice2 is gone");
    }

    #[test]
    fn a_loop_point_after_an_intro_is_kept() {
        let text = OLD.replace("order 1  00 00+12 loop 0", "order 1  00 00+12 loop 1");
        let s = parse(&text).unwrap();
        // Channel 1: 2 intro rows, then a 2-row loop; channel 2 loops 4 rows.
        assert_eq!(s.loop_to, 1, "the song loops back after the intro");
        assert_eq!(s.order.len(), 3);
    }

    #[test]
    fn old_errors_still_name_their_line() {
        let e = |t: &str| parse(t).unwrap_err().to_string();
        assert_eq!(e(&OLD.replace("order 4  02 loop 0\n", "")), "22: no order for channel 4");
        assert_eq!(e(&OLD.replace("voice2 detune 6", "voice2 wobble")), "5: unknown voice2 mode 'wobble'");
        assert_eq!(e(&OLD.replace("voice2 detune 6", "voice2 detune 999")), "5: detune must be -768 to 768");
        assert_eq!(e(&OLD.replace("C-2 02 . .. E-2", "C-2 02 . .. ===")), "17: bad note '==='");
    }

    #[test]
    fn songs_too_long_for_tracks_are_refused() {
        // Coprime loop lengths multiply past 64 x 128 rows.
        let mut text = alloc::string::String::from("acid-track 1\ntitle t\nspeed 6\n");
        let lens = [61, 59, 53, 47];
        for ch in 0..lens.len() {
            text += &alloc::format!("order {} {ch:02X} loop 0\n", ch + 1);
        }
        for (ch, n) in lens.iter().enumerate() {
            text += &alloc::format!("pattern {ch:02X} {n}\n");
            for _ in 0..*n {
                text += "... .. . .. ...\n";
            }
        }
        assert!(parse(&text).unwrap_err().to_string().contains("too long"));
    }
}
