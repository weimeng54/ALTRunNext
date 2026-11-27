// Copyright (c) 2026 GPT-6
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

//! Child construction for themed settings and management forms.
use super::{ui, wide};
use windows_sys::Win32::{Foundation::*, Graphics::Gdi::HFONT, UI::WindowsAndMessaging::*};

pub(crate) struct ChildControls {
    pub(crate) parent: HWND,
    pub(crate) instance: HINSTANCE,
    pub(crate) font: HFONT,
}

impl ChildControls {
    pub(crate) fn create(
        &self,
        class: &str,
        label: &str,
        style: u32,
        rect: RECT,
        id: usize,
    ) -> HWND {
        let style = if class == "BUTTON" && style & 15 != BS_AUTOCHECKBOX as u32 {
            (style & !15) | BS_OWNERDRAW as u32
        } else {
            style
        };
        unsafe {
            let child = CreateWindowExW(
                0,
                wide(class).as_ptr(),
                ui(label).as_ptr(),
                WS_CHILD | WS_VISIBLE | style,
                rect.left,
                rect.top,
                (rect.right - rect.left).max(1),
                (rect.bottom - rect.top).max(1),
                self.parent,
                id as HMENU,
                self.instance,
                std::ptr::null(),
            );
            if class != "EDIT" {
                super::language::caption(child, label);
            }
            SendMessageW(child, WM_SETFONT, self.font as WPARAM, 0);
            child
        }
    }
}
