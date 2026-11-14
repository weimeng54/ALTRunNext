// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

//! Native report list for the shortcut editor. Windows owns selection, hit testing and scrolling.
use crate::{
    native::{scaled, ui, wide},
    theme, theme_ui,
};
use std::ptr::{null, null_mut};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    UI::{
        Controls::*,
        HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi},
        Shell::{DefSubclassProc, SetWindowSubclass},
        WindowsAndMessaging::*,
    },
};

const TITLES: [&str; 2] = ["ui.alias", "ui.name"];

pub unsafe fn configure(list: HWND, dpi: i32) {
    unsafe {
        SendMessageW(list, LVM_SETUNICODEFORMAT, 1, 0);
        let style = LVS_EX_FULLROWSELECT | LVS_EX_DOUBLEBUFFER;
        SendMessageW(
            list,
            LVM_SETEXTENDEDLISTVIEWSTYLE,
            style as usize,
            style as isize,
        );
        set_background(list);
        let mut client = RECT::default();
        GetClientRect(list, &mut client);
        let width = client.right - GetSystemMetricsForDpi(SM_CXVSCROLL, GetDpiForWindow(list));
        let first = width * 42 / 100;
        for (column, (title, cx)) in TITLES.into_iter().zip([first, width - first]).enumerate() {
            let mut title = ui(title);
            let data = LVCOLUMNW {
                mask: LVCF_TEXT | LVCF_WIDTH,
                cx,
                pszText: title.as_mut_ptr(),
                ..Default::default()
            };
            SendMessageW(list, LVM_INSERTCOLUMNW, column, &data as *const _ as LPARAM);
        }
        SetWindowSubclass(list, Some(table_proc), 3, dpi as usize);
        let header = SendMessageW(list, LVM_GETHEADER, 0, 0) as HWND;
        let style = GetWindowLongW(header, GWL_STYLE);
        SetWindowLongW(header, GWL_STYLE, style | HDS_FULLDRAG as i32);
        SetWindowSubclass(header, Some(header_proc), 1, dpi as usize);
        SetWindowPos(
            list,
            null_mut(),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED,
        );
        let region = CreateRoundRectRgn(
            0,
            0,
            client.right + 1,
            client.bottom + 1,
            scaled(12, dpi),
            scaled(12, dpi),
        );
        if !region.is_null() && SetWindowRgn(list, region, 0) == 0 {
            DeleteObject(region);
        }
        theme_ui::style_vertical_scrollbar(list, 2164, dpi);
        theme_ui::style_horizontal_scrollbar(list, 2165, dpi);
    }
}

unsafe extern "system" fn header_proc(
    header: HWND,
    message: u32,
    w: WPARAM,
    l: LPARAM,
    _: usize,
    dpi: usize,
) -> LRESULT {
    unsafe {
        if matches!(message, WM_PAINT | WM_PRINTCLIENT) {
            let mut paint = PAINTSTRUCT::default();
            let dc = if message == WM_PAINT {
                BeginPaint(header, &mut paint)
            } else {
                w as HDC
            };
            draw_header(header, dc, dpi as i32);
            if message == WM_PAINT {
                EndPaint(header, &paint);
            }
            return 0;
        }
        if message == WM_ERASEBKGND {
            return 1;
        }
        let result = DefSubclassProc(header, message, w, l);
        if message == HDM_LAYOUT && result != 0 && l != 0 {
            let layout = &*(l as *const HDLAYOUT);
            let height = scaled(30, dpi as i32);
            (*layout.prc).top += height - (*layout.pwpos).cy;
            (*layout.pwpos).cy = height;
        }
        result
    }
}

pub fn set_background(list: HWND) {
    unsafe {
        SendMessageW(list, LVM_SETBKCOLOR, 0, theme::paper() as LPARAM);
    }
}

pub unsafe fn select(list: HWND, row: usize) {
    unsafe {
        let state = LVIS_SELECTED | LVIS_FOCUSED;
        let item = LVITEMW {
            stateMask: state,
            state,
            ..Default::default()
        };
        SendMessageW(list, LVM_SETITEMSTATE, row, &item as *const _ as LPARAM);
    }
}

