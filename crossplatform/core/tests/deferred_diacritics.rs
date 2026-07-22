//! deferred_diacritics.rs
//! Windows-only "Deferred diacritics" (Vietnamese: "Bỏ dấu sau") tests: a
//! modifier key typed later in the word applies backward — "did" -> "đi",
//! "thana" -> "thân". Opt-in sub-option of Quick Vietnamese; off by default
//! and inert unless Quick Vietnamese is also enabled.

use novakey_core::engine::{EngineResult, KeyClass, TelexEngine};

fn key_for(c: char) -> KeyClass {
    if c.is_ascii_alphabetic() {
        KeyClass::Letter(c.to_ascii_lowercase())
    } else {
        KeyClass::Other
    }
}

/// Deferred diacritics requires Quick Vietnamese: enabling it sets both flags.
fn engine(deferred: bool) -> TelexEngine {
    let mut e = TelexEngine::new();
    e.is_vietnamese_mode = true;
    e.set_quick_vietnamese(deferred);
    e.set_deferred_diacritics(deferred);
    e
}

fn typ(s: &str, deferred: bool) -> String {
    let mut e = engine(deferred);
    for c in s.chars() {
        e.process_key(key_for(c), c.is_uppercase(), false, false);
    }
    e.buffer.text()
}

/// Apply every EngineResult to a running visible string — validates exact
/// backspace counts, not just final buffer text.
fn simulate_app(chars: &str, deferred: bool) -> String {
    let mut e = engine(deferred);
    let mut visible: Vec<char> = Vec::new();
    for c in chars.chars() {
        let shift = c.is_uppercase();
        let result = e.process_key(key_for(c), shift, false, false);
        match result {
            EngineResult::PassThrough | EngineResult::WordBreak => {
                let ch = if shift { c.to_ascii_uppercase() } else { c };
                visible.push(ch);
            }
            EngineResult::Replace { backspaces, text }
            | EngineResult::Restore { backspaces, text } => {
                let bs = backspaces.min(visible.len());
                visible.truncate(visible.len() - bs);
                visible.extend(text.chars());
            }
        }
    }
    visible.into_iter().collect()
}

/// Type letters then press a word break; return (break result, text before break).
fn type_then_break(chars: &str, deferred: bool) -> (EngineResult, String) {
    let mut e = engine(deferred);
    for c in chars.chars() {
        e.process_key(key_for(c), c.is_uppercase(), false, false);
    }
    let composed = e.buffer.text();
    let result = e.process_key(KeyClass::WordBreak, false, false, false);
    (result, composed)
}

// ============================================================
// Deferred đ
// ============================================================

#[test]
fn did_to_di() {
    assert_eq!(typ("did", true), "\u{0111}i"); // đi
}

#[test]
fn did_uppercase() {
    assert_eq!(typ("Did", true), "\u{0110}i"); // Đi
}

#[test]
fn dend_to_den() {
    assert_eq!(typ("dend", true), "\u{0111}en"); // đen
}

#[test]
fn dad_to_da() {
    // Valid syllable shape — transforms by design (documented trade-off).
    assert_eq!(typ("dad", true), "\u{0111}a"); // đa
}

#[test]
fn did_backspace_count() {
    assert_eq!(simulate_app("did", true), "\u{0111}i");
}

// ============================================================
// Deferred doubled vowels (circumflex)
// ============================================================

#[test]
fn thana_to_than() {
    assert_eq!(typ("thana", true), "th\u{00E2}n"); // thân
}

#[test]
fn viene_to_vien() {
    assert_eq!(typ("viene", true), "vi\u{00EA}n"); // viên
}

#[test]
fn nguyene_to_nguyen() {
    assert_eq!(typ("nguyene", true), "nguy\u{00EA}n"); // nguyên
}

#[test]
fn thana_backspace_count() {
    assert_eq!(simulate_app("thana", true), "th\u{00E2}n");
}

// ============================================================
// Both deferred forms + tone interplay
// ============================================================

#[test]
fn dongdo_to_dong() {
    // deferred đ first, then deferred ô.
    assert_eq!(typ("dongdo", true), "\u{0111}\u{00F4}ng"); // đông
}

#[test]
fn dongod_to_dong() {
    // deferred ô first, then deferred đ.
    assert_eq!(typ("dongod", true), "\u{0111}\u{00F4}ng"); // đông
}

#[test]
fn tone_then_deferred_vowel() {
    // "muonso": tone lands on o ("muón"), deferred o upgrades it to ố.
    assert_eq!(typ("muonso", true), "mu\u{1ED1}n"); // muốn
}

