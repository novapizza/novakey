# NovaKey Windows Port (Rust) — macOS Left Untouched

## Context

NovaKey is a working macOS Vietnamese Telex IME (Swift, backspace technique via CGEventTap). The goal is to add a **standalone Windows version**. Decision (revised): **the macOS app is frozen — no changes to it at all.** No shared-with-Swift core, no FFI, no C ABI. The macOS Swift engine stays exactly as-is and serves only as the *reference oracle* for validating the Windows port's behavior.

**Why the engine ports cleanly:** `Sources/NovaKey/Engine/` (TelexEngine, SyllableBuffer, VietnameseData, TonePlacement, SpellingChecker — ~1,200 lines) is pure logic, Foundation-only, with a clean numeric boundary: `processKey(...) -> EngineResult(.passThrough | .replace | .wordBreak | .restore)`. Everything macOS-specific lives in EventTap/, KeySender, UI/, Permissions/, Settings — none of which the Windows app reuses.

**Decisions made with the user:**
- Language: **Rust** (memory safety in the system-wide keyboard hook, native Unicode, official `windows-rs` bindings, built-in tests, ~1 MB no-runtime-DLL exe). C++ rejected — the safety stakes of a global hook favor Rust, and the shared-C-ABI reason for possibly preferring otherwise is moot now that macOS is untouched.
- Injection: **low-level keyboard hook (`WH_KEYBOARD_LL`) + `SendInput` with `KEYEVENTF_UNICODE`** — the same backspace technique as macOS (UniKey/EVKey approach). Not TSF.
- Rules/tone/spelling: **reproduced exactly** in Rust, proven via parity vectors generated from the current Swift engine.

## Scope note

