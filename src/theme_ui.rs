//! Shared painting for native controls which do not honor application colors.
use crate::{
    native::{scaled, wide},
    theme,
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    UI::{
        Controls::*,
        HiDpi::GetDpiForWindow,
        Input::KeyboardAndMouse::GetFocus,
        Shell::{DefSubclassProc, SetWindowSubclass},
        WindowsAndMessaging::*,
    },
};

pub fn fill(dc: HDC, rect: &RECT, colour: u32) {
    unsafe {
        let brush = CreateSolidBrush(colour);
        FillRect(dc, rect, brush);
        DeleteObject(brush);
    }
}

pub fn line_text(dc: HDC, mut rect: RECT, value: &str, colour: u32, flags: u32) {
    unsafe {
        SetBkMode(dc, TRANSPARENT as i32);
        SetTextColor(dc, colour);
        DrawTextW(
            dc,
            wide(value).as_ptr(),
            -1,
            &mut rect,
            DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS | DT_NOPREFIX | flags,
        );
    }
}

pub fn window_text(hwnd: HWND) -> String {
    unsafe {
        let mut value = vec![0; GetWindowTextLengthW(hwnd).max(0) as usize + 1];
        let n = GetWindowTextW(hwnd, value.as_mut_ptr(), value.len() as i32);
        String::from_utf16_lossy(&value[..n.max(0) as usize])
    }
}

fn rounded(dc: HDC, rect: RECT, radius: i32, colour: u32, border: u32) {
    unsafe {
        let brush = CreateSolidBrush(colour);
        let pen = CreatePen(PS_SOLID, 1, border);
        let old_brush = SelectObject(dc, brush);
        let old_pen = SelectObject(dc, pen);
        RoundRect(
            dc,
            rect.left,
            rect.top,
            rect.right,
            rect.bottom,
            radius,
            radius,
        );
        SelectObject(dc, old_pen);
        SelectObject(dc, old_brush);
        DeleteObject(pen);
        DeleteObject(brush);
    }
}

pub fn button(item: &DRAWITEMSTRUCT, primary: bool) {
    let dc = item.hDC;
    let rect = item.rcItem;
    let pressed = item.itemState & ODS_SELECTED != 0;
    let disabled = item.itemState & ODS_DISABLED != 0;
    let dpi = unsafe { GetDpiForWindow(item.hwndItem) }.max(96) as i32;
    let colour = if disabled {
        theme::surface()
    } else if primary && !pressed {
        theme::accent()
    } else if pressed {
        theme::selection()
    } else {
        theme::control_surface()
    };
    let text = if disabled {
        theme::muted()
    } else if primary && !pressed {
        theme::on_accent()
    } else {
        theme::text()
    };
    fill(dc, &rect, theme::surface());
    rounded(
        dc,
        rect,
        scaled(7, dpi),
        colour,
        if item.itemState & ODS_FOCUS != 0 {
            theme::accent()
        } else {
            theme::border()
        },
    );
    unsafe {
        let font = SendMessageW(item.hwndItem, WM_GETFONT, 0, 0) as HFONT;
        let old = SelectObject(dc, font);
        line_text(dc, rect, &window_text(item.hwndItem), text, DT_CENTER);
        SelectObject(dc, old);
    }
}

pub fn combo(item: &DRAWITEMSTRUCT) {
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
        rect.left += 5;
        rect.right -= 4;
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
            let pen = CreatePen(PS_SOLID, 1, ink);
            let old = SelectObject(dc, pen);
            let x = bounds.right - scaled(14, dpi);
            let y = bounds.bottom / 2;
            let d = scaled(3, dpi).max(2);
            MoveToEx(dc, x - d, y - 1, std::ptr::null_mut());
            LineTo(dc, x, y + d - 1);
            LineTo(dc, x + d + 1, y - 2);
            SelectObject(dc, old);
            DeleteObject(pen);
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

pub fn switch(hwnd: HWND, dpi: i32) {
    unsafe { SetWindowSubclass(hwnd, Some(switch_proc), 1, dpi as usize) };
}

unsafe extern "system" fn switch_proc(
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
            let mut bounds: RECT = std::mem::zeroed();
            GetClientRect(hwnd, &mut bounds);
            let dpi = dpi as i32;
            let checked = DefSubclassProc(hwnd, BM_GETCHECK, 0, 0) == BST_CHECKED as isize;
            let enabled =
                windows_sys::Win32::UI::Input::KeyboardAndMouse::IsWindowEnabled(hwnd) != 0;
            let focused = GetFocus() == hwnd;
            let h = scaled(18, dpi);
            let right = bounds.right - scaled(8, dpi);
            let top = (bounds.bottom - h) / 2;
            let track = RECT {
                left: right - scaled(34, dpi),
                top,
                right,
                bottom: top + h,
            };
            fill(dc, &bounds, theme::control_surface());
            rounded(
                dc,
                track,
                h,
                if checked && enabled {
                    theme::accent()
                } else {
                    theme::paper()
                },
                if focused {
                    theme::accent()
                } else {
                    theme::border()
                },
            );
            let inset = scaled(3, dpi);
            let diameter = h - inset * 2;
            let left = if checked {
                track.right - inset - diameter
            } else {
                track.left + inset
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
            bounds.left += scaled(10, dpi);
            bounds.right = track.left - scaled(7, dpi);
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
            SelectObject(dc, old);
            if focused {
                DrawFocusRect(dc, &bounds);
            }
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
