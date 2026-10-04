//! Acid OS v3, hosted. Run from the repo root:
//!   cargo run --manifest-path v3/Cargo.toml -p acid-os
//! Add `-- --screen 800x600` to skip the screen-size picker.

use acid_hosted::window::{BootFn, run_window};
use acid_hosted::{HostedPlatform, UserEvent};
use winit::event_loop::EventLoop;

fn main() {
    let preselected = match acid_os::screen_arg(std::env::args().skip(1)) {
        Ok(s) => s,
        Err(msg) => {
            eprintln!("Acid OS v3: {msg}");
            std::process::exit(2);
        }
    };
    let event_loop = EventLoop::<UserEvent>::with_user_event().build().expect("event loop");
    let platform = HostedPlatform::new(".");
    platform.set_proxy(event_loop.create_proxy());
    let boot_platform = platform.clone();
    // Nothing kernel-side exists until a screen size is chosen.
    let boot: BootFn = Box::new(move |screen| {
        let kernel = acid_os::boot_with(boot_platform, screen);
        // Development launcher: `-- --app tetris` opens an app from its
        // manifest, the way Terminal's `run` does (spec 10.5).
        if let Some(name) = acid_os::app_arg(std::env::args().skip(1)) {
            match acid_os::spawn_from_manifest(&kernel, &name) {
                Some(task) => kernel.activate_window(task),
                None => eprintln!("Acid OS v3: --app {name}: no such app"),
            }
        }
        // Sound: the device's callback pulls straight from the kernel synth.
        // The handle is !Send; the window keeps it for the rest of the run.
        let audio_kernel = kernel.clone();
        let audio = acid_hosted::audio::start_output(std::sync::Arc::new(move |buf: &mut [u8]| {
            audio_kernel.render_audio(buf)
        }));
        std::thread::Builder::new()
            .name("router".into())
            .spawn(move || kernel.run_router())
            .expect("router thread");
        Box::new(audio)
    });
    run_window(event_loop, platform, preselected, boot);
}
