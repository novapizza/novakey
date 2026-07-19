//! settings.rs
//! Persistent settings at %APPDATA%\NovaKey\settings.json and HKCU "Run"
//! autostart. No serde dependency — the schema is a handful of booleans, so a
//! tiny hand-rolled reader/writer keeps the binary small.

use std::fs;
use std::path::PathBuf;

use crate::hotkey;

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const RUN_VALUE: &str = "NovaKey";

#[derive(Clone, Copy, Debug)]
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
    /// Language-toggle hotkey modifier bitmask (`RegisterHotKey` MOD_* flags).
    pub hotkey_mods: u32,
    /// Language-toggle hotkey virtual-key code.
    pub hotkey_vk: u32,
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
            hotkey_mods: hotkey::DEFAULT_MODS,
            hotkey_vk: hotkey::DEFAULT_VK,
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
                s.hotkey_mods = read_u32(&text, "hotkeyMods").unwrap_or(s.hotkey_mods);
                s.hotkey_vk = read_u32(&text, "hotkeyVk").unwrap_or(s.hotkey_vk);
            }
        }
        s
    }

    pub fn save(&self) {
        if let Some(path) = settings_path() {
            if let Some(dir) = path.parent() {
                let _ = fs::create_dir_all(dir);
            }
            let json = format!(
                "{{\n  \"enabled\": {},\n  \"stepByStep\": {},\n  \"startWithWindows\": {},\n  \"fixBrowserAutocomplete\": {},\n  \"playSound\": {},\n  \"quickVietnamese\": {},\n  \"hotkeyMods\": {},\n  \"hotkeyVk\": {}\n}}\n",
                self.enabled,
                self.step_by_step,
                self.start_with_windows,
                self.fix_browser_autocomplete,
                self.play_sound,
                self.quick_vietnamese,
                self.hotkey_mods,
                self.hotkey_vk
            );
            let _ = fs::write(&path, json);
        }
    }
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

// MARK: - Autostart (HKCU Run)

/// Add or remove the current executable from the per-user Run key.
pub fn set_autostart(enable: bool) {
    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(_) => return,
    };
    let exe_str = exe.to_string_lossy().to_string();
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
}
