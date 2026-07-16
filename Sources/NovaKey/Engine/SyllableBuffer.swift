// SyllableBuffer.swift
// Tracks the current Vietnamese syllable being typed.
// Maintains both the raw keystrokes and resolved Vietnamese characters.

import Foundation

/// Maximum buffer size. Vietnamese syllables are at most ~7 characters,
/// but we allow extra room for typing sequences before word breaks.
private let maxBufferSize = 32

/// Manages the character buffer for the current syllable being composed.
struct SyllableBuffer {

    // MARK: - State

    /// The resolved Vietnamese characters in the current syllable.
    private(set) var chars: [ViChar] = []

    /// Whether the 'd' has been converted to 'đ' (d-stroke).
    private(set) var hasDStroke: Bool = false

    /// Index of the 'd' character in chars (if present and converted).
    private(set) var dStrokeIndex: Int? = nil

    /// The current tone mark applied to the syllable.
    private(set) var currentTone: ToneMark = .none

    /// Index in `chars` where the tone mark is placed.
    private(set) var toneIndex: Int? = nil

    // MARK: - Computed Properties

    /// Whether the buffer is empty.
    var isEmpty: Bool { chars.isEmpty }

    /// Number of characters in the buffer.
    var count: Int { chars.count }

    /// The full Unicode string represented by the current buffer.
    var text: String {
        String(chars.map { $0.unicode })
    }

    /// Indices of all vowel characters in the buffer.
    var vowelIndices: [Int] {
        chars.indices.filter { chars[$0].isVowel }
    }

    /// Number of vowels in the buffer.
    var vowelCount: Int {
        chars.filter { $0.isVowel }.count
    }

    /// Index of the first vowel, or nil if none.
    var firstVowelIndex: Int? {
        chars.firstIndex(where: { $0.isVowel })
    }

    /// Index of the last vowel, or nil if none.
    var lastVowelIndex: Int? {
        chars.lastIndex(where: { $0.isVowel })
    }

    /// Whether the syllable currently ends with a consonant (after the vowel cluster).
    var hasEndingConsonant: Bool {
        guard let lastVowel = lastVowelIndex else { return false }
        // Check if there are any consonant characters after the last vowel
        return chars[(lastVowel + 1)...].contains(where: { !$0.isVowel })
    }

    /// The ending consonant string (e.g., "ng", "nh", "ch", "n", "m", "t", "c", "p").
    var endingConsonant: String {
        guard let lastVowel = lastVowelIndex else { return "" }
        let trailing = chars[(lastVowel + 1)...]
        return String(trailing.map { $0.base })
    }

    /// The initial consonant string (everything before the first vowel).
    var initialConsonant: String {
        guard let firstVowel = firstVowelIndex else {
            return String(chars.map { $0.base })
        }
        return String(chars[..<firstVowel].map { $0.base })
    }

    /// The vowel cluster as lowercase base characters.
    var vowelCluster: String {
        let indices = vowelIndices
        return String(indices.map { chars[$0].base })
    }

    // MARK: - Mutations

    /// Append a character to the buffer.
    /// Returns false if the buffer is full.
    @discardableResult
    mutating func append(_ char: ViChar) -> Bool {
        guard chars.count < maxBufferSize else { return false }
        chars.append(char)
        return true
    }

    /// Remove and return the last character from the buffer.
    @discardableResult
    mutating func removeLast() -> ViChar? {
        guard !chars.isEmpty else { return nil }
        let removed = chars.removeLast()

        // If we removed the character that had the tone, clear tone tracking
        if let ti = toneIndex, ti >= chars.count {
            toneIndex = nil
            currentTone = .none
        }

        // If we removed the đ character, clear d-stroke tracking
        if let di = dStrokeIndex, di >= chars.count {
            dStrokeIndex = nil
            hasDStroke = false
        }

        return removed
    }

    /// Reset the buffer to empty state.
    mutating func reset() {
        chars.removeAll(keepingCapacity: true)
        hasDStroke = false
        dStrokeIndex = nil
        currentTone = .none
        toneIndex = nil
    }

    /// Apply a tone mark at the specified index.
    /// Removes any existing tone mark first.
    mutating func applyTone(_ tone: ToneMark, at index: Int) {
        // Remove old tone if present
        if let oldIndex = toneIndex, oldIndex < chars.count {
            chars[oldIndex].tone = .none
        }

        // Apply new tone
        if tone == .none {
            toneIndex = nil
            currentTone = .none
        } else {
            guard index < chars.count, chars[index].isVowel else { return }
            chars[index].tone = tone
            toneIndex = index
            currentTone = tone
        }
    }

    /// Apply a vowel modifier (circumflex, breve, horn) at the specified index.
    mutating func applyModifier(_ modifier: VowelModifier, at index: Int) {
        guard index < chars.count else { return }
        chars[index].modifier = modifier
    }

    /// Remove the vowel modifier at the specified index, restoring it to plain.
    mutating func removeModifier(at index: Int) {
        guard index < chars.count else { return }
        chars[index].modifier = .none
    }

    /// Mark the character at `index` as d-stroke (đ).
    mutating func applyDStroke(at index: Int) {
        guard index < chars.count, chars[index].base == "d" else { return }
        chars[index].hasDStroke = true
        hasDStroke = true
        dStrokeIndex = index
    }

    /// Remove d-stroke, reverting đ back to d.
    mutating func removeDStroke() {
        if let idx = dStrokeIndex, idx < chars.count {
            chars[idx].hasDStroke = false
        }
        hasDStroke = false
        dStrokeIndex = nil
    }

    /// Move the tone mark to a new index (used during grammar re-check).
    mutating func moveTone(to newIndex: Int) {
        guard currentTone != .none else { return }
        applyTone(currentTone, at: newIndex)
    }

    // MARK: - Query

    /// Get the character at the given index.
    func character(at index: Int) -> ViChar? {
        guard index >= 0, index < chars.count else { return nil }
        return chars[index]
    }

    /// Find the last character matching a given base letter (case-insensitive).
    func lastIndex(ofBase base: Character) -> Int? {
        let lower = base.lowercased().first ?? base
        return chars.lastIndex(where: { $0.base == lower })
    }
}
