// VietnameseData.swift
// Vietnamese character data built from Unicode standard and Telex input specification.
//
// Unicode source: Latin Extended Additional block (U+1EA0-U+1EF9)
// and Latin Extended-B (U+01A0-U+01B0) for horn characters.
// Telex source: Standard Telex input method as documented in Vietnamese computing standards.

import Foundation

// MARK: - Tone Marks

/// The five Vietnamese diacritical tone marks plus "no tone."
enum ToneMark: Int, CaseIterable {
    case none   = 0  // a
    case sac    = 1  // á  (acute accent - rising tone)
    case huyen  = 2  // à  (grave accent - falling tone)
    case hoi    = 3  // ả  (hook above - dipping tone)
    case nga    = 4  // ã  (tilde - creaky rising tone)
    case nang   = 5  // ạ  (dot below - heavy tone)
}

// MARK: - Vowel Modifiers

/// Modifiers that change the base vowel shape (independent of tone).
enum VowelModifier: Equatable {
    case none        // a, e, o, u
    case circumflex  // â, ê, ô  (hat/mũ)
    case breve       // ă        (trăng/móc ngắn)
    case horn        // ơ, ư     (móc/râu)
}

// MARK: - Vietnamese Character

/// A single Vietnamese character with its base letter, modifier, and tone.
struct ViChar: Equatable {
    var base: Character       // The ASCII base letter: a, e, i, o, u, y, or consonant
    var modifier: VowelModifier
    var tone: ToneMark
    var isUpperCase: Bool
    var hasDStroke: Bool      // Whether this 'd' has been converted to 'đ'
    /// Whether this 'ư' was conjured from a *lone* 'w' (standalone `w`->`ư`, or
    /// Quick Vietnamese `<initial>w`->`ư`) with no real 'u' typed. A following
    /// 'w' then reverts it to a literal 'w' ("tw"+w -> "tw") instead of leaking
    /// a spurious 'u' ("tuw").
    var bareW: Bool

    init(base: Character, modifier: VowelModifier = .none, tone: ToneMark = .none, isUpperCase: Bool = false, hasDStroke: Bool = false, bareW: Bool = false) {
        self.base = base
        self.modifier = modifier
        self.tone = tone
        self.isUpperCase = isUpperCase
        self.hasDStroke = hasDStroke
        self.bareW = bareW
    }

    /// Resolve this ViChar to its Unicode character.
    var unicode: Character {
        // Special case: đ/Đ
        if base == "d" && hasDStroke {
            return isUpperCase ? VietnameseData.dStrokeUpper : VietnameseData.dStroke
        }
        let result = VietnameseData.resolve(base: base, modifier: modifier, tone: tone)
        if isUpperCase {
            return Character(result.uppercased())
        }
        return result
    }

    /// Whether this character is a Vietnamese vowel (a, e, i, o, u, y).
    var isVowel: Bool {
        VietnameseData.vowels.contains(base.lowercased().first ?? " ")
    }
}

// MARK: - Vietnamese Data Tables

/// Static lookup tables for Vietnamese character resolution.
/// Built from the Unicode standard (Latin Extended Additional U+1EA0-U+1EF9).
enum VietnameseData {

    /// The six Vietnamese vowel base letters.
    static let vowels: Set<Character> = ["a", "e", "i", "o", "u", "y"]

    /// Vietnamese consonants that can appear at the start of a syllable.
    static let initialConsonants: Set<String> = [
        "b", "c", "ch", "d", "g", "gh", "gi", "h", "k", "kh",
        "l", "m", "n", "ng", "ngh", "nh", "p", "ph", "qu",
        "r", "s", "t", "th", "tr", "v", "x"
    ]

    /// Vietnamese consonants that can appear at the end of a syllable.
    static let finalConsonants: Set<String> = [
        "c", "ch", "m", "n", "ng", "nh", "p", "t"
    ]

    // MARK: - Unicode Resolution

    /// Resolve a base vowel + modifier + tone to a single Unicode character.
    /// For consonants (d with stroke), handles đ/Đ.
    /// For non-Vietnamese combinations, returns the base character.
    static func resolve(base: Character, modifier: VowelModifier, tone: ToneMark) -> Character {
        let lower = base.lowercased().first ?? base

        // Special case: đ (d with stroke)
        if lower == "d" && modifier == .none && tone == .none {
            return base // plain d
        }

        // Look up in the vowel table
        if let table = vowelTable[lower], let row = table[modifier] {
            return row[tone.rawValue]
        }

        // Not a vowel with this modifier - return base
        return base
    }

