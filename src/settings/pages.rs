// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::*;

pub(super) fn show_page(page: usize) {
    let Some(design) = PAGES.get(page) else {
        return;
    };
    cancel_capture();
    let (window, tabs, title, sections, extras, preview, more_colors, dpi, height) =
        EDITOR.with(|c| {
            let mut editor = c.borrow_mut();
            let editor = editor.as_mut().unwrap();
            editor.page = page;
            (
                editor.hwnd,
                editor.tabs,
                editor.title,
                editor.sections.clone(),
                editor.extra_controls.clone(),
                editor.preview_controls,
                editor.more_colors,
                editor.dpi,
                editor.height,
            )
        });
    let image = page == 3 && background_image();
    let showing_preview = matches!(page, 3 | 4);
    unsafe {
        if SendMessageW(tabs, LB_GETCURSEL, 0, 0) != page as isize {
            SendMessageW(tabs, LB_SETCURSEL, page, 0);
        }
        SetWindowTextW(title, ui(design.name).as_ptr());
        for (spec, field) in SPECS.iter().zip(fields()) {
            let visible = field.page == page
                && !hidden_field(spec.key)
                && (!extra_color(spec.key) || more_colors);
            for hwnd in [field.label, field.value, field.pick, field.clear] {
                if hwnd.is_null() {
                    continue;
                }
                let is_color_value = hwnd == field.value
                    && matches!(spec.kind, Kind::Text)
                    && spec.key.ends_with("Color");
                show_without_redraw(
                    hwnd,
                    visible
                        && !is_color_value
                        && (page != 3
                            || shows_in_background_mode(GetDlgCtrlID(hwnd) as usize, image)),
                );
            }
        }
        for (group, hwnd) in sections {
            show_without_redraw(hwnd, group == page);
        }
        for (group, hwnd) in extras {
            show_without_redraw(
                hwnd,
                group == page
                    && (page != 3 || shows_in_background_mode(GetDlgCtrlID(hwnd) as usize, image)),
            );
        }
        for hwnd in preview {
            show_without_redraw(hwnd, showing_preview);
        }
    }
    sync_save_button(window);
    if showing_preview {
        update_preview();
        align_preview();
    }
    unsafe {
        let mut content = RECT::default();
        GetClientRect(window, &mut content);
        content.left = scaled(layout::RAIL, dpi);
        content.top = scaled(layout::CAPTION, dpi) + 1;
        content.bottom = height - scaled(layout::FOOTER, dpi);
        // The sidebar, caption and footer own their repaint independently.
        RedrawWindow(
            window,
            &content,
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
