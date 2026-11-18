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
            allow_direct && !execution::parameter_open() && s.preferences.flag("EnglishSearch"),
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
    let composing = STATE.with(|c| c.borrow().as_ref().unwrap().composing);
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
        if (control || alt) {
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
                0x43 => {
                    selected_action(true);
                    return 0;
                }
                0x44 => {
                    selected_action(false);
                    return 0;
                }
                0x4c => {
                    recent();
                    return 0;
                }
                _ => {}
            }
        }
        match w as u16 {
            VK_F1 => {
                about();
                return 0;
            }
            VK_F2 => {
                edit_selected_command();
                return 0;
            }
            VK_INSERT => {
                let query = STATE.with(|c| c.borrow().as_ref().unwrap().current_query.clone());
                crate::manager::enqueue(Vec::new(), Some((query, VK_INSERT)));
                open_manager();
                return 0;
            }
            VK_DELETE => {
                request_delete(hwnd, l);
                return 0;
            }
            VK_PRIOR => {
                move_selection(-10);
                return 0;
            }
            VK_NEXT => {
                move_selection(10);
                return 0;
            }
            0xba | 0xde => {
                choose_number(if w == 0xba { 2 } else { 3 });
                return 0;
            }
            VK_DOWN => {
                move_selection(1);
                return 0;
            }
            VK_UP => {
                move_selection(-1);
                return 0;
            }
            VK_TAB => {
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
    if msg == WM_CHAR && !composing && !matches!(w, 3 | 4 | 12 | 13 | 27 | 9 | 59 | 39) {
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
    if msg == WM_SYSCHAR && (0x30..=0x39).contains(&w) {
        return 0;
    }
    if msg == WM_CHAR && unsafe { GetKeyState(VK_CONTROL as i32) } < 0 && matches!(w, 3 | 4 | 12) {
        return 0;
    }
    if msg == WM_CHAR && matches!(w, 59 | 39) && !composing {
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

pub(super) fn request_delete(control: HWND, key_flags: LPARAM) {
    if key_flags & (1 << 30) == 0 {
        unsafe { PostMessageW(GetParent(control), WM_COMMAND, ID_DELETE_COMMAND, 0) };
    }
}
