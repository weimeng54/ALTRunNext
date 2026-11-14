// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::*;

pub(super) fn edit_text(hwnd: HWND) -> String {
    unsafe {
        let mut text = vec![0; GetWindowTextLengthW(hwnd) as usize + 1];
        let n = GetWindowTextW(hwnd, text.as_mut_ptr(), text.len() as i32);
        String::from_utf16_lossy(&text[..n as usize])
    }
}

pub(super) fn search_hint(show_operations: bool) -> Vec<u16> {
    if show_operations {
        ui("ui.type_a_shortcut_enter_to_launch_esc")
    } else {
        ui("ui.type_a_shortcut")
    }
}

pub(super) fn update_search_ime(allow_direct: bool) {
    let target = STATE.with(|c| {
        let state = c.borrow();
        let s = state.as_ref()?;
        Some((
            s.hwnd,
            s.edit,
            allow_direct && s.parameter_entry.is_none() && s.preferences.flag("EnglishSearch"),
        ))
    });
    let Some((hwnd, edit, direct)) = target else {
        return;
    };
    unsafe {
        if InSendMessageEx(std::ptr::null()) != ISMEX_NOSEND {
            // Defer across cross-thread sends so IMM cannot reenter another IME call.
            PostMessageW(hwnd, SEARCH_IME_UPDATE, 0, 0);
            return;
        }
        crate::search_ime::set_direct(
            edit,
            direct && GetFocus() == edit && GetActiveWindow() == hwnd && IsWindowVisible(hwnd) != 0,
        );
    }
}

pub(super) fn cancel_parameter() -> bool {
    let restore = STATE.with(|c| {
        let mut s = c.borrow_mut();
        let s = s.as_mut().unwrap();
        s.parameter_entry.take().map(|_| {
            (
                s.edit,
                s.list,
                std::mem::take(&mut s.saved_query),
                s.preferences.flag("ShowOperationHint"),
            )
        })
    });
    if let Some((edit, list, query, show_operations)) = restore {
        unsafe {
            EnableWindow(list, 1);
            SetWindowTextW(edit, wide(&query).as_ptr());
            SendMessageW(
                edit,
                EM_SETCUEBANNER,
                0,
                search_hint(show_operations).as_ptr() as LPARAM,
            );
        }
        refresh();
        update_search_ime(true);
        true
    } else {
        false
    }
}

