//! settings.rs
//! Persistent settings at %APPDATA%\NovaKey\settings.json and HKCU "Run"
//! autostart. No serde dependency — the schema is a handful of booleans, so a
//! tiny hand-rolled reader/writer keeps the binary small.

use std::fs;
use std::path::PathBuf;

use crate::hotkey;

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const RUN_VALUE: &str = "NovaKey";

/// Which apps NovaKey composes Vietnamese in.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AppMode {
    /// Every app (the default).
    All,
    /// Only the apps in `apps_only`.
    Only,
    /// Every app except those in `apps_except`.
    Except,
}

impl AppMode {
    fn to_u32(self) -> u32 {
        match self {
            AppMode::All => 0,
            AppMode::Only => 1,
            AppMode::Except => 2,
        }
    }

    fn from_u32(v: u32) -> Option<AppMode> {
        match v {
            0 => Some(AppMode::All),
            1 => Some(AppMode::Only),
            2 => Some(AppMode::Except),
            _ => None,
        }
    }
}

/// The look of the Settings window.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Theme {
    /// Windows 9x: gray bevels and a gradient title bar (the default).
    Classic,
    /// Windows 3.1: white dialog, solid navy title bar, black outlines.
    Win31,
    /// Windows 10/11: flat, light, Segoe UI, blue accent.
    Flat,
    /// Mac OS 9 Platinum: soft gray bevels and a pinstriped title bar.
    Platinum,
}

impl Theme {
    /// Every theme, in the order the picker lists them.
    pub const ALL: [Theme; 4] = [Theme::Classic, Theme::Win31, Theme::Flat, Theme::Platinum];

    pub fn name(self) -> &'static str {
        match self {
            Theme::Classic => "Classic (Windows 9x)",
            Theme::Win31 => "Windows 3.1",
            Theme::Flat => "Windows 10/11 Flat",
            Theme::Platinum => "Mac OS 9 Platinum",
        }
    }

    fn to_u32(self) -> u32 {
        match self {
            Theme::Classic => 0,
            Theme::Win31 => 1,
            Theme::Flat => 2,
            Theme::Platinum => 3,
        }
    }

    fn from_u32(v: u32) -> Option<Theme> {
        match v {
            0 => Some(Theme::Classic),
            1 => Some(Theme::Win31),
            2 => Some(Theme::Flat),
            3 => Some(Theme::Platinum),
            _ => None,
        }
    }
}

/// Separator between exe names in the stored app lists. `|` cannot appear in
/// a Windows file name, so no escaping is needed.
const APP_SEP: char = '|';

#[derive(Clone, Debug)]
pub struct Settings {
    /// Whether Vietnamese mode is currently on.
    pub enabled: bool,
    /// Send each replacement char as a separate event (slower, more compatible).
    pub step_by_step: bool,
    /// Launch on Windows sign-in (mirrored into the HKCU Run key).
    pub start_with_windows: bool,
    /// Defeat browser URL-bar autocomplete with the U+202F prefix trick.
    pub fix_browser_autocomplete: bool,
    /// Play a short system beep when the language is toggled.
    pub play_sound: bool,
    /// "Quick Vietnamese": a lone `w` after an initial consonant -> ư.
    pub quick_vietnamese: bool,
    /// "Deferred diacritics" (Bỏ dấu sau): modifier keys typed later in the
    /// word apply backward ("did" -> "đi"). Sub-option of Quick Vietnamese.
    pub deferred_diacritics: bool,
    /// Language-toggle hotkey modifier bitmask (`RegisterHotKey` MOD_* flags).
    pub hotkey_mods: u32,
    /// Language-toggle hotkey virtual-key code.
    pub hotkey_vk: u32,
    /// Unix timestamp (seconds) of the last background update check.
    pub last_update_check: u64,
    /// Which apps Vietnamese composition is active in.
    pub app_mode: AppMode,
    /// Exe names (normalized, see `normalize_exe`) for `AppMode::Only`.
    pub apps_only: Vec<String>,
    /// Exe names (normalized, see `normalize_exe`) for `AppMode::Except`.
    /// Kept apart from `apps_only` so switching modes never reinterprets an
    /// allow-list as a block-list or vice versa.
    pub apps_except: Vec<String>,
    /// The Settings window's look.
    pub theme: Theme,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            enabled: true,
            step_by_step: false,
            start_with_windows: false,
            fix_browser_autocomplete: true,
            play_sound: false,
            quick_vietnamese: false,
            deferred_diacritics: false,
            hotkey_mods: hotkey::DEFAULT_MODS,
            hotkey_vk: hotkey::DEFAULT_VK,
            last_update_check: 0,
            app_mode: AppMode::All,
            apps_only: Vec::new(),
            apps_except: Vec::new(),
            theme: Theme::Classic,
        }
    }
}

