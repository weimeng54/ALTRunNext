// Copyright (c) 2026 GPT-6
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

//! Shared outer contour for transient native windows. USER32 owns an installed region.
use super::scaled;
use crate::theme;
use windows_sys::Win32::{Foundation::*, Graphics::Gdi::*, UI::WindowsAndMessaging::*};

pub(crate) fn shape(hwnd: HWND, dpi: i32) {
    super::window_region::shape(hwnd, scaled(12, dpi));
}

pub(crate) fn draw(hwnd: HWND, dc: HDC, dpi: i32) {
    super::window_region::draw(hwnd, dc, scaled(12, dpi), theme::divider());
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
