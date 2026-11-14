// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::*;

pub(super) enum BindingUpdate<'a> {
    Startup(&'a mut Vec<String>, &'a mut Option<String>),
    Settings(&'a altrun_probe::preferences::Preferences),
}

pub(super) fn register_bindings(
    hwnd: HWND,
    prefs: &altrun_probe::preferences::Preferences,
    old: &[(i32, Option<altrun_probe::preferences::Hotkey>)],
    mut update: BindingUpdate<'_>,
) -> Result<Bindings, String> {
    let requested = ["HotKey1", "HotKey2", "LastItemHotKey"]
        .iter()
        .map(|key| altrun_probe::preferences::parse_hotkey(&prefs.get(key)))
        .collect::<Result<Vec<_>, _>>()?;
    if requested.iter().all(Option::is_none) && !prefs.flag("ShowTrayIcon") {
        return Err("ui.keep_a_launcher_hotkey_or_the_tray".into());
    }
    for (i, value) in requested.iter().enumerate() {
        if value.is_some() && requested[..i].contains(value) {
            return Err("ui.choose_different_hotkeys_for_opening_the_launcher".into());
        }
    }
    let mut next = Vec::new();
    let mut added = Vec::new();
    for (i, key) in ["HotKey1", "HotKey2", "LastItemHotKey"].iter().enumerate() {
        let value = requested[i];
        if let Some(existing) = old
            .iter()
            .find(|(_, old_value)| value.is_some() && *old_value == value)
        {
            next.push(*existing);
            continue;
        }
        let id = (HOTKEY..HOTKEY + 20)
            .find(|id| !old.iter().any(|v| v.0 == *id) && !next.iter().any(|v| v.0 == *id))
            .ok_or("ui.no_hotkey_slot_is_available")?;
        let previously_unavailable = if let BindingUpdate::Settings(previous) = &update {
            old.get(i).is_none_or(|entry| entry.1.is_none())
                && altrun_probe::preferences::parse_hotkey(&previous.get(key))? == value
        } else {
            false
        };
        if let Some(value) = value {
            if unsafe { RegisterHotKey(hwnd, id, value.modifiers | MOD_NOREPEAT, value.key) } == 0 {
                let error = unsafe { GetLastError() };
                let binding = prefs.get(key);
                if let BindingUpdate::Startup(warnings, primary) = &mut update {
                    eprintln!("RegisterHotKey({binding}) failed: Win32 {error}");
                    if i == 0 {
                        **primary = Some(binding);
                    } else {
                        warnings.push(
                            crate::localization::tr("ui.hotkey_binding_is_unavailable")
                                .replace("{binding}", &binding),
                        );
                    }
                    next.push((id, None));
                    continue;
                }
                // Retry unchanged bindings in case the conflict has gone away,
                // but keep unrelated edits saveable while it persists.
                if previously_unavailable {
                    next.push((id, None));
                    continue;
                }
                for id in added {
                    unsafe {
                        UnregisterHotKey(hwnd, id);
                    }
                }
                if error == ERROR_HOTKEY_ALREADY_REGISTERED {
                    return Err(crate::localization::tr(
                        "ui.hotkey_binding_is_already_in_use_your",
                    )
                    .replace("{binding}", &binding));
                }
                return Err(crate::localization::message(
                    "ui.could_not_register_binding_windows_error_error",
                    &[("binding", &binding), ("error", &error.to_string())],
                ));
            }
            added.push(id);
        }
        next.push((id, value));
    }
    Ok(next)
}