/// `%APPDATA%\NovaKey\settings.json`
fn settings_path() -> Option<PathBuf> {
    let appdata = std::env::var_os("APPDATA")?;
    let mut p = PathBuf::from(appdata);
    p.push("NovaKey");
    p.push("settings.json");
    Some(p)
}

impl Settings {
    pub fn load() -> Settings {
        let mut s = Settings::default();
        if let Some(path) = settings_path() {
            if let Ok(text) = fs::read_to_string(&path) {
                s.enabled = read_bool(&text, "enabled").unwrap_or(s.enabled);
                s.step_by_step = read_bool(&text, "stepByStep").unwrap_or(s.step_by_step);
                s.start_with_windows =
                    read_bool(&text, "startWithWindows").unwrap_or(s.start_with_windows);
                s.fix_browser_autocomplete = read_bool(&text, "fixBrowserAutocomplete")
                    .unwrap_or(s.fix_browser_autocomplete);
                s.play_sound = read_bool(&text, "playSound").unwrap_or(s.play_sound);
                s.quick_vietnamese =
                    read_bool(&text, "quickVietnamese").unwrap_or(s.quick_vietnamese);
                s.deferred_diacritics =
                    read_bool(&text, "deferredDiacritics").unwrap_or(s.deferred_diacritics);
                s.hotkey_mods = read_u32(&text, "hotkeyMods").unwrap_or(s.hotkey_mods);
                s.hotkey_vk = read_u32(&text, "hotkeyVk").unwrap_or(s.hotkey_vk);
                s.last_update_check =
                    read_u64(&text, "lastUpdateCheck").unwrap_or(s.last_update_check);
                s.app_mode = read_u32(&text, "appMode")
                    .and_then(AppMode::from_u32)
                    .unwrap_or(s.app_mode);
                s.apps_only = read_str(&text, "appsOnly")
                    .map(|v| parse_app_list(&v))
                    .unwrap_or_default();
                s.apps_except = read_str(&text, "appsExcept")
                    .map(|v| parse_app_list(&v))
                    .unwrap_or_default();
                s.theme = read_u32(&text, "theme")
                    .and_then(Theme::from_u32)
                    .unwrap_or(s.theme);
            }
        }
        s.sanitize();
        s
    }

    /// Repair values a hand-edited or truncated settings file could contain.
    /// A bogus hotkey would otherwise fail to register and leave the toggle
    /// silently unbound, so fall back to the built-in default pair.
    pub fn sanitize(&mut self) {
        if !hotkey::is_valid(self.hotkey_mods, self.hotkey_vk) {
            self.hotkey_mods = hotkey::DEFAULT_MODS;
            self.hotkey_vk = hotkey::DEFAULT_VK;
        }
    }

