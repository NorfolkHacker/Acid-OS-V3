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
        self.draw_text_scaled(x, y, text, fg, bg, 1);
    }

    pub fn draw_text_scaled(&mut self, x: i32, y: i32, text: &str, fg: u32, bg: u32, scale: i32) {
        let s = scale.max(1);
        let fill_bg = fg != bg;
        let (fg, bg) = (rgb565(fg), rgb565(bg));
        // A row wholly above or below the canvas draws nothing; glyphs wholly
        // left of it are skipped and drawing stops past its right edge, so
        // draw_glyph only ever sees coordinates near the canvas and its
        // i32 sums cannot overflow, whatever x and y are. The bounds check
        // accounts for a glyph cell (CHAR_W·scale × CHAR_H·scale).
        if y >= self.h || y <= -(CHAR_H * s) {
            return;
        }
        let mut cx = x as i64;
        for ch in text.chars() {
            if cx >= self.w as i64 {
                break;
            }
            if cx > -((CHAR_W * s) as i64) {
                self.draw_glyph(cx as i32, y, ch as u32, fg, bg, fill_bg, s);
            }
            cx += (CHAR_W * s) as i64;
        }
    }

    fn draw_glyph(&mut self, x: i32, y: i32, code: u32, fg: u16, bg: u16, fill_bg: bool, s: i32) {
        // LovyanGFX range-checks the code against the font's 0..=255 span
        // BEFORE the shift (GLCDfont::drawChar, lgfx_fonts.cpp). Past it,
        // drawCharDummy paints the cell's background (only when fg != bg)
        // and ALWAYS outlines a 4x6 box inset by 1 in the fg colour.
        if code > 255 {
            if fill_bg {
                self.fill_rect565(x, y, CHAR_W * s, CHAR_H * s, bg);
            }
            self.fill_rect565(x + 1 * s, y + 1 * s, 4 * s, 1 * s, fg);
            self.fill_rect565(x + 1 * s, y + 6 * s, 4 * s, 1 * s, fg);
            self.fill_rect565(x + 1 * s, y + 1 * s, 1 * s, 6 * s, fg);
            self.fill_rect565(x + 4 * s, y + 1 * s, 1 * s, 6 * s, fg);
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
                self.fill_rect565(x, y, CHAR_W * s, CHAR_H * s, bg);
            }
            return;
        }
        let glyph = &GLCD_FONT[index as usize * 5..index as usize * 5 + 5];
        for (col, bits) in glyph.iter().enumerate() {
            for row in 0..CHAR_H {
                if (bits >> row) & 1 == 1 {
                    self.fill_rect565(x + col as i32 * s, y + row * s, s, s, fg);
                } else if fill_bg {
                    self.fill_rect565(x + col as i32 * s, y + row * s, s, s, bg);
                }
            }
        }
        if fill_bg {
            self.fill_rect565(x + 5 * s, y, 1 * s, CHAR_H * s, bg);
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
                    c.fill_rect565(col as i32, row, 1, 1, rgb565(FG));
                } else if fill_bg {
                    c.fill_rect565(col as i32, row, 1, 1, rgb565(BG));
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
                second.fill_rect565(x, y, 1, 1, c.pixel(x + 6, y).unwrap());
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

    /// The scale-1 glyph with every pixel as an `s × s` block.
    fn blown_up(small: &Canvas, s: i32) -> Canvas {
        let mut c = Canvas::new(small.width() * s, small.height() * s);
        for y in 0..small.height() {
            for x in 0..small.width() {
                c.fill_rect565(x * s, y * s, s, s, small.pixel(x, y).unwrap());
            }
        }
        c
    }

    fn dummy_box(fill_bg: bool) -> Canvas {
        let mut c = Canvas::new(6, 8);
        if fill_bg {
            c.fill_rect(0, 0, 6, 8, BG);
        }
        for x in 1..5 {
            for y in 1..7 {
                if x == 1 || x == 4 || y == 1 || y == 6 {
                    c.fill_rect565(x, y, 1, 1, rgb565(FG));
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

    #[test]
    fn scale_2_draws_each_font_pixel_as_a_2x2_block() {
        for (fg, bg) in [(FG, BG), (FG, FG)] {
            let mut small = Canvas::new(6, 8);
            small.draw_text(0, 0, "A", fg, bg);
            let mut big = Canvas::new(12, 16);
            big.draw_text_scaled(0, 0, "A", fg, bg, 2);
            assert_eq!(big, blown_up(&small, 2), "fg {fg:#x} bg {bg:#x}");
        }
    }

    #[test]
    fn scale_2_advances_twelve_pixels_a_character() {
        let mut c = Canvas::new(24, 16);
        c.draw_text_scaled(0, 0, "AA", FG, BG, 2);
        let mut one = Canvas::new(12, 16);
        one.draw_text_scaled(0, 0, "A", FG, BG, 2);
        for y in 0..16 {
            for x in 0..12 {
                assert_eq!(c.pixel(x + 12, y), one.pixel(x, y), "({x},{y})");
            }
        }
    }

    #[test]
    fn scale_1_is_draw_text() {
        let mut a = Canvas::new(40, 8);
        a.draw_text(0, 0, "Hi \u{1F600}", FG, BG);
        let mut b = Canvas::new(40, 8);
        b.draw_text_scaled(0, 0, "Hi \u{1F600}", FG, BG, 1);
        assert_eq!(a, b);
    }

    #[test]
    fn scaled_text_clips_at_the_canvas_edges() {
        let mut c = Canvas::new(10, 10);
        c.draw_text_scaled(-30, -30, "AAAA", FG, BG, 2);
        c.draw_text_scaled(8, 8, "AAAA", FG, BG, 2);
        c.draw_text_scaled(i32::MAX - 5, i32::MAX - 5, "A", FG, BG, 2);
    }
}
