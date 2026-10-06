//! Files that name other files: a song's script instruments, a program's
//! `song "PATH"` lines. `read` fetches text by fsroot-relative path, so
//! this crate never touches a filesystem itself.

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::compile::compile;
use crate::lexer::CompileError;
use crate::player::LoadedSong;
use crate::program::{BlockKind, Program};
use crate::song::{parse, Kind, SongError};

pub type Read<'a> = &'a dyn Fn(&str) -> Result<String, String>;

/// Parses a song and compiles its script instruments. A script that won't
/// load leaves its instrument silent and adds a warning; a bad song is an error.
pub fn load_song(text: &str, read: Read) -> Result<(LoadedSong, Vec<String>), SongError> {
    let song = parse(text)?;
    let mut cache: BTreeMap<String, Result<Arc<Program>, String>> = BTreeMap::new();
    let mut scripts = BTreeMap::new();
    let mut warnings = Vec::new();
    for (num, inst) in &song.instruments {
        let Kind::Script { path, block } = &inst.kind else { continue };
        let prog = cache
            .entry(path.clone())
            .or_insert_with(|| read(path).and_then(|src| compile(&src).map(Arc::new).map_err(|e| e.to_string())))
            .clone();
        match prog {
            Err(e) => warnings.push(format!("instrument {num:02X}: {path}: {e}")),
            Ok(p) => match p.blocks.iter().position(|b| b.name == *block && b.kind == BlockKind::Instrument) {
                Some(i) => {
                    scripts.insert(*num, (p, i));
                }
                None => warnings.push(format!("instrument {num:02X}: {path}: no instrument '{block}'")),
            },
        }
    }
    Ok((LoadedSong { song, scripts }, warnings))
}

/// Compiles a program and loads every song it names.
pub fn load_program(src: &str, read: Read) -> Result<Program, CompileError> {
    let mut prog = compile(src)?;
    let mut songs = Vec::with_capacity(prog.song_paths.len());
    for r in &prog.song_paths {
        let fail = |e: String| CompileError::new(r.line, r.col, format!("song \"{}\": {e}", r.path));
        let text = read(&r.path).map_err(fail)?;
        let (song, _warnings) = load_song(&text, read).map_err(|e| fail(e.to_string()))?;
        songs.push(Some(Arc::new(song)));
    }
    prog.songs = songs;
    Ok(prog)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;

    const SONG: &str = "acid-track 1\ntitle t\nspeed 6\nsfx-donor 4\ninstrument 01 \"A\"  script \"Home/good.snd\" bass\ninstrument 02 \"B\"  script \"Home/bad.snd\" bass\ninstrument 03 \"C\"  script \"Home/missing.snd\" bass\ninstrument 04 \"D\"  script \"Home/good.snd\" zap\norder 1  00 loop 0\norder 2  00 loop 0\norder 3  00 loop 0\norder 4  00 loop 0\n\npattern 00 1\n... .. . .. ...\n";

    fn files(path: &str) -> Result<String, String> {
        match path {
            "Home/good.snd" => Ok("instrument bass\ngate on\nend\nsound zap\ngate on\nend".into()),
            "Home/bad.snd" => Ok("wav saw".into()),
            "Home/s.trk" => Ok(SONG.into()),
            _ => Err("not found".into()),
        }
    }

    #[test]
    fn script_instruments_load_or_warn() {
        let (song, warnings) = load_song(SONG, &files).unwrap();
        assert_eq!(
            warnings,
            [
                "instrument 02: Home/bad.snd: 1:1 unknown command 'wav'",
                "instrument 03: Home/missing.snd: not found",
                "instrument 04: Home/good.snd: no instrument 'zap'",
            ]
        );
        assert_eq!(song.scripts.keys().copied().collect::<Vec<u8>>(), [1]);
        assert_eq!(song.scripts[&1].1, 0, "block 0 is 'bass'");
    }

    #[test]
    fn a_bad_song_file_is_an_error_not_a_warning() {
        assert_eq!(load_song("nope", &files).unwrap_err().to_string(), "1: not an acid-track file");
    }

    #[test]
    fn programs_load_the_songs_they_name() {
        let p = load_program("song \"Home/s.trk\"\nplay", &files).unwrap();
        assert_eq!(p.songs.len(), 1);
        assert!(p.songs[0].is_some());
    }

    #[test]
    fn a_song_that_wont_load_points_at_its_line() {
        assert_eq!(load_program("wait 1\nsong \"Home/x.trk\"", &files).unwrap_err().to_string(), "2:6 song \"Home/x.trk\": not found");
        assert_eq!(
            load_program("song \"Home/bad.snd\"", &files).unwrap_err().to_string(),
            "1:6 song \"Home/bad.snd\": 1: not an acid-track file"
        );
    }
}
