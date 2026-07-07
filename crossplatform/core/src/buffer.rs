//! buffer.rs
//! Tracks the current Vietnamese syllable being typed — port of SyllableBuffer.swift.

use crate::data::{ToneMark, ViChar, VowelModifier};

/// Maximum buffer size (Vietnamese syllables are ~7 chars; extra room for sequences).
const MAX_BUFFER_SIZE: usize = 32;

/// Manages the character buffer for the current syllable being composed.
#[derive(Clone, Debug, Default)]
pub struct SyllableBuffer {
    /// The resolved Vietnamese characters in the current syllable.
    pub chars: Vec<ViChar>,
    /// Whether the 'd' has been converted to 'đ'.
    pub has_dstroke: bool,
    /// Index of the 'd' character in `chars` (if present and converted).
    pub dstroke_index: Option<usize>,
    /// The current tone mark applied to the syllable.
    pub current_tone: ToneMark,
    /// Index in `chars` where the tone mark is placed.
    pub tone_index: Option<usize>,
}

impl SyllableBuffer {
    pub fn new() -> SyllableBuffer {
        SyllableBuffer {
            chars: Vec::new(),
            has_dstroke: false,
            dstroke_index: None,
            current_tone: ToneMark::None,
            tone_index: None,
        }
    }

    // MARK: - Computed Properties

    pub fn is_empty(&self) -> bool {
        self.chars.is_empty()
    }

    pub fn count(&self) -> usize {
        self.chars.len()
    }

    /// The full Unicode string represented by the current buffer.
    pub fn text(&self) -> String {
        self.chars.iter().map(|c| c.unicode()).collect()
    }

    /// Indices of all vowel characters in the buffer.
    pub fn vowel_indices(&self) -> Vec<usize> {
        self.chars
            .iter()
            .enumerate()
            .filter(|(_, c)| c.is_vowel())
            .map(|(i, _)| i)
            .collect()
    }

    /// Number of vowels in the buffer.
    pub fn vowel_count(&self) -> usize {
        self.chars.iter().filter(|c| c.is_vowel()).count()
    }

    /// Index of the first vowel, or None.
    pub fn first_vowel_index(&self) -> Option<usize> {
        self.chars.iter().position(|c| c.is_vowel())
    }

    /// Index of the last vowel, or None.
    pub fn last_vowel_index(&self) -> Option<usize> {
        self.chars.iter().rposition(|c| c.is_vowel())
    }

    /// Whether the syllable currently ends with a consonant (after the vowel cluster).
    pub fn has_ending_consonant(&self) -> bool {
        match self.last_vowel_index() {
            None => false,
            Some(last_vowel) => self.chars[(last_vowel + 1)..].iter().any(|c| !c.is_vowel()),
        }
    }

    /// The ending consonant string (e.g., "ng", "nh", "ch", "n", "m", "t", "c", "p").
    pub fn ending_consonant(&self) -> String {
        match self.last_vowel_index() {
            None => String::new(),
            Some(last_vowel) => self.chars[(last_vowel + 1)..].iter().map(|c| c.base).collect(),
        }
    }

    /// The initial consonant string (everything before the first vowel).
    pub fn initial_consonant(&self) -> String {
        match self.first_vowel_index() {
            None => self.chars.iter().map(|c| c.base).collect(),
            Some(first_vowel) => self.chars[..first_vowel].iter().map(|c| c.base).collect(),
        }
    }

    /// The vowel cluster as lowercase base characters.
    pub fn vowel_cluster(&self) -> String {
        self.vowel_indices().iter().map(|&i| self.chars[i].base).collect()
    }

    // MARK: - Mutations

    /// Append a character to the buffer. Returns false if the buffer is full.
    pub fn append(&mut self, ch: ViChar) -> bool {
        if self.chars.len() >= MAX_BUFFER_SIZE {
            return false;
        }
        self.chars.push(ch);
        true
    }

    /// Remove and return the last character from the buffer.
    pub fn remove_last(&mut self) -> Option<ViChar> {
        if self.chars.is_empty() {
            return None;
        }
        let removed = self.chars.pop();

        // If we removed the character that had the tone, clear tone tracking.
        if let Some(ti) = self.tone_index {
            if ti >= self.chars.len() {
                self.tone_index = None;
                self.current_tone = ToneMark::None;
            }
        }

        // If we removed the đ character, clear d-stroke tracking.
        if let Some(di) = self.dstroke_index {
            if di >= self.chars.len() {
                self.dstroke_index = None;
                self.has_dstroke = false;
            }
        }

        removed
    }

    /// Reset the buffer to empty state.
    pub fn reset(&mut self) {
        self.chars.clear();
        self.has_dstroke = false;
        self.dstroke_index = None;
        self.current_tone = ToneMark::None;
        self.tone_index = None;
    }

    /// Apply a tone mark at the specified index. Removes any existing tone first.
    pub fn apply_tone(&mut self, tone: ToneMark, index: usize) {
        // Remove old tone if present.
        if let Some(old_index) = self.tone_index {
            if old_index < self.chars.len() {
                self.chars[old_index].tone = ToneMark::None;
            }
        }

        if tone == ToneMark::None {
            self.tone_index = None;
            self.current_tone = ToneMark::None;
        } else {
            if index >= self.chars.len() || !self.chars[index].is_vowel() {
                return;
            }
            self.chars[index].tone = tone;
            self.tone_index = Some(index);
            self.current_tone = tone;
        }
    }

    /// Apply a vowel modifier (circumflex, breve, horn) at the specified index.
    pub fn apply_modifier(&mut self, modifier: VowelModifier, index: usize) {
        if index >= self.chars.len() {
            return;
        }
        self.chars[index].modifier = modifier;
    }

    /// Remove the vowel modifier at the specified index, restoring it to plain.
    pub fn remove_modifier(&mut self, index: usize) {
        if index >= self.chars.len() {
            return;
        }
        self.chars[index].modifier = VowelModifier::Plain;
        self.chars[index].auto_horn = false;
    }

    /// Mark the character at `index` as d-stroke (đ).
    pub fn apply_dstroke(&mut self, index: usize) {
        if index >= self.chars.len() || self.chars[index].base != 'd' {
            return;
        }
        self.chars[index].has_dstroke = true;
        self.has_dstroke = true;
        self.dstroke_index = Some(index);
    }

    /// Remove d-stroke, reverting đ back to d.
    pub fn remove_dstroke(&mut self) {
        if let Some(idx) = self.dstroke_index {
            if idx < self.chars.len() {
                self.chars[idx].has_dstroke = false;
            }
        }
        self.has_dstroke = false;
        self.dstroke_index = None;
    }

    /// Move the tone mark to a new index (used during grammar re-check).
    pub fn move_tone(&mut self, new_index: usize) {
        if self.current_tone == ToneMark::None {
            return;
        }
        self.apply_tone(self.current_tone, new_index);
    }

    // MARK: - Query

    /// Find the last character matching a given base letter (case-insensitive).
    pub fn last_index_of_base(&self, base: char) -> Option<usize> {
        let lower = base.to_ascii_lowercase();
        self.chars.iter().rposition(|c| c.base == lower)
    }
}
