// KeyCode.swift
// Virtual keycodes from Apple's Carbon HIToolbox/Events.h
// Source: /System/Library/Frameworks/Carbon.framework/Frameworks/HIToolbox.framework/Headers/Events.h

import Carbon.HIToolbox

/// macOS virtual keycodes for ANSI keyboard layout.
/// These represent physical key positions, not characters.
enum KeyCode: UInt16 {
    // MARK: - Letters (ANSI layout positions)
    case a              = 0x00
    case s              = 0x01
    case d              = 0x02
    case f              = 0x03
    case h              = 0x04
    case g              = 0x05
    case z              = 0x06
    case x              = 0x07
    case c              = 0x08
    case v              = 0x09
    case b              = 0x0B
    case q              = 0x0C
    case w              = 0x0D
    case e              = 0x0E
    case r              = 0x0F
    case y              = 0x10
    case t              = 0x11
    case o              = 0x1F
    case u              = 0x20
    case i              = 0x22
    case p              = 0x23
    case l              = 0x25
    case j              = 0x26
    case k              = 0x28
    case n              = 0x2D
    case m              = 0x2E

    // MARK: - Numbers
    case one            = 0x12
    case two            = 0x13
    case three          = 0x14
    case four           = 0x15
    case five           = 0x17
    case six            = 0x16
    case seven          = 0x1A
    case eight          = 0x1C
    case nine           = 0x19
    case zero           = 0x1D

    // MARK: - Punctuation & Symbols
    case equal          = 0x18
    case minus          = 0x1B
    case rightBracket   = 0x1E
    case leftBracket    = 0x21
    case quote          = 0x27
    case semicolon      = 0x29
    case backslash      = 0x2A
    case comma          = 0x2B
    case slash          = 0x2C
    case period         = 0x2F
    case grave          = 0x32

    // MARK: - Control Keys
    case returnKey      = 0x24
    case tab            = 0x30
    case space          = 0x31
    case delete         = 0x33  // Backspace
    case escape         = 0x35
    case forwardDelete  = 0x75

    // MARK: - Modifier Keys
    case command        = 0x37
    case shift          = 0x38
    case capsLock       = 0x39
    case option         = 0x3A
    case control        = 0x3B
    case rightCommand   = 0x36
    case rightShift     = 0x3C
    case rightOption    = 0x3D
    case rightControl   = 0x3E
    case function       = 0x3F

    // MARK: - Arrow Keys
    case leftArrow      = 0x7B
    case rightArrow     = 0x7C
    case downArrow      = 0x7D
    case upArrow        = 0x7E

    // MARK: - Navigation
    case home           = 0x73
    case pageUp         = 0x74
    case end            = 0x77
    case pageDown       = 0x79

    // MARK: - Keypad
    case keypadEnter    = 0x4C
    case keypadDecimal  = 0x41
    case keypadMultiply = 0x43
    case keypadPlus     = 0x45
    case keypadClear    = 0x47
    case keypadDivide   = 0x4B
    case keypadMinus    = 0x4E
    case keypadEquals   = 0x51
    case keypad0        = 0x52
    case keypad1        = 0x53
    case keypad2        = 0x54
    case keypad3        = 0x55
    case keypad4        = 0x56
    case keypad5        = 0x57
    case keypad6        = 0x58
    case keypad7        = 0x59
    case keypad8        = 0x5B
    case keypad9        = 0x5C
}

// MARK: - Key Classification

extension KeyCode {

    /// Whether this key is a letter (A-Z).
    var isLetter: Bool {
        switch self {
        case .a, .b, .c, .d, .e, .f, .g, .h, .i, .j,
             .k, .l, .m, .n, .o, .p, .q, .r, .s, .t,
             .u, .v, .w, .x, .y, .z:
            return true
        default:
            return false
        }
    }

    /// Whether this key breaks the current Vietnamese syllable session.
    /// When one of these is pressed, the engine resets its buffer.
    var isWordBreak: Bool {
        switch self {
        case .space, .returnKey, .tab, .escape,
             .leftArrow, .rightArrow, .upArrow, .downArrow,
             .home, .end, .pageUp, .pageDown,
             .comma, .period, .slash, .semicolon, .quote,
             .backslash, .minus, .equal, .grave,
             .leftBracket, .rightBracket,
             .keypadEnter, .forwardDelete:
            return true
        default:
            return false
        }
    }

    /// Whether this key leaves the caret at the start of a fresh word (Space,
    /// Enter, Tab, Escape, caret moves), as opposed to punctuation and forward
    /// delete, which leave the caret glued to the text before it. An
    /// abbreviation only expands when it was typed right after one of these.
    var startsWord: Bool {
        switch self {
        case .space, .returnKey, .keypadEnter, .tab, .escape,
             .leftArrow, .rightArrow, .upArrow, .downArrow,
             .home, .end, .pageUp, .pageDown:
            return true
        default:
            return false
        }
    }

    /// Whether this key is a modifier key.
    var isModifier: Bool {
        switch self {
        case .command, .shift, .capsLock, .option, .control,
             .rightCommand, .rightShift, .rightOption, .rightControl, .function:
            return true
        default:
            return false
        }
    }

    /// Convert keycode to the lowercase ASCII character it produces on ANSI layout.
    /// Returns nil for non-letter keys.
    var asciiLetter: Character? {
        switch self {
        case .a: return "a"
        case .b: return "b"
        case .c: return "c"
        case .d: return "d"
        case .e: return "e"
        case .f: return "f"
        case .g: return "g"
        case .h: return "h"
        case .i: return "i"
        case .j: return "j"
        case .k: return "k"
        case .l: return "l"
        case .m: return "m"
        case .n: return "n"
        case .o: return "o"
        case .p: return "p"
        case .q: return "q"
        case .r: return "r"
        case .s: return "s"
        case .t: return "t"
        case .u: return "u"
        case .v: return "v"
        case .w: return "w"
        case .x: return "x"
        case .y: return "y"
        case .z: return "z"
        default: return nil
        }
    }
}
