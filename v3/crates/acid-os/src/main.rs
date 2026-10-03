//! Acid OS v3, hosted. Run from the repo root:
//!   cargo run --manifest-path v3/Cargo.toml -p acid-os

use acid_hosted::{HostedPlatform, UserEvent, window::run_window};
use winit::event_loop::EventLoop;

fn main() {
    let event_loop = EventLoop::<UserEvent>::with_user_event().build().expect("event loop");
    let platform = HostedPlatform::new(".");
    platform.set_proxy(event_loop.create_proxy());
    let kernel = acid_os::boot(platform.clone());
    // Development launcher: `-- --app tetris` opens an app from its
    // manifest, the way Terminal's `run` does (spec 10.5).
    if let Some(name) = acid_os::app_arg(std::env::args().skip(1)) {
        match acid_os::spawn_from_manifest(&kernel, &name) {
            Some(task) => kernel.activate_window(task),
            None => eprintln!("Acid OS v3: --app {name}: no such app"),
        }
    }
    // Sound: the device's callback pulls straight from the kernel synth.
    // The returned handle must live as long as the process (it is !Send,
    // so it stays here on main).
    let audio_kernel = kernel.clone();
    let _audio = acid_hosted::audio::start_output(std::sync::Arc::new(move |buf: &mut [u8]| {
        audio_kernel.render_audio(buf)
    }));
    std::thread::Builder::new()
        .name("router".into())
        .spawn(move || kernel.run_router())
        .expect("router thread");
    run_window(event_loop, platform);
}
