//! Text in LovyanGFX's 6x8 GLCD font0, drawn the way its GLCDfont::drawChar
//! does at text size 1: 5 glyph columns plus a 6th spacing column, with
//! the background painted only when fg != bg (compared as 24-bit colours,
//! before conversion -- LovyanGFX's own `fillbg` rule).

use crate::font_data::GLCD_FONT;
use crate::{Canvas, rgb565};

pub const CHAR_W: i32 = 6;
pub const CHAR_H: i32 = 8;

impl Canvas {
    pub fn draw_text(&mut self, x: i32, y: i32, text: &str, fg: u32, bg: u32) {
        let fill_bg = fg != bg;
        let (fg, bg) = (rgb565(fg), rgb565(bg));
        // A row wholly above or below the canvas draws nothing; glyphs wholly
        // left of it are skipped and drawing stops past its right edge, so
        // draw_glyph only ever sees coordinates near the canvas and its
        // i32 sums cannot overflow, whatever x and y are.
        if y >= self.h || y <= -CHAR_H {
            return;
        }
        let mut cx = x as i64;
        for ch in text.chars() {
            if cx >= self.w as i64 {
                break;
            }
            if cx > -(CHAR_W as i64) {
                self.draw_glyph(cx as i32, y, ch as u32, fg, bg, fill_bg);
            }
            cx += CHAR_W as i64;
        }
    }

