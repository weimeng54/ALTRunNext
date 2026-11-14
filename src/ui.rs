// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

//! Common Win32 painting, controls, dialogs and DPI geometry.
mod buttons;
pub(crate) mod dialogs;
mod fields;
pub(crate) mod geometry;
mod painting;
mod scrollbars;

pub(crate) use buttons::{button_on, caption_close, menu_button, style_caption_close, switch};
pub(crate) use fields::{
    NUMBER_STEP, combo, slider, style_combo, style_edit, style_number, style_preview_edit,
    style_slider,
};
pub(crate) use painting::{
    Chevron, STATIC_CENTER_IMAGE, app_icon, chevron, draw_app_icon, fill, line_text, round_panel,
    rounded, window_text,
};
pub(crate) use scrollbars::{
    draw_scrollbar, style_horizontal_scrollbar, style_vertical_scrollbar, vertical_scrollbar,
};

use std::ptr::null_mut;
use windows_sys::Win32::{Graphics::Gdi::*, UI::WindowsAndMessaging::*};

pub(crate) fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}
pub(crate) fn ui(text: &str) -> Vec<u16> {
    wide(&crate::localization::tr(text))
}
pub(crate) fn scaled(value: i32, dpi: i32) -> i32 {
    (value * dpi + 48 * value.signum()) / 96
}

pub fn show_error(message: &str) {
    unsafe {
        MessageBoxW(
            null_mut(),
            ui(message).as_ptr(),
            wide("ALTRunNext").as_ptr(),
            MB_OK | MB_ICONERROR,
        );
    }
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
