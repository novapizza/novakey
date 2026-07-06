# NovaKey — Windows port (Rust)

Standalone Windows build of the NovaKey Vietnamese Telex IME. The macOS Swift
app (`../Sources`, `../Tests`, `../Package.swift`) is **frozen and untouched** —
it serves only as the reference oracle for the engine's behavior.

## Layout

```
crossplatform/
├── core/      novakey-core — pure Telex engine, no OS deps (runs anywhere)
│   ├── src/{data,buffer,tone,spelling,engine}.rs, lib.rs
│   └── tests/parity.rs   — Rust port of ../Tests/run_tests.swift (the oracle)
└── windows/   novakey-win — the app: WH_KEYBOARD_LL hook + SendInput
    ├── build.rs             — embeds the app icon into the exe
    ├── assets/NovaKey.ico   — multi-size icon (16–256 px) from ../../Asset/NewLogo.png
    └── src/{main,hook,sender,vk,tray,settings}.rs
```

## App icon

`assets/NovaKey.ico` is generated from `../Asset/NewLogo.png` (the branded
NovaKey logo the macOS app also uses) as a 9-size icon (16/20/24/32/40/48/64/
128/256 px). `build.rs` embeds it via `winresource` as resource id 1, so it is
the Explorer file icon; `tray::app_icon()` loads the same resource for the tray
icon and window class. Regenerate after changing the logo:

```sh
python -c "from PIL import Image; Image.open(r'../Asset/NewLogo.png').convert('RGBA').save(r'windows/assets/NovaKey.ico', sizes=[(s,s) for s in (16,20,24,32,40,48,64,128,256)])"
```

`novakey-core` is a manual, line-for-line reimplementation of the Swift engine
in `../Sources/NovaKey/Engine/`. Every character it emits is a single
precomposed NFC BMP scalar, so the Windows sender can treat
"1 replaced char = 1 UTF-16 unit = 1 backspace".

## Build & test

```sh
# From this directory (crossplatform/):
cargo test                       # 103 tests: 93 engine parity + 10 Windows unit
cargo test -p novakey-core       # engine parity only (runs on macOS/CI too)
cargo build -p novakey-win --release   # -> target/release/novakey.exe (~184 KB)
```

Run `novakey.exe`: it lives in the system tray (no window). Left-click the
tray icon or press **Ctrl+Shift+Z** to toggle Vietnamese input on/off;
right-click for the menu (toggle, Start with Windows, compatibility mode, quit).
Settings persist to `%APPDATA%\NovaKey\settings.json`.

## How it works

- A hidden message-only window installs `WH_KEYBOARD_LL` + `WH_MOUSE_LL` and a
  `EVENT_SYSTEM_FOREGROUND` WinEvent hook on one thread with a `GetMessage` pump.
- Each keydown is classified by `vk.rs` into a neutral `KeyClass`, then fed to
  `TelexEngine::process_key`. The result maps to:
  - `PassThrough` / `WordBreak` → `CallNextHookEx` (let the key through).
  - `Replace` → suppress the key, batch `N × VK_BACK` then per-UTF-16-unit
    `KEYEVENTF_UNICODE` in one `SendInput`.
  - `Restore` → suppress the key, emit the restore text, then re-inject the
    original word-break key at the end of the batch (injected input queues
    *behind* the original on Windows, so we append it to preserve order).
- Self-injected events are tagged `dwExtraInfo = NOVAKEY_MAGIC` and skipped by
  the hook (equivalent of the macOS CGEventSource stateID). We do **not** skip
  all `LLKHF_INJECTED`.
- The session resets on mouse click and foreground change.

## Browser URL-bar autocomplete

Inline autocomplete in browser address bars keeps its suggested suffix
*selected*, so our first backspace deletes that selection instead of the real
character (`dd`→`dđ`, `hôm`→`hoô`). NovaKey defeats this by prepending a
U+202F (narrow no-break space) to the injected batch and sending one extra
backspace: the U+202F collapses the selection, then the backspaces (now N+1)
delete it back out along with the real characters. It's self-correcting —
harmless when no selection is present.

Rather than the macOS AX/UI-Automation probe (COM from the hook thread is
timeout-risky), this is scoped to **browsers only**, detected by caching the
foreground process name on each `EVENT_SYSTEM_FOREGROUND` (chrome/msedge/
firefox/brave/opera/vivaldi/chromium/arc/iexplore). Toggle via the tray menu
("Fix browser URL autocomplete", default on).

## Known limitations

- A non-elevated low-level hook is **inert while an elevated app has focus**.
  Run NovaKey elevated to type into elevated apps.

## Reserved

A `linux/` crate (IBus/Fcitx) is a future one-line `members` addition to the
workspace `Cargo.toml`; it is intentionally not created yet.
