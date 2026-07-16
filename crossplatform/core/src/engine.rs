//! engine.rs
//! Core Telex input processing engine — port of TelexEngine.swift.
//! Pure logic, no OS dependencies — fully testable.

use crate::buffer::SyllableBuffer;
use crate::data::{telex_tone_key, ToneMark, ViChar, VowelModifier, TELEX_VOWEL_MODIFIERS};
use crate::spelling::is_valid_syllable;
use crate::tone::find_tone_position;

// MARK: - Key Classification

/// A platform-neutral classification of an incoming key.
/// The Windows `vk.rs` (and any other frontend) maps raw key events into this
/// before the engine sees them, so the engine stays OS-agnostic.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum KeyClass {
    /// A letter A-Z, carried as its lowercase ASCII form. Case comes from `shift`.
    Letter(char),
    /// The Backspace/Delete key.
    Backspace,
    /// A key that ends the current syllable (space, return, punctuation, arrows…).
    WordBreak,
    /// Any other key the engine doesn't compose with.
    Other,
}

// MARK: - Engine Result

/// The result of processing a single keystroke through the engine.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum EngineResult {
    /// Let the original key event pass through unchanged.
    PassThrough,
    /// Suppress the original key and send `backspaces` backspaces then `text`.
    Replace { backspaces: usize, text: String },
    /// The key is a word break — reset the session and let it through.
    WordBreak,
    /// Emit a replacement (backspace + raw text) THEN let the original key pass.
    /// Used when the accumulated syllable is invalid at word-break time.
    Restore { backspaces: usize, text: String },
}

// MARK: - Telex Engine

/// Processes keystrokes according to the Telex input method.
pub struct TelexEngine {
    /// The current syllable being composed.
    pub buffer: SyllableBuffer,
    /// Whether Vietnamese mode is active.
    pub is_vietnamese_mode: bool,
    /// "Quick Vietnamese" (Windows-only): a lone `w` typed right after a valid
    /// syllable-initial consonant (cluster) becomes `ư`, e.g. "tw" -> "tư",
    /// "chw" -> "chư". Off by default; opted in from Settings.
    pub quick_vietnamese: bool,

    /// The last raw (lowercase) key typed, for double-press detection.
    last_raw_key: Option<char>,
    /// Raw letter keystrokes typed this session, with original case.
    raw_keystrokes: String,

    /// The syllable state at the moment of the most recent word-break.
    saved_buffer: Option<SyllableBuffer>,
    saved_raw_keystrokes: String,
}

impl Default for TelexEngine {
    fn default() -> Self {
        TelexEngine::new()
    }
}

impl TelexEngine {
    pub fn new() -> TelexEngine {
        TelexEngine {
            buffer: SyllableBuffer::new(),
            is_vietnamese_mode: true,
            quick_vietnamese: false,
            last_raw_key: None,
            raw_keystrokes: String::new(),
            saved_buffer: None,
            saved_raw_keystrokes: String::new(),
        }
    }

    // MARK: - Main Entry Point

