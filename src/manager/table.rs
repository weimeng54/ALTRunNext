// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

//! Layout and initial column widths for the shortcut editor.
use super::{FOOTER_TOP, LIST_TOP, ROW_HEIGHT, TABLE_TOP};
use crate::ui::{report, scaled};
use windows_sys::Win32::{
    Foundation::*,
    UI::{
        HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi},
        WindowsAndMessaging::*,
    },
};

const TITLES: [&str; 2] = ["ui.alias", "ui.name"];

pub(super) unsafe fn configure(list: HWND, dpi: i32) {
    unsafe {
        let mut client = RECT::default();
        GetClientRect(list, &mut client);
        let width = client.right - GetSystemMetricsForDpi(SM_CXVSCROLL, GetDpiForWindow(list));
        let first = width * 42 / 100;
        report::configure_pixels(list, dpi, &[(TITLES[0], first), (TITLES[1], width - first)]);
    }
}

pub(super) fn list_height(dpi: i32) -> i32 {
    let row = scaled(ROW_HEIGHT, dpi);
    scaled(LIST_TOP, dpi) - scaled(TABLE_TOP, dpi)
        + scaled(FOOTER_TOP - LIST_TOP - 33, dpi) / row * row
}
