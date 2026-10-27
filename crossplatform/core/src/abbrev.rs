//! abbrev.rs
//! User-defined abbreviations (Vietnamese: "gõ tắt"): a short form typed and
//! followed by Tab expands to its full text — "btw" -> "by the way".
//!
//! Pure logic, like the rest of the core: the table, its case rule, the text
//! file it is stored in, and the word tracker the frontend uses while
//! Vietnamese composition is off. The engine side (expanding the word it is
//! composing) is `TelexEngine::expand_abbreviation`.

use std::collections::BTreeMap;

use crate::engine::EngineResult;

/// Shortest abbreviation, in letters.
pub const MIN_KEY_LEN: usize = 2;
/// Longest abbreviation, in letters.
pub const MAX_KEY_LEN: usize = 32;
/// Longest expansion, in characters.
pub const MAX_EXPANSION_LEN: usize = 500;
/// Most entries the table holds, so a hand-edited file cannot grow it without
/// bound.
pub const MAX_ENTRIES: usize = 2000;

/// Why an entry was refused.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AbbrevError {
    /// Not 2–32 letters (digits, spaces and punctuation are not allowed).
    InvalidKey,
    /// Empty, too long, or contains a tab or line break.
    InvalidExpansion,
    /// The table already holds `MAX_ENTRIES` entries.
    Full,
}

/// The user's abbreviation table. Keys match case-insensitively; each entry
/// keeps the key as the user wrote it, for display and for the case rule.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Abbreviations {
    /// Lowercased key -> (key as written, expansion). A `BTreeMap` keeps the
    /// list sorted for the Settings window.
    entries: BTreeMap<String, (String, String)>,
}

impl Abbreviations {
    pub fn new() -> Abbreviations {
        Abbreviations::default()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Entries as (key, expansion), sorted by key.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.entries.values().map(|(k, e)| (k.as_str(), e.as_str()))
    }

    /// The expansion stored for `key` (any case).
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries.get(&key.trim().to_lowercase()).map(|(_, e)| e.as_str())
    }

    /// Add an entry, replacing any existing one with the same key (any case).
    pub fn insert(&mut self, key: &str, expansion: &str) -> Result<(), AbbrevError> {
        let key = normalize_key(key).ok_or(AbbrevError::InvalidKey)?;
        let expansion = normalize_expansion(expansion).ok_or(AbbrevError::InvalidExpansion)?;
        let lower = key.to_lowercase();
        if !self.entries.contains_key(&lower) && self.entries.len() >= MAX_ENTRIES {
            return Err(AbbrevError::Full);
        }
        self.entries.insert(lower, (key, expansion));
        Ok(())
    }

    /// Remove the entry for `key` (any case). Returns whether one existed.
    pub fn remove(&mut self, key: &str) -> bool {
        self.entries.remove(&key.trim().to_lowercase()).is_some()
    }

    /// The text `typed` expands to, cased to follow how it was typed (see
    /// `apply_case`), or `None` if it is not an abbreviation.
    pub fn expand(&self, typed: &str) -> Option<String> {
        let (key, expansion) = self.entries.get(&typed.to_lowercase())?;
        Some(apply_case(typed, key, expansion))
    }

    /// The replacement for a word typed while Vietnamese composition is off:
    /// what is on screen is exactly the letters typed.
    pub fn expand_word(&self, word: &str) -> Option<EngineResult> {
        let text = self.expand(word)?;
        Some(EngineResult::Restore {
            backspaces: word.chars().count(),
            text,
        })
    }

    /// Parse the stored file: one `key<TAB>expansion` per line. Blank lines,
    /// `#` comments and invalid lines are skipped, so a hand edit gone wrong
    /// loses that line rather than the whole list.
    pub fn parse(text: &str) -> Abbreviations {
        let mut table = Abbreviations::new();
        for line in text.lines() {
            let line = line.trim_start_matches('\u{FEFF}');
            if line.trim().is_empty() || line.trim_start().starts_with('#') {
                continue;
            }
            if let Some((key, expansion)) = line.split_once('\t') {
                let _ = table.insert(key, expansion);
            }
        }
        table
    }

    /// The file format `parse` reads.
    pub fn serialize(&self) -> String {
        let mut out = String::from(
            "# NovaKey abbreviations: one per line, abbreviation<TAB>expansion\n",
        );
        for (key, expansion) in self.iter() {
            out.push_str(key);
            out.push('\t');
            out.push_str(expansion);
            out.push('\n');
        }
        out
    }
}

