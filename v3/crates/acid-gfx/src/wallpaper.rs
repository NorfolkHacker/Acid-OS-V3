//! The desktop wallpaper, replayed from its run table into a canvas once
//! and composited from there.

use crate::Canvas;
pub use crate::wallpaper_data::{WALLPAPER_H, WALLPAPER_W};
use crate::wallpaper_data::{WALLPAPER_PALETTE, WALLPAPER_RUNS};

pub fn wallpaper_canvas() -> Canvas {
    let mut c = Canvas::new(WALLPAPER_W, WALLPAPER_H);
    for &(y, x, len, ci) in WALLPAPER_RUNS.iter() {
        c.fill_rect(x as i32, y as i32, len as i32, 1, WALLPAPER_PALETTE[ci as usize]);
    }
    c
}

/// The wallpaper for a `w × h` screen: the art scaled (nearest neighbour)
/// until it covers the screen, with the overflow cropped evenly from both
/// sides, so the middle of the picture stays in the middle. At the art's
/// own 640×360 it is the art itself. Integer maths only, so every platform
/// draws the same pixels.
pub fn wallpaper_canvas_for(w: i32, h: i32) -> Canvas {
    let art = wallpaper_canvas();
    if (w, h) == (WALLPAPER_W, WALLPAPER_H) {
        return art;
    }
    // scale = num / den = max(w / WALLPAPER_W, h / WALLPAPER_H).
    let (num, den) = if w * WALLPAPER_H >= h * WALLPAPER_W { (w, WALLPAPER_W) } else { (h, WALLPAPER_H) };
    let scaled_w = (WALLPAPER_W * num + den - 1) / den;
    let scaled_h = (WALLPAPER_H * num + den - 1) / den;
    let (off_x, off_y) = ((scaled_w - w) / 2, (scaled_h - h) / 2);
    let mut c = Canvas::new(w, h);
    for y in 0..h {
        let sy = ((y + off_y) * den / num).min(WALLPAPER_H - 1);
        for x in 0..w {
            let sx = ((x + off_x) * den / num).min(WALLPAPER_W - 1);
            if let Some(p) = art.pixel(sx, sy) {
                c.fill_rect565(x, y, 1, 1, p);
            }
        }
    }
    c
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rgb565;
    use crate::wallpaper_data::{WALLPAPER_PALETTE, WALLPAPER_RUNS};

    #[test]
    fn has_expected_run_count_and_size() {
        assert_eq!(WALLPAPER_RUNS.len(), 3100);
        let c = wallpaper_canvas();
        assert_eq!((c.width(), c.height()), (640, 360));
    }

    #[test]
    fn every_row_is_fully_covered() {
        let mut per_row = [0u32; 360];
        for &(y, _, len, _) in WALLPAPER_RUNS.iter() {
            per_row[y as usize] += len as u32;
        }
        assert!(per_row.iter().all(|&n| n == 640));
    }

    #[test]
    fn sample_pixels_match_their_runs() {
        let c = wallpaper_canvas();
        assert_eq!(c.pixel(0, 0), Some(rgb565(0x05060A)), "run {{0,0,640,0}}");
        assert_eq!(c.pixel(224, 8), Some(rgb565(0x00E5FF)), "run {{8,224,4,9}}");
        assert_eq!(c.pixel(464, 16), Some(rgb565(0xD4E6DB)), "run {{16,464,4,8}}");
    }

    #[test]
    fn wallpaper_for_its_own_size_is_the_art() {
        assert_eq!(wallpaper_canvas_for(640, 360).pixels(), wallpaper_canvas().pixels());
    }

    #[test]
    fn scaled_wallpaper_has_the_screen_size_and_only_art_colours() {
        let palette: Vec<u16> = WALLPAPER_PALETTE.iter().map(|&c| rgb565(c)).collect();
        for (w, h) in [(640, 480), (800, 600)] {
            let c = wallpaper_canvas_for(w, h);
            assert_eq!((c.width(), c.height()), (w, h));
            assert!(c.pixels().iter().all(|p| palette.contains(p)), "{w}x{h} has a pixel not from the art");
        }
    }

    #[test]
    fn scaled_wallpaper_is_centre_cropped() {
        let art = wallpaper_canvas();
        // 640x480: scale 480/360, 854 px wide scaled, 107 px cropped each side.
        let c = wallpaper_canvas_for(640, 480);
        assert_eq!(c.pixel(0, 0), art.pixel(80, 0));
        assert_eq!(c.pixel(639, 479), art.pixel(559, 359));
        // 800x600: scale 600/360, 1067 px wide scaled, 133 px cropped each side.
        let c = wallpaper_canvas_for(800, 600);
        assert_eq!(c.pixel(0, 0), art.pixel(79, 0));
        assert_eq!(c.pixel(400, 300), art.pixel(319, 180));
        assert_eq!(c.pixel(799, 599), art.pixel(559, 359));
    }
}