    /// Process a single keystroke.
    ///
    /// - `key`: the platform-neutral key classification.
    /// - `shift`: whether Shift is held.
    /// - `ctrl_or_cmd`: whether Ctrl (Windows) / Cmd or Ctrl (macOS) is held.
    /// - `alt`: whether Alt / Option is held.
    pub fn process_key(
        &mut self,
        key: KeyClass,
        shift: bool,
        ctrl_or_cmd: bool,
        alt: bool,
    ) -> EngineResult {
        // Modifier combos (Ctrl+C, etc.) always pass through and reset.
        if ctrl_or_cmd {
            self.reset_session();
            return EngineResult::PassThrough;
        }
        // Alt / Option combos pass through and reset.
        if alt {
            self.reset_session();
            return EngineResult::PassThrough;
        }

        match key {
            KeyClass::WordBreak => {
                // Word break: check for invalid syllable and restore if needed.
                if let Some(restore) = self.restore_if_invalid() {
                    // After an invalid-syllable restore the visible text is the
                    // raw keys, so we cannot reliably resume on backspace.
                    self.saved_buffer = None;
                    self.reset_session();
                    return restore;
                }
                // Save the completed syllable so the user can re-enter it by
                // pressing backspace to delete the word-break character.
                if !self.buffer.is_empty() {
                    self.saved_buffer = Some(self.buffer.clone());
                    self.saved_raw_keystrokes = self.raw_keystrokes.clone();
                } else {
                    self.saved_buffer = None;
                }
                self.reset_session();
                EngineResult::WordBreak
            }

            KeyClass::Backspace => {
                // If we just word-broke with a non-empty syllable, the user is
                // stepping back into that word — rehydrate the buffer.
                if self.buffer.is_empty() && self.saved_buffer.is_some() {
                    let saved = self.saved_buffer.take().unwrap();
                    self.buffer = saved;
                    self.raw_keystrokes = self.saved_raw_keystrokes.clone();
                    self.last_raw_key = self
                        .saved_raw_keystrokes
                        .chars()
                        .last()
                        .map(|c| c.to_ascii_lowercase());
                    return EngineResult::PassThrough;
                }
                self.handle_backspace()
            }

            KeyClass::Other => {
                // Any other key following a word-break discards the saved state.
                self.saved_buffer = None;
                if !self.is_vietnamese_mode {
                    return EngineResult::PassThrough;
                }
                // Non-letter key that isn't a word break — reset and pass through.
                self.reset_session();
                EngineResult::PassThrough
            }

            KeyClass::Letter(lower) => {
                // Starting/continuing a word discards any saved word-break state.
                self.saved_buffer = None;

                if !self.is_vietnamese_mode {
                    return EngineResult::PassThrough;
                }

                let ch = if shift { lower.to_ascii_uppercase() } else { lower };

                // Record the raw keystroke (with original case) for restoration.
                self.raw_keystrokes.push(ch);

                self.process_letter(ch, shift)
            }
        }
    }

    /// Reset the syllable buffer and start fresh.
    pub fn reset_session(&mut self) {
        self.buffer.reset();
        self.last_raw_key = None;
        self.raw_keystrokes.clear();
    }

    /// Enable/disable "Quick Vietnamese" (w-after-initial-consonant -> ư).
    /// Persists across `reset_session`, so it survives word breaks.
    pub fn set_quick_vietnamese(&mut self, on: bool) {
        self.quick_vietnamese = on;
    }

    /// Whether the buffer currently holds exactly one valid syllable-initial
    /// consonant cluster and no vowels yet — the precondition for Quick
    /// Vietnamese turning a following `w` into `ư`.
    fn is_quick_initial(&self) -> bool {
        if self.buffer.vowel_count() != 0 || self.buffer.is_empty() {
            return false;
        }
        matches!(
            self.buffer.text().to_lowercase().as_str(),
            "b" | "c" | "d" | "đ" | "g" | "h" | "l" | "m" | "n" | "r" | "s" | "t" | "v" | "x"
                | "ch" | "kh" | "ng" | "nh" | "ph" | "th" | "tr"
        )
    }

    // MARK: - Letter Processing

