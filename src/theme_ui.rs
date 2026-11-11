//! Shared painting for native controls which do not honor application colors.
use crate::{
    native::{scaled, wide},
    theme,
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::{Gdi::*, GdiPlus::*},
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

pub fn rounded(dc: HDC, rect: RECT, radius: i32, colour: u32, border: u32) {
    if smooth_round(dc, rect, radius, colour, border) {
        return;
    }
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

struct GdiPlus(usize);
impl Drop for GdiPlus {
    fn drop(&mut self) {
        if self.0 != 0 {
            unsafe { GdiplusShutdown(self.0) };
        }
    }
}

thread_local! {
    static GDIPLUS: GdiPlus = {
        let mut token = 0;
        let input = GdiplusStartupInput { GdiplusVersion: 1, ..Default::default() };
        unsafe { GdiplusStartup(&mut token,&input,std::ptr::null_mut()); }
        GdiPlus(token)
    };
}

fn smooth_round(dc: HDC, rect: RECT, diameter: i32, colour: u32, border: u32) -> bool {
    if GDIPLUS.with(|token| token.0 == 0) {
        return false;
    }
    let mut graphics = std::ptr::null_mut();
    let mut path = std::ptr::null_mut();
    let mut brush = std::ptr::null_mut();
    let mut pen = std::ptr::null_mut();
    let argb = |c: u32| 0xff000000 | ((c & 255) << 16) | (c & 0xff00) | ((c >> 16) & 255);
    unsafe {
        let outline = colour != border;
        let ready = GdipCreateFromHDC(dc, &mut graphics) == 0
            && GdipCreatePath(FillModeAlternate, &mut path) == 0
            && GdipCreateSolidFill(argb(colour), &mut brush) == 0
            && (!outline || GdipCreatePen1(argb(border), 1.0, UnitPixel, &mut pen) == 0);
        if ready {
            GdipSetSmoothingMode(graphics, SmoothingModeAntiAlias8x8);
            let edge = if outline { 0.5 } else { 0.0 };
            let x = rect.left as f32 + edge;
            let y = rect.top as f32 + edge;
            let right = rect.right as f32 - edge;
            let bottom = rect.bottom as f32 - edge;
            let d = (diameter as f32).min(right - x).min(bottom - y).max(1.0);
            for (left, top, angle) in [
                (x, y, 180.0),
                (right - d, y, 270.0),
                (right - d, bottom - d, 0.0),
                (x, bottom - d, 90.0),
            ] {
                GdipAddPathArc(path, left, top, d, d, angle, 90.0);
            }
            GdipClosePathFigure(path);
            GdipFillPath(graphics, brush as _, path);
            if outline {
                GdipDrawPath(graphics, pen, path);
            }
        }
        if !pen.is_null() {
            GdipDeletePen(pen);
        }
        if !brush.is_null() {
            GdipDeleteBrush(brush as _);
        }
        if !path.is_null() {
            GdipDeletePath(path);
        }
        if !graphics.is_null() {
            GdipDeleteGraphics(graphics);
        }
        ready
    }
}

pub fn button(item: &DRAWITEMSTRUCT, primary: bool) {
    button_on(item, primary, theme::surface());
}

pub fn button_on(item: &DRAWITEMSTRUCT, primary: bool, backdrop: u32) {
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
    fill(dc, &rect, backdrop);
    rounded(
        dc,
        rect,
        scaled(12, dpi),
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

pub fn style_edit(hwnd: HWND, dpi: i32, card: bool) {
    let mode = if card { 0x10000 } else { 0 };
    set_edit_style(hwnd, dpi, mode);
}

pub fn style_hotkey_value(hwnd: HWND, dpi: i32) {
    set_edit_style(hwnd, dpi, 0x30000);
}

fn set_edit_style(hwnd: HWND, dpi: i32, mode: usize) {
    unsafe {
        SetWindowSubclass(hwnd, Some(edit_proc), 1, mode | dpi as usize);
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

unsafe extern "system" fn edit_proc(
    hwnd: HWND,
    msg: u32,
    w: WPARAM,
    l: LPARAM,
    _: usize,
    mode: usize,
) -> LRESULT {
    let dpi = (mode & 0xffff) as i32;
    let inset_x = scaled(9, dpi);
    let inset_y = scaled(7, dpi);
    unsafe {
        if mode & 0x20000 != 0 && matches!(msg, WM_PAINT | WM_PRINTCLIENT) {
            let mut paint: PAINTSTRUCT = std::mem::zeroed();
            let dc = if msg == WM_PAINT {
                BeginPaint(hwnd, &mut paint)
            } else {
                w as HDC
            };
            let mut rect = RECT::default();
            GetClientRect(hwnd, &mut rect);
            fill(dc, &rect, theme::paper());
            let text = window_text(hwnd);
            let old = SelectObject(dc, DefSubclassProc(hwnd, WM_GETFONT, 0, 0) as HFONT);
            let label = if text.is_empty() {
                crate::localization::tr("未设置")
            } else {
                text
            };
            line_text(dc, rect, &label, theme::text(), DT_CENTER);
            SelectObject(dc, old);
            if msg == WM_PAINT {
                EndPaint(hwnd, &paint);
            }
            return 0;
        }
        if msg == WM_NCCALCSIZE && l != 0 {
            let rect = &mut *(l as *mut RECT);
            let x = inset_x.min((rect.right - rect.left - 1).max(0) / 2);
            let y = inset_y.min((rect.bottom - rect.top - 1).max(0) / 2);
            InflateRect(rect, -x, -y);
            return 0;
        }
        if msg == WM_NCPAINT {
            let dc = GetWindowDC(hwnd);
            if !dc.is_null() {
                let mut rect: RECT = std::mem::zeroed();
                GetWindowRect(hwnd, &mut rect);
                OffsetRect(&mut rect, -rect.left, -rect.top);
                ExcludeClipRect(
                    dc,
                    inset_x,
                    inset_y,
                    rect.right - inset_x,
                    rect.bottom - inset_y,
                );
                fill(
                    dc,
                    &rect,
                    if mode & 0x10000 != 0 {
                        theme::control_surface()
                    } else {
                        theme::surface()
                    },
                );
                rounded(
                    dc,
                    rect,
                    scaled(12, dpi),
                    theme::paper(),
                    if GetFocus() == hwnd {
                        theme::accent()
                    } else {
                        theme::border()
                    },
                );
                ReleaseDC(hwnd, dc);
            }
            return 0;
        }
        let result = DefSubclassProc(hwnd, msg, w, l);
        if matches!(msg, WM_SETFOCUS | WM_KILLFOCUS | WM_ENABLE)
            || (mode & 0x20000 != 0 && msg == WM_SETTEXT)
        {
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
            } else {
                theme::muted()
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

pub fn settings_switch(hwnd: HWND, dpi: i32, description: bool) {
    let mode = 0x10000 | if description { 0x20000 } else { 0 };
    unsafe { SetWindowSubclass(hwnd, Some(switch_proc), 1, mode | dpi as usize) };
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
            let settings = dpi & 0x10000 != 0;
            let description = dpi & 0x20000 != 0;
            let dpi = (dpi & 0xffff) as i32;
            let px = |n| {
                if settings {
                    crate::settings_layout::scale(n, dpi)
                } else {
                    scaled(n, dpi)
                }
            };
            let checked = DefSubclassProc(hwnd, BM_GETCHECK, 0, 0) == BST_CHECKED as isize;
            let enabled =
                windows_sys::Win32::UI::Input::KeyboardAndMouse::IsWindowEnabled(hwnd) != 0;
            let focused = GetFocus() == hwnd;
            let h = px(if settings { 20 } else { 18 });
            let right = bounds.right - px(if settings { 9 } else { 8 });
            let top = (bounds.bottom - h) / 2;
            let track = RECT {
                left: right - px(if settings { 35 } else { 34 }),
                top,
                right,
                bottom: top + h,
            };
            fill(dc, &bounds, theme::control_surface());
            if settings
                && focused
                && DefSubclassProc(hwnd, WM_QUERYUISTATE, 0, 0) as u32 & UISF_HIDEFOCUS == 0
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
                if focused || (settings && checked && enabled) {
                    theme::accent()
                } else {
                    theme::border()
                },
            );
            let diameter = if settings { px(12) } else { h - scaled(6, dpi) };
            let inset = (h - diameter) / 2;
            let side = if settings { px(3) } else { inset };
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
            bounds.left += px(if settings { 9 } else { 10 });
            bounds.right = track.left - px(7);
            if description {
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
            if description && let Some((caption, font)) = crate::settings::switch_description(hwnd)
            {
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
            if focused && !settings {
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