    pub fn save(&self) {
        if let Some(path) = settings_path() {
            if let Some(dir) = path.parent() {
                let _ = fs::create_dir_all(dir);
            }
            let json = format!(
                "{{\n  \"enabled\": {},\n  \"stepByStep\": {},\n  \"startWithWindows\": {},\n  \"fixBrowserAutocomplete\": {},\n  \"playSound\": {},\n  \"quickVietnamese\": {},\n  \"deferredDiacritics\": {},\n  \"hotkeyMods\": {},\n  \"hotkeyVk\": {},\n  \"lastUpdateCheck\": {},\n  \"appMode\": {},\n  \"appsOnly\": \"{}\",\n  \"appsExcept\": \"{}\",\n  \"theme\": {}\n}}\n",
                self.enabled,
                self.step_by_step,
                self.start_with_windows,
                self.fix_browser_autocomplete,
                self.play_sound,
                self.quick_vietnamese,
                self.deferred_diacritics,
                self.hotkey_mods,
                self.hotkey_vk,
                self.last_update_check,
                self.app_mode.to_u32(),
                join_app_list(&self.apps_only),
                join_app_list(&self.apps_except),
                self.theme.to_u32()
            );
            let _ = fs::write(&path, json);
        }
    }
}

// MARK: - Per-app rules

impl Settings {
    /// The list the current mode consults (empty slice for `AppMode::All`).
    pub fn active_app_list(&self) -> &[String] {
        match self.app_mode {
            AppMode::All => &[],
            AppMode::Only => &self.apps_only,
            AppMode::Except => &self.apps_except,
        }
    }

    /// Whether Vietnamese composition should run in the app `exe` (a
    /// normalized exe name; `None` when the foreground process could not be
    /// identified, e.g. an elevated window).
    pub fn allows_app(&self, exe: Option<&str>) -> bool {
        let listed = |list: &[String]| exe.is_some_and(|e| list.iter().any(|a| a == e));
        match self.app_mode {
            AppMode::All => true,
            AppMode::Only => listed(&self.apps_only),
            AppMode::Except => !listed(&self.apps_except),
        }
    }

    /// The current mode's list, for editing (`None` in `AppMode::All`).
    fn active_app_list_mut(&mut self) -> Option<&mut Vec<String>> {
        match self.app_mode {
            AppMode::All => None,
            AppMode::Only => Some(&mut self.apps_only),
            AppMode::Except => Some(&mut self.apps_except),
        }
    }

    /// Add `exe` to the current mode's list. Returns false if the name is not
    /// a valid exe name, the list is full, or it is already listed.
    pub fn add_app(&mut self, exe: &str) -> bool {
        let Some(exe) = normalize_exe(exe) else { return false };
        let Some(list) = self.active_app_list_mut() else { return false };
        if list.len() >= MAX_APPS || list.contains(&exe) {
            return false;
        }
        list.push(exe);
        list.sort();
        true
    }

    /// Remove the entry at `index` of the current mode's list.
    pub fn remove_app_at(&mut self, index: usize) {
        if let Some(list) = self.active_app_list_mut() {
            if index < list.len() {
                list.remove(index);
            }
        }
    }

    /// Turn Vietnamese composition on (`on`) or off in `exe`, editing whichever
    /// list the current mode uses. From "All apps", turning one app off switches
    /// to "All except" with that app listed — the only way to honour the request.
    pub fn set_app_enabled(&mut self, exe: &str, on: bool) {
        let Some(exe) = normalize_exe(exe) else { return };
        if self.allows_app(Some(&exe)) == on {
            return;
        }
        if self.app_mode == AppMode::All {
            self.app_mode = AppMode::Except;
        }
        // Only-mode lists the apps that are on; Except-mode the ones that are off.
        if (self.app_mode == AppMode::Only) == on {
            self.add_app(&exe);
        } else if let Some(list) = self.active_app_list_mut() {
            list.retain(|a| *a != exe);
        }
    }
}

/// Upper bound on each app list, so a hand-edited settings file cannot make
/// the Settings window (one row per app) unboundedly tall.
pub const MAX_APPS: usize = 64;

