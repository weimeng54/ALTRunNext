// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::{painting::*, scaled};
use crate::theme;
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    UI::{
        Controls::*,
        Input::KeyboardAndMouse::GetFocus,
        Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
        WindowsAndMessaging::*,
    },
};

pub fn button_on(item: &DRAWITEMSTRUCT, primary: bool, backdrop: u32, dpi: i32) {
    draw_button(item, primary, backdrop, dpi, false);
}

pub fn menu_button(item: &DRAWITEMSTRUCT, dpi: i32) {
    draw_button(item, false, theme::chrome(), dpi, true);
}

fn draw_button(item: &DRAWITEMSTRUCT, primary: bool, backdrop: u32, dpi: i32, menu: bool) {
    let dc = item.hDC;
    let rect = item.rcItem;
    let pressed = item.itemState & ODS_SELECTED != 0;
    let disabled = item.itemState & ODS_DISABLED != 0;
    let colour = if disabled && primary && !theme::high_contrast() {
        if theme::is_dark() {
            theme::rgb(32, 38, 46)
        } else {
            theme::rgb(227, 234, 242)
        }
    } else if disabled {
        theme::surface()
    } else if primary && !pressed {
        theme::accent()
    } else if pressed {
        theme::selection()
    } else {
        theme::control_surface()
    };
    let text = if disabled && primary && !theme::high_contrast() {
        if theme::is_dark() {
            theme::rgb(139, 152, 166)
        } else {
            theme::rgb(105, 122, 142)
        }
    } else if disabled {
        theme::muted()
    } else if primary && !pressed {
        theme::on_accent()
    } else {
        theme::text()
    };
    fill(dc, &rect, backdrop);
    rounded(
        dc,
        rect,
        scaled(12, dpi),
        colour,
        if disabled && primary {
            theme::divider()
        } else if item.itemState & ODS_FOCUS != 0 {
            theme::accent()
        } else {
            theme::border()
        },
    );
    unsafe {
        let font = SendMessageW(item.hwndItem, WM_GETFONT, 0, 0) as HFONT;
        let old = SelectObject(dc, font);
        let mut label = rect;
        if menu {
            label.right -= scaled(16, dpi);
            chevron(
                dc,
                POINT {
                    x: rect.right - scaled(13, dpi),
                    y: (rect.top + rect.bottom) / 2,
                },
                Chevron::Down,
                dpi,
                text,
            );
        }
        line_text(dc, label, &window_text(item.hwndItem), text, DT_CENTER);
        SelectObject(dc, old);
    }
}

pub fn style_caption_close(hwnd: HWND) {
    unsafe { SetWindowSubclass(hwnd, Some(caption_close_proc), 1, 0) };
}

unsafe extern "system" fn caption_close_proc(
    hwnd: HWND,
    msg: u32,
    w: WPARAM,
    l: LPARAM,
    _: usize,
    _: usize,
) -> LRESULT {
    unsafe {
        if msg == WM_MOUSEMOVE {
            use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
            TrackMouseEvent(&mut TRACKMOUSEEVENT {
                cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                dwFlags: TME_LEAVE,
                hwndTrack: hwnd,
                dwHoverTime: 0,
            });
        }
        let result = DefSubclassProc(hwnd, msg, w, l);
        if matches!(
            msg,
            WM_MOUSEMOVE | WM_MOUSELEAVE | WM_KILLFOCUS | WM_SETFOCUS | WM_UPDATEUISTATE
        ) {
            InvalidateRect(hwnd, std::ptr::null(), 0);
        }
        result
    }
}

pub fn caption_close(item: &DRAWITEMSTRUCT, dpi: i32) {
    let rect = item.rcItem;
    unsafe {
        let active = GetForegroundWindow() == GetParent(item.hwndItem);
        let focused = active
            && item.itemState & ODS_FOCUS != 0
            && SendMessageW(item.hwndItem, WM_QUERYUISTATE, 0, 0) as u32 & UISF_HIDEFOCUS == 0;
        let mut cursor = POINT::default();
        let hovered =
            active && GetCursorPos(&mut cursor) != 0 && WindowFromPoint(cursor) == item.hwndItem;
        fill(
            item.hDC,
            &rect,
            if hovered {
                theme::selection()
            } else {
                theme::chrome()
            },
        );
        if focused {
            let mut bounds = rect;
            InflateRect(&mut bounds, -scaled(4, dpi), -scaled(4, dpi));
            rounded(
                item.hDC,
                bounds,
                scaled(6, dpi),
                if hovered {
                    theme::selection()
                } else {
                    theme::chrome()
                },
                theme::accent(),
            );
        }
        let x = (rect.left + rect.right) / 2;
        let y = (rect.top + rect.bottom) / 2;
        let r = scaled(5, dpi).max(3);
        let pen = CreatePen(PS_SOLID, scaled(1, dpi).max(1), theme::text());
        let old = SelectObject(item.hDC, pen);
        for (from, to) in [(x - r, x + r), (x + r, x - r)] {
            MoveToEx(item.hDC, from, y - r, std::ptr::null_mut());
            LineTo(item.hDC, to, y + r);
        }
        SelectObject(item.hDC, old);
        DeleteObject(pen);
    }
}

