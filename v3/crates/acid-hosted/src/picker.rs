//! The startup screen-size picker: the three preset sizes, a countdown to
//! the default, and keys or the mouse to choose. Pure logic and drawing
//! into a Canvas; window.rs feeds it events and shows its canvas.

use acid_gfx::Canvas;
use acid_kernel::layout::Screen;
use acid_kernel::theme::{THEME_BG, THEME_HARD, THEME_MUTED, THEME_PANEL, THEME_TEXT};

/// How long the picker waits before booting the default.
pub const COUNTDOWN_MS: u32 = 3000;
/// The window's size while picking.
pub const PICKER_SCREEN: Screen = Screen::DEFAULT;

/// The system font's glyph width (acid-gfx's 6 × 8 font).
const CHAR_W: i32 = 6;
const ROW_X: i32 = 220;
const ROW_W: i32 = 200;
const ROW_Y: i32 = 180;
const ROW_H: i32 = 28;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickerKey {
    Up,
    Down,
    Enter,
    /// Any other key: it only stops the countdown.
    Other,
}

pub struct Picker {
    selected: usize,
    remaining_ms: u32,
    counting: bool,
}

impl Default for Picker {
    fn default() -> Self {
        Self::new()
    }
}

impl Picker {
    pub fn new() -> Picker {
        Picker { selected: 0, remaining_ms: COUNTDOWN_MS, counting: true }
    }

    /// Advances the countdown; the default once it runs out.
    pub fn tick(&mut self, elapsed_ms: u32) -> Option<Screen> {
        if !self.counting {
            return None;
        }
        self.remaining_ms = self.remaining_ms.saturating_sub(elapsed_ms);
        (self.remaining_ms == 0).then_some(Screen::DEFAULT)
    }

    pub fn key(&mut self, k: PickerKey) -> Option<Screen> {
        self.counting = false;
        match k {
            PickerKey::Up => self.selected = self.selected.saturating_sub(1),
            PickerKey::Down => self.selected = (self.selected + 1).min(Screen::PRESETS.len() - 1),
            PickerKey::Enter => return Some(Screen::PRESETS[self.selected]),
            PickerKey::Other => {}
        }
        None
    }

    pub fn click(&mut self, x: i32, y: i32) -> Option<Screen> {
        self.counting = false;
        let i = row_at(x, y)?;
        self.selected = i;
        Some(Screen::PRESETS[i])
    }

    pub fn hover(&mut self, x: i32, y: i32) {
        if let Some(i) = row_at(x, y) {
            self.counting = false;
            self.selected = i;
        }
    }

    pub fn draw(&self, c: &mut Canvas) {
        c.fill_rect(0, 0, c.width(), c.height(), THEME_BG);
        centred(c, 140, "Acid OS v3 - screen size", THEME_HARD, THEME_BG);
        for (i, s) in Screen::PRESETS.iter().enumerate() {
            let y = ROW_Y + i as i32 * ROW_H;
            let (fg, bg) = if i == self.selected { (THEME_HARD, THEME_PANEL) } else { (THEME_TEXT, THEME_BG) };
            c.fill_rect(ROW_X, y, ROW_W, ROW_H, bg);
            let label = if *s == Screen::DEFAULT { format!("{}x{} (default)", s.w, s.h) } else { format!("{}x{}", s.w, s.h) };
            centred(c, y + (ROW_H - 8) / 2, &label, fg, bg);
        }
        let footer = if self.counting {
            let d = Screen::DEFAULT;
            format!("starting {}x{} in {}", d.w, d.h, self.remaining_ms.div_ceil(1000))
        } else {
            "Enter to start".to_string()
        };
        centred(c, ROW_Y + 3 * ROW_H + 20, &footer, THEME_MUTED, THEME_BG);
    }
}

/// The preset row under (x, y), if any.
fn row_at(x: i32, y: i32) -> Option<usize> {
    if !(ROW_X..ROW_X + ROW_W).contains(&x) || y < ROW_Y {
        return None;
    }
    let i = ((y - ROW_Y) / ROW_H) as usize;
    (i < Screen::PRESETS.len()).then_some(i)
}

/// Text centred across the canvas at row `y`.
fn centred(c: &mut Canvas, y: i32, text: &str, fg: u32, bg: u32) {
    let x = (c.width() - text.len() as i32 * CHAR_W) / 2;
    c.draw_text(x, y, text, fg, bg);
}

#[cfg(test)]
mod tests {
    use super::*;
    use acid_gfx::rgb565;

    fn row_centre(i: usize) -> (i32, i32) {
        (ROW_X + ROW_W / 2, ROW_Y + i as i32 * ROW_H + ROW_H / 2)
    }

    #[test]
    fn the_countdown_boots_the_default() {
        let mut p = Picker::new();
        assert_eq!(p.tick(COUNTDOWN_MS - 1), None);
        assert_eq!(p.tick(1), Some(Screen::DEFAULT));
    }

    #[test]
    fn a_key_stops_the_countdown() {
        let mut p = Picker::new();
        assert_eq!(p.key(PickerKey::Other), None);
        assert_eq!(p.tick(COUNTDOWN_MS * 10), None, "no timeout once stopped");
    }

    #[test]
    fn arrows_move_and_clamp_and_enter_picks() {
        let mut p = Picker::new();
        p.key(PickerKey::Up);
        assert_eq!(p.key(PickerKey::Enter), Some(Screen::PRESETS[0]), "Up at the top stays");
        let mut p = Picker::new();
        for _ in 0..5 {
            p.key(PickerKey::Down);
        }
        assert_eq!(p.key(PickerKey::Enter), Some(Screen::PRESETS[2]), "Down at the bottom stays");
    }

    #[test]
    fn a_click_picks_the_row_under_it() {
        let mut p = Picker::new();
        let (x, y) = row_centre(1);
        assert_eq!(p.click(x, y), Some(Screen::WIDE));
        let mut p = Picker::new();
        assert_eq!(p.click(5, 5), None, "outside the rows");
        assert_eq!(p.tick(COUNTDOWN_MS), None, "any click stops the countdown");
    }

    #[test]
    fn hover_highlights_and_stops_the_countdown() {
        let mut p = Picker::new();
        let (x, y) = row_centre(2);
        p.hover(x, y);
        assert_eq!(p.tick(COUNTDOWN_MS), None);
        assert_eq!(p.key(PickerKey::Enter), Some(Screen::SVGA));
        let mut p = Picker::new();
        p.hover(5, 5);
        assert_eq!(p.tick(COUNTDOWN_MS), Some(Screen::DEFAULT), "hovering off the rows changes nothing");
    }

    #[test]
    fn draw_highlights_the_selected_row() {
        let p = Picker::new();
        let mut c = Canvas::new(PICKER_SCREEN.w, PICKER_SCREEN.h);
        p.draw(&mut c);
        let (x, y) = (ROW_X + 1, ROW_Y + 1);
        assert_eq!(c.pixel(x, y), Some(rgb565(THEME_PANEL)), "row 0 is selected");
        assert_eq!(c.pixel(x, y + ROW_H), Some(rgb565(THEME_BG)), "row 1 is not");
    }
}
