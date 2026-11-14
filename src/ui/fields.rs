// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::{painting::*, scaled};
use crate::theme;
use std::ptr::null_mut;
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

pub fn combo(item: &DRAWITEMSTRUCT, dpi: i32) {
    let selected = item.itemState & ODS_SELECTED != 0;
    let dc = item.hDC;
    let mut rect = item.rcItem;
    fill(
        dc,
        &rect,
        if selected {
            theme::selection()
        } else {
            theme::paper()
        },
    );
    if item.itemID == u32::MAX {
        return;
    }
    unsafe {
        let hwnd = item.hwndItem;
        let n = SendMessageW(hwnd, CB_GETLBTEXTLEN, item.itemID as usize, 0);
        if n < 0 {
            return;
        }
        let mut label = vec![0u16; n as usize + 1];
        let n = SendMessageW(
            hwnd,
            CB_GETLBTEXT,
            item.itemID as usize,
            label.as_mut_ptr() as isize,
        );
        if n < 0 {
            return;
        }
        let old = SelectObject(dc, SendMessageW(hwnd, WM_GETFONT, 0, 0) as HFONT);
        rect.left += scaled(5, dpi);
        rect.right -= scaled(4, dpi);
        line_text(
            dc,
            rect,
            &String::from_utf16_lossy(&label[..n as usize]),
            theme::text(),
            0,
        );
        SelectObject(dc, old);
        if item.itemState & ODS_FOCUS != 0 {
            DrawFocusRect(dc, &rect);
        }
    }
}

pub fn style_combo(hwnd: HWND, dpi: i32) {
    unsafe { SetWindowSubclass(hwnd, Some(combo_proc), 1, dpi as usize) };
}

#[derive(Clone, Copy)]
struct EditStyle {
    dpi: i32,
    number: bool,
    colours: Option<fn() -> (u32, u32)>,
    cue: Option<&'static str>,
}

pub fn style_edit(hwnd: HWND, dpi: i32) {
    set_edit_style(
        hwnd,
        EditStyle {
            dpi,
            number: false,
            colours: None,
            cue: None,
        },
    );
}

pub fn style_edit_with_colours(hwnd: HWND, dpi: i32, colours: fn() -> (u32, u32)) {
    set_edit_style(
        hwnd,
        EditStyle {
            dpi,
            number: false,
            colours: Some(colours),
            cue: None,
        },
    );
}

pub fn style_search(hwnd: HWND, dpi: i32, cue: &'static str, colours: fn() -> (u32, u32)) {
    unsafe {
        SendMessageW(hwnd, EM_SETCUEBANNER, 0, super::ui(cue).as_ptr() as LPARAM);
    }
    set_edit_style(
        hwnd,
        EditStyle {
            dpi,
            number: false,
            colours: Some(colours),
            cue: Some(cue),
        },
    );
}

pub const NUMBER_STEP: u32 = 0x670;

pub fn style_number(hwnd: HWND, dpi: i32) {
    set_edit_style(
        hwnd,
        EditStyle {
            dpi,
            number: true,
            colours: None,
            cue: None,
        },
    );
}

fn number_arrow(rect: RECT, dpi: i32) -> RECT {
    RECT {
        left: rect.right - scaled(25, dpi),
        right: rect.right - scaled(6, dpi),
        top: rect.top + scaled(5, dpi),
        bottom: rect.bottom - scaled(5, dpi),
    }
}

fn draw_arrows(dc: HDC, rect: RECT, dpi: i32) {
    let well = number_arrow(rect, dpi);
    let center = (well.left + well.right) / 2;
    let middle = (well.top + well.bottom) / 2;
    let offset = scaled(4, dpi);
    for (y, direction) in [
        (middle - offset, Chevron::Up),
        (middle + offset, Chevron::Down),
    ] {
        chevron(dc, POINT { x: center, y }, direction, dpi, theme::muted());
    }
}

fn set_edit_style(hwnd: HWND, style: EditStyle) {
    let data = Box::into_raw(Box::new(style));
    unsafe {
        if SetWindowSubclass(hwnd, Some(edit_proc), 1, data as usize) == 0 {
            drop(Box::from_raw(data));
        }
        SetWindowPos(
            hwnd,
            std::ptr::null_mut(),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED | SWP_NOACTIVATE,
        );
    }
}