    fn process_letter(&mut self, ch: char, is_upper: bool) -> EngineResult {
        let lower = ch.to_ascii_lowercase();

        // Telex tone key (s, f, r, x, j, z).
        if let Some(tone) = telex_tone_key(lower) {
            if self.buffer.vowel_count() > 0 {
                if let Some(result) = self.handle_tone_key(lower, tone, is_upper) {
                    self.last_raw_key = Some(lower);
                    return result;
                }
            }
        }

        // d-stroke trigger (dd -> đ).
        if lower == 'd' {
            let result = self.handle_d_key(is_upper);
            self.last_raw_key = Some('d');
            return result;
        }

        // Vowel modifier trigger (aa, ee, oo, aw, ow, uw, w).
        if lower == 'w' || self.is_double_key_trigger(lower) {
            if let Some(result) = self.handle_vowel_modifier(lower, is_upper) {
                self.last_raw_key = Some(lower);
                return result;
            }
        }

        // Regular letter — add to buffer. Capture the pre-append text so a
        // re-check can compute a correct backspace-and-replace diff.
        let pre_append_text = self.buffer.text();
        let vi_char = ViChar::with(lower, VowelModifier::Plain, ToneMark::None, is_upper);
        self.buffer.append(vi_char);

        let mut transformed = false;

        // Forward "ươ" linking (Windows enhancement, not in the Swift engine):
        // an 'o' typed right after a horned 'ư' auto-takes the horn, because ư
        // and ơ always co-occur in Vietnamese (there is no valid "ư"+plain-"o").
        // This makes the "type uw first" style work: "thuwong" -> "thương",
        // "tuwongr" -> "tưởng". A later explicit 'w' on this 'o' confirms rather
        // than undoes the horn (see handle_vowel_modifier), so the canonical
        // two-'w' style "thuwowng" still lands on the same result.
        let last = self.buffer.count() - 1;
        if lower == 'o' && last >= 1 {
            let prev = self.buffer.chars[last - 1];
            if prev.base == 'u'
                && prev.modifier == VowelModifier::Horn
                && self.buffer.chars[last].modifier == VowelModifier::Plain
            {
                self.buffer.apply_modifier(VowelModifier::Horn, last);
                self.buffer.chars[last].auto_horn = true;
                transformed = true;
            }
        }

        // After adding any letter, re-check tone placement. Matters for vowel
        // appends: "hos" -> "hó", then "a" -> "hoá" (tone moves to 2nd vowel).
        if self.buffer.current_tone != ToneMark::None {
            if let Some(current_tone_idx) = self.buffer.tone_index {
                if let Some(new_position) = find_tone_position(&self.buffer) {
                    if new_position != current_tone_idx {
                        self.buffer.move_tone(new_position);
                        transformed = true;
                    }
                }
            }
        }

        self.last_raw_key = Some(lower);
        if transformed {
            return self.build_replacement(&pre_append_text, &self.buffer.text());
        }
        EngineResult::PassThrough
    }

    // MARK: - Tone Handling

    /// Handle a tone key press (s, f, r, x, j, z). Returns None to treat as a
    /// regular letter.
    fn handle_tone_key(&mut self, key: char, tone: ToneMark, is_upper: bool) -> Option<EngineResult> {
        // z key: remove existing tone.
        if tone == ToneMark::None {
            return self.handle_remove_tone();
        }

        // Same tone already applied -> undo it (double-press reversal).
        if self.buffer.current_tone == tone {
            return Some(self.undo_tone(key, is_upper));
        }

        // Only reinterpret the key as a tone mark when the syllable is
        // structurally valid Vietnamese (keeps English words literal).
        if !is_valid_syllable(&self.buffer) {
            return None;
        }

        let position = find_tone_position(&self.buffer)?;

        let old_text = self.buffer.text();
        self.buffer.apply_tone(tone, position);
        let new_text = self.buffer.text();
        Some(self.build_replacement(&old_text, &new_text))
    }

    /// Handle z key: remove existing tone mark.
    fn handle_remove_tone(&mut self) -> Option<EngineResult> {
        if self.buffer.current_tone == ToneMark::None {
            return None;
        }
        let old_text = self.buffer.text();
        self.buffer.apply_tone(ToneMark::None, 0); // index irrelevant for None
        let new_text = self.buffer.text();
        Some(self.build_replacement(&old_text, &new_text))
    }

    /// Undo a tone mark when the same tone key is pressed again ("as" -> "á", "ass" -> "as").
    fn undo_tone(&mut self, key: char, is_upper: bool) -> EngineResult {
        let old_text = self.buffer.text();
        self.buffer.apply_tone(ToneMark::None, 0);
        self.buffer
            .append(ViChar::with(key, VowelModifier::Plain, ToneMark::None, is_upper));
        let new_text = self.buffer.text();
        self.build_replacement(&old_text, &new_text)
    }

    // MARK: - D-Stroke Handling

    /// Handle the 'd' key. dd -> đ only syllable-initially.
    fn handle_d_key(&mut self, is_upper: bool) -> EngineResult {
        // dd -> đ only when the 'd' is the sole char so far (đ is syllable-initial).
        if let Some(last) = self.buffer.chars.last() {
            if last.base == 'd' && !last.has_dstroke && self.buffer.count() == 1 {
                let old_text = self.buffer.text();
                let idx = self.buffer.count() - 1;
                self.buffer.apply_dstroke(idx);
                let new_text = self.buffer.text();
                return self.build_replacement(&old_text, &new_text);
            }
        }

        // Already has đ and typing another d -> undo (đd -> dd).
        if self.buffer.has_dstroke && self.last_raw_key == Some('d') {
            let old_text = self.buffer.text();
            self.buffer.remove_dstroke();
            self.buffer
                .append(ViChar::with('d', VowelModifier::Plain, ToneMark::None, is_upper));
            let new_text = self.buffer.text();
            return self.build_replacement(&old_text, &new_text);
        }

        // Regular d.
        self.buffer
            .append(ViChar::with('d', VowelModifier::Plain, ToneMark::None, is_upper));
        EngineResult::PassThrough
    }

