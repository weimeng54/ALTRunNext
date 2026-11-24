// Copyright (c) 2026 GPT-6.2 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::{Axis, appearance::Visual, scrollbar_info, wide};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    UI::{
        Controls::{
            COMBOBOXINFO, GetComboBoxInfo, STATE_SYSTEM_PRESSED, SetWindowTheme, WM_MOUSELEAVE,
        },
        Input::KeyboardAndMouse::{TME_LEAVE, TME_NONCLIENT, TRACKMOUSEEVENT, TrackMouseEvent},
        Shell::{DefSubclassProc, SetWindowSubclass},
        WindowsAndMessaging::*,
    },
};

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
            SetWindowTheme(info.hwndList, disable_theme.as_ptr(), disable_theme.as_ptr());
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
        let result = DefSubclassProc(hwnd, msg, w, l);
        let mut active = hover != 0;
        match msg {
            WM_NCMOUSEMOVE => {
                active = w == HTVSCROLL as usize;
                if active && hover == 0 {
                    TrackMouseEvent(&mut TRACKMOUSEEVENT {
                        cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                        dwFlags: TME_LEAVE | TME_NONCLIENT,
                        hwndTrack: hwnd,
                        dwHoverTime: 0,
                    });
                }
            }
            WM_NCMOUSELEAVE | WM_MOUSELEAVE => active = false,
            WM_SHOWWINDOW if w == 0 => active = false,
            WM_PRINT if l as i32 & PRF_NONCLIENT != 0 => {
                paint(hwnd, w as HDC, dpi as i32, active);
                return result;
            }
            _ => {}
        }
        let changed = active != (hover != 0);
        if changed {
            SetWindowSubclass(hwnd, Some(procedure), dpi, usize::from(active));
        }
        if IsWindowVisible(hwnd) != 0
            && (changed
                || matches!(
                    msg,
                    WM_NCPAINT
                        | WM_PAINT
                        | WM_VSCROLL
                        | WM_MOUSEWHEEL
                        | WM_NCLBUTTONDOWN
                        | WM_NCMOUSEMOVE
                        | WM_KEYDOWN
                ))
        {
            let dc = GetWindowDC(hwnd);
            if !dc.is_null() {
                paint(hwnd, dc, dpi as i32, active);
                ReleaseDC(hwnd, dc);
            }
        }
        result
    }
}
