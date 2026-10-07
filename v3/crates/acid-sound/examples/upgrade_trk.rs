//! Prints an acid-track file in the current (eight-track) form, so an old
//! four-channel song can be converted by hand:
//!   cargo run --manifest-path v3/Cargo.toml -p acid-sound --example upgrade_trk -- OLD.trk > NEW.trk

fn main() {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: upgrade_trk FILE.trk");
        std::process::exit(2);
    };
    let text = std::fs::read_to_string(&path).expect("read the song");
    match acid_sound::song::parse(&text) {
        Ok(song) => print!("{}", acid_sound::song::write(&song)),
        Err(e) => {
            eprintln!("{path}: {e}");
            std::process::exit(1);
        }
    }
}