pub(super) fn apply_settings_messages() {
    loop {
        let change = STATE.with(|c| {
            c.borrow()
                .as_ref()
                .unwrap()
                .settings_updates
                .try_recv()
                .ok()
        });
        let Some(change) = change else {
            break;
        };
        match change {
            crate::settings::Change::FlushHistory(reply) => {
                let flushed = STATE.with(|s| s.borrow().as_ref().unwrap().usage_writer.flush());
                let _ = reply.send(flushed);
            }
            crate::settings::Change::BeginRestore(reply) => {
                let result = STATE.with(|cell| {
                    let state = cell.borrow();
                    let state = state.as_ref().unwrap();
                    if RESTORING_BACKUP.get()
                        || state.launching.is_some()
                        || state.pending_settings.is_some()
                    {
                        return Err("backup.busy".into());
                    }
                    RESTORING_BACKUP.set(true);
                    Ok((state.hwnd, state.usage_writer.flush()))
                });
                match result {
                    Ok((hwnd, flushed)) => {
                        unsafe {
                            ShowWindow(hwnd, SW_HIDE);
                        }
                        if reply.send(Ok(flushed)).is_err() {
                            RESTORING_BACKUP.set(false);
                        }
                    }
                    Err(error) => {
                        let _ = reply.send(Err(error));
                    }
                }
            }
            crate::settings::Change::CancelRestore => {
                RESTORING_BACKUP.set(false);
            }
            crate::settings::Change::FinishRestore => super::tray::finish_restore(),
            crate::settings::Change::ExitForRecovery => {
                let hwnd = STATE.with(|s| s.borrow().as_ref().unwrap().hwnd);
                unsafe {
                    DestroyWindow(hwnd);
                }
            }
            crate::settings::Change::Prepare(token, prefs, reply) => {
                let (hwnd, old, previous, pending) = STATE.with(|c| {
                    let s = c.borrow();
                    let s = s.as_ref().unwrap();
                    (
                        s.hwnd,
                        s.hotkeys.clone(),
                        s.preferences.clone(),
                        s.pending_settings.is_some(),
                    )
                });
                if pending {
                    let _ = reply.send(Err("ui.another_settings_change_is_in_progress".into()));
                    continue;
                }
                match register_bindings(hwnd, &prefs, &old, BindingUpdate::Settings(&previous)) {
                    Ok(keys) => {
                        STATE.with(|c| {
                            c.borrow_mut().as_mut().unwrap().pending_settings =
                                Some((token, prefs, keys))
                        });
                        let _ = reply.send(Ok(()));
                    }
                    Err(e) => {
                        let _ = reply.send(Err(e));
                    }
                }
            }
            crate::settings::Change::Commit(token) => {
                let accepted = STATE.with(|c| {
                    let mut s = c.borrow_mut();
                    let s = s.as_mut().unwrap();
                    if !s.pending_settings.as_ref().is_some_and(|p| p.0 == token) {
                        return false;
                    }
                    let (_, prefs, keys) = s.pending_settings.take().unwrap();
                    for (id, value) in &s.hotkeys {
                        if value.is_some() && !keys.iter().any(|v| v.0 == *id) {
                            unsafe {
                                UnregisterHotKey(s.hwnd, *id);
                            }
                        }
                    }
                    s.preferences = prefs;
                    (
                        s.context.base_directory,
                        s.context.base_is_working_directory,
                    ) = working_directory(&s.preferences, &s.data_path);
                    s.context.allow_dynamic_target = s.preferences.flag("AllowDynamicTarget");
                    crate::manager::configure_context(s.context.clone());
                    s.hotkeys = keys;
                    true
                });
                if accepted {
                    let language =
                        STATE.with(|c| c.borrow().as_ref().unwrap().preferences.get("Lang"));
                    crate::localization::set_active(&language);
                    let empty = STATE.with(|c| c.borrow().as_ref().unwrap().empty);
                    unsafe {
                        SetWindowTextW(empty, ui("ui.no_matches_try_another_shortcut").as_ptr());
                    }
                    apply_appearance();
                    refresh();
                    update_search_ime(true);
                }
            }
            crate::settings::Change::Cancel(token) => {
                STATE.with(|c| {
                    let mut s = c.borrow_mut();
                    let s = s.as_mut().unwrap();
                    if s.pending_settings.as_ref().is_some_and(|p| p.0 == token) {
                        let (_, _, keys) = s.pending_settings.take().unwrap();
                        for (id, value) in keys {
                            if value.is_some() && !s.hotkeys.iter().any(|v| v.0 == id) {
                                unsafe {
                                    UnregisterHotKey(s.hwnd, id);
                                }
                            }
                        }
                    }
                });
            }
        }
    }
}
