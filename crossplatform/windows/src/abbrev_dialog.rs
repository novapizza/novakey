//! abbrev_dialog.rs
//! The abbreviation editor: a modal dialog of standard Windows controls — a
//! list of entries, two text boxes and Add/Remove/Close — opened from the
//! Settings window. Unlike Settings it is not owner-drawn: real text boxes
//! bring the caret, selection, clipboard and Vietnamese input for free.
//!
//! The dialog is built from an in-memory template (there is no resource
//! script), so the dialog manager still provides Tab navigation, Enter for
//! the default button and Esc to close. Its font follows the Settings theme.
//!
//! Like Settings, it runs on the thread that owns the hooks, so it edits the
//! hook's abbreviation table directly; every change is saved at once.

use std::cell::RefCell;

use windows::core::PCWSTR;
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
use windows::Win32::UI::WindowsAndMessaging::{
    DialogBoxIndirectParamW, EndDialog, GetDlgItem, GetWindowTextLengthW, GetWindowTextW,
    SendDlgItemMessageW, SetDlgItemTextW, SetWindowTextW, DLGTEMPLATE, LBN_SELCHANGE,
    LB_ADDSTRING, LB_ERR, LB_GETCURSEL, LB_RESETCONTENT, LB_SETCURSEL, LB_SETTABSTOPS, WM_CLOSE,
    WM_COMMAND, WM_DESTROY, WM_INITDIALOG,
};

use novakey_core::abbrev::{AbbrevError, MAX_ENTRIES, MAX_EXPANSION_LEN, MAX_KEY_LEN};

use crate::settings::{self, Theme};

// MARK: - Control ids

/// The default button: the dialog manager sends IDOK (1) for Enter.
const ID_ADD: i32 = 1;
/// The dialog manager sends IDCANCEL (2) for Esc and the caption's close box.
const ID_CLOSE: i32 = 2;
const ID_HINT: i32 = 100;
const ID_LIST: i32 = 101;
const ID_KEY_LABEL: i32 = 102;
const ID_KEY: i32 = 103;
const ID_TEXT_LABEL: i32 = 104;
const ID_TEXT: i32 = 105;
const ID_STATUS: i32 = 106;
const ID_REMOVE: i32 = 107;

/// Edit-control messages and notification (in the Controls feature, which
/// nothing else here needs).
const EM_SETSEL: u32 = 0x00B1;
const EM_LIMITTEXT: u32 = 0x00C5;
/// The text of an edit control changed.
const EN_CHANGE: u32 = 0x0300;

// MARK: - Template styles (raw values: the template is a raw word stream)

const WS_POPUP: u32 = 0x8000_0000;
const WS_CHILD: u32 = 0x4000_0000;
const WS_VISIBLE: u32 = 0x1000_0000;
const WS_CAPTION: u32 = 0x00C0_0000;
const WS_BORDER: u32 = 0x0080_0000;
const WS_SYSMENU: u32 = 0x0008_0000;
const WS_VSCROLL: u32 = 0x0020_0000;
const WS_TABSTOP: u32 = 0x0001_0000;
const DS_SETFONT: u32 = 0x40;
const DS_MODALFRAME: u32 = 0x80;
const DS_CENTER: u32 = 0x0800;
const ES_AUTOHSCROLL: u32 = 0x80;
const BS_DEFPUSHBUTTON: u32 = 0x01;
const LBS_NOTIFY: u32 = 0x01;
const LBS_USETABSTOPS: u32 = 0x80;
const LBS_NOINTEGRALHEIGHT: u32 = 0x100;
const SS_NOPREFIX: u32 = 0x80;

/// Predefined window-class atoms for dialog items.
const CLASS_BUTTON: u16 = 0x0080;
const CLASS_EDIT: u16 = 0x0081;
const CLASS_STATIC: u16 = 0x0082;
const CLASS_LISTBOX: u16 = 0x0083;

