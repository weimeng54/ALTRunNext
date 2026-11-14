// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::*;

pub(super) fn show_page(page: usize) {
    let Some(design) = PAGES.get(page) else {
        return;
    };
    cancel_capture();
    let (window, tabs, title, hint, sections, extras, preview, more_colors) = EDITOR.with(|c| {
        let mut editor = c.borrow_mut();
        let editor = editor.as_mut().unwrap();
        editor.page = page;
        (
            editor.hwnd,
            editor.tabs,
            editor.title,
            editor.hint,
            editor.sections.clone(),
            editor.extra_controls.clone(),
            editor.preview_controls,
            editor.more_colors,
        )
    });
    unsafe {
        SendMessageW(tabs, LB_SETCURSEL, page, 0);
        SetWindowTextW(title, ui(design.name).as_ptr());
        SetWindowTextW(hint, ui(design.hint).as_ptr());
        show_without_redraw(hint, !design.hint.is_empty());
        SendMessageW(tabs, LB_SETTOPINDEX, 0, 0);
        for (spec, field) in SPECS.iter().zip(fields()) {
            let visible = field.page == page
                && !hidden_field(spec.key)
                && (!extra_color(spec.key) || more_colors);
            for hwnd in [field.label, field.value, field.pick, field.clear] {
                let is_color_value = hwnd == field.value
                    && matches!(spec.kind, Kind::Text)
                    && spec.key.ends_with("Color");
                show_without_redraw(hwnd, visible && !is_color_value);
            }
        }
        for (group, hwnd) in sections {
            show_without_redraw(hwnd, group == page);
        }
        for (group, hwnd) in extras {
            show_without_redraw(hwnd, group == page);
        }
        for hwnd in preview {
            show_without_redraw(hwnd, matches!(page, 3 | 4));
        }
    }
    sync_save_button(window);
    update_background_mode();
    update_preview();
    if matches!(page, 3 | 4) {
        align_preview();
    }
    unsafe {
        RedrawWindow(
            window,
            std::ptr::null(),
            null_mut(),
            RDW_INVALIDATE | RDW_ALLCHILDREN | RDW_NOERASE,
        );
    }
}

pub(super) fn choices(key: &str) -> &'static [(&'static str, &'static str)] {
    match key {
        "Theme" => &[
            ("light", "ui.light"),
            ("dark", "ui.dark"),
            ("system", "settings.follow_system"),
        ],
        "BGCrop" => &[
            ("top", "ui.top"),
            ("center", "ui.center"),
            ("bottom", "ui.bottom"),
        ],
        _ => &[],
    }
}

fn hidden_field(key: &str) -> bool {
    matches!(key, "Theme" | "ShowSkin")
}

pub(super) fn switch_field(spec: &altrun_probe::preferences::Spec) -> bool {
    matches!(spec.kind, Kind::Bool) || spec.key == "PositionMode"
}

pub(super) fn image_control(id: usize) -> bool {
    matches!(
        id,
        IMAGE_THUMB | STRENGTH_LABEL | STRENGTH_VALUE | STRENGTH_UNIT | CROP_LABEL
    ) || [FIELD, PICK].iter().any(|start| {
        id.checked_sub(*start)
            .and_then(|i| SPECS.get(i))
            .is_some_and(|spec| matches!(spec.key, "BGFileName" | "BGStrength" | "BGCrop"))
    })
}

pub(super) fn solid_control(id: usize) -> bool {
    (SOLID_LABEL..=SOLID_PICK).contains(&id)
}

pub(super) fn shows_in_background_mode(id: usize, image: bool) -> bool {
    (!image_control(id) || image) && (!solid_control(id) || !image)
}

pub(super) fn open_about_link(hwnd: HWND, url: &str, error: &str) {
    let result = unsafe {
        ShellExecuteW(
            hwnd,
            wide("open").as_ptr(),
            wide(url).as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    if result as usize <= 32 {
        status(error);
    }
}
