// Copyright (c) 2026 GPT-6.2 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::*;

pub(super) const MARGIN: i32 = 22;
pub(super) const FOOTER: i32 = 59;
const OPTION_ROW: i32 = 41;

#[derive(Clone, Copy)]
pub(super) struct Layout {
    pub(super) width: i32,
    pub(super) height: i32,
    pub(super) field_top: i32,
    pub(super) options_top: i32,
    pub(super) value_left: i32,
    pub(super) value_width: i32,
    pub(super) default_width: i32,
    pub(super) program_width: i32,
    pub(super) save_width: i32,
}

impl Layout {
    pub(super) fn new(batch: bool, document: bool) -> Self {
        let font = make_font(14, 400, "Microsoft YaHei UI", 96);
        let dc = unsafe { GetDC(null_mut()) };
        let old = unsafe { SelectObject(dc, font) };
        let measure = |key| {
            let caption = ui(key);
            let mut size = SIZE::default();
            unsafe {
                GetTextExtentPoint32W(dc, caption.as_ptr(), caption.len() as i32 - 1, &mut size)
            };
            size.cx
        };
        let value_left = (MARGIN + 32 + measure("ui.run_as_administrator")).max(171);
        let value_width = (measure("ui.system_default") + 8).max(76);
        let default_width = (measure("ui.use_system_default") + 22).max(108);
        let program_width = (measure("ui.choose_app") + 22).max(91);
        let save_width = (measure("ui.add_shortcut") + 24).max(119);
        unsafe {
            SelectObject(dc, old);
            ReleaseDC(null_mut(), dc);
            DeleteObject(font);
        }
        let width = if batch {
            840
        } else if document {
            (value_left + value_width + default_width + program_width + MARGIN + 26)
                .max(526)
                .max(save_width + 257)
        } else {
            (value_left + 62).max(526).max(save_width + 257)
        };
        let field_top = if batch { 245 } else { 56 };
        let options_top = field_top + 138;
        let height = options_top + Self::options_height(document) + 20 + FOOTER;
        Self {
            width,
            height,
            field_top,
            options_top,
            value_left,
            value_width,
            default_width,
            program_width,
            save_width,
        }
    }

    pub(super) fn admin_top(self, document: bool) -> i32 {
        self.options_top + 3 + if document { OPTION_ROW } else { 0 }
    }

    fn options_height(document: bool) -> i32 {
        OPTION_ROW * if document { 2 } else { 1 }
    }

    pub(super) fn options_bounds(self, dpi: i32, document: bool) -> RECT {
        let px = |v| scaled(v, dpi);
        RECT {
            left: px(MARGIN),
            top: px(self.options_top),
            right: px(self.width - MARGIN),
            bottom: px(self.options_top + Self::options_height(document)),
        }
    }

    pub(super) fn paint_options(self, dc: HDC, dpi: i32, document: bool) {
        let bounds = self.options_bounds(dpi, document);
        ui::rounded(
            dc,
            bounds,
            scaled(8, dpi),
            theme::control_surface(),
            theme::divider(),
        );
        if document {
            let top = scaled(self.options_top + OPTION_ROW, dpi);
            ui::fill(
                dc,
                &RECT {
                    left: bounds.left + 1,
                    top,
                    right: bounds.right - 1,
                    bottom: top + scaled(1, dpi),
                },
                theme::divider(),
            );
        }
    }
}