    // MARK: - Vowel Modifier Handling

    /// Whether typing this char triggers a double-key modifier (aa->â, ee->ê, oo->ô).
    fn is_double_key_trigger(&self, ch: char) -> bool {
        match self.last_raw_key {
            None => false,
            Some(last) => last == ch && (ch == 'a' || ch == 'e' || ch == 'o'),
        }
    }

    /// Handle vowel modifier keys. Returns None if no modification can be applied.
    fn handle_vowel_modifier(&mut self, key: char, is_upper: bool) -> Option<EngineResult> {
        // "ww" -> literal "w": first 'w' on empty buffer becomes 'ư' (standalone);
        // pressing 'w' again reverts to literal 'w'.
        if key == 'w'
            && self.buffer.count() == 1
            && self.buffer.chars[0].base == 'u'
            && self.buffer.chars[0].modifier == VowelModifier::Horn
            && self.raw_keystrokes.to_lowercase() == "ww"
        {
            let old_text = self.buffer.text();
            self.buffer.reset();
            self.buffer
                .append(ViChar::with('w', VowelModifier::Plain, ToneMark::None, is_upper));
            let new_text = self.buffer.text();
            return Some(self.build_replacement(&old_text, &new_text));
        }

        // Special case: "ua" + w -> "ưa" (horn on u, a stays plain).
        if key == 'w' && is_valid_syllable(&self.buffer) {
            if let Some(a_idx) = self.buffer.last_index_of_base('a') {
                if a_idx > 0 && self.buffer.chars[a_idx].modifier == VowelModifier::Plain {
                    let prev = self.buffer.chars[a_idx - 1];
                    let preceded_by_q = a_idx >= 2 && self.buffer.chars[a_idx - 2].base == 'q';
                    if prev.base == 'u' && prev.modifier == VowelModifier::Plain && !preceded_by_q {
                        let old_text = self.buffer.text();
                        self.buffer.apply_modifier(VowelModifier::Horn, a_idx - 1);
                        let new_text = self.buffer.text();
                        return Some(self.build_replacement(&old_text, &new_text));
                    }
                }
            }
        }

        // Try each modifier rule.
        for &(trigger, target, modifier) in TELEX_VOWEL_MODIFIERS {
            if trigger == key {
                if let Some(target_idx) = self.buffer.last_index_of_base(target) {
                    let current_mod = self.buffer.chars[target_idx].modifier;

                    // Already has this modifier -> undo (double-press reversal),
                    // EXCEPT: a 'w' on an 'o' whose horn was auto-applied by
                    // forward "ươ" linking is redundant confirmation, not an
                    // escape — consume it and keep the ơ. This lets the
                    // canonical two-'w' style ("thuwowng") converge with the
                    // "uw first" style ("thuwong").
                    if current_mod == modifier {
                        if target == 'o' && self.buffer.chars[target_idx].auto_horn {
                            self.buffer.chars[target_idx].auto_horn = false;
                            return Some(EngineResult::Replace {
                                backspaces: 0,
                                text: String::new(),
                            });
                        }
                        return Some(self.undo_vowel_modifier(key, target_idx, is_upper));
                    }

                    // No modifier yet -> apply, only if syllable is valid Vietnamese.
                    if current_mod == VowelModifier::Plain {
                        if !is_valid_syllable(&self.buffer) {
                            return None;
                        }
                        let old_text = self.buffer.text();
                        self.buffer.apply_modifier(modifier, target_idx);
                        // "uo" + w -> "ươ": propagate horn to a preceding unmodified
                        // 'u' (except after "qu").
                        if modifier == VowelModifier::Horn && target == 'o' && target_idx > 0 {
                            let prev = self.buffer.chars[target_idx - 1];
                            let preceded_by_q =
                                target_idx >= 2 && self.buffer.chars[target_idx - 2].base == 'q';
                            if prev.base == 'u'
                                && prev.modifier == VowelModifier::Plain
                                && !preceded_by_q
                            {
                                self.buffer.apply_modifier(VowelModifier::Horn, target_idx - 1);
                            }
                        }
                        let new_text = self.buffer.text();
                        return Some(self.build_replacement(&old_text, &new_text));
                    }
                }
            }
        }

        // Standalone 'w' -> 'ư' when there's no target vowel to modify.
        // Also allowed right after a syllable-initial đ ("ddw" -> "đư"), and —
        // with Quick Vietnamese on — right after any valid initial consonant
        // cluster ("tw" -> "tư", "chw" -> "chư").
        if key == 'w'
            && (self.buffer.is_empty()
                || (self.buffer.has_dstroke && self.buffer.vowel_count() == 0)
                || (self.quick_vietnamese && self.is_quick_initial()))
        {
            let vi_char = ViChar::with('u', VowelModifier::Horn, ToneMark::None, is_upper);
            self.buffer.append(vi_char);
            return Some(EngineResult::Replace {
                backspaces: 0,
                text: vi_char.unicode().to_string(),
            });
        }

        None
    }

