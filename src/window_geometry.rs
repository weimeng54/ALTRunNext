use crate::native::scaled;
use std::ptr::null_mut;
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    UI::{HiDpi::*, WindowsAndMessaging::*},
};
pub fn fit(hwnd: HWND, width: i32, height: i32, position: Option<(i32, i32)>) -> i32 {
    unsafe {
        let current = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        let monitor = if let Some((x, y)) = position {
            MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTONEAREST)
        } else {
            current
        };
        let mut info: MONITORINFO = std::mem::zeroed();
        info.cbSize = std::mem::size_of_val(&info) as u32;
        if GetMonitorInfoW(monitor, &mut info) == 0 {
            return 96;
        }
        let work = info.rcWork;
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
        let dpi = (actual as i32)
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
        let (x, y) = position.unwrap_or((
            work.left + (work.right - work.left - w) / 2,
            work.top + (work.bottom - work.top - h) / 2,
        ));
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
