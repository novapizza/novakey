// Abbreviations.swift
// User-defined abbreviations (Vietnamese: "gõ tắt"): a short form typed and
// followed by Tab expands to its full text -- "btw" -> "by the way".
//
// Pure logic, like the rest of the engine: the table, its case rule, the text
// file it is stored in, and the word tracker the event tap uses while
// Vietnamese composition is off. The engine side (expanding the word it is
// composing) is `TelexEngine.expandAbbreviation`. Port of the Rust core's
// abbrev.rs; the file format is shared with the Windows app.

import Foundation

/// Why an entry was refused.
enum AbbrevError: Error, Equatable {
    /// Not 2–32 letters (digits, spaces and punctuation are not allowed).
    case invalidKey
    /// Empty, too long, or contains a tab or line break.
    case invalidExpansion
    /// The table already holds `Abbreviations.maxEntries` entries.
    case full
}

/// The user's abbreviation table. Keys match case-insensitively; each entry
/// keeps the key as the user wrote it, for display and for the case rule.
struct Abbreviations: Equatable {

    /// Shortest abbreviation, in letters.
    static let minKeyLength = 2
    /// Longest abbreviation, in letters.
    static let maxKeyLength = 32
    /// Longest expansion, in characters.
    static let maxExpansionLength = 500
    /// Most entries the table holds, so a hand-edited file cannot grow it
    /// without bound.
    static let maxEntries = 2000

    /// One row of the table, as the user wrote it.
    struct Entry: Equatable, Hashable {
        let key: String
        let expansion: String
    }

    /// Lowercased key -> entry.
    private var entries: [String: Entry] = [:]

    init() {}

    var count: Int { entries.count }
    var isEmpty: Bool { entries.isEmpty }

    /// Entries sorted by key, for the Settings window.
    var sortedEntries: [Entry] {
        entries.sorted { $0.key < $1.key }.map { $0.value }
    }

    /// The expansion stored for `key` (any case).
    func get(_ key: String) -> String? {
        entries[Self.lookupKey(key)]?.expansion
    }

    /// Add an entry, replacing any existing one with the same key (any case).
    /// Returns why it was refused, or nil once stored.
    @discardableResult
    mutating func insert(_ key: String, _ expansion: String) -> AbbrevError? {
        guard let key = Self.normalizeKey(key) else { return .invalidKey }
        guard let expansion = Self.normalizeExpansion(expansion) else { return .invalidExpansion }
        let lower = key.lowercased()
        if entries[lower] == nil && entries.count >= Self.maxEntries {
            return .full
        }
        entries[lower] = Entry(key: key, expansion: expansion)
        return nil
    }

    /// Remove the entry for `key` (any case). Returns whether one existed.
    @discardableResult
    mutating func remove(_ key: String) -> Bool {
        entries.removeValue(forKey: Self.lookupKey(key)) != nil
    }

    /// The text `typed` expands to, cased to follow how it was typed (see
    /// `applyCase`), or nil if it is not an abbreviation.
    func expand(_ typed: String) -> String? {
        guard let entry = entries[typed.lowercased()] else { return nil }
        return Self.applyCase(typed: typed, key: entry.key, expansion: entry.expansion)
    }

    /// The replacement for a word typed while Vietnamese composition is off:
    /// what is on screen is exactly the letters typed.
    func expandWord(_ word: String) -> EngineResult? {
        guard let text = expand(word) else { return nil }
        return .restore(backspaces: word.count, text: text)
    }

    // MARK: - File format

    /// Parse the stored file: one `key<TAB>expansion` per line. Blank lines,
    /// `#` comments and invalid lines are skipped, so a hand edit gone wrong
    /// loses that line rather than the whole list.
    static func parse(_ text: String) -> Abbreviations {
        var table = Abbreviations()
        var body = Substring(text)
        if body.hasPrefix("\u{FEFF}") { body = body.dropFirst() }
        for rawLine in body.split(omittingEmptySubsequences: false, whereSeparator: { $0.isNewline }) {
            let line = rawLine.trimmingCharacters(in: .whitespaces)
            if line.isEmpty || line.hasPrefix("#") { continue }
            guard let tab = rawLine.firstIndex(of: "\t") else { continue }
            let key = String(rawLine[..<tab])
            let expansion = String(rawLine[rawLine.index(after: tab)...])
            table.insert(key, expansion)
        }
        return table
    }

    /// The file format `parse` reads.
    func serialize() -> String {
        var out = "# NovaKey abbreviations: one per line, abbreviation<TAB>expansion\n"
        for entry in sortedEntries {
            out += "\(entry.key)\t\(entry.expansion)\n"
        }
        return out
    }

    // MARK: - Validation

