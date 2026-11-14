// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::*;

fn change_open_with(hwnd: HWND, use_default: bool) {
    let program = if use_default {
        None
    } else {
        match crate::windows::file_picker::pick_program(hwnd) {
            Ok(Some(path)) => Some(path),
            Ok(None) => return,
            Err(error) => {
                status(&error, true);
                return;
            }
        }
    };
    let changed = EDITOR.with(|c| {
        let mut editor = c.borrow_mut();
        let s = editor.as_mut().unwrap();
        let Some(entry) = s.draft.get_mut(s.selected) else {
            return false;
        };
        let Some(OpenTarget::Document(current)) = entry.open_target.as_mut() else {
            return false;
        };
        if *current == program {
            return false;
        }
        *current = program;
        true
    });
    if changed {
        sync_fields();
        pending_status("ui.unsaved_saving_updates_the_launcher_immediately");
    }
}

fn choose_command_target(hwnd: HWND) {
    let Some(path) = pick_path(hwnd, false, false) else {
        return;
    };
    let Ok(path) = altrun_probe::management::absolute_path(&path) else {
        return;
    };
    let field = unsafe { GetDlgItem(hwnd, COMMAND as i32) };
    let existing = text(field);
    let value = EDITOR.with(|c| {
        let state = c.borrow();
        let s = state.as_ref().unwrap();
        let entry = s.draft.get(s.selected)?;
        if entry.open_target.is_some() {
            Some(path.clone())
        } else {
            altrun_probe::launch::replace_command_target(&existing, &path)
        }
    });
    let value = match value {
        Some(value) => value,
        None if confirm(hwnd, "ui.replace_ambiguous_command") => format!("\"{path}\""),
        None => return,
    };
    unsafe {
        SetWindowTextW(field, wide(&value).as_ptr());
        SetFocus(field);
        let end = value.encode_utf16().count();
        SendMessageW(field, EM_SETSEL, end, end as isize);
    }
}

pub(super) fn tool(hwnd: HWND, id: usize) {
    if matches!(id, SORT_ORIGINAL | SORT_ALIAS | SORT_NAME) {
        EDITOR.with(|c| {
            let mut s = c.borrow_mut();
            let s = s.as_mut().unwrap();
            s.sort = id;
        });
        refresh_list_rows();
        return;
    }
    if matches!(id, MOVE_UP | MOVE_DOWN) {
        let moved = EDITOR.with(|c| {
            let mut state = c.borrow_mut();
            let s = state.as_mut().unwrap();
            if s.sort != SORT_ORIGINAL {
                return false;
            }
            let next = if id == MOVE_UP {
                s.selected.checked_sub(1)
            } else {
                Some(s.selected + 1)
            };
            if let Some(next) = next.filter(|&next| next < s.draft.len()) {
                s.draft.swap(s.selected, next);
                s.selected = next;
                true
            } else {
                false
            }
        });
        if moved {
            refresh_list_rows();
            sync_fields();
            pending_status("ui.order_changed_save_to_apply_it");
        }
        return;
    }
    if id == ADD_FILE || id == ADD_FOLDER {
        if let Some(path) = pick_path(hwnd, false, id == ADD_FOLDER) {
            add_paths(vec![path]);
        }
        return;
    }
    let (path, rows) = EDITOR.with(|c| {
        let s = c.borrow();
        let s = s.as_ref().unwrap();
        (s.file.path.clone(), s.draft.clone())
    });
    let context = TEST_CONTEXT.lock().map(|c| c.clone()).unwrap_or_default();
    let base = if context.base_directory.is_empty() {
        path.parent().unwrap_or(std::path::Path::new("."))
    } else {
        std::path::Path::new(&context.base_directory)
    };
    match id {
        VALIDATE => {
            repair::inspect(hwnd, &rows, base);
        }
        EXPORT => {
            if let Some(destination) = pick_path(hwnd, true, false) {
                if destination == path
                    || std::fs::canonicalize(&destination)
                        .ok()
                        .is_some_and(|p| Some(p) == std::fs::canonicalize(&path).ok())
                {
                    status("ui.choose_a_different_backup_file_select_save", true);
                    return;
                }
                let result = library::serialize(&rows).and_then(|text| {
                    altrun_probe::state_store::TextFile::open(&destination)
                        .and_then(|(mut file, _)| file.save(&text))
                });
                match result {
                    Ok(()) => status("ui.shortcut_draft_exported", false),
                    Err(e) => status(&e, true),
                }
            }
        }
        RESTORE => super::imports::launch_items(hwnd),
        IMPORT_BOOKMARKS => super::imports::bookmarks(hwnd),
        _ => {}
    }
}

