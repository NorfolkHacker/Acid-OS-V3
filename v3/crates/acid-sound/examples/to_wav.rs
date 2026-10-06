//! Writes a .u8 recording (22050 Hz unsigned 8-bit mono) as a .wav, so a
//! golden can be listened to before it's blessed:
//!   cargo run --manifest-path v3/Cargo.toml -p acid-sound --example to_wav -- IN.u8 OUT.wav

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let [_, input, output] = args.as_slice() else {
        eprintln!("usage: to_wav IN.u8 OUT.wav");
        std::process::exit(2);
    };
    let data = std::fs::read(input).expect("read input");
    let rate: u32 = 22050;
    let mut w = Vec::with_capacity(44 + data.len());
    w.extend_from_slice(b"RIFF");
    w.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
    w.extend_from_slice(b"WAVEfmt ");
    w.extend_from_slice(&16u32.to_le_bytes());
    w.extend_from_slice(&1u16.to_le_bytes()); // PCM
    w.extend_from_slice(&1u16.to_le_bytes()); // mono
    w.extend_from_slice(&rate.to_le_bytes());
    w.extend_from_slice(&rate.to_le_bytes()); // bytes per second
    w.extend_from_slice(&1u16.to_le_bytes()); // block align
    w.extend_from_slice(&8u16.to_le_bytes()); // bits
    w.extend_from_slice(b"data");
    w.extend_from_slice(&(data.len() as u32).to_le_bytes());
    w.extend_from_slice(&data);
    std::fs::write(output, w).expect("write output");
}