unsafe fn search_inset(hwnd: HWND, height: i32, dpi: i32) -> i32 {
    unsafe {
        let dc = GetDC(hwnd);
        let font = SendMessageW(hwnd, WM_GETFONT, 0, 0) as HFONT;
        let old = SelectObject(dc, font);
        let mut metrics = TEXTMETRICW::default();
        let measured = GetTextMetricsW(dc, &mut metrics) != 0;
        SelectObject(dc, old);
        ReleaseDC(hwnd, dc);
        if measured {
            ((height - metrics.tmHeight) / 2).max(0)
        } else {
            scaled(4, dpi)
        }
    }
}

unsafe extern "system" fn edit_proc(
    hwnd: HWND,
    msg: u32,
    w: WPARAM,
    l: LPARAM,
    id: usize,
    data: usize,
) -> LRESULT {
    let style = data as *mut EditStyle;
    if msg == WM_NCDESTROY {
        unsafe {
            RemoveWindowSubclass(hwnd, Some(edit_proc), id);
            drop(Box::from_raw(style));
            return DefSubclassProc(hwnd, msg, w, l);
        }
    }
    let EditStyle {
        dpi,
        number,
        colours,
        cue,
    } = unsafe { *style };
    let inset_x = scaled(9, dpi);
    let inset_y = scaled(7, dpi);
    let inset_right = if number { scaled(27, dpi) } else { inset_x };
    unsafe {
        if number {
            let delta = if msg == WM_KEYDOWN && matches!(w as u16, 0x26 | 0x28) {
                Some(if w as u16 == 0x26 { 1 } else { -1 })
            } else if matches!(msg, WM_NCHITTEST | WM_NCLBUTTONDOWN | WM_NCLBUTTONDBLCLK) {
                let mut rect = RECT::default();
                GetWindowRect(hwnd, &mut rect);
                let well = number_arrow(rect, dpi);
                let point = POINT {
                    x: l as i16 as i32,
                    y: (l >> 16) as i16 as i32,
                };
                if PtInRect(&well, point) != 0 {
                    if msg == WM_NCHITTEST {
                        return HTBORDER as isize;
                    }
                    windows_sys::Win32::UI::Input::KeyboardAndMouse::SetFocus(hwnd);
                    Some(if point.y < (well.top + well.bottom) / 2 {
                        1
                    } else {
                        -1
                    })
                } else {
                    None
                }
            } else {
                None
            };
            if let Some(delta) = delta {
                let id = GetDlgCtrlID(hwnd) as usize;
                SendMessageW(
                    GetParent(hwnd),
                    WM_COMMAND,
                    id | ((NUMBER_STEP as usize) << 16),
                    delta,
                );
                return 0;
            }
            if msg == WM_GETDLGCODE {
                return DefSubclassProc(hwnd, msg, w, l) | DLGC_WANTARROWS as isize;
            }
        }
        if msg == WM_NCCALCSIZE && l != 0 {
            let rect = &mut *(l as *mut RECT);
            let available = (rect.right - rect.left - 1).max(0);
            let x = inset_x.min(available / 2);
            let height = rect.bottom - rect.top;
            let y = if cue.is_some() {
                search_inset(hwnd, height, dpi)
            } else {
                inset_y
            }
            .min((height - 1).max(0) / 2);
            rect.left += x;
            rect.right -= inset_right.min(available - x);
            rect.top += y;
            rect.bottom -= y;
            return 0;
        }
        if msg == WM_NCPAINT {
            let dc = GetWindowDC(hwnd);
            if !dc.is_null() {
                let mut rect: RECT = std::mem::zeroed();
                GetWindowRect(hwnd, &mut rect);
                let mut client = RECT::default();
                GetClientRect(hwnd, &mut client);
                MapWindowPoints(hwnd, null_mut(), (&raw mut client).cast::<POINT>(), 2);
                OffsetRect(&mut client, -rect.left, -rect.top);
                OffsetRect(&mut rect, -rect.left, -rect.top);
                ExcludeClipRect(dc, client.left, client.top, client.right, client.bottom);
                let (backdrop, paper) = colours.map_or_else(
                    || (theme::control_surface(), theme::paper()),
                    |colours| colours(),
                );
                fill(dc, &rect, backdrop);
                rounded(
                    dc,
                    rect,
                    scaled(12, dpi),
                    paper,
                    if GetFocus() == hwnd {
                        theme::accent()
                    } else {
                        theme::border()
                    },
                );
                if number {
                    draw_arrows(dc, rect, dpi);
                }
                ReleaseDC(hwnd, dc);
            }
            return 0;
        }
        let result = DefSubclassProc(hwnd, msg, w, l);
        if let Some(cue) = cue
            && matches!(msg, WM_PAINT | WM_PRINTCLIENT)
            && GetFocus() != hwnd
            && GetWindowTextLengthW(hwnd) == 0
        {
            let dc = if msg == WM_PAINT {
                GetDC(hwnd)
            } else {
                w as HDC
            };
            let mut bounds = RECT::default();
            GetClientRect(hwnd, &mut bounds);
            if !dc.is_null() {
                let old = SelectObject(dc, SendMessageW(hwnd, WM_GETFONT, 0, 0) as HFONT);
                fill(
                    dc,
                    &bounds,
                    colours.map_or_else(theme::paper, |colours| colours().1),
                );
                line_text(dc, bounds, &crate::localization::tr(cue), theme::muted(), 0);
                SelectObject(dc, old);
                if msg == WM_PAINT {
                    ReleaseDC(hwnd, dc);
                }
            }
        }
        if matches!(msg, WM_SETFOCUS | WM_KILLFOCUS | WM_ENABLE) {
            RedrawWindow(
                hwnd,
                std::ptr::null(),
                std::ptr::null_mut(),
                RDW_INVALIDATE | RDW_FRAME,
            );
        }
        result
    }
}

