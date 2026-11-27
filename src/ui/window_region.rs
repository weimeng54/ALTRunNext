// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

//! Rounded clipping and matching GDI outline for application windows and popups.
use windows_sys::Win32::{Foundation::*, Graphics::Gdi::*, UI::WindowsAndMessaging::*};

fn outline(hwnd: HWND, diameter: i32) -> HRGN {
    let mut rect = RECT::default();
    unsafe {
        if GetWindowRect(hwnd, &mut rect) == 0 || rect.right <= rect.left || rect.bottom <= rect.top
        {
            return std::ptr::null_mut();
        }
        CreateRoundRectRgn(
            0,
            0,
            rect.right - rect.left + 1,
            rect.bottom - rect.top + 1,
            diameter,
            diameter,
        )
    }
}

pub(super) fn shape(hwnd: HWND, diameter: i32) {
    let region = outline(hwnd, diameter);
    if region.is_null() {
        return;
    }
    unsafe {
        let current = CreateRectRgn(0, 0, 0, 0);
        let matches = !current.is_null()
            && GetWindowRgn(hwnd, current) != 0
            && EqualRgn(current, region) != 0;
        DeleteObject(current);
        if matches || SetWindowRgn(hwnd, region, 0) == 0 {
            DeleteObject(region);
        }
    }
}

pub(super) fn draw(hwnd: HWND, dc: HDC, diameter: i32, colour: u32) {
    let region = outline(hwnd, diameter);
    if !region.is_null() {
        unsafe {
            let brush = GetStockObject(DC_BRUSH);
            let previous = SetDCBrushColor(dc, colour);
            FrameRgn(dc, region, brush, 1, 1);
            SetDCBrushColor(dc, previous);
            DeleteObject(region);
        }
    }
}
