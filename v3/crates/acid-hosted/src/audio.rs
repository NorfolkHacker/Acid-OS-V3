//! Sound output: pulls Acid OS's 22050 Hz unsigned 8-bit mono stream from
//! the kernel (Kernel::render_audio) and plays it on the default device
//! through cpal. cpal doesn't resample, so any other device rate gets
//! linear interpolation here. No device, or an unsupported format: one log
//! line and silence.

use std::sync::Arc;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream, StreamConfig};

pub const SOURCE_RATE: u32 = 22050;

pub type RenderFn = Arc<dyn Fn(&mut [u8]) + Send + Sync>;

/// Source samples fetched per render call.
const CHUNK: usize = 256;

pub struct Resampler {
    step: f64,
    pos: f64,
    cur: f32,
    next: f32,
    primed: bool,
    chunk: Vec<u8>,
    idx: usize,
}

impl Resampler {
    pub fn new(out_rate: u32) -> Self {
        Self {
            step: SOURCE_RATE as f64 / out_rate as f64,
            pos: 0.0,
            cur: 0.0,
            next: 0.0,
            primed: false,
            chunk: vec![128; CHUNK],
            idx: CHUNK,
        }
    }

    fn pull(&mut self, render: &dyn Fn(&mut [u8])) -> f32 {
        if self.idx >= self.chunk.len() {
            render(&mut self.chunk);
            self.idx = 0;
        }
        let s = self.chunk[self.idx];
        self.idx += 1;
        (s as f32 - 128.0) / 128.0
    }

    /// The next output sample, -1.0..1.0.
    pub fn next_sample(&mut self, render: &dyn Fn(&mut [u8])) -> f32 {
        if !self.primed {
            self.cur = self.pull(render);
            self.next = self.pull(render);
            self.primed = true;
        }
        let out = self.cur + (self.next - self.cur) * self.pos as f32;
        self.pos += self.step;
        while self.pos >= 1.0 {
            self.pos -= 1.0;
            self.cur = self.next;
            self.next = self.pull(render);
        }
        out
    }
}

/// Keeps the stream playing; drop it to stop. Not Send (cpal's Stream
/// isn't), so it lives on the thread that created it -- main.
pub struct AudioOutput {
    _stream: Stream,
}

fn log_stream_error(e: cpal::StreamError) {
    eprintln!("Acid OS v3: audio: stream error: {e}");
}

pub fn start_output(render: RenderFn) -> Option<AudioOutput> {
    let host = cpal::default_host();
    let Some(device) = host.default_output_device() else {
        eprintln!("Acid OS v3: audio: no output device; running silently");
        return None;
    };
    let supported = match device.default_output_config() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Acid OS v3: audio: no usable output config ({e}); running silently");
            return None;
        }
    };
    let format = supported.sample_format();
    let config: StreamConfig = supported.into();
    let channels = config.channels as usize;
    let rate = config.sample_rate.0;
    let stream = match format {
        SampleFormat::F32 => {
            let mut rs = Resampler::new(rate);
            device.build_output_stream(
                &config,
                move |data: &mut [f32], _| {
                    for frame in data.chunks_mut(channels) {
                        frame.fill(rs.next_sample(&*render));
                    }
                },
                log_stream_error,
                None,
            )
        }
        SampleFormat::I16 => {
            let mut rs = Resampler::new(rate);
            device.build_output_stream(
                &config,
                move |data: &mut [i16], _| {
                    for frame in data.chunks_mut(channels) {
                        frame.fill((rs.next_sample(&*render) * i16::MAX as f32) as i16);
                    }
                },
                log_stream_error,
                None,
            )
        }
        other => {
            eprintln!("Acid OS v3: audio: unsupported sample format {other:?}; running silently");
            return None;
        }
    };
    let stream = match stream {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Acid OS v3: audio: could not open stream ({e}); running silently");
            return None;
        }
    };
    if let Err(e) = stream.play() {
        eprintln!("Acid OS v3: audio: could not start stream ({e}); running silently");
        return None;
    }
    Some(AudioOutput { _stream: stream })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU8, Ordering};

    /// A source producing 0, 1, 2, ... (wrapping).
    fn counter() -> impl Fn(&mut [u8]) {
        let n = AtomicU8::new(0);
        move |buf: &mut [u8]| {
            for b in buf.iter_mut() {
                *b = n.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    fn f(s: u8) -> f32 {
        (s as f32 - 128.0) / 128.0
    }

    #[test]
    fn same_rate_is_identity() {
        let src = counter();
        let mut r = Resampler::new(SOURCE_RATE);
        for i in 0..600u32 {
            assert_eq!(r.next_sample(&src), f((i % 256) as u8));
        }
    }

    #[test]
    fn double_rate_interpolates_midpoints() {
        let src = counter();
        let mut r = Resampler::new(SOURCE_RATE * 2);
        let got: Vec<f32> = (0..6).map(|_| r.next_sample(&src)).collect();
        let want = [f(0), (f(0) + f(1)) / 2.0, f(1), (f(1) + f(2)) / 2.0, f(2), (f(2) + f(3)) / 2.0];
        for (g, w) in got.iter().zip(want) {
            assert!((g - w).abs() < 1e-6, "{got:?}");
        }
    }

    #[test]
    fn constant_input_stays_constant_at_any_rate() {
        let src = |buf: &mut [u8]| buf.fill(200);
        for rate in [8_000, 44_100, 48_000, 96_000] {
            let mut r = Resampler::new(rate);
            for _ in 0..1000 {
                assert!((r.next_sample(&src) - f(200)).abs() < 1e-6);
            }
        }
    }

    #[test]
    fn rate_48k_matches_the_closed_form_across_chunk_boundaries() {
        let src = counter();
        let mut r = Resampler::new(48_000);
        let step = SOURCE_RATE as f64 / 48_000.0;
        for k in 0..1200usize {
            let t = k as f64 * step;
            let i = t.floor() as usize;
            let frac = (t - i as f64) as f32;
            let a = f(((i) % 256) as u8);
            let b = f(((i + 1) % 256) as u8);
            let want = a + (b - a) * frac;
            let got = r.next_sample(&src);
            assert!((got - want).abs() < 1e-4, "k={k} got {got} want {want}");
        }
    }
}
