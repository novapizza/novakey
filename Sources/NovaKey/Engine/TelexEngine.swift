// TelexEngine.swift
// Core Telex input processing engine.
// Pure Swift, no UI or system dependencies -- fully testable.

import Foundation

// MARK: - Engine Result

/// The result of processing a single keystroke through the engine.
enum EngineResult: Equatable {
    /// Let the original key event pass through unchanged.
    case passThrough

    /// Suppress the original key event and send replacement characters.
    /// `backspaces`: how many backspace events to send first.
    /// `text`: the replacement Unicode string to send after backspaces.
    case replace(backspaces: Int, text: String)

    /// The key is a word break -- reset the session and let it through.
    case wordBreak

    /// Emit a replacement (backspace + raw text) THEN let the original key
    /// event pass through. Used when the accumulated syllable is invalid
    /// at word-break time and must be restored to the raw keystrokes.
    case restore(backspaces: Int, text: String)
}

// MARK: - Telex Engine

/// Processes keystrokes according to the Telex input method.
/// Maintains a syllable buffer and produces engine results that tell the
/// event tap what to do (pass through, replace, or reset).
final class TelexEngine {

    // MARK: - State

    /// The current syllable being composed.
    private(set) var buffer = SyllableBuffer()

    /// Whether Vietnamese mode is active.
    var isVietnameseMode: Bool = true

    /// "Quick Vietnamese" (opt-in): a lone `w` typed right after a valid
    /// syllable-initial consonant (cluster) becomes `ư`, e.g. "tw" -> "tư",
    /// "chw" -> "chư". Also turns on a real-time English-word guard that reverts
    /// invalid mid-word transformations to literal keystrokes. Off by default;
    /// mirrors the Windows port.
    var quickVietnamese: Bool = false

    /// The last raw key that was typed (for double-press detection).
    private var lastRawKey: Character? = nil

    /// Raw letter keystrokes typed in the current session, with original case.
    /// Used to restore the original keys if the syllable fails spelling check
    /// at word-break. Cleared on reset and on backspace (since backspace makes
    /// exact restoration impossible).
    private var rawKeystrokes: String = ""

    /// The syllable state at the moment of the most recent word-break.
    /// If the user immediately presses backspace (to delete the word-break
    /// character), we re-enter this state so further Telex keys continue to
    /// operate on the previous word. Cleared as soon as any non-backspace key
    /// follows the word-break.
    private var savedBuffer: SyllableBuffer? = nil
    private var savedRawKeystrokes: String = ""

    // MARK: - Main Entry Point

