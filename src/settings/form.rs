// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::*;

pub(super) fn text(hwnd: HWND) -> String {
    unsafe {
        let mut buffer = vec![0; GetWindowTextLengthW(hwnd) as usize + 1];
        let len = GetWindowTextW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32);
        String::from_utf16_lossy(&buffer[..len as usize])
    }
}

pub(super) fn status(message: &str) {
    let key = message;
    let hwnd = EDITOR.with(|c| c.borrow().as_ref().unwrap().status);
    unsafe {
        let message = crate::localization::tr(message);
        if text(hwnd) != message {
            SetWindowTextW(hwnd, wide(&message).as_ptr());
            InvalidateRect(hwnd, std::ptr::null(), 0);
        }
    }
    ui::language::caption(hwnd, key);
}

pub(super) fn fields() -> Vec<Field> {
    EDITOR.with(|c| c.borrow().as_ref().unwrap().fields.clone())
}

pub(super) fn field(key: &str) -> Field {
    EDITOR.with(|c| c.borrow().as_ref().unwrap().fields[spec_index(key)])
}

pub(super) fn spec_index(key: &str) -> usize {
    SPECS.iter().position(|spec| spec.key == key).unwrap()
}

pub(super) fn load_form(prefs: &Preferences) {
    EDITOR.with(|c| {
        let mut state = c.borrow_mut();
        let s = state.as_mut().unwrap();
        s.draft = prefs.clone();
        s.loading_form = true;
    });
    for index in 0..SPECS.len() {
        load_field(index, prefs);
    }
    refresh_related_controls(prefs);
    EDITOR.with(|c| c.borrow_mut().as_mut().unwrap().loading_form = false);
}

pub(super) fn restore_fields(old: &Preferences, new: &Preferences) {
    EDITOR.with(|c| c.borrow_mut().as_mut().unwrap().loading_form = true);
    for (index, spec) in SPECS.iter().enumerate() {
        if old.get(spec.key) != new.get(spec.key) {
            load_field(index, new);
        }
    }
    refresh_related_controls(new);
    EDITOR.with(|c| c.borrow_mut().as_mut().unwrap().loading_form = false);
}

fn load_field(index: usize, prefs: &Preferences) {
    let field = EDITOR.with(|c| c.borrow().as_ref().unwrap().fields[index]);
    let spec = &SPECS[index];
    if field.value.is_null() {
        return;
    }
    let value = prefs.get(spec.key);
    unsafe {
        if spec.key == "Lang" {
            load_language_choice(field.value, &value);
        } else if spec.key == "BGStrength" {
            let strength = prefs.number("BGStrength");
            if SendMessageW(field.value, TBM_GETPOS, 0, 0) != strength as isize {
                SendMessageW(field.value, TBM_SETPOS, 1, strength as isize);
            }
            strength_label(field, strength);
        } else if matches!(spec.kind, Kind::Font) {
            if let Ok(font) = altrun_probe::preferences::Font::parse(&value) {
                let size = font.size_dip().to_string();
                if text(field.value) != size {
                    SetWindowTextW(field.value, wide(&size).as_ptr());
                }
            }
        } else if switch_field(spec) {
            SendMessageW(
                field.value,
                BM_SETCHECK,
                usize::from(matches!(value.as_str(), "1" | "remember")),
                0,
            );
        } else if let Some(index) = choices(spec.key).iter().position(|(key, _)| *key == value) {
            SendMessageW(field.value, CB_SETCURSEL, index, 0);
        } else {
            if text(field.value) != value {
                SetWindowTextW(field.value, wide(&value).as_ptr());
            }
            if spec.key.ends_with("Color") {
                InvalidateRect(field.pick, std::ptr::null(), 0);
            }
        }
    }
}

fn refresh_related_controls(prefs: &Preferences) {
    refresh_font_family(prefs);
    sync_sound_picker(prefs.flag("PlayPopupNotify"));
    let window = EDITOR.with(|c| c.borrow().as_ref().unwrap().hwnd);
    unsafe {
        for id in (SEGMENT..SEGMENT + 5).chain(SOLID_DEFAULT..=SOLID_PICK) {
            InvalidateRect(GetDlgItem(window, id as i32), std::ptr::null(), 0);
        }
    }
    update_background_mode();
}

