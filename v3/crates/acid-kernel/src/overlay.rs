//! The kernel overlay: one full-screen canvas that a single app at a time
//! can draw effects on, above every window (the terminal's easter eggs fly
//! across it). Pixels left at ACID_OVERLAY_KEY are transparent when
//! composited, because this codebase has no alpha.

use acid_gfx::Canvas;

use crate::TaskId;
use crate::kernel::Kernel;
use crate::layout::Screen;
use crate::theme::ACID_OVERLAY_KEY;

pub struct Overlay {
    canvas: Option<Canvas>,
    owner: Option<TaskId>,
    screen: Screen,
}

impl Overlay {
    pub fn new(screen: Screen) -> Self {
        Self { canvas: None, owner: None, screen }
    }

    /// Opens (or, for the current owner, re-opens and clears) the overlay.
    /// False if another task holds it.
    pub fn open(&mut self, owner: TaskId) -> bool {
        if self.owner.is_some_and(|o| o != owner) {
            return false;
        }
        // Allocated on first use: nobody pays for a screen-sized canvas until an app wants it.
        let s = self.screen;
        let c = self.canvas.get_or_insert_with(|| Canvas::new(s.w, s.h));
        c.fill_rect(0, 0, s.w, s.h, ACID_OVERLAY_KEY);
        self.owner = Some(owner);
        true
    }

    /// Closes the overlay if `owner` holds it, and returns whether it did.
    /// This is also the release-on-exit path.
    pub fn close(&mut self, owner: TaskId) -> bool {
        if self.owner == Some(owner) {
            self.owner = None;
            true
        } else {
            false
        }
    }

    pub fn is_open(&self) -> bool {
        self.owner.is_some()
    }

    /// The canvas to composite, if the overlay is open.
    pub fn canvas(&self) -> Option<&Canvas> {
        if self.is_open() { self.canvas.as_ref() } else { None }
    }

    /// The owner's drawing surface. None for anyone else, or when closed:
    /// those draws silently do nothing.
    pub fn canvas_for(&mut self, owner: TaskId) -> Option<&mut Canvas> {
        if self.owner == Some(owner) { self.canvas.as_mut() } else { None }
    }
}

impl Kernel {
    pub fn overlay_open(&self, task: TaskId) -> bool {
        let ok = self.overlay.lock().open(task);
        if ok {
            self.mark_dirty();
        }
        ok
    }

    pub fn overlay_clear(&self, task: TaskId) {
        if let Some(c) = self.overlay.lock().canvas_for(task) {
            c.fill_rect(0, 0, self.screen().w, self.screen().h, ACID_OVERLAY_KEY);
            self.mark_dirty();
        }
    }

    /// Canvas::fill_rect already clips to the screen-sized canvas, so no
    /// explicit clamp is needed here.
    pub fn overlay_fill_rect(&self, task: TaskId, x: i32, y: i32, w: i32, h: i32, color: u32) {
        if let Some(c) = self.overlay.lock().canvas_for(task) {
            c.fill_rect(x, y, w, h, color);
            self.mark_dirty();
        }
    }

    pub fn overlay_close(&self, task: TaskId) {
        if self.overlay.lock().close(task) {
            self.mark_dirty();
        }
    }

