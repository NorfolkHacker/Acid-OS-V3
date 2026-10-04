//! Acid OS's drawing layer. A Canvas is RGB565, and every 0xRRGGBB colour
//! quantises to it the same way every time -- the overlay's magenta colour
//! key depends on that.
#![cfg_attr(not(test), no_std)]

extern crate alloc;

use alloc::{vec, vec::Vec};

mod font;
mod font_data;
mod raster;
pub mod wallpaper;
mod wallpaper_data;

/// 0xRRGGBB -> RGB565 by truncation, LovyanGFX's own conversion.
pub const fn rgb565(c: u32) -> u16 {
    (((c >> 8) & 0xF800) | ((c >> 5) & 0x07E0) | ((c >> 3) & 0x001F)) as u16
}

/// RGB565 -> 0xRRGGBB, replicating each channel's high bits into the low
/// ones so full white stays 0xFFFFFF. Only used to show frames on a 24-bit
/// display; never fed back into drawing.
pub const fn rgb565_to_888(p: u16) -> u32 {
    let r5 = ((p >> 11) & 0x1F) as u32;
    let g6 = ((p >> 5) & 0x3F) as u32;
    let b5 = (p & 0x1F) as u32;
    let r = (r5 << 3) | (r5 >> 2);
    let g = (g6 << 2) | (g6 >> 4);
    let b = (b5 << 3) | (b5 >> 2);
    (r << 16) | (g << 8) | b
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Canvas {
    w: i32,
    h: i32,
    px: Vec<u16>,
}

impl Canvas {
    /// Zero-initialised (black), as LovyanGFX's createSprite leaves a sprite.
    pub fn new(w: i32, h: i32) -> Self {
        let w = w.max(0);
        let h = h.max(0);
        Self { w, h, px: vec![0; (w * h) as usize] }
    }

    pub fn width(&self) -> i32 {
        self.w
    }

    pub fn height(&self) -> i32 {
        self.h
    }

    pub fn pixels(&self) -> &[u16] {
        &self.px
    }

    pub fn pixel(&self, x: i32, y: i32) -> Option<u16> {
        if x < 0 || y < 0 || x >= self.w || y >= self.h {
            return None;
        }
        Some(self.px[(y * self.w + x) as usize])
    }

    pub fn fill_rect(&mut self, x: i32, y: i32, w: i32, h: i32, color: u32) {
        self.fill_rect565(x, y, w, h, rgb565(color));
    }

    pub fn fill_rect565(&mut self, x: i32, y: i32, w: i32, h: i32, c: u16) {
        let x0 = x.max(0);
        let y0 = y.max(0);
        let x1 = x.saturating_add(w).min(self.w);
        let y1 = y.saturating_add(h).min(self.h);
        if x0 >= x1 || y0 >= y1 {
            return;
        }
        for yy in y0..y1 {
            let row = (yy * self.w) as usize;
            self.px[row + x0 as usize..row + x1 as usize].fill(c);
        }
    }

    /// A line-for-line port of LovyanGFX's fillCircle/fillCircleHelper
    /// (LGFXBase.cpp), so every circle -- the title-bar close dot above
    /// all -- comes out pixel-identical to the golden frames.
    ///
    /// Outside that port's range it stays bounded and panic-free: a
    /// negative radius draws nothing (as LovyanGFX's empty spans did), a
    /// circle wholly off the canvas returns at once, and a radius far beyond
    /// the canvas (more than twice its width plus height) is rasterised one
    /// canvas row at a time from its exact span (`isqrt`), never by walking
    /// the radius -- the port's loop is O(r) and runs under the app's canvas
    /// lock, which the compositor waits on. Only such radii differ from
    /// LovyanGFX, which itself overflowed at r >= 2^30.
    pub fn fill_circle(&mut self, x: i32, y: i32, r: i32, color: u32) {
        let c = rgb565(color);
        if r < 0 {
            return;
        }
        let (x64, y64, r64) = (x as i64, y as i64, r as i64);
        if x64 + r64 < 0 || y64 + r64 < 0 || x64 - r64 >= self.w as i64 || y64 - r64 >= self.h as i64 {
            return;
        }
        if r64 > 2 * (self.w as i64 + self.h as i64) + 64 {
            self.fill_huge_circle(x64, y64, r64, c);
            return;
        }
        // Here r and |x|, |y| are bounded by a few canvas sizes, so the
        // port's i32 arithmetic cannot overflow.
        self.fill_rect565(x - r, y, (r << 1) + 1, 1, c);
        self.fill_circle_helper(x, y, r, 3, 0, c);
    }

    /// Row-by-row fill of a circle whose radius dwarfs the canvas: on each
    /// visible row `dy` from the centre the span is x +- isqrt(r^2 - dy^2).
    fn fill_huge_circle(&mut self, x: i64, y: i64, r: i64, c: u16) {
        let (w, h) = (self.w as i64, self.h as i64);
        for row in 0..h {
            let dy = row - y;
            if dy.abs() > r {
                continue;
            }
            let half = (r * r - dy * dy).isqrt();
            let x0 = (x - half).max(0);
            let x1 = (x + half + 1).min(w);
            if x0 < x1 {
                self.fill_rect565(x0 as i32, row as i32, (x1 - x0) as i32, 1, c);
            }
        }
    }

    fn fill_circle_helper(&mut self, x: i32, y: i32, mut r: i32, corners: u8, delta: i32, c: u16) {
        if r <= 0 {
            return;
        }
        let delta = delta + 1;
        let mut f = 1 - r;
        let mut ddf_y = -(r << 1);
        let mut ddf_x = 1;
        let mut i = 0;
        loop {
            let mut len = 0;
            while f < 0 {
                ddf_x += 2;
                f += ddf_x;
                len += 1;
            }
            i += len;
            ddf_y += 2;
            f += ddf_y;
            if corners & 0x1 != 0 {
                if len != 0 {
                    self.fill_rect565(x - r, y + i - len + 1, (r << 1) + delta, len, c);
                }
                self.fill_rect565(x - i, y + r, (i << 1) + delta, 1, c);
            }
            if corners & 0x2 != 0 {
                self.fill_rect565(x - i, y - r, (i << 1) + delta, 1, c);
                if len != 0 {
                    self.fill_rect565(x - r, y - i, (r << 1) + delta, len, c);
                }
            }
            r -= 1;
            if i >= r {
                break;
            }
        }
    }

    /// Copies the (x, y, w, h) rectangle of `src` to the same position in
    /// self, clipped to both canvases. Used to repaint wallpaper into an
    /// app canvas, giving the same pixels as replaying the run table for
    /// that rectangle.
    pub fn copy_rect_from(&mut self, src: &Canvas, x: i32, y: i32, w: i32, h: i32) {
        let x0 = x.max(0);
        let y0 = y.max(0);
        let x1 = x.saturating_add(w).min(self.w).min(src.w);
        let y1 = y.saturating_add(h).min(self.h).min(src.h);
        if x0 >= x1 || y0 >= y1 {
            return;
        }
        for yy in y0..y1 {
            let d = (yy * self.w) as usize;
            let s = (yy * src.w) as usize;
            self.px[d + x0 as usize..d + x1 as usize].copy_from_slice(&src.px[s + x0 as usize..s + x1 as usize]);
        }
    }

    pub fn blit(&mut self, src: &Canvas, x: i32, y: i32) {
        self.blit_inner(src, x, y, None);
    }

    /// Like blit, but source pixels equal to `key` (after RGB565
    /// conversion, as LovyanGFX's pushSprite compares) are left transparent.
    pub fn blit_keyed(&mut self, src: &Canvas, x: i32, y: i32, key: u32) {
        self.blit_inner(src, x, y, Some(rgb565(key)));
    }

    fn blit_inner(&mut self, src: &Canvas, x: i32, y: i32, key: Option<u16>) {
        // Clip in source coordinates; saturating so extreme x/y can't overflow.
        let sx0 = x.saturating_neg().max(0);
        let sy0 = y.saturating_neg().max(0);
        let sx1 = src.w.min(self.w.saturating_sub(x));
        let sy1 = src.h.min(self.h.saturating_sub(y));
        if sx0 >= sx1 || sy0 >= sy1 {
            return;
        }
        // After clipping, every destination column/row is non-negative.
        let dx0 = (x + sx0) as usize;
        let dx1 = (x + sx1) as usize;
        for sy in sy0..sy1 {
            let s = sy as usize * src.w as usize;
            let d = (y + sy) as usize * self.w as usize;
            let src_row = &src.px[s + sx0 as usize..s + sx1 as usize];
            let dst_row = &mut self.px[d + dx0..d + dx1];
            match key {
                None => dst_row.copy_from_slice(src_row),
                Some(k) => {
                    for (dp, &sp) in dst_row.iter_mut().zip(src_row) {
                        if sp != k {
                            *dp = sp;
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WHITE: u16 = 0xFFFF;

    #[test]
    fn rgb565_truncates_like_lovyangfx() {
        assert_eq!(rgb565(0x00FF66), 0x07EC);
        assert_eq!(rgb565(0xFFFFFF), 0xFFFF);
        assert_eq!(rgb565(0x050607), 0x0020);
        assert_eq!(rgb565(0xFF00FF), 0xF81F);
    }

    #[test]
    fn rgb565_to_888_replicates_high_bits() {
        assert_eq!(rgb565_to_888(0xFFFF), 0xFFFFFF);
        assert_eq!(rgb565_to_888(0x0000), 0x000000);
        assert_eq!(rgb565(rgb565_to_888(0x07EC)), 0x07EC);
    }

    #[test]
    fn new_canvas_is_black() {
        let c = Canvas::new(3, 2);
        assert_eq!(c.pixels(), &[0u16; 6]);
        assert_eq!((c.width(), c.height()), (3, 2));
    }

    #[test]
    fn fill_rect_clips_to_canvas() {
        let mut c = Canvas::new(4, 4);
        c.fill_rect(-2, -2, 4, 4, 0xFFFFFF);
        assert_eq!(c.pixel(0, 0), Some(WHITE));
        assert_eq!(c.pixel(1, 1), Some(WHITE));
        assert_eq!(c.pixel(2, 0), Some(0));
        assert_eq!(c.pixel(0, 2), Some(0));
        c.fill_rect(3, 3, 100, 100, 0xFFFFFF);
        assert_eq!(c.pixel(3, 3), Some(WHITE));
    }

    #[test]
    fn fill_rect_with_empty_size_draws_nothing() {
        let mut c = Canvas::new(4, 4);
        c.fill_rect(1, 1, 0, 2, 0xFFFFFF);
        c.fill_rect(1, 1, -3, 2, 0xFFFFFF);
        assert!(c.pixels().iter().all(|&p| p == 0));
    }

    #[test]
    fn copy_rect_from_copies_the_same_position() {
        let mut src = Canvas::new(4, 4);
        src.fill_rect(1, 1, 2, 2, 0xFFFFFF);
        let mut dst = Canvas::new(4, 4);
        dst.copy_rect_from(&src, 1, 1, 1, 2);
        assert_eq!(dst.pixel(1, 1), Some(WHITE));
        assert_eq!(dst.pixel(1, 2), Some(WHITE));
        assert_eq!(dst.pixel(2, 1), Some(0), "outside the rect");
    }

    #[test]
    fn copy_rect_from_clips_to_both_canvases() {
        let mut src = Canvas::new(6, 3);
        src.fill_rect(0, 0, 6, 3, 0xFFFFFF);
        let mut dst = Canvas::new(4, 5);
        dst.copy_rect_from(&src, -2, -2, 100, 100);
        assert_eq!(dst.pixel(3, 2), Some(WHITE));
        assert_eq!(dst.pixel(3, 3), Some(0), "src is only 3 rows tall");
        dst.copy_rect_from(&src, 2, 2, 0, 5); // empty
    }

    #[test]
    fn pixel_out_of_bounds_is_none() {
        let c = Canvas::new(2, 2);
        assert_eq!(c.pixel(2, 0), None);
        assert_eq!(c.pixel(-1, 0), None);
    }

    #[test]
    fn blit_copies_and_clips() {
        let mut src = Canvas::new(2, 2);
        src.fill_rect(0, 0, 2, 2, 0xFFFFFF);
        let mut dst = Canvas::new(3, 3);
        dst.blit(&src, 2, -1);
        assert_eq!(dst.pixel(2, 0), Some(WHITE));
        assert_eq!(dst.pixel(1, 0), Some(0));
        assert_eq!(dst.pixel(2, 1), Some(0));
    }

    #[test]
    fn blit_keyed_skips_key_pixels() {
        let mut src = Canvas::new(2, 1);
        src.fill_rect(0, 0, 1, 1, 0xFF00FF);
        src.fill_rect(1, 0, 1, 1, 0xFFFFFF);
        let mut dst = Canvas::new(2, 1);
        dst.fill_rect(0, 0, 2, 1, 0x00FF66);
        dst.blit_keyed(&src, 0, 0, 0xFF00FF);
        assert_eq!(dst.pixel(0, 0), Some(rgb565(0x00FF66)));
        assert_eq!(dst.pixel(1, 0), Some(WHITE));
    }

    #[test]
    fn blit_negative_x_on_row_zero() {
        let mut src = Canvas::new(2, 2);
        src.fill_rect(0, 0, 2, 2, 0xFFFFFF);
        let mut dst = Canvas::new(3, 3);
        dst.blit(&src, -1, 0);
        assert_eq!(dst.pixel(0, 0), Some(WHITE));
        assert_eq!(dst.pixel(0, 1), Some(WHITE));
        assert_eq!(dst.pixel(1, 0), Some(0));
        assert_eq!(dst.pixel(0, 2), Some(0));
        assert_eq!(dst.pixels().iter().filter(|&&p| p == WHITE).count(), 2);
    }

    #[test]
    fn blit_clips_on_right_edge() {
        let mut src = Canvas::new(3, 1);
        src.fill_rect(0, 0, 3, 1, 0xFFFFFF);
        let mut dst = Canvas::new(4, 2);
        dst.blit(&src, 2, 1);
        assert_eq!(dst.pixel(1, 1), Some(0));
        assert_eq!(dst.pixel(2, 1), Some(WHITE));
        assert_eq!(dst.pixel(3, 1), Some(WHITE));
        assert_eq!(dst.pixels().iter().filter(|&&p| p == WHITE).count(), 2);
        dst.blit(&src, i32::MIN, i32::MAX);
        dst.blit(&src, i32::MAX, i32::MIN);
    }

    #[test]
    fn blit_keyed_negative_x() {
        let mut src = Canvas::new(3, 1);
        src.fill_rect(0, 0, 1, 1, 0xFFFFFF);
        src.fill_rect(1, 0, 1, 1, 0xFF00FF);
        src.fill_rect(2, 0, 1, 1, 0xFFFFFF);
        let mut dst = Canvas::new(3, 1);
        dst.fill_rect(0, 0, 3, 1, 0x00FF66);
        dst.blit_keyed(&src, -1, 0, 0xFF00FF);
        assert_eq!(dst.pixel(0, 0), Some(rgb565(0x00FF66)));
        assert_eq!(dst.pixel(1, 0), Some(WHITE));
        assert_eq!(dst.pixel(2, 0), Some(rgb565(0x00FF66)));
    }

    /// Width and starting x of the run of `c` pixels in row `y`.
    fn span(cv: &Canvas, y: i32, c: u16) -> (i32, i32) {
        let xs: Vec<i32> = (0..cv.width()).filter(|&x| cv.pixel(x, y) == Some(c)).collect();
        if xs.is_empty() { (0, -1) } else { (xs.len() as i32, xs[0]) }
    }

    #[test]
    fn fill_circle_radius_zero_is_one_pixel() {
        let mut c = Canvas::new(3, 3);
        c.fill_circle(1, 1, 0, 0xFFFFFF);
        assert_eq!(c.pixels().iter().filter(|&&p| p == WHITE).count(), 1);
        assert_eq!(c.pixel(1, 1), Some(WHITE));
    }

    #[test]
    fn fill_circle_radius_five_matches_lovyangfx_rows() {
        // Row widths worked by hand through LovyanGFX's fillCircle +
        // fillCircleHelper (LGFXBase.cpp) for r = 5 centred at (8, 8) -- the
        // close dot's radius (CLOSE_BTN_R).
        let mut c = Canvas::new(17, 17);
        c.fill_circle(8, 8, 5, 0xFFFFFF);
        let expect = [(-5, 5), (-4, 7), (-3, 9), (-2, 11), (-1, 11), (0, 11), (1, 11), (2, 11), (3, 9), (4, 7), (5, 5)];
        for (dy, width) in expect {
            let (w, x0) = span(&c, 8 + dy, WHITE);
            assert_eq!(w, width, "row dy={dy}");
            assert_eq!(x0, 8 - width / 2, "row dy={dy} start");
        }
        assert_eq!(span(&c, 2, WHITE).0, 0);
        assert_eq!(span(&c, 14, WHITE).0, 0);
    }

    #[test]
    fn a_huge_radius_fills_the_clip_quickly() {
        for r in [i32::MAX, 1_000_000_000] {
            let mut c = Canvas::new(64, 48);
            let t = std::time::Instant::now();
            c.fill_circle(10, 10, r, 0xFFFFFF);
            assert!(t.elapsed() < std::time::Duration::from_millis(50), "r = {r} took {:?}", t.elapsed());
            assert!(c.pixels().iter().all(|&p| p == WHITE), "r = {r} covers the whole canvas");
        }
    }

    #[test]
    fn a_huge_circle_mostly_off_canvas_paints_only_its_visible_edge() {
        let mut c = Canvas::new(64, 48);
        let t = std::time::Instant::now();
        c.fill_circle(-1_000_000_000, 10, 1_000_000_005, 0xFFFFFF);
        assert!(t.elapsed() < std::time::Duration::from_millis(50), "took {:?}", t.elapsed());
        for y in 0..48 {
            for x in 0..5 {
                assert_eq!(c.pixel(x, y), Some(WHITE), "({x}, {y}) is inside");
            }
            for x in 6..64 {
                assert_eq!(c.pixel(x, y), Some(0), "({x}, {y}) is outside");
            }
        }
    }

    #[test]
    fn extreme_circle_arguments_do_not_panic() {
        let mut c = Canvas::new(16, 16);
        for (x, y, r) in [
            (i32::MIN, i32::MIN, 5),
            (i32::MAX, i32::MAX, 5),
            (i32::MAX, 0, i32::MAX),
            (i32::MIN, 0, i32::MAX),
            (0, 0, i32::MIN),
            (0, 0, -1),
            (i32::MAX, i32::MIN, 1 << 30),
        ] {
            c.fill_circle(x, y, r, 0xFFFFFF);
        }
        let mut d = Canvas::new(16, 16);
        d.fill_circle(0, 0, -5, 0xFFFFFF);
        assert!(d.pixels().iter().all(|&p| p == 0), "a negative radius draws nothing");
    }
}
