//! Acid OS's keycode vocabulary.
//! Printable keys arrive as their (Shift-resolved) character code; these
//! named keys sit above 255 so the two ranges never collide.

pub const KEY_ENTER: i32 = 257;
pub const KEY_BACKSPACE: i32 = 258;
pub const KEY_ESCAPE: i32 = 259;
pub const KEY_TAB: i32 = 260;
pub const KEY_DELETE: i32 = 261;
pub const KEY_UP: i32 = 262;
pub const KEY_DOWN: i32 = 263;
pub const KEY_LEFT: i32 = 264;
pub const KEY_RIGHT: i32 = 265;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_keys_have_fixed_values() {
        assert_eq!(
            [KEY_ENTER, KEY_BACKSPACE, KEY_ESCAPE, KEY_TAB, KEY_DELETE, KEY_UP, KEY_DOWN, KEY_LEFT, KEY_RIGHT],
            [257, 258, 259, 260, 261, 262, 263, 264, 265]
        );
    }
}
