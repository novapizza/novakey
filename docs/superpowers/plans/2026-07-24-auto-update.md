# Auto-Update (Windows + macOS) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give both NovaKey apps a "Check for Updates…" action plus a silent daily background check that upgrades the app in place from Cloudflare-R2-hosted manifests.

**Architecture:** The existing release pipeline already ships every tagged build to Cloudflare R2 under `/{TAG}/` and rolling `/latest/`. We add two feed files to that `/latest/` upload: a Sparkle **appcast.xml** (macOS) and a flat **latest.json** (Windows). macOS uses the Sparkle framework (industry-standard: EdDSA-signed appcast, self-replace, scheduled checks). Windows gets a small hand-rolled updater (WinHTTP download → SHA-256 + Authenticode verify → rename-swap the exe → relaunch), matching the project's zero-serde / minimal-crate ethos.

**Tech Stack:** Swift 5.9 / SPM + Sparkle 2.x (macOS); Rust MSVC + `windows` crate WinHTTP/BCrypt/WinTrust (Windows); GitHub Actions + AWS CLI to R2 (CI).

## Global Constraints

- **R2 public host:** all feed/asset URLs derive from one base, `R2_PUBLIC_BASE` — the public HTTPS origin of the releases bucket (r2.dev subdomain or custom domain), no trailing slash. Copied verbatim into exactly two places: macOS `SUFeedURL` and Windows `updater::FEED_URL`. Resolved in Task 1; must be a real reachable HTTPS origin before Tasks 8/13 can be verified.
- **Feed paths (fixed):** `${R2_PUBLIC_BASE}/latest/appcast.xml` (macOS), `${R2_PUBLIC_BASE}/latest/latest.json` (Windows).
- **macOS:** target macOS 14+, Swift tools 5.9, hardened runtime + Developer ID + notarized. App is **not** sandboxed.
- **Windows:** Rust edition 2021, MSVC target `x86_64-pc-windows-msvc`. **No serde**; extend the existing hand-rolled JSON reader in `settings.rs`. Prefer `windows`-crate system APIs over new crates. Preserve the single-instance mutex `Local\NovaKeySingleInstanceMutex`.
- **Version source of truth:** macOS `CFBundleShortVersionString` (Info.plist) and Rust `CARGO_PKG_VERSION` (Cargo.toml). Both currently `0.2.5`. Never hardcode a version string in updater code — read it from these.
- **Security is non-negotiable:** downloaded artifacts MUST pass signature verification (Sparkle EdDSA on macOS; SHA-256 match **and** Authenticode `WinVerifyTrust` on Windows) before being launched. A failed check aborts the update silently — never run an unverified binary.
- **Pre-commit:** per repo `CLAUDE.md`, run the `/security-review` skill on the pending diff before every commit and address findings. This is doubly important here — updater code is a remote-code-execution surface.

---

## File Structure