#[derive(Clone, Copy)]
struct SwitchStyle {
    dpi: i32,
    description: Option<(&'static str, HFONT)>,
}

pub fn switch(hwnd: HWND, dpi: i32, description: Option<(&'static str, HFONT)>) {
    let style = Box::new(SwitchStyle { dpi, description });
    let data = Box::into_raw(style);
    unsafe {
        if SetWindowSubclass(hwnd, Some(switch_proc), 1, data as usize) == 0 {
            drop(Box::from_raw(data));
        }
    }
}

fn switch_track(bounds: RECT, dpi: i32) -> RECT {
    let height = scaled(20, dpi);
    let right = bounds.right - scaled(9, dpi);
    let top = (bounds.bottom - height) / 2;
    RECT {
        left: right - scaled(35, dpi),
        top,
        right,
        bottom: top + height,
    }
}

unsafe extern "system" fn switch_proc(
    hwnd: HWND,
    msg: u32,
    w: WPARAM,
    l: LPARAM,
    id: usize,
    data: usize,
) -> LRESULT {
    unsafe {
        let style = data as *mut SwitchStyle;
        if msg == WM_NCDESTROY {
            RemoveWindowSubclass(hwnd, Some(switch_proc), id);
            drop(Box::from_raw(style));
            return DefSubclassProc(hwnd, msg, w, l);
        }
        let SwitchStyle { dpi, description } = *style;
        if matches!(
            msg,
            WM_NCHITTEST
                | WM_LBUTTONDOWN
                | WM_LBUTTONDBLCLK
                | WM_LBUTTONUP
                | WM_MOUSEMOVE
                | BM_CLICK
        ) {
            let mut bounds = RECT::default();
            GetClientRect(hwnd, &mut bounds);
            let track = switch_track(bounds, dpi);
            if msg == BM_CLICK {
                // Native button automation should click the track, not the row origin.
                let x = (track.left + track.right) / 2;
                let y = (track.top + track.bottom) / 2;
                let center = (x | (y << 16)) as LPARAM;
                DefSubclassProc(hwnd, WM_LBUTTONDOWN, 1, center);
                return DefSubclassProc(hwnd, WM_LBUTTONUP, 0, center);
            }
            let mut point = POINT {
                x: l as i16 as i32,
                y: (l >> 16) as i16 as i32,
            };
            if msg == WM_NCHITTEST {
                ScreenToClient(hwnd, &mut point);
            }
            let inside = (track.left..track.right).contains(&point.x)
                && (track.top..track.bottom).contains(&point.y);
            if !inside {
                return match msg {
                    WM_NCHITTEST => HTTRANSPARENT as LRESULT,
                    WM_LBUTTONUP => DefSubclassProc(hwnd, WM_CANCELMODE, 0, 0),
                    WM_MOUSEMOVE => DefSubclassProc(hwnd, msg, w, -1),
                    _ => 0,
                };
            }
        }
        if matches!(msg, WM_PAINT | WM_PRINTCLIENT) {
            let mut paint: PAINTSTRUCT = std::mem::zeroed();
            let dc = if msg == WM_PAINT {
                BeginPaint(hwnd, &mut paint)
            } else {
                w as HDC
            };
            let mut bounds: RECT = std::mem::zeroed();
            GetClientRect(hwnd, &mut bounds);
            let px = |n| scaled(n, dpi);
            let checked = DefSubclassProc(hwnd, BM_GETCHECK, 0, 0) == BST_CHECKED as isize;
            let enabled =
                windows_sys::Win32::UI::Input::KeyboardAndMouse::IsWindowEnabled(hwnd) != 0;
            let focused = GetFocus() == hwnd;
            let track = switch_track(bounds, dpi);
            let top = track.top;
            let h = track.bottom - track.top;
            fill(dc, &bounds, theme::control_surface());
            if focused && DefSubclassProc(hwnd, WM_QUERYUISTATE, 0, 0) as u32 & UISF_HIDEFOCUS == 0
            {
                let mut outline = track;
                InflateRect(&mut outline, px(5), px(5));
                rounded(dc, outline, px(30), theme::accent(), theme::accent());
                InflateRect(&mut outline, -px(2), -px(2));
                rounded(
                    dc,
                    outline,
                    px(26),
                    theme::control_surface(),
                    theme::control_surface(),
                );
            }
            rounded(
                dc,
                track,
                h,
                if checked && enabled {
                    theme::accent()
                } else {
                    theme::paper()
                },
                if focused || (checked && enabled) {
                    theme::accent()
                } else {
                    theme::border()
                },
            );
            let diameter = px(12);
            let inset = (h - diameter) / 2;
            let side = px(3);
            let left = if checked {
                track.right - side - diameter
            } else {
                track.left + side
            };
            let knob = RECT {
                left,
                top: top + inset,
                right: left + diameter,
                bottom: top + inset + diameter,
            };
            let ink = if checked && enabled {
                theme::on_accent()
            } else {
                theme::muted()
            };
            rounded(dc, knob, diameter, ink, ink);
            let font = DefSubclassProc(hwnd, WM_GETFONT, 0, 0) as HFONT;
            let old = SelectObject(dc, font);
            bounds.left += px(9);
            bounds.right = track.left - px(7);
            if description.is_some() {
                bounds.bottom = px(30);
            }
            line_text(
                dc,
                bounds,
                &window_text(hwnd),
                if enabled {
                    theme::text()
                } else {
                    theme::muted()
                },
                0,
            );
            if let Some((caption, font)) = description {
                SelectObject(dc, font);
                let detail = RECT {
                    top: px(27),
                    bottom: px(48),
                    ..bounds
                };
                line_text(
                    dc,
                    detail,
                    &crate::localization::tr(caption),
                    theme::muted(),
                    0,
                );
            }
            SelectObject(dc, old);
            if msg == WM_PAINT {
                EndPaint(hwnd, &paint);
            }
            return 0;
        }
        let result = DefSubclassProc(hwnd, msg, w, l);
        if matches!(
            msg,
            BM_SETCHECK | BM_SETSTATE | WM_SETFOCUS | WM_KILLFOCUS | WM_ENABLE
        ) {
            InvalidateRect(hwnd, std::ptr::null(), 0);
        }
        result
    }
}