#[test]
fn deferred_vowel_then_tone() {
    // "muonos": deferred ô first, then tone -> ố.
    assert_eq!(typ("muonos", true), "mu\u{1ED1}n"); // muốn
}

#[test]
fn nguyenxe_to_nguyen_tilde() {
    // Tone re-placement after the deferred circumflex: ẽ -> ễ.
    assert_eq!(typ("nguyenxe", true), "nguy\u{1EC5}n"); // nguyễn
}

// ============================================================
// Undo / escape (n+1 convention, same as aaa -> aa)
// ============================================================

#[test]
fn didd_escapes_to_did() {
    assert_eq!(typ("didd", true), "did");
}

#[test]
fn thanaa_escapes_to_thana() {
    assert_eq!(typ("thanaa", true), "thana");
}

#[test]
fn dataa_escapes_to_data() {
    assert_eq!(typ("dataa", true), "data");
}

#[test]
fn photoo_escapes_to_photo() {
    assert_eq!(typ("photoo", true), "photo");
}

// ============================================================
// English guard: invalid results revert to literal in real time
// ============================================================

#[test]
fn disabled_stays_literal() {
    assert_eq!(simulate_app("disabled", true), "disabled");
}

#[test]
fn banana_stays_literal() {
    assert_eq!(simulate_app("banana", true), "banana");
}

#[test]
fn cocoa_stays_literal() {
    assert_eq!(simulate_app("cocoa", true), "cocoa");
}

#[test]
fn dido_stays_literal() {
    // "did" -> "đi" (bare deferred đ, no visible tone/modifier), then 'o'
    // makes it invalid — proves the deferred_transform guard term works.
    assert_eq!(simulate_app("dido", true), "dido");
}

// ============================================================
// Never fires: invalid syllables, open nuclei, wrong shapes
// ============================================================

#[test]
fn seven_stays_literal() {
    assert_eq!(simulate_app("seven", true), "seven");
}

#[test]
fn element_stays_literal() {
    assert_eq!(simulate_app("element", true), "element");
}

#[test]
fn khoeo_open_nucleus_untouched() {
    assert_eq!(typ("khoeo", true), "khoeo");
}

#[test]
fn xoong_unchanged_from_default() {
    // Adjacent "oo" -> ô fires before deferred logic, exactly as in default
    // mode; the real word "xoong" is typed "xooong" (triple-o escape).
    assert_eq!(typ("xoong", true), typ("xoong", false));
    assert_eq!(typ("xooong", true), "xoong");
}

#[test]
fn add_stays_add() {
    // chars[0] is not 'd'; adjacent dd branch needs count()==1.
    assert_eq!(typ("add", true), "add");
}

// ============================================================
// Word break
// ============================================================

#[test]
fn dend_survives_word_break() {
    let (result, composed) = type_then_break("dend", true);
    assert_eq!(composed, "\u{0111}en");
    assert_eq!(result, EngineResult::WordBreak); // no restore — valid syllable
}

#[test]
fn adjacent_dd_survives_word_break() {
    // Bare adjacent đ keeps its historical word-break exemption.
    let (result, composed) = type_then_break("dd", true);
    assert_eq!(composed, "\u{0111}");
    assert_eq!(result, EngineResult::WordBreak);
}

// ============================================================
// Documented limitation: valid-shaped English words transform
// ============================================================

#[test]
fn data_transforms_by_design() {
    // "dât" is structurally valid Vietnamese — indistinguishable from the
    // intended "thana" -> "thân". Escape: "dataa" -> "data".
    assert_eq!(typ("data", true), "d\u{00E2}t");
}

// ============================================================
// Gating: inert without Quick Vietnamese
// ============================================================

#[test]
fn inert_without_quick_vietnamese() {
    let mut e = TelexEngine::new();
    e.is_vietnamese_mode = true;
    e.set_deferred_diacritics(true); // quick_vietnamese stays off
    for c in "did".chars() {
        e.process_key(key_for(c), false, false, false);
    }
    assert_eq!(e.buffer.text(), "did");

    let mut e2 = TelexEngine::new();
    e2.is_vietnamese_mode = true;
    e2.set_deferred_diacritics(true);
    for c in "thana".chars() {
        e2.process_key(key_for(c), false, false, false);
    }
    assert_eq!(e2.buffer.text(), "thana");
}

// ============================================================
// Flag off: today's behavior unchanged
// ============================================================

#[test]
fn flag_off_regressions() {
    assert_eq!(typ("did", false), "did");
    assert_eq!(typ("thana", false), "thana");
    assert_eq!(typ("dend", false), "dend");
    assert_eq!(typ("data", false), "data");
    assert_eq!(typ("dongdo", false), "dongdo");
}
