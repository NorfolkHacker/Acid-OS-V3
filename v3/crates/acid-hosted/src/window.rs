//! The host window: 1:1 pixels, not resizable by the user. It opens on the
//! screen-size picker (or straight at a size given on the command line),
//! then resizes to the chosen size and boots the OS there. Mouse = touch,
//! keys go through keymap::translate_key.

use std::any::Any;
use std::num::NonZeroU32;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use acid_gfx::{Canvas, rgb565_to_888};
use acid_kernel::layout::Screen;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, MouseButton, StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

use crate::keymap::translate_key;
use crate::picker::{PICKER_SCREEN, Picker, PickerKey};
use crate::{HostedPlatform, UserEvent};

/// Boots the OS at the chosen size. What it returns is kept for the rest
/// of the run (the audio output must outlive the closure).
pub type BootFn = Box<dyn FnOnce(Screen) -> Box<dyn Any>>;

/// How often the countdown redraws.
const PICKER_TICK: Duration = Duration::from_millis(100);

enum Stage {
    Picking { picker: Picker, canvas: Canvas, last_tick: Instant, cursor: (i32, i32) },
    Running,
}

struct App {
    platform: Arc<HostedPlatform>,
    window: Option<Rc<Window>>,
    surface: Option<softbuffer::Surface<Rc<Window>, Rc<Window>>>,
    shift: bool,
    /// The surface's size: the picker's until a size is chosen.
    size: Screen,
    stage: Stage,
    boot: Option<BootFn>,
    keep_alive: Option<Box<dyn Any>>,
    warned_size: bool,
    /// The last wake-up was the picker's timer, so the countdown redraws.
    timer_due: bool,
}

impl App {
    fn choose(&mut self, s: Screen) {
        let size = PhysicalSize::new(s.w as u32, s.h as u32);
        if let Some(w) = &self.window {
            // Min = max = the size: a hint to window managers that would
            // otherwise ignore a resize of a non-resizable window.
            w.set_min_inner_size(Some(size));
            w.set_max_inner_size(Some(size));
            let _ = w.request_inner_size(size);
        }
        if let Some(surface) = &mut self.surface {
            surface
                .resize(NonZeroU32::new(s.w as u32).unwrap(), NonZeroU32::new(s.h as u32).unwrap())
                .expect("size surface");
        }
        self.size = s;
        // The picker's pointer position carries over, so a click without
        // moving the mouse lands where the pointer is.
        if let Stage::Picking { cursor, .. } = &self.stage {
            self.platform.input.set_position(cursor.0, cursor.1);
        }
        self.stage = Stage::Running;
        if let Some(boot) = self.boot.take() {
            self.keep_alive = Some(boot(s));
        }
    }

    fn request_redraw(&self) {
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }
}