/// Canonical form of an app identifier: the lowercase file name of an `.exe`
/// (a full path is reduced to its last component). Rejects anything that is
/// not a plausible Windows exe name — including the characters our flat JSON
/// and `APP_SEP` rely on never appearing (`"`, `|`, control characters).
pub fn normalize_exe(name: &str) -> Option<String> {
    let base = name.trim().rsplit(['\\', '/']).next()?.trim().to_lowercase();
    let valid = base.len() > 4
        && base.len() <= 255
        && base.ends_with(".exe")
        && !base
            .chars()
            .any(|c| c.is_control() || matches!(c, '"' | '|' | ':' | '*' | '?' | '<' | '>'));
    valid.then_some(base)
}

/// Parse a stored `a.exe|b.exe` list, dropping invalid and duplicate entries.
fn parse_app_list(text: &str) -> Vec<String> {
    let mut apps: Vec<String> = text.split(APP_SEP).filter_map(normalize_exe).collect();
    apps.sort();
    apps.dedup();
    apps.truncate(MAX_APPS);
    apps
}

fn join_app_list(apps: &[String]) -> String {
    apps.iter()
        .filter_map(|a| normalize_exe(a))
        .collect::<Vec<_>>()
        .join(&APP_SEP.to_string())
}

/// Extremely small `"key": true/false` extractor for our flat JSON.
fn read_bool(text: &str, key: &str) -> Option<bool> {
    let needle = format!("\"{}\"", key);
    let idx = text.find(&needle)?;
    let rest = &text[idx + needle.len()..];
    let colon = rest.find(':')?;
    let after = rest[colon + 1..].trim_start();
    if after.starts_with("true") {
        Some(true)
    } else if after.starts_with("false") {
        Some(false)
    } else {
        None
    }
}

/// Read a non-negative integer value for `"key": 123` from our flat JSON.
fn read_u32(text: &str, key: &str) -> Option<u32> {
    let needle = format!("\"{}\"", key);
    let idx = text.find(&needle)?;
    let rest = &text[idx + needle.len()..];
    let colon = rest.find(':')?;
    let digits: String = rest[colon + 1..]
        .trim_start()
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    digits.parse().ok()
}

/// Read a non-negative 64-bit integer value for `"key": 123` from our flat
/// JSON. Mirrors `read_u32` for wider values (e.g. Unix timestamps).
fn read_u64(text: &str, key: &str) -> Option<u64> {
    let needle = format!("\"{}\"", key);
    let idx = text.find(&needle)?;
    let rest = &text[idx + needle.len()..];
    let colon = rest.find(':')?;
    let digits: String = rest[colon + 1..]
        .trim_start()
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    digits.parse().ok()
}

/// Read a `"key": "value"` string from our flat JSON (no escape handling —
/// our manifests contain only plain URLs, hex, and short notes).
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

// MARK: - Autostart (HKCU Run)

/// Whether `path` lives in a temp folder — e.g. the exe was launched by
/// double-clicking it inside the downloaded zip, which Explorer extracts to
/// `%TEMP%\<guid>_NovaKey-windows.zip.xxx\`. Windows cleans those up, so a Run
/// entry pointing there stops working after a while.
fn is_temporary(path: &std::path::Path) -> bool {
    let p = path.to_string_lossy().to_lowercase();
    let temp = std::env::temp_dir().to_string_lossy().to_lowercase();
    p.contains(r"\appdata\local\temp\") || (!temp.is_empty() && p.starts_with(&temp))
}

/// The exe the Run entry should launch: this one, or — when running from a
/// temp folder — a copy installed at `%LOCALAPPDATA%\Programs\NovaKey`.
fn autostart_exe() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    if !is_temporary(&exe) {
        return Some(exe);
    }
    let mut dest = PathBuf::from(std::env::var_os("LOCALAPPDATA")?);
    dest.push("Programs");
    dest.push("NovaKey");
    let _ = fs::create_dir_all(&dest);
    dest.push("NovaKey.exe");
    match fs::copy(&exe, &dest) {
        Ok(_) => Some(dest),
        // e.g. that copy is locked: keep pointing at it if it exists at all.
        Err(_) if dest.exists() => Some(dest),
        Err(_) => Some(exe),
    }
}

