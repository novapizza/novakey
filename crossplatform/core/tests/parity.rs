//! parity.rs
//! Rust port of Tests/run_tests.swift (the Swift oracle). Every case here
//! mirrors a Swift test with the same expected value, proving the Rust engine
//! composes identically. Additional per-key trace tests assert exact backspace
//! counts (final-text equality alone doesn't prove them).

use novakey_core::buffer::SyllableBuffer;
use novakey_core::data::{ToneMark, ViChar, VowelModifier};
use novakey_core::engine::{EngineResult, KeyClass, TelexEngine};
use novakey_core::tone::find_tone_position;

// ============================================================
// Helpers (mirror the Swift harness)
// ============================================================

/// Classify a source character the way vk.rs / KeyCode would: letters become
/// `Letter(lowercase)`, everything else `Other`. Used only to drive tests via
/// letter strings.
fn key_for(c: char) -> KeyClass {
    if c.is_ascii_alphabetic() {
        KeyClass::Letter(c.to_ascii_lowercase())
    } else {
        KeyClass::Other
    }
}

/// Type a string of letters and return the composed buffer text.
fn type_and_get_text(chars: &str) -> String {
    let mut engine = TelexEngine::new();
    engine.is_vietnamese_mode = true;
    for c in chars.chars() {
        let shift = c.is_uppercase();
        engine.process_key(key_for(c), shift, false, false);
    }
    engine.buffer.text()
}

fn make_buffer(text: &str) -> SyllableBuffer {
    let mut buffer = SyllableBuffer::new();
    for c in text.to_lowercase().chars() {
        buffer.append(ViChar::new(c));
    }
    buffer
}

/// Type letters, then press Space; return (space result, composed text before break).
fn type_then_space(chars: &str) -> (EngineResult, String) {
    let mut engine = TelexEngine::new();
    engine.is_vietnamese_mode = true;
    for c in chars.chars() {
        let shift = c.is_uppercase();
        engine.process_key(key_for(c), shift, false, false);
    }
    let composed = engine.buffer.text();
    let result = engine.process_key(KeyClass::WordBreak, false, false, false);
    (result, composed)
}