pub fn replace_rows(
    list: HWND,
    rows: &[(String, String)],
    selected: Option<usize>,
    row_height: i32,
) {
    unsafe {
        let top = SendMessageW(list, LVM_GETTOPINDEX, 0, 0);
        let left = GetScrollPos(list, SB_HORZ);
        SendMessageW(list, WM_SETREDRAW, 0, 0);
        SendMessageW(list, LVM_DELETEALLITEMS, 0, 0);
        for (row, (alias, name)) in rows.iter().enumerate() {
            let mut alias = wide(alias);
            let item = LVITEMW {
                mask: LVIF_TEXT,
                iItem: row as i32,
                pszText: alias.as_mut_ptr(),
                ..Default::default()
            };
            let inserted = SendMessageW(list, LVM_INSERTITEMW, 0, &item as *const _ as LPARAM);
            if inserted >= 0 {
                let mut name = wide(name);
                let item = LVITEMW {
                    iSubItem: 1,
                    pszText: name.as_mut_ptr(),
                    ..Default::default()
                };
                SendMessageW(
                    list,
                    LVM_SETITEMTEXTW,
                    inserted as usize,
                    &item as *const _ as LPARAM,
                );
            }
        }
        if let Some(row) = selected {
            select(list, row);
        }
        SendMessageW(list, WM_SETREDRAW, 1, 0);
        let top = top.clamp(0, rows.len().saturating_sub(1) as isize);
        let current_top = SendMessageW(list, LVM_GETTOPINDEX, 0, 0);
        SendMessageW(
            list,
            LVM_SCROLL,
            (left - GetScrollPos(list, SB_HORZ)) as usize,
            (top - current_top) * row_height as isize,
        );
        if let Some(row) = selected {
            SendMessageW(list, LVM_ENSUREVISIBLE, row, 0);
        }
        InvalidateRect(list, null(), 0);
    }
}

pub unsafe fn row_at(list: HWND, point: POINT) -> Option<usize> {
    unsafe {
        let mut hit = LVHITTESTINFO {
            pt: point,
            ..Default::default()
        };
        let row = SendMessageW(list, LVM_SUBITEMHITTEST, 0, &mut hit as *mut _ as LPARAM);
        usize::try_from(row).ok()
    }
}

pub unsafe fn row_bounds(list: HWND, row: usize) -> RECT {
    unsafe {
        let mut rect = RECT {
            left: LVIR_BOUNDS as i32,
            ..Default::default()
        };
        SendMessageW(list, LVM_GETITEMRECT, row, &mut rect as *mut _ as LPARAM);
        rect
    }
}

pub fn draw_row(item: &DRAWITEMSTRUCT, alias: &str, name: &str, font: HFONT, dpi: i32) {
    unsafe {
        let list = item.hwndItem;
        let dc = item.hDC;
        let mut client = RECT::default();
        GetClientRect(list, &mut client);
        let rect = RECT {
            left: 0,
            right: client.right,
            ..item.rcItem
        };
        let state = SendMessageW(
            list,
            LVM_GETITEMSTATE,
            item.itemID as usize,
            LVIS_SELECTED as LPARAM,
        );
        let selected = state & LVIS_SELECTED as LPARAM != 0;
        let background = if selected {
            theme::selection()
        } else {
            theme::paper()
        };
        theme_ui::fill(dc, &rect, background);
        let left = -GetScrollPos(list, SB_HORZ);
        let first = SendMessageW(list, LVM_GETCOLUMNWIDTH, 0, 0) as i32;
        let second = SendMessageW(list, LVM_GETCOLUMNWIDTH, 1, 0) as i32;
        let split = left + first;
        let padding = scaled(8, dpi);
        theme_ui::fill(
            dc,
            &RECT {
                left: split - 1,
                right: split,
                ..rect
            },
            theme::divider(),
        );
        if selected {
            theme_ui::fill(
                dc,
                &RECT {
                    left: left + scaled(3, dpi),
                    right: left + scaled(6, dpi),
                    top: rect.top + scaled(5, dpi),
                    bottom: rect.bottom - scaled(5, dpi),
                },
                theme::accent(),
            );
        }
        let old_font = SelectObject(dc, font);
        let old_mode = SetBkMode(dc, TRANSPARENT as i32);
        theme_ui::line_text(
            dc,
            RECT {
                left: left + padding,
                right: split - padding,
                ..rect
            },
            alias,
            theme::text(),
            0,
        );
        theme_ui::line_text(
            dc,
            RECT {
                left: split + padding,
                right: split + second - padding,
                ..rect
            },
            name,
            if selected && theme::high_contrast() {
                theme::text()
            } else {
                theme::muted()
            },
            0,
        );
        SetBkMode(dc, old_mode);
        SelectObject(dc, old_font);
        if item.itemState & ODS_FOCUS != 0
            && item.itemState & ODS_NOFOCUSRECT == 0
            && SendMessageW(list, WM_QUERYUISTATE, 0, 0) & UISF_HIDEFOCUS as isize == 0
        {
            DrawFocusRect(dc, &rect);
        }
    }
}

