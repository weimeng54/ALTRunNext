// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use std::{ptr::null_mut, thread, time::Duration};
use windows_sys::Win32::{Foundation::*, Graphics::Gdi::*, UI::WindowsAndMessaging::*};

fn border(theme: &str) -> u32 {
    match theme {
        "light" => 0xE9DDD3,
        "dark" => 0x4A3E34,
        _ => unsafe { GetSysColor(COLOR_WINDOWTEXT) },
    }
}

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
    let expected = border(theme);
    thread::sleep(Duration::from_millis(400));
    assert_eq!(first_gap(hwnd, region, expected), None, "opening {theme}");
    unsafe {
        let mut previous = POINT::default();
        assert_ne!(GetCursorPos(&mut previous), 0);
        let menu = SendMessageW(hwnd, MN_GETHMENU, 0, 0) as HMENU;
        let last = GetMenuItemCount(menu) - 1;
        assert!(last > 0);
        let gaps = [0, last as u32].map(|position| {
            hover(menu, position);
            thread::sleep(Duration::from_millis(150));
            first_gap(hwnd, region, expected)
        });
        SetCursorPos(previous.x, previous.y);
        assert_eq!(
            gaps,
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

fn hover(menu: HMENU, row: u32) {
    unsafe {
        let mut bounds = RECT::default();
        assert_ne!(GetMenuItemRect(null_mut(), menu, row, &mut bounds), 0);
        assert_ne!(
            SetCursorPos(
                (bounds.left + bounds.right) / 2,
                (bounds.top + bounds.bottom) / 2,
            ),
            0
        );
    }
}

fn wait_popup(pid: u32, menu: Option<HMENU>) -> HWND {
    let class = super::review_native_support::wide("#32768");
    let mut hwnd = null_mut();
    super::wait(|| unsafe {
        hwnd = null_mut();
        loop {
            hwnd = FindWindowExW(null_mut(), hwnd, class.as_ptr(), std::ptr::null());
            if hwnd.is_null() {
                return false;
            }
            let mut owner = 0;
            GetWindowThreadProcessId(hwnd, &mut owner);
            if owner == pid && IsWindowVisible(hwnd) != 0 {
                let actual = SendMessageW(hwnd, MN_GETHMENU, 0, 0) as HMENU;
                if !actual.is_null() && menu.is_none_or(|menu| actual == menu) {
                    return true;
                }
            }
        }
    });
    hwnd
}

fn check_sort_frame(hwnd: HWND, theme: &str, stage: &str) {
    unsafe {
        let region = CreateRectRgn(0, 0, 0, 0);
        assert!(!region.is_null());
        super::wait(|| GetWindowRgn(hwnd, region) > 1);
        let mut bounds = RECT::default();
        assert_ne!(GetWindowRect(hwnd, &mut bounds), 0);
        let width = bounds.right - bounds.left;
        let height = bounds.bottom - bounds.top;
        for x in [0, width - 1] {
            for y in [0, height - 1] {
                assert_eq!(PtInRegion(region, x, y), 0, "{stage}: square corner");
            }
        }
        windows_sys::Win32::Graphics::Dwm::DwmFlush();
        let gap = first_gap(hwnd, region, border(theme));
        DeleteObject(region);
        assert_eq!(gap, None, "{theme} sort submenu on {stage}");

        // The straight edges must also have just one outline. A native gray
        // rectangle immediately inside the themed perimeter is visible on screen.
        let screen = GetDC(null_mut());
        let surface = if theme == "dark" { 0x322922 } else { 0xffffff };
        for (x, y) in [
            (1, height / 2),
            (width - 2, height / 2),
            (width / 2, 1),
            (width / 2, height - 2),
        ] {
            assert_eq!(
                GetPixel(screen, bounds.left + x, bounds.top + y),
                surface,
                "{theme} {stage}: inner frame at {x},{y}"
            );
        }
        ReleaseDC(null_mut(), screen);
    }
}

pub(super) fn check_sort(manager: HWND, pid: u32, theme: &str, scale: &str) {
    use windows_sys::Win32::UI::{
        HiDpi::{DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetThreadDpiAwarenessContext},
        Input::KeyboardAndMouse::{VK_DOWN, VK_HOME, VK_RETURN, VK_RIGHT, VK_UP},
    };
    unsafe {
        let dpi = SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let mut previous = POINT::default();
        assert_ne!(GetCursorPos(&mut previous), 0);
        for (pass, expected) in [2130, 2132].into_iter().enumerate() {
            super::begin(manager, 2141);
            let parent = wait_popup(pid, None);
            let tools = SendMessageW(parent, MN_GETHMENU, 0, 0) as HMENU;
            let sort = GetSubMenu(tools, 2);
            assert!(!sort.is_null());
            if pass == 0 {
                hover(tools, 2);
            } else {
                for key in [VK_HOME, VK_DOWN, VK_RIGHT] {
                    super::press(manager, key);
                }
            }
            let child = wait_popup(pid, Some(sort));
            assert_eq!(GetMenuItemCount(sort), 3);
            for id in [2130, 2132, 2133] {
                let state = GetMenuState(sort, id, MF_BYCOMMAND);
                assert_ne!(state, u32::MAX);
                assert_eq!(state & MF_CHECKED != 0, id == expected);
            }
            thread::sleep(Duration::from_millis(150));
            check_sort_frame(
                child,
                theme,
                if pass == 0 { "opening" } else { "reopening" },
            );
            if pass == 0 {
                for (row, stage) in [(0, "first hover"), (2, "last hover")] {
                    hover(sort, row);
                    super::wait(|| GetMenuState(sort, row, MF_BYPOSITION) & MF_HILITE != 0);
                    check_sort_frame(child, theme, stage);
                }
                super::press(manager, VK_UP);
                super::wait(|| GetMenuState(sort, 1, MF_BYPOSITION) & MF_HILITE != 0);
                check_sort_frame(child, theme, "keyboard selection");
                super::native_snapshot::save_painted(child, &format!("menu-sort-{theme}-{scale}"));
            } else {
                super::press(manager, VK_HOME);
                super::wait(|| GetMenuState(sort, 0, MF_BYPOSITION) & MF_HILITE != 0);
            }
            super::press(manager, VK_RETURN);
            super::wait(|| IsWindowVisible(parent) == 0);
        }
        SetCursorPos(previous.x, previous.y);
        SetThreadDpiAwarenessContext(dpi);
    }
}