/// Simulate the app's visible text by applying each EngineResult to a running
/// String — mirror of Swift `simulateApp`. This also exercises exact backspace
/// counts, catching any char-vs-byte miscount.
fn simulate_app(chars: &str) -> String {
    let mut engine = TelexEngine::new();
    engine.is_vietnamese_mode = true;
    let mut visible: Vec<char> = Vec::new();
    for c in chars.chars() {
        let shift = c.is_uppercase();
        let result = engine.process_key(key_for(c), shift, false, false);
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

// ============================================================
// SyllableBuffer Tests
// ============================================================

#[test]
fn empty_buffer() {
    let buffer = SyllableBuffer::new();
    assert!(buffer.is_empty());
    assert_eq!(buffer.count(), 0);
    assert_eq!(buffer.text(), "");
}

#[test]
fn append_and_text() {
    let mut buffer = SyllableBuffer::new();
    buffer.append(ViChar::new('h'));
    buffer.append(ViChar::new('a'));
    assert_eq!(buffer.text(), "ha");
    assert_eq!(buffer.count(), 2);
}

#[test]
fn remove_last() {
    let mut buffer = SyllableBuffer::new();
    buffer.append(ViChar::new('a'));
    buffer.append(ViChar::new('b'));
    let removed = buffer.remove_last();
    assert_eq!(removed.map(|c| c.base), Some('b'));
    assert_eq!(buffer.text(), "a");
}

#[test]
fn reset() {
    let mut buffer = SyllableBuffer::new();
    buffer.append(ViChar::new('t'));
    buffer.append(ViChar::new('o'));
    buffer.apply_tone(ToneMark::Sac, 1);
    buffer.reset();
    assert!(buffer.is_empty());
    assert_eq!(buffer.current_tone, ToneMark::None);
    assert_eq!(buffer.tone_index, None);
}

#[test]
fn vowel_indices() {
    let mut buffer = SyllableBuffer::new();
    for c in "toan".chars() {
        buffer.append(ViChar::new(c));
    }
    assert_eq!(buffer.vowel_indices(), vec![1, 2]);
    assert_eq!(buffer.vowel_count(), 2);
    assert_eq!(buffer.first_vowel_index(), Some(1));
    assert_eq!(buffer.last_vowel_index(), Some(2));
}

#[test]
fn ending_consonant() {
    let mut buffer = SyllableBuffer::new();
    for c in "toa".chars() {
        buffer.append(ViChar::new(c));
    }
    assert!(!buffer.has_ending_consonant());
    buffer.append(ViChar::new('n'));
    assert!(buffer.has_ending_consonant());
    assert_eq!(buffer.ending_consonant(), "n");
}

#[test]
fn apply_tone() {
    let mut buffer = SyllableBuffer::new();
    buffer.append(ViChar::new('a'));
    buffer.apply_tone(ToneMark::Sac, 0);
    assert_eq!(buffer.text(), "\u{00E1}"); // á
    assert_eq!(buffer.current_tone, ToneMark::Sac);
    assert_eq!(buffer.tone_index, Some(0));
}

#[test]
fn apply_modifier_circumflex() {
    let mut buffer = SyllableBuffer::new();
    buffer.append(ViChar::new('a'));
    buffer.apply_modifier(VowelModifier::Circumflex, 0);
    assert_eq!(buffer.text(), "\u{00E2}"); // â
}

#[test]
fn apply_tone_plus_modifier() {
    let mut buffer = SyllableBuffer::new();
    buffer.append(ViChar::new('a'));
    buffer.apply_modifier(VowelModifier::Circumflex, 0);
    buffer.apply_tone(ToneMark::Sac, 0);
    assert_eq!(buffer.text(), "\u{1EA5}"); // ấ
}

#[test]
fn move_tone() {
    let mut buffer = SyllableBuffer::new();
    for c in "toa".chars() {
        buffer.append(ViChar::new(c));
    }
    buffer.apply_tone(ToneMark::Sac, 1);
    buffer.move_tone(2);
    assert_eq!(buffer.chars[1].tone, ToneMark::None);
    assert_eq!(buffer.chars[2].tone, ToneMark::Sac);
}

#[test]
fn initial_vowel_ending_parsing() {
    let mut buffer = SyllableBuffer::new();
    for c in "trong".chars() {
        buffer.append(ViChar::new(c));
    }
    assert_eq!(buffer.initial_consonant(), "tr");
    assert_eq!(buffer.vowel_cluster(), "o");
    assert_eq!(buffer.ending_consonant(), "ng");
}

// ============================================================
// TonePlacement Tests
// ============================================================

#[test]
fn tp_single_vowel_ba() {
    assert_eq!(find_tone_position(&make_buffer("ba")), Some(1));
}

#[test]
fn tp_single_vowel_ti() {
    assert_eq!(find_tone_position(&make_buffer("ti")), Some(1));
}

#[test]
fn tp_toan() {
    assert_eq!(find_tone_position(&make_buffer("toan")), Some(2));
}

#[test]
fn tp_hoang() {
    assert_eq!(find_tone_position(&make_buffer("hoang")), Some(2));
}

#[test]
fn tp_hoa() {
    assert_eq!(find_tone_position(&make_buffer("hoa")), Some(2));
}

#[test]
fn tp_hai() {
    assert_eq!(find_tone_position(&make_buffer("hai")), Some(1));
}

#[test]
fn tp_cao() {
    assert_eq!(find_tone_position(&make_buffer("cao")), Some(1));
}

#[test]
fn tp_khoai() {
    assert_eq!(find_tone_position(&make_buffer("khoai")), Some(3));
}

#[test]
fn tp_quan() {
    assert_eq!(find_tone_position(&make_buffer("quan")), Some(2));
}

#[test]
fn tp_no_vowels_tr() {
    assert_eq!(find_tone_position(&make_buffer("tr")), None);
}

// ============================================================
// TelexEngine Tests
// ============================================================

#[test]
fn tone_sac() {
    assert_eq!(type_and_get_text("as"), "\u{00E1}");
}
#[test]
fn tone_huyen() {
    assert_eq!(type_and_get_text("af"), "\u{00E0}");
}
#[test]
fn tone_hoi() {
    assert_eq!(type_and_get_text("ar"), "\u{1EA3}");
}
#[test]
fn tone_nga() {
    assert_eq!(type_and_get_text("ax"), "\u{00E3}");
}
#[test]
fn tone_nang() {
    assert_eq!(type_and_get_text("aj"), "\u{1EA1}");
}
#[test]
fn remove_tone_asz() {
    assert_eq!(type_and_get_text("asz"), "a");
}
#[test]
fn circumflex_aa() {
    assert_eq!(type_and_get_text("aa"), "\u{00E2}");
}
#[test]
fn circumflex_ee() {
    assert_eq!(type_and_get_text("ee"), "\u{00EA}");
}
#[test]
fn circumflex_oo() {
    assert_eq!(type_and_get_text("oo"), "\u{00F4}");
}
#[test]
fn breve_aw() {
    assert_eq!(type_and_get_text("aw"), "\u{0103}");
}
#[test]
fn horn_ow() {
    assert_eq!(type_and_get_text("ow"), "\u{01A1}");
}
#[test]
fn horn_uw() {
    assert_eq!(type_and_get_text("uw"), "\u{01B0}");
}
#[test]
fn ww_standalone_reversal() {
    assert_eq!(type_and_get_text("ww"), "w");
}
#[test]
fn uww_undo() {
    assert_eq!(type_and_get_text("uww"), "uw");
}
#[test]
fn dstroke_dd() {
    assert_eq!(type_and_get_text("dd"), "\u{0111}");
}
#[test]
fn combined_vieejt() {
    assert_eq!(type_and_get_text("Vieejt"), "Việt");
}

#[test]
fn english_mode_passthrough() {
    let mut engine = TelexEngine::new();
    engine.is_vietnamese_mode = false;
    let result = engine.process_key(KeyClass::Letter('a'), false, false, false);
    assert_eq!(result, EngineResult::PassThrough);
}

#[test]
fn word_break_space_resets() {
    let mut engine = TelexEngine::new();
    engine.is_vietnamese_mode = true;
    engine.process_key(KeyClass::Letter('a'), false, false, false);
    engine.process_key(KeyClass::Letter('s'), false, false, false);
    let result = engine.process_key(KeyClass::WordBreak, false, false, false);
    assert_eq!(result, EngineResult::WordBreak);
    assert!(engine.buffer.is_empty());
}

#[test]
fn modifier_cmd_resets() {
    let mut engine = TelexEngine::new();
    engine.is_vietnamese_mode = true;
    engine.process_key(KeyClass::Letter('a'), false, false, false);
    let result = engine.process_key(KeyClass::Letter('c'), false, true, false);
    assert_eq!(result, EngineResult::PassThrough);
    assert!(engine.buffer.is_empty());
}

// ============================================================
// Tone re-check on late vowel append
// ============================================================

#[test]
fn hosa_to_hoa() {
    assert_eq!(type_and_get_text("hosa"), "ho\u{00E1}"); // hoá
}
#[test]
fn tosa_to_toa() {
    assert_eq!(type_and_get_text("tosa"), "to\u{00E1}"); // toá
}
#[test]
fn hofa_to_hoa_grave() {
    assert_eq!(type_and_get_text("hofa"), "ho\u{00E0}"); // hoà
}

// ============================================================
// Replacement diff correctness (app-visible)
// ============================================================

#[test]
fn appvis_hosa() {
    assert_eq!(simulate_app("hosa"), "ho\u{00E1}");
}
#[test]
fn appvis_disab() {
    assert_eq!(simulate_app("disab"), "di\u{00E1}b");
}
#[test]
fn appvis_disabled() {
    assert_eq!(simulate_app("disabled"), "di\u{00E1}bled");
}
#[test]
fn appvis_dd() {
    assert_eq!(simulate_app("dd"), "\u{0111}");
}
#[test]
fn appvis_dad() {
    assert_eq!(simulate_app("dad"), "dad");
}

// ============================================================
// Horn propagation for "uo" + w
// ============================================================

#[test]
fn uow_to_uo_horn() {
    assert_eq!(type_and_get_text("uow"), "\u{01B0}\u{01A1}"); // ươ
}
#[test]
fn thuowng() {
    assert_eq!(type_and_get_text("thuowng"), "th\u{01B0}\u{01A1}ng"); // thương
}
#[test]
fn nuowcs() {
    assert_eq!(type_and_get_text("nuowcs"), "n\u{01B0}\u{1EDB}c"); // nước
}
#[test]
fn quow_qu_exception() {
    assert_eq!(type_and_get_text("quow"), "qu\u{01A1}"); // quơ
}

// ============================================================
// Restore on invalid word-break
// ============================================================

#[test]
fn valid_as_space_no_restore() {
    let (result, composed) = type_then_space("as");
    assert_eq!(composed, "\u{00E1}");
    assert_eq!(result, EngineResult::WordBreak);
}
#[test]
fn valid_viet_space_no_restore() {
    let (result, _) = type_then_space("viet");
    assert_eq!(result, EngineResult::WordBreak);
}
#[test]
fn invalid_wd_restore() {
    let (result, composed) = type_then_space("wd");
    assert_eq!(composed, "\u{01B0}d"); // ưd
    assert_eq!(
        result,
        EngineResult::Restore {
            backspaces: 2,
            text: "wd".to_string()
        }
    );
}
#[test]
fn invalid_aal_restore() {
    let (result, composed) = type_then_space("aal");
    assert_eq!(composed, "\u{00E2}l"); // âl
    assert_eq!(
        result,
        EngineResult::Restore {
            backspaces: 2,
            text: "aal".to_string()
        }
    );
}
#[test]
fn plain_hello_no_restore() {
    let (result, _) = type_then_space("hello");
    assert_eq!(result, EngineResult::WordBreak);
}
#[test]
fn case_preserved_on_restore() {
    let (result, _) = type_then_space("AAL");
    assert_eq!(
        result,
        EngineResult::Restore {
            backspaces: 2,
            text: "AAL".to_string()
        }
    );
}
#[test]
fn backspace_disables_restore() {
    let mut engine = TelexEngine::new();
    engine.is_vietnamese_mode = true;
    for c in "aa".chars() {
        engine.process_key(KeyClass::Letter(c), false, false, false);
    }
    engine.process_key(KeyClass::Backspace, false, false, false);
    engine.process_key(KeyClass::Letter('l'), false, false, false);
    let result = engine.process_key(KeyClass::WordBreak, false, false, false);
    assert_eq!(result, EngineResult::WordBreak);
}
#[test]
fn word_break_resets_after_restore() {
    let mut engine = TelexEngine::new();
    engine.is_vietnamese_mode = true;
    for c in "wd".chars() {
        engine.process_key(KeyClass::Letter(c), false, false, false);
    }
    engine.process_key(KeyClass::WordBreak, false, false, false);
    assert!(engine.buffer.is_empty());
}

// ============================================================
// Resume-on-backspace
// ============================================================

#[test]
fn resume_cai_backspace_j() {
    let mut engine = TelexEngine::new();
    engine.is_vietnamese_mode = true;
    for c in "cais".chars() {
        engine.process_key(KeyClass::Letter(c), false, false, false);
    }
    assert_eq!(engine.buffer.text(), "c\u{00E1}i"); // cái
    let space_result = engine.process_key(KeyClass::WordBreak, false, false, false);
    assert_eq!(space_result, EngineResult::WordBreak);
    assert!(engine.buffer.is_empty());
    let bs_result = engine.process_key(KeyClass::Backspace, false, false, false);
    assert_eq!(bs_result, EngineResult::PassThrough);
    assert_eq!(engine.buffer.text(), "c\u{00E1}i"); // cái restored
    engine.process_key(KeyClass::Letter('j'), false, false, false);
    assert_eq!(engine.buffer.text(), "c\u{1EA1}i"); // cại
}

#[test]
fn letter_after_break_discards_saved() {
    let mut engine = TelexEngine::new();
    engine.is_vietnamese_mode = true;
    for c in "cais".chars() {
        engine.process_key(KeyClass::Letter(c), false, false, false);
    }
    engine.process_key(KeyClass::WordBreak, false, false, false);
    engine.process_key(KeyClass::Letter('h'), false, false, false);
    assert_eq!(engine.buffer.text(), "h");
    let bs_result = engine.process_key(KeyClass::Backspace, false, false, false);
    assert_eq!(bs_result, EngineResult::PassThrough);
    assert!(engine.buffer.is_empty());
}

#[test]
fn double_word_break_clears_saved() {
    let mut engine = TelexEngine::new();
    engine.is_vietnamese_mode = true;
    for c in "cais".chars() {
        engine.process_key(KeyClass::Letter(c), false, false, false);
    }
    engine.process_key(KeyClass::WordBreak, false, false, false);
    engine.process_key(KeyClass::WordBreak, false, false, false);
    let bs_result = engine.process_key(KeyClass::Backspace, false, false, false);
    assert_eq!(bs_result, EngineResult::PassThrough);
    assert!(engine.buffer.is_empty());
}

#[test]
fn invalid_restore_no_resumable_state() {
    let mut engine = TelexEngine::new();
    engine.is_vietnamese_mode = true;
    for c in "wd".chars() {
        engine.process_key(KeyClass::Letter(c), false, false, false);
    }
    let space_result = engine.process_key(KeyClass::WordBreak, false, false, false);
    assert!(matches!(space_result, EngineResult::Restore { .. }));
    let bs_result = engine.process_key(KeyClass::Backspace, false, false, false);
    assert_eq!(bs_result, EngineResult::PassThrough);
    assert!(engine.buffer.is_empty());
}

// ============================================================
// Validity gating (English-word protection)
// ============================================================

#[test]
fn corr_to_cor() {
    assert_eq!(type_and_get_text("corr"), "cor");
}
#[test]
fn corrr_to_corr() {
    assert_eq!(type_and_get_text("corrr"), "corr");
}
#[test]
fn class_stays_class() {
    assert_eq!(type_and_get_text("class"), "class");
}
#[test]
fn know_stays_know() {
    assert_eq!(type_and_get_text("know"), "know");
}
#[test]
fn add_stays_add() {
    assert_eq!(type_and_get_text("add"), "add");
}
#[test]
fn ddoong_to_dong() {
    assert_eq!(type_and_get_text("ddoong"), "\u{0111}\u{00F4}ng"); // đông
}
#[test]
fn coffee_live() {
    assert_eq!(type_and_get_text("coffee"), "cofee");
}

// ============================================================
// Double-press escape (n+1 typing)
// ============================================================

#[test]
fn noww_space_now() {
    let (result, composed) = type_then_space("noww");
    assert_eq!(composed, "now");
    assert_eq!(result, EngineResult::WordBreak);
}
#[test]
fn hass_space_has() {
    let (result, composed) = type_then_space("hass");
    assert_eq!(composed, "has");
    assert_eq!(result, EngineResult::WordBreak);
}
#[test]
fn disst_space_dist() {
    let (result, composed) = type_then_space("disst");
    assert_eq!(composed, "dist");
    assert_eq!(result, EngineResult::WordBreak);
}
#[test]
fn tesst_space_test() {
    let (result, composed) = type_then_space("tesst");
    assert_eq!(composed, "test");
    assert_eq!(result, EngineResult::WordBreak);
}
#[test]
fn passs_space_pass() {
    let (result, composed) = type_then_space("passs");
    assert_eq!(composed, "pass");
    assert_eq!(result, EngineResult::WordBreak);
}
#[test]
fn corrrection_space() {
    let (result, composed) = type_then_space("corrrection");
    assert_eq!(composed, "correction");
    assert_eq!(result, EngineResult::WordBreak);
}
#[test]
fn cofffee_space() {
    let (result, composed) = type_then_space("cofffee");
    assert_eq!(composed, "coffee");
    assert_eq!(result, EngineResult::WordBreak);
}
#[test]
fn errror_space() {
    let (result, composed) = type_then_space("errror");
    assert_eq!(composed, "error");
    assert_eq!(result, EngineResult::WordBreak);
}
#[test]
fn ddw_to_dstroke_u_horn() {
    assert_eq!(type_and_get_text("ddw"), "\u{0111}\u{01B0}"); // đư
}
#[test]
fn ddwowngf_duong() {
    assert_eq!(type_and_get_text("ddwowngf"), "\u{0111}\u{01B0}\u{1EDD}ng"); // đường
}
#[test]
fn dduwowngf_duong() {
    assert_eq!(type_and_get_text("dduwowngf"), "\u{0111}\u{01B0}\u{1EDD}ng"); // đường
}
#[test]
fn ddwa_dua() {
    assert_eq!(type_and_get_text("ddwa"), "\u{0111}\u{01B0}a"); // đưa
}
#[test]
fn swift_stays_swift() {
    assert_eq!(type_and_get_text("swift"), "swift");
}
#[test]
fn dd_space_kept() {
    let (result, composed) = type_then_space("dd");
    assert_eq!(composed, "\u{0111}"); // đ
    assert_eq!(result, EngineResult::WordBreak);
}
#[test]
fn ddc_space_kept() {
    let (result, composed) = type_then_space("ddc");
    assert_eq!(composed, "\u{0111}c"); // đc
    assert_eq!(result, EngineResult::WordBreak);
}
#[test]
fn correction_2r_corection() {
    let (result, composed) = type_then_space("correction");
    assert_eq!(composed, "corection");
    assert_eq!(result, EngineResult::WordBreak);
}

// ============================================================
// Per-key trace vectors (exact backspace counts for tricky sequences).
// Final-text equality alone doesn't prove per-key backspace math.
// ============================================================

/// Drive the engine key-by-key, returning (result, backspaces, replacement text)
/// for each key so we can assert the exact injection contract.
fn trace(chars: &str) -> Vec<(String, usize, String)> {
    let mut engine = TelexEngine::new();
    engine.is_vietnamese_mode = true;
    let mut out = Vec::new();
    for c in chars.chars() {
        let shift = c.is_uppercase();
        let r = engine.process_key(key_for(c), shift, false, false);
        let tuple = match r {
            EngineResult::PassThrough => ("pass".to_string(), 0, String::new()),
            EngineResult::WordBreak => ("break".to_string(), 0, String::new()),
            EngineResult::Replace { backspaces, text } => ("replace".to_string(), backspaces, text),
            EngineResult::Restore { backspaces, text } => ("restore".to_string(), backspaces, text),
        };
        out.push(tuple);
    }
    out
}

#[test]
fn trace_hosa_tone_move() {
    // h,o -> pass. s -> replace 1 bs "ó" (hó). a -> tone moves: buffer "hó"->"hoá",
    // common prefix "h", delete "ó" (1), send "oá".
    let t = trace("hosa");
    assert_eq!(t[0], ("pass".to_string(), 0, String::new())); // h
    assert_eq!(t[1], ("pass".to_string(), 0, String::new())); // o
    assert_eq!(t[2], ("replace".to_string(), 1, "\u{00F3}".to_string())); // ó
    assert_eq!(t[3], ("replace".to_string(), 1, "o\u{00E1}".to_string())); // oá
}

#[test]
fn trace_dd_single_backspace() {
    // d -> pass. d -> dd->đ: old "d", new "đ", no common prefix, 1 backspace, "đ".
    let t = trace("dd");
    assert_eq!(t[0], ("pass".to_string(), 0, String::new()));
    assert_eq!(t[1], ("replace".to_string(), 1, "\u{0111}".to_string()));
}

#[test]
fn trace_ww_standalone() {
    // w -> replace 0 bs "ư" (standalone, no delete). w -> "ư"->"w": 1 bs, "w".
    let t = trace("ww");
    assert_eq!(t[0], ("replace".to_string(), 0, "\u{01B0}".to_string()));
    assert_eq!(t[1], ("replace".to_string(), 1, "w".to_string()));
}

#[test]
fn trace_uow_horn_propagation() {
    // u -> pass. o -> pass. w -> "uo"->"ươ": both change, 0 common prefix,
    // 2 backspaces, "ươ".
    let t = trace("uow");
    assert_eq!(t[0], ("pass".to_string(), 0, String::new()));
    assert_eq!(t[1], ("pass".to_string(), 0, String::new()));
    assert_eq!(
        t[2],
        ("replace".to_string(), 2, "\u{01B0}\u{01A1}".to_string())
    );
}

#[test]
fn trace_wd_restore() {
    // w -> replace 0 "ư". d -> pass (ưd). space -> restore 2 "wd".
    let mut engine = TelexEngine::new();
    engine.is_vietnamese_mode = true;
    let r0 = engine.process_key(KeyClass::Letter('w'), false, false, false);
    assert_eq!(
        r0,
        EngineResult::Replace {
            backspaces: 0,
            text: "\u{01B0}".to_string()
        }
    );
    let r1 = engine.process_key(KeyClass::Letter('d'), false, false, false);
    assert_eq!(r1, EngineResult::PassThrough);
    let r2 = engine.process_key(KeyClass::WordBreak, false, false, false);
    assert_eq!(
        r2,
        EngineResult::Restore {
            backspaces: 2,
            text: "wd".to_string()
        }
    );
}

#[test]
fn trace_output_is_single_scalar() {
    // Every Replace/Restore text the engine emits must be single-scalar BMP.
    for word in ["thuowng", "nuowcs", "ddwowngf", "hosa", "Vieejt", "quow", "dduwowngf"] {
        let mut engine = TelexEngine::new();
        engine.is_vietnamese_mode = true;
        for c in word.chars() {
            let shift = c.is_uppercase();
            let r = engine.process_key(key_for(c), shift, false, false);
            if let EngineResult::Replace { text, .. } | EngineResult::Restore { text, .. } = r {
                novakey_core::assert_single_scalar_output(&text);
            }
        }
    }
}