pub fn style_slider(hwnd: HWND, dpi: i32) {
    unsafe {
        SetWindowSubclass(hwnd, Some(slider_proc), 1, dpi as usize);
    }
}

unsafe extern "system" fn slider_proc(
    hwnd: HWND,
    msg: u32,
    w: WPARAM,
    l: LPARAM,
    _: usize,
    dpi: usize,
) -> LRESULT {
    unsafe {
        if matches!(msg, WM_PAINT | WM_PRINTCLIENT) {
            let mut paint: PAINTSTRUCT = std::mem::zeroed();
            let dc = if msg == WM_PAINT {
                BeginPaint(hwnd, &mut paint)
            } else {
                w as HDC
            };
            let mut rect: RECT = std::mem::zeroed();
            GetClientRect(hwnd, &mut rect);
            let dpi = dpi as i32;
            fill(dc, &rect, theme::control_surface());
            let enabled =
                windows_sys::Win32::UI::Input::KeyboardAndMouse::IsWindowEnabled(hwnd) != 0;
            let mut thumb: RECT = std::mem::zeroed();
            DefSubclassProc(hwnd, TBM_GETTHUMBRECT, 0, &mut thumb as *mut _ as isize);
            let x = ((thumb.left + thumb.right) / 2).clamp(
                scaled(6, dpi),
                (rect.right - scaled(6, dpi)).max(scaled(6, dpi)),
            );
            let y = rect.bottom / 2;
            let left = scaled(1, dpi);
            let right = (rect.right - left).max(left + 1);
            let width = scaled(8, dpi).max(2);
            let track = RECT {
                left,
                top: y - width / 2,
                right,
                bottom: y + width / 2,
            };
            rounded(
                dc,
                track,
                width,
                theme::paper(),
                if enabled {
                    theme::border()
                } else {
                    theme::divider()
                },
            );
            let ink = if enabled {
                theme::accent()
            } else if theme::high_contrast() {
                theme::muted()
            } else {
                theme::rgb(89, 94, 101)
            };
            if x > left {
                rounded(dc, RECT { right: x, ..track }, width, ink, ink);
            }
            let radius = scaled(7, dpi);
            rounded(
                dc,
                RECT {
                    left: x - radius,
                    top: y - radius,
                    right: x + radius,
                    bottom: y + radius,
                },
                2 * radius,
                ink,
                ink,
            );
            if GetFocus() == hwnd {
                DrawFocusRect(dc, &rect);
            }
            if msg == WM_PAINT {
                EndPaint(hwnd, &paint);
            }
            return 0;
        }
        let result = DefSubclassProc(hwnd, msg, w, l);
        if matches!(
            msg,
            TBM_SETPOS
                | WM_MOUSEMOVE
                | WM_LBUTTONDOWN
                | WM_LBUTTONUP
                | WM_KEYDOWN
                | WM_SETFOCUS
                | WM_KILLFOCUS
                | WM_ENABLE
        ) {
            InvalidateRect(hwnd, std::ptr::null(), 0);
        }
        result
    }
}

