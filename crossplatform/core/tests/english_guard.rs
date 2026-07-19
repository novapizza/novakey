//! english_guard.rs
//! Real-time English-word guard — part of Quick Vietnamese (Windows-only). With
//! Quick Vietnamese on, the instant a diacritic transformation yields a
//! structurally invalid syllable the engine reverts to the literal keystrokes
//! mid-word — no word break needed. Prevents transient mojibake like
//! "huawei" -> "hưaei". Default mode is untouched (keeps Swift parity).

use novakey_core::engine::{KeyClass, TelexEngine};

fn key_for(c: char) -> KeyClass {
    if c.is_ascii_alphabetic() {
        KeyClass::Letter(c.to_ascii_lowercase())
    } else {
        KeyClass::Other
    }
}

fn typ(s: &str, quick: bool) -> String {
    let mut e = TelexEngine::new();
    e.is_vietnamese_mode = true;
    e.set_quick_vietnamese(quick);
    for c in s.chars() {
        e.process_key(key_for(c), c.is_uppercase(), false, false);
    }
    e.buffer.text()
}

// --- The reported case: "huawei" stays literal mid-word (Quick Vietnamese on) ---

#[test]
fn huawei_stays_literal_midword() {
    // "hua"+w -> "hưa" (valid), +e -> invalid -> revert to raw, +i -> "huawei".
    assert_eq!(typ("huawei", true), "huawei");
}

#[test]
fn other_mixed_words_stay_literal() {
    assert_eq!(typ("await", true), "await"); // "a"+w->"ă", +i "ăi" invalid -> revert
    assert_eq!(typ("sword", true), "sword"); // "sw"->"sư"->"sươ"->"sửơ", +d invalid -> revert
    assert_eq!(typ("nuance", true), "nuance"); // no diacritic trigger -> stays literal
}

// --- Default mode is unchanged: the guard only runs with Quick Vietnamese ---

#[test]
fn default_mode_keeps_legacy_midword_behavior() {
    // With Quick Vietnamese off, standard Telex still composes "hưaei" mid-word
    // (it only restores on the word break) — exactly the Swift-parity behavior.
    assert_eq!(typ("huawei", false), "h\u{01B0}aei"); // hưaei
}

// --- Genuine Vietnamese words must still compose correctly (guard on) ---

#[test]
fn vietnamese_words_unaffected() {
    assert_eq!(typ("muaw", true), "m\u{01B0}a"); // mưa (valid ưa nucleus)
    assert_eq!(typ("huaws", true), "h\u{1EE9}a"); // hứa (ua+w then sắc)
    assert_eq!(typ("thuwowng", true), "th\u{01B0}\u{01A1}ng"); // thương
    assert_eq!(typ("chuaw", true), "ch\u{01B0}a"); // chưa
    assert_eq!(typ("xuaw", true), "x\u{01B0}a"); // xưa
}