    /// Process a single keystroke.
    ///
    /// - Parameters:
    ///   - keyCode: The macOS virtual keycode.
    ///   - isShift: Whether Shift is held.
    ///   - hasCommandOrControl: Whether Cmd or Ctrl is held.
    ///   - hasOption: Whether Option is held.
    /// - Returns: An `EngineResult` telling the event tap what to do.
    func processKey(
        keyCode: UInt16,
        isShift: Bool = false,
        hasCommandOrControl: Bool = false,
        hasOption: Bool = false
    ) -> EngineResult {
        guard let key = KeyCode(rawValue: keyCode) else {
            return .passThrough
        }

        // Modifier combos (Cmd+C, Ctrl+A, etc.) always pass through and reset
        if hasCommandOrControl {
            resetSession()
            return .passThrough
        }

        // Option key combos pass through and reset
        if hasOption {
            resetSession()
            return .passThrough
        }

        // Word break keys: check for invalid syllable and restore if needed
        if key.isWordBreak {
            if let restore = restoreIfInvalid() {
                // After an invalid-syllable restore, the visible text is the
                // raw keys -- not the composed buffer -- so we cannot reliably
                // resume editing on backspace.
                savedBuffer = nil
                resetSession()
                return restore
            }
            // Save the completed syllable so the user can re-enter it by
            // pressing backspace to delete the word-break character.
            if !buffer.isEmpty {
                savedBuffer = buffer
                savedRawKeystrokes = rawKeystrokes
            } else {
                savedBuffer = nil
            }
            resetSession()
            return .wordBreak
        }

        // Backspace: if we just word-broke with a non-empty syllable, the
        // user is stepping back into that word -- rehydrate the buffer so
        // subsequent Telex keys act on it.
        if key == .delete {
            if buffer.isEmpty, let saved = savedBuffer {
                buffer = saved
                rawKeystrokes = savedRawKeystrokes
                lastRawKey = savedRawKeystrokes.last?.lowercased().first
                savedBuffer = nil
                return .passThrough
            }
            return handleBackspace()
        }

        // Any other key following a word-break means the user is starting a
        // new word, so discard the saved state.
        savedBuffer = nil

        // Vietnamese mode off: just track nothing
        if !isVietnameseMode {
            return .passThrough
        }

        // Get the ASCII character for this key
        guard let ascii = key.asciiLetter else {
            // Non-letter key that isn't a word break -- reset and pass through
            resetSession()
            return .passThrough
        }

        let char = isShift ? Character(ascii.uppercased()) : ascii

        // What is currently on screen, before this keystroke.
        let screenBefore = buffer.text

        // Record the raw keystroke (with original case) for potential
        // restoration on word-break if the syllable is invalid.
        rawKeystrokes.append(char)

        let result = processLetter(char, isUpperCase: isShift)

        // Real-time English-word guard -- part of Quick Vietnamese (kept off in
        // default mode to preserve exact legacy behavior). If the composition is
        // now a structurally invalid syllable carrying a diacritic that no longer
        // matches the raw keys, abandon it and restore the literal keystrokes
        // immediately -- don't wait for the word break. e.g. "huawei": "hua"+w
        // -> "hưa" (valid), +e -> "hưae" (invalid) -> restore "huawe", then +i
        // -> "huawei". Because every prefix of a valid Vietnamese nucleus is
        // itself valid, real Vietnamese words never hit this mid-word; only
        // foreign/mixed input does. Runs here (not in processLetter) so it also
        // covers the tone-key and d-key paths.
        if quickVietnamese,
           hasVisibleTransformation(),
           !SpellingChecker.isValidSyllable(buffer),
           buffer.text != rawKeystrokes {
            let raw = rawKeystrokes
            rebuildAsLiteral(raw)
            return buildReplacement(oldText: screenBefore, newText: raw)
        }

        return result
    }

    /// Reset the syllable buffer and start fresh.
    func resetSession() {
        buffer.reset()
        lastRawKey = nil
        rawKeystrokes = ""
    }

    // MARK: - Quick Vietnamese Helpers

    /// Whether the buffer currently holds exactly one valid syllable-initial
    /// consonant cluster and no vowels yet -- the precondition for Quick
    /// Vietnamese turning a following `w` into `ư`.
    private func isQuickInitial() -> Bool {
        guard buffer.vowelCount == 0, !buffer.isEmpty else { return false }
        switch buffer.text.lowercased() {
        case "b", "c", "d", "đ", "g", "h", "l", "m", "n", "r", "s", "t", "v", "x",
             "ch", "kh", "ng", "nh", "ph", "th", "tr":
            return true
        default:
            return false
        }
    }

    /// Whether any character carries a visible diacritic transformation (a vowel
    /// modifier or a tone). A bare đ does not count -- a vowel-less "đ" is a
    /// legitimate standalone result.
    private func hasVisibleTransformation() -> Bool {
        buffer.chars.contains { $0.modifier != .none || $0.tone != .none }
    }

    /// Replace the buffer with the raw keystrokes as plain, unmodified letters,
    /// so the remainder of the word is composed literally. Used by the inline
    /// English-word guard once a transformation has proven to be a dead end.
    private func rebuildAsLiteral(_ raw: String) {
        buffer.reset()
        for c in raw {
            let lower = c.lowercased().first ?? c
            buffer.append(ViChar(base: lower, isUpperCase: c.isUppercase))
        }
    }

