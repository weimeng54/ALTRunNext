// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::{APPEND, CAPTION, CARD_TOP, FOOTER, REPLACE, ROW, WIDTH, Window, rect};
use crate::{
    theme,
    ui::{self, scaled},
};
use windows_sys::Win32::{Graphics::Gdi::*, UI::Controls::NMCUSTOMDRAW};

pub(super) fn draw_frame(state: &Window<'_>, dc: HDC) {
    let bounds = |y, height| rect(state.dpi, 0, y, WIDTH, height);
    ui::fill(dc, &bounds(0, state.height), theme::dialog_surface());
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

pub(super) fn draw_option(state: &Window<'_>, info: &NMCUSTOMDRAW) -> isize {
    let radio = matches!(info.hdr.idFrom, APPEND | REPLACE);
    ui::check_option(
        info,
        state.dpi,
        state.fonts[usize::from(!radio)],
        theme::paper(),
        radio,
    )
}
