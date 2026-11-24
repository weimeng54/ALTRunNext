// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::{
    painting::{fill, rounded},
    scaled, wide,
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    UI::{
        Controls::*,
        Shell::{DefSubclassProc, SetWindowSubclass},
        WindowsAndMessaging::*,
    },
};

mod appearance;
mod combo;
mod overlay;

pub(super) use combo::style_combo_scrollbar;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Axis {
    Vertical,
    Horizontal,
}

impl Axis {
    fn object(self) -> OBJECT_IDENTIFIER {
        match self {
            Self::Vertical => OBJID_VSCROLL,
            Self::Horizontal => OBJID_HSCROLL,
        }
    }

    fn scroll_bar(self) -> SCROLLBAR_CONSTANTS {
        match self {
            Self::Vertical => SB_VERT,
            Self::Horizontal => SB_HORZ,
        }
    }

    fn length(self, bounds: RECT) -> i32 {
        match self {
            Self::Vertical => bounds.bottom - bounds.top,
            Self::Horizontal => bounds.right - bounds.left,
        }
    }

    fn cross_length(self, bounds: RECT) -> i32 {
        match self {
            Self::Vertical => bounds.right - bounds.left,
            Self::Horizontal => bounds.bottom - bounds.top,
        }
    }

    fn mouse_position(self, l: LPARAM) -> i32 {
        let coordinate = match self {
            Self::Vertical => l >> 16,
            Self::Horizontal => l,
        };
        coordinate as i16 as i32
    }

    fn centered(self, bounds: RECT, width: i32, start: i32, end: i32) -> RECT {
        let offset = (self.cross_length(bounds) - width) / 2;
        match self {
            Self::Vertical => RECT {
                left: bounds.left + offset,
                top: bounds.top + start,
                right: bounds.left + offset + width,
                bottom: bounds.top + end,
            },
            Self::Horizontal => RECT {
                left: bounds.left + start,
                top: bounds.top + offset,
                right: bounds.left + end,
                bottom: bounds.top + offset + width,
            },
        }
    }
}

fn scrollbar_info(hwnd: HWND, axis: Axis) -> Option<SCROLLBARINFO> {
    let mut bar = SCROLLBARINFO {
        cbSize: size_of::<SCROLLBARINFO>() as u32,
        ..Default::default()
    };
    unsafe {
        (GetScrollBarInfo(hwnd, axis.object(), &mut bar) != 0
            && bar.rgstate[0] & STATE_SYSTEM_INVISIBLE == 0)
            .then_some(bar)
    }
}

#[derive(Clone, Copy)]
enum Target {
    List,
    ReportRows,
    ReportColumns,
    Text,
}

impl Target {
    fn axis(self) -> Axis {
        if matches!(self, Self::ReportColumns) {
            Axis::Horizontal
        } else {
            Axis::Vertical
        }
    }

    fn decode(style: usize) -> Self {
        match style % 4 {
            0 => Self::List,
            1 => Self::ReportRows,
            2 => Self::ReportColumns,
            _ => Self::Text,
        }
    }

    fn set_position(self, control: HWND, minimum: i32, current: i32, position: i32) {
        let delta = position - current;
        unsafe {
            match self {
                Self::List => {
                    SendMessageW(control, LB_SETTOPINDEX, (minimum + position) as usize, 0);
                }
                Self::ReportRows => {
                    let mut row = RECT::default();
                    if SendMessageW(control, LVM_GETITEMRECT, 0, &mut row as *mut RECT as LPARAM)
                        != 0
                    {
                        let dy = delta.saturating_mul(row.bottom - row.top);
                        SendMessageW(control, LVM_SCROLL, 0, dy as LPARAM);
                    }
                }
                Self::ReportColumns => {
                    SendMessageW(control, LVM_SCROLL, delta as usize, 0);
                }
                Self::Text => {
                    SendMessageW(control, EM_LINESCROLL, 0, delta as LPARAM);
                }
            }
        }
    }
}

pub(crate) fn style_list_scrollbar(control: HWND, id: usize, dpi: i32) {
    style_scrollbar(control, id, Target::List, dpi);
}

pub(crate) fn style_text_scrollbar(control: HWND, id: usize, dpi: i32) {
    style_scrollbar(control, id, Target::Text, dpi);
}

pub(crate) fn style_report_scrollbars(control: HWND, vertical: usize, horizontal: usize, dpi: i32) {
    style_scrollbar(control, vertical, Target::ReportRows, dpi);
    style_scrollbar(control, horizontal, Target::ReportColumns, dpi);
}

fn style_scrollbar(control: HWND, id: usize, target: Target, dpi: i32) {
    let style = dpi as usize * 4 + target as usize;
    let horizontal = target.axis() == Axis::Horizontal;
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
        if scrollbar.is_null() {
            return;
        }
        if SetWindowSubclass(scrollbar, Some(overlay::procedure), style, control as usize) == 0
            || SetWindowSubclass(
                control,
                Some(scroll_control_proc),
                1 + usize::from(horizontal),
                scrollbar as usize,
            ) == 0
        {
            DestroyWindow(scrollbar);
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
    let scrollbar = scrollbar as HWND;
    if msg == WM_NCDESTROY {
        unsafe { DestroyWindow(scrollbar) };
    }
    let result = unsafe { DefSubclassProc(hwnd, msg, w, l) };
    if matches!(
        msg,
        WM_NCPAINT
            | WM_PAINT
            | WM_VSCROLL
            | WM_HSCROLL
            | WM_MOUSEWHEEL
            | WM_MOUSEHWHEEL
            | WM_KEYDOWN
            | WM_NCLBUTTONDOWN
            | WM_SIZE
            | WM_SHOWWINDOW
            | WM_WINDOWPOSCHANGED
            | LB_SETTOPINDEX
            | LB_SETCURSEL
            | EM_LINESCROLL
            | EM_SCROLL
            | EM_SCROLLCARET
            | WM_SETTEXT
            | LVM_SCROLL
            | LVM_SETCOLUMNWIDTH
            | LVM_ENSUREVISIBLE
    ) {
        let axis = if subclass == 2 {
            Axis::Horizontal
        } else {
            Axis::Vertical
        };
        update_scrollbar(hwnd, scrollbar, axis);
    }
    result
}

fn update_scrollbar(control: HWND, scrollbar: HWND, axis: Axis) {
    unsafe {
        let visible = GetWindowLongW(scrollbar, GWL_STYLE) as u32 & WS_VISIBLE != 0;
        let bar = scrollbar_info(control, axis)
            .filter(|_| GetWindowLongW(control, GWL_STYLE) as u32 & WS_VISIBLE != 0);
        let Some(bar) = bar else {
            if visible {
                ShowWindow(scrollbar, SW_HIDE);
            }
            return;
        };
        let mut bounds = RECT::default();
        GetWindowRect(scrollbar, &mut bounds);
        let mut track = bar.rcScrollBar;
        if axis == Axis::Horizontal
            && let Some(vertical) = scrollbar_info(control, Axis::Vertical)
        {
            // Cover the native size box. The horizontal thumb ends before it.
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
        RedrawWindow(
            scrollbar,
            std::ptr::null(),
            std::ptr::null_mut(),
            RDW_INVALIDATE | RDW_UPDATENOW,
        );
    }
}
