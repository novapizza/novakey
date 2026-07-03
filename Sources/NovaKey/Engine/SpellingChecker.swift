// SpellingChecker.swift
// Validates whether a sequence of characters forms a valid Vietnamese syllable.
//
// Vietnamese syllable structure: [initial consonant] + vowel nucleus + [final consonant]
// This checker validates structural rules, not a dictionary lookup.

import Foundation

enum SpellingChecker {

    /// Check if the current buffer could form a valid Vietnamese syllable.
    /// This is a structural check, not an exhaustive dictionary lookup.
    static func isValidSyllable(_ buffer: SyllableBuffer) -> Bool {
        guard !buffer.isEmpty else { return false }

        let initial = buffer.initialConsonant.lowercased()
        var vowelCluster = buffer.vowelCluster.lowercased()
        let ending = buffer.endingConsonant.lowercased()

        // Must have at least one vowel
        guard !vowelCluster.isEmpty else { return false }

        // Vowels must be contiguous. A consonant between two vowels means
        // this cannot be a single Vietnamese syllable (e.g. "cofe"), even
        // though the extracted vowel cluster ("oe") looks valid on its own.
        if let first = buffer.firstVowelIndex, let last = buffer.lastVowelIndex,
           buffer.chars[first...last].contains(where: { !$0.isVowel }) {
            return false
        }

        // "gi" + vowel: the leading 'i' is part of the consonant digraph, not
        // the nucleus (e.g. "giữa" -> nucleus "ưa"/"ua", not "iua"). When the
        // buffer starts with 'g' followed by an 'i' that has more vowels after
        // it, the firstVowelIndex sits on the 'i' and the initial reads as "g".
        if (initial == "g" || initial == "gi"),
           vowelCluster.count > 1,
           vowelCluster.first == "i" {
            vowelCluster.removeFirst()
        }

        // "qu" + vowel: the 'u' in 'qu' is part of the consonant cluster,
        // mirroring the same exception used during tone placement.
        if initial.hasSuffix("q"),
           vowelCluster.count > 1,
           vowelCluster.first == "u" {
            vowelCluster.removeFirst()
        }

        // Validate initial consonant (if present)
        if !initial.isEmpty && !isValidInitialConsonant(initial) {
            return false
        }

        // Validate final consonant (if present)
        if !ending.isEmpty && !isValidFinalConsonant(ending) {
            return false
        }

        // Validate vowel nucleus
        if !isValidVowelNucleus(vowelCluster) {
            return false
        }

        return true
    }

    // MARK: - Initial Consonants

    /// Valid single and multi-character initial consonants.
    private static let validInitials: Set<String> = [
        // Single
        "b", "c", "d", "g", "h", "k", "l", "m", "n", "p", "q", "r", "s", "t", "v", "x",
        // Double
        "ch", "gh", "gi", "kh", "ng", "nh", "ph", "qu", "th", "tr",
        // Triple
        "ngh",
    ]

    private static func isValidInitialConsonant(_ c: String) -> Bool {
        validInitials.contains(c)
    }

    // MARK: - Final Consonants

    /// Valid final consonants in Vietnamese.
    private static let validFinals: Set<String> = [
        "c", "ch", "m", "n", "ng", "nh", "p", "t"
    ]

    private static func isValidFinalConsonant(_ c: String) -> Bool {
        validFinals.contains(c)
    }

    // MARK: - Vowel Nucleus

    /// Valid vowel nuclei in Vietnamese (base forms, before modifiers).
    private static let validVowelNuclei: Set<String> = [
        // Single vowels
        "a", "e", "i", "o", "u", "y",
        // Two-vowel combinations
        "ai", "ao", "au", "ay",
        "eo", "eu",
        "ia", "ie", "iu", "iy",
        "oa", "oe", "oi", "oo", "ou",
        "ua", "ue", "ui", "uo", "uu", "uy",
        "ya", "ye",
        // Three-vowel combinations
        "ieu", "oai", "oay", "oeo", "uai", "uay",
        "uoi", "uou", "uya", "uye", "uyu",
        "yeu",
    ]

    private static func isValidVowelNucleus(_ v: String) -> Bool {
        // Check base form
        validVowelNuclei.contains(v)
    }
}
