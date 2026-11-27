// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

//! Window chrome shared by the settings, manager and configuration dialogs.
use super::{geometry, scaled, window_region};
use crate::theme;
use windows_sys::Win32::{
    Foundation::*,
    Graphics::{Dwm::*, Gdi::*},
    UI::{
        Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
        WindowsAndMessaging::*,
    },
};

// The native flat border reserves non-client space for the outline. Child
// controls, including the caption close button, always stay inside that space.
pub(crate) const STYLE: u32 = WS_POPUP | WS_SYSMENU | WS_BORDER;

pub(crate) fn fit(hwnd: HWND, width: i32, height: i32) -> i32 {
    theme::watch(hwnd);
    let dpi = geometry::fit(hwnd, width, height);
    unsafe {
        if GetWindowLongW(hwnd, GWL_STYLE) as u32 & WS_CAPTION == WS_CAPTION {
            // Native-caption dialogs keep DWM caption buttons and hit testing.
            return dpi;
        }
        let diameter = scaled(22, dpi);
        if SetWindowSubclass(hwnd, Some(procedure), 1, diameter as usize) != 0 {
            let policy = DWMNCRP_DISABLED;
            DwmSetWindowAttribute(
                hwnd,
                DWMWA_NCRENDERING_POLICY as u32,
                (&raw const policy).cast(),
                std::mem::size_of_val(&policy) as u32,
            );
            window_region::shape(hwnd, diameter);
        }
    }
    dpi
}

unsafe extern "system" fn procedure(
    hwnd: HWND,
    msg: u32,
    w: WPARAM,
    l: LPARAM,
    id: usize,
    diameter: usize,
) -> LRESULT {
    unsafe {
        match msg {
            WM_PAINT | WM_NCPAINT | WM_NCACTIVATE => {
                let result = if msg == WM_PAINT {
                    DefSubclassProc(hwnd, msg, w, l)
                } else {
                    1
                };
                let dc = GetWindowDC(hwnd);
                if !dc.is_null() {
                    window_region::draw(hwnd, dc, diameter as i32, theme::window_outline());
                    ReleaseDC(hwnd, dc);
                }
                return result;
            }
            WM_SIZE => {
                let result = DefSubclassProc(hwnd, msg, w, l);
                if w != SIZE_MINIMIZED as usize {
                    window_region::shape(hwnd, diameter as i32);
                }
                return result;
            }
            WM_PRINT if w != 0 && l as i32 & PRF_NONCLIENT != 0 => {
                let result = DefSubclassProc(hwnd, msg, w, l);
                window_region::draw(hwnd, w as HDC, diameter as i32, theme::window_outline());
                return result;
            }
            WM_NCDESTROY => {
                RemoveWindowSubclass(hwnd, Some(procedure), id);
            }
            _ => {}
        }
        DefSubclassProc(hwnd, msg, w, l)
    }
}
