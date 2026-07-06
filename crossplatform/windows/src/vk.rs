//! vk.rs
//! Maps Windows virtual-key codes to the engine's neutral `KeyClass`
//! (the Windows equivalent of KeyCode.swift). Pure numeric logic so it is
//! unit-testable without the `windows` crate.

use novakey_core::KeyClass;

/// Classify a virtual-key code.
///
/// Returns `None` for keys the engine must never see (modifier and toggle
/// keys): the hook passes those straight through without touching the engine,
/// otherwise pressing Shift to type an uppercase letter would reset the buffer.
pub fn classify(vk: u32) -> Option<KeyClass> {
    match vk {
        // A-Z -> lowercase Letter; case is decided by Shift/CapsLock in the hook.
        0x41..=0x5A => Some(KeyClass::Letter((b'a' + (vk as u8 - 0x41)) as char)),

        // VK_BACK
        0x08 => Some(KeyClass::Backspace),

        // Modifier & toggle keys -> ignore (pass through untouched):
        // VK_SHIFT(10) CONTROL(11) MENU/alt(12) CAPITAL(14) LWIN(5B) RWIN(5C)
        // NUMLOCK(90) SCROLL(91) L/R SHIFT/CONTROL/MENU (A0-A5)
        0x10 | 0x11 | 0x12 | 0x14 | 0x5B | 0x5C | 0x90 | 0x91 | 0xA0..=0xA5 => None,

        v if is_word_break(v) => Some(KeyClass::WordBreak),

        _ => Some(KeyClass::Other),
    }
}

/// Word-break keys — mirrors `KeyCode.isWordBreak` from the macOS engine.
fn is_word_break(vk: u32) -> bool {
    matches!(
        vk,
        0x20 |          // VK_SPACE
        0x0D |          // VK_RETURN (incl. numpad Enter)
        0x09 |          // VK_TAB
        0x1B |          // VK_ESCAPE
        0x25..=0x28 |   // VK_LEFT / UP / RIGHT / DOWN
        0x24 |          // VK_HOME
        0x23 |          // VK_END
        0x21 |          // VK_PRIOR (Page Up)
        0x22 |          // VK_NEXT  (Page Down)
        0x2E |          // VK_DELETE (forward delete)
        0xBA |          // VK_OEM_1     ;:
        0xBB |          // VK_OEM_PLUS  =+
        0xBC |          // VK_OEM_COMMA ,<
        0xBD |          // VK_OEM_MINUS -_
        0xBE |          // VK_OEM_PERIOD .>
        0xBF |          // VK_OEM_2     /?
        0xC0 |          // VK_OEM_3     `~
        0xDB |          // VK_OEM_4     [{
        0xDC |          // VK_OEM_5     \|
        0xDD |          // VK_OEM_6     ]}
        0xDE            // VK_OEM_7     '"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letters_map_to_lowercase() {
        assert_eq!(classify(0x41), Some(KeyClass::Letter('a'))); // VK_A
        assert_eq!(classify(0x5A), Some(KeyClass::Letter('z'))); // VK_Z
        assert_eq!(classify(0x44), Some(KeyClass::Letter('d'))); // VK_D
    }

    #[test]
    fn backspace() {
        assert_eq!(classify(0x08), Some(KeyClass::Backspace));
    }

    #[test]
    fn word_breaks() {
        assert_eq!(classify(0x20), Some(KeyClass::WordBreak)); // space
        assert_eq!(classify(0x0D), Some(KeyClass::WordBreak)); // enter
        assert_eq!(classify(0xBC), Some(KeyClass::WordBreak)); // comma
        assert_eq!(classify(0x27), Some(KeyClass::WordBreak)); // right arrow
    }

    #[test]
    fn modifiers_ignored() {
        assert_eq!(classify(0x10), None); // shift
        assert_eq!(classify(0x11), None); // control
        assert_eq!(classify(0x14), None); // capslock
        assert_eq!(classify(0xA0), None); // lshift
    }

    #[test]
    fn digits_are_other() {
        // Digits aren't Vietnamese letters — engine treats Other as a reset.
        assert_eq!(classify(0x30), Some(KeyClass::Other)); // '0'
        assert_eq!(classify(0x39), Some(KeyClass::Other)); // '9'
    }
}
