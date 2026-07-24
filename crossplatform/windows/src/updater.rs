//! updater.rs
//! Self-update: fetch the R2 manifest, verify, swap the exe, relaunch.

use std::os::windows::ffi::OsStrExt;
use std::path::Path;

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

/// Parse "x.y.z" (ignoring a leading 'v') into a comparable tuple.
fn parse_semver(v: &str) -> Option<(u32, u32, u32)> {
    let v = v.trim().strip_prefix('v').unwrap_or(v.trim());
    let mut it = v.split('.');
    let a = it.next()?.parse().ok()?;
    let b = it.next()?.parse().ok()?;
    let c = it.next()?.parse().ok()?;
    Some((a, b, c))
}

/// True when `remote` is strictly newer than `current`.
pub fn is_newer(current: &str, remote: &str) -> bool {
    match (parse_semver(current), parse_semver(remote)) {
        (Some(c), Some(r)) => r > c,
        _ => false,
    }
}

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

/// True when `path` carries a valid Authenticode signature that chains to a
/// trusted root. Gates every downloaded exe before we launch it.
#[allow(dead_code)]
pub fn verify_authenticode(path: &Path) -> bool {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::Security::WinTrust::{
        WinVerifyTrust, WINTRUST_ACTION_GENERIC_VERIFY_V2, WINTRUST_DATA, WINTRUST_FILE_INFO,
        WTD_CHOICE_FILE, WTD_REVOKE_NONE, WTD_STATEACTION_CLOSE, WTD_STATEACTION_VERIFY,
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
            dwStateAction: WTD_STATEACTION_VERIFY,
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
        data.dwStateAction = WTD_STATEACTION_CLOSE;
        let _ = WinVerifyTrust(
            windows::Win32::Foundation::HWND(std::ptr::null_mut()),
            &mut action,
            &mut data as *mut _ as *mut core::ffi::c_void,
        );
        status == 0 // ERROR_SUCCESS
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
}