    // MARK: - Letter Processing

    private func processLetter(_ char: Character, isUpperCase: Bool) -> EngineResult {
        let lower = char.lowercased().first!

        // Check if this is a Telex tone key (s, f, r, x, j, z)
        if let tone = VietnameseData.telexToneKeys[lower], buffer.vowelCount > 0 {
            let result = handleToneKey(lower, tone: tone, isUpperCase: isUpperCase)
            if result != nil {
                lastRawKey = lower
                return result!
            }
        }

        // Check if this is a d-stroke trigger (dd -> đ)
        if lower == "d" {
            let result = handleDKey(isUpperCase: isUpperCase)
            lastRawKey = lower
            return result
        }

        // Check if this is a vowel modifier trigger (aa, ee, oo, aw, ow, uw, w)
        if lower == "w" || isDoubleKeyTrigger(lower) {
            if let result = handleVowelModifier(lower, isUpperCase: isUpperCase) {
                lastRawKey = lower
                return result
            }
        }

        // Regular letter -- add to buffer. Capture the app's current display
        // (= buffer text BEFORE this append) so any re-check that follows can
        // compute a correct backspace-and-replace diff. The app hasn't seen
        // the new letter yet, so `buffer.text` post-append is NOT a valid
        // oldText.
        let preAppendText = buffer.text
        let viChar = ViChar(base: lower, isUpperCase: isUpperCase)
        buffer.append(viChar)

        // After adding any letter, re-check tone placement. This matters
        // for vowel appends too -- e.g. "hos" -> "hó", then typing "a"
        // should produce "hoá" (tone moves to the second vowel), not "hóa".
        if buffer.currentTone != .none,
           let currentToneIdx = buffer.toneIndex,
           let newPosition = TonePlacement.findTonePosition(in: buffer),
           newPosition != currentToneIdx {
            buffer.moveTone(to: newPosition)
            lastRawKey = lower
            return buildReplacement(oldText: preAppendText, newText: buffer.text)
        }

        lastRawKey = lower
        return .passThrough
    }

    // MARK: - Tone Handling

    /// Handle a tone key press (s, f, r, x, j, z).
    /// Returns nil if the tone cannot be applied (treat as regular letter).
    private func handleToneKey(_ key: Character, tone: ToneMark, isUpperCase: Bool) -> EngineResult? {
        // z key: remove existing tone
        if tone == .none {
            return handleRemoveTone(key, isUpperCase: isUpperCase)
        }

        // If same tone is already applied, undo it (double-press reversal)
        if buffer.currentTone == tone {
            return undoTone(key, isUpperCase: isUpperCase)
        }

        // Only reinterpret the key as a tone mark when the current syllable
        // is structurally valid Vietnamese. Otherwise English words trigger
        // spurious tones ("class" -> "clás") and an undone tone re-applies
        // on the next press ("cor" + r -> "cỏr" -> "corr" oscillation).
        guard SpellingChecker.isValidSyllable(buffer) else { return nil }

        // Find the correct position for the tone mark
        guard let position = TonePlacement.findTonePosition(in: buffer) else {
            return nil
        }

        // Capture old state for backspace calculation
        let oldText = buffer.text

        // Apply the tone
        buffer.applyTone(tone, at: position)

        let newText = buffer.text
        return buildReplacement(oldText: oldText, newText: newText)
    }

    /// Handle z key: remove existing tone mark.
    private func handleRemoveTone(_ key: Character, isUpperCase: Bool) -> EngineResult? {
        guard buffer.currentTone != .none else {
            // No tone to remove -- treat as regular letter
            return nil
        }

        let oldText = buffer.text
        buffer.applyTone(.none, at: 0) // index doesn't matter for .none
        let newText = buffer.text
        return buildReplacement(oldText: oldText, newText: newText)
    }

