//! Physical key -> Acid OS keycode: Shift is resolved here so apps only ever see a
//! final character or a KEY_* constant. Anything unmapped (F13 and up, Ctrl/Alt) is ignored.

use acid_platform::KeyEvent;
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
        KeyCode::F1 => Some(KEY_F1), KeyCode::F2 => Some(KEY_F2), KeyCode::F3 => Some(KEY_F3),
        KeyCode::F4 => Some(KEY_F4), KeyCode::F5 => Some(KEY_F5), KeyCode::F6 => Some(KEY_F6),
        KeyCode::F7 => Some(KEY_F7), KeyCode::F8 => Some(KEY_F8), KeyCode::F9 => Some(KEY_F9),
        KeyCode::F10 => Some(KEY_F10), KeyCode::F11 => Some(KEY_F11), KeyCode::F12 => Some(KEY_F12),
        _ => None,
    }
}

/// Which physical keys are down, and the code each was pressed as. A
/// release sends the code from the press, so `a` pressed then Shift then
/// release still releases `a`. Repeats of a held key send nothing.
#[derive(Default)]
pub struct HeldKeys {
    down: Vec<(KeyCode, i32)>,
}

impl HeldKeys {
    /// The event a press should send, if the key maps and isn't already down.
    pub fn press(&mut self, key: KeyCode, shift: bool) -> Option<KeyEvent> {
        if self.down.iter().any(|&(k, _)| k == key) {
            return None;
        }
        let code = translate_key(key, shift)?;
        self.down.push((key, code));
        Some(KeyEvent { code, pressed: true })
    }

    /// The event a release should send, if that key was pressed.
    pub fn release(&mut self, key: KeyCode) -> Option<KeyEvent> {
        let i = self.down.iter().position(|&(k, _)| k == key)?;
        let (_, code) = self.down.remove(i);
        Some(KeyEvent { code, pressed: false })
    }

    /// Releases for every key still down: the window lost focus, so their
    /// real releases will never arrive.
    pub fn release_all(&mut self) -> Vec<KeyEvent> {
        self.down.drain(..).map(|(_, code)| KeyEvent { code, pressed: false }).collect()
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
        assert_eq!(translate_key(KeyCode::F13, false), None);
        assert_eq!(translate_key(KeyCode::ControlLeft, false), None);
    }

    #[test]
    fn a_release_pairs_with_its_press() {
        let mut h = HeldKeys::default();
        assert_eq!(h.press(KeyCode::KeyA, false), Some(KeyEvent { code: 'a' as i32, pressed: true }));
        assert_eq!(h.press(KeyCode::KeyA, false), None, "a repeat sends nothing");
        // Shift went down while A was held: the release is still 'a'.
        assert_eq!(h.release(KeyCode::KeyA), Some(KeyEvent { code: 'a' as i32, pressed: false }));
        assert_eq!(h.release(KeyCode::KeyA), None, "no second release");
    }

    #[test]
    fn unmapped_keys_never_press_or_release() {
        let mut h = HeldKeys::default();
        assert_eq!(h.press(KeyCode::F13, false), None);
        assert_eq!(h.release(KeyCode::F13), None);
    }

    #[test]
    fn losing_focus_releases_everything_held() {
        let mut h = HeldKeys::default();
        h.press(KeyCode::ArrowLeft, false);
        h.press(KeyCode::KeyW, true);
        assert_eq!(
            h.release_all(),
            [KeyEvent { code: KEY_LEFT, pressed: false }, KeyEvent { code: 'W' as i32, pressed: false }]
        );
        assert_eq!(h.release_all(), []);
    }

    #[test]
    fn function_keys_map_with_or_without_shift() {
        assert_eq!(translate_key(KeyCode::F1, false), Some(KEY_F1));
        assert_eq!(translate_key(KeyCode::F12, true), Some(KEY_F12));
        let mut h = HeldKeys::default();
        assert_eq!(h.press(KeyCode::F5, false), Some(KeyEvent { code: KEY_F5, pressed: true }));
        assert_eq!(h.release(KeyCode::F5), Some(KeyEvent { code: KEY_F5, pressed: false }));
    }
}
