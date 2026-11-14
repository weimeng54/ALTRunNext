// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::{table::list_height, *};

pub(super) fn paint_manager(hwnd: HWND) {
    unsafe {
        let mut ps: PAINTSTRUCT = zeroed();
        let dc = BeginPaint(hwnd, &mut ps);
        let mut bounds: RECT = zeroed();
        GetClientRect(hwnd, &mut bounds);
        let (dpi, font, caption) = EDITOR.with(|c| {
            let s = c.borrow();
            let s = s.as_ref().unwrap();
            (s.dpi, s.font, s.caption)
        });
        let rect = |x, y, w, h| RECT {
            left: scaled(x, dpi),
            top: scaled(y, dpi),
            right: scaled(x + w, dpi),
            bottom: scaled(y + h, dpi),
        };
        crate::ui::fill(dc, &bounds, theme::surface());
        crate::ui::fill(dc, &rect(0, 0, WIDTH, CAPTION), theme::chrome());
        crate::ui::fill(
            dc,
            &rect(0, CAPTION, RAIL, FOOTER_TOP - CAPTION),
            theme::chrome(),
        );
        crate::ui::fill(
            dc,
            &rect(0, FOOTER_TOP, WIDTH, HEIGHT - FOOTER_TOP),
            theme::chrome(),
        );
        let mut list_card = rect(16, TABLE_TOP, RAIL - 32, 0);
        list_card.bottom = scaled(TABLE_TOP + 1, dpi) + list_height(dpi) + scaled(2, dpi);
        crate::ui::rounded(
            dc,
            list_card,
            scaled(16, dpi),
            theme::control_surface(),
            theme::divider(),
        );
        let kind = EDITOR.with(|c| {
            let s = c.borrow();
            let s = s.as_ref().unwrap();
            s.draft.get(s.selected).map(|e| e.kind)
        });
        if let Some(kind) = kind {
            crate::ui::rounded(
                dc,
                rect(
                    FORM_LEFT,
                    FORM_TOP,
                    FORM_WIDTH,
                    if kind == library::EntryKind::Command {
                        COMMAND_FORM_HEIGHT
                    } else {
                        164
                    },
                ),
                scaled(16, dpi),
                theme::control_surface(),
                theme::divider(),
            );
        }
        for line in [
            rect(0, CAPTION, WIDTH, 1),
            rect(RAIL, CAPTION, 1, FOOTER_TOP - CAPTION),
            rect(0, FOOTER_TOP - 1, WIDTH, 1),
        ] {
            crate::ui::fill(dc, &line, theme::divider());
        }
        crate::ui::panel_caption(dc, dpi, caption, "ui.altrunnext_manage_shortcuts_2");
        SetBkMode(dc, TRANSPARENT as i32);
        SetTextColor(dc, theme::text());
        SelectObject(dc, font);
        let mut library_title = rect(17, 62, 120, 32);
        DrawTextW(
            dc,
            ui("ui.shortcuts").as_ptr(),
            -1,
            &mut library_title,
            DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX,
        );
        EndPaint(hwnd, &ps);
    }
}
