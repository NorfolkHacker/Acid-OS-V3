//! Physical key -> Acid OS keycode: Shift is resolved here so apps only ever see a
//! final character or a KEY_* constant. Anything unmapped (function keys,
//! Ctrl/Alt) is ignored.

use acid_platform::keys::*;
pub use winit::keyboard::KeyCode;

pub fn translate_key(code: KeyCode, shift: bool) -> Option<i32> {
    let pick = |plain: char, shifted: char| Some(if shift { shifted } else { plain } as i32);
    let letter = |c: char| Some(if shift { c.to_ascii_uppercase() } else { c } as i32);
    match code {
        KeyCode::KeyA => letter('a'), KeyCode::KeyB => letter('b'), KeyCode::KeyC => letter('c'),
        KeyCode::KeyD => letter('d'), KeyCode::KeyE => letter('e'), KeyCode::KeyF => letter('f'),
        KeyCode::KeyG => letter('g'), KeyCode::KeyH => letter('h'), KeyCode::KeyI => letter('i'),
        KeyCode::KeyJ => letter('j'), KeyCode::KeyK => letter('k'), KeyCode::KeyL => letter('l'),
        KeyCode::KeyM => letter('m'), KeyCode::KeyN => letter('n'), KeyCode::KeyO => letter('o'),
        KeyCode::KeyP => letter('p'), KeyCode::KeyQ => letter('q'), KeyCode::KeyR => letter('r'),
        KeyCode::KeyS => letter('s'), KeyCode::KeyT => letter('t'), KeyCode::KeyU => letter('u'),
        KeyCode::KeyV => letter('v'), KeyCode::KeyW => letter('w'), KeyCode::KeyX => letter('x'),
        KeyCode::KeyY => letter('y'), KeyCode::KeyZ => letter('z'),
        KeyCode::Digit1 => pick('1', '!'), KeyCode::Digit2 => pick('2', '@'),
        KeyCode::Digit3 => pick('3', '#'), KeyCode::Digit4 => pick('4', '$'),
        KeyCode::Digit5 => pick('5', '%'), KeyCode::Digit6 => pick('6', '^'),
        KeyCode::Digit7 => pick('7', '&'), KeyCode::Digit8 => pick('8', '*'),
        KeyCode::Digit9 => pick('9', '('), KeyCode::Digit0 => pick('0', ')'),
        KeyCode::Space => Some(' ' as i32),
        KeyCode::Enter | KeyCode::NumpadEnter => Some(KEY_ENTER),
        KeyCode::Backspace => Some(KEY_BACKSPACE),
        KeyCode::Escape => Some(KEY_ESCAPE),
        KeyCode::Tab => Some(KEY_TAB),
        KeyCode::Delete => Some(KEY_DELETE),
        KeyCode::ArrowUp => Some(KEY_UP),
        KeyCode::ArrowDown => Some(KEY_DOWN),
        KeyCode::ArrowLeft => Some(KEY_LEFT),
        KeyCode::ArrowRight => Some(KEY_RIGHT),
        KeyCode::Minus => pick('-', '_'),
        KeyCode::Equal => pick('=', '+'),
        KeyCode::BracketLeft => pick('[', '{'),
        KeyCode::BracketRight => pick(']', '}'),
        KeyCode::Backslash => pick('\\', '|'),
        KeyCode::Semicolon => pick(';', ':'),
        KeyCode::Quote => pick('\'', '"'),
        KeyCode::Comma => pick(',', '<'),
        KeyCode::Period => pick('.', '>'),
        KeyCode::Slash => pick('/', '?'),
        KeyCode::Backquote => pick('`', '~'),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letters_and_shift() {
        assert_eq!(translate_key(KeyCode::KeyA, false), Some('a' as i32));
        assert_eq!(translate_key(KeyCode::KeyZ, true), Some('Z' as i32));
    }

    #[test]
    fn digits_and_their_shifted_symbols() {
        assert_eq!(translate_key(KeyCode::Digit1, false), Some('1' as i32));
        assert_eq!(translate_key(KeyCode::Digit1, true), Some('!' as i32));
        assert_eq!(translate_key(KeyCode::Digit9, true), Some('(' as i32));
        assert_eq!(translate_key(KeyCode::Digit0, false), Some('0' as i32));
        assert_eq!(translate_key(KeyCode::Digit0, true), Some(')' as i32));
    }

    #[test]
    fn named_keys() {
        assert_eq!(translate_key(KeyCode::Enter, false), Some(KEY_ENTER));
        assert_eq!(translate_key(KeyCode::NumpadEnter, false), Some(KEY_ENTER));
        assert_eq!(translate_key(KeyCode::Backspace, false), Some(KEY_BACKSPACE));
        assert_eq!(translate_key(KeyCode::Escape, false), Some(KEY_ESCAPE));
        assert_eq!(translate_key(KeyCode::Tab, false), Some(KEY_TAB));
        assert_eq!(translate_key(KeyCode::Delete, false), Some(KEY_DELETE));
        assert_eq!(translate_key(KeyCode::ArrowUp, false), Some(KEY_UP));
        assert_eq!(translate_key(KeyCode::ArrowDown, false), Some(KEY_DOWN));
        assert_eq!(translate_key(KeyCode::ArrowLeft, false), Some(KEY_LEFT));
        assert_eq!(translate_key(KeyCode::ArrowRight, false), Some(KEY_RIGHT));
        assert_eq!(translate_key(KeyCode::Space, true), Some(' ' as i32));
    }

    #[test]
    fn punctuation_pairs_resolve_shift() {
        let pairs = [
            (KeyCode::Minus, '-', '_'), (KeyCode::Equal, '=', '+'),
            (KeyCode::BracketLeft, '[', '{'), (KeyCode::BracketRight, ']', '}'),
            (KeyCode::Backslash, '\\', '|'), (KeyCode::Semicolon, ';', ':'),
            (KeyCode::Quote, '\'', '"'), (KeyCode::Comma, ',', '<'),
            (KeyCode::Period, '.', '>'), (KeyCode::Slash, '/', '?'),
            (KeyCode::Backquote, '`', '~'),
        ];
        for (k, plain, shifted) in pairs {
            assert_eq!(translate_key(k, false), Some(plain as i32), "{k:?}");
            assert_eq!(translate_key(k, true), Some(shifted as i32), "{k:?} shifted");
        }
    }

    #[test]
    fn unmapped_keys_are_ignored() {
        assert_eq!(translate_key(KeyCode::F1, false), None);
        assert_eq!(translate_key(KeyCode::ControlLeft, false), None);
    }
}