    /// Undo a tone mark when the same tone key is pressed again.
    /// e.g., "as" -> "á", then "ass" -> "as"
    private func undoTone(_ key: Character, isUpperCase: Bool) -> EngineResult {
        let oldText = buffer.text

        // Remove the tone
        buffer.applyTone(.none, at: 0)

        // Add the key as a literal character
        let viChar = ViChar(base: key, isUpperCase: isUpperCase)
        buffer.append(viChar)

        let newText = buffer.text
        return buildReplacement(oldText: oldText, newText: newText)
    }

    // MARK: - D-Stroke Handling

    /// Handle the 'd' key. If the last character in the buffer is also 'd',
    /// convert to đ. Otherwise, add as regular 'd'.
    private func handleDKey(isUpperCase: Bool) -> EngineResult {
        // Only trigger dd -> đ when the 'd' is immediately adjacent (the last
        // char in the buffer) AND is the sole character so far: đ only occurs
        // syllable-initially in Vietnamese. Matching a 'd' later in the
        // syllable would wrongly convert English words like "add" or
        // "disabled".
        if let last = buffer.chars.last, last.base == "d", !last.hasDStroke,
           buffer.count == 1 {
            // Double d -> đ
            let oldText = buffer.text
            buffer.applyDStroke(at: buffer.count - 1)
            let newText = buffer.text

            // The second 'd' is consumed; we replace the first 'd' with 'đ'
            return buildReplacement(oldText: oldText, newText: newText)
        }

        // If already has đ and typing another d -> undo (đd -> dd)
        if buffer.hasDStroke, lastRawKey == "d" {
            let oldText = buffer.text
            buffer.removeDStroke()
            let viChar = ViChar(base: "d", isUpperCase: isUpperCase)
            buffer.append(viChar)
            let newText = buffer.text
            return buildReplacement(oldText: oldText, newText: newText)
        }

        // Regular d
        let viChar = ViChar(base: "d", isUpperCase: isUpperCase)
        buffer.append(viChar)
        return .passThrough
    }

    // MARK: - Vowel Modifier Handling

    /// Check if typing this character triggers a double-key modifier.
    /// (a after a -> â, e after e -> ê, o after o -> ô)
    private func isDoubleKeyTrigger(_ char: Character) -> Bool {
        guard let last = lastRawKey else { return false }
        return last == char && (char == "a" || char == "e" || char == "o")
    }