The Rust engine is a **manual line-for-line reimplementation** of the Swift engine (Swift can't compile on Windows here), not literal reuse. Behavior is kept identical and *proven*, not assumed.

## Target layout

The Windows work is self-contained; the existing macOS tree is not moved or modified. Rust lives in a workspace under `crossplatform/`, split into a **platform-agnostic engine crate** (`novakey-core`, no `windows` dependency — its tests run on the dev Mac / CI / anywhere) and a **Windows bin crate** (`novakey-win`, depends on core + `windows-rs`). A `linux/` crate is a reserved slot — NOT created now (empty crate = clutter); adding it later is a one-line `members` change plus the IBus/Fcitx shell work.

```
NovaKey/
├── Package.swift, Sources/NovaKey/...  # existing macOS Swift app — UNCHANGED
├── Tests/run_tests.swift               # existing — UNCHANGED (used once to emit vectors)
├── testdata/telex_vectors.tsv          # NEW: parity vectors generated FROM the Swift engine (neutral, shared, in root)
└── crossplatform/
    ├── Cargo.toml                      # workspace: members = ["core", "windows"]   (add "linux" later)
    ├── core/                           # novakey-core: pure engine, NO OS deps
    │   ├── Cargo.toml
    │   ├── src/
    │   │   ├── lib.rs                  # public API: Engine, EngineResult, KeyClass
    │   │   ├── engine.rs               # <- TelexEngine.swift (state machine)
    │   │   ├── buffer.rs               # <- SyllableBuffer.swift
    │   │   ├── data.rs                 # <- VietnameseData.swift (ViChar, Unicode tables)
    │   │   ├── tone.rs                 # <- TonePlacement.swift
    │   │   └── spelling.rs             # <- SpellingChecker.swift
    │   └── tests/                      # ported unit tests + vector runner — `cargo test -p novakey-core` on macOS
    └── windows/                        # novakey-win: bin (Windows-only build)
        ├── Cargo.toml                  # depends on novakey-core + windows-rs
        └── src/
            ├── main.rs                 # message-only window + hook install + message loop
            ├── hook.rs                 # WH_KEYBOARD_LL callback logic
            ├── sender.rs               # SendInput batching (backspaces + KEYEVENTF_UNICODE)
            ├── vk.rs                   # VK code -> neutral KeyClass
            ├── tray.rs                 # Shell_NotifyIcon + menu
            └── settings.rs             # %APPDATA% JSON + HKCU Run autostart
```

No C ABI / no `staticlib` — the Windows bin links `novakey-core` as a normal Rust dependency. `KeyCode.swift` is not ported; `vk.rs` does the equivalent classification (VK_A..Z are ASCII already) producing a neutral `KeyClass` (Letter(a–z) | Backspace | WordBreak | Other) before the engine sees it. Because `novakey-core` has no `windows` dependency, the full parity suite runs on the macOS dev machine.

## Phases

**Phase 0 — Parity vectors from the Swift engine (½–1 day).** Write a one-off Swift generator (variant of run_tests.swift) that replays every test sequence through the *current, unmodified* Swift engine and dumps `testdata/telex_vectors.tsv`: input keys → composed text → break action (150–300 rows incl. English traps: class, add, disst, cocs, know…). For the ~10 trickiest sequences (tone-move `hos`+`a`→`hoá`, undo, `ww`, restore) also record per-key result traces (action + backspace count + text) — final-text equality alone doesn't prove backspace counts. This is a read-only use of the Swift app; the app itself isn't changed.

**Phase 1 — Rust engine + parity proof (5–8 days), in `crossplatform/core`.** Port in dependency order: data.rs → buffer.rs → tone.rs → spelling.rs → engine.rs. Hand-port run_tests.swift cases to `#[test]`s. Add a vector runner that replays the TSV and asserts composed text + break action + traced backspace counts. Exit: `cargo test -p novakey-core` green (runs on the macOS dev machine), full vector parity.

**Phase 2 — Windows app shell (7–10 days).** Rust bin, `windows` crate, `#![windows_subsystem = "windows"]`.
- Hidden message-only window + `SetWindowsHookExW(WH_KEYBOARD_LL)` on one thread with a `GetMessage` pump; engine in `thread_local!`/`RefCell`.
- Self-event detection: tag all `SendInput` with `dwExtraInfo = NOVAKEY_MAGIC`; skip those in the hook (equivalent of macOS CGEventSource stateID). Do NOT skip all `LLKHF_INJECTED`.
- On keydown: modifiers via `GetKeyState`, VK→KeyClass via vk.rs, `engine.process_key`. Map results:
  - PassThrough/WordBreak → `CallNextHookEx`.
  - Replace → return 1 (suppress) + one batched `SendInput` [N× VK_BACK down/up, then per-UTF-16-unit KEYEVENTF_UNICODE]. Output is BMP single-scalar, so 1 char = 1 UTF-16 unit = 1 backspace — assert in engine.
  - Restore → injected input queues behind the original (unlike the macOS tap proxy), so **suppress the original key too and append it to the SendInput batch** to preserve order.
- Reset session on mouse click (`WH_MOUSE_LL`) and foreground change (`SetWinEventHook(EVENT_SYSTEM_FOREGROUND)`).
- Callback must never block — 300 ms `LowLevelHooksTimeout` or Windows silently drops the hook; async logging only + watchdog re-install.
- Tray (`Shell_NotifyIcon`): Toggle, Start with Windows (HKCU Run), Quit; toggle hotkey via `RegisterHotKey`. Settings JSON at `%APPDATA%\NovaKey\settings.json`.
- Release profile `opt-level="z", lto, codegen-units=1, panic="abort", strip` → ~1 MB exe.
- **Known limitation (document):** a non-elevated LL hook is inert while an elevated app has focus.
- **Deferred:** browser-autocomplete compensation (macOS uses AX; Windows needs UI Automation — COM from the hook thread is timeout-risky). Ship without it; cheap U+202F fallback first if URL-bar bugs appear.

**Phase 3 — Polish (3–5 days).** Installer (zip first, `cargo-wix` MSI later), settings UI, step-by-step-send compatibility toggle, autocomplete fix if needed.

## Main risks

1. **Swift vs Rust string semantics** — Swift `.count` = graphemes, Rust `.len()` = bytes; every count in `buildReplacement`/`restoreIfInvalid` must be `chars().count()`. Per-key trace vectors catch this.
2. **Backspace units** — safe only because all output is single-scalar NFC BMP; debug-assert the engine never emits combining marks.
3. **Case handling** — explicit precomposed uppercase table; ASCII-only ops for input.
4. **Windows app compat** — KEYEVENTF_UNICODE quirks in Java apps/old consoles; test matrix: Notepad, Word, Chrome/Edge (page + URL bar), Windows Terminal, VS Code, IntelliJ, Discord, RDP.

## Verification

- Phase 1: `cargo test` (ported unit tests + vector runner + per-key trace diff against the Swift oracle output).
- Phase 2: manual Windows matrix above; fast-typing stress (backspace ordering); hook-timeout survival under load; sleep/resume; elevated-app behavior matches docs; toggle hotkey; settings persistence; autostart. Unit tests on vk.rs mapping + a mock sender capturing the INPUT array per EngineResult.

## Critical reference files (read-only; macOS side never modified)

- [TelexEngine.swift](Sources/NovaKey/Engine/TelexEngine.swift) — state machine to reproduce; `buildReplacement` backspace math
- [VietnameseData.swift](Sources/NovaKey/Engine/VietnameseData.swift) — ViChar model + Unicode tables → data.rs
- [TonePlacement.swift](Sources/NovaKey/Engine/TonePlacement.swift), [SpellingChecker.swift](Sources/NovaKey/Engine/SpellingChecker.swift) → tone.rs / spelling.rs
- [KeyCode.swift](Sources/NovaKey/Engine/KeyCode.swift) — classification logic mirrored by vk.rs
- [run_tests.swift](Tests/run_tests.swift) — 87 cases; source of the vector oracle
- [EventTapManager.swift](Sources/NovaKey/EventTap/EventTapManager.swift), [KeySender.swift](Sources/NovaKey/EventTap/KeySender.swift) — result-consumption + backspace/injection contract the Windows hook reproduces
