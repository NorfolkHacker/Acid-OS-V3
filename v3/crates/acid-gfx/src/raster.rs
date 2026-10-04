//! Lines and filled triangles. Integer-only, clipped to the canvas, and
//! every loop is bounded by the canvas size -- never by the input
//! coordinates -- so huge or hostile values cannot panic or stall.

use crate::{rgb565, Canvas};

impl Canvas {
    /// Line with both endpoints included, clipped to the canvas. For each
    /// pixel along the major axis the minor coordinate is computed in
    /// closed form from the true endpoints (rounded half up), so clipping
    /// can never change a pixel and the loop is bounded by the canvas.
    pub fn draw_line(&mut self, x1: i32, y1: i32, x2: i32, y2: i32, color: u32) {
        let (w, h) = (self.width() as i64, self.height() as i64);
        if w == 0 || h == 0 {
            return;
        }
        let c = rgb565(color);
        let (mut a, mut b) = ((x1 as i64, y1 as i64), (x2 as i64, y2 as i64));
        let x_major = (b.0 - a.0).abs() >= (b.1 - a.1).abs();
        // Iterate from the smaller major coordinate so both directions draw
        // the same pixels.
        if (x_major && a.0 > b.0) || (!x_major && a.1 > b.1) {
            core::mem::swap(&mut a, &mut b);
        }
        let ((m1, n1), (m2, n2), mlim, nlim) =
            if x_major { ((a.0, a.1), (b.0, b.1), w, h) } else { ((a.1, a.0), (b.1, b.0), h, w) };
        let dm = (m2 - m1) as i128;
        let dn = (n2 - n1) as i128;
        for m in m1.max(0)..=m2.min(mlim - 1) {
            let n = if dm == 0 {
                n1
            } else {
                let num = 2 * (m - m1) as i128 * dn + dm;
                n1 + num.div_euclid(2 * dm) as i64
            };
            if n < 0 || n >= nlim {
                continue;
            }
            let (x, y) = if x_major { (m, n) } else { (n, m) };
            self.fill_rect565(x as i32, y as i32, 1, 1, c);
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
        let p = [(x1 as i64, y1 as i64), (x2 as i64, y2 as i64), (x3 as i64, y3 as i64)];
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
                    let x = a.0 + (((b.0 - a.0) as i128 * (y - a.1) as i128).div_euclid((b.1 - a.1) as i128)) as i64;
                    lo = lo.min(x);
                    hi = hi.max(x);
                }
            }
            if lo > hi {
                continue;
            }
            let x0 = lo.max(0);
            let x1 = hi.saturating_add(1).min(w);
            if x0 < x1 {
                self.fill_rect565(x0 as i32, y as i32, (x1 - x0) as i32, 1, c);
            }
        }
    }
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
        assert_eq!(lit(&a), lit(&b), "reversed endpoints draw the same pixels");
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

    #[test]
    fn triangle_with_far_vertex_keeps_its_exact_spans() {
        let mut c = Canvas::new(120, 120);
        c.fill_triangle(10, 10, 50, 60, 100000, 30, C);
        let row = |c: &Canvas, y: i32| -> Vec<i32> { (0..120).filter(|&x| c.pixel(x, y) == Some(rgb565(C))).collect() };
        let r11 = row(&c, 11);
        assert!(r11.first().is_some_and(|&x| x <= 11) && r11.last() == Some(&119), "y=11: {r11:?}");
        for y in [12, 13, 14, 56, 57, 58, 59] {
            assert_eq!(row(&c, y).last().copied(), Some(119), "row {y} reaches the right edge");
        }
        // Same shape unclipped on a wide canvas: rows 11..=59 agree on 0..120.
        let mut big = Canvas::new(120_000, 70);
        big.fill_triangle(10, 10, 50, 60, 100000, 30, C);
        for y in 11..=59 {
            let want: Vec<i32> = (0..120).filter(|&x| big.pixel(x, y) == Some(rgb565(C))).collect();
            assert_eq!(row(&c, y), want, "row {y}");
        }
    }

    #[test]
    fn clipped_lines_equal_unclipped_lines() {
        let mut seed: u64 = 12345;
        let mut next = || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((seed >> 33) % 1321) as i32 - 600
        };
        for i in 0..500 {
            let (x1, y1, x2, y2) = (next(), next(), next(), next());
            let mut a = Canvas::new(120, 120);
            a.draw_line(x1, y1, x2, y2, C);
            let mut b = Canvas::new(1440, 1440);
            b.draw_line(x1 + 600, y1 + 600, x2 + 600, y2 + 600, C);
            for y in 0..120 {
                for x in 0..120 {
                    assert_eq!(a.pixel(x, y), b.pixel(x + 600, y + 600), "line {i} ({x1},{y1})-({x2},{y2}) at ({x},{y})");
                }
            }
        }
    }
}