    /// A valid key, trimmed: 2–32 letters (Vietnamese letters included).
    static func normalizeKey(_ key: String) -> String? {
        let key = key.trimmingCharacters(in: .whitespaces)
        let n = key.count
        guard (minKeyLength...maxKeyLength).contains(n), key.allSatisfy({ $0.isLetter }) else {
            return nil
        }
        return key
    }

    /// A valid expansion, trimmed: non-empty, single-line, at most
    /// `maxExpansionLength` characters. Tabs are refused because they separate
    /// the fields of the stored file.
    static func normalizeExpansion(_ expansion: String) -> String? {
        let expansion = expansion.trimmingCharacters(in: .whitespaces)
        guard !expansion.isEmpty, expansion.count <= maxExpansionLength else { return nil }
        guard !expansion.unicodeScalars.contains(where: { CharacterSet.controlCharacters.contains($0) }) else {
            return nil
        }
        return expansion
    }

    private static func lookupKey(_ key: String) -> String {
        key.trimmingCharacters(in: .whitespaces).lowercased()
    }

    /// Case the expansion to follow how the key was typed:
    /// - exactly as defined -> the expansion as defined (`NZ` -> `New Zealand`);
    /// - all capitals -> all capitals (`BTW` -> `BY THE WAY`);
    /// - first capital only -> first letter capitalized (`Btw` -> `By the way`);
    /// - anything else -> the expansion as defined.
    static func applyCase(typed: String, key: String, expansion: String) -> String {
        if typed == key { return expansion }
        guard let first = typed.first, first.isUppercase else { return expansion }
        let rest = typed.dropFirst()
        if rest.allSatisfy({ $0.isUppercase }) {
            return expansion.uppercased()
        }
        if rest.allSatisfy({ $0.isLowercase }) {
            guard let head = expansion.first else { return expansion }
            return head.uppercased() + expansion.dropFirst()
        }
        return expansion
    }
}

/// Tracks the word being typed for abbreviation lookup. The event tap feeds
/// it every key; it knows whether the current word started at a clean
/// boundary (Space, Enter, a click, a shortcut…) -- an abbreviation glued to
/// a digit or punctuation ("x.btw", "2btw") must not expand -- and, for when
/// Vietnamese composition is off, the letters typed (which are then exactly
/// what is on screen).
struct WordTracker {
    private var word: String = ""
    private var clean: Bool = true
    /// The clean word a Space just ended, so a Backspace that deletes that
    /// Space steps back into it.
    private var saved: String? = nil
    /// Characters known to sit between a clean boundary and the caret: an
    /// expansion NovaKey typed, plus any letters typed after it. Backspacing
    /// through all of them returns to that boundary.
    private var tail: Int = 0

    init() {}

    /// A letter was typed (with its case).
    mutating func letter(_ ch: Character) {
        saved = nil
        if tail > 0 {
            // Glued to an expansion: known, but not a word that can expand.
            tail += 1
            return
        }
        if !clean { return }
        if word.count >= Abbreviations.maxKeyLength {
            // Longer than any key can be: this word cannot expand.
            interrupt()
            return
        }
        word.append(ch)
    }

    /// Backspace: drop the last letter. Deleting the Space that ended a word
    /// steps back into that word; backspacing any further past the start of
    /// the word reaches text we never saw, so the word is no longer known.
    mutating func backspace() {
        if tail > 0 {
            tail -= 1
            // Deleted the whole expansion: back at the boundary before it.
            clean = tail == 0
            return
        }
        if word.popLast() != nil { return }
        if let ended = saved, clean {
            saved = nil
            word = ended
        } else {
            saved = nil
            clean = false
        }
    }

    /// Space: starts a fresh word, remembering the one it ended.
    mutating func space() {
        let ended = currentWord
        boundary()
        saved = ended
    }

    /// A key that starts a fresh word: Enter, Tab, a caret move, a click or
    /// an app switch.
    mutating func boundary() {
        word = ""
        clean = true
        saved = nil
        tail = 0
    }

    /// A key that glues onto the word (digit, punctuation, a paste): the word
    /// cannot expand until the next boundary.
    mutating func interrupt() {
        word = ""
        clean = false
        saved = nil
        tail = 0
    }

    /// The word was replaced by `expansion`. Letters typed right after it are
    /// glued to it; backspacing through all of it returns to the clean
    /// boundary the abbreviation started at.
    mutating func expanded(_ expansion: String) {
        interrupt()
        tail = expansion.count
        clean = tail == 0
    }

    /// Whether the current word started at a boundary.
    var isClean: Bool { clean }

    /// The letters typed since the boundary, if the word is clean.
    var currentWord: String? {
        (clean && !word.isEmpty) ? word : nil
    }
}
