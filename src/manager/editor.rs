// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::*;

pub(super) fn text(hwnd: HWND) -> String {
    unsafe {
        let mut buffer = vec![0; GetWindowTextLengthW(hwnd) as usize + 1];
        let n = GetWindowTextW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32);
        String::from_utf16_lossy(&buffer[..n as usize])
    }
}

pub(super) fn status(message: &str, error: bool) {
    let (hwnd, color_changed) = EDITOR.with(|c| {
        let mut state = c.borrow_mut();
        let s = state.as_mut().unwrap();
        let color_changed = s.error != error;
        s.error = error;
        (s.status, color_changed)
    });
    let message = crate::localization::tr(message);
    unsafe {
        if text(hwnd) != message {
            SetWindowTextW(hwnd, wide(&message).as_ptr());
        } else if color_changed {
            InvalidateRect(hwnd, std::ptr::null(), 1);
        }
    }
}

pub(super) fn pending_status(message: &str) {
    let dirty = EDITOR.with(|c| c.borrow().as_ref().unwrap().is_dirty());
    status(if dirty { message } else { "" }, false);
}

pub(super) fn can_test(entry: &Entry) -> bool {
    entry.kind == library::EntryKind::Command && !entry.command.trim().is_empty()
}

pub(super) fn supports_parameter(entry: &Entry) -> bool {
    entry.kind == library::EntryKind::Command
        && match entry.open_target {
            None => true,
            Some(OpenTarget::Path) => {
                altrun_probe::management::is_executable(std::path::Path::new(&entry.command))
            }
            Some(OpenTarget::Document(_) | OpenTarget::LinkArguments(_) | OpenTarget::WebUrl) => {
                false
            }
        }
}

fn parameter_choice(entry: &Entry) -> usize {
    let command = entry
        .command
        .trim_start()
        .trim_start_matches(['@', '+', '-'])
        .trim_start();
    let prefix = command.get(..8).unwrap_or(command).to_ascii_lowercase();
    let default = if prefix.starts_with("https://") || prefix.starts_with("http://") {
        ParamType::Utf8Url
    } else {
        ParamType::Raw
    };
    let visible_kind = match entry.param_type {
        ParamType::Auto if entry.param_type.needs_input(&entry.command) => ParamType::Raw,
        ParamType::Auto | ParamType::None => default,
        kind => kind,
    };
    PARAMETER_CHOICES
        .iter()
        .position(|&(kind, _)| kind == visible_kind)
        .expect("every visible parameter type is in the dropdown")
}

fn sync_parameter_controls(entry: Option<&Entry>, toggle: HWND, kind: HWND) {
    unsafe {
        let checked =
            entry.is_some_and(|e| supports_parameter(e) && e.param_type.needs_input(&e.command));
        if (SendMessageW(toggle, BM_GETCHECK, 0, 0) != 0) != checked {
            SendMessageW(toggle, BM_SETCHECK, usize::from(checked), 0);
        }
        let index = entry.map_or(0, parameter_choice);
        if SendMessageW(kind, CB_GETCURSEL, 0, 0) != index as isize {
            SendMessageW(kind, CB_SETCURSEL, index, 0);
        }
    }
}

pub(super) fn sync_fields() {
    let (alias, name, command, kind, toggle, save, delete, entry, dirty) = EDITOR.with(|c| {
        let mut state = c.borrow_mut();
        let s = state.as_mut().unwrap();
        s.loading = true;
        (
            s.alias,
            s.name,
            s.command,
            s.kind,
            s.ask_parameter,
            s.save,
            s.delete,
            s.draft.get(s.selected).cloned(),
            s.is_dirty(),
        )
    });
    unsafe {
        for hwnd in [alias, name, command, kind, toggle, delete] {
            EnableWindow(hwnd, i32::from(entry.is_some()));
        }
        for (field, value) in [
            (alias, entry.as_ref().map_or("", |e| e.alias.as_str())),
            (name, entry.as_ref().map_or("", |e| e.name.as_str())),
            (command, entry.as_ref().map_or("", |e| e.command.as_str())),
        ] {
            if text(field) != value {
                SetWindowTextW(field, wide(value).as_ptr());
            }
        }
        let program = entry.as_ref().and_then(|e| match &e.open_target {
            Some(OpenTarget::Document(Some(program))) => Some(program),
            _ => None,
        });
        let hwnd = GetParent(command);
        let label = program.map_or_else(
            || crate::localization::tr("ui.choose_app"),
            |p| altrun_probe::local_target::program_name(p).to_owned(),
        );
        SetWindowTextW(GetDlgItem(hwnd, OPEN_WITH as i32), wide(&label).as_ptr());
        sync_parameter_controls(entry.as_ref(), toggle, kind);
        EnableWindow(
            GetDlgItem(hwnd, TEST as i32),
            i32::from(entry.as_ref().is_some_and(can_test)),
        );
        EnableWindow(save, i32::from(dirty));
    }
    EDITOR.with(|c| c.borrow_mut().as_mut().unwrap().loading = false);
    sync_kind_layout();
}

