// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::*;

pub(super) fn stop_capture() {
    if let Some(recording) = CAPTURE.take() {
        unsafe {
            if !recording.hook.is_null() {
                UnhookWindowsHookEx(recording.hook);
            }
            let control = GetDlgItem(recording.window, (FIELD + recording.index) as i32);
            InvalidateRect(control, std::ptr::null(), 0);
        }
    }
}

pub(super) fn cancel_capture() {
    stop_capture();
}

fn key_name(vk: u32) -> Option<String> {
    Some(match vk {
        0x30..=0x39 | 0x41..=0x5a => char::from(vk as u8).to_string(),
        0x70..=0x87 => format!("F{}", vk - 0x6f),
        0x13 => "Pause".into(),
        0x20 => "Space".into(),
        0x09 => "Tab".into(),
        0x0d => "Enter".into(),
        0x1b => "Esc".into(),
        0x2d => "Insert".into(),
        0x2e => "Delete".into(),
        0x24 => "Home".into(),
        0x23 => "End".into(),
        0x21 => "PageUp".into(),
        0x22 => "PageDown".into(),
        _ => return None,
    })
}

pub(super) fn modifier_bits(state: impl Fn(i32) -> i16) -> usize {
    usize::from(state(VK_MENU as i32) < 0)
        | (usize::from(state(VK_CONTROL as i32) < 0) << 1)
        | (usize::from(state(VK_SHIFT as i32) < 0) << 2)
        | (usize::from(state(VK_LWIN as i32) < 0 || state(VK_RWIN as i32) < 0) << 3)
}

fn is_modifier(vk: u32) -> bool {
    matches!(vk, 0x10..=0x12 | 0x5b | 0x5c | 0xa0..=0xa5)
}

pub(super) fn record_key(vk: u32, modifiers: usize) -> bool {
    let Some(recording) = CAPTURE.get() else {
        return false;
    };
    if is_modifier(vk) {
        return false;
    }
    if vk == VK_TAB as u32 && modifiers & !4 == 0 {
        stop_capture();
        return false;
    }
    if vk == VK_ESCAPE as u32 && modifiers == 0 {
        stop_capture();
        return true;
    }
    let Some(key) = key_name(vk) else {
        status("ui.press_a_supported_key_such_as_a");
        return true;
    };
    let mut value = String::new();
    for (bit, label) in [(2, "Ctrl+"), (1, "Alt+"), (4, "Shift+"), (8, "Win+")] {
        if modifiers & bit != 0 {
            value.push_str(label);
        }
    }
    value.push_str(&key);
    if altrun_probe::preferences::parse_hotkey(&value).is_err() {
        status("ui.press_a_supported_key_such_as_a");
        return true;
    }
    let field = fields()[recording.index];
    if text(field.value) != value {
        unsafe {
            SetWindowTextW(field.value, wide(&value).as_ptr());
            InvalidateRect(field.value, std::ptr::null(), 0);
        }
        status("ui.shortcut_changed_save_to_apply_it");
    }
    true
}

unsafe extern "system" fn capture_hook(code: i32, w: WPARAM, l: LPARAM) -> LRESULT {
    if code >= 0
        && matches!(w as u32, WM_KEYDOWN | WM_SYSKEYDOWN)
        && let Some(recording) = CAPTURE.get()
        && unsafe { GetForegroundWindow() == recording.window }
    {
        let vk = unsafe { (*(l as *const KBDLLHOOKSTRUCT)).vkCode };
        if !is_modifier(vk) {
            let modifiers = modifier_bits(|key| unsafe { GetAsyncKeyState(key) });
            if vk == VK_TAB as u32 && modifiers & !4 == 0 {
                return unsafe { CallNextHookEx(null_mut(), code, w, l) };
            }
            unsafe {
                PostMessageW(
                    recording.window,
                    RECORDED,
                    vk as usize,
                    (recording.index << 8 | modifiers) as isize,
                );
            }
            return 1;
        }
    }
    unsafe { CallNextHookEx(null_mut(), code, w, l) }
}

fn start_capture(hwnd: HWND, index: usize) {
    if CAPTURE.get().is_some_and(|capture| capture.index == index) {
        return;
    }
    stop_capture();
    let hook = unsafe {
        SetWindowsHookExW(
            WH_KEYBOARD_LL,
            Some(capture_hook),
            GetModuleHandleW(std::ptr::null()),
            0,
        )
    };
    CAPTURE.set(Some(Capture {
        window: hwnd,
        index,
        hook,
    }));
    unsafe {
        InvalidateRect(
            GetDlgItem(hwnd, (FIELD + index) as i32),
            std::ptr::null(),
            0,
        );
    }
}

pub(super) unsafe extern "system" fn hotkey_proc(
    control: HWND,
    msg: u32,
    w: WPARAM,
    l: LPARAM,
    _: usize,
    index: usize,
) -> LRESULT {
    unsafe {
        match msg {
            WM_MOUSEMOVE => {
                let window = GetParent(control);
                if GetForegroundWindow() == window {
                    let mut track = TRACKMOUSEEVENT {
                        cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                        dwFlags: TME_LEAVE,
                        hwndTrack: control,
                        dwHoverTime: 0,
                    };
                    TrackMouseEvent(&mut track);
                    start_capture(window, index);
                }
            }
            WM_SETFOCUS => start_capture(GetParent(control), index),
            WM_MOUSELEAVE | WM_KILLFOCUS | WM_NCDESTROY
                if CAPTURE.get().is_some_and(|capture| capture.index == index) =>
            {
                stop_capture();
            }
            _ => {}
        }
        DefSubclassProc(control, msg, w, l)
    }
}
