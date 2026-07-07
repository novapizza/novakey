//! data.rs
//! Vietnamese character data — a faithful Rust port of VietnameseData.swift.
//!
//! Unicode source: Latin Extended Additional block (U+1EA0-U+1EF9)
//! and Latin Extended-B (U+01A0-U+01B0) for horn characters.
//! Telex source: standard Telex input method.

// MARK: - Tone Marks

/// The five Vietnamese diacritical tone marks plus "no tone."
/// Discriminants match the Swift `ToneMark.rawValue` (used to index the vowel table).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ToneMark {
    #[default]
    None = 0,  // a
    Sac = 1,   // á  (acute — rising)
    Huyen = 2, // à  (grave — falling)
    Hoi = 3,   // ả  (hook above — dipping)
    Nga = 4,   // ã  (tilde — creaky rising)
    Nang = 5,  // ạ  (dot below — heavy)
}

impl ToneMark {
    /// Column index into a vowel-table row.
    #[inline]
    pub fn index(self) -> usize {
        self as usize
    }

    /// Build a tone from a table column index (mirror of `ToneMark(rawValue:)`).
    pub fn from_index(i: usize) -> Option<ToneMark> {
        match i {
            0 => Some(ToneMark::None),
            1 => Some(ToneMark::Sac),
            2 => Some(ToneMark::Huyen),
            3 => Some(ToneMark::Hoi),
            4 => Some(ToneMark::Nga),
            5 => Some(ToneMark::Nang),
            _ => None,
        }
    }
}

// MARK: - Vowel Modifiers

/// Modifiers that change the base vowel shape (independent of tone).
/// `Plain` corresponds to Swift `VowelModifier.none`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum VowelModifier {
    Plain,      // a, e, o, u
    Circumflex, // â, ê, ô  (mũ)
    Breve,      // ă        (móc ngắn)
    Horn,       // ơ, ư     (móc/râu)
}

// MARK: - Vietnamese Character

/// A single Vietnamese character with its base letter, modifier, and tone.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ViChar {
    /// The ASCII base letter (lowercase): a vowel or consonant.
    pub base: char,
    pub modifier: VowelModifier,
    pub tone: ToneMark,
    pub is_upper: bool,
    /// Whether this 'd' has been converted to 'đ'.
    pub has_dstroke: bool,
    /// Whether this vowel's horn was applied automatically by forward "ươ"
    /// linking (an 'o' auto-horned because it follows a 'ư'), rather than by an
    /// explicit 'w'. Lets a subsequent redundant 'w' confirm the horn instead
    /// of undoing it. Windows-only enhancement; not present in the Swift engine.
    pub auto_horn: bool,
}

impl ViChar {
    pub fn new(base: char) -> ViChar {
        ViChar {
            base,
            modifier: VowelModifier::Plain,
            tone: ToneMark::None,
            is_upper: false,
            has_dstroke: false,
            auto_horn: false,
        }
    }

    pub fn with(base: char, modifier: VowelModifier, tone: ToneMark, is_upper: bool) -> ViChar {
        ViChar {
            base,
            modifier,
            tone,
            is_upper,
            has_dstroke: false,
            auto_horn: false,
        }
    }

    /// Resolve this ViChar to its Unicode character.
    pub fn unicode(&self) -> char {
        // Special case: đ / Đ
        if self.base == 'd' && self.has_dstroke {
            return if self.is_upper { D_STROKE_UPPER } else { D_STROKE };
        }
        let result = resolve(self.base, self.modifier, self.tone);
        if self.is_upper {
            to_upper_single(result)
        } else {
            result
        }
    }

    /// Whether this character is a Vietnamese vowel (a, e, i, o, u, y).
    pub fn is_vowel(&self) -> bool {
        is_vowel_base(self.base)
    }
}

/// đ (d with stroke) and its uppercase form.
pub const D_STROKE: char = '\u{0111}'; // đ
pub const D_STROKE_UPPER: char = '\u{0110}'; // Đ

/// The six Vietnamese vowel base letters.
#[inline]
pub fn is_vowel_base(c: char) -> bool {
    matches!(
        c.to_ascii_lowercase(),
        'a' | 'e' | 'i' | 'o' | 'u' | 'y'
    )
}

/// Uppercase a single precomposed scalar. All Vietnamese precomposed letters
/// have a simple 1:1 uppercase mapping, so this always yields one scalar.
fn to_upper_single(c: char) -> char {
    let mut it = c.to_uppercase();
    match (it.next(), it.next()) {
        (Some(u), None) => u,
        _ => c, // never happens for our tables; keep the original defensively
    }
}