    fn draw_glyph(&mut self, x: i32, y: i32, code: u32, fg: u16, bg: u16, fill_bg: bool) {
        // LovyanGFX range-checks the code against the font's 0..=255 span
        // BEFORE the shift (GLCDfont::drawChar, lgfx_fonts.cpp). Past it,
        // drawCharDummy paints the cell's background (only when fg != bg)
        // and ALWAYS outlines a 4x6 box inset by 1 in the fg colour.
        if code > 255 {
            if fill_bg {
                self.fill_rect565(x, y, CHAR_W, CHAR_H, bg);
            }
            self.fill_rect565(x + 1, y + 1, 4, 1, fg);
            self.fill_rect565(x + 1, y + 6, 4, 1, fg);
            self.fill_rect565(x + 1, y + 1, 1, 6, fg);
            self.fill_rect565(x + 4, y + 1, 1, 6, fg);
            return;
        }
        // Codes from 176 up shift by one glyph: LovyanGFX's "classic"
        // (non-cp437) charset behaviour, its default. Code 255 passes the
        // range check but shifts to 256, one past the table; LovyanGFX reads
        // out of bounds there (undefined behaviour), so we deliberately draw
        // a blank cell instead.
        let index = if code >= 176 { code + 1 } else { code };
        if index > 255 {
            if fill_bg {
                self.fill_rect565(x, y, CHAR_W, CHAR_H, bg);
            }
            return;
        }
        let glyph = &GLCD_FONT[index as usize * 5..index as usize * 5 + 5];
        for (col, bits) in glyph.iter().enumerate() {
            for row in 0..CHAR_H {
                if (bits >> row) & 1 == 1 {
                    self.put(x + col as i32, y + row, fg);
                } else if fill_bg {
                    self.put(x + col as i32, y + row, bg);
                }
            }
        }
        if fill_bg {
            self.fill_rect565(x + 5, y, 1, CHAR_H, bg);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{Canvas, rgb565};
    use crate::font_data::GLCD_FONT;

    const FG: u32 = 0xFFFFFF;
    const BG: u32 = 0x0000FF;

    fn expected_glyph(index: usize, fill_bg: bool) -> Canvas {
        let mut c = Canvas::new(6, 8);
        for col in 0..5 {
            let bits = GLCD_FONT[index * 5 + col];
            for row in 0..8 {
                if (bits >> row) & 1 == 1 {
                    c.put(col as i32, row, rgb565(FG));
                } else if fill_bg {
                    c.put(col as i32, row, rgb565(BG));
                }
            }
        }
        if fill_bg {
            c.fill_rect(5, 0, 1, 8, BG);
        }
        c
    }

    #[test]
    fn letter_a_matches_font_bytes() {
        assert_eq!(&GLCD_FONT[65 * 5..65 * 5 + 5], &[0x7C, 0x12, 0x11, 0x12, 0x7C]);
        let mut c = Canvas::new(6, 8);
        c.draw_text(0, 0, "A", FG, BG);
        assert_eq!(c, expected_glyph(65, true));
        assert_eq!(c.pixel(0, 2), Some(rgb565(FG)));
        assert_eq!(c.pixel(0, 0), Some(rgb565(BG)));
        assert_eq!(c.pixel(5, 3), Some(rgb565(BG)), "6th column is background");
    }

    #[test]
    fn equal_fg_and_bg_draws_no_background() {
        let mut c = Canvas::new(6, 8);
        c.draw_text(0, 0, "A", FG, FG);
        // fg == bg: LovyanGFX's fillbg is false, so only glyph pixels change.
        assert_eq!(c, expected_glyph(65, false));
    }

    #[test]
    fn text_advances_six_pixels_per_char() {
        let mut c = Canvas::new(12, 8);
        c.draw_text(0, 0, "AA", FG, BG);
        let mut second = Canvas::new(6, 8);
        for y in 0..8 {
            for x in 0..6 {
                second.put(x, y, c.pixel(x + 6, y).unwrap());
            }
        }
        assert_eq!(second, expected_glyph(65, true));
    }

    #[test]
    fn codes_from_176_shift_up_one_glyph() {
        // LovyanGFX's non-cp437 "classic" charset: code 176 draws glyph 177.
        let mut c = Canvas::new(6, 8);
        c.draw_text(0, 0, "\u{B0}", FG, BG);
        assert_eq!(c, expected_glyph(177, true));
    }

    #[test]
    fn text_clips_at_canvas_edge() {
        let mut c = Canvas::new(4, 4);
        c.draw_text(-3, -3, "A", FG, BG);
        assert_eq!(c.pixel(0, 0), expected_glyph(65, true).pixel(3, 3));
    }

    fn dummy_box(fill_bg: bool) -> Canvas {
        let mut c = Canvas::new(6, 8);
        if fill_bg {
            c.fill_rect(0, 0, 6, 8, BG);
        }
        for x in 1..5 {
            for y in 1..7 {
                if x == 1 || x == 4 || y == 1 || y == 6 {
                    c.put(x, y, rgb565(FG));
                }
            }
        }
        c
    }

    #[test]
    fn code_past_255_draws_outline_box_like_draw_char_dummy() {
        let mut c = Canvas::new(6, 8);
        c.draw_text(0, 0, "\u{100}", FG, BG);
        assert_eq!(c, dummy_box(true));
        assert_eq!(c.pixel(1, 1), Some(rgb565(FG)));
        assert_eq!(c.pixel(2, 2), Some(rgb565(BG)));
        assert_eq!(c.pixel(0, 0), Some(rgb565(BG)));
    }

    #[test]
    fn code_past_255_outline_without_background_when_fg_equals_bg() {
        let mut c = Canvas::new(6, 8);
        c.draw_text(0, 0, "\u{100}", FG, FG);
        assert_eq!(c, dummy_box(false));
        assert_eq!(c.pixels().iter().filter(|&&p| p != 0).count(), 16);
    }

    #[test]
    fn code_255_is_a_blank_cell() {
        let mut c = Canvas::new(6, 8);
        c.draw_text(0, 0, "\u{FF}", FG, BG);
        assert!(c.pixels().iter().all(|&p| p == rgb565(BG)));
    }

    #[test]
    fn text_at_extreme_coordinates_does_not_panic() {
        let mut c = Canvas::new(12, 8);
        for (x, y) in [(i32::MIN, i32::MIN), (i32::MAX, i32::MAX), (i32::MAX - 3, 0), (0, i32::MAX - 3),
                       (i32::MIN, 0), (0, i32::MIN), (i32::MAX, i32::MIN)] {
            c.draw_text(x, y, "AB\u{100}\u{FF}", FG, BG);
        }
        assert!(c.pixels().iter().all(|&p| p == 0), "nothing lands on the canvas");
        c.draw_text(-6, 0, "AB", FG, BG);
        let mut d = Canvas::new(12, 8);
        d.draw_text(0, 0, "B", FG, BG);
        assert_eq!(c.pixel(0, 0), d.pixel(0, 0), "a partly off-canvas string still draws its visible part");
    }
}