    pub fn overlay_is_open(&self) -> bool {
        self.overlay.lock().is_open()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::*;
    use acid_gfx::rgb565;

    const KEY565: u16 = rgb565(ACID_OVERLAY_KEY);

    #[test]
    fn one_owner_at_a_time() {
        let mut o = Overlay::new(Screen::WIDE);
        assert!(!o.is_open());
        assert!(o.open(TaskId(1)));
        assert!(!o.open(TaskId(2)), "held by 1");
        assert!(o.open(TaskId(1)), "the owner may re-open");
        assert!(!o.close(TaskId(2)), "only the owner closes");
        assert!(o.is_open());
        assert!(o.close(TaskId(1)));
        assert!(!o.is_open());
        assert!(o.open(TaskId(2)));
    }

    #[test]
    fn open_fills_with_the_key_and_only_the_owner_draws() {
        let mut o = Overlay::new(Screen::WIDE);
        o.open(TaskId(1));
        assert_eq!(o.canvas().unwrap().pixel(0, 0), Some(KEY565));
        assert!(o.canvas_for(TaskId(2)).is_none());
        o.canvas_for(TaskId(1)).unwrap().fill_rect(0, 0, 1, 1, 0xFFFFFF);
        assert_eq!(o.canvas().unwrap().pixel(0, 0), Some(0xFFFF));
    }

    #[test]
    fn reopen_clears_previous_drawing() {
        let mut o = Overlay::new(Screen::WIDE);
        o.open(TaskId(1));
        o.canvas_for(TaskId(1)).unwrap().fill_rect(0, 0, 1, 1, 0xFFFFFF);
        o.open(TaskId(1));
        assert_eq!(o.canvas().unwrap().pixel(0, 0), Some(KEY565));
    }

    #[test]
    fn a_closed_overlay_is_not_composited() {
        let mut o = Overlay::new(Screen::WIDE);
        o.open(TaskId(1));
        o.close(TaskId(1));
        assert!(o.canvas().is_none());
        assert!(o.canvas_for(TaskId(1)).is_none());
    }

    #[test]
    fn overlay_composites_above_windows_with_the_key_transparent() {
        let (p, k, rx) = setup();
        let t = k.spawn_app(req(0, 30, 20, 20)).unwrap();
        recv(&rx).canvas.lock().fill_rect(0, 0, 20, 20, 0xFF0000);
        assert!(k.overlay_open(t));
        k.overlay_fill_rect(t, 5, 35, 4, 4, 0x00FF00);
        k.composite_frame();
        let f = p.display.last_frame().unwrap();
        let at = |x: i32, y: i32| f[(y * 640 + x) as usize];
        assert_eq!(at(6, 36), rgb565(0x00FF00), "overlay pixel on top");
        assert_eq!(at(1, 31), rgb565(0xFF0000), "key pixels show the window");
    }

    #[test]
    fn overlay_fill_rect_clips_to_the_screen() {
        let (p, k, rx) = setup();
        let t = k.spawn_app(req(0, 30, 10, 10)).unwrap();
        recv(&rx);
        k.overlay_open(t);
        k.overlay_fill_rect(t, -10, 350, 40, 40, 0x00E5FF);
        k.composite_frame();
        let f = p.display.last_frame().unwrap();
        assert_eq!(f[(359 * 640) as usize], rgb565(0x00E5FF));
        assert_eq!(f[(355 * 640 + 29) as usize], rgb565(0x00E5FF));
    }

    #[test]
    fn only_the_owner_draws_and_exit_releases() {
        let (p, k, rx) = setup();
        let t1 = k.spawn_app(req(0, 30, 10, 10)).unwrap();
        recv(&rx);
        let t2 = k.spawn_app(req(50, 30, 10, 10)).unwrap();
        recv(&rx);
        assert!(k.overlay_open(t1));
        assert!(!k.overlay_open(t2));
        k.overlay_fill_rect(t2, 300, 300, 5, 5, 0xFFFFFF);
        k.overlay_clear(t2);
        k.composite_frame();
        let wall = p.display.last_frame().unwrap()[(301 * 640 + 301) as usize];
        let want = acid_gfx::wallpaper::wallpaper_canvas().pixel(301, 301).unwrap();
        assert_eq!(wall, want, "a non-owner's draw is ignored");
        k.close_window(t1);
        wait_until(|| !k.overlay_is_open());
        assert!(k.overlay_open(t2));
    }

    #[test]
    fn overlay_changes_mark_dirty() {
        let (_p, k, rx) = setup();
        let t = k.spawn_app(req(0, 30, 10, 10)).unwrap();
        recv(&rx);
        k.take_dirty();
        k.overlay_open(t);
        assert!(k.take_dirty());
        k.overlay_fill_rect(t, 0, 0, 1, 1, 0);
        assert!(k.take_dirty());
        k.overlay_clear(t);
        assert!(k.take_dirty());
        k.overlay_close(t);
        assert!(k.take_dirty());
        k.overlay_close(t);
        assert!(!k.take_dirty(), "closing a closed overlay changes nothing");
    }

    #[test]
    fn the_overlay_covers_a_bigger_screen() {
        let (p, k, rx) = setup_at(crate::layout::Screen::SVGA);
        let t = k.spawn_app(req(0, 30, 10, 10)).unwrap();
        recv(&rx);
        assert!(k.overlay_open(t));
        k.overlay_fill_rect(t, 790, 590, 20, 20, 0x00E5FF);
        k.composite_frame();
        let f = p.display.last_frame().unwrap();
        assert_eq!(f[(599 * 800 + 799) as usize], rgb565(0x00E5FF));
    }
}
