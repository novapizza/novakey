//! abbreviations.rs
//! Abbreviation expansion through the engine ("gõ tắt"): the word being
//! composed expands when Tab ends it, matched by what is on screen or by
//! the raw keys behind it.

use novakey_core::abbrev::Abbreviations;
use novakey_core::engine::{EngineResult, KeyClass, TelexEngine};

fn table() -> Abbreviations {
    let mut t = Abbreviations::new();
    t.insert("btw", "by the way").unwrap();
    t.insert("omw", "on my way").unwrap();
    t.insert("đc", "được").unwrap();
    t.insert("ko", "không").unwrap();
    t
}

fn type_word(e: &mut TelexEngine, s: &str) {
    for c in s.chars() {
        let key = if c == '\u{8}' {
            KeyClass::Backspace
        } else {
            KeyClass::Letter(c.to_ascii_lowercase())
        };
        e.process_key(key, c.is_uppercase(), false, false);
    }
}

fn expand(s: &str) -> Option<EngineResult> {
    let mut e = TelexEngine::new();
    type_word(&mut e, s);
    e.expand_abbreviation(&table())
}

fn restore(backspaces: usize, text: &str) -> Option<EngineResult> {
    Some(EngineResult::Restore {
        backspaces,
        text: text.to_string(),
    })
}

#[test]
fn plain_word_expands() {
    assert_eq!(expand("btw"), restore(3, "by the way"));
    assert_eq!(expand("ko"), restore(2, "không"));
}

#[test]
fn case_follows_typing() {
    assert_eq!(expand("Btw"), restore(3, "By the way"));
    assert_eq!(expand("BTW"), restore(3, "BY THE WAY"));
}

#[test]
fn raw_keys_match_when_telex_changed_the_screen() {
    // "omw" composes to "ơm" (2 chars on screen) — still the abbreviation.
    assert_eq!(expand("omw"), restore(2, "on my way"));
}

#[test]
fn composed_screen_text_matches() {
    // "ddc" shows "đc", which is itself a key.
    assert_eq!(expand("ddc"), restore(2, "được"));
}

#[test]
fn non_abbreviations_are_left_alone() {
    assert_eq!(expand("bt"), None);
    assert_eq!(expand("btwx"), None);
    assert_eq!(expand("vieetj"), None);
    assert_eq!(expand(""), None);
}

#[test]
fn raw_keys_are_ignored_after_a_mid_word_backspace() {
    // After a backspace into composed text the raw keys no longer describe
    // the screen; only the screen text can match. "omw<BS>w" shows "ơw".
    assert_eq!(expand("omw\u{8}w"), None);
    // "btx<BS>w" shows "btw" -> still expands.
    assert_eq!(expand("btx\u{8}w"), restore(3, "by the way"));
}

#[test]
fn raw_keys_survive_fixing_a_typo_before_any_composition() {
    // "omq<BS>" deletes the q the screen shows, so the keys stay exact and
    // "omw" (shown "ơm") still matches by its keys.
    assert_eq!(expand("omq\u{8}w"), restore(2, "on my way"));
    assert_eq!(expand("o\u{8}omw"), restore(2, "on my way"));
}

#[test]
fn backspace_over_the_space_steps_back_into_the_word() {
    let mut e = TelexEngine::new();
    type_word(&mut e, "omw");
    e.process_key(KeyClass::WordBreak, false, false, false);
    e.process_key(KeyClass::Backspace, false, false, false);
    assert_eq!(e.expand_abbreviation(&table()), restore(2, "on my way"));
}

#[test]
fn expansion_forgets_the_word() {
    let mut e = TelexEngine::new();
    type_word(&mut e, "btw");
    assert!(e.expand_abbreviation(&table()).is_some());
    assert!(e.buffer.is_empty());
    // Backspace does not step back into the replaced word.
    assert_eq!(
        e.process_key(KeyClass::Backspace, false, false, false),
        EngineResult::PassThrough
    );
    assert!(e.buffer.is_empty());
}

#[test]
fn a_miss_leaves_the_engine_untouched() {
    let mut e = TelexEngine::new();
    type_word(&mut e, "vieet");
    assert_eq!(e.expand_abbreviation(&table()), None);
    assert_eq!(e.buffer.text(), "viêt");
}
