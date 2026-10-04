//! Screen and chrome geometry -- one source of truth so a click always
//! lands where chrome is drawn.

/// A screen size. The kernel gets one when it's built and keeps it for
/// its whole life (`Kernel::screen`), so a click always lands where
/// chrome is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Screen {
    pub w: i32,
    pub h: i32,
}

impl Screen {
    pub const DEFAULT: Screen = Screen { w: 640, h: 480 };
    /// v2's size: the golden frames that prove parity with v2 use it.
    pub const WIDE: Screen = Screen { w: 640, h: 360 };
    pub const SVGA: Screen = Screen { w: 800, h: 600 };
    /// Every size the OS runs at, in the startup picker's order.
    pub const PRESETS: [Screen; 3] = [Screen::DEFAULT, Screen::WIDE, Screen::SVGA];

    /// "640x480" -> `DEFAULT`. Only the presets parse.
    pub fn parse(s: &str) -> Option<Screen> {
        fn num(s: &str) -> Option<i32> {
            if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            s.parse().ok()
        }
        let (w, h) = s.split_once('x')?;
        let (w, h) = (num(w)?, num(h)?);
        Self::PRESETS.into_iter().find(|p| p.w == w && p.h == h)
    }
}

/// A window must fit the screen; the check runs before anything is allocated.
pub fn window_size_ok(screen: Screen, w: i32, h: i32) -> bool {
    (1..=screen.w).contains(&w) && (1..=screen.h).contains(&h)
}
pub const TITLE_BAR_H: i32 = 16;
pub const CLOSE_BTN_R: i32 = 5;
pub const CLOSE_BTN_MARGIN: i32 = 8;
/// The desktop's own top strip -- every touch inside it goes to the
/// desktop window unconditionally (see router.rs).
pub const DESKTOP_STRIP_H: i32 = 24;
/// The most windows that can be open at once.
pub const WINDOW_MAX: usize = 8;
/// Spec §16.2: a cart-level app can't start another app while this many
/// cart-level windows are open. Built-in callers are bound only by WINDOW_MAX.
pub const CART_WINDOW_MAX: usize = 4;
// Carts can never take every window: built-in callers always have room.
const _: () = assert!(CART_WINDOW_MAX < WINDOW_MAX);

/// A window's size when its app draws at `scale`: the text area below the
/// title bar grows by `scale` in both directions (the title bar doesn't),
/// clamped to the screen less the desktop strip. Scale 1 is the size itself.
pub fn grown_size(screen: Screen, w: i32, h: i32, scale: i32) -> (i32, i32) {
    if scale <= 1 {
        return (w, h);
    }
    let w2 = (w * scale).min(screen.w);
    let h2 = (TITLE_BAR_H + (h - TITLE_BAR_H) * scale).max(h).min(screen.h - DESKTOP_STRIP_H);
    (w2, h2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_presets_parse() {
        assert_eq!(Screen::parse("640x480"), Some(Screen::DEFAULT));
        assert_eq!(Screen::parse("640x360"), Some(Screen::WIDE));
        assert_eq!(Screen::parse("800x600"), Some(Screen::SVGA));
        for bad in ["1024x768", "640", "abc", "", "640X480", "+640x480", "640x480x1", "x"] {
            assert_eq!(Screen::parse(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn the_default_is_first_and_640x480() {
        assert_eq!(Screen::PRESETS[0], Screen::DEFAULT);
        assert_eq!((Screen::DEFAULT.w, Screen::DEFAULT.h), (640, 480));
    }

    #[test]
    fn window_size_ok_follows_the_screen() {
        assert!(!window_size_ok(Screen::DEFAULT, 800, 100), "too wide at 640x480");
        assert!(window_size_ok(Screen::SVGA, 800, 100));
        assert!(window_size_ok(Screen::DEFAULT, 640, 480));
        assert!(!window_size_ok(Screen::WIDE, 640, 480), "too tall at 640x360");
        assert!(!window_size_ok(Screen::SVGA, 0, 10));
    }

    #[test]
    fn grown_size_doubles_the_text_area_and_clamps_to_the_screen() {
        assert_eq!(grown_size(Screen::DEFAULT, 220, 160, 1), (220, 160), "scale 1 changes nothing");
        assert_eq!(grown_size(Screen::DEFAULT, 220, 160, 2), (440, 304), "the title bar isn't doubled");
        assert_eq!(grown_size(Screen::DEFAULT, 420, 280, 2), (640, 456), "clamped to the screen less the strip");
        assert_eq!(grown_size(Screen::WIDE, 420, 280, 2), (640, 336));
        assert_eq!(grown_size(Screen::SVGA, 420, 280, 2), (800, 544));
        assert_eq!(grown_size(Screen::DEFAULT, 50, 10, 2), (100, 10), "a window no taller than its title bar keeps its height");
    }
}
