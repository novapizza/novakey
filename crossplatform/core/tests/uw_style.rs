//! uw_style.rs
//! Windows-only enhancement tests: forward "ươ" linking so the "type uw first"
//! style composes the same word as the canonical style. NOT part of the Swift
//! parity suite (the Swift engine lacks this); these assert the added behavior.

use novakey_core::engine::{KeyClass, TelexEngine};

fn key_for(c: char) -> KeyClass {
    if c.is_ascii_alphabetic() {
        KeyClass::Letter(c.to_ascii_lowercase())
    } else {
        KeyClass::Other
    }
}

fn typ(s: &str) -> String {
    let mut e = TelexEngine::new();
    e.is_vietnamese_mode = true;
    for c in s.chars() {
        e.process_key(key_for(c), c.is_uppercase(), false, false);
    }
    e.buffer.text()
}

// --- The "uw first" style now composes ươ correctly ---

#[test]
fn uw_first_thuong() {
    assert_eq!(typ("thuwong"), "th\u{01B0}\u{01A1}ng"); // thương
}
#[test]
fn uw_first_tuong_hoi() {
    assert_eq!(typ("tuwongr"), "t\u{01B0}\u{1EDF}ng"); // tưởng
}
#[test]
fn uw_first_truong_huyen() {
    assert_eq!(typ("truwongf"), "tr\u{01B0}\u{1EDD}ng"); // trường
}
#[test]
fn uw_first_nguoi() {
    assert_eq!(typ("nguwoif"), "ng\u{01B0}\u{1EDD}i"); // người
}

// --- The canonical styles STILL land on the same word ---

#[test]
fn canonical_uo_w_still_works() {
    assert_eq!(typ("thuowng"), "th\u{01B0}\u{01A1}ng"); // thương (uo + w)
}
#[test]
fn canonical_two_w_still_works() {
    // uw + ow: the second 'w' confirms the auto-horned ơ instead of undoing it.
    assert_eq!(typ("thuwowng"), "th\u{01B0}\u{01A1}ng"); // thương
}
#[test]
fn tuong_three_ways_agree() {
    let want = "t\u{01B0}\u{1EDF}ng"; // tưởng
    assert_eq!(typ("tuowngr"), want); // canonical uo+w
    assert_eq!(typ("tuwongr"), want); // uw first, plain o
    assert_eq!(typ("tuwowngr"), want); // uw + ow
}

// --- Guards: the enhancement must not break unrelated behavior ---

#[test]
fn now_escape_unaffected() {
    // 'o'+'w' here is a user horn (not auto), so double-w still escapes to "now".
    let mut e = TelexEngine::new();
    e.is_vietnamese_mode = true;
    for c in "noww".chars() {
        e.process_key(key_for(c), false, false, false);
    }
    assert_eq!(e.buffer.text(), "now");
}
#[test]
fn uu_not_linked() {
    // 'u' after ư must stay plain (ưu is valid), only 'o' links.
    assert_eq!(typ("cuwu"), "c\u{01B0}u"); // cưu
}
#[test]
fn dduong_still_works() {
    assert_eq!(typ("dduwowngf"), "\u{0111}\u{01B0}\u{1EDD}ng"); // đường
}
