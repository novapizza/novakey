//! spelling.rs
//! Validates whether a buffer forms a structurally valid Vietnamese syllable —
//! port of SpellingChecker.swift. Structural rules, not a dictionary lookup.

use crate::buffer::SyllableBuffer;

/// Check if the current buffer could form a valid Vietnamese syllable.
pub fn is_valid_syllable(buffer: &SyllableBuffer) -> bool {
    if buffer.is_empty() {
        return false;
    }

    let initial = buffer.initial_consonant().to_lowercase();
    let mut vowel_cluster = buffer.vowel_cluster().to_lowercase();
    let ending = buffer.ending_consonant().to_lowercase();

    // Must have at least one vowel.
    if vowel_cluster.is_empty() {
        return false;
    }

    // Vowels must be contiguous. A consonant between two vowels means this
    // cannot be a single Vietnamese syllable (e.g. "cofe").
    if let (Some(first), Some(last)) = (buffer.first_vowel_index(), buffer.last_vowel_index()) {
        if buffer.chars[first..=last].iter().any(|c| !c.is_vowel()) {
            return false;
        }
    }

    // "gi" + vowel: leading 'i' is part of the consonant digraph.
    if (initial == "g" || initial == "gi") && vowel_cluster.chars().count() > 1 {
        if vowel_cluster.starts_with('i') {
            vowel_cluster.remove(0);
        }
    }

    // "qu" + vowel: 'u' in 'qu' is part of the consonant cluster.
    if initial.ends_with('q') && vowel_cluster.chars().count() > 1 {
        if vowel_cluster.starts_with('u') {
            vowel_cluster.remove(0);
        }
    }

    // Validate initial consonant (if present).
    if !initial.is_empty() && !is_valid_initial_consonant(&initial) {
        return false;
    }

    // Validate final consonant (if present).
    if !ending.is_empty() && !is_valid_final_consonant(&ending) {
        return false;
    }

    // Validate vowel nucleus.
    if !is_valid_vowel_nucleus(&vowel_cluster) {
        return false;
    }

    true
}

/// Valid single, double and triple initial consonants.
fn is_valid_initial_consonant(c: &str) -> bool {
    matches!(
        c,
        // Single
        "b" | "c" | "d" | "g" | "h" | "k" | "l" | "m" | "n" | "p" | "q" | "r" | "s" | "t" | "v" | "x"
        // Double
        | "ch" | "gh" | "gi" | "kh" | "ng" | "nh" | "ph" | "qu" | "th" | "tr"
        // Triple
        | "ngh"
    )
}

/// Valid final consonants in Vietnamese.
fn is_valid_final_consonant(c: &str) -> bool {
    matches!(c, "c" | "ch" | "m" | "n" | "ng" | "nh" | "p" | "t")
}

/// Valid vowel nuclei (base forms, before modifiers).
fn is_valid_vowel_nucleus(v: &str) -> bool {
    matches!(
        v,
        // Single vowels
        "a" | "e" | "i" | "o" | "u" | "y"
        // Two-vowel combinations
        | "ai" | "ao" | "au" | "ay"
        | "eo" | "eu"
        | "ia" | "ie" | "iu" | "iy"
        | "oa" | "oe" | "oi" | "oo" | "ou"
        | "ua" | "ue" | "ui" | "uo" | "uu" | "uy"
        | "ya" | "ye"
        // Three-vowel combinations
        | "ieu" | "oai" | "oay" | "oeo" | "uai" | "uay"
        | "uoi" | "uou" | "uya" | "uye" | "uyu"
        | "yeu"
    )
}
