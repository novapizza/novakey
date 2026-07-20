// NovaKey Test Runner
// Compile: swiftc -o /tmp/novakey_tests Sources/NovaKey/Engine/*.swift Tests/run_tests.swift -framework Carbon
// Run:     /tmp/novakey_tests

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
