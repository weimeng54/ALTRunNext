// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::*;

pub(super) fn launch_selected() {
    let (enabled, allow, busy, selected, parameter_mode, hwnd, edit, title) = STATE.with(|c| {
        let s = c.borrow();
        let s = s.as_ref().unwrap();
        (
            s.enabled,
            s.allow_launch,
            s.launching.is_some(),
            s.parameter_entry
                .or_else(|| s.hits.get(s.selected).copied()),
            s.parameter_entry.is_some(),
            s.hwnd,
            s.edit,
            s.title,
        )
    });
    if busy {
        return;
    }
    let Some(index) = selected else {
        let query = STATE.with(|c| c.borrow().as_ref().unwrap().current_query.clone());
        if enabled
            && !query.is_empty()
            && unsafe {
                MessageBoxW(
                    hwnd,
                    ui("ui.no_matches_add_a_new_shortcut").as_ptr(),
                    ui("ui.new_shortcut").as_ptr(),
                    MB_OKCANCEL | MB_ICONQUESTION,
                )
            } == IDOK
        {
            crate::manager::enqueue(Vec::new(), Some((query, VK_INSERT)));
            open_manager();
        }
        return;
    };
    let entry = STATE.with(|c| c.borrow().as_ref().unwrap().entries[index].clone());
    if !enabled {
        if allow
            && entry.command == "notepad.exe"
            && let Ok(path) = altrun_probe::safe_notepad_path()
        {
            let _ = std::process::Command::new(path).spawn();
        }
        unsafe {
            ShowWindow(hwnd, SW_HIDE);
        }
        return;
    }
    if entry.param_type.needs_input(&entry.command)
        && !parameter_mode
        && (entry.open_target.is_some()
            || altrun_probe::launch::has_parameter(&entry.command)
            || !altrun_probe::launch::has_context(&entry.command))
    {
        let query = edit_text(edit);
        let list = STATE.with(|c| c.borrow().as_ref().unwrap().list);
        STATE.with(|c| {
            let mut s = c.borrow_mut();
            let s = s.as_mut().unwrap();
            s.saved_query = query;
            s.parameter_entry = Some(index);
            s.history_position = usize::MAX;
        });
        update_search_ime(false);
        unsafe {
            EnableWindow(list, 0);
            SetWindowTextW(
                title,
                wide(&crate::localization::message(
                    "ui.name_enter_a_parameter",
                    &[("name", &entry.name)],
                ))
                .as_ptr(),
            );
            SetWindowTextW(edit, wide("").as_ptr());
            SendMessageW(
                edit,
                EM_SETCUEBANNER,
                0,
                ui("ui.enter_a_parameter_enter_to_launch_esc").as_ptr() as LPARAM,
            );
        }
        return;
    }
    let parameter = if parameter_mode {
        Some(edit_text(edit))
    } else {
        None
    };
    let result_sender = LAUNCH_SENDER.with(|s| s.borrow().as_ref().unwrap().clone());
    let query = if parameter_mode {
        STATE.with(|c| c.borrow().as_ref().unwrap().saved_query.clone())
    } else {
        STATE.with(|c| c.borrow().as_ref().unwrap().current_query.clone())
    };
    let mut context = STATE.with(|c| c.borrow().as_ref().unwrap().context.clone());
    STATE.with(|c| {
        let mut state = c.borrow_mut();
        let s = state.as_mut().unwrap();
        s.successful = Some((entry.id.clone(), query, parameter.clone()));
        s.launching = Some(s.generation);
    });
    unsafe {
        ShowWindow(hwnd, SW_HIDE);
    }
    let owner = hwnd as usize;
    let language = crate::localization::current();
    // All path checks, environment expansion, COM initialization and shell work
    // happen off the resident UI thread. No implicit cmd.exe is introduced.
    std::thread::spawn(move || {
        crate::localization::install_snapshot(language);
        if entry.open_target.is_none()
            && altrun_probe::launch::has_context(&entry.command)
            && entry.command.contains("%c")
        {
            context.clipboard = clipboard_text();
        }
        let result = altrun_probe::launch::resolve_entry(&entry, parameter.as_deref(), &context)
            .and_then(|(request, mode, directory)| {
                execute_request_with(&request, mode, directory.as_deref())
            });
        let _ = result_sender.send(result);
        unsafe {
            PostMessageW(owner as HWND, LAUNCH_DONE, 0, 0);
        }
    });
}

pub(super) fn working_directory(
    preferences: &altrun_probe::preferences::Preferences,
    data_path: &std::path::Path,
) -> (String, bool) {
    let parent = data_path.parent().unwrap_or(std::path::Path::new("."));
    let configured = preferences.get("WorkingDirectory");
    if configured.is_empty() {
        (parent.to_string_lossy().into_owned(), false)
    } else {
        (parent.join(configured).to_string_lossy().into_owned(), true)
    }
}

pub(super) fn capture_context() {
    unsafe {
        let foreground = GetForegroundWindow();
        if foreground.is_null() {
            return;
        }
        let mut process = 0;
        GetWindowThreadProcessId(foreground, &mut process);
        if process == std::process::id() {
            return;
        }
        let title = edit_text(foreground);
        let mut class = [0u16; 256];
        let n = GetClassNameW(foreground, class.as_mut_ptr(), class.len() as i32);
        STATE.with(|c| {
            let mut s = c.borrow_mut();
            let s = s.as_mut().unwrap();
            s.context.window_id = foreground as usize;
            s.context.window_title = title;
            s.context.window_class = String::from_utf16_lossy(&class[..n.max(0) as usize]);
        });
    }
}

pub(super) fn selected_action(copy: bool) {
    let entry = STATE.with(|c| {
        let s = c.borrow();
        let s = s.as_ref().unwrap();
        s.hits
            .get(s.selected)
            .map(|i| (s.entries[*i].clone(), s.context.clone()))
    });
    if let Some((entry, context)) = entry {
        if copy {
            let owner = STATE.with(|c| c.borrow().as_ref().unwrap().hwnd);
            if let Err(e) = copy_text(owner, &entry.command) {
                show_inline_error(&e);
            }
        } else {
            std::thread::spawn(move || {
                let base = if entry.working_directory.is_empty() {
                    context.base_directory
                } else {
                    entry.working_directory
                };
                let directory = if entry.open_target.is_some() {
                    std::path::Path::new(&entry.command)
                        .parent()
                        .map(std::path::Path::to_path_buf)
                        .ok_or_else(|| "ui.could_not_find_the_target_s_folder".to_owned())
                } else {
                    altrun_probe::local_target::directory(
                        &entry.command,
                        std::path::Path::new(&base),
                    )
                };
                match directory {
                    Ok(parent) => {
                        let _ = execute_request(&altrun_probe::launch::LaunchRequest {
                            file: parent.to_string_lossy().into_owned(),
                            parameters: String::new(),
                        });
                    }
                    Err(e) => crate::ui::show_error(&e),
                }
            });
        }
    }
}

pub(super) fn trace_event(event: &'static str) {
    let path = STATE.with(|c| {
        let s = c.borrow();
        let s = s.as_ref().unwrap();
        s.preferences
            .flag("TraceEnable")
            .then(|| s.data_path.with_extension("diagnostic.log"))
    });
    if let Some(path) = path {
        std::thread::spawn(move || {
            use std::io::Write;
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis();
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
            {
                let _ = writeln!(f, "{stamp}\t{event}");
            }
        });
    }
}