fn test_selected(hwnd: HWND) {
    if !EDITOR.with(|c| c.borrow().as_ref().unwrap().launch_enabled) {
        status("ui.preview_mode_the_test_command_was_not", false);
        return;
    }
    let (entry, base) = EDITOR.with(|c| {
        let s = c.borrow();
        let s = s.as_ref().unwrap();
        (
            s.draft.get(s.selected).cloned(),
            s.file
                .path
                .parent()
                .unwrap_or(std::path::Path::new("."))
                .to_path_buf(),
        )
    });
    let Some(entry) = entry else {
        return;
    };
    if !can_test(&entry) {
        status("ui.the_selected_entry_has_no_command_to", true);
        return;
    }
    let mut context = TEST_CONTEXT.lock().map(|c| c.clone()).unwrap_or_default();
    if context.base_directory.is_empty() {
        context.base_directory = base.to_string_lossy().into_owned();
    }
    let needs_input = entry.param_type.needs_input(&entry.command)
        && (entry.open_target.is_some()
            || altrun_probe::launch::has_parameter(&entry.command)
            || !altrun_probe::launch::has_context(&entry.command));
    let parameter = if needs_input {
        let Some(value) = crate::ui::dialogs::input::ask(
            hwnd,
            &crate::localization::message(
                "ui.name_enter_a_test_parameter",
                &[("name", &entry.name)],
            ),
        ) else {
            return;
        };
        Some(value)
    } else {
        None
    };
    status("ui.test_launch_requested", false);
    let language = crate::localization::current();
    std::thread::spawn(move || {
        crate::localization::install_snapshot(language);
        if entry.open_target.is_none()
            && altrun_probe::launch::has_context(&entry.command)
            && entry.command.contains("%c")
        {
            context.clipboard = crate::windows::clipboard::clipboard_text();
        }
        let result = altrun_probe::launch::resolve_entry(&entry, parameter.as_deref(), &context)
            .and_then(|(request, mode, directory)| {
                crate::windows::shell_execute::execute_request_with(
                    &request,
                    mode,
                    directory.as_deref(),
                )
            });
        if let Err(error) = result {
            crate::ui::show_error(&error);
        }
    });
}

pub(super) fn prepare_backup_restore(hwnd: HWND) -> Result<bool, &'static str> {
    use crate::ui::dialogs::input::{Choice, choose};

    if unsafe { IsWindowEnabled(hwnd) } == 0 || crate::shortcut_intake::is_open() {
        return Err("backup.close_manager");
    }
    let dirty = || EDITOR.with(|c| c.borrow().as_ref().is_some_and(Editor::is_dirty));
    if dirty() {
        unsafe {
            ShowWindow(hwnd, SW_SHOWNORMAL);
            SetForegroundWindow(hwnd);
        }
        match choose(
            hwnd,
            "backup.unsaved_title",
            "backup.unsaved_manager",
            "backup.save_continue",
            "backup.discard_continue",
        ) {
            Choice::Primary => {
                save();
                if dirty() {
                    return Ok(false);
                }
            }
            Choice::Alternate => {}
            Choice::Cancel => return Ok(false),
        }
    }
    unsafe { DestroyWindow(hwnd) };
    Ok(true)
}

pub(super) fn handle_command(hwnd: HWND, w: WPARAM) -> LRESULT {
    let id = w & 0xffff;
    let notification = (w >> 16) & 0xffff;
    if id == FILTER && notification == EN_CHANGE as usize {
        let field = unsafe { GetDlgItem(hwnd, FILTER as i32) };
        let value = text(field);
        EDITOR.with(|c| c.borrow_mut().as_mut().unwrap().query = value);
        refresh_list_rows();
    } else if [ALIAS, NAME, COMMAND].contains(&id) && notification == EN_CHANGE as usize
        || id == KIND && notification == CBN_SELCHANGE as usize
        || id == ASK_PARAMETER && notification == BN_CLICKED as usize
    {
        update_draft(id);
    } else if notification == BN_CLICKED as usize {
        match id {
            CAPTION_CLOSE => unsafe {
                PostMessageW(hwnd, WM_CLOSE, 0, 0);
            },
            NEW_MENU | TOOLS_MENU => popup(hwnd, id, None),
            OPEN_WITH | OPEN_DEFAULT => change_open_with(hwnd, id == OPEN_DEFAULT),
            CHOOSE_TARGET => choose_command_target(hwnd),
            ADD => {
                EDITOR.with(|c| {
                    let mut state = c.borrow_mut();
                    let s = state.as_mut().unwrap();
                    s.draft.push(Entry {
                        alias: String::new(),
                        name: String::new(),
                        command: String::new(),
                        frequency: 0,
                        param_type: ParamType::None,
                        id: library::new_id(),
                        working_directory: String::new(),
                        open_target: None,
                        kind: library::EntryKind::Command,
                    });
                    s.selected = s.draft.len() - 1;
                });
                refresh_list();
                status("ui.enter_an_alias_and_a_command_for", false);
                let field = EDITOR.with(|c| c.borrow().as_ref().unwrap().alias);
                unsafe {
                    SetFocus(field);
                }
            }
            DELETE => {
                let name = EDITOR.with(|c| {
                    let s = c.borrow();
                    let s = s.as_ref().unwrap();
                    s.draft
                        .get(s.selected)
                        .map(|e| format!("{} · {}", e.alias, e.name))
                });
                if let Some(name) = name
                    && confirm(
                        hwnd,
                        &crate::localization::message(
                            "ui.remove_name_from_the_draft_save_to",
                            &[("name", &name)],
                        ),
                    )
                {
                    EDITOR.with(|c| {
                        let mut state = c.borrow_mut();
                        let s = state.as_mut().unwrap();
                        s.draft.remove(s.selected);
                        s.selected = s.selected.min(s.draft.len().saturating_sub(1));
                    });
                    refresh_list();
                    pending_status("ui.removed_from_the_draft_changes_are_unsaved");
                }
            }
            SAVE => save(),
            TEST => test_selected(hwnd),
            CLOSE => unsafe {
                PostMessageW(hwnd, WM_CLOSE, 0, 0);
            },
            ADD_FILE | ADD_FOLDER | VALIDATE | EXPORT | RESTORE | IMPORT_BOOKMARKS
            | SORT_ORIGINAL | SORT_ALIAS | SORT_NAME | MOVE_UP | MOVE_DOWN => tool(hwnd, id),
            _ => {}
        }
    }
    0
}
