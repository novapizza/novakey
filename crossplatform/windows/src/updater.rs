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