/// Add or remove NovaKey from the per-user Run key.
pub fn set_autostart(enable: bool) {
    let exe = match autostart_exe() {
        Some(p) => p,
        None => return,
    };
    // Quoted: an unquoted path containing spaces (e.g. a user folder like
    // "C:\Users\Nguyen Van A\...") is ambiguous and may fail to launch.
    let exe_str = format!("\"{}\"", exe.to_string_lossy());
    unsafe {
        use windows_sys::Win32::System::Registry::{
            RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER,
            KEY_SET_VALUE, REG_SZ,
        };

        let subkey = wide(RUN_KEY);
        let value = wide(RUN_VALUE);
        let mut hkey: HKEY = std::ptr::null_mut();
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            subkey.as_ptr(),
            0,
            KEY_SET_VALUE,
            &mut hkey,
        ) != 0
        {
            return;
        }

        if enable {
            let data = wide(&exe_str);
            let bytes = data.len() * std::mem::size_of::<u16>();
            RegSetValueExW(
                hkey,
                value.as_ptr(),
                0,
                REG_SZ,
                data.as_ptr() as *const u8,
                bytes as u32,
            );
        } else {
            RegDeleteValueW(hkey, value.as_ptr());
        }
        RegCloseKey(hkey);
    }
}

/// Where Task Manager / Settings > Startup apps records a per-user Run entry
/// the user switched off. The Run value stays; Windows just skips it.
const STARTUP_APPROVED_KEY: &str =
    r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";

/// Whether Windows has our Run entry marked disabled (Startup apps). The value
/// is REG_BINARY; an odd first byte (0x03) means disabled, 0x02 enabled.
pub fn autostart_disabled_by_windows() -> bool {
    unsafe {
        use windows_sys::Win32::System::Registry::{
            RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_BINARY,
        };
        let subkey = wide(STARTUP_APPROVED_KEY);
        let value = wide(RUN_VALUE);
        let mut data = [0u8; 12];
        let mut len = data.len() as u32;
        let rc = RegGetValueW(
            HKEY_CURRENT_USER,
            subkey.as_ptr(),
            value.as_ptr(),
            RRF_RT_REG_BINARY,
            std::ptr::null_mut(),
            data.as_mut_ptr() as *mut _,
            &mut len,
        );
        rc == 0 && len > 0 && data[0] & 1 == 1
    }
}

/// Drop Windows' "disabled" mark on our Run entry; a missing value means
/// enabled. Only called when the user turns "Start with Windows" on.
pub fn clear_startup_disabled() {
    unsafe {
        use windows_sys::Win32::System::Registry::{RegDeleteKeyValueW, HKEY_CURRENT_USER};
        let subkey = wide(STARTUP_APPROVED_KEY);
        let value = wide(RUN_VALUE);
        RegDeleteKeyValueW(HKEY_CURRENT_USER, subkey.as_ptr(), value.as_ptr());
    }
}

