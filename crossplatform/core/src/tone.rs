//! tone.rs
//! Smart (modern) Vietnamese tone mark placement — port of TonePlacement.swift.
//!
//! Core principle: the tone mark goes on the **main vowel** of the syllable nucleus.

use crate::buffer::SyllableBuffer;
use crate::data::VowelModifier;

/// Determine which vowel index in the buffer should receive the tone mark,
/// or None if no valid position exists.
pub fn find_tone_position(buffer: &SyllableBuffer) -> Option<usize> {
    let vowel_indices = buffer.vowel_indices();
    if vowel_indices.is_empty() {
        return None;
    }

    // Single vowel: tone goes on it.
    if vowel_indices.len() == 1 {
        return Some(vowel_indices[0]);
    }

    let has_ending = buffer.has_ending_consonant();
    let initial = buffer.initial_consonant().to_lowercase();

    // Handle special consonant-vowel combinations ("qu", "gi").
    let effective = adjust_for_consonant_clusters(&vowel_indices, &initial, buffer);

    if effective.is_empty() {
        return vowel_indices.last().copied();
    }

    if effective.len() == 1 {
        return Some(effective[0]);
    }

    let effective_vowels: String = effective.iter().map(|&i| buffer.chars[i].base).collect();

    Some(position_for_vowel_cluster(
        &effective_vowels,
        &effective,
        has_ending,
        buffer,
    ))
}

/// Adjust vowel indices to account for "qu" and "gi" consonant clusters
/// where a vowel letter is actually part of the initial consonant.
fn adjust_for_consonant_clusters(
    vowel_indices: &[usize],
    initial: &str,
    buffer: &SyllableBuffer,
) -> Vec<usize> {
    let mut adjusted = vowel_indices.to_vec();

    // "qu" + vowel: 'u' is part of consonant cluster (only if more vowels follow).
    if initial.ends_with('q') && adjusted.len() > 1 {
        if let Some(&first) = adjusted.first() {
            if buffer.chars[first].base == 'u' {
                adjusted.remove(0);
            }
        }
    }

    // "gi" + vowel: 'i' is part of consonant when followed by another vowel.
    if initial == "g" || initial == "gi" {
        if adjusted.len() > 1 {
            if let Some(&first) = adjusted.first() {
                if buffer.chars[first].base == 'i' {
                    adjusted.remove(0);
                }
            }
        }
    }

    adjusted
}

/// Determine tone position for a multi-vowel cluster.
fn position_for_vowel_cluster(
    vowels: &str,
    indices: &[usize],
    has_ending_consonant: bool,
    buffer: &SyllableBuffer,
) -> usize {
    let count = indices.len();
    let lower = vowels.to_lowercase();

    // Three-vowel clusters: tone on the modified vowel if present, else middle.
    if count >= 3 {
        if let Some(&mod_idx) = indices
            .iter()
            .rev()
            .find(|&&i| buffer.chars[i].modifier != VowelModifier::Plain)
        {
            return mod_idx;
        }
        return indices[1];
    }

    // Two-vowel clusters.
    if count != 2 {
        return *indices.last().unwrap();
    }

    let mut chs = lower.chars();
    let first = chs.next().unwrap();
    let second = lower.chars().last().unwrap();

    let first_mod = buffer.chars[indices[0]].modifier;
    let second_mod = buffer.chars[indices[1]].modifier;

    // If a vowel has a modifier, it's the main vowel and gets the tone.
    if second_mod != VowelModifier::Plain && first_mod == VowelModifier::Plain {
        return indices[1];
    }
    if first_mod != VowelModifier::Plain && second_mod == VowelModifier::Plain {
        return indices[0];
    }

    // Both or neither have modifiers -> positional rules.

    // With ending consonant: tone on the SECOND vowel (toán, hoàng, uyên).
    if has_ending_consonant {
        return indices[1];
    }

    // Falling diphthongs ending in i/y/u -> tone on FIRST vowel (hái, cáo, đáu).
    let falling = |c: char| c == 'i' || c == 'y' || c == 'u';
    if falling(second) && !falling(first) {
        return indices[0];
    }

    // Rising diphthongs ia/ua/ưa -> tone on FIRST vowel (mía, của, mùa).
    if (first == 'i' || first == 'u') && second == 'a' {
        return indices[0];
    }

    // "oa", "oe" -> tone on SECOND vowel (hoà modern).
    if first == 'o' && (second == 'a' || second == 'e') {
        return indices[1];
    }

    // "ue" -> tone on SECOND vowel.
    if first == 'u' && second == 'e' {
        return indices[1];
    }

    // Default: first vowel.
    indices[0]
}