pub(super) fn refresh_list_rows() {
    let (list, rows, selected, dpi, filter_hint, show_filter_hint) = EDITOR.with(|c| {
        let mut state = c.borrow_mut();
        let s = state.as_mut().unwrap();
        s.refreshing_list = true;
        s.view = view_order(&s.draft, s.sort, &s.query);
        let selected = s.view.iter().position(|&i| i == s.selected);
        (
            s.list,
            s.view
                .iter()
                .map(|&i| {
                    let entry = &s.draft[i];
                    (entry.alias.clone(), entry.name.clone())
                })
                .collect::<Vec<_>>(),
            selected,
            s.dpi,
            s.filter_hint,
            !s.query.trim().is_empty() && s.draft.get(s.selected).is_some() && selected.is_none(),
        )
    });
    table::replace_rows(list, &rows, selected, scaled(ROW_HEIGHT, dpi));
    EDITOR.with(|c| c.borrow_mut().as_mut().unwrap().refreshing_list = false);
    unsafe {
        ShowWindow(
            filter_hint,
            if show_filter_hint {
                SW_SHOWNOACTIVATE
            } else {
                SW_HIDE
            },
        );
    }
}

pub(super) fn refresh_list() {
    refresh_list_rows();
    sync_fields();
}

pub(super) fn show_selected_in_list() {
    let filter = EDITOR.with(|c| {
        let state = c.borrow();
        let s = state.as_ref().unwrap();
        let query = s.query.trim().to_lowercase();
        s.draft
            .get(s.selected)
            .is_some_and(|entry| !view::matches_filter(entry, &query))
            .then_some(s.filter)
    });
    if let Some(filter) = filter {
        unsafe {
            SetWindowTextW(filter, wide("").as_ptr());
        }
    }
    refresh_list();
}

pub(super) fn select_view_row(row: usize) -> bool {
    let filter_hint = EDITOR.with(|c| {
        let mut state = c.borrow_mut();
        let s = state.as_mut().unwrap();
        if let Some(&index) = s.view.get(row) {
            s.selected = index;
            Some(s.filter_hint)
        } else {
            None
        }
    });
    if let Some(hint) = filter_hint {
        unsafe { ShowWindow(hint, SW_HIDE) };
    }
    filter_hint.is_some()
}