thread_local! {
    /// Keys of the list rows, in row order.
    static ROWS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

// MARK: - Entry point

/// Show the editor, modal to `owner` (the Settings window). Returns when the
/// user closes it.
pub unsafe fn open(owner: HWND) {
    let hinst = GetModuleHandleW(None).map(|m| HINSTANCE(m.0)).unwrap_or_default();
    let template = build_template();
    DialogBoxIndirectParamW(
        hinst,
        template.as_ptr() as *const DLGTEMPLATE,
        owner,
        Some(dialog_proc),
        LPARAM(0),
    );
}

/// The dialog's font: the face the Settings theme uses for body text, at the
/// classic dialog size.
fn theme_font() -> (u16, &'static str) {
    match crate::SETTINGS.with(|s| s.borrow().theme) {
        Theme::Classic => (8, "Tahoma"),
        Theme::Win31 => (8, "Microsoft Sans Serif"),
        Theme::Flat => (9, "Segoe UI"),
        Theme::Platinum => (8, "Verdana"),
    }
}

/// The dialog template, in dialog units. Returned as `u32`s so the buffer is
/// DWORD-aligned, as `DLGTEMPLATE` requires.
fn build_template() -> Vec<u32> {
    let (size, face) = theme_font();
    let mut t = Template::new(
        WS_POPUP | WS_CAPTION | WS_SYSMENU | DS_MODALFRAME | DS_SETFONT | DS_CENTER,
        (280, 202),
        "Abbreviations",
        (size, face),
    );
    let child = WS_CHILD | WS_VISIBLE;
    // Creation order is the Tab order; a label just before a text box lends
    // it its Alt+letter shortcut.
    t.item(
        child | SS_NOPREFIX,
        (7, 7, 266, 18),
        ID_HINT,
        CLASS_STATIC,
        "Type an abbreviation, then Tab, to get its expansion — in Vietnamese or English. Select a row to edit it.",
    );
    t.item(
        child | WS_BORDER | WS_VSCROLL | WS_TABSTOP | LBS_NOTIFY | LBS_USETABSTOPS
            | LBS_NOINTEGRALHEIGHT,
        (7, 28, 266, 104),
        ID_LIST,
        CLASS_LISTBOX,
        "",
    );
    t.item(child, (7, 138, 76, 9), ID_KEY_LABEL, CLASS_STATIC, "&Abbreviation:");
    t.item(
        child | WS_BORDER | WS_TABSTOP | ES_AUTOHSCROLL,
        (7, 148, 76, 14),
        ID_KEY,
        CLASS_EDIT,
        "",
    );
    t.item(child, (89, 138, 184, 9), ID_TEXT_LABEL, CLASS_STATIC, "&Expands to:");
    t.item(
        child | WS_BORDER | WS_TABSTOP | ES_AUTOHSCROLL,
        (89, 148, 184, 14),
        ID_TEXT,
        CLASS_EDIT,
        "",
    );
    t.item(child | SS_NOPREFIX, (7, 166, 266, 9), ID_STATUS, CLASS_STATIC, "");
    t.item(
        child | WS_TABSTOP | BS_DEFPUSHBUTTON,
        (7, 180, 70, 15),
        ID_ADD,
        CLASS_BUTTON,
        "&Add",
    );
    t.item(child | WS_TABSTOP, (81, 180, 54, 15), ID_REMOVE, CLASS_BUTTON, "&Remove");
    t.item(child | WS_TABSTOP, (219, 180, 54, 15), ID_CLOSE, CLASS_BUTTON, "Close");
    t.finish()
}

/// Builds a `DLGTEMPLATE` followed by its `DLGITEMTEMPLATE`s as a word stream.
struct Template {
    words: Vec<u16>,
    items: u16,
}

impl Template {
    fn new(style: u32, (cx, cy): (i16, i16), title: &str, (size, face): (u16, &str)) -> Template {
        let mut t = Template {
            words: Vec::new(),
            items: 0,
        };
        t.dword(style);
        t.dword(0); // extended style
        t.words.push(0); // item count, filled in by `finish`
        for v in [0, 0, cx, cy] {
            t.words.push(v as u16);
        }
        t.words.push(0); // no menu
        t.words.push(0); // default dialog class
        t.string(title);
        t.words.push(size); // DS_SETFONT: point size, then face name
        t.string(face);
        t
    }

    fn item(&mut self, style: u32, (x, y, cx, cy): (i16, i16, i16, i16), id: i32, class: u16, text: &str) {
        // Each item starts on a DWORD boundary.
        if self.words.len() % 2 == 1 {
            self.words.push(0);
        }
        self.dword(style);
        self.dword(0); // extended style
        for v in [x, y, cx, cy] {
            self.words.push(v as u16);
        }
        self.words.push(id as u16);
        self.words.push(0xFFFF); // predefined class follows as an atom
        self.words.push(class);
        self.string(text);
        self.words.push(0); // no creation data
        self.items += 1;
    }

    fn dword(&mut self, v: u32) {
        self.words.push(v as u16);
        self.words.push((v >> 16) as u16);
    }

    fn string(&mut self, s: &str) {
        self.words.extend(s.encode_utf16());
        self.words.push(0);
    }

    fn finish(mut self) -> Vec<u32> {
        self.words[4] = self.items; // DLGTEMPLATE::cdit
        if self.words.len() % 2 == 1 {
            self.words.push(0);
        }
        self.words
            .chunks(2)
            .map(|w| w[0] as u32 | (w[1] as u32) << 16)
            .collect()
    }
}

// MARK: - Dialog procedure

unsafe extern "system" fn dialog_proc(dlg: HWND, msg: u32, wparam: WPARAM, _lparam: LPARAM) -> isize {
    match msg {
        WM_INITDIALOG => {
            // The abbreviation box takes keys literally (no Telex), so what is
            // stored is exactly what the user will type.
            let key = item(dlg, ID_KEY);
            crate::hook::set_literal_field(Some(key.0 as isize));
            send(dlg, ID_KEY, EM_LIMITTEXT, MAX_KEY_LEN, 0);
            send(dlg, ID_TEXT, EM_LIMITTEXT, MAX_EXPANSION_LEN, 0);
            // One tab stop: the expansion column, in dialog units.
            let stops: [i32; 1] = [84];
            send(dlg, ID_LIST, LB_SETTABSTOPS, 1, stops.as_ptr() as isize);
            fill_list(dlg, None);
            let _ = SetFocus(key);
            0 // focus set ourselves
        }
        WM_COMMAND => {
            let id = (wparam.0 & 0xFFFF) as i32;
            let code = ((wparam.0 >> 16) & 0xFFFF) as u32;
            match (id, code) {
                (ID_ADD, _) => add_entry(dlg),
                (ID_REMOVE, _) => remove_entry(dlg),
                (ID_CLOSE, _) => {
                    let _ = EndDialog(dlg, 0);
                }
                (ID_LIST, LBN_SELCHANGE) => show_selected(dlg),
                (ID_KEY, EN_CHANGE) => update_add_label(dlg),
                _ => return 0,
            }
            1
        }
        WM_CLOSE => {
            let _ = EndDialog(dlg, 0);
            1
        }
        WM_DESTROY => {
            crate::hook::set_literal_field(None);
            ROWS.with(|r| r.borrow_mut().clear());
            0
        }
        _ => 0,
    }
}

// MARK: - Actions

/// Add the entry in the text boxes, or update the one with that key.
unsafe fn add_entry(dlg: HWND) {
    let key = text(dlg, ID_KEY);
    let expansion = text(dlg, ID_TEXT);
    let mut table = crate::hook::abbreviations();
    if let Err(e) = table.insert(&key, &expansion) {
        let (msg, field) = match e {
            AbbrevError::InvalidKey => (
                format!("An abbreviation is 2–{MAX_KEY_LEN} letters: no digits, spaces or punctuation."),
                ID_KEY,
            ),
            AbbrevError::InvalidExpansion => (
                format!("Type what it expands to: one line, up to {MAX_EXPANSION_LEN} characters."),
                ID_TEXT,
            ),
            AbbrevError::Full => (format!("The list is full ({MAX_ENTRIES} entries)."), ID_KEY),
        };
        set_status(dlg, &msg);
        focus_all(dlg, field);
        return;
    }
    if !commit(dlg, table) {
        return;
    }
    fill_list(dlg, Some(key.trim()));
    set_text(dlg, ID_KEY, "");
    set_text(dlg, ID_TEXT, "");
    set_status(dlg, &format!("Saved “{}”.", key.trim()));
    focus_all(dlg, ID_KEY);
}

/// Remove the selected entry, or else the one named in the abbreviation box.
unsafe fn remove_entry(dlg: HWND) {
    let key = selected_key(dlg).unwrap_or_else(|| text(dlg, ID_KEY));
    let mut table = crate::hook::abbreviations();
    if !table.remove(&key) {
        set_status(dlg, "Select an abbreviation to remove.");
        return;
    }
    if !commit(dlg, table) {
        return;
    }
    fill_list(dlg, None);
    set_text(dlg, ID_KEY, "");
    set_text(dlg, ID_TEXT, "");
    set_status(dlg, &format!("Removed “{}”.", key.trim()));
    focus_all(dlg, ID_KEY);
}

/// Save `table` and hand it to the hook. Nothing changes if the file cannot
/// be written, so what the user sees always matches what is saved.
unsafe fn commit(dlg: HWND, table: novakey_core::Abbreviations) -> bool {
    if !settings::save_abbreviations(&table) {
        set_status(dlg, "Could not save abbreviations.txt. Nothing was changed.");
        return false;
    }
    crate::hook::set_abbreviations(table);
    true
}

/// Copy the selected row into the text boxes for editing.
unsafe fn show_selected(dlg: HWND) {
    let Some(key) = selected_key(dlg) else { return };
    let expansion = crate::hook::abbreviations().get(&key).unwrap_or_default().to_string();
    set_text(dlg, ID_KEY, &key);
    set_text(dlg, ID_TEXT, &expansion);
    set_status(dlg, "");
}

/// "Add" for a new key, "Update" for one already listed.
unsafe fn update_add_label(dlg: HWND) {
    let exists = crate::hook::abbreviations().get(&text(dlg, ID_KEY)).is_some();
    set_text(dlg, ID_ADD, if exists { "&Update" } else { "&Add" });
}

/// Refill the list from the hook's table, selecting `select` if given.
unsafe fn fill_list(dlg: HWND, select: Option<&str>) {
    let table = crate::hook::abbreviations();
    send(dlg, ID_LIST, LB_RESETCONTENT, 0, 0);
    let mut rows = Vec::with_capacity(table.len());
    let mut selected = None;
    for (i, (key, expansion)) in table.iter().enumerate() {
        let line = wide(&format!("{key}\t{expansion}"));
        send(dlg, ID_LIST, LB_ADDSTRING, 0, line.as_ptr() as isize);
        if select.is_some_and(|s| s.to_lowercase() == key.to_lowercase()) {
            selected = Some(i);
        }
        rows.push(key.to_string());
    }
    ROWS.with(|r| *r.borrow_mut() = rows);
    if let Some(i) = selected {
        send(dlg, ID_LIST, LB_SETCURSEL, i, 0);
    }
    // The title carries the count.
    let title = match table.len() {
        0 => "Abbreviations".to_string(),
        n => format!("Abbreviations ({n})"),
    };
    let title = wide(&title);
    let _ = SetWindowTextW(dlg, PCWSTR(title.as_ptr()));
}

/// The key of the selected row.
unsafe fn selected_key(dlg: HWND) -> Option<String> {
    let i = send(dlg, ID_LIST, LB_GETCURSEL, 0, 0);
    if i == LB_ERR as isize || i < 0 {
        return None;
    }
    ROWS.with(|r| r.borrow().get(i as usize).cloned())
}

// MARK: - Control helpers

unsafe fn item(dlg: HWND, id: i32) -> HWND {
    GetDlgItem(dlg, id).unwrap_or_default()
}

unsafe fn send(dlg: HWND, id: i32, msg: u32, wparam: usize, lparam: isize) -> isize {
    SendDlgItemMessageW(dlg, id, msg, WPARAM(wparam), LPARAM(lparam)).0
}

unsafe fn text(dlg: HWND, id: i32) -> String {
    let hwnd = item(dlg, id);
    let len = GetWindowTextLengthW(hwnd).max(0) as usize;
    let mut buf = vec![0u16; len + 1];
    let n = GetWindowTextW(hwnd, &mut buf).max(0) as usize;
    String::from_utf16_lossy(&buf[..n])
}

unsafe fn set_text(dlg: HWND, id: i32, s: &str) {
    let w = wide(s);
    let _ = SetDlgItemTextW(dlg, id, PCWSTR(w.as_ptr()));
}

unsafe fn set_status(dlg: HWND, s: &str) {
    set_text(dlg, ID_STATUS, s);
}

/// Focus a text box with its contents selected, ready to retype.
unsafe fn focus_all(dlg: HWND, id: i32) {
    let _ = SetFocus(item(dlg, id));
    send(dlg, id, EM_SETSEL, 0, -1);
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Read the word stream back out of a finished template.
    fn words(t: &[u32]) -> Vec<u16> {
        t.iter().flat_map(|d| [*d as u16, (*d >> 16) as u16]).collect()
    }

    #[test]
    fn template_header_counts_its_items() {
        let w = words(&build_template());
        let style = w[0] as u32 | (w[1] as u32) << 16;
        assert_ne!(style & DS_SETFONT, 0);
        assert_eq!(w[4], 10); // cdit: hint, list, 2 labels, 2 edits, status, 3 buttons
        assert_eq!((w[7], w[8]), (280, 202)); // cx, cy
    }

    #[test]
    fn items_start_on_dword_boundaries() {
        let mut t = Template::new(WS_POPUP | DS_SETFONT, (10, 10), "T", (8, "Tahoma"));
        t.item(WS_CHILD, (0, 0, 1, 1), 5, CLASS_STATIC, "odd");
        let before = t.words.len();
        t.item(WS_CHILD, (0, 0, 1, 1), 6, CLASS_EDIT, "");
        // The second item's style DWORD is aligned.
        let start = (before..t.words.len()).find(|i| t.words[*i] == WS_CHILD as u16).unwrap();
        assert_eq!(start % 2, 0);
        assert_eq!(t.items, 2);
    }
}
