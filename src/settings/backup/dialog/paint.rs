// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::{APPEND, CAPTION, CARD_TOP, FOOTER, REPLACE, ROW, WIDTH, Window, rect};
use crate::{
    theme,
    ui::{self, scaled},
};
use windows_sys::Win32::{
    Foundation::{POINT, RECT},
    Graphics::Gdi::*,
    UI::{Controls::*, WindowsAndMessaging::*},
};

pub(super) fn draw_frame(state: &Window<'_>, dc: HDC) {
    let bounds = |y, height| rect(state.dpi, 0, y, WIDTH, height);
    ui::fill(dc, &bounds(0, state.height), theme::surface());
    for (y, height) in [(0, CAPTION), (state.height - FOOTER, FOOTER)] {
        ui::fill(dc, &bounds(y, height), theme::chrome());
    }
    for y in [CAPTION, state.height - FOOTER] {
        ui::fill(dc, &bounds(y, 1), theme::divider());
    }
    ui::rounded(
        dc,
        rect(
            state.dpi,
            20,
            CARD_TOP,
            WIDTH - 40,
            state.first_row + ROW * 2,
        ),
        scaled(10, state.dpi),
        theme::paper(),
        theme::divider(),
    );
    for y in [CARD_TOP + state.first_row, CARD_TOP + state.first_row + ROW] {
        ui::fill(dc, &rect(state.dpi, 21, y, WIDTH - 42, 1), theme::divider());
    }
    ui::panel_caption(dc, state.dpi, state.fonts[1], state.title);
}

pub(super) fn draw_option(state: &Window<'_>, info: &NMCUSTOMDRAW) {
    unsafe {
        let child = info.hdr.hwndFrom;
        let radio = matches!(info.hdr.idFrom, APPEND | REPLACE);
        let marked = SendMessageW(child, BM_GETCHECK, 0, 0) == BST_CHECKED as isize;
        let active = info.uItemState & CDIS_DISABLED == 0;
        let focused = info.uItemState & CDIS_FOCUS != 0;
        let backdrop = theme::paper();
        let fill = if marked && active && !radio {
            theme::accent()
        } else {
            backdrop
        };
        let border = if marked || focused {
            theme::accent()
        } else {
            theme::border()
        };
        let px = |v| scaled(v, state.dpi);
        ui::fill(info.hdc, &info.rc, backdrop);
        let top = (info.rc.bottom + info.rc.top - px(17)) / 2;
        let box_rect = RECT {
            left: 0,
            top,
            right: px(17),
            bottom: top + px(17),
        };
        ui::rounded(
            info.hdc,
            box_rect,
            px(if radio { 17 } else { 5 }),
            fill,
            border,
        );
        if marked && radio {
            let dot = RECT {
                left: px(4),
                right: px(13),
                top: top + px(4),
                bottom: top + px(13),
            };
            ui::rounded(info.hdc, dot, px(9), border, border);
        } else if marked && active {
            let mark = [
                POINT {
                    x: px(4),
                    y: top + px(8),
                },
                POINT {
                    x: px(7),
                    y: top + px(11),
                },
                POINT {
                    x: px(13),
                    y: top + px(5),
                },
            ];
            let pen = CreatePen(PS_SOLID, px(2).max(1), theme::on_accent());
            let previous = SelectObject(info.hdc, pen);
            Polyline(info.hdc, mark.as_ptr(), mark.len() as i32);
            SelectObject(info.hdc, previous);
            DeleteObject(pen);
        }
        let font = SelectObject(info.hdc, state.fonts[usize::from(!radio)]);
        let label = RECT {
            left: px(27),
            ..info.rc
        };
        ui::line_text(
            info.hdc,
            label,
            &ui::window_text(child),
            if active {
                theme::text()
            } else {
                theme::muted()
            },
            0,
        );
        SelectObject(info.hdc, font);
        if focused {
            DrawFocusRect(info.hdc, &label);
        }
    }
}
