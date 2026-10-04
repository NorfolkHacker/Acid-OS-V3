//! Where a newly launched window goes -- an 18 px cascade from the top
//! left, clamped so the window stays on screen and below the desktop strip.

use crate::layout::{DESKTOP_STRIP_H, Screen};

/// `existing` is the number of windows already registered.
pub fn cascade_position(screen: Screen, existing: usize, w: i32, h: i32) -> (i32, i32) {
    let n = existing as i32;
    let mut x = 20 + (n * 18) % 400;
    let mut y = DESKTOP_STRIP_H + 10 + (n * 18) % 200;
    let max_x = screen.w - w;
    let max_y = screen.h - h;
    if max_x < 0 {
        x = 0;
    } else if x > max_x {
        x = max_x;
    }
    if max_y < DESKTOP_STRIP_H {
        y = DESKTOP_STRIP_H;
    } else if y > max_y {
        y = max_y;
    }
    if y < DESKTOP_STRIP_H {
        y = DESKTOP_STRIP_H;
    }
    (x, y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn second_window_steps_down_and_right() {
        assert_eq!(cascade_position(Screen::WIDE, 1, 200, 150), (38, 52));
    }

    #[test]
    fn first_window() {
        assert_eq!(cascade_position(Screen::WIDE, 0, 200, 150), (20, 34));
    }

    #[test]
    fn cascade_wraps() {
        // n*18 % 400 and % 200: n = 12 -> 216 % 400 = 216, 216 % 200 = 16.
        assert_eq!(cascade_position(Screen::WIDE, 12, 100, 100), (236, 50));
    }

    #[test]
    fn clamps_to_screen() {
        assert_eq!(cascade_position(Screen::WIDE, 0, 700, 150), (0, 34));
        assert_eq!(cascade_position(Screen::WIDE, 0, 200, 400), (20, 24));
        assert_eq!(cascade_position(Screen::WIDE, 10, 600, 300), (40, 60));
    }

    #[test]
    fn a_bigger_screen_clamps_later() {
        assert_eq!(cascade_position(Screen::WIDE, 0, 700, 150), (0, 34), "wider than 640");
        assert_eq!(cascade_position(Screen::SVGA, 0, 700, 150), (20, 34), "fits at 800");
        assert_eq!(cascade_position(Screen::DEFAULT, 0, 200, 400), (20, 34), "fits at 480 tall");
    }
}