pub(super) fn load_language_choice(combo: HWND, selected: &str) {
    let mut languages = localization::options().0;
    let index = languages
        .iter()
        .position(|(code, _)| code == selected)
        .unwrap_or_else(|| {
            let label =
                localization::message("language.unavailable_option", &[("language", selected)]);
            languages.push((selected.to_owned(), label));
            languages.len() - 1
        });
    unsafe {
        SendMessageW(combo, CB_RESETCONTENT, 0, 0);
        for (_, name) in &languages {
            SendMessageW(combo, CB_ADDSTRING, 0, wide(name).as_ptr() as isize);
        }
        SendMessageW(combo, CB_SETCURSEL, index, 0);
    }
    EDITOR.with(|c| c.borrow_mut().as_mut().unwrap().languages = languages);
}

pub(super) fn sync_sound_picker(enabled: bool) {
    if let Some(index) = SPECS.iter().position(|spec| spec.key == "PopupFileName") {
        unsafe {
            EnableWindow(fields()[index].pick, i32::from(enabled));
        }
    }
}

pub(super) fn draft() -> Preferences {
    EDITOR.with(|c| c.borrow().as_ref().unwrap().draft.clone())
}

fn read_field(index: usize, prefs: &mut Preferences) -> Result<(), String> {
    let spec = &SPECS[index];
    let field = EDITOR.with(|c| c.borrow().as_ref().unwrap().fields[index]);
    let value = if matches!(spec.kind, Kind::Font) {
        altrun_probe::preferences::Font::resized(&prefs.get(spec.key), &text(field.value))?
    } else if spec.key == "BGStrength" {
        text(unsafe { GetDlgItem(GetParent(field.value), STRENGTH_VALUE as i32) })
    } else if switch_field(spec) {
        let checked = unsafe { SendMessageW(field.value, BM_GETCHECK, 0, 0) != 0 };
        match (spec.key, checked) {
            ("PositionMode", true) => "remember",
            ("PositionMode", false) => "center",
            (_, true) => "1",
            (_, false) => "0",
        }
        .to_owned()
    } else if spec.key == "Lang" {
        let selected = unsafe { SendMessageW(field.value, CB_GETCURSEL, 0, 0) };
        EDITOR
            .with(|c| {
                c.borrow()
                    .as_ref()
                    .unwrap()
                    .languages
                    .get(selected as usize)
                    .map(|(code, _)| code.clone())
            })
            .ok_or_else(|| "ui.select_a_value".to_owned())?
    } else if !choices(spec.key).is_empty() {
        let selected = unsafe { SendMessageW(field.value, CB_GETCURSEL, 0, 0) };
        choices(spec.key)
            .get(selected as usize)
            .map(|(value, _)| (*value).to_owned())
            .ok_or_else(|| "ui.select_a_value".to_owned())?
    } else {
        text(field.value)
    };
    prefs.set(spec.key, &value)
}

pub(super) fn commit_fields(keys: &[&str]) {
    if !EDITOR.with(|c| c.borrow().as_ref().is_some_and(|s| !s.loading_form)) {
        return;
    }
    let old = draft();
    let mut prefs = old.clone();
    for key in keys {
        if let Err(error) = read_field(spec_index(key), &mut prefs) {
            for key in keys {
                load_field(spec_index(key), &old);
            }
            status(&error);
            return;
        }
    }
    for &key in keys {
        let index = spec_index(key);
        let spec = &SPECS[index];
        if matches!(spec.kind, Kind::Font | Kind::Number(..)) {
            load_field(index, &prefs);
        }
    }
    if old != prefs {
        super::apply::submit(prefs);
        update_preview();
    }
}

pub(super) fn edit_key(hwnd: HWND, control: HWND) -> Option<&'static str> {
    if control.is_null() || unsafe { GetParent(control) } != hwnd {
        return None;
    }
    let id = unsafe { GetDlgCtrlID(control) } as usize;
    if id == STRENGTH_VALUE {
        return Some("BGStrength");
    }
    let spec = SPECS.get(id.checked_sub(FIELD)?)?;
    (matches!(spec.kind, Kind::Font | Kind::Number(..)) && spec.key != "BGStrength")
        .then_some(spec.key)
}

pub(super) fn finish_edit(hwnd: HWND) {
    if let Some(key) = edit_key(hwnd, unsafe { GetFocus() }) {
        commit_fields(&[key]);
    }
}

pub(super) fn report_language_packs(prefs: &Preferences) {
    let (choices, errors) = localization::options();
    let requested = prefs.get("Lang");
    let mut messages = Vec::new();
    if !errors.is_empty() {
        messages.push(localization::message(
            "language.load_failed",
            &[("file", &errors.join(", "))],
        ));
    }
    if !choices.iter().any(|(code, _)| code == &requested) {
        messages.push(localization::message(
            "language.unavailable_fallback",
            &[("locale", &requested)],
        ));
    }
    if !messages.is_empty() {
        status(&messages.join("\n"));
    }
}