pub(super) fn update_draft(changed: usize) {
    let handles = EDITOR.with(|c| {
        let s = c.borrow();
        let s = s.as_ref()?;
        if s.loading {
            None
        } else {
            Some((s.alias, s.name, s.command, s.kind, s.ask_parameter))
        }
    });
    let Some((alias, name, command, kind, toggle)) = handles else {
        return;
    };
    let (alias, name, command_text) = (text(alias), text(name), text(command));
    let selected_parameter = if changed == KIND {
        let index = unsafe { SendMessageW(kind, CB_GETCURSEL, 0, 0) } as usize;
        PARAMETER_CHOICES.get(index).map(|&(kind, _)| kind)
    } else {
        None
    };
    let asks_parameter =
        changed == ASK_PARAMETER && unsafe { SendMessageW(toggle, BM_GETCHECK, 0, 0) != 0 };
    if changed == ASK_PARAMETER
        && !asks_parameter
        && EDITOR.with(|c| {
            let state = c.borrow();
            let s = state.as_ref().unwrap();
            s.draft
                .get(s.selected)
                .is_some_and(|entry| entry.open_target.is_none())
        })
        && altrun_probe::launch::has_parameter(&command_text)
    {
        unsafe {
            SendMessageW(toggle, BM_SETCHECK, 1, 0);
        }
        status("ui.remove_the_parameter_placeholder_from_the_command", true);
        return;
    }
    let change = EDITOR.with(|c| {
        let mut state = c.borrow_mut();
        let s = state.as_mut().unwrap();
        let entry = s.draft.get_mut(s.selected)?;
        let previous = entry.clone();
        let row_changed = entry.alias != alias
            || entry.name != name
            || (!s.query.trim().is_empty() && entry.command != command_text);
        entry.name = name;
        if entry.kind == library::EntryKind::Command {
            entry.alias = alias;
            if entry.command != command_text {
                entry.command = if entry.open_target.is_some() {
                    altrun_probe::local_target::plain_path(&command_text).to_owned()
                } else {
                    command_text
                };
            }
            if !supports_parameter(entry) {
                entry.param_type = ParamType::None;
            } else if changed == ASK_PARAMETER {
                entry.param_type = if asks_parameter {
                    PARAMETER_CHOICES[parameter_choice(entry)].0
                } else {
                    ParamType::None
                };
            } else if let Some(parameter) = selected_parameter {
                entry.param_type = parameter;
            }
        }
        if *entry == previous {
            return None;
        }
        let entry = entry.clone();
        Some((s.save, s.is_dirty(), row_changed, entry))
    });
    let Some((save, dirty, row_changed, entry)) = change else {
        return;
    };
    unsafe {
        sync_parameter_controls(Some(&entry), toggle, kind);
        EnableWindow(save, i32::from(dirty));
        EnableWindow(
            GetDlgItem(GetParent(command), TEST as i32),
            i32::from(can_test(&entry)),
        );
    }
    if row_changed {
        refresh_list_rows();
    }
    sync_kind_layout();
    status(
        if dirty {
            "ui.unsaved_saving_updates_the_launcher_immediately"
        } else {
            ""
        },
        false,
    );
}

pub(super) fn save() {
    if !EDITOR.with(|c| c.borrow().as_ref().unwrap().is_dirty()) {
        return;
    }
    let duplicates = EDITOR.with(|c| {
        let s = c.borrow();
        let s = s.as_ref().unwrap();
        let mut seen = std::collections::HashSet::new();
        s.draft
            .iter()
            .filter(|e| {
                e.kind == library::EntryKind::Command
                    && !seen.insert(e.alias.to_lowercase())
                    && !s
                        .file
                        .entries
                        .iter()
                        .any(|old| old.id == e.id && old.alias == e.alias)
            })
            .map(|e| e.alias.clone())
            .collect::<Vec<_>>()
    });
    if !duplicates.is_empty()
        && !confirm(
            WINDOW.load(Ordering::Acquire) as HWND,
            &crate::localization::message(
                "ui.these_aliases_are_used_more_than_once",
                &[("aliases", &duplicates.join(", "))],
            ),
        )
    {
        return;
    }
    status("ui.saving", false);
    let result = EDITOR.with(|c| {
        let mut state = c.borrow_mut();
        let s = state.as_mut().unwrap();
        match s.file.save(&s.draft) {
            Ok(()) => Ok((s.sender.clone(), s.draft.clone(), s.parent)),
            Err(error) => Err(error),
        }
    });
    match result {
        Ok((sender, entries, parent)) => {
            sender.saved(entries);
            unsafe {
                PostMessageW(parent as HWND, UPDATED, 0, 0);
            }
            status("ui.saved_the_launcher_is_updated_and_a", false);
            sync_fields();
        }
        Err(error) => status(&error, true),
    }
}

pub(super) fn confirm(hwnd: HWND, message: &str) -> bool {
    crate::ui::dialogs::input::confirm(hwnd, "ui.manage_shortcuts", message)
}

pub(super) fn publish_selection() {
    let (parent, selected) = EDITOR.with(|c| {
        let s = c.borrow();
        let s = s.as_ref().unwrap();
        (
            s.parent,
            s.draft
                .get(s.selected)
                .map(|e| e.id.clone())
                .unwrap_or_default(),
        )
    });
    if let Ok(mut state) = WINDOW_STATE.lock() {
        state.selected = selected;
    }
    unsafe {
        PostMessageW(parent as HWND, GEOMETRY_CHANGED, 0, 0);
    }
}
