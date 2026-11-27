// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use crate::{theme, ui::scaled};
use windows_sys::Win32::{Foundation::RECT, Graphics::Gdi::*};

use altrun_probe::preferences::{Preferences, parse_colour};

#[derive(Clone, Copy)]
pub(crate) struct ResultDividers {
    vertical: bool,
    horizontal: bool,
    colour: Option<u32>,
}

impl ResultDividers {
    pub(crate) fn from_preferences(prefs: &Preferences) -> Self {
        Self {
            vertical: prefs.flag("VerticalResultDividers"),
            horizontal: prefs.flag("HorizontalResultDividers"),
            colour: parse_colour(&prefs.get("ResultDividerColor")).ok(),
        }
    }

    pub(crate) fn draw(
        self,
        dc: HDC,
        row: RECT,
        columns: [i32; 2],
        dpi: i32,
        selected: bool,
        has_next: bool,
    ) {
        if !(self.vertical || self.horizontal && has_next) {
            return;
        }
        let background = if selected {
            theme::selection()
        } else {
            theme::paper()
        };
        let line = |default: u32, weight: u32| {
            if theme::high_contrast() {
                return if selected {
                    theme::selection_text()
                } else {
                    theme::divider()
                };
            }
            let channel = |shift: u32| {
                let base = (background >> shift) & 255u32;
                let ink = (self.colour.unwrap_or(default) >> shift) & 255u32;
                (base * (100 - weight) + ink * weight + 50) / 100
            };
            theme::rgb(channel(0), channel(8), channel(16))
        };
        let n = scaled(1, dpi).max(1);
        unsafe {
            let brush = GetStockObject(DC_BRUSH) as HBRUSH;
            let old = GetDCBrushColor(dc);
            if self.vertical {
                for (x, weight) in columns.into_iter().zip([24, 36]) {
                    if x >= row.right {
                        break;
                    }
                    let weight = if self.colour.is_some() {
                        weight * 100 / 36
                    } else {
                        weight
                    };
                    SetDCBrushColor(dc, line(theme::border(), weight));
                    FillRect(
                        dc,
                        &RECT {
                            left: x,
                            right: (x + n).min(row.right),
                            ..row
                        },
                        brush,
                    );
                }
            }
            if self.horizontal && has_next {
                let left = columns[0] + scaled(4, dpi);
                let right = row.right - scaled(6, dpi);
                if left < right {
                    SetDCBrushColor(
                        dc,
                        line(
                            theme::divider(),
                            if self.colour.is_some() { 100 } else { 68 },
                        ),
                    );
                    FillRect(
                        dc,
                        &RECT {
                            left,
                            top: row.bottom - n,
                            right,
                            bottom: row.bottom,
                        },
                        brush,
                    );
                }
            }
            SetDCBrushColor(dc, old);
        }
    }
}