/// UTF-16, NUL-terminated.
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_flat_json() {
        let json = "{\n  \"enabled\": false,\n  \"stepByStep\": true,\n  \"startWithWindows\": false\n}";
        assert_eq!(read_bool(json, "enabled"), Some(false));
        assert_eq!(read_bool(json, "stepByStep"), Some(true));
        assert_eq!(read_bool(json, "startWithWindows"), Some(false));
        assert_eq!(read_bool(json, "missing"), None);
    }

    #[test]
    fn round_trips_deferred_diacritics() {
        let s = Settings {
            quick_vietnamese: true,
            deferred_diacritics: true,
            ..Settings::default()
        };
        let json = format!(
            "{{\n  \"quickVietnamese\": {},\n  \"deferredDiacritics\": {}\n}}",
            s.quick_vietnamese, s.deferred_diacritics
        );
        assert_eq!(read_bool(&json, "quickVietnamese"), Some(true));
        assert_eq!(read_bool(&json, "deferredDiacritics"), Some(true));
    }

    #[test]
    fn detects_temp_locations() {
        use std::path::Path;
        assert!(is_temporary(Path::new(
            r"C:\Users\An\AppData\Local\Temp\1a2b_NovaKey-windows.zip.565\NovaKey.exe"
        )));
        assert!(!is_temporary(Path::new(
            r"C:\Users\An\AppData\Local\Programs\NovaKey\NovaKey.exe"
        )));
        assert!(!is_temporary(Path::new(r"C:\Tools\NovaKey.exe")));
    }

    #[test]
    fn parses_integers() {
        let json = "{\n  \"hotkeyMods\": 2,\n  \"hotkeyVk\": 32\n}";
        assert_eq!(read_u32(json, "hotkeyMods"), Some(2));
        assert_eq!(read_u32(json, "hotkeyVk"), Some(32));
        assert_eq!(read_u32(json, "missing"), None);
    }

    #[test]
    fn round_trips_hotkey() {
        // A saved-then-reparsed hotkey must survive intact.
        let s = Settings {
            hotkey_mods: 6,
            hotkey_vk: 0x5A,
            ..Settings::default()
        };
        let json = format!(
            "{{\n  \"hotkeyMods\": {},\n  \"hotkeyVk\": {}\n}}",
            s.hotkey_mods, s.hotkey_vk
        );
        assert_eq!(read_u32(&json, "hotkeyMods"), Some(6));
        assert_eq!(read_u32(&json, "hotkeyVk"), Some(0x5A));
    }

    #[test]
    fn sanitize_repairs_a_broken_hotkey() {
        // vk 0 never registers; a bare key would be swallowed system-wide.
        for (mods, vk) in [(0u32, 0u32), (0, 0x5A), (hotkey::MOD_SHIFT, 0x41), (0x4000, 0x41)] {
            let mut s = Settings {
                hotkey_mods: mods,
                hotkey_vk: vk,
                ..Settings::default()
            };
            s.sanitize();
            assert_eq!(s.hotkey_mods, hotkey::DEFAULT_MODS);
            assert_eq!(s.hotkey_vk, hotkey::DEFAULT_VK);
        }
    }

    #[test]
    fn sanitize_keeps_a_valid_hotkey() {
        let mut s = Settings {
            hotkey_mods: hotkey::MOD_CONTROL | hotkey::MOD_ALT,
            hotkey_vk: 0x56, // V
            ..Settings::default()
        };
        s.sanitize();
        assert_eq!(s.hotkey_mods, hotkey::MOD_CONTROL | hotkey::MOD_ALT);
        assert_eq!(s.hotkey_vk, 0x56);
    }

    #[test]
    fn normalizes_exe_names() {
        assert_eq!(normalize_exe("Zalo.exe"), Some("zalo.exe".into()));
        assert_eq!(
            normalize_exe(r"C:\Program Files\Slack\slack.EXE"),
            Some("slack.exe".into())
        );
        assert_eq!(normalize_exe("  code.exe "), Some("code.exe".into()));
        for bad in ["", ".exe", "notepad", "a\"b.exe", "a|b.exe", "a\nb.exe", "enabled"] {
            assert_eq!(normalize_exe(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn parses_app_lists() {
        let list = parse_app_list("zalo.exe|Slack.exe||bogus|zalo.exe");
        assert_eq!(list, vec!["slack.exe".to_string(), "zalo.exe".to_string()]);
        assert!(parse_app_list("").is_empty());
    }

    #[test]
    fn round_trips_app_rules_through_flat_json() {
        let s = Settings {
            app_mode: AppMode::Except,
            apps_only: vec!["zalo.exe".into()],
            apps_except: vec!["code.exe".into(), "windowsterminal.exe".into()],
            ..Settings::default()
        };
        let json = format!(
            "{{\n  \"enabled\": true,\n  \"appMode\": {},\n  \"appsOnly\": \"{}\",\n  \"appsExcept\": \"{}\"\n}}",
            s.app_mode.to_u32(),
            join_app_list(&s.apps_only),
            join_app_list(&s.apps_except)
        );
        assert_eq!(read_u32(&json, "appMode").and_then(AppMode::from_u32), Some(AppMode::Except));
        assert_eq!(parse_app_list(&read_str(&json, "appsOnly").unwrap()), s.apps_only);
        assert_eq!(parse_app_list(&read_str(&json, "appsExcept").unwrap()), s.apps_except);
        // App names never shadow a settings key.
        assert_eq!(read_bool(&json, "enabled"), Some(true));
        assert_eq!(read_str("{ \"appsOnly\": \"\" }", "appsOnly"), Some(String::new()));
    }

    #[test]
    fn round_trips_theme() {
        for theme in Theme::ALL {
            let json = format!("{{ \"appMode\": 0, \"theme\": {} }}", theme.to_u32());
            assert_eq!(read_u32(&json, "theme").and_then(Theme::from_u32), Some(theme));
        }
        // An unknown value falls back to the default rather than failing.
        assert_eq!(Theme::from_u32(9), None);
    }

    #[test]
    fn app_modes_decide_who_is_allowed() {
        let mut s = Settings {
            apps_only: vec!["zalo.exe".into()],
            apps_except: vec!["code.exe".into()],
            ..Settings::default()
        };
        assert!(s.allows_app(Some("code.exe")));
        assert!(s.allows_app(None));

        s.app_mode = AppMode::Only;
        assert!(s.allows_app(Some("zalo.exe")));
        assert!(!s.allows_app(Some("code.exe")));
        assert!(!s.allows_app(None));

        s.app_mode = AppMode::Except;
        assert!(s.allows_app(Some("zalo.exe")));
        assert!(!s.allows_app(Some("code.exe")));
        assert!(s.allows_app(None));
    }

    #[test]
    fn set_app_enabled_edits_the_right_list() {
        let mut s = Settings::default();
        // All -> turning one app off switches to Except.
        s.set_app_enabled("Code.exe", false);
        assert_eq!(s.app_mode, AppMode::Except);
        assert_eq!(s.apps_except, vec!["code.exe".to_string()]);
        assert!(!s.allows_app(Some("code.exe")));
        s.set_app_enabled("code.exe", true);
        assert!(s.apps_except.is_empty());

        s.app_mode = AppMode::Only;
        s.set_app_enabled("zalo.exe", true);
        assert_eq!(s.apps_only, vec!["zalo.exe".to_string()]);
        s.set_app_enabled("zalo.exe", true); // no duplicate
        assert_eq!(s.apps_only.len(), 1);
        s.set_app_enabled("zalo.exe", false);
        assert!(s.apps_only.is_empty());
        // The Except list was left alone throughout Only-mode edits.
        assert!(s.apps_except.is_empty());
    }

    #[test]
    fn add_and_remove_apps() {
        let mut s = Settings::default();
        assert!(!s.add_app("zalo.exe"), "All mode has no list to add to");
        s.app_mode = AppMode::Only;
        assert!(s.add_app("zalo.exe"));
        assert!(!s.add_app("ZALO.EXE"), "duplicate");
        assert!(!s.add_app("not-an-exe"));
        assert!(s.add_app("slack.exe"));
        assert_eq!(s.active_app_list(), ["slack.exe".to_string(), "zalo.exe".to_string()]);
        s.remove_app_at(0);
        assert_eq!(s.active_app_list(), ["zalo.exe".to_string()]);
        s.remove_app_at(5); // out of range: no-op
        assert_eq!(s.active_app_list().len(), 1);
        for i in 0..MAX_APPS {
            s.add_app(&format!("app{i}.exe"));
        }
        assert_eq!(s.apps_only.len(), MAX_APPS);
    }

    #[test]
    fn parses_u64_timestamp() {
        let json = "{\n  \"lastUpdateCheck\": 1750000000\n}";
        assert_eq!(read_u64(json, "lastUpdateCheck"), Some(1_750_000_000));
        assert_eq!(read_u64(json, "missing"), None);
    }
}
