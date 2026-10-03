//! Screen and chrome geometry -- one source of truth so a click always
//! lands where chrome is drawn.

pub const SCREEN_W: i32 = 640;
pub const SCREEN_H: i32 = 360;

/// A window must fit the screen; the check runs before anything is allocated.
pub fn window_size_ok(w: i32, h: i32) -> bool {
    (1..=SCREEN_W).contains(&w) && (1..=SCREEN_H).contains(&h)
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
