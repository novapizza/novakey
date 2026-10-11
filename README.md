# NovaKey

[Tiếng Việt](README.vi.md)

Vietnamese input method for macOS using the backspace technique.

Fast, lightweight (228KB), compatible with browsers, terminals, and all macOS apps.

## Features

- **Telex input method** with full support for tones (s/f/r/x/j/z) and modifiers (aa/ee/oo/aw/ow/uw/dd)
- **Backspace technique** -- uses CGEvent tap instead of IMKit, so it works in browser URL bars, Terminal, VS Code, Spotlight, and everywhere else
- **Smart tone placement** -- modern Vietnamese orthographic rules (e.g., `hoang` + `f` places tone on `a`, not `o`)
- **English-word protection** -- Telex transforms are gated on structural syllable validity, so `class`, `know`, and `add` stay literal instead of turning into Vietnamese
- **Double-press escape / n+1 typing** -- typing a Telex key twice trusts the second press unconditionally, so `disst` → `dist`, `noww` → `now`, `corrrection` → `correction`
- **Quick Vietnamese (opt-in)** -- a lone `w` after an initial consonant becomes `ư` (e.g. `tw` → `tư`), with real-time revert so mixed English words like `huawei` stay literal
- **Menu bar app** -- runs as a status bar icon (V/E), no dock icon
- **Browser autocomplete fix** -- probes the focused field's selection to compensate for inline URL-bar suggestions, avoiding backspace miscounts
- **Sleep/wake recovery** -- automatically restarts event tap after system sleep
- **Customizable toggle hotkey** -- defaults to `Option+Z`; rebindable in Settings
- **Fn (Globe) key switching** -- optionally follow the system language switch via the Fn/Globe key
- **Launch at login** and an optional **switch sound**

## Requirements

- macOS 14.0+ (Sonoma or later)
- Apple Silicon (arm64)
- **Input Monitoring** permission (System Settings > Privacy & Security > Input Monitoring)
- **Accessibility** permission (System Settings > Privacy & Security > Accessibility)

## Build

```bash
swift build -c release
```

### Package as .app bundle (macOS)

```bash
./build.sh
```

This assembles the app bundle, copies all resources, and signs it. With no
environment set it signs **ad-hoc** (local dev only — not distributable).

### Run

```bash
open build/NovaKey.app
```

On first launch, macOS will ask for permissions. Grant both:
1. **System Settings > Privacy & Security > Input Monitoring** -- enable NovaKey
2. **System Settings > Privacy & Security > Accessibility** -- enable NovaKey

The menu bar will show **V** (Vietnamese mode) or **E** (English mode).

## Usage

### Telex Input

| You type | Result | Rule |
|----------|--------|------|
| `as` | `a` | sắc tone |
| `af` | `a` | huyền tone |
| `ar` | `ả` | hỏi tone |
| `ax` | `ã` | ngã tone |
| `aj` | `ạ` | nặng tone |
| `az` | `a` | remove tone |
| `aa` | `â` | circumflex |
| `ee` | `ê` | circumflex |
| `oo` | `ô` | circumflex |
| `aw` | `ă` | breve |
| `ow` | `ơ` | horn |
| `uw` | `ư` | horn |
| `dd` | `đ` | d-stroke |

### Hotkey

| Shortcut | Action |
|----------|--------|
| `Option+Z` | Toggle Vietnamese/English mode (default -- rebindable in Settings) |

### Settings

Click the **V/E** icon in the menu bar > **Settings** to configure:

**Input Method**
- **Toggle hotkey** -- record your own shortcut for Vietnamese/English switching
- **Quick Vietnamese** -- off by default; a lone `w` after a consonant becomes `ư` (e.g. `tw` → `tư`), while mixed words like `huawei` stay literal

**General**
- **Launch at login** -- start NovaKey automatically
- **Play sound on switch** -- audible cue when toggling modes
- **Switch with Fn (Globe) key** -- on by default; follow the system language switch

**Compatibility**
- **Fix browser autocomplete** -- on by default, helps with Chrome/Safari URL bars
- **Send keys step-by-step** -- off by default, enable if you see garbled output in specific apps

## Architecture

```
Keyboard → CGEventTap (intercept) → TelexEngine (process) → KeySender (backspace + replace) → App
```

### Project Structure

```
Sources/NovaKey/
├── App/                    # App entry point, delegate, logging
├── Engine/                 # Pure Swift Telex engine (no UI/system deps)
│   ├── TelexEngine.swift   # Core state machine
│   ├── SyllableBuffer.swift# Current syllable tracking
│   ├── TonePlacement.swift # Smart tone mark placement
│   ├── VietnameseData.swift# Unicode tables, Telex mappings
│   ├── SpellingChecker.swift# Syllable validation
│   └── KeyCode.swift       # macOS virtual keycodes
├── EventTap/               # CGEvent tap + synthetic key sending
│   ├── EventTapManager.swift   # Tap lifecycle, event callback
│   ├── KeySender.swift         # Backspace + Unicode sending
│   └── EventSourceManager.swift# Self-event detection
├── UI/                     # Menu bar icon + SwiftUI settings
├── Settings/               # UserDefaults persistence, hotkey
└── Permissions/            # Input Monitoring + Accessibility checks
```

### How the Backspace Technique Works

1. CGEvent tap intercepts every keystroke globally
2. TelexEngine processes the key through its state machine
3. If a Telex transformation applies (e.g., `s` after `a` → `a`):
   - The original keystroke is **suppressed** (callback returns nil)
   - KeySender sends **N backspaces** to delete old characters
   - KeySender sends the **replacement Vietnamese text** via `CGEventKeyboardSetUnicodeString`
4. Self-event detection (via `CGEventSource.sourceStateID`) prevents infinite loops

### Why Not IMKit?

Apple's Input Method Kit uses a "composition window" (marked text) to show in-progress input. Many apps don't support this properly:
- Browser URL bars ignore marked text
- Terminal emulators handle it inconsistently
- Electron apps often break

The backspace technique bypasses all of this by working at the keystroke level.

## Tests

```bash
# Compile and run engine tests (no Xcode required)
swiftc -o /tmp/novakey_tests \
  Sources/NovaKey/Engine/*.swift \
  Tests/run_tests.swift \
  -framework Carbon \
  -parse-as-library && /tmp/novakey_tests
```

87 tests covering:
- All tone marks (sắc, huyền, hỏi, ngã, nặng, remove)
- All vowel modifiers (circumflex, breve, horn)
- D-stroke, combined sequences
- Smart tone placement rules
- Syllable buffer operations
- Word break and modifier key handling
- English-word protection and spelling restore
- Double-press escape / n+1 typing

## Debug

Logs are written to `/tmp/novakey.log`:

```bash
tail -f /tmp/novakey.log
```

## License

MIT — see [LICENSE](LICENSE).
