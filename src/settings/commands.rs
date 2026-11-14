// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::*;

pub(super) fn handle_command(hwnd: HWND, w: WPARAM, l: LPARAM) -> LRESULT {
    let id = w & 65535;
    let code = (w >> 16) as u32;
    if backup::busy() {
        return 0;
    }
    if code == BN_CLICKED && matches!(id, CREATE_BACKUP | RESTORE_BACKUP) {
        backup::open(hwnd, id == RESTORE_BACKUP);
        return 0;
    }
    if code == BN_CLICKED
        && let Some((_, _, _, url)) = PROJECT_LINKS.iter().find(|(action, _, _, _)| *action == id)
    {
        open_about_link(hwnd, url, "ui.couldn_t_open_the_link_open_the");
        return 0;
    }
    if (SEGMENT..SEGMENT + 5).contains(&id) && code == BN_CLICKED {
        unsafe {
            if id < SEGMENT + 3 {
                SendMessageW(
                    field("Theme").value,
                    CB_SETCURSEL,
                    [2, 0, 1][id - SEGMENT],
                    0,
                );
                for segment in SEGMENT..SEGMENT + 3 {
                    InvalidateRect(GetDlgItem(hwnd, segment as i32), std::ptr::null(), 0);
                }
            } else {
                if background_image() == (id == SEGMENT + 4) {
                    return 0;
                }
                SendMessageW(
                    field("ShowSkin").value,
                    BM_SETCHECK,
                    usize::from(id == SEGMENT + 4),
                    0,
                );
                update_background_mode();
            }
        }
        update_preview();
        return 0;
    }
    if (SOLID_DEFAULT..=SOLID_PICK).contains(&id) && code == BN_CLICKED {
        if id == SOLID_PICK {
            choose_colour(hwnd, "TitleColor");
        } else {
            set_colour("TitleColor", &preset(id - SOLID_DEFAULT));
        }
        return 0;
    }
    if code == ui::NUMBER_STEP {
        adjust_number(hwnd, id, l as i32);
        return 0;
    }
    if id == STRENGTH_VALUE && code == EN_CHANGE {
        if EDITOR.with(|c| c.borrow().as_ref().is_some_and(|s| !s.loading_form))
            && let Ok(value) = text(l as HWND).parse::<i32>()
            && (0..=100).contains(&value)
        {
            let slider = field("BGStrength").value;
            unsafe {
                if SendMessageW(slider, TBM_GETPOS, 0, 0) != value as isize {
                    SendMessageW(slider, TBM_SETPOS, 1, value as isize);
                    update_preview();
                }
            }
        }
        return 0;
    }
    if id == MORE_COLORS && code == BN_CLICKED {
        toggle_extra_colors(hwnd);
        return 0;
    }
    if id == FONT_FAMILY && code == BN_CLICKED {
        choose_panel_font(hwnd);
        return 0;
    }
    if id == TAB && code == LBN_SELCHANGE {
        let page = unsafe { SendMessageW(l as HWND, LB_GETCURSEL, 0, 0) };
        if EDITOR.with(|c| c.borrow().as_ref().unwrap().page) != page as usize {
            show_page(page as usize);
        }
        return 0;
    }
    if id == PREVIEW_QUERY && code == EN_CHANGE {
        let preview = EDITOR.with(|c| {
            c.borrow_mut().as_mut().map(|e| {
                e.preview.selected = 0;
                e.preview_controls[1]
            })
        });
        if let Some(preview) = preview {
            align_preview();
            unsafe {
                InvalidateRect(preview, std::ptr::null(), 0);
            }
        }
        return 0;
    }
    if id == PREVIEW && code == STN_CLICKED {
        let canvas = l as HWND;
        let input = unsafe { GetDlgItem(hwnd, PREVIEW_QUERY as i32) };
        let query = text(input);
        let mut cursor = POINT::default();
        if unsafe { GetCursorPos(&mut cursor) != 0 && ScreenToClient(canvas, &mut cursor) != 0 } {
            let selected = EDITOR.with(|c| {
                let mut state = c.borrow_mut();
                let Some(s) = state.as_mut() else {
                    return false;
                };
                let count = s.preview.matches(&query).len();
                let (panel, input_height, row_height) = s.preview.geometry(count);
                let px = |n| scaled(n, s.dpi);
                let top = panel.y + input_height + 39;
                if cursor.x < px(panel.x + 9) || cursor.x >= px(panel.x + panel.w - 9) {
                    return false;
                }
                if let Some(index) = (0..count).find(|i| {
                    let y = top + *i as i32 * row_height;
                    cursor.y >= px(y) && cursor.y < px(y + row_height)
                }) {
                    s.preview.selected = index;
                    return true;
                }
                false
            });
            if selected {
                unsafe {
                    SetFocus(input);
                    InvalidateRect(canvas, std::ptr::null(), 0);
                }
            }
        }
        return 0;
    }
    if let Some(index) = id.checked_sub(FIELD).filter(|i| *i < SPECS.len()) {
        let spec = &SPECS[index];
        let changed = if switch_field(spec) {
            code == BN_CLICKED
        } else if spec.key == "Lang" || !choices(spec.key).is_empty() {
            code == CBN_SELCHANGE
        } else if matches!(spec.kind, Kind::Font | Kind::Number(..)) {
            code == EN_CHANGE
        } else {
            code == EN_KILLFOCUS
        };
        if changed && EDITOR.with(|c| c.borrow().as_ref().is_some_and(|s| !s.loading_form)) {
            if spec.key == "PlayPopupNotify" {
                let checked = unsafe { SendMessageW(l as HWND, BM_GETCHECK, 0, 0) != 0 };
                sync_sound_picker(checked);
            }
            update_preview();
        }
        return 0;
    }
    if let Some(index) = id.checked_sub(PICK).filter(|index| *index < SPECS.len()) {
        let spec = &SPECS[index];
        stop_capture();
        if spec.key.ends_with("Color") {
            choose_colour(hwnd, spec.key);
        } else {
            let value = fields()[index].value;
            if let Some(chosen) = pickers::choose(hwnd, spec.key, &text(value)) {
                unsafe {
                    SetWindowTextW(value, wide(&chosen).as_ptr());
                }
                if spec.key == "BGFileName" {
                    let skin = SPECS
                        .iter()
                        .position(|spec| spec.key == "ShowSkin")
                        .unwrap();
                    unsafe {
                        SendMessageW(fields()[skin].value, BM_SETCHECK, BST_CHECKED as usize, 0);
                    }
                    update_background_mode();
                }
                update_preview();
            }
        }
        return 0;
    }
    if let Some(index) = id.checked_sub(CLEAR).filter(|index| *index < SPECS.len()) {
        stop_capture();
        unsafe {
            SetWindowTextW(fields()[index].value, wide("").as_ptr());
        }
        status("ui.shortcut_cleared_select_save_and_apply");
        return 0;
    }
    match id {
        2310 => {
            stop_capture();
            apply();
        }
        2311 => {
            stop_capture();
            if crate::ui::dialogs::input::confirm(
                hwnd,
                "ui.reset_defaults",
                "ui.load_default_settings_save_to_apply_them",
            ) {
                load_form(&Preferences::default());
                update_preview();
                sync_save_button(hwnd);
                status("ui.defaults_loaded_save_to_apply_them");
            }
        }
        2312 => {
            stop_capture();
            if let Some(path) = choose_file(false) {
                let result = std::fs::read(&path)
                    .map_err(|e| e.to_string())
                    .and_then(|b| {
                        altrun_probe::library::decode(&b).map_err(|_| {
                            altrun_probe::preferences::SETTINGS_IMPORT_FORMAT_ERROR.to_owned()
                        })
                    })
                    .and_then(|t| Preferences::import(&t));
                match result {
                    Ok(mut prefs) => {
                        prefs.set("AutoRun", "0").unwrap();
                        prefs.set("AddToSendTo", "0").unwrap();
                        load_form(&prefs);
                        update_preview();
                        sync_save_button(hwnd);
                        status("ui.settings_imported_review_system_integration_options_and");
                    }
                    Err(e) => status(&e),
                }
            }
        }
        2313 => {
            stop_capture();
            if let Some(prefs) = validated_form()
                && let Some(path) = choose_file(true)
            {
                let source = EDITOR.with(|c| c.borrow().as_ref().unwrap().file.path.clone());
                if path == source
                    || std::fs::canonicalize(&path)
                        .ok()
                        .is_some_and(|p| Some(p) == std::fs::canonicalize(&source).ok())
                {
                    status("ui.choose_a_different_file_for_export");
                    return 0;
                }
                match TextFile::open(&path).and_then(|(mut file, _)| file.save(&prefs.text())) {
                    Ok(()) => status("ui.settings_exported"),
                    Err(e) => status(&e),
                }
            }
        }
        2314 | TITLE_CLOSE => unsafe {
            SendMessageW(hwnd, WM_CLOSE, 0, 0);
        },
        _ => {}
    }
    0
}
