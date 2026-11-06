// AbbreviationCoordinator.swift
// Glue between the event tap and abbreviation expansion: feeds every keyDown
// to the word tracker, and on Tab decides whether the word just typed is an
// abbreviation -- via the engine in Vietnamese mode (it knows both what is on
// screen and the keys behind it), via the tracker's letters in English mode
// (then exactly what is on screen). Pure logic so the tap's behaviour is
// testable without CGEvents.

import Foundation

final class AbbreviationCoordinator {

    /// Whether abbreviations expand (Settings > Abbreviations).
    var enabled = false

    /// Whether one of NovaKey's own windows is in front. Abbreviations never
    /// expand there, or typing "btw" into the abbreviation editor would
    /// expand it.
    var suspended = false

    private(set) var tracker = WordTracker()

    /// The caret may have moved without us seeing it (click, app switch, tap
    /// revival, mode toggle): the next word starts at a clean boundary.
    func reset() {
        tracker.boundary()
    }

    /// The replacement for the word Tab is ending, if it is an abbreviation
    /// that started at a clean boundary; nil lets the Tab through. Must be
    /// called before the Tab reaches `engine.processKey`.
    func expandOnTab(engine: TelexEngine, table: Abbreviations) -> EngineResult? {
        guard enabled, !suspended, tracker.isClean, !table.isEmpty else { return nil }
        let result: EngineResult?
        if engine.isVietnameseMode {
            result = engine.expandAbbreviation(table)
        } else {
            result = tracker.currentWord.flatMap { table.expandWord($0) }
        }
        if case .restore(_, let text)? = result {
            tracker.expanded(text)
        }
        return result
    }

    /// Feed a keyDown to the word tracker. `modified`: ⌘, ⌃ or ⌥ is held, so
    /// the key is a shortcut rather than typing. A shortcut starts a fresh
    /// word -- ⌃C, ⌃U or ⌃L leave a terminal on a clean prompt, ⌥⌫ and
    /// ⌥-arrows land on a word edge -- except paste, which leaves unseen text
    /// glued before the caret.
    func track(keyCode: UInt16, isShift: Bool, modified: Bool) {
        guard let key = KeyCode(rawValue: keyCode) else {
            tracker.interrupt()
            return
        }
        if modified {
            if key == .v { tracker.interrupt() } else { tracker.boundary() }
            return
        }
        if let letter = key.asciiLetter {
            tracker.letter(isShift ? Character(letter.uppercased()) : letter)
        } else if key == .delete {
            tracker.backspace()
        } else if key == .space {
            tracker.space()
        } else if key.startsWord {
            tracker.boundary()
        } else if !key.isModifier {
            // Digits, punctuation, forward delete, keypad…: glued to the word.
            // (Modifier keys arrive as flagsChanged, never keyDown.)
            tracker.interrupt()
        }
    }
}
