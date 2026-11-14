// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::{Dialog, FOOTER, HEADER, Preview, REPLACE, alias_bounds};
use crate::{manager::table, theme, ui};
use altrun_probe::browser_bookmarks::BookmarkStatus;
use std::ptr::null_mut;
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    UI::{Controls::*, WindowsAndMessaging::*},
};

pub(super) fn draw_row(item: &DRAWITEMSTRUCT, dialog: &Dialog) {
    let Some(row) = dialog.model.borrow().display_row(item.itemID as usize) else {
        return;
    };
    let columns = row.cells.iter().map(String::as_str).collect::<Vec<_>>();
    table::draw_columns(item, &columns, dialog.font, dialog.dpi);
    if let Some((status, checked, alias)) = row.bookmark {
        draw_alias_cell(item, dialog, status, &alias);
        let px = |n| ui::scaled(n, dialog.dpi);
        let width = unsafe { SendMessageW(item.hwndItem, LVM_GETCOLUMNWIDTH, 0, 0) } as i32;
        let side = px(16);
        if width < side + px(8) {
            return;
        }
        let left = -unsafe { GetScrollPos(item.hwndItem, SB_HORZ) } + (width - side) / 2;
        let top = (item.rcItem.top + item.rcItem.bottom - side) / 2;
        let bounds = RECT {
            left,
            top,
            right: left + side,
            bottom: top + side,
        };
        let active = status == BookmarkStatus::Ready;
        let fill = if active && checked {
            theme::accent()
        } else {
            theme::paper()
        };
        let border = if active && checked {
            theme::accent()
        } else {
            theme::border()
        };
        ui::rounded(item.hDC, bounds, px(4), fill, border);
        if active && checked {
            unsafe {
                let pen = CreatePen(PS_SOLID, px(2).max(1), theme::on_accent());
                let old = SelectObject(item.hDC, pen);
                MoveToEx(item.hDC, left + px(4), top + px(8), null_mut());
                LineTo(item.hDC, left + px(7), top + px(11));
                LineTo(item.hDC, left + px(13), top + px(5));
                SelectObject(item.hDC, old);
                DeleteObject(pen);
            }
        }
    }
}

fn draw_alias_cell(item: &DRAWITEMSTRUCT, dialog: &Dialog, status: BookmarkStatus, alias: &str) {
    let px = |n| ui::scaled(n, dialog.dpi);
    unsafe {
        let Some(bounds) = alias_bounds(item.hwndItem, item.itemID as usize, dialog.dpi) else {
            return;
        };
        let dc = item.hDC;
        let colour = if status == BookmarkStatus::Ready {
            ui::rounded(dc, bounds, px(8), theme::paper(), theme::border());
            theme::text()
        } else {
            theme::muted()
        };
        let old = SelectObject(dc, dialog.font);
        ui::line_text(
            dc,
            RECT {
                left: bounds.left + px(7),
                right: bounds.right - px(5),
                ..bounds
            },
            if status == BookmarkStatus::Ready {
                alias
            } else {
                "—"
            },
            colour,
            0,
        );
        SelectObject(dc, old);
    }
}

pub(super) fn draw_mode(item: &DRAWITEMSTRUCT, dialog: &Dialog) {
    let replace = matches!(
        &dialog.model.borrow().preview,
        Some(Preview::LaunchItems { replace: true, .. })
    );
    let checked = replace == (item.CtlID as usize == REPLACE);
    let px = |n| ui::scaled(n, dialog.dpi);
    let dc = item.hDC;
    let rect = item.rcItem;
    ui::fill(dc, &rect, theme::surface());
    let side = px(16);
    let top = (rect.top + rect.bottom - side) / 2;
    let bounds = RECT {
        left: rect.left + px(2),
        top,
        right: rect.left + px(2) + side,
        bottom: top + side,
    };
    let focus = item.itemState & ODS_FOCUS != 0;
    ui::rounded(
        dc,
        bounds,
        side,
        theme::paper(),
        if focus || checked {
            theme::accent()
        } else {
            theme::border()
        },
    );
    if checked {
        let inset = px(4);
        ui::rounded(
            dc,
            RECT {
                left: bounds.left + inset,
                top: top + inset,
                right: bounds.right - inset,
                bottom: bounds.bottom - inset,
            },
            side - 2 * inset,
            theme::accent(),
            theme::accent(),
        );
    }
    unsafe {
        let old = SelectObject(dc, dialog.font);
        ui::line_text(
            dc,
            RECT {
                left: bounds.right + px(8),
                ..rect
            },
            &ui::window_text(item.hwndItem),
            theme::text(),
            0,
        );
        SelectObject(dc, old);
    }
}

pub(super) fn paint(hwnd: HWND, dialog: &Dialog, dc: HDC) {
    let mut bounds = RECT::default();
    unsafe { GetClientRect(hwnd, &mut bounds) };
    let y = ui::scaled(HEADER, dialog.dpi);
    let footer_y = ui::scaled(dialog.height - FOOTER, dialog.dpi);
    ui::fill(dc, &bounds, theme::surface());
    ui::fill(
        dc,
        &RECT {
            bottom: y,
            ..bounds
        },
        theme::chrome(),
    );
    ui::fill(
        dc,
        &RECT {
            top: footer_y,
            ..bounds
        },
        theme::chrome(),
    );
    for top in [y - 1, footer_y] {
        ui::fill(
            dc,
            &RECT {
                top,
                bottom: top + 1,
                ..bounds
            },
            theme::divider(),
        );
    }
    ui::panel_caption(dc, dialog.dpi, dialog.caption, &dialog.title);
}
