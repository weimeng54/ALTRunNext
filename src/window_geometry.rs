// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use crate::native::scaled;
use std::{
    ptr::null_mut,
    sync::atomic::{AtomicI32, Ordering},
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    UI::{HiDpi::*, WindowsAndMessaging::*},
};
static TEST_DPI: AtomicI32 = AtomicI32::new(0);

pub fn set_test_dpi(dpi: i32) {
    TEST_DPI.store(dpi, Ordering::Relaxed);
}

pub fn window_dpi(hwnd: HWND) -> i32 {
    match TEST_DPI.load(Ordering::Relaxed) {
        0 => unsafe { GetDpiForWindow(hwnd).max(96) as i32 },
        dpi => dpi,
    }
}

pub fn cursor_monitor(hwnd: HWND) -> HMONITOR {
    unsafe {
        let mut cursor = POINT::default();
        if GetCursorPos(&mut cursor) == 0 {
            MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST)
        } else {
            MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST)
        }
    }
}

pub fn work_area(monitor: HMONITOR) -> Option<RECT> {
    unsafe {
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..std::mem::zeroed()
        };
        (GetMonitorInfoW(monitor, &mut info) != 0).then_some(info.rcWork)
    }
}

pub fn fit(hwnd: HWND, width: i32, height: i32) -> i32 {
    unsafe {
        let current = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        let monitor = cursor_monitor(hwnd);
        let Some(work) = work_area(monitor) else {
            return 96;
        };
        if monitor != current {
            // Move the hidden window before reading its per-monitor DPI.
            SetWindowPos(
                hwnd,
                null_mut(),
                work.left + (work.right - work.left) / 2,
                work.top + (work.bottom - work.top) / 2,
                1,
                1,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
        let actual = GetDpiForWindow(hwnd).max(96);
        let dpi = window_dpi(hwnd)
            .min((work.right - work.left - 48) * 96 / width)
            .min((work.bottom - work.top - 80) * 96 / height)
            .max(48);
        let mut outer = RECT {
            left: 0,
            top: 0,
            right: scaled(width, dpi),
            bottom: scaled(height, dpi),
        };
        AdjustWindowRectExForDpi(
            &mut outer,
            GetWindowLongW(hwnd, GWL_STYLE) as u32,
            i32::from(!GetMenu(hwnd).is_null()),
            GetWindowLongW(hwnd, GWL_EXSTYLE) as u32,
            actual,
        );
        let w = (outer.right - outer.left).min(work.right - work.left);
        let h = (outer.bottom - outer.top).min(work.bottom - work.top);
        let (x, y) = (
            work.left + (work.right - work.left - w) / 2,
            work.top + (work.bottom - work.top - h) / 2,
        );
        SetWindowPos(
            hwnd,
            null_mut(),
            x.clamp(work.left, (work.right - w).max(work.left)),
            y.clamp(work.top, (work.bottom - h).max(work.top)),
            w,
            h,
            SWP_NOZORDER | SWP_NOACTIVATE,
        );
        dpi
    }
}
