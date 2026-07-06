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
- **Menu bar app** -- runs as a status bar icon (V/E), no dock icon
- **Browser autocomplete fix** -- probes the focused field's selection to compensate for inline URL-bar suggestions, avoiding backspace miscounts
- **Sleep/wake recovery** -- automatically restarts event tap after system sleep
- **Option+Z** to toggle Vietnamese/English mode

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

**Release (Developer ID + notarization):**

```bash
# Sign with a real Developer ID Application identity
SIGN_IDENTITY="Developer ID Application: Your Name (TEAMID)" ./build.sh

# Sign + notarize + staple (pick one credential set)
SIGN_IDENTITY="Developer ID Application: Your Name (TEAMID)" \
  NOTARIZE=1 NOTARY_PROFILE=novakey ./build.sh
```

Notarization credentials (choose one):

| Method | Env vars |
|--------|----------|
| Keychain profile | `NOTARY_PROFILE` (created via `xcrun notarytool store-credentials`) |
| App Store Connect API key | `APPLE_API_KEY` (.p8 path), `APPLE_API_KEY_ID`, `APPLE_API_ISSUER` |
| Apple ID | `APPLE_ID`, `APPLE_TEAM_ID`, `APPLE_APP_PASSWORD` (app-specific) |

Signing is inside-out with hardened runtime (`--options runtime --timestamp`);
the notary ticket is stapled into the bundle so it validates offline.

### Package + sign (Windows)

Windows artifacts are Authenticode-signed with **Azure Trusted Signing** via
`signtool` — no certificate is stored locally.

```powershell
# Service-principal auth for the Trusted Signing account
$env:AZURE_TENANT_ID='...'; $env:AZURE_CLIENT_ID='...'; $env:AZURE_CLIENT_SECRET='...'

./scripts/sign-windows.ps1 -Path build/NovaKey.exe
```

Prerequisites: Windows SDK (`signtool.exe`), the Trusted Signing client
(`dotnet tool install --global Microsoft.Trusted.Signing.Client`), and account
details filled into [scripts/trusted-signing-metadata.json](scripts/trusted-signing-metadata.json).
The script auto-locates `signtool` and the signing dlib, or honor `SIGNTOOL` /
`TRUSTED_SIGNING_DLIB` env overrides.

### Release (CI)

Pushing a `v*.*.*` tag triggers [.github/workflows/release.yml](.github/workflows/release.yml),
which builds, signs, and attaches macOS + Windows artifacts to a GitHub Release.

Required repository secrets:

| Platform | Secret | Purpose |
|----------|--------|---------|
| macOS | `CSC_LINK` | Developer ID Application cert (.p12), base64 |
| macOS | `CSC_KEY_PASSWORD` | .p12 export password |
| macOS | `APPLE_ID` | Apple ID for notarization |
| macOS | `APPLE_APP_SPECIFIC_PASSWORD` | app-specific password for the Apple ID |
| macOS | `APPLE_TEAM_ID` | Apple Developer team ID |
| Windows | `AZURE_TENANT_ID` / `AZURE_CLIENT_ID` / `AZURE_CLIENT_SECRET` | Trusted Signing service principal |
| Windows | `AZURE_TRUSTED_SIGNING_ENDPOINT` | e.g. `https://eus.codesigning.azure.net/` |
| Windows | `AZURE_CODE_SIGNING_ACCOUNT_NAME` | Trusted Signing account name |
| Windows | `AZURE_CERT_PROFILE_NAME` | certificate profile name |
| Windows | `AZURE_PUBLISHER_NAME` | MSIX manifest publisher CN (only if packaging `.msix`) |
| R2 | `R2_RELEASES_ACCESS_KEY_ID` / `R2_RELEASES_SECRET_ACCESS_KEY` | R2 S3 API credentials |
| R2 | `R2_RELEASES_ACCOUNT_ID` | Cloudflare account ID (S3 endpoint host) |
| R2 | `R2_RELEASES_BUCKET` | target R2 bucket |

The Developer ID signing identity is auto-detected from the imported `CSC_LINK`
certificate — no separate identity secret is needed.

Signed artifacts are also mirrored to Cloudflare R2 at
`novakey/<tag>/` (immutable) and `novakey/latest/` (rolling) via the S3 API.

The Windows job builds the Rust port in [crossplatform/](crossplatform/)
(`cargo build -p novakey-win --release`), runs the engine parity suite, then
signs the resulting `novakey.exe`. The macOS Swift app is unchanged.

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
| `Option+Z` | Toggle Vietnamese/English mode |

### Settings

Click the **V/E** icon in the menu bar > **Settings** to configure:
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
