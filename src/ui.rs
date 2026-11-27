// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

//! Common Win32 painting, controls, dialogs and DPI geometry.
mod buttons;
mod child;
pub(crate) mod dialogs;
mod fields;
pub(crate) mod geometry;
pub(crate) mod language;
pub(crate) mod menu_style;
mod painting;
pub(crate) mod popup_frame;
pub(crate) mod report;
mod scrollbars;
pub(crate) mod window_frame;
mod window_region;

pub(crate) use buttons::{
    CaptionIcon, button_on, caption_button, caption_close, check_option, menu_button,
    option_focus_cues, style_caption_close, switch,
};
pub(crate) use child::ChildControls;
pub(crate) use fields::{
    NUMBER_STEP, buffer_edit_selection, combo, slider, style_combo, style_edit,
    style_edit_with_colours, style_number, style_search, style_slider,
};
pub(crate) use painting::{
    Chevron, ResultDividers, STATIC_CENTER_IMAGE, app_icon, buffered, chevron, draw_app_icon, fill,
    launcher_title, line_text, panel_caption, rounded, row_focus, window_text,
};
pub(crate) use scrollbars::{style_list_scrollbar, style_report_scrollbars, style_text_scrollbar};

use std::ptr::null_mut;
use windows_sys::Win32::Graphics::Gdi::*;

pub(crate) fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}
pub(crate) fn ui(text: &str) -> Vec<u16> {
    wide(&crate::localization::tr(text))
}
pub(crate) fn scaled(value: i32, dpi: i32) -> i32 {
    (value * dpi + 48 * value.signum()) / 96
}

// The input is a NUL-terminated UTF-16 caption.
fn text_width(dc: HDC, caption: &[u16]) -> i32 {
    let mut size = windows_sys::Win32::Foundation::SIZE::default();
    unsafe { GetTextExtentPoint32W(dc, caption.as_ptr(), caption.len() as i32 - 1, &mut size) };
    size.cx
}

pub(crate) fn caption_width(
    hwnd: windows_sys::Win32::Foundation::HWND,
    font: HFONT,
    dpi: i32,
    key: &str,
) -> i32 {
    unsafe {
        let dc = GetDC(hwnd);
        let old = SelectObject(dc, font);
        let pixels = text_width(dc, &ui(key));
        SelectObject(dc, old);
        ReleaseDC(hwnd, dc);
        (pixels * 96 + dpi - 1) / dpi
    }
}

pub fn show_error(message: &str) {
    dialogs::input::notice(null_mut(), "ALTRunNext", message);
}

pub(crate) fn make_font(size: i32, weight: i32, family: &str, dpi: i32) -> HFONT {
    unsafe {
        CreateFontW(
            -scaled(size, dpi),
            0,
            0,
            0,
            weight,
            0,
            0,
            0,
            DEFAULT_CHARSET as u32,
            OUT_DEFAULT_PRECIS as u32,
            CLIP_DEFAULT_PRECIS as u32,
            CLEARTYPE_QUALITY as u32,
            0,
            wide(family).as_ptr(),
        )
    }
}