/// A valid key, trimmed: 2–32 letters (Vietnamese letters included).
pub fn normalize_key(key: &str) -> Option<String> {
    let key = key.trim();
    let n = key.chars().count();
    let valid = (MIN_KEY_LEN..=MAX_KEY_LEN).contains(&n) && key.chars().all(char::is_alphabetic);
    valid.then(|| key.to_string())
}

/// A valid expansion, trimmed: non-empty, single-line, at most
/// `MAX_EXPANSION_LEN` characters. Tabs are refused because they separate the
/// fields of the stored file.
pub fn normalize_expansion(expansion: &str) -> Option<String> {
    let expansion = expansion.trim();
    let valid = !expansion.is_empty()
        && expansion.chars().count() <= MAX_EXPANSION_LEN
        && !expansion.chars().any(char::is_control);
    valid.then(|| expansion.to_string())
}

/// Case the expansion to follow how the key was typed:
/// - exactly as defined -> the expansion as defined (`NZ` -> `New Zealand`);
/// - all capitals -> all capitals (`BTW` -> `BY THE WAY`);
/// - first capital only -> first letter capitalized (`Btw` -> `By the way`);
/// - anything else -> the expansion as defined.
fn apply_case(typed: &str, key: &str, expansion: &str) -> String {
    if typed == key {
        return expansion.to_string();
    }
    let mut chars = typed.chars();
    let first_upper = chars.next().is_some_and(char::is_uppercase);
    let rest: Vec<char> = chars.collect();
    if first_upper && rest.iter().all(|c| c.is_uppercase()) {
        return expansion.to_uppercase();
    }
    if first_upper && rest.iter().all(|c| c.is_lowercase()) {
        let mut out = expansion.chars();
        return match out.next() {
            Some(c) => c.to_uppercase().chain(out).collect(),
            None => String::new(),
        };
    }
    expansion.to_string()
}

/// Tracks the word being typed for abbreviation lookup. The frontend feeds it
/// every key; it knows whether the current word started at a clean boundary
/// (Space, Enter, a click, a shortcut…) — an abbreviation glued to a digit or
/// punctuation ("x.btw", "2btw") must not expand — and, for when Vietnamese
/// composition is off, the letters typed (which are then exactly what is on
/// screen).
#[derive(Clone, Debug)]
pub struct WordTracker {
    word: String,
    clean: bool,
    /// The clean word a Space just ended, so a Backspace that deletes that
    /// Space steps back into it.
    saved: Option<String>,
    /// Characters known to sit between a clean boundary and the caret: an
    /// expansion NovaKey typed, plus any letters typed after it. Backspacing
    /// through all of them returns to that boundary.
    tail: usize,
}

impl Default for WordTracker {
    fn default() -> Self {
        WordTracker::new()
    }
}

impl WordTracker {
    pub fn new() -> WordTracker {
        WordTracker {
            word: String::new(),
            clean: true,
            saved: None,
            tail: 0,
        }
    }

    /// A letter was typed (with its case).
    pub fn letter(&mut self, ch: char) {
        self.saved = None;
        if self.tail > 0 {
            // Glued to an expansion: known, but not a word that can expand.
            self.tail += 1;
            return;
        }
        if !self.clean {
            return;
        }
        if self.word.chars().count() >= MAX_KEY_LEN {
            // Longer than any key can be: this word cannot expand.
            self.interrupt();
            return;
        }
        self.word.push(ch);
    }

    /// Backspace: drop the last letter. Deleting the Space that ended a word
    /// steps back into that word; backspacing any further past the start of
    /// the word reaches text we never saw, so the word is no longer known.
    pub fn backspace(&mut self) {
        if self.tail > 0 {
            self.tail -= 1;
            // Deleted the whole expansion: back at the boundary before it.
            self.clean = self.tail == 0;
            return;
        }
        if self.word.pop().is_some() {
            return;
        }
        match self.saved.take() {
            Some(word) if self.clean => self.word = word,
            _ => self.clean = false,
        }
    }

    /// Space: starts a fresh word, remembering the one it ended.
    pub fn space(&mut self) {
        let ended = self.word().map(str::to_string);
        self.boundary();
        self.saved = ended;
    }

    /// A key that starts a fresh word: Enter, Tab, a caret move, a click or an
    /// app switch.
    pub fn boundary(&mut self) {
        self.word.clear();
        self.clean = true;
        self.saved = None;
        self.tail = 0;
    }