unsafe extern "system" fn table_proc(
    list: HWND,
    message: u32,
    w: WPARAM,
    l: LPARAM,
    _: usize,
    dpi: usize,
) -> LRESULT {
    unsafe {
        if message == WM_NOTIFY && l != 0 {
            let notification = &*(l as *const NMHDR);
            let header = SendMessageW(list, LVM_GETHEADER, 0, 0) as HWND;
            if notification.hwndFrom == header {
                match notification.code {
                    HDN_ITEMCHANGINGW | HDN_ITEMCHANGINGA => {
                        let change = &*(l as *const NMHEADERW);
                        if let Some(item) = change.pitem.as_ref()
                            && item.mask & HDI_WIDTH != 0
                            && item.cxy < scaled(54, dpi as i32)
                        {
                            return 1;
                        }
                    }
                    HDN_DIVIDERDBLCLICKW | HDN_DIVIDERDBLCLICKA => {
                        let column = (*(l as *const NMHEADERW)).iItem as usize;
                        if column < TITLES.len() {
                            fit_column(list, column, dpi as i32);
                            return 0;
                        }
                    }
                    _ => {}
                }
            }
        }
        DefSubclassProc(list, message, w, l)
    }
}

unsafe fn draw_header(header: HWND, dc: HDC, dpi: i32) {
    unsafe {
        let mut client = RECT::default();
        GetClientRect(header, &mut client);
        theme_ui::fill(dc, &client, theme::control_surface());
        let font = SendMessageW(header, WM_GETFONT, 0, 0) as HFONT;
        let old_font = SelectObject(dc, font);
        let old_mode = SetBkMode(dc, TRANSPARENT as i32);
        for (column, title) in TITLES.into_iter().enumerate() {
            let mut rect = RECT::default();
            if SendMessageW(
                header,
                HDM_GETITEMRECT,
                column,
                &mut rect as *mut _ as LPARAM,
            ) != 0
            {
                theme_ui::fill(
                    dc,
                    &RECT {
                        left: rect.right - 1,
                        ..rect
                    },
                    theme::divider(),
                );
                rect.left += scaled(8, dpi);
                rect.right -= scaled(8, dpi);
                theme_ui::line_text(dc, rect, &crate::localization::tr(title), theme::muted(), 0);
            }
        }
        theme_ui::fill(
            dc,
            &RECT {
                top: client.bottom - 1,
                ..client
            },
            theme::divider(),
        );
        SetBkMode(dc, old_mode);
        SelectObject(dc, old_font);
    }
}

unsafe fn fit_column(list: HWND, column: usize, dpi: i32) {
    unsafe {
        let title = ui(TITLES[column]);
        let text_width = |text: &[u16]| {
            SendMessageW(list, LVM_GETSTRINGWIDTHW, 0, text.as_ptr() as LPARAM) as i32
        };
        let mut width = text_width(&title);
        let count = SendMessageW(list, LVM_GETITEMCOUNT, 0, 0);
        let mut buffer = vec![0_u16; 256];
        for row in 0..count {
            loop {
                let mut item = LVITEMW {
                    iSubItem: column as i32,
                    pszText: buffer.as_mut_ptr(),
                    cchTextMax: buffer.len() as i32,
                    ..Default::default()
                };
                let length = SendMessageW(
                    list,
                    LVM_GETITEMTEXTW,
                    row as usize,
                    &mut item as *mut _ as LPARAM,
                );
                if length < buffer.len() as isize - 1 || buffer.len() >= 32768 {
                    width = width.max(text_width(&buffer));
                    break;
                }
                buffer.resize(buffer.len() * 2, 0);
            }
        }
        let width = (width + scaled(20, dpi)).clamp(scaled(54, dpi), 32767);
        SendMessageW(list, LVM_SETCOLUMNWIDTH, column, width as LPARAM);
    }
}
