// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::*;

pub(super) fn change_open_with(hwnd: HWND, use_default: bool) {
    let program = if use_default {
        None
    } else {
        match crate::shortcut_intake::pick_program(hwnd) {
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

pub(super) fn choose_command_target(hwnd: HWND) {
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
            use crate::localization::tr;
            use altrun_probe::management::Validation;
            let mut missing = Vec::new();
            let mut present = 0;
            let mut skipped = 0;
            for (i, entry) in rows.iter().enumerate() {
                match altrun_probe::management::validate(entry, base) {
                    Validation::Missing => missing.push(i),
                    Validation::Present => present += 1,
                    Validation::Unconfirmed(_) => skipped += 1,
                    Validation::NonCommand => {}
                }
            }
            let summary = format!(
                "{} {}     {} {}     {} {}\n{}\n{}",
                tr("ui.available"),
                present,
                tr("ui.missing"),
                missing.len(),
                tr("ui.not_checked"),
                skipped,
                tr("ui.urls_system_entries_and_commands_resolved_by"),
                tr(if missing.is_empty() {
                    "ui.no_missing_local_targets_were_found"
                } else {
                    "ui.review_the_missing_entries_below_save_after"
                }),
            );
            let report = crate::input_dialog::Report {
                summary,
                rows: missing
                    .iter()
                    .map(|&i| (rows[i].alias.clone(), rows[i].name.clone()))
                    .collect(),
            };
            if crate::input_dialog::report(hwnd, "ui.check_missing_paths_2", report)
                && !missing.is_empty()
            {
                EDITOR.with(|c| {
                    let mut s = c.borrow_mut();
                    let s = s.as_mut().unwrap();
                    for i in missing.into_iter().rev() {
                        s.draft.remove(i);
                    }
                    s.selected = 0;
                });
                refresh_list();
                status("ui.missing_entries_removed_from_the_draft_save", false);
            }
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
        RESTORE => {
            if let Some(source) = pick_path(hwnd, false, false) {
                match library::import_shortcuts(&source) {
                    Ok(snapshot) => {
                        let mut message = crate::localization::tr(
                            "ui.import_count_shortcuts_and_replace_the_current",
                        )
                        .replace("{count}", &snapshot.entries.len().to_string());
                        if snapshot.ignored_lines > 0 {
                            message.push('\n');
                            message.push_str(
                                &crate::localization::tr("ui.skipped_count_blank_lines_or_notes")
                                    .replace("{count}", &snapshot.ignored_lines.to_string()),
                            );
                        }
                        if confirm(hwnd, &message) {
                            EDITOR.with(|c| {
                                let mut s = c.borrow_mut();
                                let s = s.as_mut().unwrap();
                                s.draft = snapshot.entries;
                                s.selected = 0;
                            });
                            refresh_list();
                        }
                    }
                    Err(e) => status(&e, true),
                }
            }
        }
        _ => {}
    }
}

pub(super) fn test_selected(hwnd: HWND) {
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
        let Some(value) = crate::input_dialog::ask(
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
            context.clipboard = crate::native::clipboard_text();
        }
        let result = altrun_probe::launch::resolve_entry(&entry, parameter.as_deref(), &context)
            .and_then(|(request, mode, directory)| {
                crate::native::execute_request_with(&request, mode, directory.as_deref())
            });
        if let Err(error) = result {
            crate::native::show_error(&error);
        }
    });
}