impl ApplicationHandler<UserEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("Acid OS v3")
            .with_inner_size(PhysicalSize::new(self.size.w as u32, self.size.h as u32))
            .with_resizable(false);
        let window = Rc::new(event_loop.create_window(attrs).expect("create window"));
        let context = softbuffer::Context::new(window.clone()).expect("softbuffer context");
        let mut surface = softbuffer::Surface::new(&context, window.clone()).expect("softbuffer surface");
        surface
            .resize(NonZeroU32::new(self.size.w as u32).unwrap(), NonZeroU32::new(self.size.h as u32).unwrap())
            .expect("size surface");
        // A frame presented before the window existed must still show.
        window.request_redraw();
        self.window = Some(window);
        self.surface = Some(surface);
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, _event: UserEvent) {
        self.request_redraw();
    }

    fn new_events(&mut self, _event_loop: &ActiveEventLoop, cause: StartCause) {
        self.timer_due = matches!(cause, StartCause::ResumeTimeReached { .. });
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let mut chosen = None;
        match &mut self.stage {
            Stage::Picking { picker, last_tick, .. } => {
                // Advance by only the whole milliseconds counted, so the
                // remainder carries into the next tick.
                let ms = last_tick.elapsed().as_millis() as u64;
                *last_tick += Duration::from_millis(ms);
                match picker.tick(ms as u32) {
                    Some(s) => chosen = Some(s),
                    None => {
                        // Redraw on the timer, not every loop: a pending
                        // redraw would override WaitUntil and spin.
                        if self.timer_due {
                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                        }
                        // Once stopped nothing animates; input requests its own redraws.
                        event_loop.set_control_flow(if picker.counting() {
                            ControlFlow::WaitUntil(*last_tick + PICKER_TICK)
                        } else {
                            ControlFlow::Wait
                        });
                    }
                }
            }
            Stage::Running => event_loop.set_control_flow(ControlFlow::Wait),
        }
        self.timer_due = false;
        if let Some(s) = chosen {
            self.choose(s);
        }
    }

    fn window_event(&mut self, _event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                self.platform.input.request_quit();
                // App threads are parked in their VMs, so the process
                // just ends.
                std::process::exit(0);
            }
            WindowEvent::ModifiersChanged(m) => self.shift = m.state().shift_key(),
            WindowEvent::RedrawRequested => {
                if let Stage::Picking { picker, canvas, .. } = &mut self.stage {
                    picker.draw(canvas);
                    if let Some(surface) = &mut self.surface {
                        if let Ok(mut buf) = surface.buffer_mut() {
                            if buf.len() == canvas.pixels().len() {
                                for (d, &p) in buf.iter_mut().zip(canvas.pixels()) {
                                    *d = rgb565_to_888(p);
                                }
                                let _ = buf.present();
                            }
                        }
                    }
                    return;
                }
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
            WindowEvent::Resized(new) => {
                if matches!(self.stage, Stage::Running)
                    && (new.width, new.height) != (self.size.w as u32, self.size.h as u32)
                    && !self.warned_size
                {
                    eprintln!(
                        "Acid OS v3: the window manager kept the window at {}x{}, not {}x{}; the screen may stay blank",
                        new.width, new.height, self.size.w, self.size.h
                    );
                    self.warned_size = true;
                }
            }
            event => {
                if matches!(self.stage, Stage::Picking { .. }) {
                    self.picking_event(event);
                } else {
                    self.running_event(event);
                }
            }
        }
    }
}

impl App {
    fn picking_event(&mut self, event: WindowEvent) {
        let Stage::Picking { picker, cursor, .. } = &mut self.stage else { return };
        let mut chosen = None;
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                *cursor = (position.x as i32, position.y as i32);
                picker.hover(cursor.0, cursor.1);
            }
            WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Left, .. } => {
                chosen = picker.click(cursor.0, cursor.1);
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed && !event.repeat {
                    let key = match event.physical_key {
                        PhysicalKey::Code(KeyCode::ArrowUp) => PickerKey::Up,
                        PhysicalKey::Code(KeyCode::ArrowDown) => PickerKey::Down,
                        PhysicalKey::Code(KeyCode::Enter | KeyCode::NumpadEnter) => PickerKey::Enter,
                        _ => PickerKey::Other,
                    };
                    chosen = picker.key(key);
                }
            }
            _ => return,
        }
        match chosen {
            Some(s) => self.choose(s),
            None => self.request_redraw(),
        }
    }

    fn running_event(&mut self, event: WindowEvent) {
        let input = &self.platform.input;
        match event {
            WindowEvent::CursorMoved { position, .. } => input.set_position(position.x as i32, position.y as i32),
            WindowEvent::MouseInput { state, button: MouseButton::Left, .. } => {
                input.set_pressed(state == ElementState::Pressed)
            }
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

pub fn run_window(
    event_loop: EventLoop<UserEvent>,
    platform: Arc<HostedPlatform>,
    preselected: Option<Screen>,
    boot: BootFn,
) {
    let mut app = match preselected {
        Some(s) => {
            // Boot before the window exists, as the kernel always did.
            let keep_alive = Some(boot(s));
            App {
                platform,
                window: None,
                surface: None,
                shift: false,
                size: s,
                stage: Stage::Running,
                boot: None,
                keep_alive,
                warned_size: false,
                timer_due: false,
            }
        }
        None => App {
            platform,
            window: None,
            surface: None,
            shift: false,
            size: PICKER_SCREEN,
            stage: Stage::Picking {
                picker: Picker::new(),
                canvas: Canvas::new(PICKER_SCREEN.w, PICKER_SCREEN.h),
                last_tick: Instant::now(),
                cursor: (0, 0),
            },
            boot: Some(boot),
            keep_alive: None,
            warned_size: false,
            timer_due: false,
        },
    };
    event_loop.run_app(&mut app).expect("event loop");
}