unsafe extern "system" fn combo_proc(
    hwnd: HWND,
    msg: u32,
    w: WPARAM,
    l: LPARAM,
    _: usize,
    dpi: usize,
) -> LRESULT {
    unsafe {
        if msg == WM_ERASEBKGND {
            return 1;
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
            let dpi = dpi as i32;
            let enabled =
                windows_sys::Win32::UI::Input::KeyboardAndMouse::IsWindowEnabled(hwnd) != 0;
            let focused = GetFocus() == hwnd;
            fill(dc, &bounds, theme::surface());
            rounded(
                dc,
                bounds,
                scaled(6, dpi),
                theme::paper(),
                if focused {
                    theme::accent()
                } else {
                    theme::border()
                },
            );
            let ink = if enabled {
                theme::text()
            } else {
                theme::muted()
            };
            let font = DefSubclassProc(hwnd, WM_GETFONT, 0, 0) as HFONT;
            let old = SelectObject(dc, font);
            let label = RECT {
                left: scaled(8, dpi),
                right: bounds.right - scaled(25, dpi),
                ..bounds
            };
            line_text(dc, label, &window_text(hwnd), ink, 0);
            SelectObject(dc, old);
            chevron(
                dc,
                POINT {
                    x: bounds.right - scaled(14, dpi),
                    y: bounds.bottom / 2,
                },
                Chevron::Down,
                dpi,
                ink,
            );
            if msg == WM_PAINT {
                EndPaint(hwnd, &paint);
            }
            return 0;
        }
        let result = DefSubclassProc(hwnd, msg, w, l);
        if matches!(
            msg,
            CB_SETCURSEL
                | CB_SHOWDROPDOWN
                | WM_SETFOCUS
                | WM_KILLFOCUS
                | WM_ENABLE
                | WM_KEYDOWN
                | WM_LBUTTONUP
        ) {
            InvalidateRect(hwnd, std::ptr::null(), 0);
        }
        result
    }
}

pub fn slider(item: &NMCUSTOMDRAW) -> LRESULT {
    let dc = item.hdc;
    let mut rect = item.rc;
    if item.dwDrawStage == CDDS_PREPAINT {
        unsafe {
            GetClientRect(item.hdr.hwndFrom, &mut rect);
        }
        fill(dc, &rect, theme::surface());
        return CDRF_NOTIFYITEMDRAW as isize;
    }
    if item.dwDrawStage == CDDS_ITEMPREPAINT {
        match item.dwItemSpec as u32 {
            TBCD_CHANNEL => {
                let middle = (rect.top + rect.bottom) / 2;
                rect.top = middle - 2;
                rect.bottom = middle + 2;
                rounded(dc, rect, 4, theme::selection(), theme::border());
                return CDRF_SKIPDEFAULT as isize;
            }
            TBCD_THUMB => {
                let size = (rect.right - rect.left).min(rect.bottom - rect.top).max(5);
                let x = (rect.left + rect.right - size) / 2;
                let y = (rect.top + rect.bottom - size) / 2;
                let thumb = RECT {
                    left: x,
                    top: y,
                    right: x + size,
                    bottom: y + size,
                };
                let color = if item.uItemState & CDIS_DISABLED == 0 {
                    theme::accent()
                } else {
                    theme::border()
                };
                rounded(dc, thumb, size, color, color);
                return CDRF_SKIPDEFAULT as isize;
            }
            _ => {}
        }
    }
    CDRF_DODEFAULT as isize
}
