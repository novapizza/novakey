//! updater.rs
//! Self-update: fetch the R2 manifest, verify, swap the exe, relaunch.

use std::os::windows::ffi::OsStrExt;
use std::path::Path;

use crate::settings;

/// Fixed manifest URL.
// TODO(release): set R2 public host before shipping
pub const FEED_URL: &str = "https://REPLACE_WITH_R2_PUBLIC_BASE/latest/latest.json";

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

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Fetch a whole https URL into a Vec<u8>. Returns None on any failure.
fn http_get_bytes(url: &str) -> Option<Vec<u8>> {
    use windows::core::PCWSTR;
    use windows::Win32::Networking::WinHttp::{
        WinHttpCloseHandle, WinHttpConnect, WinHttpCrackUrl, WinHttpOpen, WinHttpOpenRequest,
        WinHttpQueryDataAvailable, WinHttpReadData, WinHttpReceiveResponse, WinHttpSendRequest,
        URL_COMPONENTS, WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, WINHTTP_FLAG_SECURE,
    };

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
        let mut ok = true;
        loop {
            let mut avail: u32 = 0;
            if WinHttpQueryDataAvailable(req, &mut avail).is_err() {
                ok = false;
                break;
            }
            if avail == 0 {
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
                ok = false;
                break;
            }
            buf.truncate(read as usize);
            out.extend_from_slice(&buf);
        }
        cleanup(req, conn, session);
        if ok {
            Some(out)
        } else {
            None
        }
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

/// Extract the single `NovaKey.exe` entry from the release zip via the `zip`
/// crate (see the Cargo.toml comment for why this is the one exception to
/// the minimal-crate rule).
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
}
