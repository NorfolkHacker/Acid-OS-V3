//! Window chrome: a THEME_PANEL title bar with the title at (4, 4) and the close
//! dot at (w - 8, 8), a 1 px THEME_HARD border, and r = 3 rounded corners.

use acid_gfx::Canvas;
use acid_kernel::layout::{CLOSE_BTN_MARGIN, CLOSE_BTN_R, TITLE_BAR_H};
use acid_kernel::theme::{THEME_BG, THEME_HARD, THEME_PANEL, THEME_TEXT};

const CORNER_RADIUS: i32 = 3;

pub(crate) fn draw_window_frame(c: &mut Canvas, w: i32, title: &str) {
    c.fill_rect(0, 0, w, TITLE_BAR_H, THEME_PANEL);
    c.draw_text(4, (TITLE_BAR_H - 8) / 2, title, THEME_TEXT, THEME_PANEL);
    c.fill_circle(w - CLOSE_BTN_MARGIN, TITLE_BAR_H / 2, CLOSE_BTN_R, THEME_HARD);
}

pub(crate) fn draw_window_border(c: &mut Canvas, w: i32, h: i32) {
    c.fill_rect(0, 0, w, 1, THEME_HARD);
    c.fill_rect(0, h - 1, w, 1, THEME_HARD);
    c.fill_rect(0, 0, 1, h, THEME_HARD);
    c.fill_rect(w - 1, 0, 1, h, THEME_HARD);
    draw_rounded_corners(c, w, h);
}

pub(crate) fn clear_user_area(c: &mut Canvas, w: i32, h: i32) {
    c.fill_rect(0, TITLE_BAR_H, w, h - TITLE_BAR_H, THEME_BG);
}

fn draw_corner(c: &mut Canvas, r: i32, corner_x: i32, corner_y: i32, flip_x: bool, flip_y: bool, bg: u32) {
    for i in 0..r {
        let bg_width = r - 1 - i;
        let y = if flip_y { corner_y - i } else { corner_y + i };
        if bg_width > 0 {
            let bg_x = if flip_x { corner_x - bg_width + 1 } else { corner_x };
            c.fill_rect(bg_x, y, bg_width, 1, bg);
        }
        let px = if flip_x { corner_x - bg_width } else { corner_x + bg_width };
        c.fill_rect(px, y, 1, 1, THEME_HARD);
    }
}

fn draw_rounded_corners(c: &mut Canvas, w: i32, h: i32) {
    let mut r = CORNER_RADIUS;
    if r * 2 > w || r * 2 > h {
        r = w.min(h) / 2;
    }
    // Top corners sit on the title bar (PANEL); bottom ones on the body (BG).
    draw_corner(c, r, 0, 0, false, false, THEME_PANEL);
    draw_corner(c, r, w - 1, 0, true, false, THEME_PANEL);
    draw_corner(c, r, 0, h - 1, false, true, THEME_BG);
    draw_corner(c, r, w - 1, h - 1, true, true, THEME_BG);
}
