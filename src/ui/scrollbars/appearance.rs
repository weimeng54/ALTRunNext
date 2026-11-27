// Copyright (c) 2026 GPT-6.2 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::{Axis, fill, rounded, scaled};
use crate::theme;
use windows_sys::Win32::{Foundation::RECT, Graphics::Gdi::HDC};

pub(super) const INSET: i32 = 4;
pub(super) const MINIMUM_THUMB: i32 = 28;
const TRACK_WIDTH: i32 = 14;
const IDLE_THUMB_WIDTH: i32 = 8;
const ACTIVE_THUMB_WIDTH: i32 = 11;

pub(super) struct Visual {
    pub(super) track_start: i32,
    pub(super) track_end: i32,
    pub(super) thumb_start: i32,
    pub(super) thumb_end: i32,
    pub(super) active: bool,
}

impl Visual {
    pub(super) fn draw(self, dc: HDC, bounds: RECT, axis: Axis, dpi: i32) {
        let (idle, active, track) = if theme::high_contrast() {
            (theme::text(), theme::selection_text(), theme::selection())
        } else if theme::is_dark() {
            (
                theme::rgb(116, 134, 154),
                theme::rgb(159, 177, 198),
                theme::rgb(35, 46, 58),
            )
        } else {
            (
                theme::rgb(132, 149, 168),
                theme::rgb(93, 114, 138),
                theme::rgb(237, 242, 247),
            )
        };
        fill(dc, &bounds, theme::paper());
        let draw = |width, start, end, colour| {
            let width = scaled(width, dpi).clamp(1, axis.cross_length(bounds).max(1));
            let rect = axis.centered(bounds, width, start, end);
            rounded(dc, rect, width, colour, colour);
        };
        if self.active {
            draw(TRACK_WIDTH, self.track_start, self.track_end, track);
        }
        let (width, colour) = if self.active {
            (ACTIVE_THUMB_WIDTH, active)
        } else {
            (IDLE_THUMB_WIDTH, idle)
        };
        draw(width, self.thumb_start, self.thumb_end, colour);
    }
}
