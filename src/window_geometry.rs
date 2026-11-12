use crate::native::scaled;
use std::ptr::null_mut;
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    UI::{HiDpi::*, WindowsAndMessaging::*},
};
pub fn fit(hwnd: HWND, width: i32, height: i32, remembered: Option<(i32, i32, i32, i32)>) -> i32 {
    unsafe {
        let monitor = if let Some((x, y, _, _)) = remembered {
            MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTONEAREST)
        } else {
            MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST)
        };
        let mut info: MONITORINFO = std::mem::zeroed();
        info.cbSize = std::mem::size_of_val(&info) as u32;
        if GetMonitorInfoW(monitor, &mut info) == 0 {
            return 96;
        }
        let work = info.rcWork;
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
        let default_width = outer.right - outer.left;
        let default_height = outer.bottom - outer.top;
        let (x, y, w, h) = remembered.unwrap_or((
            work.left + (work.right - work.left - default_width) / 2,
            work.top + (work.bottom - work.top - default_height) / 2,
            default_width,
            default_height,
        ));
        let w = w.clamp(
            (scaled(width.min(500), dpi) + default_width - scaled(width, dpi))
                .min(work.right - work.left),
            work.right - work.left,
        );
        let h = h.clamp(
            (scaled(height.min(360), dpi) + default_height - scaled(height, dpi))
                .min(work.bottom - work.top),
            work.bottom - work.top,
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
#[derive(Clone)]
pub struct Child {
    pub handle: usize,
    pub rectangle: RECT,
}
pub fn children(hwnd: HWND) -> Vec<Child> {
    struct Request {
        parent: HWND,
        children: Vec<Child>,
    }
    unsafe extern "system" fn visit(hwnd: HWND, value: LPARAM) -> i32 {
        unsafe {
            let request = &mut *(value as *mut Request);
            if GetParent(hwnd) == request.parent {
                let mut rect: RECT = std::mem::zeroed();
                GetWindowRect(hwnd, &mut rect);
                let mut p = POINT {
                    x: rect.left,
                    y: rect.top,
                };
                ScreenToClient(request.parent, &mut p);
                rect.right = p.x + rect.right - rect.left;
                rect.bottom = p.y + rect.bottom - rect.top;
                rect.left = p.x;
                rect.top = p.y;
                request.children.push(Child {
                    handle: hwnd as usize,
                    rectangle: rect,
                });
            }
        }
        1
    }
    let mut request = Request {
        parent: hwnd,
        children: Vec::new(),
    };
    unsafe {
        EnumChildWindows(hwnd, Some(visit), &mut request as *mut _ as isize);
    }
    request.children
}
