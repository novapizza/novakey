// NovaKey Test Runner
// Compile: swiftc -o /tmp/novakey_tests Sources/NovaKey/Engine/*.swift \
//              Sources/NovaKey/Settings/*.swift Sources/NovaKey/App/Constants.swift \
//              Tests/run_tests.swift -framework Carbon
// Run:     /tmp/novakey_tests

import CoreGraphics
import Foundation

// ============================================================
// Minimal test harness
// ============================================================
var totalTests = 0
var passedTests = 0
var failedTests: [(String, String)] = []

func test(_ name: String, _ body: () throws -> Void) {
    totalTests += 1
    do {
        try body()
        passedTests += 1
        print("  PASS  \(name)")
    } catch {
        failedTests.append((name, "\(error)"))
        print("  FAIL  \(name) -- \(error)")
    }
}

struct AssertionError: Error, CustomStringConvertible {
    let description: String
}
func expect<T: Equatable>(_ actual: T, _ expected: T, file: String = #file, line: Int = #line) throws {
    if actual != expected {
        throw AssertionError(description: "Expected \(expected), got \(actual) at line \(line)")
    }
}
func expectNil<T>(_ value: T?, file: String = #file, line: Int = #line) throws {
    if value != nil {
        throw AssertionError(description: "Expected nil, got \(value!) at line \(line)")
    }
}
func expectTrue(_ value: Bool, file: String = #file, line: Int = #line) throws {
    if !value {
        throw AssertionError(description: "Expected true at line \(line)")
    }
}
func expectFalse(_ value: Bool, file: String = #file, line: Int = #line) throws {
    if value {
        throw AssertionError(description: "Expected false at line \(line)")
    }
}

// ============================================================
// Test helper: type into engine
// ============================================================
func keyCodeFor(_ char: Character) -> UInt16 {
    switch char {
    case "a": return KeyCode.a.rawValue
    case "b": return KeyCode.b.rawValue
    case "c": return KeyCode.c.rawValue
    case "d": return KeyCode.d.rawValue
    case "e": return KeyCode.e.rawValue
    case "f": return KeyCode.f.rawValue
    case "g": return KeyCode.g.rawValue
    case "h": return KeyCode.h.rawValue
    case "i": return KeyCode.i.rawValue
    case "j": return KeyCode.j.rawValue
    case "k": return KeyCode.k.rawValue
    case "l": return KeyCode.l.rawValue
    case "m": return KeyCode.m.rawValue
    case "n": return KeyCode.n.rawValue
    case "o": return KeyCode.o.rawValue
    case "p": return KeyCode.p.rawValue
    case "q": return KeyCode.q.rawValue
    case "r": return KeyCode.r.rawValue
    case "s": return KeyCode.s.rawValue
    case "t": return KeyCode.t.rawValue
    case "u": return KeyCode.u.rawValue
    case "v": return KeyCode.v.rawValue
    case "w": return KeyCode.w.rawValue
    case "x": return KeyCode.x.rawValue
    case "y": return KeyCode.y.rawValue
    case "z": return KeyCode.z.rawValue
    default: return 0xFF
    }
}

func typeAndGetText(_ chars: String) -> String {
    let engine = TelexEngine()
    engine.isVietnameseMode = true
    for char in chars {
        let lower = char.lowercased().first!
        let isShift = char.isUppercase
        let keyCode = keyCodeFor(lower)
        _ = engine.processKey(keyCode: keyCode, isShift: isShift)
    }
    return engine.buffer.text
}

func makeBuffer(_ text: String) -> SyllableBuffer {
    var buffer = SyllableBuffer()
    for char in text.lowercased() {
        buffer.append(ViChar(base: char))
    }
    return buffer
}

/// Type a sequence of letters, then press space, and return the engine
/// result for the space key and the composed text right before the break.
func typeThenSpace(_ chars: String) -> (space: EngineResult, composed: String) {
    let engine = TelexEngine()
    engine.isVietnameseMode = true
    for char in chars {
        let lower = char.lowercased().first!
        let isShift = char.isUppercase
        let keyCode = keyCodeFor(lower)
        _ = engine.processKey(keyCode: keyCode, isShift: isShift)
    }
    let composed = engine.buffer.text
    let result = engine.processKey(keyCode: KeyCode.space.rawValue)
    return (result, composed)
}

/// Type a sequence of letters with Quick Vietnamese optionally enabled.
func typeQuick(_ chars: String, quick: Bool) -> String {
    let engine = TelexEngine()
    engine.isVietnameseMode = true
    engine.quickVietnamese = quick
    for char in chars {
        let lower = char.lowercased().first!
        let isShift = char.isUppercase
        let keyCode = keyCodeFor(lower)
        _ = engine.processKey(keyCode: keyCode, isShift: isShift)
    }
    return engine.buffer.text
}

/// Type a sequence of letters with Deferred diacritics ("Bỏ dấu sau")
/// optionally enabled. Deferred diacritics requires Quick Vietnamese, so
/// enabling it turns on both flags. Returns the composed buffer text.
func typeDeferred(_ chars: String, deferred: Bool) -> String {
    let engine = TelexEngine()
    engine.isVietnameseMode = true
    engine.quickVietnamese = deferred
    engine.deferredDiacritics = deferred
    for char in chars {
        let lower = char.lowercased().first!
        _ = engine.processKey(keyCode: keyCodeFor(lower), isShift: char.isUppercase)
    }
    return engine.buffer.text
}

/// Apply every EngineResult to a running visible string -- validates exact
/// backspace counts, not just the final buffer text.
func simulateDeferred(_ chars: String, deferred: Bool) -> String {
    let engine = TelexEngine()
    engine.isVietnameseMode = true
    engine.quickVietnamese = deferred
    engine.deferredDiacritics = deferred
    var visible: [Character] = []
    for char in chars {
        let isShift = char.isUppercase
        let lower = char.lowercased().first!
        let result = engine.processKey(keyCode: keyCodeFor(lower), isShift: isShift)
        switch result {
        case .passThrough, .wordBreak:
            visible.append(isShift ? Character(lower.uppercased()) : lower)
        case .replace(let bs, let text), .restore(let bs, let text):
            visible.removeLast(min(bs, visible.count))
            visible.append(contentsOf: text)
        }
    }
    return String(visible)
}

/// Type letters (deferred on) then a space; return (break result, composed).
func typeThenSpaceDeferred(_ chars: String) -> (space: EngineResult, composed: String) {
    let engine = TelexEngine()
    engine.isVietnameseMode = true
    engine.quickVietnamese = true
    engine.deferredDiacritics = true
    for char in chars {
        let lower = char.lowercased().first!
        _ = engine.processKey(keyCode: keyCodeFor(lower), isShift: char.isUppercase)
    }
    let composed = engine.buffer.text
    let result = engine.processKey(keyCode: KeyCode.space.rawValue)
    return (result, composed)
}

// ============================================================
// Main entry point
// ============================================================
@main
struct TestRunner {
    static func main() {
        runAllTests()
        printSummary()
    }
}

