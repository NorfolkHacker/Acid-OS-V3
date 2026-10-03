//! Hear the synth: a two-second pulse arpeggio, then release.
//!   cargo run --manifest-path v3/Cargo.toml -p acid-hosted --example beep

use std::sync::{Arc, Mutex};
use std::time::Duration;

use acid_hosted::audio::start_output;
use acid_synth::Synth;

fn main() {
    let synth = Arc::new(Mutex::new(Synth::new()));
    {
        let mut s = synth.lock().unwrap();
        for (slot, note) in [40, 44, 47, 52].iter().enumerate() {
            s.set_arp_note(0, slot as i32, *note);
        }
        s.set_arp_rate(0, 90);
        s.arp_on(0, 4);
        s.set_duty(0, 30);
        s.set_adsr(0, 5, 80, 60, 400);
        s.gate_on(0);
    }
    let src = synth.clone();
    let Some(_out) = start_output(Arc::new(move |buf: &mut [u8]| src.lock().unwrap().render(buf))) else {
        eprintln!("beep: no audio output available");
        std::process::exit(1);
    };
    std::thread::sleep(Duration::from_millis(2000));
    synth.lock().unwrap().gate_off(0);
    std::thread::sleep(Duration::from_millis(600));
}