    /// Handle vowel modifier keys (aa->â, ee->ê, oo->ô, aw->ă, ow->ơ, uw->ư, w standalone).
    /// Returns nil if no modification can be applied.
    private func handleVowelModifier(_ key: Character, isUpperCase: Bool) -> EngineResult? {
        // Escape hatch: a 'w' typed right after a 'ư' that was conjured from a
        // *lone* 'w' (standalone `w`->`ư`, or Quick Vietnamese `<init>w`->`ư`)
        // reverts it to a literal 'w'. Keeps English/mixed fragments intact:
        // "w"+w -> "w", "tw"+w -> "tw" -- instead of leaking a spurious 'u'
        // ("tuw"). A ư from a real "uw" (u actually typed) is unaffected and
        // still reverts to "u"+"w". Provenance-based via the bareW flag, this
        // subsumes the old rawKeystrokes == "ww" special case.
        if key == "w", let last = buffer.chars.last,
           last.base == "u", last.modifier == .horn, last.tone == .none, last.bareW {
            let oldText = buffer.text
            buffer.removeLast()
            buffer.append(ViChar(base: "w", isUpperCase: isUpperCase))
            let newText = buffer.text
            return buildReplacement(oldText: oldText, newText: newText)
        }

        // Escape: "ưa" + another 'w' reverts the horn, yielding literal "uaw".
        // The horn-u here was conjured by the "ua"+w rule below; since there is
        // no "ưă" syllable in Vietnamese, a 'w' at this point was never meant to
        // breve the 'a' -- it's the user undoing the horn (double-press style).
        // e.g. "Huawei": "hua"+w -> "hưa", +w -> "huaw", then +e,+i -> "huawei".
        if key == "w", buffer.count >= 2 {
            let n = buffer.count
            let a = buffer.chars[n - 1]
            let u = buffer.chars[n - 2]
            if a.base == "a", a.modifier == .none, a.tone == .none,
               u.base == "u", u.modifier == .horn, u.tone == .none {
                let oldText = buffer.text
                buffer.removeModifier(at: n - 2)
                buffer.append(ViChar(base: "w", isUpperCase: isUpperCase))
                let newText = buffer.text
                return buildReplacement(oldText: oldText, newText: newText)
            }
        }

        // Special case: "ua" + w -> "ưa" (horn on u, a stays plain).
        // Without this, the "aw -> ă" rule fires first and gives "uă" instead.
        // Same guard as the "uo" propagation: skip if u is part of "qu" cluster.
        if key == "w",
           SpellingChecker.isValidSyllable(buffer),
           let aIdx = buffer.lastIndex(ofBase: "a"),
           aIdx > 0,
           buffer.chars[aIdx].modifier == .none {
            let prev = buffer.chars[aIdx - 1]
            let precededByQ = aIdx >= 2 && buffer.chars[aIdx - 2].base == "q"
            if prev.base == "u" && prev.modifier == .none && !precededByQ {
                let oldText = buffer.text
                buffer.applyModifier(.horn, at: aIdx - 1)
                let newText = buffer.text
                return buildReplacement(oldText: oldText, newText: newText)
            }
        }

        // Try each modifier rule
        for rule in VietnameseData.telexVowelModifiers {
            if rule.trigger == key {
                // Find the target vowel in the buffer
                if let targetIdx = buffer.lastIndex(ofBase: rule.target) {
                    let currentMod = buffer.chars[targetIdx].modifier

                    // If already has this modifier -> undo (double press reversal)
                    if currentMod == rule.modifier {
                        return undoVowelModifier(key, targetIndex: targetIdx, isUpperCase: isUpperCase)
                    }

                    // If no modifier yet -> apply, but only when the current
                    // syllable is structurally valid Vietnamese. This keeps
                    // English words literal ("know" stays "know", the "kno"
                    // initial is invalid so 'w' never becomes a horn).
                    if currentMod == .none {
                        guard SpellingChecker.isValidSyllable(buffer) else { return nil }
                        let oldText = buffer.text
                        buffer.applyModifier(rule.modifier, at: targetIdx)
                        // "uo" + w -> "ươ": when horn is applied to an 'o'
                        // immediately preceded by an unmodified 'u', propagate
                        // horn to the 'u' as well. Exception: "qu" + 'o' + w
                        // should only horn the 'o' (giving quơ, not qươ) since
                        // the 'u' there is part of the consonant cluster.
                        if rule.modifier == .horn && rule.target == "o" && targetIdx > 0 {
                            let prev = buffer.chars[targetIdx - 1]
                            let precededByQ = targetIdx >= 2
                                && buffer.chars[targetIdx - 2].base == "q"
                            if prev.base == "u" && prev.modifier == .none && !precededByQ {
                                buffer.applyModifier(.horn, at: targetIdx - 1)
                            }
                        }
                        let newText = buffer.text
                        return buildReplacement(oldText: oldText, newText: newText)
                    }
                }
            }
        }

        // Standalone 'w' -> 'ư' when there's no target vowel to modify.
        // Also allowed right after a syllable-initial đ ("ddw" -> "đư", so
        // "ddwowngf" -> "đường"). đ never occurs in English words, so this
        // cannot misfire -- unlike a general consonant+w rule, which would
        // wreck words like "swift". With Quick Vietnamese on, it also fires
        // right after any valid initial consonant cluster ("tw" -> "tư").
        if key == "w" && (buffer.isEmpty
            || (buffer.hasDStroke && buffer.vowelCount == 0)
            || (quickVietnamese && isQuickInitial())) {
            // Mark it so a following 'w' can escape back to a literal 'w'.
            let viChar = ViChar(base: "u", modifier: .horn, isUpperCase: isUpperCase, bareW: true)
            buffer.append(viChar)
            // Replace the 'w' keystroke with 'ư'
            return .replace(backspaces: 0, text: String(viChar.unicode))
        }

        return nil
    }