**Windows (Rust) — `crossplatform/windows/`**
- Create `src/updater.rs` — manifest fetch/parse, semver compare, download, SHA-256 (BCrypt), Authenticode verify (WinTrust), rename-swap + relaunch, orchestrator. Owns all update logic.
- Modify `src/main.rs` — `mod updater;`, `--finish-update` startup path (mutex retry + delete old exe), `CMD_UPDATE` dispatch, background check thread on launch.
- Modify `src/tray.rs` — add `CMD_UPDATE` id + "Check for Updates…" menu item.
- Modify `src/settings.rs` — add `last_update_check: u64` field + `read_u64` helper + `read_str` helper (reused by updater's JSON parse).
- Modify `Cargo.toml` — add `windows` features: `Win32_Networking_WinHttp`, `Win32_Security_Cryptography`, `Win32_Security_WinTrust`, `Win32_System_Com`.

**macOS (Swift) — `Sources/NovaKey/`, `Resources/`, root**
- Create `Sources/NovaKey/Updates/UpdaterController.swift` — thin wrapper over `SPUStandardUpdaterController`.
- Modify `Package.swift` — add Sparkle SPM dependency + link into target.
- Modify `Resources/Info.plist` — `SUFeedURL`, `SUPublicEDKey`, `SUEnableAutomaticChecks`, `SUScheduledCheckInterval`.
- Modify `Sources/NovaKey/UI/StatusBarController.swift` — "Check for Updates…" menu row + `onCheckForUpdates` callback.
- Modify `Sources/NovaKey/App/AppDelegate.swift` — own `UpdaterController`, wire the callback.
- Modify `build.sh` — embed `Sparkle.framework` (+ XPCServices) into the bundle and code-sign inside-out.

**CI — `.github/workflows/`**
- Modify `release.yml` — macOS job: generate EdDSA-signed `appcast.xml`. release job: compute Windows zip SHA-256, emit `latest.json`, upload both feeds to R2 `/latest/` and `/{TAG}/`.

---

## Task 1: Resolve config + generate signing keys (setup, no code)

**Files:** none (produces values + secrets consumed by later tasks).

**Interfaces:**
- Produces: `R2_PUBLIC_BASE` (the literal HTTPS origin string used in Tasks 5 & 9); GitHub Actions secret `SPARKLE_ED_PRIVATE_KEY`; the Sparkle EdDSA **public** key string used in Task 6.

- [ ] **Step 1: Confirm R2 public access + record the base URL**

Confirm the releases bucket (`secrets.R2_RELEASES_BUCKET`, used in [release.yml](.github/workflows/release.yml)) is reachable over public HTTPS. Either enable the managed `r2.dev` subdomain or attach a custom domain in the Cloudflare dashboard. Verify an existing asset resolves:

```bash
curl -I "${R2_PUBLIC_BASE}/latest/NovaKey-windows.zip"
```

Expected: `HTTP/2 200`. Record the exact origin (no trailing slash) as `R2_PUBLIC_BASE` — it is pasted verbatim in Task 5 (`updater::FEED_URL`) and Task 6 (`SUFeedURL`).

- [ ] **Step 2: Generate the Sparkle EdDSA key pair**

Sparkle ships `generate_keys`. Fetch the Sparkle 2.x release tarball once locally (macOS), then:

```bash
./bin/generate_keys
```

It prints the **public** key (a base64 string) and stores the private key in the login keychain. Export the private key for CI:

```bash
./bin/generate_keys -x sparkle_private_key.pem
```

- [ ] **Step 3: Store secrets + record the public key**

Add repo secret `SPARKLE_ED_PRIVATE_KEY` = contents of `sparkle_private_key.pem`. Delete the local `.pem`. Record the printed **public** key string — it goes into Info.plist `SUPublicEDKey` in Task 6. No commit in this task.

---

## Task 2: Windows — JSON string reader + semver compare (pure, TDD)

**Files:**
- Create: `crossplatform/windows/src/updater.rs`
- Modify: `crossplatform/windows/src/settings.rs` (add `read_str`)
- Modify: `crossplatform/windows/src/main.rs:11-17` (add `mod updater;`)

**Interfaces:**
- Consumes: existing `read_bool`/`read_u32` pattern in `settings.rs`.
- Produces:
  - `settings::read_str(text: &str, key: &str) -> Option<String>`
  - `updater::RemoteVersion { version: String, url: String, sha256: String, notes: String }`
  - `updater::parse_manifest(text: &str) -> Option<RemoteVersion>`
  - `updater::is_newer(current: &str, remote: &str) -> bool`

- [ ] **Step 1: Write the failing tests**

Create `crossplatform/windows/src/updater.rs` with only the test module + stub signatures:

```rust
//! updater.rs
//! Self-update: fetch the R2 manifest, verify, swap the exe, relaunch.

use crate::settings;

/// Fixed manifest URL. R2_PUBLIC_BASE is filled in Task 5.
pub const FEED_URL: &str = "PLACEHOLDER_SET_IN_TASK_5";

/// This build's version, from Cargo. Never hardcode.
pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone, Debug, PartialEq)]
pub struct RemoteVersion {
    pub version: String,
    pub url: String,
    pub sha256: String,
    pub notes: String,
}

/// Parse `{ "version", "windows": { "url", "sha256" }, "notes" }`.
pub fn parse_manifest(text: &str) -> Option<RemoteVersion> {
    unimplemented!()
}

/// Parse "x.y.z" (ignoring a leading 'v') into a comparable tuple.
fn parse_semver(v: &str) -> Option<(u32, u32, u32)> {
    unimplemented!()
}

/// True when `remote` is strictly newer than `current`.
pub fn is_newer(current: &str, remote: &str) -> bool {
    match (parse_semver(current), parse_semver(remote)) {
        (Some(c), Some(r)) => r > c,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newer_detects_patch_minor_major() {
        assert!(is_newer("0.2.5", "0.2.6"));
        assert!(is_newer("0.2.5", "0.3.0"));
        assert!(is_newer("0.9.9", "1.0.0"));
    }

    #[test]
    fn not_newer_when_equal_or_older() {
        assert!(!is_newer("0.2.5", "0.2.5"));
        assert!(!is_newer("0.2.6", "0.2.5"));
        assert!(!is_newer("1.0.0", "0.9.9"));
    }

    #[test]
    fn tolerates_v_prefix_and_bad_input() {
        assert!(is_newer("v0.2.5", "v0.2.6"));
        assert!(!is_newer("0.2.5", "not-a-version"));
    }

    #[test]
    fn parses_flat_manifest() {
        let json = r#"{
  "version": "0.2.6",
  "windows": { "url": "https://x/NovaKey-windows.zip", "sha256": "abc123" },
  "notes": "Bug fixes"
}"#;
        let m = parse_manifest(json).expect("should parse");
        assert_eq!(m.version, "0.2.6");
        assert_eq!(m.url, "https://x/NovaKey-windows.zip");
        assert_eq!(m.sha256, "abc123");
        assert_eq!(m.notes, "Bug fixes");
    }
}
```

Add `read_str` stub to `settings.rs` after `read_u32`:

```rust
/// Read a `"key": "value"` string from our flat JSON (no escape handling —
/// our manifests contain only plain URLs, hex, and short notes).
pub fn read_str(text: &str, key: &str) -> Option<String> {
    unimplemented!()
}
```

Add `mod updater;` to the module list in `main.rs` (after `mod tray;`).

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p novakey-win updater`
Expected: FAIL (panics: `not implemented`).

- [ ] **Step 3: Implement `read_str`, `parse_manifest`, `parse_semver`**

Replace `read_str` in `settings.rs`:

```rust
pub fn read_str(text: &str, key: &str) -> Option<String> {
    let needle = format!("\"{}\"", key);
    let idx = text.find(&needle)?;
    let rest = &text[idx + needle.len()..];
    let colon = rest.find(':')?;
    let after = rest[colon + 1..].trim_start();
    let after = after.strip_prefix('"')?;
    let end = after.find('"')?;
    Some(after[..end].to_string())
}
```

Replace the two stubs in `updater.rs`:

```rust
pub fn parse_manifest(text: &str) -> Option<RemoteVersion> {
    // The nested "windows" object is scanned as a substring; keys are unique
    // enough across the flat manifest that a windowed search is safe.
    let win_idx = text.find("\"windows\"")?;
    let win = &text[win_idx..];
    Some(RemoteVersion {
        version: settings::read_str(text, "version")?,
        url: settings::read_str(win, "url")?,
        sha256: settings::read_str(win, "sha256")?,
        notes: settings::read_str(text, "notes").unwrap_or_default(),
    })
}

fn parse_semver(v: &str) -> Option<(u32, u32, u32)> {
    let v = v.trim().strip_prefix('v').unwrap_or(v.trim());
    let mut it = v.split('.');
    let a = it.next()?.parse().ok()?;
    let b = it.next()?.parse().ok()?;
    let c = it.next()?.parse().ok()?;
    Some((a, b, c))
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p novakey-win updater`
Expected: PASS (4 tests).

- [ ] **Step 5: Commit**

```bash
git add crossplatform/windows/src/updater.rs crossplatform/windows/src/settings.rs crossplatform/windows/src/main.rs
git commit -m "feat(win): manifest parse + semver compare for updater"
```

---

## Task 3: Windows — SHA-256 (BCrypt) + Authenticode (WinTrust) verification

**Files:**
- Modify: `crossplatform/windows/src/updater.rs`
- Modify: `crossplatform/windows/Cargo.toml:16-28` (add features)

**Interfaces:**
- Produces:
  - `updater::sha256_hex(path: &std::path::Path) -> Option<String>` (lowercase hex)
  - `updater::verify_authenticode(path: &std::path::Path) -> bool`

- [ ] **Step 1: Add the required `windows` features**

In `crossplatform/windows/Cargo.toml`, add to the `features` list under `[dependencies.windows]`:

```toml
    "Win32_Networking_WinHttp",
    "Win32_Security_Cryptography",
    "Win32_Security_WinTrust",
    "Win32_System_Com",
```

- [ ] **Step 2: Write the failing test**

Add to the `tests` module in `updater.rs`:

```rust
    #[test]
    fn sha256_matches_known_vector() {
        use std::io::Write;
        let dir = std::env::temp_dir();
        let p = dir.join("novakey_sha_test.bin");
        let mut f = std::fs::File::create(&p).unwrap();
        f.write_all(b"abc").unwrap();
        drop(f);
        let got = super::sha256_hex(&p).unwrap();
        assert_eq!(
            got,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        let _ = std::fs::remove_file(&p);
    }
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test -p novakey-win sha256_matches`
Expected: FAIL to compile (`sha256_hex` not found) or panic.

- [ ] **Step 4: Implement `sha256_hex` via BCrypt**

Add to `updater.rs`:

```rust
use std::path::Path;

/// SHA-256 of a file, lowercase hex, via CNG BCrypt (no external crate).
pub fn sha256_hex(path: &Path) -> Option<String> {
    use windows::core::w;
    use windows::Win32::Security::Cryptography::{
        BCryptCloseAlgorithmProvider, BCryptCreateHash, BCryptDestroyHash, BCryptFinishHash,
        BCryptHashData, BCryptOpenAlgorithmProvider, BCRYPT_ALG_HANDLE, BCRYPT_HASH_HANDLE,
        BCRYPT_OPEN_ALGORITHM_PROVIDER_FLAGS,
    };

    let data = std::fs::read(path).ok()?;
    unsafe {
        let mut alg = BCRYPT_ALG_HANDLE::default();
        if BCryptOpenAlgorithmProvider(
            &mut alg,
            w!("SHA256"),
            None,
            BCRYPT_OPEN_ALGORITHM_PROVIDER_FLAGS(0),
        )
        .is_err()
        {
            return None;
        }
        let mut hash = BCRYPT_HASH_HANDLE::default();
        let mut out = [0u8; 32];
        let ok = BCryptCreateHash(alg, &mut hash, None, None, 0).is_ok()
            && BCryptHashData(hash, &data, 0).is_ok()
            && BCryptFinishHash(hash, &mut out, 0).is_ok();
        let _ = BCryptDestroyHash(hash);
        let _ = BCryptCloseAlgorithmProvider(alg, 0);
        if !ok {
            return None;
        }
        Some(out.iter().map(|b| format!("{:02x}", b)).collect())
    }
}
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p novakey-win sha256_matches`
Expected: PASS.

- [ ] **Step 6: Implement `verify_authenticode` (no unit test — needs a signed file)**

Add to `updater.rs`. This is verified end-to-end in Task 8, not by a unit test (CI runners have no NovaKey-signed sample):

```rust
/// True when `path` carries a valid Authenticode signature that chains to a
/// trusted root. Gates every downloaded exe before we launch it.
pub fn verify_authenticode(path: &Path) -> bool {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::Security::WinTrust::{
        WinVerifyTrust, WINTRUST_ACTION_GENERIC_VERIFY_V2, WINTRUST_DATA, WINTRUST_FILE_INFO,
        WTD_CHOICE_FILE, WTD_REVOKE_NONE, WTD_STATE_ACTION_CLOSE, WTD_STATE_ACTION_VERIFY,
        WTD_UI_NONE, WINTRUST_DATA_UICONTEXT,
    };

    let wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    unsafe {
        let mut file = WINTRUST_FILE_INFO {
            cbStruct: std::mem::size_of::<WINTRUST_FILE_INFO>() as u32,
            pcwszFilePath: PCWSTR(wide.as_ptr()),
            hFile: HANDLE::default(),
            pgKnownSubject: std::ptr::null_mut(),
        };
        let mut data = WINTRUST_DATA {
            cbStruct: std::mem::size_of::<WINTRUST_DATA>() as u32,
            dwUIChoice: WTD_UI_NONE,
            fdwRevocationChecks: WTD_REVOKE_NONE,
            dwUnionChoice: WTD_CHOICE_FILE,
            dwStateAction: WTD_STATE_ACTION_VERIFY,
            dwUIContext: WINTRUST_DATA_UICONTEXT(0),
            ..Default::default()
        };
        data.Anonymous.pFile = &mut file;
        let mut action = WINTRUST_ACTION_GENERIC_VERIFY_V2;
        let status = WinVerifyTrust(
            windows::Win32::Foundation::HWND(std::ptr::null_mut()),
            &mut action,
            &mut data as *mut _ as *mut core::ffi::c_void,
        );
        // Close the WinVerifyTrust state regardless of result.
        data.dwStateAction = WTD_STATE_ACTION_CLOSE;
        let _ = WinVerifyTrust(
            windows::Win32::Foundation::HWND(std::ptr::null_mut()),
            &mut action,
            &mut data as *mut _ as *mut core::ffi::c_void,
        );
        status == 0 // ERROR_SUCCESS
    }
}
```

Add `use std::os::windows::ffi::OsStrExt;` at the top of `updater.rs` for `encode_wide`.

- [ ] **Step 7: Confirm the crate still builds**

Run: `cargo build -p novakey-win`
Expected: builds clean (no unused-import errors — `verify_authenticode` is referenced in Task 5).

If `verify_authenticode` is unused at this point, add `#[allow(dead_code)]` above it and remove that attribute in Task 5.

- [ ] **Step 8: Commit**

```bash
git add crossplatform/windows/src/updater.rs crossplatform/windows/Cargo.toml
git commit -m "feat(win): sha256 + authenticode verification for updater"
```

---

## Task 4: Windows — WinHTTP download (manifest text + binary file)

**Files:**
- Modify: `crossplatform/windows/src/updater.rs`

**Interfaces:**
- Produces:
  - `updater::http_get_string(url: &str) -> Option<String>`
  - `updater::download_to(url: &str, dest: &std::path::Path) -> bool`

- [ ] **Step 1: Implement the WinHTTP helper (no unit test — needs network)**

WinHTTP GET is verified live in Task 8. Add to `updater.rs`:

```rust
use windows::core::PCWSTR;
use windows::Win32::Networking::WinHttp::{
    WinHttpCloseHandle, WinHttpConnect, WinHttpCrackUrl, WinHttpOpen, WinHttpOpenRequest,
    WinHttpQueryDataAvailable, WinHttpReadData, WinHttpReceiveResponse, WinHttpSendRequest,
    URL_COMPONENTS, WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, WINHTTP_FLAG_SECURE,
    WINHTTP_OPEN_REQUEST_FLAGS,
};

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Fetch a whole https URL into a Vec<u8>. Returns None on any failure.
fn http_get_bytes(url: &str) -> Option<Vec<u8>> {
    let wide = to_wide(url);
    unsafe {
        // Split scheme/host/path with WinHttpCrackUrl.
        let mut comp = URL_COMPONENTS {
            dwStructSize: std::mem::size_of::<URL_COMPONENTS>() as u32,
            dwSchemeLength: u32::MAX,
            dwHostNameLength: u32::MAX,
            dwUrlPathLength: u32::MAX,
            dwExtraInfoLength: u32::MAX,
            ..Default::default()
        };
        if WinHttpCrackUrl(&wide[..wide.len() - 1], 0, &mut comp).is_err() {
            return None;
        }
        let host: Vec<u16> = std::slice::from_raw_parts(
            comp.lpszHostName.0,
            comp.dwHostNameLength as usize,
        )
        .to_vec();
        let host_z: Vec<u16> = host.iter().copied().chain(std::iter::once(0)).collect();
        let path: Vec<u16> = std::slice::from_raw_parts(
            comp.lpszUrlPath.0,
            (comp.dwUrlPathLength + comp.dwExtraInfoLength) as usize,
        )
        .to_vec();
        let path_z: Vec<u16> = path.iter().copied().chain(std::iter::once(0)).collect();

        let session = WinHttpOpen(
            PCWSTR(to_wide("NovaKey-Updater/1.0").as_ptr()),
            WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
            PCWSTR::null(),
            PCWSTR::null(),
            0,
        );
        if session.is_null() {
            return None;
        }
        let conn = WinHttpConnect(session, PCWSTR(host_z.as_ptr()), comp.nPort, 0);
        if conn.is_null() {
            let _ = WinHttpCloseHandle(session);
            return None;
        }
        let req = WinHttpOpenRequest(
            conn,
            PCWSTR(to_wide("GET").as_ptr()),
            PCWSTR(path_z.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            std::ptr::null_mut(),
            WINHTTP_FLAG_SECURE,
        );
        let cleanup = |r, c, s| {
            let _ = WinHttpCloseHandle(r);
            let _ = WinHttpCloseHandle(c);
            let _ = WinHttpCloseHandle(s);
        };
        if req.is_null() {
            cleanup(std::ptr::null_mut(), conn, session);
            return None;
        }
        if WinHttpSendRequest(req, None, None, 0, 0, 0).is_err()
            || WinHttpReceiveResponse(req, std::ptr::null_mut()).is_err()
        {
            cleanup(req, conn, session);
            return None;
        }
        let mut out = Vec::new();
        loop {
            let mut avail: u32 = 0;
            if WinHttpQueryDataAvailable(req, &mut avail).is_err() || avail == 0 {
                break;
            }
            let mut buf = vec![0u8; avail as usize];
            let mut read: u32 = 0;
            if WinHttpReadData(
                req,
                buf.as_mut_ptr() as *mut core::ffi::c_void,
                avail,
                &mut read,
            )
            .is_err()
            {
                break;
            }
            buf.truncate(read as usize);
            out.extend_from_slice(&buf);
        }
        cleanup(req, conn, session);
        Some(out)
    }
}

/// Fetch a URL as UTF-8 text (the manifest).
pub fn http_get_string(url: &str) -> Option<String> {
    let bytes = http_get_bytes(url)?;
    String::from_utf8(bytes).ok()
}

/// Download a URL to a file. Returns whether it succeeded.
pub fn download_to(url: &str, dest: &Path) -> bool {
    match http_get_bytes(url) {
        Some(bytes) if !bytes.is_empty() => {
            if let Some(dir) = dest.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            std::fs::write(dest, bytes).is_ok()
        }
        _ => false,
    }
}
```

> Note for the implementer: `WinHttpOpenRequest`'s accept-types arg and the exact `URL_COMPONENTS` field names can vary slightly across `windows` 0.58 — if a name mismatches, resolve it against the crate docs for 0.58 rather than changing the logic. The unused `WINHTTP_OPEN_REQUEST_FLAGS` import may be dropped if the compiler flags it.

- [ ] **Step 2: Confirm it builds**

Run: `cargo build -p novakey-win`
Expected: builds clean.

- [ ] **Step 3: Commit**

```bash
git add crossplatform/windows/src/updater.rs
git commit -m "feat(win): winhttp download for updater"
```

---

## Task 5: Windows — rename-swap + relaunch + orchestrator

**Files:**
- Modify: `crossplatform/windows/src/updater.rs`

**Interfaces:**
- Consumes: everything from Tasks 2–4.
- Produces:
  - `updater::UpdateOutcome { UpToDate, Available(RemoteVersion), Applied, Failed }`
  - `updater::check_now() -> UpdateOutcome` — full pipeline: fetch → compare → download → verify → swap → relaunch (does not return on success; process is replaced).
  - `updater::finish_pending_update()` — called at startup with `--finish-update`: delete the leftover `NovaKey-old.exe`.

- [ ] **Step 1: Set the real FEED_URL**

Replace the `FEED_URL` constant placeholder with the value from Task 1 (`R2_PUBLIC_BASE` verbatim):

```rust
pub const FEED_URL: &str = "https://<R2_PUBLIC_BASE>/latest/latest.json";
```

- [ ] **Step 2: Implement the swap + orchestrator**

Add to `updater.rs`:

```rust
#[derive(Debug)]
pub enum UpdateOutcome {
    UpToDate,
    Applied,          // process is being replaced; caller should exit
    Failed(&'static str),
}

/// Directory for staged downloads: %LOCALAPPDATA%\NovaKey\update
fn staging_dir() -> Option<std::path::PathBuf> {
    let base = std::env::var_os("LOCALAPPDATA")?;
    let mut p = std::path::PathBuf::from(base);
    p.push("NovaKey");
    p.push("update");
    Some(p)
}

/// Full update pipeline. On success the running exe is renamed aside, the new
/// exe takes its place, a fresh instance is spawned with `--finish-update`, and
/// this function returns `Applied` so the caller can quit.
pub fn check_now() -> UpdateOutcome {
    let text = match http_get_string(FEED_URL) {
        Some(t) => t,
        None => return UpdateOutcome::Failed("fetch manifest"),
    };
    let remote = match parse_manifest(&text) {
        Some(r) => r,
        None => return UpdateOutcome::Failed("parse manifest"),
    };
    if !is_newer(CURRENT_VERSION, &remote.version) {
        return UpdateOutcome::UpToDate;
    }

    let dir = match staging_dir() {
        Some(d) => d,
        None => return UpdateOutcome::Failed("staging dir"),
    };
    let zip_path = dir.join("NovaKey-windows.zip");
    if !download_to(&remote.url, &zip_path) {
        return UpdateOutcome::Failed("download");
    }
    // Integrity: manifest SHA-256 must match.
    match sha256_hex(&zip_path) {
        Some(h) if h.eq_ignore_ascii_case(&remote.sha256) => {}
        _ => return UpdateOutcome::Failed("sha256 mismatch"),
    }
    // Extract NovaKey.exe from the zip.
    let new_exe = dir.join("NovaKey-new.exe");
    if !extract_exe(&zip_path, &new_exe) {
        return UpdateOutcome::Failed("extract");
    }
    // Authenticity: new exe must pass Authenticode.
    if !verify_authenticode(&new_exe) {
        return UpdateOutcome::Failed("authenticode");
    }
    match swap_and_relaunch(&new_exe) {
        true => UpdateOutcome::Applied,
        false => UpdateOutcome::Failed("swap"),
    }
}

/// Rename the running exe aside, move the new exe into its place, spawn a fresh
/// instance with `--finish-update`. Windows allows renaming a running exe.
fn swap_and_relaunch(new_exe: &Path) -> bool {
    let cur = match std::env::current_exe() {
        Ok(p) => p,
        Err(_) => return false,
    };
    let old = cur.with_extension("old.exe");
    let _ = std::fs::remove_file(&old); // clear a stale one
    if std::fs::rename(&cur, &old).is_err() {
        return false;
    }
    if std::fs::rename(new_exe, &cur).is_err() {
        // Roll back so we're never left without an exe.
        let _ = std::fs::rename(&old, &cur);
        return false;
    }
    // Spawn the replacement; it waits for our mutex to free, then deletes .old.
    use std::os::windows::process::CommandExt;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    std::process::Command::new(&cur)
        .arg("--finish-update")
        .creation_flags(DETACHED_PROCESS)
        .spawn()
        .is_ok()
}

/// Extract the single `NovaKey.exe` entry from the release zip. Uses the tiny
/// stored/deflate reader below — no zip crate.
fn extract_exe(zip: &Path, dest: &Path) -> bool {
    match crate::updater::zip::extract_named(zip, "NovaKey.exe", dest) {
        Ok(()) => true,
        Err(_) => false,
    }
}

/// Startup handler for the `--finish-update` relaunch: remove the leftover
/// `NovaKey-old.exe`. Safe to call when nothing is pending.
pub fn finish_pending_update() {
    if let Ok(cur) = std::env::current_exe() {
        let old = cur.with_extension("old.exe");
        // The parent may still be exiting; retry briefly.
        for _ in 0..25 {
            if std::fs::remove_file(&old).is_ok() || !old.exists() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
    }
}
```

- [ ] **Step 3: Add a minimal zip extractor sub-module + test**

The release zip is produced by PowerShell `Compress-Archive`, which stores entries with **deflate** compression. Add a small extractor to `updater.rs` using the `windows` Compression API (`Cabinet.dll`'s raw deflate is awkward; instead use the already-available `flate2`? No — no new crate). Use the built-in **`Compression`** COM? Simplest zero-crate path: read the zip's central directory, then inflate with the Win32 `RtlDecompressBufferEx`? That does not support zlib/deflate streams cleanly.

Decision: add the well-audited, tiny `zip` avoidance is not worth it here — **add one crate `zip = { version = "2", default-features = false, features = ["deflate"] }`** to `Cargo.toml`. It is the pragmatic, safe choice for archive extraction and pulls only `flate2`. Document the exception to the minimal-crate rule in a comment.

Update `Cargo.toml`:

```toml
zip = { version = "2", default-features = false, features = ["deflate"] }
```

Replace `extract_exe` body with:

```rust
fn extract_exe(zip_path: &Path, dest: &Path) -> bool {
    let file = match std::fs::File::open(zip_path) {
        Ok(f) => f,
        Err(_) => return false,
    };
    let mut archive = match zip::ZipArchive::new(file) {
        Ok(a) => a,
        Err(_) => return false,
    };
    let mut entry = match archive.by_name("NovaKey.exe") {
        Ok(e) => e,
        Err(_) => return false,
    };
    let mut out = match std::fs::File::create(dest) {
        Ok(f) => f,
        Err(_) => return false,
    };
    std::io::copy(&mut entry, &mut out).is_ok()
}
```

Remove the `crate::updater::zip::extract_named` reference and the `zip` sub-module note from Step 2's `extract_exe`. Add a unit test that round-trips a tiny in-memory zip:

```rust
    #[test]
    fn extracts_named_entry() {
        use std::io::Write;
        let dir = std::env::temp_dir();
        let zp = dir.join("novakey_extract_test.zip");
        {
            let f = std::fs::File::create(&zp).unwrap();
            let mut w = zip::ZipWriter::new(f);
            w.start_file("NovaKey.exe", zip::write::SimpleFileOptions::default())
                .unwrap();
            w.write_all(b"MZ-fake-exe").unwrap();
            w.finish().unwrap();
        }
        let out = dir.join("novakey_extract_out.exe");
        assert!(super::extract_exe(&zp, &out));
        assert_eq!(std::fs::read(&out).unwrap(), b"MZ-fake-exe");
        let _ = std::fs::remove_file(&zp);
        let _ = std::fs::remove_file(&out);
    }
```

(`zip` dev-usage in the test needs the `zip` write feature — add `features = ["deflate"]` already covers `ZipWriter`.)

- [ ] **Step 4: Run tests**

Run: `cargo test -p novakey-win`
Expected: all updater tests PASS (semver, sha256, extract).

- [ ] **Step 5: Commit**

```bash
git add crossplatform/windows/src/updater.rs crossplatform/windows/Cargo.toml
git commit -m "feat(win): update swap/relaunch + zip extract + orchestrator"
```

---

## Task 6: Windows — wire tray menu, startup path, background check

**Files:**
- Modify: `crossplatform/windows/src/tray.rs:26-35` (add `CMD_UPDATE`), `:124-157` (menu item)
- Modify: `crossplatform/windows/src/main.rs` (startup arg, dispatch, background thread)
- Modify: `crossplatform/windows/src/settings.rs` (add `last_update_check` + `read_u64`)

**Interfaces:**
- Consumes: `updater::check_now`, `updater::finish_pending_update`, `updater::UpdateOutcome`.
- Produces: user-visible "Check for Updates…" tray item; silent daily check on launch.

- [ ] **Step 1: Add `last_update_check` + `read_u64` (TDD)**

Add a failing test to `settings.rs` tests module:

```rust
    #[test]
    fn parses_u64_timestamp() {
        let json = "{\n  \"lastUpdateCheck\": 1750000000\n}";
        assert_eq!(read_u64(json, "lastUpdateCheck"), Some(1_750_000_000));
        assert_eq!(read_u64(json, "missing"), None);
    }
```

Run: `cargo test -p novakey-win parses_u64` → FAIL (no `read_u64`).

Implement `read_u64` (mirror `read_u32` with `u64`), add `pub last_update_check: u64` to `Settings` (default `0`), include it in `Default`, `load` (`read_u64(&text, "lastUpdateCheck").unwrap_or(0)`), and `save` (append `,\n  "lastUpdateCheck": {}`). Run the test → PASS.

- [ ] **Step 2: Add the tray menu item**

In `tray.rs`, add after `CMD_DEFERRED`:

```rust
pub const CMD_UPDATE: usize = 10;
```

In `show_menu`, add before the final separator/Quit:

```rust
        let _ = AppendMenuW(menu, MF_STRING, CMD_UPDATE, w("Check for Updates…"));
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
```

(Place it after the `CMD_AUTOCOMPLETE` line and before the existing `MF_SEPARATOR` that precedes `CMD_QUIT`.)

- [ ] **Step 3: Handle `--finish-update` at startup + mutex retry**

In `main.rs` `run()`, immediately after computing `mutex_name`, branch on the CLI arg. Replace the single-shot mutex guard with a version that retries when finishing an update (the parent is still exiting and holds the mutex):

```rust
    let finishing = std::env::args().any(|a| a == "--finish-update");

    let _instance_mutex = {
        let mut handle = None;
        let attempts = if finishing { 25 } else { 1 };
        for _ in 0..attempts {
            match CreateMutexW(None, false, PCWSTR(mutex_name.as_ptr())) {
                Ok(h) => {
                    if GetLastError() == ERROR_ALREADY_EXISTS {
                        let _ = CloseHandle(h);
                        std::thread::sleep(std::time::Duration::from_millis(200));
                        continue;
                    }
                    handle = Some(h);
                    break;
                }
                Err(_) => return,
            }
        }
        match handle {
            Some(h) => h,
            None => return, // another instance genuinely running
        }
    };

    if finishing {
        updater::finish_pending_update();
    }
```

(Remove the old `let _instance_mutex = match CreateMutexW(...)` block and its `ERROR_ALREADY_EXISTS` early return that this replaces.)

- [ ] **Step 4: Dispatch `CMD_UPDATE`**

In `main.rs` `handle_command`, add an arm. Run the check off the UI thread so the pump keeps servicing messages; on `Applied`, quit:

```rust
        tray::CMD_UPDATE => {
            spawn_update_check(hwnd, /*background=*/ false);
        }
```

Add the helper near `do_toggle`:

```rust
/// Run the update check on a worker thread. `background` suppresses the
/// "already up to date" and failure notifications.
fn spawn_update_check(hwnd: HWND, background: bool) {
    let hwnd_val = hwnd.0 as isize;
    std::thread::spawn(move || {
        let outcome = updater::check_now();
        match outcome {
            updater::UpdateOutcome::Applied => {
                // New instance is launching; tear this one down cleanly.
                // MUST post, not call DestroyWindow directly: Win32 forbids
                // destroying a window from a thread other than its creator.
                unsafe {
                    let _ = PostMessageW(HWND(hwnd_val as *mut _), WM_CLOSE, WPARAM(0), LPARAM(0));
                }
            }
            updater::UpdateOutcome::UpToDate if !background => {
                unsafe { let _ = MessageBeep(MB_OK); }
            }
            _ => {}
        }
    });
}
```

> `PostMessageW(WM_CLOSE)` is thread-safe. `wnd_proc` has no `WM_CLOSE` arm, so it falls through to `DefWindowProcW` **on the pump thread**, whose default `WM_CLOSE` handling calls `DestroyWindow` there — triggering the existing `WM_DESTROY` arm (`tray::remove` + `PostQuitMessage`). Calling `DestroyWindow` directly from the worker thread would silently no-op and the old process would never exit.

- [ ] **Step 5: Background check on launch (daily throttle)**

In `run()`, after `tray::add(...)`, spawn a one-shot background check if >24h since last:

```rust
    {
        let last = SETTINGS.with(|s| s.borrow().last_update_check);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        if now.saturating_sub(last) > 86_400 {
            SETTINGS.with(|s| {
                let mut s = s.borrow_mut();
                s.last_update_check = now;
                s.save();
            });
            spawn_update_check(hwnd, /*background=*/ true);
        }
    }
```

- [ ] **Step 6: Build + run the existing suite**

Run: `cargo build -p novakey-win && cargo test -p novakey-win`
Expected: builds; all tests PASS.

- [ ] **Step 7: Commit**

```bash
git add crossplatform/windows/src
git commit -m "feat(win): tray Check for Updates + daily background check"
```

---

## Task 7: macOS — add Sparkle, UpdaterController, Info.plist keys

**Files:**
- Modify: `Package.swift`
- Create: `Sources/NovaKey/Updates/UpdaterController.swift`
- Modify: `Resources/Info.plist:20-24` (insert Sparkle keys)

**Interfaces:**
- Produces: `UpdaterController.shared` with `func checkForUpdates()`.

- [ ] **Step 1: Add Sparkle to Package.swift**

```swift
// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "NovaKey",
    platforms: [
        .macOS(.v14)
    ],
    dependencies: [
        .package(url: "https://github.com/sparkle-project/Sparkle", from: "2.6.0"),
    ],
    targets: [
        .executableTarget(
            name: "NovaKey",
            dependencies: [
                .product(name: "Sparkle", package: "Sparkle"),
            ],
            path: "Sources/NovaKey",
            linkerSettings: [
                .linkedFramework("Cocoa"),
                .linkedFramework("Carbon"),
            ]
        ),
    ]
)
```

- [ ] **Step 2: Add the Info.plist keys**

Insert after the `CFBundleVersion` block (line ~24). Use the **public** key from Task 1 Step 3:

```xml
    <key>SUFeedURL</key>
    <string>https://<R2_PUBLIC_BASE>/latest/appcast.xml</string>
    <key>SUPublicEDKey</key>
    <string><PASTE_SPARKLE_PUBLIC_KEY_FROM_TASK_1></string>
    <key>SUEnableAutomaticChecks</key>
    <true/>
    <key>SUScheduledCheckInterval</key>
    <integer>86400</integer>
```

- [ ] **Step 3: Create UpdaterController**

`Sources/NovaKey/Updates/UpdaterController.swift`:

```swift
// UpdaterController.swift
// Thin wrapper over Sparkle's standard updater. Reads SUFeedURL / SUPublicEDKey
// from Info.plist; SUEnableAutomaticChecks drives the daily background check.

import Foundation
import Sparkle

final class UpdaterController {
    static let shared = UpdaterController()

    private let controller: SPUStandardUpdaterController

    private init() {
        // startingUpdater: true begins the scheduled background check loop.
        controller = SPUStandardUpdaterController(
            startingUpdater: true,
            updaterDelegate: nil,
            userDriverDelegate: nil
        )
    }

    /// User-initiated check — shows Sparkle's UI (progress, release notes).
    func checkForUpdates() {
        controller.checkForUpdates(nil)
    }
}
```

- [ ] **Step 4: Resolve dependencies + build**

Run: `swift build -c release`
Expected: Sparkle resolves and the target builds. (A dev build won't self-update without a signed feed — that's exercised in Task 13.)

- [ ] **Step 5: Commit**

```bash
git add Package.swift Package.resolved Sources/NovaKey/Updates/UpdaterController.swift Resources/Info.plist
git commit -m "feat(mac): integrate Sparkle updater"
```

---

## Task 8: macOS — wire "Check for Updates…" into the menu

**Files:**
- Modify: `Sources/NovaKey/UI/StatusBarController.swift:36-44` (callback), `:132-234` (menu row)
- Modify: `Sources/NovaKey/App/AppDelegate.swift:69-76`

**Interfaces:**
- Consumes: `UpdaterController.shared.checkForUpdates()`.

- [ ] **Step 1: Add the callback through the popover**

In `StatusBarController`, add a stored callback and pass it into the SwiftUI view:

```swift
    var onCheckForUpdates: (() -> Void)?
```

In `setup()`, extend `MenuBarPopoverView(...)` construction with:

```swift
            onCheckForUpdates: { [weak self] in
                self?.closePopover()
                self?.onCheckForUpdates?()
            },
```

Add the matching stored property to `MenuBarPopoverView`:

```swift
    let onCheckForUpdates: () -> Void
```

- [ ] **Step 2: Add the menu row**

In `MenuBarPopoverView.menuRows`, add after the "About NovaKey" row:

```swift
            MenuRow(title: "Check for Updates…", trailing: AnyView(EmptyView()),
                    action: onCheckForUpdates)
```

- [ ] **Step 3: Wire it in AppDelegate**

In `applicationDidFinishLaunching`, after `statusBarController.onQuit = { ... }`:

```swift
        statusBarController.onCheckForUpdates = {
            UpdaterController.shared.checkForUpdates()
        }
```

Also touch `UpdaterController.shared` once at launch (after `enableLaunchAtLogin()`) so the scheduled background checker starts:

```swift
        _ = UpdaterController.shared
```

- [ ] **Step 4: Build + smoke-run**

Run: `./build.sh` then launch `build/NovaKey.app`. Open the menu-bar popover, confirm "Check for Updates…" appears and clicking it opens Sparkle's UI (it will report a network/feed error against the not-yet-published feed — that's expected until Task 13).

Expected: no crash; menu item present and clickable.

- [ ] **Step 5: Commit**

```bash
git add Sources/NovaKey/UI/StatusBarController.swift Sources/NovaKey/App/AppDelegate.swift
git commit -m "feat(mac): Check for Updates menu item"
```

---

## Task 9: macOS — embed + sign Sparkle.framework in build.sh

**Files:**
- Modify: `build.sh:30-58`

**Interfaces:**
- Produces: a bundle whose hardened-runtime signature is valid with Sparkle embedded (required for notarization to pass in CI).

- [ ] **Step 1: Copy Sparkle.framework into the bundle**

Sparkle resolves under `.build/.../checkouts` / `.build/artifacts`. After `swift build`, locate the built `Sparkle.framework` and copy it into `Contents/Frameworks`. Add after the resource copies in `build.sh` (before the Sign section):

```bash
# ── Embed Sparkle.framework ─────────────────────────────────────────────────
FRAMEWORKS="$CONTENTS/Frameworks"
mkdir -p "$FRAMEWORKS"
SPARKLE_FW="$(find .build -type d -name 'Sparkle.framework' -path '*artifacts*' | head -1)"
if [[ -z "$SPARKLE_FW" ]]; then
  SPARKLE_FW="$(find .build -type d -name 'Sparkle.framework' | head -1)"
fi
if [[ -z "$SPARKLE_FW" ]]; then echo "✗ Sparkle.framework not found under .build" >&2; exit 1; fi
cp -R "$SPARKLE_FW" "$FRAMEWORKS/Sparkle.framework"
```

- [ ] **Step 2: Sign the framework + nested helpers inside-out**

Sparkle bundles helper executables (Autoupdate, Updater.app, XPC services) that each need signing **before** the outer bundle. Extend the release-signing branch (`if [[ -n "${SIGN_IDENTITY:-}" ]]`) so it signs Sparkle's nested code first:

```bash
    # Sparkle's nested helpers/XPC first (inside-out), then its framework.
    SPARKLE="$FRAMEWORKS/Sparkle.framework"
    find "$SPARKLE/Versions/B" \( -name '*.xpc' -o -name '*.app' -o -type f -perm +111 \) \
        -print0 2>/dev/null | while IFS= read -r -d '' item; do
        codesign "${SIGN_ARGS[@]}" "$item" || true
    done
    codesign "${SIGN_ARGS[@]}" "$SPARKLE"
```

Do the same in the ad-hoc branch using its `--sign -` args so local `./build.sh` also produces a loadable bundle. Then the existing `codesign "$BIN"` / `codesign "$APP"` lines run last (unchanged), signing the main binary and the outer bundle after the framework.

- [ ] **Step 3: Verify a full signed bundle**

Run: `SIGN_IDENTITY="Developer ID Application: … (TEAMID)" ./build.sh` (or ad-hoc `./build.sh` if no cert locally).
Then: `codesign --verify --strict --verbose=2 build/NovaKey.app`
Expected: `valid on disk` / `satisfies its Designated Requirement`. `spctl` is not expected to pass for ad-hoc.

- [ ] **Step 4: Commit**

```bash
git add build.sh
git commit -m "build(mac): embed + sign Sparkle.framework"
```

---

## Task 10: CI — generate the EdDSA-signed appcast (macOS job)

**Files:**
- Modify: `.github/workflows/release.yml:64-73` (macOS job, after packaging)

**Interfaces:**
- Consumes: `NovaKey-macos.zip` artifact; secret `SPARKLE_ED_PRIVATE_KEY`.
- Produces: `appcast.xml` artifact carrying the EdDSA signature + `${R2_PUBLIC_BASE}` download URL.

- [ ] **Step 1: Add appcast generation steps**

After the `Package artifact (.zip)` step in the `macos` job, insert:

```yaml
      - name: Fetch Sparkle tools
        run: |
          set -euo pipefail
          curl -L -o sparkle.tar.xz \
            https://github.com/sparkle-project/Sparkle/releases/download/2.6.4/Sparkle-2.6.4.tar.xz
          mkdir sparkle && tar -xf sparkle.tar.xz -C sparkle

      - name: Generate + sign appcast
        env:
          SPARKLE_ED_PRIVATE_KEY: ${{ secrets.SPARKLE_ED_PRIVATE_KEY }}
          R2_PUBLIC_BASE: ${{ secrets.R2_PUBLIC_BASE }}
          TAG: ${{ github.event.inputs.tag || github.ref_name }}
        run: |
          set -euo pipefail
          mkdir feed && cp NovaKey-macos.zip feed/
          printf '%s' "$SPARKLE_ED_PRIVATE_KEY" > ed_private.pem
          # download-url-prefix points at the versioned R2 path for this tag.
          ./sparkle/bin/generate_appcast \
            --ed-key-file ed_private.pem \
            --download-url-prefix "${R2_PUBLIC_BASE}/${TAG}/" \
            feed
          rm -f ed_private.pem
          cp feed/appcast.xml appcast.xml

      - uses: actions/upload-artifact@v4
        with:
          name: novakey-appcast
          path: appcast.xml
```

> Store `R2_PUBLIC_BASE` as a repo secret (or `vars`) so both jobs read the same origin. If a repo **variable** is preferred, reference `${{ vars.R2_PUBLIC_BASE }}` consistently in Tasks 10 & 11.

- [ ] **Step 2: Validate the workflow syntax**

Run locally: `actionlint .github/workflows/release.yml` (or push to a branch and confirm the Actions tab parses it).
Expected: no syntax errors.

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/release.yml
git commit -m "ci(mac): generate EdDSA-signed Sparkle appcast"
```

---

## Task 11: CI — emit latest.json + upload both feeds to R2 (release job)

**Files:**
- Modify: `.github/workflows/release.yml:151-186` (release job)

**Interfaces:**
- Consumes: `NovaKey-windows.zip`, `appcast.xml` (from merged artifacts).
- Produces: `latest.json` + both feeds under R2 `/latest/` and `/{TAG}/`.

- [ ] **Step 1: Pull the appcast artifact into the release job**

The existing `download-artifact` step uses `merge-multiple: true` into `dist/`, so `appcast.xml` lands in `dist/` alongside the zips. No change needed beyond referencing `dist/appcast.xml`.

- [ ] **Step 2: Build latest.json before the R2 upload**

Insert a step before `Upload assets to Cloudflare R2`:

```yaml
      - name: Build Windows update manifest (latest.json)
        env:
          TAG: ${{ github.event.inputs.tag || github.ref_name }}
          R2_PUBLIC_BASE: ${{ secrets.R2_PUBLIC_BASE }}
        run: |
          set -euo pipefail
          VER="${TAG#v}"
          SHA="$(sha256sum dist/NovaKey-windows.zip | cut -d' ' -f1)"
          cat > dist/latest.json <<EOF
          {
            "version": "${VER}",
            "windows": {
              "url": "${R2_PUBLIC_BASE}/${TAG}/NovaKey-windows.zip",
              "sha256": "${SHA}"
            },
            "notes": "See the GitHub release for ${TAG}."
          }
          EOF
          cat dist/latest.json
```

- [ ] **Step 3: Upload the feeds to R2 (versioned + latest)**

Extend the existing R2 upload loop's step. After the `for f in dist/*.zip` loop, add feed uploads:

```bash
          # Update feeds: versioned copy + rolling latest.
          for feed in latest.json appcast.xml; do
            if [[ -f "dist/$feed" ]]; then
              aws s3 cp "dist/$feed" "s3://${R2_BUCKET}/${TAG}/${feed}"  --endpoint-url "$ENDPOINT" --cache-control "no-cache"
              aws s3 cp "dist/$feed" "s3://${R2_BUCKET}/latest/${feed}"  --endpoint-url "$ENDPOINT" --cache-control "no-cache"
            fi
          done
```

> `--cache-control no-cache` keeps the rolling `/latest/` feeds from being served stale by Cloudflare's cache. The zips remain immutable per tag.

- [ ] **Step 4: Validate + dry-run reasoning**

Run: `actionlint .github/workflows/release.yml`.
Confirm by inspection: `sha256sum` output feeds the same value the Windows updater compares (`updater::sha256_hex`, lowercase hex — matches `sha256sum`), and the `url` matches `FEED_URL`'s host + the `/{TAG}/NovaKey-windows.zip` path the release loop uploads.
Expected: no lint errors; URL/host/hash consistency confirmed.

- [ ] **Step 5: Commit**

```bash
git add .github/workflows/release.yml
git commit -m "ci: publish latest.json + appcast to R2 update feeds"
```

---

## Task 12: End-to-end release dry-run (Windows)

**Files:** none (verification against a real pre-release tag).

- [ ] **Step 1: Cut a test tag one patch above current**

Bump both version sources to a test value **above** `0.2.5`, e.g. `0.2.6`:
- `crossplatform/windows/Cargo.toml:3` → `version = "0.2.6"`
- `Resources/Info.plist` `CFBundleShortVersionString` → `0.2.6`
- `Sources/NovaKey/App/Constants.swift:6` → `0.2.5` stays only if unused by updater; update to `0.2.6` for consistency.

Commit, then `git tag v0.2.6 && git push --tags`. Let the release workflow run.

- [ ] **Step 2: Verify feeds are live**

```bash
curl -s "${R2_PUBLIC_BASE}/latest/latest.json"
curl -sI "${R2_PUBLIC_BASE}/latest/appcast.xml"
```

Expected: `latest.json` shows `"version": "0.2.6"` and a reachable `url`; `appcast.xml` returns `200`.

- [ ] **Step 3: Verify Windows self-update from an older build**

On a Windows machine, run a `0.2.5` build of `NovaKey.exe` (build one from the pre-tag commit if needed). Tray → "Check for Updates…". Confirm: it downloads, passes SHA-256 + Authenticode, swaps the exe, relaunches, and the new tray tooltip/version reflects `0.2.6`; `NovaKey-old.exe` is gone from the install dir.

Expected: seamless in-place upgrade; single instance preserved (no duplicate tray icon).

- [ ] **Step 4: Verify a tampered download is rejected (negative test)**

Temporarily point a local test build's `FEED_URL` at a manifest whose `sha256` is wrong. Confirm the updater aborts with `Failed("sha256 mismatch")` and does **not** launch anything.

Expected: no swap occurs; the running exe is untouched.

- [ ] **Step 5: Commit any version bumps**

```bash
git add -A
git commit -m "chore: bump to 0.2.6 for update dry-run"
```

---

## Task 13: End-to-end release dry-run (macOS via Sparkle)

**Files:** none (verification).

- [ ] **Step 1: Install the older signed build**

Install a notarized `0.2.5` `NovaKey.app` (from before the `v0.2.6` tag) into `/Applications`. Launch it.

- [ ] **Step 2: Trigger the check**

Menu-bar popover → "Check for Updates…". Sparkle should read `${R2_PUBLIC_BASE}/latest/appcast.xml`, find `0.2.6`, verify the EdDSA signature against `SUPublicEDKey`, download from the `/{TAG}/` URL, and prompt to install.

Expected: Sparkle installs `0.2.6`, relaunches; "Check for Updates…" now reports up to date.

- [ ] **Step 3: Verify signature enforcement (negative test)**

Confirm Sparkle refuses an appcast entry whose EdDSA signature doesn't match (e.g. hand-edit a local appcast). Sparkle logs a signature failure and does not install.

Expected: update blocked.

- [ ] **Step 4: Verify background scheduling**

Confirm `SUEnableAutomaticChecks` is honored: with the app left running, Sparkle performs its scheduled check (defaults from `SUScheduledCheckInterval`). Inspect `Console.app` for Sparkle scheduling logs, or set a short interval locally to observe it.

Expected: a background check occurs without user action.

---

## Self-Review

**Spec coverage:**
- R2-hosted manifests → Tasks 10 (appcast), 11 (latest.json), both uploaded to `/latest/` + `/{TAG}/`. ✓
- macOS via Sparkle → Tasks 7 (integrate), 8 (menu), 9 (embed/sign), 13 (verify). ✓
- Windows custom updater → Tasks 2–6 (parse, verify, download, swap, wire), 12 (verify). ✓
- Manual trigger → Task 6 (`CMD_UPDATE`), Task 8 (menu row). ✓
- Background check → Task 6 (daily throttle, Windows), Task 7 (`SUEnableAutomaticChecks`, macOS). ✓
- Security (signature/hash verify before launch) → Task 3 + Task 5 gates (Windows), Sparkle EdDSA (macOS); negative tests in 12/13. ✓

**Type consistency:**
- `RemoteVersion` fields (`version`, `url`, `sha256`, `notes`) used identically in Tasks 2 and 5. ✓
- `UpdateOutcome` variants (`UpToDate`, `Applied`, `Failed`) match between Task 5 (def) and Task 6 (match). ✓ (Task 5 defines these three; no `Available` variant is referenced — the earlier draft mention was dropped.)
- `sha256_hex` returns lowercase hex; CI uses `sha256sum` (lowercase hex); compared with `eq_ignore_ascii_case` — consistent. ✓
- `FEED_URL` (Task 5) and `SUFeedURL` (Task 7) both derive from `R2_PUBLIC_BASE` (Task 1). ✓
- `UpdaterController.shared.checkForUpdates()` defined in Task 7, called in Task 8. ✓

**Open config values (must be filled, one place each — not code placeholders):**
- `R2_PUBLIC_BASE`: Task 5 `FEED_URL`, Task 7 Info.plist `SUFeedURL`, CI secret. Resolved in Task 1.
- `SUPublicEDKey`: Task 7 Info.plist, from Task 1 Step 3.

**Note on the minimal-crate exception:** Task 5 adds the `zip` crate (deflate only) rather than hand-rolling inflate — justified inline; it is the one deliberate departure from the no-new-crate constraint, and pulls only `flate2`.
