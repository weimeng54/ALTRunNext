// Copyright (c) 2026 GPT-6.2 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::{Axis, appearance::Visual, scrollbar_info, wide};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    UI::{
        Controls::{COMBOBOXINFO, GetComboBoxInfo, STATE_SYSTEM_PRESSED, SetWindowTheme},
        Shell::{DefSubclassProc, SetWindowSubclass},
        WindowsAndMessaging::*,
    },
};

const HOVER_TIMER: usize = 0x414c;

pub(in crate::ui) fn style_combo_scrollbar(combo: HWND, dpi: i32) {
    let mut info = COMBOBOXINFO {
        cbSize: size_of::<COMBOBOXINFO>() as u32,
        ..Default::default()
    };
    unsafe {
        if GetComboBoxInfo(combo, &mut info) != 0
            && !info.hwndList.is_null()
            && SetWindowSubclass(info.hwndList, Some(procedure), dpi as usize, 0) != 0
        {
            // Prevent Windows' themed scrollbar animation from repainting over the track.
            let disable_theme = wide("");
            SetWindowTheme(
                info.hwndList,
                disable_theme.as_ptr(),
                disable_theme.as_ptr(),
            );
        }
    }
}

fn paint(hwnd: HWND, dc: HDC, dpi: i32, active: bool) {
    let Some(bar) = scrollbar_info(hwnd, Axis::Vertical) else {
        return;
    };
    let mut window = RECT::default();
    unsafe { GetWindowRect(hwnd, &mut window) };
    let mut bounds = bar.rcScrollBar;
    unsafe { OffsetRect(&mut bounds, -window.left, -window.top) };
    if bar.xyThumbBottom <= bar.xyThumbTop {
        return;
    }
    // The dropdown retains the system's thumb geometry and selection tracking.
    let length = Axis::Vertical.length(bounds);
    Visual {
        track_start: bar.dxyLineButton,
        track_end: length - bar.dxyLineButton,
        thumb_start: bar.xyThumbTop,
        thumb_end: bar.xyThumbBottom,
        active: active || bar.rgstate[3] & STATE_SYSTEM_PRESSED != 0,
    }
    .draw(dc, bounds, Axis::Vertical, dpi);
}

unsafe extern "system" fn procedure(
    hwnd: HWND,
    msg: u32,
    w: WPARAM,
    l: LPARAM,
    dpi: usize,
    hover: usize,
) -> LRESULT {
    unsafe {
        let polling = msg == WM_TIMER && w == HOVER_TIMER;
        let result = if polling {
            0
        } else {
            DefSubclassProc(hwnd, msg, w, l)
        };
        if matches!(msg, WM_WINDOWPOSCHANGED | WM_SHOWWINDOW) {
            let flags = match msg {
                WM_WINDOWPOSCHANGED => (*(l as *const WINDOWPOS)).flags,
                _ if w != 0 => SWP_SHOWWINDOW,
                _ => SWP_HIDEWINDOW,
            };
            if flags & SWP_SHOWWINDOW != 0 {
                // A dropdown can capture the pointer before the scrollbar gets mouse messages.
                SetTimer(hwnd, HOVER_TIMER, 40, None);
            } else if flags & SWP_HIDEWINDOW != 0 {
                KillTimer(hwnd, HOVER_TIMER);
            }
        }
        let repaint = matches!(
            msg,
            WM_NCPAINT | WM_PAINT | WM_VSCROLL | WM_MOUSEWHEEL | WM_NCLBUTTONDOWN | WM_KEYDOWN
        );
        let print = msg == WM_PRINT && l as i32 & PRF_NONCLIENT != 0;
        if !polling && !repaint && !print {
            return result;
        }
        let mut pointer = POINT::default();
        let active = GetCursorPos(&mut pointer) != 0
            && scrollbar_info(hwnd, Axis::Vertical)
                .is_some_and(|bar| PtInRect(&bar.rcScrollBar, pointer) != 0);
        if print {
            paint(hwnd, w as HDC, dpi as i32, active);
        } else if IsWindowVisible(hwnd) != 0 && (repaint || active != (hover != 0)) {
            SetWindowSubclass(hwnd, Some(procedure), dpi, usize::from(active));
            let dc = GetWindowDC(hwnd);
            if !dc.is_null() {
                paint(hwnd, dc, dpi as i32, active);
                ReleaseDC(hwnd, dc);
            }
        }
        result
    }
}
