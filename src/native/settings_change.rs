// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

//! Update the resident launcher on its owning thread. A pending transaction keeps
//! the old hotkeys registered until the writer confirms the new settings on disk.
use super::*;
use altrun_probe::preferences::Preferences;

fn changed(a: &Preferences, b: &Preferences, keys: &[&str]) -> bool {
    keys.iter()
        .any(|key| a.values.get(*key) != b.values.get(*key))
}

// These preferences take effect together with their hotkey reservation or OS integration.
const RESERVED: &[&str] = &[
    "HotKey1",
    "HotKey2",
    "LastItemHotKey",
    "ShowTrayIcon",
    "AutoRun",
    "AddToSendTo",
];

pub(super) fn live(fields: Vec<(&'static str, String)>) {
    let (hwnd, old, next) = STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        let s = state.as_mut().unwrap();
        let old = s.preferences.clone();
        for (key, value) in fields {
            if !RESERVED.contains(&key) {
                s.preferences.values.insert(key.into(), value);
            }
        }
        (s.hwnd, old, s.preferences.clone())
    });
    if old != next {
        update(hwnd, &old, &next);
    }
}

fn with_reservations(current: &Preferences, requested: &Preferences) -> Preferences {
    let mut next = current.clone();
    for &key in RESERVED {
        next.values.insert(key.to_owned(), requested.get(key));
    }
    next
}

pub(super) fn prepare(token: u64, next: Preferences) -> Result<(), String> {
    let (hwnd, old, previous) = STATE.with(|cell| {
        let state = cell.borrow();
        let s = state.as_ref().unwrap();
        if s.pending_settings.is_some() || RESTORING_BACKUP.get() {
            return Err("ui.another_settings_change_is_in_progress".to_owned());
        }
        Ok((s.hwnd, s.hotkeys.clone(), s.preferences.clone()))
    })?;
    let next = with_reservations(&previous, &next);
    let keys = if changed(
        &previous,
        &next,
        &["HotKey1", "HotKey2", "LastItemHotKey", "ShowTrayIcon"],
    ) {
        register_bindings(hwnd, &next, &old, BindingUpdate::Settings(&previous))?
    } else {
        old.clone()
    };
    STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        let s = state.as_mut().unwrap();
        s.preferences = next.clone();
        s.hotkeys = keys;
        s.pending_settings = Some((token, previous.clone(), old));
    });
    update(hwnd, &previous, &next);
    Ok(())
}

pub(super) fn finish(token: u64, success: bool) {
    let rollback = STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        let s = state.as_mut().unwrap();
        if s.pending_settings.as_ref()?.0 != token {
            return None;
        }
        let (_, previous, mut obsolete) = s.pending_settings.take()?;
        let rollback = if success {
            None
        } else {
            std::mem::swap(&mut s.hotkeys, &mut obsolete);
            let restored = with_reservations(&s.preferences, &previous);
            Some((
                s.hwnd,
                std::mem::replace(&mut s.preferences, restored.clone()),
                restored,
            ))
        };
        for (id, value) in obsolete {
            if value.is_some() && !s.hotkeys.iter().any(|key| key.0 == id) {
                unsafe {
                    UnregisterHotKey(s.hwnd, id);
                }
            }
        }
        rollback
    });
    if let Some((hwnd, old, next)) = rollback {
        update(hwnd, &old, &next);
    }
}

fn update(hwnd: HWND, old: &Preferences, next: &Preferences) {
    let any = |keys: &[&str]| changed(old, next, keys);
    if any(&["Theme", "MenuFont"]) {
        theme::publish(next);
    }
    if any(&["Lang"]) {
        crate::ui::language::publish(&next.get("Lang"));
    }
    if any(&["WorkingDirectory", "AllowDynamicTarget", "EnglishSearch"]) {
        STATE.with(|cell| {
            let mut state = cell.borrow_mut();
            let s = state.as_mut().unwrap();
            (
                s.context.base_directory,
                s.context.base_is_working_directory,
            ) = working_directory(next, &s.data_path);
            s.context.allow_dynamic_target = next.flag("AllowDynamicTarget");
            crate::manager::configure_context(s.context.clone(), next.flag("EnglishSearch"));
        });
    }
    if any(&["EnglishSearch"]) {
        update_search_ime(true);
    }

    let fonts = any(&["TitleFont", "KeywordFont", "ListFont"]);
    if fonts {
        apply_appearance();
    } else {
        if any(&[
            "TitleColor",
            "KeywordColor",
            "ListColor",
            "SelectionColor",
            "TextColor",
            "UseCustomTextColor",
            "ResultDividerColor",
            "VerticalResultDividers",
            "HorizontalResultDividers",
            "ShowSkin",
            "BGFileName",
            "BGStrength",
            "BGCrop",
        ]) {
            panel::refresh_colours();
            unsafe {
                RedrawWindow(
                    hwnd,
                    std::ptr::null(),
                    null_mut(),
                    RDW_INVALIDATE | RDW_ALLCHILDREN,
                );
            }
        }
        if any(&["ShowTrayIcon", "HotKey1", "HotKey2", "LastItemHotKey"]) {
            panel::sync_tray(hwnd, next);
        }
        let (edit, list, command, dpi) = STATE.with(|c| {
            let state = c.borrow();
            let s = state.as_ref().unwrap();
            (s.edit, s.list, s.command, s.dpi)
        });
        unsafe {
            if any(&["RowHeight"]) {
                SendMessageW(
                    list,
                    LB_SETITEMHEIGHT,
                    0,
                    scaled(next.number("RowHeight"), dpi) as isize,
                );
            }
            if any(&["ShowCommandLine"]) {
                ShowWindow(
                    command,
                    if next.flag("ShowCommandLine") {
                        SW_SHOWNOACTIVATE
                    } else {
                        SW_HIDE
                    },
                );
            }
            if any(&["ShowOperationHint"]) {
                SendMessageW(
                    edit,
                    EM_SETCUEBANNER,
                    0,
                    search_hint(next.flag("ShowOperationHint")).as_ptr() as isize,
                );
            }
        }
    }
    if any(&["MatchAnywhere", "RememberFavouratMatch"]) {
        refresh();
    } else if fonts
        || any(&[
            "RowHeight",
            "FormWidth",
            "MaxRows",
            "ShowOperationHint",
            "ShowCommandLine",
        ])
    {
        panel::render_results(false);
    }
}
