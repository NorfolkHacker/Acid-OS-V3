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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rgb565;
    use crate::wallpaper_data::WALLPAPER_RUNS;

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
}
