//! Lines and filled triangles. Integer-only, clipped to the canvas, and
//! every loop is bounded by the canvas size -- never by the input
//! coordinates -- so huge or hostile values cannot panic or stall.

use crate::{rgb565, Canvas};

impl Canvas {
    /// Bresenham line, both endpoints included, clipped to the canvas.
    pub fn draw_line(&mut self, x1: i32, y1: i32, x2: i32, y2: i32, color: u32) {
        let (w, h) = (self.width() as i64, self.height() as i64);
        if w == 0 || h == 0 {
            return;
        }
        // Clip to a box a little larger than the canvas; that bounds the
        // stepping loop to a few canvas sizes.
        let Some((mut x, mut y, ex, ey)) = clip_line(
            (x1 as i64, y1 as i64, x2 as i64, y2 as i64),
            (-(w + 1), -(h + 1), 2 * w + 1, 2 * h + 1),
        ) else {
            return;
        };
        let c = rgb565(color);
        let dx = (ex - x).abs();
        let dy = -(ey - y).abs();
        let sx = if x < ex { 1 } else { -1 };
        let sy = if y < ey { 1 } else { -1 };
        let mut err = dx + dy;
        loop {
            if x >= 0 && y >= 0 && x < w && y < h {
                self.fill_rect565(x as i32, y as i32, 1, 1, c);
            }
            if x == ex && y == ey {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
    }

    /// Filled triangle. Each row from the top vertex to the bottom one is
    /// filled over the inclusive span between the leftmost and rightmost
    /// edge crossing, so triangles sharing an edge leave no gap (the shared
    /// pixels are drawn by both). Clipped to the canvas.
    pub fn fill_triangle(&mut self, x1: i32, y1: i32, x2: i32, y2: i32, x3: i32, y3: i32, color: u32) {
        let (w, h) = (self.width() as i64, self.height() as i64);
        if w == 0 || h == 0 {
            return;
        }
        // Anything beyond this is off-canvas anyway; clamping keeps the
        // interpolation products small.
        let lim = 4 * w.max(h);
        let cl = |v: i32| (v as i64).clamp(-lim, lim);
        let p = [(cl(x1), cl(y1)), (cl(x2), cl(y2)), (cl(x3), cl(y3))];
        let top = p[0].1.min(p[1].1).min(p[2].1);
        let bot = p[0].1.max(p[1].1).max(p[2].1);
        let c = rgb565(color);
        for y in top.max(0)..=bot.min(h - 1) {
            let mut lo = i64::MAX;
            let mut hi = i64::MIN;
            for (a, b) in [(p[0], p[1]), (p[1], p[2]), (p[2], p[0])] {
                // Always interpolate from the upper endpoint so a shared
                // edge gives the same x in both triangles.
                let (a, b) = if (a.1, a.0) <= (b.1, b.0) { (a, b) } else { (b, a) };
                if y < a.1 || y > b.1 {
                    continue;
                }
                if a.1 == b.1 {
                    lo = lo.min(a.0.min(b.0));
                    hi = hi.max(a.0.max(b.0));
                } else {
                    let x = a.0 + ((b.0 - a.0) * (y - a.1)).div_euclid(b.1 - a.1);
                    lo = lo.min(x);
                    hi = hi.max(x);
                }
            }
            if lo > hi {
                continue;
            }
            let x0 = lo.max(0);
            let x1 = (hi + 1).min(w);
            if x0 < x1 {
                self.fill_rect565(x0 as i32, y as i32, (x1 - x0) as i32, 1, c);
            }
        }
    }
}

/// Cohen-Sutherland clip of a segment to the inclusive box
/// (xmin, ymin, xmax, ymax). Products use i128 since coordinates may span
/// the whole i32 range.
fn clip_line(l: (i64, i64, i64, i64), b: (i64, i64, i64, i64)) -> Option<(i64, i64, i64, i64)> {
    let (mut x1, mut y1, mut x2, mut y2) = l;
    let (xmin, ymin, xmax, ymax) = b;
    let code = |x: i64, y: i64| {
        (x < xmin) as u8 | ((x > xmax) as u8) << 1 | ((y < ymin) as u8) << 2 | ((y > ymax) as u8) << 3
    };
    for _ in 0..8 {
        let (c1, c2) = (code(x1, y1), code(x2, y2));
        if c1 | c2 == 0 {
            return Some((x1, y1, x2, y2));
        }
        if c1 & c2 != 0 {
            return None;
        }
        let out = if c1 != 0 { c1 } else { c2 };
        let (dx, dy) = ((x2 - x1) as i128, (y2 - y1) as i128);
        let (nx, ny);
        if out & 1 != 0 {
            nx = xmin;
            ny = y1 + (dy * (xmin - x1) as i128 / dx) as i64;
        } else if out & 2 != 0 {
            nx = xmax;
            ny = y1 + (dy * (xmax - x1) as i128 / dx) as i64;
        } else if out & 4 != 0 {
            ny = ymin;
            nx = x1 + (dx * (ymin - y1) as i128 / dy) as i64;
        } else {
            ny = ymax;
            nx = x1 + (dx * (ymax - y1) as i128 / dy) as i64;
        }
        if c1 != 0 {
            (x1, y1) = (nx, ny);
        } else {
            (x2, y2) = (nx, ny);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use crate::{Canvas, rgb565};
    const C: u32 = 0xFFFFFF;
    fn lit(c: &Canvas) -> Vec<(i32, i32)> {
        let mut v = Vec::new();
        for y in 0..c.height() { for x in 0..c.width() { if c.pixel(x, y) == Some(rgb565(C)) { v.push((x, y)); } } }
        v
    }

    #[test]
    fn lines_hit_exactly_their_pixels() {
        let mut c = Canvas::new(8, 8);
        c.draw_line(1, 2, 5, 2, C);
        assert_eq!(lit(&c), [(1, 2), (2, 2), (3, 2), (4, 2), (5, 2)], "horizontal");
        let mut c = Canvas::new(8, 8);
        c.draw_line(3, 5, 3, 1, C);
        assert_eq!(lit(&c), [(3, 1), (3, 2), (3, 3), (3, 4), (3, 5)], "vertical, drawn upward");
        let mut c = Canvas::new(8, 8);
        c.draw_line(0, 0, 4, 4, C);
        assert_eq!(lit(&c), [(0, 0), (1, 1), (2, 2), (3, 3), (4, 4)], "45 degrees");
        let mut c = Canvas::new(8, 8);
        c.draw_line(0, 0, 2, 6, C);
        assert_eq!(lit(&c).len(), 7, "steep: one pixel per row");
        let mut a = Canvas::new(8, 8);
        a.draw_line(1, 1, 6, 3, C);
        let mut b = Canvas::new(8, 8);
        b.draw_line(6, 3, 1, 1, C);
        assert_eq!(lit(&a).len(), lit(&b).len(), "reversed endpoints draw the same number of pixels");
    }

    #[test]
    fn lines_clip_and_never_panic() {
        let mut c = Canvas::new(8, 8);
        c.draw_line(-5, 3, 20, 3, C);
        assert_eq!(lit(&c).len(), 8, "clipped to the row");
        c.draw_line(i32::MIN, i32::MIN, i32::MAX, i32::MAX, C);
        c.draw_line(i32::MAX, 0, i32::MIN, 7, C);
    }

    #[test]
    fn triangles_fill_exactly() {
        let mut c = Canvas::new(8, 8);
        c.fill_triangle(0, 0, 4, 0, 0, 4, C);
        // Rows 0..4: row y covers x 0..=4-y.
        let want: Vec<(i32, i32)> = (0..=4).flat_map(|y| (0..=4 - y).map(move |x| (x, y))).collect();
        assert_eq!(lit(&c), want, "flat-top right triangle");
        let mut c = Canvas::new(8, 8);
        c.fill_triangle(2, 0, 0, 4, 4, 4, C);
        assert!(lit(&c).contains(&(2, 0)) && lit(&c).contains(&(0, 4)) && lit(&c).contains(&(4, 4)), "flat-bottom keeps its corners");
    }

    #[test]
    fn two_triangles_tile_a_quad_without_gaps() {
        let mut c = Canvas::new(10, 10);
        c.fill_triangle(1, 1, 8, 1, 8, 8, C);
        c.fill_triangle(1, 1, 8, 8, 1, 8, C);
        for y in 1..=8 { for x in 1..=8 { assert_eq!(c.pixel(x, y), Some(rgb565(C)), "gap at ({x},{y})"); } }
    }

    #[test]
    fn degenerate_and_extreme_triangles_are_safe() {
        let mut c = Canvas::new(8, 8);
        c.fill_triangle(1, 1, 5, 5, 3, 3, C);
        assert!(!lit(&c).is_empty(), "a zero-area triangle still draws its span");
        c.fill_triangle(i32::MIN, i32::MIN, i32::MAX, 0, 0, i32::MAX, C);
        c.fill_triangle(-100, -100, -50, -100, -75, -50, C);
    }
}