// MARK: - Unicode Resolution

/// Resolve a base vowel + modifier + tone to a single Unicode character.
/// For non-Vietnamese combinations, returns the (lowercase) base character.
pub fn resolve(base: char, modifier: VowelModifier, tone: ToneMark) -> char {
    let lower = base.to_ascii_lowercase();

    // Special case: plain d (đ handled by ViChar::unicode via has_dstroke).
    if lower == 'd' && modifier == VowelModifier::Plain && tone == ToneMark::None {
        return base;
    }

    if let Some(row) = vowel_row(lower, modifier) {
        return row[tone.index()];
    }

    base
}

/// Each row maps a (base, modifier) pair to [none, sac, huyen, hoi, nga, nang].
/// Values from the Unicode Latin Extended Additional block.
fn vowel_row(base: char, modifier: VowelModifier) -> Option<[char; 6]> {
    use VowelModifier::*;
    let row: [char; 6] = match (base, modifier) {
        //                   none        sac         huyen       hoi         nga         nang
        ('a', Plain) => ['a', '\u{00E1}', '\u{00E0}', '\u{1EA3}', '\u{00E3}', '\u{1EA1}'],
        ('a', Circumflex) => ['\u{00E2}', '\u{1EA5}', '\u{1EA7}', '\u{1EA9}', '\u{1EAB}', '\u{1EAD}'],
        ('a', Breve) => ['\u{0103}', '\u{1EAF}', '\u{1EB1}', '\u{1EB3}', '\u{1EB5}', '\u{1EB7}'],

        ('e', Plain) => ['e', '\u{00E9}', '\u{00E8}', '\u{1EBB}', '\u{1EBD}', '\u{1EB9}'],
        ('e', Circumflex) => ['\u{00EA}', '\u{1EBF}', '\u{1EC1}', '\u{1EC3}', '\u{1EC5}', '\u{1EC7}'],

        ('i', Plain) => ['i', '\u{00ED}', '\u{00EC}', '\u{1EC9}', '\u{0129}', '\u{1ECB}'],

        ('o', Plain) => ['o', '\u{00F3}', '\u{00F2}', '\u{1ECF}', '\u{00F5}', '\u{1ECD}'],
        ('o', Circumflex) => ['\u{00F4}', '\u{1ED1}', '\u{1ED3}', '\u{1ED5}', '\u{1ED7}', '\u{1ED9}'],
        ('o', Horn) => ['\u{01A1}', '\u{1EDB}', '\u{1EDD}', '\u{1EDF}', '\u{1EE1}', '\u{1EE3}'],

        ('u', Plain) => ['u', '\u{00FA}', '\u{00F9}', '\u{1EE7}', '\u{0169}', '\u{1EE5}'],
        ('u', Horn) => ['\u{01B0}', '\u{1EE9}', '\u{1EEB}', '\u{1EED}', '\u{1EEF}', '\u{1EF1}'],

        ('y', Plain) => ['y', '\u{00FD}', '\u{1EF3}', '\u{1EF7}', '\u{1EF9}', '\u{1EF5}'],

        _ => return None,
    };
    Some(row)
}

// MARK: - Telex Key Mappings

/// Telex keys that add a tone mark. `z` removes tone (maps to `None`).
pub fn telex_tone_key(c: char) -> Option<ToneMark> {
    match c {
        's' => Some(ToneMark::Sac),
        'f' => Some(ToneMark::Huyen),
        'r' => Some(ToneMark::Hoi),
        'x' => Some(ToneMark::Nga),
        'j' => Some(ToneMark::Nang),
        'z' => Some(ToneMark::None),
        _ => None,
    }
}

/// Telex vowel-modifier rules: (trigger, target base vowel, resulting modifier).
/// "aa"->â, "ee"->ê, "oo"->ô, "aw"->ă, "ow"->ơ, "uw"->ư
pub const TELEX_VOWEL_MODIFIERS: &[(char, char, VowelModifier)] = &[
    ('a', 'a', VowelModifier::Circumflex), // aa -> â
    ('e', 'e', VowelModifier::Circumflex), // ee -> ê
    ('o', 'o', VowelModifier::Circumflex), // oo -> ô
    ('w', 'a', VowelModifier::Breve),      // aw -> ă
    ('w', 'o', VowelModifier::Horn),       // ow -> ơ
    ('w', 'u', VowelModifier::Horn),       // uw -> ư
];
