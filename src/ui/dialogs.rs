// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

pub(crate) mod input;
pub(crate) mod warning;

use windows_sys::Win32::{
    Foundation::HWND,
    UI::{Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
};

pub(crate) struct ModalOwner {
    window: HWND,
    focus: HWND,
    was_enabled: bool,
}

impl ModalOwner {
    pub(crate) fn disable(window: HWND) -> Self {
        unsafe {
            let focus = GetFocus();
            let was_enabled = IsWindowEnabled(window) != 0;
            if was_enabled {
                EnableWindow(window, 0);
            }
            Self {
                window,
                focus,
                was_enabled,
            }
        }
    }

    pub(crate) fn restore(self, activate: bool) {
        unsafe {
            if !self.was_enabled || IsWindow(self.window) == 0 {
                return;
            }
            EnableWindow(self.window, 1);
            if !activate || IsWindowVisible(self.window) == 0 {
                return;
            }
            SetForegroundWindow(self.window);
            if IsChild(self.window, self.focus) != 0
                && IsWindowVisible(self.focus) != 0
                && IsWindowEnabled(self.focus) != 0
            {
                SetFocus(self.focus);
            }
        }
    }
}

pub(crate) fn accept_key(dialog: HWND, primary: usize, actions: &[usize]) {
    unsafe {
        let focus = GetFocus();
        let command = if IsChild(dialog, focus) != 0 {
            let id = GetDlgCtrlID(focus) as usize;
            actions.iter().copied().find(|&action| action == id)
        } else {
            None
        }
        .unwrap_or(primary);
        let button = GetDlgItem(dialog, command as i32);
        if !button.is_null() && IsWindowEnabled(button) != 0 {
            SendMessageW(dialog, WM_COMMAND, command, 0);
        }
    }
}
