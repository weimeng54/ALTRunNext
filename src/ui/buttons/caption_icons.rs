// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use crate::ui::scaled;
use windows_sys::Win32::{
    Foundation::{POINT, RECT},
    Graphics::Gdi::*,
};

#[derive(Clone, Copy)]
pub(crate) enum CaptionIcon {
    Manage,
    Settings,
    Hide,
}

impl CaptionIcon {
    pub(super) fn draw(self, dc: HDC, bounds: RECT, dpi: i32, colour: u32) {
        let x = (bounds.left + bounds.right) / 2;
        let y = (bounds.top + bounds.bottom) / 2;
        let point = |(dx, dy)| POINT {
            x: x + scaled(dx, dpi),
            y: y + scaled(dy, dpi),
        };
        let line = |from, to| {
            let endpoints = [point(from), point(to)];
            unsafe { Polyline(dc, endpoints.as_ptr(), endpoints.len() as i32) };
        };
        unsafe {
            let pen = CreatePen(PS_SOLID, scaled(1, dpi).max(1), colour);
            let previous_pen = SelectObject(dc, pen);
            match self {
                Self::Manage => {
                    for y in [-3, 0, 3] {
                        line((-5, y), (-3, y));
                        line((-1, y), (5, y));
                    }
                }
                Self::Settings => {
                    // Six shallow teeth keep the outline readable at 100% DPI.
                    let outline = [
                        (-1, -5),
                        (1, -5),
                        (1, -4),
                        (3, -3),
                        (4, -3),
                        (5, -2),
                        (4, -1),
                        (4, 1),
                        (5, 2),
                        (4, 3),
                        (3, 3),
                        (1, 4),
                        (1, 5),
                        (-1, 5),
                        (-1, 4),
                        (-3, 3),
                        (-4, 3),
                        (-5, 2),
                        (-4, 1),
                        (-4, -1),
                        (-5, -2),
                        (-4, -3),
                        (-3, -3),
                        (-1, -4),
                    ]
                    .map(point);
                    let hub = scaled(2, dpi).max(1);
                    let previous_brush = SelectObject(dc, GetStockObject(NULL_BRUSH));
                    Polygon(dc, outline.as_ptr(), outline.len() as i32);
                    Ellipse(dc, x - hub, y - hub, x + hub + 1, y + hub + 1);
                    SelectObject(dc, previous_brush);
                }
                Self::Hide => {
                    line((-4, -4), (4, 4));
                    line((4, -4), (-4, 4));
                }
            }
            SelectObject(dc, previous_pen);
            DeleteObject(pen);
        }
    }
}