    /// Get the đ character.
    static let dStroke: Character = "\u{0111}"      // đ
    static let dStrokeUpper: Character = "\u{0110}" // Đ

    // MARK: - Vowel Lookup Table
    // Each vowel maps modifier -> [none, sac, huyen, hoi, nga, nang]
    // Values from Unicode Latin Extended Additional block.

    static let vowelTable: [Character: [VowelModifier: [Character]]] = [
        "a": [
            //                    none   sac    huyen  hoi    nga    nang
            .none:        [       "a",   "\u{00E1}", "\u{00E0}", "\u{1EA3}", "\u{00E3}", "\u{1EA1}"],
            .circumflex:  [       "\u{00E2}", "\u{1EA5}", "\u{1EA7}", "\u{1EA9}", "\u{1EAB}", "\u{1EAD}"],
            .breve:       [       "\u{0103}", "\u{1EAF}", "\u{1EB1}", "\u{1EB3}", "\u{1EB5}", "\u{1EB7}"],
        ],
        "e": [
            .none:        [       "e",   "\u{00E9}", "\u{00E8}", "\u{1EBB}", "\u{1EBD}", "\u{1EB9}"],
            .circumflex:  [       "\u{00EA}", "\u{1EBF}", "\u{1EC1}", "\u{1EC3}", "\u{1EC5}", "\u{1EC7}"],
        ],
        "i": [
            .none:        [       "i",   "\u{00ED}", "\u{00EC}", "\u{1EC9}", "\u{0129}", "\u{1ECB}"],
        ],
        "o": [
            .none:        [       "o",   "\u{00F3}", "\u{00F2}", "\u{1ECF}", "\u{00F5}", "\u{1ECD}"],
            .circumflex:  [       "\u{00F4}", "\u{1ED1}", "\u{1ED3}", "\u{1ED5}", "\u{1ED7}", "\u{1ED9}"],
            .horn:        [       "\u{01A1}", "\u{1EDB}", "\u{1EDD}", "\u{1EDF}", "\u{1EE1}", "\u{1EE3}"],
        ],
        "u": [
            .none:        [       "u",   "\u{00FA}", "\u{00F9}", "\u{1EE7}", "\u{0169}", "\u{1EE5}"],
            .horn:        [       "\u{01B0}", "\u{1EE9}", "\u{1EEB}", "\u{1EED}", "\u{1EEF}", "\u{1EF1}"],
        ],
        "y": [
            .none:        [       "y",   "\u{00FD}", "\u{1EF3}", "\u{1EF7}", "\u{1EF9}", "\u{1EF5}"],
        ],
    ]

    // MARK: - Telex Key Mappings

    /// Telex keys that add a tone mark.
    static let telexToneKeys: [Character: ToneMark] = [
        "s": .sac,
        "f": .huyen,
        "r": .hoi,
        "x": .nga,
        "j": .nang,
        "z": .none,   // z removes tone (resets to .none)
    ]

    /// Telex keys that modify a vowel (double-key or w-key).
    /// Maps: (trigger key) -> (target base vowel, resulting modifier)
    /// "aa" -> â, "ee" -> ê, "oo" -> ô, "aw" -> ă, "ow" -> ơ, "uw" -> ư
    static let telexVowelModifiers: [(trigger: Character, target: Character, modifier: VowelModifier)] = [
        // Double-key circumflex
        ("a", "a", .circumflex),  // aa -> â
        ("e", "e", .circumflex),  // ee -> ê
        ("o", "o", .circumflex),  // oo -> ô

        // w-key breve and horn
        ("w", "a", .breve),       // aw -> ă
        ("w", "o", .horn),        // ow -> ơ
        ("w", "u", .horn),        // uw -> ư
    ]

    // MARK: - Reverse Lookup

    /// Given a Unicode Vietnamese character, decompose it into (base, modifier, tone).
    /// Returns nil if the character is not a recognized Vietnamese letter.
    static func decompose(_ char: Character) -> (base: Character, modifier: VowelModifier, tone: ToneMark)? {
        let lower = Character(char.lowercased())

        // Check đ
        if lower == dStroke {
            return ("d", .none, .none)
        }

        // Search the vowel table
        for (base, modifiers) in vowelTable {
            for (modifier, tones) in modifiers {
                if let toneIndex = tones.firstIndex(of: lower) {
                    let tone = ToneMark(rawValue: tones.distance(from: tones.startIndex, to: toneIndex))!
                    return (base, modifier, tone)
                }
            }
        }

        return nil
    }
}