    /// Undo a vowel modifier when the same trigger is pressed again ("aa" -> "â", "aaa" -> "aa").
    fn undo_vowel_modifier(&mut self, key: char, target_index: usize, is_upper: bool) -> EngineResult {
        let old_text = self.buffer.text();
        self.buffer.remove_modifier(target_index);
        self.buffer
            .append(ViChar::with(key, VowelModifier::Plain, ToneMark::None, is_upper));
        let new_text = self.buffer.text();
        self.build_replacement(&old_text, &new_text)
    }

    // MARK: - Backspace Handling

    fn handle_backspace(&mut self) -> EngineResult {
        self.buffer.remove_last();
        // Once the user corrects mid-word, we can't reconstruct the original
        // raw keystrokes, so disable the restore path for this session.
        self.raw_keystrokes.clear();
        EngineResult::PassThrough
    }

    // MARK: - Spelling Restore

    /// If the composed text diverged from the raw keystrokes and does not form a
    /// structurally valid syllable, return a restore result. None otherwise.
    fn restore_if_invalid(&self) -> Option<EngineResult> {
        if self.buffer.is_empty() {
            return None;
        }
        if self.raw_keystrokes.is_empty() {
            return None;
        }

        // Only restore when a tone or vowel-modifier transformation is visible.
        // (đ deliberately does NOT count — it only fires syllable-initially and
        // a vowel-less "đ" is legitimate on its own.)
        let has_transformation = self.buffer.chars.iter().any(|c| {
            c.modifier != VowelModifier::Plain || c.tone != ToneMark::None
        });
        if !has_transformation {
            return None;
        }

        // Don't restore if the syllable is structurally valid.
        if is_valid_syllable(&self.buffer) {
            return None;
        }

        // Nothing to fix if the screen already shows the raw keystrokes.
        let composed = self.buffer.text();
        if composed == self.raw_keystrokes {
            return None;
        }

        Some(EngineResult::Restore {
            backspaces: composed.chars().count(),
            text: self.raw_keystrokes.clone(),
        })
    }

    // MARK: - Replacement Building

    /// Build an `EngineResult::Replace` by comparing old and new buffer text.
    /// Calculates the minimum number of backspaces needed. All counts are in
    /// Unicode scalars (`chars()`), mirroring Swift's grapheme-safe `.count`
    /// for our single-scalar NFC output.
    fn build_replacement(&self, old_text: &str, new_text: &str) -> EngineResult {
        // Common prefix length (in chars).
        let common_prefix = old_text
            .chars()
            .zip(new_text.chars())
            .take_while(|(a, b)| a == b)
            .count();

        let old_len = old_text.chars().count();
        let backspaces = old_len - common_prefix;

        let new_suffix: String = new_text.chars().skip(common_prefix).collect();

        if backspaces == 0 && new_suffix.is_empty() {
            return EngineResult::PassThrough;
        }

        EngineResult::Replace {
            backspaces,
            text: new_suffix,
        }
    }
}