pub(super) unsafe extern "system" fn edit_proc(
    hwnd: HWND,
    msg: u32,
    w: WPARAM,
    l: LPARAM,
) -> LRESULT {
    if msg == WM_IME_STARTCOMPOSITION {
        STATE.with(|c| c.borrow_mut().as_mut().unwrap().composing = true);
    }
    if msg == WM_IME_ENDCOMPOSITION {
        STATE.with(|c| c.borrow_mut().as_mut().unwrap().composing = false);
    }
    let (composing, parameter_mode) = STATE.with(|c| {
        let s = c.borrow();
        let s = s.as_ref().unwrap();
        (s.composing, s.parameter_entry.is_some())
    });
    if matches!(msg, WM_KEYDOWN | WM_SYSKEYDOWN) && !composing {
        let control = unsafe { GetKeyState(VK_CONTROL as i32) } < 0;
        let alt = unsafe { GetKeyState(VK_MENU as i32) } < 0;
        STATE.with(|c| {
            c.borrow_mut().as_mut().unwrap().last_key =
                if control && matches!(w as u16, 0x56 | 0x58 | 0x5a) {
                    u16::MAX
                } else {
                    0
                }
        });
        if (control || alt) && !parameter_mode {
            let digit = match w as u16 {
                0x30..=0x39 => Some(w - 0x30),
                0x60..=0x69 => Some(w - 0x60),
                _ => None,
            };
            if let Some(digit) = digit {
                choose_number(digit);
                return 0;
            }
        }
        let (delay, parent) = STATE.with(|c| {
            let s = c.borrow();
            let s = s.as_ref().unwrap();
            (s.preferences.number("HideDelay"), s.hwnd)
        });
        if delay > 0 {
            unsafe {
                SetTimer(parent, AUTO_HIDE, delay as u32 * 1000, None);
            }
        }
        if control {
            match w as u16 {
                0x43 if !parameter_mode => {
                    selected_action(true);
                    return 0;
                }
                0x44 if !parameter_mode => {
                    selected_action(false);
                    return 0;
                }
                0x4c if !parameter_mode => {
                    recent();
                    return 0;
                }
                VK_UP | VK_DOWN
                    if parameter_mode
                        && parameter_history(if w as u16 == VK_UP { -1 } else { 1 }) =>
                {
                    return 0;
                }
                _ => {}
            }
        }
        match w as u16 {
            VK_F1 if !parameter_mode => {
                about();
                return 0;
            }
            VK_F2 if !parameter_mode => {
                edit_selected_command();
                return 0;
            }
            VK_INSERT | VK_DELETE if !parameter_mode => {
                let id = STATE.with(|c| {
                    let s = c.borrow();
                    let s = s.as_ref().unwrap();
                    if w as u16 == VK_INSERT {
                        return s.current_query.clone();
                    }
                    s.hits
                        .get(s.selected)
                        .map(|i| s.entries[*i].id.clone())
                        .unwrap_or_default()
                });
                if id.is_empty() && w as u16 != VK_INSERT {
                    return 0;
                }
                crate::manager::enqueue(Vec::new(), Some((id, w as u16)));
                open_manager();
                return 0;
            }
            VK_PRIOR if !parameter_mode => {
                move_selection(-10);
                return 0;
            }
            VK_NEXT if !parameter_mode => {
                move_selection(10);
                return 0;
            }
            0xba | 0xde if !parameter_mode => {
                choose_number(if w == 0xba { 2 } else { 3 });
                return 0;
            }
            VK_DOWN if alt => {
                parameter_menu();
                return 0;
            }
            VK_DOWN if !parameter_mode => {
                move_selection(1);
                return 0;
            }
            VK_UP if !parameter_mode => {
                move_selection(-1);
                return 0;
            }
            VK_TAB if !parameter_mode => {
                move_selection(if unsafe { GetKeyState(VK_SHIFT as i32) } < 0 {
                    -1
                } else {
                    1
                });
                return 0;
            }
            VK_RETURN => {
                launch_selected();
                return 0;
            }
            VK_ESCAPE => {
                if cancel_parameter() {
                    return 0;
                }
                if unsafe { GetWindowTextLengthW(hwnd) } > 0 {
                    unsafe {
                        SetWindowTextW(hwnd, wide("").as_ptr());
                    }
                } else {
                    let parent = unsafe { GetParent(hwnd) };
                    unsafe {
                        ShowWindow(parent, SW_HIDE);
                    }
                }
                return 0;
            }
            _ => {}
        }
    }
    if msg == WM_CHAR
        && !parameter_mode
        && !composing
        && !matches!(w, 3 | 4 | 12 | 13 | 27 | 9 | 59 | 39)
    {
        let control = unsafe { GetKeyState(VK_CONTROL as i32) } < 0;
        if control && (0x30..=0x39).contains(&w) {
            return 0;
        }
        STATE.with(|c| {
            c.borrow_mut().as_mut().unwrap().last_key = if w == 32 {
                VK_SPACE
            } else if (0x30..=0x39).contains(&w) {
                w as u16
            } else {
                u16::MAX
            }
        });
    }
    if msg == WM_SYSCHAR && !parameter_mode && (0x30..=0x39).contains(&w) {
        return 0;
    }
    if msg == WM_CHAR
        && !parameter_mode
        && unsafe { GetKeyState(VK_CONTROL as i32) } < 0
        && matches!(w, 3 | 4 | 12)
    {
        return 0;
    }
    if msg == WM_CHAR && !parameter_mode && matches!(w, 59 | 39) && !composing {
        return 0;
    }
    if msg == WM_CHAR && matches!(w as u16, VK_RETURN | VK_ESCAPE | VK_TAB) && !composing {
        return 0;
    }
    let old = STATE.with(|c| c.borrow().as_ref().unwrap().old_edit);
    let result = unsafe {
        CallWindowProcW(
            Some(std::mem::transmute::<
                isize,
                unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT,
            >(old)),
            hwnd,
            msg,
            w,
            l,
        )
    };
    if matches!(msg, WM_SETFOCUS | WM_KILLFOCUS) {
        update_search_ime(msg == WM_SETFOCUS);
    }
    result
}

fn parameter_history(delta: isize) -> bool {
    let value = STATE.with(|c| {
        let mut s = c.borrow_mut();
        let s = s.as_mut().unwrap();
        let index = s.parameter_entry?;
        let history = s
            .usage
            .parameters
            .get(&s.entries[index].id)
            .or_else(|| s.usage.parameters.get("*"))?;
        if history.is_empty() {
            return None;
        }
        s.history_position = if s.history_position == usize::MAX {
            0
        } else {
            advance(s.history_position, history.len(), delta)
        };
        Some((s.edit, history[s.history_position].clone()))
    });
    if let Some((edit, value)) = value {
        unsafe {
            SetWindowTextW(edit, wide(&value).as_ptr());
            SendMessageW(edit, EM_SETSEL, 0, -1);
        }
        true
    } else {
        false
    }
}

fn parameter_menu() {
    let (hwnd, values) = STATE.with(|c| {
        let s = c.borrow();
        let s = s.as_ref().unwrap();
        let values = s
            .parameter_entry
            .and_then(|i| s.usage.parameters.get(&s.entries[i].id))
            .or_else(|| s.usage.parameters.get("*"))
            .cloned()
            .unwrap_or_default();
        (s.hwnd, values)
    });
    if values.is_empty() {
        return;
    }
    unsafe {
        let menu = CreatePopupMenu();
        for (i, value) in values.iter().take(1000).enumerate() {
            let display = value.chars().take(80).collect::<String>();
            AppendMenuW(menu, MF_STRING, 27000 + i, wide(&display).as_ptr());
        }
        let mut point: POINT = zeroed();
        GetCursorPos(&mut point);
        let selected = TrackPopupMenu(
            menu,
            TPM_RETURNCMD,
            point.x,
            point.y,
            0,
            hwnd,
            std::ptr::null(),
        );
        DestroyMenu(menu);
        if selected >= 27000
            && let Some(value) = values.get(selected as usize - 27000)
        {
            let edit = STATE.with(|c| c.borrow().as_ref().unwrap().edit);
            SetWindowTextW(edit, wide(value).as_ptr());
            SetFocus(edit);
        }
    }
}
