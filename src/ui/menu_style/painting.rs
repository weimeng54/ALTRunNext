// Copyright (c) 2026 GPT-6
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::*;

impl PopupStyle {
    pub(super) fn measure(&self, info: &mut MEASUREITEMSTRUCT) -> bool {
        if info.CtlType != ODT_MENU {
            return false;
        }
        let Some(item) = self.items.get(&info.itemData) else {
            return false;
        };
        let columns = &self.columns[&item.menu];
        let padding = 25
            + 14
            + if columns.shortcut != 0 { 18 } else { 0 }
            + if columns.cascade { 12 } else { 0 };
        info.itemWidth = (columns.label + columns.shortcut + scaled(padding, self.dpi))
            .min(self.max_width.get()) as u32;
        info.itemHeight = if item.rule {
            scaled(7, self.dpi)
        } else {
            self.row_height
        } as u32;
        true
    }

    pub(super) fn draw(&self, info: &DRAWITEMSTRUCT) -> bool {
        if info.CtlType != ODT_MENU {
            return false;
        }
        let Some(item) = self.items.get(&info.itemData) else {
            return false;
        };
        let px = |value| scaled(value, self.dpi);
        let rect = info.rcItem;
        let dc = info.hDC;
        ui::fill(dc, &rect, theme::control_surface());
        if item.rule {
            let top = (rect.top + rect.bottom) / 2;
            ui::fill(
                dc,
                &RECT {
                    left: rect.left + px(6),
                    right: rect.right - px(6),
                    top,
                    bottom: top + 1,
                },
                theme::divider(),
            );
            return true;
        }
        let disabled = info.itemState & (ODS_GRAYED | ODS_DISABLED) != 0;
        let selected = !disabled && info.itemState & ODS_SELECTED != 0;
        if selected {
            ui::rounded(
                dc,
                RECT {
                    left: rect.left + px(3),
                    right: rect.right - px(3),
                    ..rect
                },
                px(8),
                theme::selection(),
                theme::selection(),
            );
        }
        let ink = if disabled {
            theme::disabled_text()
        } else if selected {
            theme::selection_text()
        } else {
            theme::text()
        };
        let cols = &self.columns[&item.menu];
        let right = rect.right - px(if cols.cascade { 26 } else { 14 });
        let left = rect.left + px(25);
        let shortcut_width = cols.shortcut.min((right - left - px(18)).max(0));
        unsafe {
            let saved = SaveDC(dc);
            SelectObject(dc, self.font);
            ui::line_text(
                dc,
                RECT {
                    left,
                    right: right - shortcut_width - if shortcut_width > 0 { px(18) } else { 0 },
                    ..rect
                },
                &item.label,
                ink,
                0,
            );
            if !item.shortcut.is_empty() {
                ui::line_text(
                    dc,
                    RECT {
                        left: right - shortcut_width,
                        right,
                        ..rect
                    },
                    &item.shortcut,
                    ink,
                    DT_RIGHT,
                );
            }
            if info.itemState & ODS_CHECKED != 0 {
                ui::line_text(
                    dc,
                    RECT {
                        left: rect.left + px(5),
                        right: left,
                        ..rect
                    },
                    "✓",
                    ink,
                    DT_CENTER,
                );
            }
            if item.child {
                ui::chevron(
                    dc,
                    POINT {
                        x: rect.right - px(14),
                        y: (rect.top + rect.bottom) / 2,
                    },
                    ui::Chevron::Right,
                    self.dpi,
                    ink,
                );
            }
            RestoreDC(dc, saved);
            if item.child {
                // USER32 draws another cascade glyph after this callback.
                ExcludeClipRect(
                    dc,
                    rect.right - px(23),
                    rect.top,
                    rect.right + px(12),
                    rect.bottom,
                );
            }
        }
        true
    }
}
