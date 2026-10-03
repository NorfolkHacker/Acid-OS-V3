//! The host window: 640x360, not resizable,
//! 1:1 pixels. Mouse = touch, keys go through keymap::translate_key.

use std::num::NonZeroU32;
use std::rc::Rc;
use std::sync::Arc;

use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::PhysicalKey;
use winit::window::{Window, WindowId};

use crate::keymap::translate_key;
use crate::{HostedPlatform, UserEvent};

const W: u32 = 640;
const H: u32 = 360;

struct App {
    platform: Arc<HostedPlatform>,
    window: Option<Rc<Window>>,
    surface: Option<softbuffer::Surface<Rc<Window>, Rc<Window>>>,
    shift: bool,
}

impl ApplicationHandler<UserEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("Acid OS v3")
            .with_inner_size(PhysicalSize::new(W, H))
            .with_resizable(false);
        let window = Rc::new(event_loop.create_window(attrs).expect("create window"));
        let context = softbuffer::Context::new(window.clone()).expect("softbuffer context");
        let mut surface = softbuffer::Surface::new(&context, window.clone()).expect("softbuffer surface");
        surface
            .resize(NonZeroU32::new(W).unwrap(), NonZeroU32::new(H).unwrap())
            .expect("size surface");
        // A frame presented before the window existed must still show.
        window.request_redraw();
        self.window = Some(window);
        self.surface = Some(surface);
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, _event: UserEvent) {
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    fn window_event(&mut self, _event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let input = &self.platform.input;
        match event {
            WindowEvent::CloseRequested => {
                input.request_quit();
                // App threads are parked in their VMs, so the process
                // just ends.
                std::process::exit(0);
            }
            WindowEvent::RedrawRequested => {
                let frame = self.platform.frame_snapshot();
                if let Some(surface) = &mut self.surface {
                    if let Ok(mut buf) = surface.buffer_mut() {
                        if buf.len() == frame.len() {
                            buf.copy_from_slice(&frame);
                            let _ = buf.present();
                        }
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => input.set_position(position.x as i32, position.y as i32),
            WindowEvent::MouseInput { state, button: MouseButton::Left, .. } => {
                input.set_pressed(state == ElementState::Pressed)
            }
            WindowEvent::ModifiersChanged(m) => self.shift = m.state().shift_key(),
            WindowEvent::KeyboardInput { event, .. } => {
                // Presses only, no auto-repeat: keys are edge-triggered.
                if event.state == ElementState::Pressed && !event.repeat {
                    if let PhysicalKey::Code(code) = event.physical_key {
                        if let Some(k) = translate_key(code, self.shift) {
                            input.push_key(k);
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

pub fn run_window(event_loop: EventLoop<UserEvent>, platform: Arc<HostedPlatform>) {
    let mut app = App { platform, window: None, surface: None, shift: false };
    event_loop.run_app(&mut app).expect("event loop");
}