    /// A key that glues onto the word (digit, punctuation, a paste): the word
    /// cannot expand until the next boundary.
    pub fn interrupt(&mut self) {
        self.word.clear();
        self.clean = false;
        self.saved = None;
        self.tail = 0;
    }

    /// The word was replaced by `expansion`. Letters typed right after it are
    /// glued to it; backspacing through all of it returns to the clean
    /// boundary the abbreviation started at.
    pub fn expanded(&mut self, expansion: &str) {
        self.interrupt();
        self.tail = expansion.chars().count();
        self.clean = self.tail == 0;
    }

    /// Whether the current word started at a boundary.
    pub fn is_clean(&self) -> bool {
        self.clean
    }

    /// The letters typed since the boundary, if the word is clean.
    pub fn word(&self) -> Option<&str> {
        (self.clean && !self.word.is_empty()).then_some(self.word.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(entries: &[(&str, &str)]) -> Abbreviations {
        let mut t = Abbreviations::new();
        for (k, e) in entries {
            t.insert(k, e).unwrap();
        }
        t
    }

    #[test]
    fn keys_need_two_to_thirty_two_letters() {
        assert_eq!(normalize_key("btw").as_deref(), Some("btw"));
        assert_eq!(normalize_key("  nz ").as_deref(), Some("nz"));
        assert_eq!(normalize_key("đc").as_deref(), Some("đc"));
        assert_eq!(normalize_key("b"), None);
        assert_eq!(normalize_key("b4"), None);
        assert_eq!(normalize_key("b t"), None);
        assert_eq!(normalize_key("a.b"), None);
        assert_eq!(normalize_key(&"a".repeat(33)), None);
        assert!(normalize_key(&"a".repeat(32)).is_some());
    }

    #[test]
    fn expansions_are_single_line_and_bounded() {
        assert_eq!(normalize_expansion(" by the way ").as_deref(), Some("by the way"));
        assert_eq!(normalize_expansion(""), None);
        assert_eq!(normalize_expansion("   "), None);
        assert_eq!(normalize_expansion("a\tb"), None);
        assert_eq!(normalize_expansion("a\nb"), None);
        assert_eq!(normalize_expansion(&"x".repeat(501)), None);
        assert!(normalize_expansion("không 👍").is_some());
    }

    #[test]
    fn insert_replaces_case_insensitively() {
        let mut t = table(&[("btw", "by the way")]);
        t.insert("BTW", "between").unwrap();
        assert_eq!(t.len(), 1);
        assert_eq!(t.get("btw"), Some("between"));
        assert!(t.remove("Btw"));
        assert!(t.is_empty());
        assert!(!t.remove("btw"));
    }

    #[test]
    fn insert_reports_why_it_refused() {
        let mut t = Abbreviations::new();
        assert_eq!(t.insert("b", "x"), Err(AbbrevError::InvalidKey));
        assert_eq!(t.insert("btw", " "), Err(AbbrevError::InvalidExpansion));
    }

    #[test]
    fn table_is_bounded() {
        let mut t = Abbreviations::new();
        let key = |i: usize| {
            // Distinct letter-only keys: base-26 digits as a..z.
            let mut s = String::new();
            let mut n = i;
            for _ in 0..3 {
                s.push((b'a' + (n % 26) as u8) as char);
                n /= 26;
            }
            s
        };
        for i in 0..MAX_ENTRIES {
            t.insert(&key(i), "x").unwrap();
        }
        assert_eq!(t.insert(&key(MAX_ENTRIES), "x"), Err(AbbrevError::Full));
        // Replacing an existing entry is still allowed when full.
        assert_eq!(t.insert(&key(0), "y"), Ok(()));
    }

    #[test]
    fn case_follows_typing() {
        let t = table(&[("btw", "by the way"), ("NZ", "New Zealand")]);
        assert_eq!(t.expand("btw").as_deref(), Some("by the way"));
        assert_eq!(t.expand("Btw").as_deref(), Some("By the way"));
        assert_eq!(t.expand("BTW").as_deref(), Some("BY THE WAY"));
        assert_eq!(t.expand("bTw").as_deref(), Some("by the way"));
        // Typed exactly as defined: the expansion as defined.
        assert_eq!(t.expand("NZ").as_deref(), Some("New Zealand"));
        assert_eq!(t.expand("nz").as_deref(), Some("New Zealand"));
        assert_eq!(t.expand("Nz").as_deref(), Some("New Zealand"));
        assert_eq!(t.expand("xyz"), None);
    }

    #[test]
    fn capitalizes_a_vietnamese_first_letter() {
        let t = table(&[("dc", "được")]);
        assert_eq!(t.expand("Dc").as_deref(), Some("Được"));
        assert_eq!(t.expand("DC").as_deref(), Some("ĐƯỢC"));
    }

    #[test]
    fn expand_word_replaces_the_typed_letters() {
        let t = table(&[("btw", "by the way")]);
        assert_eq!(
            t.expand_word("btw"),
            Some(EngineResult::Restore {
                backspaces: 3,
                text: "by the way".to_string()
            })
        );
        assert_eq!(t.expand_word("bt"), None);
    }

    #[test]
    fn file_round_trips() {
        let t = table(&[("btw", "by the way"), ("NZ", "New Zealand"), ("ko", "không")]);
        let text = t.serialize();
        assert_eq!(Abbreviations::parse(&text), t);
    }

    #[test]
    fn parse_skips_comments_and_bad_lines() {
        let text = "\u{FEFF}# comment\n\nbtw\tby the way\r\nb\tbad key\nnotab\nok\t  \nnz\tNew Zealand\n";
        let t = Abbreviations::parse(text);
        assert_eq!(t.len(), 2);
        assert_eq!(t.get("btw"), Some("by the way"));
        assert_eq!(t.get("nz"), Some("New Zealand"));
    }

    #[test]
    fn a_second_tab_invalidates_the_line() {
        // Only the first tab separates; a later one makes the expansion invalid.
        let t = Abbreviations::parse("ab\tone\ttwo\ncd\tfine\n");
        assert_eq!(t.get("ab"), None);
        assert_eq!(t.get("cd"), Some("fine"));
    }

    #[test]
    fn tracker_collects_a_clean_word() {
        let mut w = WordTracker::new();
        for c in "btw".chars() {
            w.letter(c);
        }
        assert_eq!(w.word(), Some("btw"));
        w.boundary();
        assert_eq!(w.word(), None);
        assert!(w.is_clean());
    }

    #[test]
    fn tracker_rejects_a_glued_word() {
        let mut w = WordTracker::new();
        w.interrupt(); // e.g. a digit or '.'
        for c in "btw".chars() {
            w.letter(c);
        }
        assert_eq!(w.word(), None);
        assert!(!w.is_clean());
    }

    #[test]
    fn tracker_follows_backspace() {
        let mut w = WordTracker::new();
        for c in "btx".chars() {
            w.letter(c);
        }
        w.backspace();
        w.letter('w');
        assert_eq!(w.word(), Some("btw"));
        // Backspacing past the word's start loses track of it.
        for _ in 0..4 {
            w.backspace();
        }
        w.letter('a');
        assert_eq!(w.word(), None);
    }

    #[test]
    fn tracker_steps_back_over_a_space() {
        let mut w = WordTracker::new();
        for c in "btw".chars() {
            w.letter(c);
        }
        w.space();
        w.backspace();
        assert_eq!(w.word(), Some("btw"));
        // Only the one Space: a second backspace edits the word.
        w.backspace();
        assert_eq!(w.word(), Some("bt"));
    }

    #[test]
    fn tracker_does_not_step_back_past_other_boundaries() {
        let mut w = WordTracker::new();
        for c in "btw".chars() {
            w.letter(c);
        }
        w.boundary(); // Enter, an arrow key, a click…
        w.backspace();
        assert_eq!(w.word(), None);
        assert!(!w.is_clean());

        // Two spaces: the first backspace removes the second space only.
        let mut w = WordTracker::new();
        for c in "btw".chars() {
            w.letter(c);
        }
        w.space();
        w.space();
        w.backspace();
        assert_eq!(w.word(), None);
        assert!(!w.is_clean());
    }

    #[test]
    fn tracker_returns_to_the_boundary_after_deleting_an_expansion() {
        let mut w = WordTracker::new();
        w.expanded("by the way");
        // Glued to the expansion: cannot expand.
        for c in "btw".chars() {
            w.letter(c);
        }
        assert_eq!(w.word(), None);
        // Delete the three letters and "by the way" (10 characters).
        for _ in 0..12 {
            w.backspace();
            assert!(!w.is_clean());
        }
        w.backspace();
        assert!(w.is_clean());
        for c in "btw".chars() {
            w.letter(c);
        }
        assert_eq!(w.word(), Some("btw"));
    }

    #[test]
    fn tracker_gives_up_on_overlong_words() {
        let mut w = WordTracker::new();
        for _ in 0..=MAX_KEY_LEN {
            w.letter('a');
        }
        assert_eq!(w.word(), None);
    }
}
