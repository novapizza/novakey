//! quick_vietnamese.rs
//! Windows-only "Quick Vietnamese" tests: a lone `w` right after a valid
//! syllable-initial consonant (cluster) becomes `ư` ("tw" -> "tư",
//! "chw" -> "chư"). Opt-in; off by default.

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

const U_HORN: char = '\u{01B0}'; // ư

// --- Enabled: w after an initial consonant becomes ư ---

#[test]
fn single_consonant_initials() {
    for init in ["b", "c", "d", "g", "h", "l", "m", "n", "r", "s", "t", "v", "x"] {
        let input = format!("{init}w");
        let want = format!("{init}{U_HORN}");
        assert_eq!(typ(&input, true), want, "input {input}");
    }
}

#[test]
fn digraph_and_trigraph_initials() {
    for init in ["ch", "kh", "ng", "nh", "ph", "th", "tr"] {
        let input = format!("{init}w");
        let want = format!("{init}{U_HORN}");
        assert_eq!(typ(&input, true), want, "input {input}");
    }
}

#[test]
fn d_stroke_initial() {
    // "ddw" -> "đư" works regardless, but with an initial 'd' too: "dw" -> "dư".
    assert_eq!(typ("dw", true), format!("d{U_HORN}"));
    assert_eq!(typ("ddw", true), format!("\u{0111}{U_HORN}")); // đư
}

#[test]
fn preserves_uppercase_initial() {
    assert_eq!(typ("Tw", true), format!("T{U_HORN}")); // Tư
}

#[test]
fn composes_a_full_word() {
    // twowng -> tương ; then a nặng tone -> tượng handled elsewhere.
    assert_eq!(typ("twowng", true), format!("t{U_HORN}\u{01A1}ng")); // tương
}

// --- Disabled by default: identical input stays literal ---

#[test]
fn off_by_default_leaves_tw_literal() {
    assert_eq!(typ("tw", false), "tw");
    assert_eq!(typ("chw", false), "chw");
}

// --- Only the listed initials trigger it ---

#[test]
fn non_listed_initials_not_triggered() {
    // 'k', 'p', 'q' are not valid standalone Vietnamese initials in the list.
    assert_eq!(typ("kw", true), "kw");
    assert_eq!(typ("pw", true), "pw");
}

// --- Must not disturb normal vowel-based composition ---

#[test]
fn standalone_w_still_becomes_u_horn() {
    assert_eq!(typ("w", true), U_HORN.to_string()); // ư
}

#[test]
fn uw_after_vowel_unchanged() {
    // "tuw" already made "tư" via uw->ư; quick mode must not change that.
    assert_eq!(typ("tuw", true), format!("t{U_HORN}"));
    assert_eq!(typ("tuw", false), format!("t{U_HORN}"));
}

// --- Safeguard: a second 'w' escapes the conjured ư back to a literal 'w' ---

#[test]
fn double_w_escapes_to_literal() {
    // The reported bug: "tww" must be "tw" (English), NOT "tuw".
    assert_eq!(typ("tww", true), "tw");
    assert_eq!(typ("chww", true), "chw");
    assert_eq!(typ("sww", true), "sw");
    assert_eq!(typ("ngww", true), "ngw");
}

#[test]
fn standalone_double_w_still_escapes() {
    // The original standalone escape is preserved: "ww" -> "w".
    assert_eq!(typ("ww", true), "w");
    assert_eq!(typ("ww", false), "w");
}

// --- "ưa" + w escapes to literal "uaw" (there is no "ưă" syllable) ---

#[test]
fn ua_horn_plus_w_reverts_to_literal_uaw() {
    // "uaw" -> "ưa" (horn on u); a further 'w' undoes the horn -> "uaw",
    // NOT the invalid "ưă". Applies in both modes.
    assert_eq!(typ("uaww", true), "uaw");
    assert_eq!(typ("uaww", false), "uaw");
    assert_eq!(typ("huaww", true), "huaw");
    assert_eq!(typ("huaww", false), "huaw");
}

#[test]
fn huawei_composes_literally() {
    // With Quick Vietnamese on, the English guard finishes the job:
    // "hua"+w -> "hưa", +w -> "huaw", +e,+i -> "huawei".
    assert_eq!(typ("Huawei", true), "Huawei");
}

#[test]
fn real_uw_double_w_still_reverts_the_u() {
    // A ư from a *real* "uw" (u was typed) reverts to "u"+"w", unchanged:
    // "tuww" -> "tuw". Only the bare-w ư escapes to a lone "w".
    assert_eq!(typ("tuww", true), "tuw");
}