func runAllTests() {
// ============================================================
// SyllableBuffer Tests
// ============================================================
print("\n--- SyllableBuffer Tests ---")

test("empty buffer") {
    let buffer = SyllableBuffer()
    try expectTrue(buffer.isEmpty)
    try expect(buffer.count, 0)
    try expect(buffer.text, "")
}

test("append and text") {
    var buffer = SyllableBuffer()
    buffer.append(ViChar(base: "h"))
    buffer.append(ViChar(base: "a"))
    try expect(buffer.text, "ha")
    try expect(buffer.count, 2)
}

test("remove last") {
    var buffer = SyllableBuffer()
    buffer.append(ViChar(base: "a"))
    buffer.append(ViChar(base: "b"))
    let removed = buffer.removeLast()
    try expect(removed?.base, Optional("b"))
    try expect(buffer.text, "a")
}

test("reset") {
    var buffer = SyllableBuffer()
    buffer.append(ViChar(base: "t"))
    buffer.append(ViChar(base: "o"))
    buffer.applyTone(.sac, at: 1)
    buffer.reset()
    try expectTrue(buffer.isEmpty)
    try expect(buffer.currentTone, ToneMark.none)
    try expectNil(buffer.toneIndex)
}

test("vowel indices") {
    var buffer = SyllableBuffer()
    buffer.append(ViChar(base: "t"))
    buffer.append(ViChar(base: "o"))
    buffer.append(ViChar(base: "a"))
    buffer.append(ViChar(base: "n"))
    try expect(buffer.vowelIndices, [1, 2])
    try expect(buffer.vowelCount, 2)
    try expect(buffer.firstVowelIndex, Optional(1))
    try expect(buffer.lastVowelIndex, Optional(2))
}

test("ending consonant") {
    var buffer = SyllableBuffer()
    buffer.append(ViChar(base: "t"))
    buffer.append(ViChar(base: "o"))
    buffer.append(ViChar(base: "a"))
    try expectFalse(buffer.hasEndingConsonant)
    buffer.append(ViChar(base: "n"))
    try expectTrue(buffer.hasEndingConsonant)
    try expect(buffer.endingConsonant, "n")
}

test("apply tone") {
    var buffer = SyllableBuffer()
    buffer.append(ViChar(base: "a"))
    buffer.applyTone(.sac, at: 0)
    try expect(buffer.text, "\u{00E1}") // á
    try expect(buffer.currentTone, ToneMark.sac)
    try expect(buffer.toneIndex, Optional(0))
}

test("apply modifier circumflex") {
    var buffer = SyllableBuffer()
    buffer.append(ViChar(base: "a"))
    buffer.applyModifier(.circumflex, at: 0)
    try expect(buffer.text, "\u{00E2}") // â
}

test("apply tone + modifier") {
    var buffer = SyllableBuffer()
    buffer.append(ViChar(base: "a"))
    buffer.applyModifier(.circumflex, at: 0)
    buffer.applyTone(.sac, at: 0)
    try expect(buffer.text, "\u{1EA5}") // ấ
}

test("move tone") {
    var buffer = SyllableBuffer()
    buffer.append(ViChar(base: "t"))
    buffer.append(ViChar(base: "o"))
    buffer.append(ViChar(base: "a"))
    buffer.applyTone(.sac, at: 1)
    buffer.moveTone(to: 2)
    try expect(buffer.chars[1].tone, ToneMark.none)
    try expect(buffer.chars[2].tone, ToneMark.sac)
}

test("initial/vowel/ending consonant parsing") {
    var buffer = SyllableBuffer()
    buffer.append(ViChar(base: "t"))
    buffer.append(ViChar(base: "r"))
    buffer.append(ViChar(base: "o"))
    buffer.append(ViChar(base: "n"))
    buffer.append(ViChar(base: "g"))
    try expect(buffer.initialConsonant, "tr")
    try expect(buffer.vowelCluster, "o")
    try expect(buffer.endingConsonant, "ng")
}

// ============================================================
// TonePlacement Tests
// ============================================================
print("\n--- TonePlacement Tests ---")

test("single vowel 'ba'") {
    let buffer = makeBuffer("ba")
    try expect(TonePlacement.findTonePosition(in: buffer), Optional(1))
}

test("single vowel 'ti'") {
    let buffer = makeBuffer("ti")
    try expect(TonePlacement.findTonePosition(in: buffer), Optional(1))
}

test("two vowels + ending: 'toan'") {
    let buffer = makeBuffer("toan")
    try expect(TonePlacement.findTonePosition(in: buffer), Optional(2))
}

test("two vowels + ending: 'hoang'") {
    let buffer = makeBuffer("hoang")
    try expect(TonePlacement.findTonePosition(in: buffer), Optional(2))
}

test("two vowels no ending: 'hoa'") {
    let buffer = makeBuffer("hoa")
    try expect(TonePlacement.findTonePosition(in: buffer), Optional(2))
}

test("falling diphthong: 'hai'") {
    let buffer = makeBuffer("hai")
    try expect(TonePlacement.findTonePosition(in: buffer), Optional(1))
}

test("falling diphthong: 'cao'") {
    let buffer = makeBuffer("cao")
    try expect(TonePlacement.findTonePosition(in: buffer), Optional(1))
}

test("three vowels: 'khoai'") {
    let buffer = makeBuffer("khoai")
    try expect(TonePlacement.findTonePosition(in: buffer), Optional(3))
}

test("qu cluster: 'quan'") {
    let buffer = makeBuffer("quan")
    try expect(TonePlacement.findTonePosition(in: buffer), Optional(2))
}

test("no vowels: 'tr'") {
    let buffer = makeBuffer("tr")
    try expectNil(TonePlacement.findTonePosition(in: buffer))
}

// ============================================================
// TelexEngine Tests
// ============================================================
print("\n--- TelexEngine Tests ---")

test("tone: sắc (as -> á)") {
    try expect(typeAndGetText("as"), "\u{00E1}")
}

test("tone: huyền (af -> à)") {
    try expect(typeAndGetText("af"), "\u{00E0}")
}

test("tone: hỏi (ar -> ả)") {
    try expect(typeAndGetText("ar"), "\u{1EA3}")
}

test("tone: ngã (ax -> ã)") {
    try expect(typeAndGetText("ax"), "\u{00E3}")
}

test("tone: nặng (aj -> ạ)") {
    try expect(typeAndGetText("aj"), "\u{1EA1}")
}

test("remove tone: asz -> a") {
    try expect(typeAndGetText("asz"), "a")
}

test("circumflex: aa -> â") {
    try expect(typeAndGetText("aa"), "\u{00E2}")
}

test("circumflex: ee -> ê") {
    try expect(typeAndGetText("ee"), "\u{00EA}")
}

test("circumflex: oo -> ô") {
    try expect(typeAndGetText("oo"), "\u{00F4}")
}

test("breve: aw -> ă") {
    try expect(typeAndGetText("aw"), "\u{0103}")
}

test("horn: ow -> ơ") {
    try expect(typeAndGetText("ow"), "\u{01A1}")
}

test("horn: uw -> ư") {
    try expect(typeAndGetText("uw"), "\u{01B0}")
}

test("ww -> w (standalone reversal)") {
    // First 'w' on empty buffer -> 'ư'. Second 'w' should revert to literal 'w'.
    try expect(typeAndGetText("ww"), "w")
}

test("uww -> uw (undo via double w after uw)") {
    // After 'u' + 'w' -> 'ư', pressing 'w' again removes horn and appends 'w'.
    try expect(typeAndGetText("uww"), "uw")
}

test("d-stroke: dd -> đ") {
    try expect(typeAndGetText("dd"), "\u{0111}")
}

test("combined: Vieejt -> Việt") {
    try expect(typeAndGetText("Vieejt"), "Việt")
}

test("english mode: passthrough") {
    let engine = TelexEngine()
    engine.isVietnameseMode = false
    let result = engine.processKey(keyCode: KeyCode.a.rawValue)
    try expect(result, EngineResult.passThrough)
}

test("word break: space resets buffer") {
    let engine = TelexEngine()
    engine.isVietnameseMode = true
    _ = engine.processKey(keyCode: KeyCode.a.rawValue)
    _ = engine.processKey(keyCode: KeyCode.s.rawValue)
    let result = engine.processKey(keyCode: KeyCode.space.rawValue)
    try expect(result, EngineResult.wordBreak)
    try expectTrue(engine.buffer.isEmpty)
}

test("modifier key: Cmd resets buffer") {
    let engine = TelexEngine()
    engine.isVietnameseMode = true
    _ = engine.processKey(keyCode: KeyCode.a.rawValue)
    let result = engine.processKey(keyCode: KeyCode.c.rawValue, hasCommandOrControl: true)
    try expect(result, EngineResult.passThrough)
    try expectTrue(engine.buffer.isEmpty)
}

// ============================================================
// Tone re-check on late vowel append (feature #3)
// ============================================================
print("\n--- Tone Re-check on Vowel Append ---")

test("hosa -> hoá (tone moves to 2nd vowel when vowel is appended)") {
    try expect(typeAndGetText("hosa"), "ho\u{00E1}") // hoá
}

test("tosa -> toá") {
    try expect(typeAndGetText("tosa"), "to\u{00E1}") // toá
}

test("hofa -> hoà (huyen moves to 2nd vowel on vowel append)") {
    try expect(typeAndGetText("hofa"), "ho\u{00E0}") // hoà
}

// ============================================================
// Replacement diff correctness (regression: "disabled" bug)
// ============================================================
print("\n--- Replacement Diff ---")

/// Simulates what the user sees in the app by applying each EngineResult
/// to a running string buffer. Returns the final visible text.
func simulateApp(_ chars: String) -> String {
    let engine = TelexEngine()
    engine.isVietnameseMode = true
    var visible = ""
    for ch in chars {
        let lower = ch.lowercased().first!
        let isShift = ch.isUppercase
        let result = engine.processKey(keyCode: keyCodeFor(lower), isShift: isShift)
        switch result {
        case .passThrough, .wordBreak:
            visible.append(isShift ? Character(String(lower).uppercased()) : lower)
        case .replace(let bs, let text):
            if bs > 0 { visible.removeLast(min(bs, visible.count)) }
            visible.append(contentsOf: text)
        case .restore(let bs, let text):
            if bs > 0 { visible.removeLast(min(bs, visible.count)) }
            visible.append(contentsOf: text)
        }
    }
    return visible
}

test("app-visible: 'hosa' -> 'hoá' (recheck diff correct)") {
    try expect(simulateApp("hosa"), "ho\u{00E1}") // hoá, not "oá"
}

test("app-visible: 'disab' -> 'diáb' (tone shift via consonant)") {
    try expect(simulateApp("disab"), "di\u{00E1}b") // diáb, 'd' preserved
}

test("app-visible: 'disabled' -> 'diábled' (no false dd->đ)") {
    // Typing English "disabled" in Vietnamese mode should not match the
    // leading 'd' for dd->đ when the trailing 'd' arrives.
    try expect(simulateApp("disabled"), "di\u{00E1}bled") // diábled
}

test("dd still converts to đ when adjacent") {
    try expect(simulateApp("dd"), "\u{0111}") // đ
}

test("dad stays literal ('d' not adjacent)") {
    try expect(simulateApp("dad"), "dad")
}

// ============================================================
// Horn propagation for "uo" + w (feature #3)
// ============================================================
print("\n--- Horn Propagation for uo ---")

test("uow -> ươ (horn propagates to u)") {
    try expect(typeAndGetText("uow"), "\u{01B0}\u{01A1}") // ươ
}

test("thuowng -> thương") {
    try expect(typeAndGetText("thuowng"), "th\u{01B0}\u{01A1}ng") // thương
}

test("nuowcs -> nướcs? -> nước") {
    // n-u-o-w-c-s: uo+w->ươ, then c appended (ươc), then s tones ư -> ướ
    try expect(typeAndGetText("nuowcs"), "n\u{01B0}\u{1EDB}c") // nước
}

test("quow -> quơ (qu exception: no horn on u)") {
    // After "qu" the u is part of the consonant cluster, so horn should only
    // apply to the following o.
    try expect(typeAndGetText("quow"), "qu\u{01A1}") // quơ
}

// ============================================================
// Spelling check + restore on word-break (feature #2)
// ============================================================
print("\n--- Restore on Invalid Word-Break ---")

test("valid syllable 'as' + space -> no restore") {
    let (result, composed) = typeThenSpace("as")
    try expect(composed, "\u{00E1}") // á
    try expect(result, EngineResult.wordBreak)
}

test("valid 'viet' + space -> no restore (plain letters)") {
    let (result, _) = typeThenSpace("viet")
    try expect(result, EngineResult.wordBreak)
}

test("invalid 'wd' + space -> restore to raw 'wd'") {
    // 'w' alone -> ư. Then 'd' makes "ưd". 'd' is not a valid ending.
    // Buffer has horn transformation -> restore to raw "wd".
    let (result, composed) = typeThenSpace("wd")
    try expect(composed, "\u{01B0}d") // ưd
    try expect(result, EngineResult.restore(backspaces: 2, text: "wd"))
}

test("invalid 'aal' + space -> restore to 'aal'") {
    // "aa" -> â, then 'l' appended. "âl" has no valid ending (l).
    // Circumflex transformation present -> restore.
    let (result, composed) = typeThenSpace("aal")
    try expect(composed, "\u{00E2}l") // âl
    try expect(result, EngineResult.restore(backspaces: 2, text: "aal"))
}

test("plain English 'hello' + space -> no restore") {
    let (result, _) = typeThenSpace("hello")
    try expect(result, EngineResult.wordBreak)
}

test("case preserved on restore: 'AAL' -> 'AAL'") {
    let (result, _) = typeThenSpace("AAL")
    try expect(result, EngineResult.restore(backspaces: 2, text: "AAL"))
}

test("backspace disables restore for the rest of the word") {
    let engine = TelexEngine()
    engine.isVietnameseMode = true
    // Type "aa" (-> â), then backspace once (clears raw tracking),
    // then type "l" and space. No restore should fire now.
    for ch in "aa" {
        _ = engine.processKey(keyCode: keyCodeFor(ch), isShift: false)
    }
    _ = engine.processKey(keyCode: KeyCode.delete.rawValue)
    _ = engine.processKey(keyCode: keyCodeFor("l"), isShift: false)
    let result = engine.processKey(keyCode: KeyCode.space.rawValue)
    try expect(result, EngineResult.wordBreak)
}

test("word break resets buffer even after restore") {
    let engine = TelexEngine()
    engine.isVietnameseMode = true
    for ch in "wd" {
        _ = engine.processKey(keyCode: keyCodeFor(ch), isShift: false)
    }
    _ = engine.processKey(keyCode: KeyCode.space.rawValue)
    try expectTrue(engine.buffer.isEmpty)
}

// ============================================================
// Resume syllable on backspace after word-break
// ============================================================
print("\n--- Resume-on-Backspace ---")

test("cái + space + backspace + j -> cại") {
    let engine = TelexEngine()
    engine.isVietnameseMode = true
    for ch in "cais" {
        _ = engine.processKey(keyCode: keyCodeFor(ch), isShift: false)
    }
    try expect(engine.buffer.text, "c\u{00E1}i") // cái (sanity)
    let spaceResult = engine.processKey(keyCode: KeyCode.space.rawValue)
    try expect(spaceResult, EngineResult.wordBreak)
    try expectTrue(engine.buffer.isEmpty)
    // Backspace: restores saved syllable, passes through so OS deletes space
    let bsResult = engine.processKey(keyCode: KeyCode.delete.rawValue)
    try expect(bsResult, EngineResult.passThrough)
    try expect(engine.buffer.text, "c\u{00E1}i") // cái restored
    // Now type j -> should replace sắc with nặng
    _ = engine.processKey(keyCode: keyCodeFor("j"), isShift: false)
    try expect(engine.buffer.text, "c\u{1EA1}i") // cại
}

test("letter after word-break discards saved state") {
    let engine = TelexEngine()
    engine.isVietnameseMode = true
    for ch in "cais" {
        _ = engine.processKey(keyCode: keyCodeFor(ch), isShift: false)
    }
    _ = engine.processKey(keyCode: KeyCode.space.rawValue)
    // Type a new letter -- user is starting a new word.
    _ = engine.processKey(keyCode: keyCodeFor("h"), isShift: false)
    try expect(engine.buffer.text, "h")
    // Backspace now just removes 'h'; no resume should fire.
    let bsResult = engine.processKey(keyCode: KeyCode.delete.rawValue)
    try expect(bsResult, EngineResult.passThrough)
    try expectTrue(engine.buffer.isEmpty)
}

test("double word-break clears saved state") {
    let engine = TelexEngine()
    engine.isVietnameseMode = true
    for ch in "cais" {
        _ = engine.processKey(keyCode: keyCodeFor(ch), isShift: false)
    }
    _ = engine.processKey(keyCode: KeyCode.space.rawValue) // saves cái
    _ = engine.processKey(keyCode: KeyCode.space.rawValue) // empty buffer clears saved
    let bsResult = engine.processKey(keyCode: KeyCode.delete.rawValue)
    try expect(bsResult, EngineResult.passThrough)
    try expectTrue(engine.buffer.isEmpty) // no restore
}

test("invalid syllable restore does not leave resumable state") {
    // "wd" + space -> restore to raw "wd". Backspace afterwards should NOT
    // re-hydrate a composed buffer, because visible text is raw keystrokes.
    let engine = TelexEngine()
    engine.isVietnameseMode = true
    for ch in "wd" {
        _ = engine.processKey(keyCode: keyCodeFor(ch), isShift: false)
    }
    let spaceResult = engine.processKey(keyCode: KeyCode.space.rawValue)
    if case .restore = spaceResult {
        // expected
    } else {
        throw AssertionError(description: "Expected .restore, got \(spaceResult)")
    }
    let bsResult = engine.processKey(keyCode: KeyCode.delete.rawValue)
    try expect(bsResult, EngineResult.passThrough)
    try expectTrue(engine.buffer.isEmpty)
}

// ============================================================
// Validity gating: English words must not trigger Telex transforms
// ============================================================
print("\n--- Validity Gating (English-word protection) ---")

test("corr -> cor (double-press undo consumes one r, no re-apply)") {
    try expect(typeAndGetText("corr"), "cor")
}

test("corrr -> corr (invalid ending 'r' blocks tone re-apply, no oscillation)") {
    try expect(typeAndGetText("corrr"), "corr")
}

test("class stays class (invalid initial 'cl' blocks tone)") {
    try expect(typeAndGetText("class"), "class")
}

test("know stays know (invalid initial 'kn' blocks horn)") {
    try expect(typeAndGetText("know"), "know")
}

test("add stays add (dd only converts at syllable start)") {
    try expect(typeAndGetText("add"), "add")
}

test("ddoong -> đông (đ + circumflex still work at syllable start)") {
    try expect(typeAndGetText("ddoong"), "\u{0111}\u{00F4}ng") // đông
}

test("coffee -> cofee live (non-contiguous vowels block ee -> ê)") {
    // f tones then un-tones (consuming one f); the final 'e' must NOT
    // become ê because "cofe" is not a single Vietnamese syllable.
    try expect(typeAndGetText("coffee"), "cofee")
}

// ============================================================
// Double-press escape is always trusted (n+1 typing style)
// ============================================================
print("\n--- Double-Press Escape ---")

test("noww + space -> 'now' (escape kept, never resurrected to raw)") {
    let (result, composed) = typeThenSpace("noww")
    try expect(composed, "now")
    try expect(result, EngineResult.wordBreak)
}

test("hass + space -> 'has'") {
    let (result, composed) = typeThenSpace("hass")
    try expect(composed, "has")
    try expect(result, EngineResult.wordBreak)
}

test("disst + space -> 'dist' (escape mid-word, then keep typing)") {
    let (result, composed) = typeThenSpace("disst")
    try expect(composed, "dist")
    try expect(result, EngineResult.wordBreak)
}

test("tesst + space -> 'test'") {
    let (result, composed) = typeThenSpace("tesst")
    try expect(composed, "test")
    try expect(result, EngineResult.wordBreak)
}

test("passs + space -> 'pass' (double letter needs triple press)") {
    let (result, composed) = typeThenSpace("passs")
    try expect(composed, "pass")
    try expect(result, EngineResult.wordBreak)
}

test("corrrection + space -> 'correction' (3-r style composes clean)") {
    let (result, composed) = typeThenSpace("corrrection")
    try expect(composed, "correction")
    try expect(result, EngineResult.wordBreak)
}

test("cofffee + space -> 'coffee'") {
    let (result, composed) = typeThenSpace("cofffee")
    try expect(composed, "coffee")
    try expect(result, EngineResult.wordBreak)
}

test("errror + space -> 'error'") {
    let (result, composed) = typeThenSpace("errror")
    try expect(composed, "error")
    try expect(result, EngineResult.wordBreak)
}

test("ddw -> đư (standalone w works after đ)") {
    try expect(typeAndGetText("ddw"), "\u{0111}\u{01B0}") // đư
}

test("ddwowngf -> đường") {
    try expect(typeAndGetText("ddwowngf"), "\u{0111}\u{01B0}\u{1EDD}ng") // đường
}

test("dduwowngf -> đường (long form still works)") {
    try expect(typeAndGetText("dduwowngf"), "\u{0111}\u{01B0}\u{1EDD}ng") // đường
}

test("ddwa -> đưa") {
    try expect(typeAndGetText("ddwa"), "\u{0111}\u{01B0}a") // đưa
}

test("swift stays swift (w after plain consonant is literal)") {
    try expect(typeAndGetText("swift"), "swift")
}

test("dd + space -> 'đ' kept (standalone đ is never unwound)") {
    let (result, composed) = typeThenSpace("dd")
    try expect(composed, "\u{0111}") // đ
    try expect(result, EngineResult.wordBreak)
}

test("ddc + space -> 'đc' kept (texting shorthand)") {
    let (result, composed) = typeThenSpace("ddc")
    try expect(composed, "\u{0111}c") // đc
    try expect(result, EngineResult.wordBreak)
}

test("correction (2 r's) + space -> 'corection' (known n-press limit, no restore)") {
    // Without an English dictionary the engine cannot tell a 2-press double
    // letter from a deliberate escape; the escape interpretation wins.
    let (result, composed) = typeThenSpace("correction")
    try expect(composed, "corection")
    try expect(result, EngineResult.wordBreak)
}

// ============================================================
// Quick Vietnamese (opt-in: w after an initial consonant -> ư)
// ============================================================
print("\n--- Quick Vietnamese ---")

let uHorn = "\u{01B0}" // ư

test("QV: single-consonant initials -> ư") {
    for initc in ["b", "c", "d", "g", "h", "l", "m", "n", "r", "s", "t", "v", "x"] {
        try expect(typeQuick("\(initc)w", quick: true), "\(initc)\(uHorn)")
    }
}

test("QV: digraph/trigraph initials -> ư") {
    for initc in ["ch", "kh", "ng", "nh", "ph", "th", "tr"] {
        try expect(typeQuick("\(initc)w", quick: true), "\(initc)\(uHorn)")
    }
}

test("QV: d-stroke initials (dw -> dư, ddw -> đư)") {
    try expect(typeQuick("dw", quick: true), "d\(uHorn)")
    try expect(typeQuick("ddw", quick: true), "\u{0111}\(uHorn)") // đư
}

test("QV: preserves uppercase initial (Tw -> Tư)") {
    try expect(typeQuick("Tw", quick: true), "T\(uHorn)")
}

test("QV: composes a full word (twowng -> tương)") {
    try expect(typeQuick("twowng", quick: true), "t\(uHorn)\u{01A1}ng") // tương
}

test("QV off by default leaves tw / chw literal") {
    try expect(typeQuick("tw", quick: false), "tw")
    try expect(typeQuick("chw", quick: false), "chw")
}

test("QV: only listed initials trigger (kw, pw stay literal)") {
    try expect(typeQuick("kw", quick: true), "kw")
    try expect(typeQuick("pw", quick: true), "pw")
}

test("QV: standalone w still becomes ư") {
    try expect(typeQuick("w", quick: true), uHorn)
}

test("QV: uw after vowel unchanged (tuw -> tư)") {
    try expect(typeQuick("tuw", quick: true), "t\(uHorn)")
    try expect(typeQuick("tuw", quick: false), "t\(uHorn)")
}

test("QV: double-w escapes conjured ư to literal (tww -> tw)") {
    try expect(typeQuick("tww", quick: true), "tw")
    try expect(typeQuick("chww", quick: true), "chw")
    try expect(typeQuick("sww", quick: true), "sw")
    try expect(typeQuick("ngww", quick: true), "ngw")
}

test("QV: standalone double-w still escapes (ww -> w) both modes") {
    try expect(typeQuick("ww", quick: true), "w")
    try expect(typeQuick("ww", quick: false), "w")
}

test("QV: real uw double-w still reverts the u (tuww -> tuw)") {
    try expect(typeQuick("tuww", quick: true), "tuw")
}

// "gi" counts as an initial: giw -> giư (giữ, giữa, giường)
test("QV: gi initial takes quick w (giw -> giư)") {
    try expect(typeQuick("giw", quick: true), "gi\(uHorn)")
    try expect(typeQuick("Giw", quick: true), "Gi\(uHorn)")
    try expect(typeQuick("giw", quick: false), "giw")
}

test("QV: gi quick w composes words") {
    try expect(typeQuick("giwx", quick: true), "gi\u{1EEF}")              // giữ
    try expect(typeQuick("giwax", quick: true), "gi\u{1EEF}a")            // giữa
    try expect(typeQuick("giwowngf", quick: true), "gi\u{01B0}\u{1EDD}ng") // giường
    try expect(typeQuick("giwj", quick: true), "gi\u{1EF1}")              // giự
}

test("QV: gi quick w double-w escapes (giww -> giw)") {
    try expect(typeQuick("giww", quick: true), "giw")
}

test("QV: toned gi does not take quick w (gisw != gíư)") {
    try expectTrue(typeQuick("gisw", quick: true) != "g\u{00ED}\(uHorn)")
}

// ============================================================
// "Huawei" fix: ưa + w reverts to literal "uaw" (both modes)
// ============================================================
print("\n--- Huawei / ưa+w revert ---")

test("uaww -> uaw (no ưă syllable) both modes") {
    try expect(typeQuick("uaww", quick: true), "uaw")
    try expect(typeQuick("uaww", quick: false), "uaw")
    try expect(typeQuick("huaww", quick: true), "huaw")
    try expect(typeQuick("huaww", quick: false), "huaw")
}

test("Huawei composes literally (QV on)") {
    try expect(typeQuick("Huawei", quick: true), "Huawei")
}

// ============================================================
// Real-time English-word guard (Quick Vietnamese only)
// ============================================================
print("\n--- English-word guard ---")

test("guard: huawei stays literal mid-word (QV on)") {
    try expect(typeQuick("huawei", quick: true), "huawei")
}

test("guard: other mixed words stay literal (QV on)") {
    try expect(typeQuick("await", quick: true), "await")
    try expect(typeQuick("sword", quick: true), "sword")
    try expect(typeQuick("nuance", quick: true), "nuance")
}

test("guard: default mode keeps legacy mid-word (huawei -> hưaei)") {
    try expect(typeQuick("huawei", quick: false), "h\(uHorn)aei") // hưaei
}

test("guard: genuine Vietnamese words unaffected (QV on)") {
    try expect(typeQuick("muaw", quick: true), "m\(uHorn)a")       // mưa
    try expect(typeQuick("huaws", quick: true), "h\u{1EE9}a")      // hứa
    try expect(typeQuick("thuwowng", quick: true), "th\(uHorn)\u{01A1}ng") // thương
    try expect(typeQuick("chuaw", quick: true), "ch\(uHorn)a")     // chưa
    try expect(typeQuick("xuaw", quick: true), "x\(uHorn)a")       // xưa
}

// ============================================================
// Deferred diacritics ("Bỏ dấu sau") -- opt-in sub-option of Quick Vietnamese
// ============================================================
print("\n--- Deferred diacritics ---")

// Deferred đ
test("deferred: did -> đi") {
    try expect(typeDeferred("did", deferred: true), "\u{0111}i")
}
test("deferred: Did -> Đi (uppercase)") {
    try expect(typeDeferred("Did", deferred: true), "\u{0110}i")
}
test("deferred: dend -> đen") {
    try expect(typeDeferred("dend", deferred: true), "\u{0111}en")
}
test("deferred: dad -> đa (valid shape, by design)") {
    try expect(typeDeferred("dad", deferred: true), "\u{0111}a")
}
test("deferred: did backspace count") {
    try expect(simulateDeferred("did", deferred: true), "\u{0111}i")
}

// Deferred circumflex
test("deferred: thana -> thân") {
    try expect(typeDeferred("thana", deferred: true), "th\u{00E2}n")
}
test("deferred: viene -> viên") {
    try expect(typeDeferred("viene", deferred: true), "vi\u{00EA}n")
}
test("deferred: nguyene -> nguyên") {
    try expect(typeDeferred("nguyene", deferred: true), "nguy\u{00EA}n")
}
test("deferred: thana backspace count") {
    try expect(simulateDeferred("thana", deferred: true), "th\u{00E2}n")
}

// Both forms + tone interplay
test("deferred: dongdo -> đông (đ then ô)") {
    try expect(typeDeferred("dongdo", deferred: true), "\u{0111}\u{00F4}ng")
}
test("deferred: dongod -> đông (ô then đ)") {
    try expect(typeDeferred("dongod", deferred: true), "\u{0111}\u{00F4}ng")
}
test("deferred: muonso -> muốn (tone then deferred vowel)") {
    try expect(typeDeferred("muonso", deferred: true), "mu\u{1ED1}n")
}
test("deferred: muonos -> muốn (deferred vowel then tone)") {
    try expect(typeDeferred("muonos", deferred: true), "mu\u{1ED1}n")
}
test("deferred: nguyenxe -> nguyễn (tone re-placement)") {
    try expect(typeDeferred("nguyenxe", deferred: true), "nguy\u{1EC5}n")
}

// Undo / escape (n+1 convention)
test("deferred: didd escapes to did") {
    try expect(typeDeferred("didd", deferred: true), "did")
}
test("deferred: thanaa escapes to thana") {
    try expect(typeDeferred("thanaa", deferred: true), "thana")
}
test("deferred: dataa escapes to data") {
    try expect(typeDeferred("dataa", deferred: true), "data")
}
test("deferred: photoo escapes to photo") {
    try expect(typeDeferred("photoo", deferred: true), "photo")
}

// English guard: invalid results revert to literal in real time
test("deferred guard: disabled stays literal") {
    try expect(simulateDeferred("disabled", deferred: true), "disabled")
}
test("deferred guard: banana stays literal") {
    try expect(simulateDeferred("banana", deferred: true), "banana")
}
test("deferred guard: cocoa stays literal") {
    try expect(simulateDeferred("cocoa", deferred: true), "cocoa")
}
test("deferred guard: dido stays literal (bare deferred đ then invalid)") {
    try expect(simulateDeferred("dido", deferred: true), "dido")
}
test("deferred guard: seven stays literal") {
    try expect(simulateDeferred("seven", deferred: true), "seven")
}
test("deferred guard: element stays literal") {
    try expect(simulateDeferred("element", deferred: true), "element")
}

// Never fires: open nuclei, wrong shapes
test("deferred: khoeo open nucleus untouched") {
    try expect(typeDeferred("khoeo", deferred: true), "khoeo")
}
test("deferred: xoong unchanged from default (adjacent oo wins)") {
    try expect(typeDeferred("xoong", deferred: true), typeDeferred("xoong", deferred: false))
    try expect(typeDeferred("xooong", deferred: true), "xoong")
}
test("deferred: add stays add (chars[0] not d for deferred)") {
    try expect(typeDeferred("add", deferred: true), "add")
}

// Word break
test("deferred: dend survives word break") {
    let (result, composed) = typeThenSpaceDeferred("dend")
    try expect(composed, "\u{0111}en")
    try expect(result, EngineResult.wordBreak)
}
test("deferred: adjacent dd survives word break") {
    let (result, composed) = typeThenSpaceDeferred("dd")
    try expect(composed, "\u{0111}")
    try expect(result, EngineResult.wordBreak)
}

// Documented limitation: valid-shaped English words transform
test("deferred: data -> dât (documented trade-off)") {
    try expect(typeDeferred("data", deferred: true), "d\u{00E2}t")
}

// Gating: inert without Quick Vietnamese
test("deferred: inert without Quick Vietnamese") {
    let e = TelexEngine()
    e.isVietnameseMode = true
    e.deferredDiacritics = true // quickVietnamese stays off
    for char in "did" { _ = e.processKey(keyCode: keyCodeFor(char), isShift: false) }
    try expect(e.buffer.text, "did")

    let e2 = TelexEngine()
    e2.isVietnameseMode = true
    e2.deferredDiacritics = true
    for char in "thana" { _ = e2.processKey(keyCode: keyCodeFor(char), isShift: false) }
    try expect(e2.buffer.text, "thana")
}

// Flag off: today's behavior unchanged
test("deferred off: regressions") {
    try expect(typeDeferred("did", deferred: false), "did")
    try expect(typeDeferred("thana", deferred: false), "thana")
    try expect(typeDeferred("dend", deferred: false), "dend")
    try expect(typeDeferred("data", deferred: false), "data")
    try expect(typeDeferred("dongdo", deferred: false), "dongdo")
}

// ============================================================
// Abbreviations (gõ tắt): table, word tracker, engine expansion, file
// ============================================================
print("\n--- Abbreviations ---")

func abbrevTable(_ entries: [(String, String)] = [
    ("btw", "by the way"), ("omw", "on my way"), ("đc", "được"), ("ko", "không"),
]) -> Abbreviations {
    var t = Abbreviations()
    for (k, e) in entries {
        if let err = t.insert(k, e) {
            fatalError("abbrevTable: could not insert \(k): \(err)")
        }
    }
    return t
}

/// Type letters into the engine; "\u{8}" is Backspace, uppercase is Shift.
func typeWord(_ engine: TelexEngine, _ s: String) {
    for ch in s {
        if ch == "\u{8}" {
            _ = engine.processKey(keyCode: KeyCode.delete.rawValue)
        } else {
            _ = engine.processKey(keyCode: keyCodeFor(ch.lowercased().first!), isShift: ch.isUppercase)
        }
    }
}

func expandAfter(_ s: String, quick: Bool = false) -> EngineResult? {
    let engine = TelexEngine()
    engine.isVietnameseMode = true
    engine.quickVietnamese = quick
    typeWord(engine, s)
    return engine.expandAbbreviation(abbrevTable())
}

// --- Table ---

test("Abbrev: keys need 2–32 letters") {
    try expect(Abbreviations.normalizeKey("btw"), "btw")
    try expect(Abbreviations.normalizeKey("  nz "), "nz")
    try expect(Abbreviations.normalizeKey("đc"), "đc")
    try expectNil(Abbreviations.normalizeKey("b"))
    try expectNil(Abbreviations.normalizeKey("b4"))
    try expectNil(Abbreviations.normalizeKey("b t"))
    try expectNil(Abbreviations.normalizeKey("a.b"))
    try expectNil(Abbreviations.normalizeKey(String(repeating: "a", count: 33)))
    try expectTrue(Abbreviations.normalizeKey(String(repeating: "a", count: 32)) != nil)
}

test("Abbrev: expansions are single-line and bounded") {
    try expect(Abbreviations.normalizeExpansion(" by the way "), "by the way")
    try expectNil(Abbreviations.normalizeExpansion(""))
    try expectNil(Abbreviations.normalizeExpansion("   "))
    try expectNil(Abbreviations.normalizeExpansion("a\tb"))
    try expectNil(Abbreviations.normalizeExpansion("a\nb"))
    try expectNil(Abbreviations.normalizeExpansion(String(repeating: "x", count: 501)))
    try expectTrue(Abbreviations.normalizeExpansion("không 👍") != nil)
}

test("Abbrev: insert replaces case-insensitively") {
    var t = abbrevTable([("btw", "by the way")])
    try expectNil(t.insert("BTW", "between"))
    try expect(t.count, 1)
    try expect(t.get("btw"), "between")
    try expectTrue(t.remove("Btw"))
    try expectTrue(t.isEmpty)
    try expectFalse(t.remove("btw"))
}

test("Abbrev: insert reports why it refused") {
    var t = Abbreviations()
    try expect(t.insert("b", "x"), .invalidKey)
    try expect(t.insert("btw", " "), .invalidExpansion)
}

test("Abbrev: table is bounded") {
    var t = Abbreviations()
    func key(_ i: Int) -> String {
        var s = ""; var n = i
        for _ in 0..<3 { s.append(Character(UnicodeScalar(UInt8(97 + n % 26)))); n /= 26 }
        return s
    }
    for i in 0..<Abbreviations.maxEntries { try expectNil(t.insert(key(i), "x")) }
    try expect(t.insert(key(Abbreviations.maxEntries), "x"), .full)
    // Replacing an existing entry is still allowed when full.
    try expectNil(t.insert(key(0), "y"))
}

test("Abbrev: case follows typing") {
    let t = abbrevTable([("btw", "by the way"), ("NZ", "New Zealand")])
    try expect(t.expand("btw"), "by the way")
    try expect(t.expand("Btw"), "By the way")
    try expect(t.expand("BTW"), "BY THE WAY")
    try expect(t.expand("bTw"), "by the way")
    // Typed exactly as defined: the expansion as defined.
    try expect(t.expand("NZ"), "New Zealand")
    try expect(t.expand("nz"), "New Zealand")
    try expect(t.expand("Nz"), "New Zealand")
    try expectNil(t.expand("xyz"))
}

test("Abbrev: capitalizes a Vietnamese first letter") {
    let t = abbrevTable([("dc", "được")])
    try expect(t.expand("Dc"), "Được")
    try expect(t.expand("DC"), "ĐƯỢC")
}

test("Abbrev: expandWord replaces the typed letters") {
    let t = abbrevTable([("btw", "by the way")])
    try expect(t.expandWord("btw"), .restore(backspaces: 3, text: "by the way"))
    try expectNil(t.expandWord("bt"))
}

// --- File format ---

test("Abbrev: file round-trips") {
    let t = abbrevTable([("btw", "by the way"), ("NZ", "New Zealand"), ("ko", "không")])
    try expect(Abbreviations.parse(t.serialize()), t)
}

test("Abbrev: parse skips comments and bad lines") {
    let text = "\u{FEFF}# comment\n\nbtw\tby the way\r\nb\tbad key\nnotab\nok\t  \nnz\tNew Zealand\n"
    let t = Abbreviations.parse(text)
    try expect(t.count, 2)
    try expect(t.get("btw"), "by the way")
    try expect(t.get("nz"), "New Zealand")
}

test("Abbrev: a second tab invalidates the line") {
    let t = Abbreviations.parse("ab\tone\ttwo\ncd\tfine\n")
    try expectNil(t.get("ab"))
    try expect(t.get("cd"), "fine")
}

test("Abbrev: store writes and reads the file") {
    let dir = FileManager.default.temporaryDirectory
        .appendingPathComponent("novakey-tests-\(UUID().uuidString)", isDirectory: true)
    defer { try? FileManager.default.removeItem(at: dir) }
    let url = dir.appendingPathComponent("abbreviations.txt")
    let t = abbrevTable()
    try expectTrue(AbbreviationStore.write(t, to: url))
    try expect(AbbreviationStore.read(from: url), t)
    // Missing file, or no file at all: an empty table, never a crash.
    try expect(AbbreviationStore.read(from: dir.appendingPathComponent("missing.txt")), Abbreviations())
    try expect(AbbreviationStore.read(from: nil), Abbreviations())
    try expectFalse(AbbreviationStore.write(t, to: nil))
}

// --- Word tracker ---

test("Tracker: collects a clean word") {
    var w = WordTracker()
    for c in "btw" { w.letter(c) }
    try expect(w.currentWord, "btw")
    w.boundary()
    try expectNil(w.currentWord)
    try expectTrue(w.isClean)
}

test("Tracker: rejects a glued word") {
    var w = WordTracker()
    w.interrupt() // e.g. a digit or '.'
    for c in "btw" { w.letter(c) }
    try expectNil(w.currentWord)
    try expectFalse(w.isClean)
}

test("Tracker: follows backspace") {
    var w = WordTracker()
    for c in "btx" { w.letter(c) }
    w.backspace()
    w.letter("w")
    try expect(w.currentWord, "btw")
    // Backspacing past the word's start loses track of it.
    for _ in 0..<4 { w.backspace() }
    w.letter("a")
    try expectNil(w.currentWord)
}

test("Tracker: steps back over a space") {
    var w = WordTracker()
    for c in "btw" { w.letter(c) }
    w.space()
    w.backspace()
    try expect(w.currentWord, "btw")
    // Only the one Space: a second backspace edits the word.
    w.backspace()
    try expect(w.currentWord, "bt")
}

test("Tracker: does not step back past other boundaries") {
    var w = WordTracker()
    for c in "btw" { w.letter(c) }
    w.boundary() // Enter, an arrow key, a click…
    w.backspace()
    try expectNil(w.currentWord)
    try expectFalse(w.isClean)

    // Two spaces: the first backspace removes the second space only.
    var w2 = WordTracker()
    for c in "btw" { w2.letter(c) }
    w2.space()
    w2.space()
    w2.backspace()
    try expectNil(w2.currentWord)
    try expectFalse(w2.isClean)
}

test("Tracker: returns to the boundary after deleting an expansion") {
    var w = WordTracker()
    w.expanded("by the way")
    // Glued to the expansion: cannot expand.
    for c in "btw" { w.letter(c) }
    try expectNil(w.currentWord)
    // Delete the three letters and "by the way" (10 characters).
    for _ in 0..<12 {
        w.backspace()
        try expectFalse(w.isClean)
    }
    w.backspace()
    try expectTrue(w.isClean)
    for c in "btw" { w.letter(c) }
    try expect(w.currentWord, "btw")
}

test("Tracker: gives up on overlong words") {
    var w = WordTracker()
    for _ in 0...Abbreviations.maxKeyLength { w.letter("a") }
    try expectNil(w.currentWord)
}

// --- Engine expansion (Vietnamese mode) ---

test("Engine abbrev: plain word expands") {
    try expect(expandAfter("btw"), .restore(backspaces: 3, text: "by the way"))
    try expect(expandAfter("ko"), .restore(backspaces: 2, text: "không"))
}

test("Engine abbrev: case follows typing") {
    try expect(expandAfter("Btw"), .restore(backspaces: 3, text: "By the way"))
    try expect(expandAfter("BTW"), .restore(backspaces: 3, text: "BY THE WAY"))
}

test("Engine abbrev: raw keys match when Telex changed the screen") {
    // "omw" composes to "ơm" (2 chars on screen) -- still the abbreviation.
    try expect(expandAfter("omw"), .restore(backspaces: 2, text: "on my way"))
}

test("Engine abbrev: composed screen text matches") {
    // "ddc" shows "đc", which is itself a key.
    try expect(expandAfter("ddc"), .restore(backspaces: 2, text: "được"))
}

test("Engine abbrev: non-abbreviations are left alone") {
    try expectNil(expandAfter("bt"))
    try expectNil(expandAfter("btwx"))
    try expectNil(expandAfter("vieetj"))
    try expectNil(expandAfter(""))
}

test("Engine abbrev: raw keys are ignored after a mid-word backspace") {
    // After a backspace into composed text the raw keys no longer describe
    // the screen; only the screen text can match. "omw<BS>w" shows "ơw".
    try expectNil(expandAfter("omw\u{8}w"))
    // "btx<BS>w" shows "btw" -> still expands.
    try expect(expandAfter("btx\u{8}w"), .restore(backspaces: 3, text: "by the way"))
}

test("Engine abbrev: raw keys survive fixing a typo before any composition") {
    // "omq<BS>" deletes the q the screen shows, so the keys stay exact and
    // "omw" (shown "ơm") still matches by its keys.
    try expect(expandAfter("omq\u{8}w"), .restore(backspaces: 2, text: "on my way"))
    try expect(expandAfter("o\u{8}omw"), .restore(backspaces: 2, text: "on my way"))
}

test("Engine abbrev: backspace over the space steps back into the word") {
    let engine = TelexEngine()
    engine.isVietnameseMode = true
    typeWord(engine, "omw")
    _ = engine.processKey(keyCode: KeyCode.space.rawValue)
    _ = engine.processKey(keyCode: KeyCode.delete.rawValue)
    try expect(engine.expandAbbreviation(abbrevTable()), .restore(backspaces: 2, text: "on my way"))
}

test("Engine abbrev: expansion forgets the word") {
    let engine = TelexEngine()
    engine.isVietnameseMode = true
    typeWord(engine, "btw")
    try expectTrue(engine.expandAbbreviation(abbrevTable()) != nil)
    try expectTrue(engine.buffer.isEmpty)
    // Backspace does not step back into the replaced word.
    try expect(engine.processKey(keyCode: KeyCode.delete.rawValue), .passThrough)
    try expectTrue(engine.buffer.isEmpty)
}

test("Engine abbrev: a miss leaves the engine untouched") {
    let engine = TelexEngine()
    engine.isVietnameseMode = true
    typeWord(engine, "vieet")
    try expectNil(engine.expandAbbreviation(abbrevTable()))
    try expect(engine.buffer.text, "viêt")
}

// --- The exact-raw backspace path must not change composition ---

test("Backspace keeps raw keys exact only while screen == keys") {
    // "tie" + BS + "eengs": the raw keys stay exact through the backspace,
    // and the rest of the word composes as if the typo never happened.
    let engine = TelexEngine()
    engine.isVietnameseMode = true
    typeWord(engine, "tie\u{8}eengs")
    try expect(engine.buffer.text, "ti\u{1EBF}ng") // tiếng
    // After a backspace into composed text the raw keys are abandoned, so an
    // invalid syllable at the word break is NOT restored to stale keys.
    let e2 = TelexEngine()
    e2.isVietnameseMode = true
    typeWord(e2, "tieeng\u{8}")
    try expect(e2.buffer.text, "ti\u{00EA}n")
    try expect(e2.processKey(keyCode: KeyCode.space.rawValue), .wordBreak)
}


// --- Coordinator: what the event tap does on every key and on Tab ---

/// A tiny model of the tap: feeds keys through the coordinator and the engine
/// exactly as EventTapManager does, applying results to a visible string.
final class TapSim {
    let engine = TelexEngine()
    let coord = AbbreviationCoordinator()
    let table: Abbreviations
    var visible: [Character] = []
    /// Whether the last Tab was swallowed (expanded) rather than passed on.
    var lastTabSwallowed = false

    init(vietnamese: Bool, table: Abbreviations = abbrevTable()) {
        self.table = table
        engine.isVietnameseMode = vietnamese
        coord.enabled = true
    }

    func key(_ code: UInt16, shift: Bool = false, modified: Bool = false) {
        if code == KeyCode.tab.rawValue, !modified, !shift,
           case .restore(let bs, let text)? = coord.expandOnTab(engine: engine, table: table) {
            visible.removeLast(min(bs, visible.count))
            visible.append(contentsOf: text)
            lastTabSwallowed = true
            return
        }
        lastTabSwallowed = false
        coord.track(keyCode: code, isShift: shift, modified: modified)
        let result = engine.processKey(keyCode: code, isShift: shift, hasCommandOrControl: modified)
        apply(result, code: code, shift: shift, modified: modified)
    }

    private func apply(_ result: EngineResult, code: UInt16, shift: Bool, modified: Bool) {
        let typed: Character? = {
            // A shortcut (⌘V, ⌃L…) types nothing by itself.
            if modified { return nil }
            if let k = KeyCode(rawValue: code), let l = k.asciiLetter {
                return shift ? Character(l.uppercased()) : l
            }
            switch code {
            case KeyCode.space.rawValue: return " "
            case KeyCode.tab.rawValue: return "\t"
            case KeyCode.returnKey.rawValue: return "\n"
            case KeyCode.period.rawValue: return "."
            case KeyCode.two.rawValue: return "2"
            default: return nil
            }
        }()
        switch result {
        case .passThrough, .wordBreak:
            if code == KeyCode.delete.rawValue {
                if !visible.isEmpty { visible.removeLast() }
            } else if let typed { visible.append(typed) }
        case .replace(let bs, let text):
            visible.removeLast(min(bs, visible.count))
            visible.append(contentsOf: text)
        case .restore(let bs, let text):
            visible.removeLast(min(bs, visible.count))
            visible.append(contentsOf: text)
            if let typed { visible.append(typed) }
        }
    }

    /// Type a string: letters (case = Shift), ' ' space, '\t' Tab, '\u{8}'
    /// Backspace, '.' period, '2' digit.
    func type(_ s: String) {
        for ch in s {
            switch ch {
            case " ": key(KeyCode.space.rawValue)
            case "\t": key(KeyCode.tab.rawValue)
            case "\u{8}": key(KeyCode.delete.rawValue)
            case ".": key(KeyCode.period.rawValue)
            case "2": key(KeyCode.two.rawValue)
            case "\n": key(KeyCode.returnKey.rawValue)
            default: key(keyCodeFor(ch.lowercased().first!), shift: ch.isUppercase)
            }
        }
    }

    var text: String { String(visible) }
}

test("Coordinator: Tab expands in English mode and is swallowed") {
    let sim = TapSim(vietnamese: false)
    sim.type("btw\t")
    try expect(sim.text, "by the way")
    try expectTrue(sim.lastTabSwallowed)
}

test("Coordinator: Tab expands in Vietnamese mode (screen and raw keys)") {
    let sim = TapSim(vietnamese: true)
    sim.type("omw\t")
    try expect(sim.text, "on my way")
    let sim2 = TapSim(vietnamese: true)
    sim2.type("ddc\t")
    try expect(sim2.text, "được")
}

test("Coordinator: a Tab that expands nothing passes through") {
    let sim = TapSim(vietnamese: false)
    sim.type("hello\t")
    try expect(sim.text, "hello\t")
    try expectFalse(sim.lastTabSwallowed)
}

test("Coordinator: only after a clean boundary (x.btw, 2btw stay)") {
    for vi in [false, true] {
        let a = TapSim(vietnamese: vi)
        a.type("x.btw\t")
        try expect(a.text, "x.btw\t")
        let b = TapSim(vietnamese: vi)
        b.type("2btw\t")
        try expect(b.text, "2btw\t")
        let c = TapSim(vietnamese: vi)
        c.type("hi btw\t")
        try expect(c.text, "hi by the way")
        let d = TapSim(vietnamese: vi)
        d.type("hi\nbtw\t")
        try expect(d.text, "hi\nby the way")
    }
}

test("Coordinator: disabled or in own window never expands") {
    let sim = TapSim(vietnamese: false)
    sim.coord.enabled = false
    sim.type("btw\t")
    try expect(sim.text, "btw\t")
    let own = TapSim(vietnamese: true)
    own.coord.suspended = true
    own.type("btw\t")
    try expect(own.text, "btw\t")
}

test("Coordinator: case follows typing through the tap") {
    let sim = TapSim(vietnamese: false)
    sim.type("Btw\t BTW\t")
    try expect(sim.text, "By the way BY THE WAY")
}

test("Coordinator: backspace over the space steps back into the word (E mode)") {
    let sim = TapSim(vietnamese: false)
    sim.type("btw \u{8}\t")
    try expect(sim.text, "by the way")
}

test("Coordinator: fixing a typo keeps the word known") {
    let e = TapSim(vietnamese: false)
    e.type("btx\u{8}w\t")
    try expect(e.text, "by the way")
    let v = TapSim(vietnamese: true)
    v.type("omq\u{8}w\t")
    try expect(v.text, "on my way")
}

test("Coordinator: deleting an expansion returns to the boundary") {
    let sim = TapSim(vietnamese: false)
    sim.type("ko\t")
    try expect(sim.text, "không")
    // Letters glued to the expansion do not expand (the Tab passes through
    // -- and, like any Tab, starts a fresh word, so test that separately).
    let glued = TapSim(vietnamese: false)
    glued.type("ko\tbtw\t")
    try expect(glued.text, "khôngbtw\t")
    // Delete the letters and the expansion: back at the boundary.
    sim.type("btw")
    try expect(sim.text, "khôngbtw")
    sim.type(String(repeating: "\u{8}", count: 8))
    try expect(sim.text, "")
    sim.type("btw\t")
    try expect(sim.text, "by the way")
}

test("Coordinator: a shortcut starts a fresh word, paste glues") {
    let sim = TapSim(vietnamese: false)
    sim.type("x")
    sim.key(KeyCode.l.rawValue, modified: true) // ⌃L: clean prompt
    sim.visible = []
    sim.type("btw\t")
    try expect(sim.text, "by the way")

    let paste = TapSim(vietnamese: false)
    paste.key(KeyCode.v.rawValue, modified: true) // ⌘V: unseen text before the caret
    paste.type("btw\t")
    try expect(paste.text, "btw\t")
}

test("Coordinator: reset (click / app switch) starts a clean word") {
    let sim = TapSim(vietnamese: false)
    sim.type("2x")
    sim.coord.reset()
    sim.engine.resetSession()
    sim.type("btw\t")
    try expect(sim.text, "2xby the way")
}

test("Coordinator: Shift-Tab and modified Tab are never expansion") {
    let sim = TapSim(vietnamese: false)
    sim.type("btw")
    sim.key(KeyCode.tab.rawValue, shift: true)
    try expectFalse(sim.lastTabSwallowed)
}

test("Coordinator: Vietnamese mode composes normally when no abbreviation") {
    let sim = TapSim(vietnamese: true)
    sim.type("tieengs vieetj ")
    try expect(sim.text, "tiếng việt ")
}


// ============================================================
// Toggle-shortcut Tests
// ============================================================
print("\n--- Toggle shortcut Tests ---")

test("hotkey: describes combinations in canonical order") {
    try expect(
        HotkeyManager.describe(keyCode: KeyCode.z.rawValue, modifiers: .maskAlternate),
        "Option+Z"
    )
    try expect(
        HotkeyManager.describe(
            keyCode: KeyCode.space.rawValue,
            modifiers: [.maskCommand, .maskShift, .maskControl, .maskAlternate]
        ),
        "Ctrl+Option+Shift+Cmd+Space"
    )
}

test("hotkey: labels letters, punctuation and function keys") {
    try expect(HotkeyManager.keyLabel(KeyCode.a.rawValue), "A")
    try expect(HotkeyManager.keyLabel(KeyCode.slash.rawValue), "/")
    try expect(HotkeyManager.keyLabel(0x7A), "F1")
    try expect(HotkeyManager.keyLabel(0x6F), "F12")
}

test("hotkey: symbols render as chips") {
    try expect(
        HotkeyManager.symbols(keyCode: KeyCode.z.rawValue, modifiers: .maskAlternate),
        ["⌥", "Z"]
    )
}

test("hotkey: rejects modifier-less ordinary keys") {
    // Bare keys would be swallowed by the tap in every application.
    try expectTrue(HotkeyManager.validate(keyCode: KeyCode.z.rawValue, modifiers: []).isReject)
    try expectTrue(HotkeyManager.validate(keyCode: KeyCode.space.rawValue, modifiers: []).isReject)
}

test("hotkey: allows modifier-less function keys") {
    try expect(HotkeyManager.validate(keyCode: 0x7A, modifiers: []), .ok)  // F1
}

test("hotkey: rejects Shift-only combinations") {
    try expectTrue(HotkeyManager.validate(keyCode: KeyCode.a.rawValue, modifiers: .maskShift).isReject)
    try expect(
        HotkeyManager.validate(keyCode: KeyCode.a.rawValue, modifiers: [.maskShift, .maskControl]),
        .ok
    )
}

test("hotkey: warns on system shortcuts but allows them") {
    let verdict = HotkeyManager.validate(keyCode: KeyCode.q.rawValue, modifiers: .maskCommand)
    try expectFalse(verdict.isReject)
    try expectFalse(verdict.message == nil)
}

test("hotkey: default binding is valid") {
    try expect(
        HotkeyManager.validate(
            keyCode: HotkeyManager.defaultKeyCode,
            modifiers: HotkeyManager.defaultModifiers
        ),
        .ok
    )
}

// MARK: Modifier-only shortcuts (⌃⇧ and friends)

test("hotkey: accepts two-modifier combinations") {
    try expect(HotkeyManager.validateModifierOnly([.maskControl, .maskShift]), .ok)
    try expect(HotkeyManager.validateModifierOnly([.maskCommand, .maskAlternate]), .ok)
}

test("hotkey: rejects single-modifier combinations") {
    // One modifier is held constantly while typing.
    try expectTrue(HotkeyManager.validateModifierOnly(.maskShift).isReject)
    try expectTrue(HotkeyManager.validateModifierOnly([]).isReject)
}

test("hotkey: describes modifier-only shortcuts by their modifiers") {
    try expect(
        HotkeyManager.describe(
            keyCode: KeyCode.z.rawValue,
            modifiers: [.maskControl, .maskShift],
            modifierOnly: true
        ),
        "Ctrl+Shift"
    )
    try expect(
        HotkeyManager.symbols(
            keyCode: KeyCode.z.rawValue,
            modifiers: [.maskControl, .maskShift],
            modifierOnly: true
        ),
        ["⌃", "⇧"]
    )
}

test("hotkey: watcher fires once on release") {
    var w = HotkeyManager.ComboWatcher()
    w.setCombo([.maskControl, .maskShift])
    try expectFalse(w.flagsChanged(to: .maskControl))
    try expectFalse(w.flagsChanged(to: [.maskControl, .maskShift]))
    try expectTrue(w.flagsChanged(to: .maskControl))
    try expectFalse(w.flagsChanged(to: []))
}

test("hotkey: watcher ignores combinations used as a shortcut") {
    // ⌃⇧S must stay a normal shortcut for the focused app.
    var w = HotkeyManager.ComboWatcher()
    w.setCombo([.maskControl, .maskShift])
    _ = w.flagsChanged(to: .maskControl)
    _ = w.flagsChanged(to: [.maskControl, .maskShift])
    w.keyPressed()
    try expectFalse(w.flagsChanged(to: .maskControl))
    try expectFalse(w.flagsChanged(to: []))
}

test("hotkey: watcher ignores partial and extra modifiers") {
    var w = HotkeyManager.ComboWatcher()
    w.setCombo([.maskControl, .maskShift])
    // Control alone.
    _ = w.flagsChanged(to: .maskControl)
    try expectFalse(w.flagsChanged(to: []))
    // A superset is not the combination.
    _ = w.flagsChanged(to: .maskControl)
    _ = w.flagsChanged(to: [.maskControl, .maskAlternate])
    _ = w.flagsChanged(to: [.maskControl, .maskAlternate, .maskShift])
    try expectFalse(w.flagsChanged(to: [.maskControl, .maskAlternate]))
    try expectFalse(w.flagsChanged(to: .maskControl))
    try expectFalse(w.flagsChanged(to: []))
}

test("hotkey: watcher re-arms between presses") {
    var w = HotkeyManager.ComboWatcher()
    w.setCombo([.maskControl, .maskShift])
    for _ in 0..<2 {
        _ = w.flagsChanged(to: .maskControl)
        _ = w.flagsChanged(to: [.maskControl, .maskShift])
        try expectTrue(w.flagsChanged(to: .maskControl))
        _ = w.flagsChanged(to: [])
    }
}

test("hotkey: watcher is inert without a combination") {
    var w = HotkeyManager.ComboWatcher()
    _ = w.flagsChanged(to: .maskControl)
    _ = w.flagsChanged(to: [.maskControl, .maskShift])
    try expectFalse(w.flagsChanged(to: .maskControl))
    try expectFalse(w.flagsChanged(to: []))
}

test("hotkey: keycode 0 (A) survives a round trip") {
    // Regression: `A` is virtual keycode 0x00, which a zero-means-unset getter
    // silently turned back into the default Z.
    let settings = AppSettings.shared
    let oldKey = settings.toggleHotkeyKeyCode
    let oldMods = settings.toggleHotkeyModifiers
    defer {
        settings.toggleHotkeyKeyCode = oldKey
        settings.toggleHotkeyModifiers = oldMods
    }

    settings.toggleHotkeyKeyCode = KeyCode.a.rawValue
    settings.toggleHotkeyModifiers = CGEventFlags.maskAlternate.rawValue
    try expect(settings.toggleHotkeyKeyCode, KeyCode.a.rawValue)
    try expect(HotkeyManager.currentSymbols, ["⌥", "A"])
}

test("hotkey: modifier-less stored value falls back to the default") {
    let settings = AppSettings.shared
    let oldKey = settings.toggleHotkeyKeyCode
    let oldMods = settings.toggleHotkeyModifiers
    defer {
        settings.toggleHotkeyKeyCode = oldKey
        settings.toggleHotkeyModifiers = oldMods
    }

    settings.toggleHotkeyModifiers = 0
    try expect(settings.toggleHotkeyModifiers, HotkeyManager.defaultModifiers.rawValue)
}

test("hotkey: reset restores the default binding") {
    let settings = AppSettings.shared
    let oldKey = settings.toggleHotkeyKeyCode
    let oldMods = settings.toggleHotkeyModifiers
    defer {
        settings.toggleHotkeyKeyCode = oldKey
        settings.toggleHotkeyModifiers = oldMods
    }

    settings.toggleHotkeyKeyCode = KeyCode.j.rawValue
    settings.toggleHotkeyModifiers = CGEventFlags.maskCommand.rawValue
    settings.resetToggleHotkey()
    try expect(settings.toggleHotkeyKeyCode, HotkeyManager.defaultKeyCode)
    try expect(settings.toggleHotkeyModifiers, HotkeyManager.defaultModifiers.rawValue)
}

} // end runAllTests()

func printSummary() {
    print("\n========================================")
    print("Results: \(passedTests)/\(totalTests) passed")
    if !failedTests.isEmpty {
        print("\nFailed tests:")
        for (name, reason) in failedTests {
            print("  - \(name): \(reason)")
        }
        print("========================================")
        exit(1)
    } else {
        print("All tests passed!")
        print("========================================")
        exit(0)
    }
}
