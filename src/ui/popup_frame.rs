// Copyright (c) 2026 GPT-6
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

//! Shared outer contour for transient native windows. USER32 owns an installed region.
use super::scaled;
use crate::theme;
use windows_sys::Win32::{Foundation::*, Graphics::Gdi::*, UI::WindowsAndMessaging::*};

fn outline(hwnd: HWND, dpi: i32) -> HRGN {
    let mut rect = RECT::default();
    unsafe {
        if GetWindowRect(hwnd, &mut rect) == 0 || rect.right <= rect.left || rect.bottom <= rect.top
        {
            return std::ptr::null_mut();
        }
        let diameter = scaled(12, dpi);
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

pub(crate) fn shape(hwnd: HWND, dpi: i32) {
    let region = outline(hwnd, dpi);
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

pub(crate) fn draw(hwnd: HWND, dc: HDC, dpi: i32) {
    let region = outline(hwnd, dpi);
    if !region.is_null() {
        unsafe {
            let brush = GetStockObject(DC_BRUSH);
            let previous = SetDCBrushColor(dc, theme::divider());
            FrameRgn(dc, region, brush, 1, 1);
            SetDCBrushColor(dc, previous);
            DeleteObject(region);
        }
    }
}

// USER32 may repaint only an individual item on hover. At large scales the
// window's rounded outline extends into the first and last client rows.
pub(crate) fn draw_row(hwnd: HWND, dc: HDC, dpi: i32, row: RECT) {
    unsafe {
        let mut client = RECT::default();
        if hwnd.is_null() || GetClientRect(hwnd, &mut client) == 0 {
            return;
        }
        let radius = scaled(6, dpi);
        if row.top >= radius && row.bottom <= client.bottom - radius {
            return;
        }
        let mut window = RECT::default();
        let mut origin = POINT::default();
        if GetWindowRect(hwnd, &mut window) == 0 || ClientToScreen(hwnd, &mut origin) == 0 {
            return;
        }
        let saved = SaveDC(dc);
        if saved != 0 {
            IntersectClipRect(dc, row.left, row.top, row.right, row.bottom);
            OffsetViewportOrgEx(
                dc,
                window.left - origin.x,
                window.top - origin.y,
                std::ptr::null_mut(),
            );
            draw(hwnd, dc, dpi);
            RestoreDC(dc, saved);
        }
    }
}
