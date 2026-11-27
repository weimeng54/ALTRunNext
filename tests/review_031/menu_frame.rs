// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use std::{ptr::null_mut, thread, time::Duration};
use windows_sys::Win32::{Foundation::*, Graphics::Gdi::*, UI::WindowsAndMessaging::*};

// Sample the composed desktop before requesting a repaint. WM_PRINT and an
// explicit WM_NCPAINT both hide an initial USER32 blit that replaces our frame.
fn first_gap(hwnd: HWND, region: HRGN, expected: u32) -> Option<(i32, i32, u32)> {
    unsafe {
        let mut bounds = RECT::default();
        assert_ne!(GetWindowRect(hwnd, &mut bounds), 0);
        let width = bounds.right - bounds.left;
        let height = bounds.bottom - bounds.top;
        let screen = GetDC(null_mut());
        let dc = CreateCompatibleDC(screen);
        let bitmap = CreateCompatibleBitmap(screen, width, height);
        let old = SelectObject(dc, bitmap);
        assert_ne!(
            BitBlt(
                dc,
                0,
                0,
                width,
                height,
                screen,
                bounds.left,
                bounds.top,
                SRCCOPY | CAPTUREBLT,
            ),
            0
        );
        // The first and last included point on each row and column cover all
        // four straight edges and both slopes of every rounded corner.
        let mut gap = None;
        let mut check = |x, y| {
            let actual = GetPixel(dc, x, y);
            if actual != expected && gap.is_none() {
                gap = Some((x, y, actual));
            }
        };
        for y in 0..height {
            let mut xs = (0..width).filter(|&x| PtInRegion(region, x, y) != 0);
            if let Some(left) = xs.next() {
                check(left, y);
                check(xs.next_back().unwrap_or(left), y);
            }
        }
        for x in 0..width {
            let mut ys = (0..height).filter(|&y| PtInRegion(region, x, y) != 0);
            if let Some(top) = ys.next() {
                check(x, top);
                check(x, ys.next_back().unwrap_or(top));
            }
        }
        SelectObject(dc, old);
        DeleteObject(bitmap);
        DeleteDC(dc);
        ReleaseDC(null_mut(), screen);
        gap
    }
}

pub(super) fn check(hwnd: HWND, region: HRGN, theme: &str) {
    use windows_sys::Win32::UI::HiDpi::{
        DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetThreadDpiAwarenessContext,
    };
    let previous_dpi =
        unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
    let expected = match theme {
        "light" => 0xE9DDD3,
        "dark" => 0x4A3E34,
        _ => unsafe { GetSysColor(COLOR_WINDOWTEXT) },
    };
    thread::sleep(Duration::from_millis(400));
    assert_eq!(first_gap(hwnd, region, expected), None, "opening {theme}");
    unsafe {
        let mut previous = POINT::default();
        assert_ne!(GetCursorPos(&mut previous), 0);
        let menu = SendMessageW(hwnd, MN_GETHMENU, 0, 0) as HMENU;
        let last = GetMenuItemCount(menu) - 1;
        assert!(last > 0);
        let hover = [0, last as u32].map(|position| {
            let mut row = RECT::default();
            assert_ne!(GetMenuItemRect(null_mut(), menu, position, &mut row), 0);
            assert_ne!(
                SetCursorPos((row.left + row.right) / 2, (row.top + row.bottom) / 2),
                0
            );
            thread::sleep(Duration::from_millis(150));
            first_gap(hwnd, region, expected)
        });
        SetCursorPos(previous.x, previous.y);
        assert_eq!(
            hover,
            [None, None],
            "hovering first and last rows in {theme}"
        );
        RedrawWindow(
            hwnd,
            std::ptr::null(),
            null_mut(),
            RDW_FRAME | RDW_INVALIDATE | RDW_UPDATENOW,
        );
        windows_sys::Win32::Graphics::Dwm::DwmFlush();
        assert_eq!(first_gap(hwnd, region, expected), None, "redrawing {theme}");
        SetThreadDpiAwarenessContext(previous_dpi);
    }
}