    /// Undo a vowel modifier when the same trigger is pressed again.
    /// e.g., "aa" -> "â", then "aaa" -> "aa"
    private func undoVowelModifier(_ key: Character, targetIndex: Int, isUpperCase: Bool) -> EngineResult {
        let oldText = buffer.text

        // Remove the modifier
        buffer.removeModifier(at: targetIndex)

        // Add the key as a literal character
        let viChar = ViChar(base: key, isUpperCase: isUpperCase)
        buffer.append(viChar)

        let newText = buffer.text
        return buildReplacement(oldText: oldText, newText: newText)
    }

    // MARK: - Backspace Handling

    private func handleBackspace() -> EngineResult {
        buffer.removeLast()
        // Once the user corrects mid-word, we cannot reliably reconstruct
        // the original raw keystrokes, so we disable the restore path for
        // the remainder of this session.
        rawKeystrokes = ""
        return .passThrough
    }

    // MARK: - Spelling Restore

    /// If the composed text diverged from the raw keystrokes and does not
    /// form a structurally valid Vietnamese syllable, return a restore result
    /// that replaces the composed text with the raw keystrokes the user
    /// originally typed. Returns nil when no restore is needed.
    private func restoreIfInvalid() -> EngineResult? {
        guard !buffer.isEmpty else { return nil }
        guard !rawKeystrokes.isEmpty else { return nil }

        // Only restore when a tone or vowel-modifier transformation is still
        // visible in the buffer.
        //
        // A double-press undo is always trusted as deliberate: the composed
        // text is what the user wants ("disst" -> "dist", "noww" -> "now"),
        // so consumed keys are never resurrected from the raw record. The
        // cost is that typing an English double letter without the extra
        // escape press composes one letter short ("correction" with two r's
        // -> "corection") -- same tradeoff as Unikey without a dictionary.
        //
        // đ deliberately does NOT count: dd -> đ only fires syllable-
        // initially, so it can't happen by accident, and a vowel-less "đ"
        // is legitimate on its own (currency "50.000đ", shorthand "đc").
        guard hasVisibleTransformation() else { return nil }

        // Don't restore if the syllable is structurally valid.
        if SpellingChecker.isValidSyllable(buffer) { return nil }

        // Nothing to fix if the screen already shows the raw keystrokes.
        let composed = buffer.text
        if composed == rawKeystrokes { return nil }

        return .restore(backspaces: composed.count, text: rawKeystrokes)
    }

    // MARK: - Replacement Building

    /// Build an EngineResult.replace by comparing old and new buffer text.
    /// Calculates the minimum number of backspaces needed.
    private func buildReplacement(oldText: String, newText: String) -> EngineResult {
        // Find the common prefix length
        let commonPrefix = zip(oldText, newText).prefix(while: { $0 == $1 }).count

        // Number of old characters to delete (after the common prefix)
        let backspaces = oldText.count - commonPrefix

        // New characters to send (after the common prefix)
        let newSuffix = String(newText.dropFirst(commonPrefix))

        if backspaces == 0 && newSuffix.isEmpty {
            return .passThrough
        }

        return .replace(backspaces: backspaces, text: newSuffix)
    }
}
