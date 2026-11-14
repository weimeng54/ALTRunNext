// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::{
    painting::{fill, rounded},
    scaled, wide,
};
use crate::theme;
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    UI::{
        Controls::*,
        Shell::{DefSubclassProc, SetWindowSubclass},
        WindowsAndMessaging::*,
    },
};

pub fn vertical_scrollbar(hwnd: HWND) -> Option<SCROLLBARINFO> {
    scrollbar_info(hwnd, OBJID_VSCROLL)
}

fn scrollbar_info(hwnd: HWND, object: OBJECT_IDENTIFIER) -> Option<SCROLLBARINFO> {
    let mut bar = SCROLLBARINFO {
        cbSize: std::mem::size_of::<SCROLLBARINFO>() as u32,
        ..Default::default()
    };
    unsafe {
        (GetScrollBarInfo(hwnd, object, &mut bar) != 0
            && bar.rgstate[0] & STATE_SYSTEM_INVISIBLE == 0)
            .then_some(bar)
    }
}

pub fn draw_scrollbar(dc: HDC, track: RECT, bar: &SCROLLBARINFO, dpi: i32) {
    draw_scrollbar_on_axis(dc, track, bar, dpi, false);
}

fn draw_scrollbar_on_axis(dc: HDC, track: RECT, bar: &SCROLLBARINFO, dpi: i32, horizontal: bool) {
    fill(dc, &track, theme::paper());
    if bar.xyThumbBottom > bar.xyThumbTop {
        let width = scaled(4, dpi).max(3);
        let thumb = if horizontal {
            let top = (track.top + track.bottom - width) / 2;
            RECT {
                left: track.left + bar.xyThumbTop,
                right: track.left + bar.xyThumbBottom,
                top,
                bottom: top + width,
            }
        } else {
            let left = (track.left + track.right - width) / 2;
            RECT {
                left,
                right: left + width,
                top: track.top + bar.xyThumbTop,
                bottom: track.top + bar.xyThumbBottom,
            }
        };
        rounded(dc, thumb, width, theme::muted(), theme::muted());
    }
}

pub fn style_vertical_scrollbar(control: HWND, id: usize, dpi: i32) {
    style_scrollbar(control, id, dpi, false);
}

pub fn style_horizontal_scrollbar(control: HWND, id: usize, dpi: i32) {
    style_scrollbar(control, id, dpi, true);
}

fn style_scrollbar(control: HWND, id: usize, dpi: i32, horizontal: bool) {
    unsafe {
        let scrollbar = CreateWindowExW(
            0,
            wide("STATIC").as_ptr(),
            std::ptr::null(),
            WS_CHILD | WS_CLIPSIBLINGS,
            0,
            0,
            0,
            0,
            GetParent(control),
            id as HMENU,
            std::ptr::null_mut(),
            std::ptr::null(),
        );
        if !scrollbar.is_null()
            && SetWindowSubclass(
                scrollbar,
                Some(scrollbar_proc),
                dpi as usize * 2 + usize::from(horizontal),
                control as usize,
            ) != 0
        {
            SetWindowSubclass(
                control,
                Some(scroll_control_proc),
                1 + usize::from(horizontal),
                scrollbar as usize,
            );
        }
    }
}

unsafe extern "system" fn scroll_control_proc(
    hwnd: HWND,
    msg: u32,
    w: WPARAM,
    l: LPARAM,
    subclass: usize,
    scrollbar: usize,
) -> LRESULT {
    let result = unsafe { DefSubclassProc(hwnd, msg, w, l) };
    if matches!(
        msg,
        WM_NCPAINT
            | WM_PAINT
            | WM_VSCROLL
            | WM_HSCROLL
            | WM_MOUSEWHEEL
            | WM_KEYDOWN
            | WM_NCLBUTTONDOWN
            | WM_SIZE
            | LB_SETTOPINDEX
            | LB_SETCURSEL
            | EM_LINESCROLL
            | EM_SCROLL
            | WM_SETTEXT
            | LVM_SCROLL
            | LVM_SETCOLUMNWIDTH
            | LVM_ENSUREVISIBLE
    ) {
        update_scrollbar(hwnd, scrollbar as HWND, subclass == 2);
    }
    result
}

fn update_scrollbar(list: HWND, scrollbar: HWND, horizontal: bool) {
    unsafe {
        let visible = GetWindowLongW(scrollbar, GWL_STYLE) as u32 & WS_VISIBLE != 0;
        let object = if horizontal {
            OBJID_HSCROLL
        } else {
            OBJID_VSCROLL
        };
        let Some(bar) = scrollbar_info(list, object) else {
            if visible {
                ShowWindow(scrollbar, SW_HIDE);
            }
            return;
        };
        let mut bounds = RECT::default();
        GetWindowRect(scrollbar, &mut bounds);
        let mut track = bar.rcScrollBar;
        if horizontal && let Some(vertical) = vertical_scrollbar(list) {
            // The native size box between the two scrollbars uses the system
            // background. Extend the themed track across that corner.
            track.right = track.right.max(vertical.rcScrollBar.right);
        }
        if !visible || EqualRect(&bounds, &track) == 0 {
            let mut origin = POINT {
                x: track.left,
                y: track.top,
            };
            ScreenToClient(GetParent(scrollbar), &mut origin);
            SetWindowPos(
                scrollbar,
                HWND_TOP,
                origin.x,
                origin.y,
                track.right - track.left,
                track.bottom - track.top,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
        }
        // Native scrollbar tracking can draw directly into its list window.
        // A separate, clipped sibling keeps our colors visible during that loop.
        RedrawWindow(
            scrollbar,
            std::ptr::null(),
            std::ptr::null_mut(),
            RDW_INVALIDATE | RDW_UPDATENOW,
        );
    }
}

unsafe extern "system" fn scrollbar_proc(
    hwnd: HWND,
    msg: u32,
    w: WPARAM,
    l: LPARAM,
    style: usize,
    list: usize,
) -> LRESULT {
    match msg {
        WM_NCHITTEST => HTTRANSPARENT as isize,
        WM_ERASEBKGND => 1,
        WM_PAINT | WM_PRINTCLIENT => unsafe {
            let mut paint = PAINTSTRUCT::default();
            let dc = if msg == WM_PAINT {
                BeginPaint(hwnd, &mut paint)
            } else {
                w as HDC
            };
            let mut track = RECT::default();
            GetClientRect(hwnd, &mut track);
            let horizontal = style & 1 != 0;
            let object = if horizontal {
                OBJID_HSCROLL
            } else {
                OBJID_VSCROLL
            };
            if let Some(bar) = scrollbar_info(list as HWND, object) {
                draw_scrollbar_on_axis(dc, track, &bar, (style / 2) as i32, horizontal);
            } else {
                fill(dc, &track, theme::paper());
            }
            if msg == WM_PAINT {
                EndPaint(hwnd, &paint);
            }
            0
        },
        _ => unsafe { DefSubclassProc(hwnd, msg, w, l) },
    }
}
