// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::*;

pub(super) fn show_menu(hwnd: HWND, tray: bool, allow_selection: bool) {
    unsafe {
        let menu = CreatePopupMenu();
        if menu.is_null() {
            return;
        }
        let items: &[(usize, &str)] = if tray {
            &[
                (ID_SHOW, "ui.show_launcher"),
                (ID_MANAGE, "ui.manage_shortcuts"),
                (ID_SETTINGS, "ui.settings"),
                (0, ""),
                (ID_EXIT, "ui.exit_altrunnext"),
            ]
        } else {
            &[
                (ID_COPY, "ui.copy_command_ctrl_c"),
                (ID_EDIT_COMMAND, "ui.edit_command_f2"),
                (ID_OPEN_FOLDER, "ui.open_containing_folder_ctrl_d"),
                (ID_RECENT, "ui.recent_launches_ctrl_l"),
                (0, ""),
                (ID_HIDE, "ui.hide"),
            ]
        };
        let has_selection = STATE.with(|c| {
            let state = c.borrow();
            let s = state.as_ref().unwrap();
            s.hits.get(s.selected).is_some() && s.parameter_entry.is_none() && allow_selection
        });
        for &(id, label) in items {
            if id == 0 {
                AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
            } else {
                let unavailable =
                    matches!(id, ID_COPY | ID_EDIT_COMMAND | ID_OPEN_FOLDER) && !has_selection;
                let flags = MF_STRING | if unavailable { MF_GRAYED } else { 0 };
                AppendMenuW(menu, flags, id, ui(label).as_ptr());
            }
        }
        let mut point: POINT = zeroed();
        GetCursorPos(&mut point);
        SetForegroundWindow(hwnd);
        TrackPopupMenu(
            menu,
            TPM_RIGHTBUTTON,
            point.x,
            point.y,
            0,
            hwnd,
            std::ptr::null(),
        );
        DestroyMenu(menu);
        PostMessageW(hwnd, WM_NULL, 0, 0);
    }
}

pub(super) fn edit_selected_command() {
    let id = STATE.with(|c| {
        let state = c.borrow();
        let s = state.as_ref().unwrap();
        if s.parameter_entry.is_some() {
            return None;
        }
        s.hits
            .get(s.selected)
            .map(|&index| s.entries[index].id.clone())
    });
    if let Some(id) = id {
        crate::manager::enqueue(Vec::new(), Some((id, VK_F2)));
        open_manager();
    }
}

pub(super) fn tray_icon(hwnd: HWND, operation: u32) {
    unsafe {
        let mut icon: NOTIFYICONDATAW = zeroed();
        icon.cbSize = std::mem::size_of_val(&icon) as u32;
        icon.hWnd = hwnd;
        icon.uID = 1;
        icon.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
        icon.uCallbackMessage = TRAY;
        icon.hIcon = crate::ui::app_icon(GetSystemMetricsForDpi(SM_CXSMICON, GetDpiForSystem()));
        let binding = STATE.with(|c| {
            let state = c.borrow();
            let state = state.as_ref()?;
            ["HotKey1", "HotKey2"]
                .into_iter()
                .zip(&state.hotkeys)
                .find(|(_, (_, key))| key.is_some())
                .map(|(name, _)| state.preferences.get(name))
        });
        let tip =
            wide(&binding.map_or_else(|| "ALTRunNext".into(), |key| format!("ALTRunNext · {key}")));
        let len = tip.len().min(icon.szTip.len() - 1);
        icon.szTip[..len].copy_from_slice(&tip[..len]);
        if Shell_NotifyIconW(operation, &icon) == 0 && operation == NIM_ADD {
            Shell_NotifyIconW(NIM_MODIFY, &icon);
        }
    }
}

pub(super) fn about() {
    open_settings_page(Some(crate::settings::ABOUT_PAGE));
}

pub(super) fn open_settings() {
    open_settings_page(None);
}

pub(super) fn open_settings_page(page: Option<usize>) {
    let (path, prefs, hwnd, sender) = STATE.with(|c| {
        let s = c.borrow();
        let s = s.as_ref().unwrap();
        (
            s.data_path.clone(),
            s.preferences.clone(),
            s.hwnd,
            s.settings_sender.clone(),
        )
    });
    crate::settings::open(path, prefs, hwnd, sender, page);
    unsafe {
        ShowWindow(hwnd, SW_HIDE);
    }
}

pub(super) fn popup_sound() {
    STATE.with(|c| {
        let s = c.borrow();
        let s = s.as_ref().unwrap();
        if !s.preferences.flag("PlayPopupNotify") {
            return;
        }
        let file = s.preferences.get("PopupFileName");
        let data_directory = s.data_path.parent().unwrap_or(std::path::Path::new("."));
        let sound = PopupSound {
            path: data_directory.join(&file),
            use_bundled_fallback: file
                .eq_ignore_ascii_case(altrun_probe::preferences::DEFAULT_POPUP_SOUND),
        };
        let _ = s.sound_sender.try_send(sound);
    });
}

pub(super) fn lifecycle(clean: bool) {
    if crate::manager::is_open() || crate::settings::is_open() {
        show_inline_error("ui.save_and_close_the_shortcut_manager_or");
        return;
    }
    let (path, enabled, hwnd) = STATE.with(|c| {
        let s = c.borrow();
        let s = s.as_ref().unwrap();
        (s.data_path.clone(), s.enabled, s.hwnd)
    });
    if clean {
        if unsafe {
            MessageBoxW(
                hwnd,
                ui("ui.turn_off_startup_remove_the_send_to").as_ptr(),
                ui("ui.remove_windows_integrations").as_ptr(),
                MB_OKCANCEL | MB_ICONQUESTION,
            )
        } != IDOK
        {
            return;
        }
        let owner = hwnd as usize;
        std::thread::spawn(move || {
            match altrun_probe::integration::Integration::for_data(&path)
                .and_then(|i| i.apply(false, false))
            {
                Ok(()) => unsafe {
                    PostMessageW(owner as HWND, QUIT, 0, 0);
                },
                Err(e) => crate::ui::show_error(&e),
            }
        });
        return;
    }
    match start_replacement(&path, enabled) {
        Ok(()) => unsafe {
            PostMessageW(hwnd, QUIT, 0, 0);
        },
        Err(e) => show_inline_error(&e),
    }
}

pub(super) fn finish_restore() {
    let (path, enabled, hwnd) = STATE.with(|cell| {
        let state = cell.borrow();
        let state = state.as_ref().unwrap();
        (state.data_path.clone(), state.enabled, state.hwnd)
    });
    if start_replacement(&path, enabled).is_err() {
        crate::ui::show_error("backup.restart_failed");
    }
    unsafe {
        DestroyWindow(hwnd);
    }
}

fn start_replacement(path: &std::path::Path, enabled: bool) -> Result<(), String> {
    std::env::current_exe()
        .map_err(|e| e.to_string())
        .and_then(|exe| {
            use std::os::windows::process::CommandExt;
            let mut command = std::process::Command::new(exe);
            command
                .arg("--data")
                .arg(path)
                .arg("--wait-pid")
                .arg(std::process::id().to_string());
            if enabled {
                command.arg("--enable-launch");
            } else if std::env::args().any(|arg| arg == "--safe-mode") {
                command.arg("--safe-mode");
            }
            command
                .creation_flags(0x08000000)
                .spawn()
                .map(|_| ())
                .map_err(|e| e.to_string())
        })
}
