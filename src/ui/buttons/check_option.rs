// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use crate::{
    theme,
    ui::{fill, line_text, rounded, scaled, window_text},
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    UI::{Controls::*, WindowsAndMessaging::*},
};

pub(crate) fn option_focus_cues(window: HWND, msg: &MSG) {
    let action = match msg.message {
        WM_LBUTTONDOWN => UIS_SET,
        WM_KEYDOWN => UIS_CLEAR,
        _ => return,
    };
    unsafe {
        if (msg.hwnd == window || IsChild(window, msg.hwnd) != 0)
            && (SendMessageW(window, WM_QUERYUISTATE, 0, 0) as u32 & UISF_HIDEFOCUS != 0)
                != (action == UIS_SET)
        {
            SendMessageW(
                window,
                WM_CHANGEUISTATE,
                (action | (UISF_HIDEFOCUS << 16)) as usize,
                0,
            );
        }
    }
}

pub(crate) fn check_option(
    info: &NMCUSTOMDRAW,
    dpi: i32,
    font: HFONT,
    backdrop: u32,
    radio: bool,
) -> isize {
    if info.dwDrawStage != CDDS_PREPAINT {
        return CDRF_DODEFAULT as isize;
    }
    unsafe {
        let child = info.hdr.hwndFrom;
        let marked = SendMessageW(child, BM_GETCHECK, 0, 0) == BST_CHECKED as isize;
        let active = info.uItemState & CDIS_DISABLED == 0;
        let high_contrast = theme::high_contrast();
        let focused = active
            && info.uItemState & CDIS_FOCUS != 0
            && (high_contrast
                || SendMessageW(child, WM_QUERYUISTATE, 0, 0) as u32 & UISF_HIDEFOCUS == 0);
        let face = if marked && active && !radio {
            if high_contrast {
                theme::selection()
            } else {
                theme::accent()
            }
        } else {
            backdrop
        };
        let border = if !active {
            theme::disabled_text()
        } else if marked && !high_contrast {
            theme::accent()
        } else {
            theme::border()
        };
        let px = |v| scaled(v, dpi);
        fill(info.hdc, &info.rc, backdrop);
        let top = (info.rc.bottom + info.rc.top - px(17)) / 2;
        let left = info.rc.left + px(3);
        let box_rect = RECT {
            left,
            top,
            right: left + px(17),
            bottom: top + px(17),
        };
        if focused {
            let mut ring = box_rect;
            InflateRect(&mut ring, px(3), px(3));
            let ink = if high_contrast {
                theme::text()
            } else {
                theme::accent()
            };
            rounded(
                info.hdc,
                ring,
                px(if radio { 23 } else { 11 }),
                backdrop,
                ink,
            );
        }
        rounded(
            info.hdc,
            box_rect,
            px(if radio { 17 } else { 5 }),
            face,
            border,
        );
        if marked && radio {
            let dot = RECT {
                left: left + px(4),
                right: left + px(13),
                top: top + px(4),
                bottom: top + px(13),
            };
            rounded(info.hdc, dot, px(9), border, border);
        } else if marked {
            let mark = [
                POINT {
                    x: left + px(4),
                    y: top + px(8),
                },
                POINT {
                    x: left + px(7),
                    y: top + px(11),
                },
                POINT {
                    x: left + px(13),
                    y: top + px(5),
                },
            ];
            let mark_colour = if !active {
                theme::disabled_text()
            } else if high_contrast {
                theme::selection_text()
            } else {
                theme::on_accent()
            };
            let pen = CreatePen(PS_SOLID, px(2).max(1), mark_colour);
            let previous = SelectObject(info.hdc, pen);
            Polyline(info.hdc, mark.as_ptr(), mark.len() as i32);
            SelectObject(info.hdc, previous);
            DeleteObject(pen);
        }
        let previous_font = SelectObject(info.hdc, font);
        let label = RECT {
            left: info.rc.left + px(27),
            ..info.rc
        };
        line_text(
            info.hdc,
            label,
            &window_text(child),
            if active {
                theme::text()
            } else {
                theme::disabled_text()
            },
            0,
        );
        SelectObject(info.hdc, previous_font);
    }
    CDRF_SKIPDEFAULT as isize
}
