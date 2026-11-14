// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::*;

pub(super) fn extra_color(key: &str) -> bool {
    matches!(
        key,
        "TitleColor" | "KeywordColor" | "ListColor" | "SelectionColor"
    )
}

pub(super) fn background_image() -> bool {
    unsafe { SendMessageW(field("ShowSkin").value, BM_GETCHECK, 0, 0) != 0 }
}

pub(super) fn preset(index: usize) -> String {
    if index == 0 {
        return SPECS[spec_index("TitleColor")].default.into();
    }
    let (_, light, dark) = PRESETS[index - 1];
    if theme::is_dark() { dark } else { light }.into()
}

pub(super) fn swatch_colour(key: &str, value: &str) -> u32 {
    if theme::high_contrast() {
        return theme::paper();
    }
    if value == SPECS[spec_index(key)].default {
        match key {
            "TitleColor" => theme::surface(),
            "SelectionColor" => theme::selection(),
            "KeywordColor" | "ListColor" => theme::paper(),
            _ => altrun_probe::preferences::parse_colour(value).unwrap_or_else(|_| theme::text()),
        }
    } else {
        altrun_probe::preferences::parse_colour(value).unwrap_or_else(|_| theme::text())
    }
}

pub(super) fn set_colour(key: &str, value: &str) {
    let colour = field(key);
    unsafe {
        SetWindowTextW(colour.value, wide(value).as_ptr());
        InvalidateRect(colour.pick, std::ptr::null(), 0);
        if key == "TextColor" {
            SendMessageW(field("UseCustomTextColor").value, BM_SETCHECK, 1, 0);
        }
        if key == "TitleColor" {
            let window = GetParent(colour.value);
            for id in SOLID_DEFAULT..=SOLID_PICK {
                InvalidateRect(GetDlgItem(window, id as i32), std::ptr::null(), 0);
            }
        }
    }
    update_preview();
}

pub(super) fn choose_colour(hwnd: HWND, key: &str) {
    let current = text(field(key).value);
    let rgb = swatch_colour(key, &current);
    let initial = format!(
        "#{:02X}{:02X}{:02X}",
        rgb & 255,
        (rgb >> 8) & 255,
        (rgb >> 16) & 255
    );
    if let Some(chosen) = pickers::choose(hwnd, key, &initial) {
        set_colour(key, &chosen);
    }
}

pub(super) fn update_background_mode() {
    let (window, page, dpi, children) = EDITOR.with(|c| {
        let state = c.borrow();
        let s = state.as_ref().unwrap();
        (s.hwnd, s.page, s.dpi, s.appearance_children.clone())
    });
    if page != 3 {
        return;
    }
    let image = background_image();
    for child in children {
        let id = unsafe { GetDlgCtrlID(child) } as usize;
        if image_control(id) || solid_control(id) {
            show_without_redraw(child, shows_in_background_mode(id, image));
        }
    }
    repaint_section(window, BoxRect::new(layout::CONTENT, 284, 290, 227), dpi);
}

pub(super) fn show_without_redraw(hwnd: HWND, visible: bool) {
    if !hwnd.is_null()
        && unsafe { (GetWindowLongW(hwnd, GWL_STYLE) as u32 & WS_VISIBLE != 0) != visible }
    {
        unsafe {
            SetWindowPos(
                hwnd,
                null_mut(),
                0,
                0,
                0,
                0,
                SWP_NOMOVE
                    | SWP_NOSIZE
                    | SWP_NOACTIVATE
                    | SWP_NOZORDER
                    | SWP_NOREDRAW
                    | if visible {
                        SWP_SHOWWINDOW
                    } else {
                        SWP_HIDEWINDOW
                    },
            );
        }
    }
}

fn repaint_section(window: HWND, bounds: BoxRect, dpi: i32) {
    unsafe {
        RedrawWindow(
            window,
            &bounds.pixels(dpi),
            null_mut(),
            RDW_INVALIDATE | RDW_ALLCHILDREN | RDW_NOERASE,
        );
    }
}

pub(super) fn toggle_extra_colors(window: HWND) {
    let (expanded, dpi) = EDITOR.with(|c| {
        let mut state = c.borrow_mut();
        let s = state.as_mut().unwrap();
        s.more_colors = !s.more_colors;
        (s.more_colors, s.dpi)
    });
    for (spec, field) in SPECS.iter().zip(fields()) {
        if extra_color(spec.key) {
            show_without_redraw(field.label, expanded);
            show_without_redraw(field.pick, expanded);
        }
    }
    repaint_section(window, layout::EXTRA_COLORS, dpi);
    unsafe {
        InvalidateRect(GetDlgItem(window, MORE_COLORS as i32), std::ptr::null(), 0);
    }
}

pub(super) fn align_preview() {
    let (query, dpi, footer) = EDITOR.with(|c| {
        let state = c.borrow();
        let s = state.as_ref().unwrap();
        (
            s.preview_controls[2],
            s.dpi,
            s.height - scaled(layout::FOOTER, s.dpi),
        )
    });
    let query_text = text(query);
    let (panel, input_height) = EDITOR.with(|c| {
        let state = c.borrow();
        let preview = &state.as_ref().unwrap().preview;
        let (bounds, input, _) = preview.geometry(preview.matches(&query_text).len());
        (bounds, input)
    });
    let px = |value| scaled(value, dpi);
    let x = px(layout::PREVIEW.x + panel.x + 9);
    let y = px(layout::PREVIEW.y + panel.y + 35);
    let height = px(input_height);
    let width = px(panel.w - 18);
    let top = (px(150) - y).max(0);
    let bottom = height.min(footer - y);
    let position = (x, y, width, height, top, bottom);
    if !EDITOR.with(|c| {
        let mut state = c.borrow_mut();
        let s = state.as_mut().unwrap();
        s.preview_position.replace(position) != Some(position)
    }) {
        return;
    }
    unsafe {
        let visible = top < bottom;
        if visible {
            SetWindowPos(query, HWND_TOP, x, y, width, height, SWP_NOACTIVATE);
            let clip = if top != 0 || bottom != height {
                CreateRectRgn(0, top, width, bottom)
            } else {
                null_mut()
            };
            if SetWindowRgn(query, clip, 0) == 0 && !clip.is_null() {
                DeleteObject(clip);
            }
        }
        ShowWindow(query, if visible { SW_SHOWNA } else { SW_HIDE });
        if visible {
            RedrawWindow(
                query,
                std::ptr::null(),
                null_mut(),
                RDW_INVALIDATE | RDW_FRAME,
            );
        }
    }
}

pub(super) fn update_preview() {
    let (page, controls, dpi) = EDITOR.with(|c| {
        let editor = c.borrow();
        let editor = editor.as_ref().unwrap();
        (editor.page, editor.preview_controls, editor.dpi)
    });
    if !matches!(page, 3 | 4) {
        return;
    }
    let Ok(prefs) = read_form() else {
        return;
    };
    if EDITOR.with(|c| {
        let state = c.borrow();
        let s = state.as_ref().unwrap();
        s.preview.prefs == prefs && !s.preview_font.is_null()
    }) {
        return;
    }
    let input_changed = EDITOR.with(|c| {
        let mut editor = c.borrow_mut();
        let editor = editor.as_mut().unwrap();
        let previous = editor.preview.input_colours;
        editor
            .preview
            .update(prefs.clone(), &editor.data, editor.hwnd);
        if editor.preview_paper.is_null() || previous.1 != editor.preview.input_colours.1 {
            unsafe {
                if !editor.preview_paper.is_null() {
                    DeleteObject(editor.preview_paper);
                }
                editor.preview_paper = CreateSolidBrush(editor.preview.input_colours.1);
            }
        }
        previous != editor.preview.input_colours
    });
    refresh_style(&prefs);
    align_preview();
    unsafe {
        let key = prefs.get("KeywordFont");
        let changed = EDITOR.with(|c| c.borrow().as_ref().unwrap().preview_font_key != key);
        if changed && let Ok(font) = altrun_probe::preferences::Font::parse(&key) {
            let font = make_font(font.size_dip(), font.weight, &font.name, dpi);
            if !font.is_null() {
                SendMessageW(controls[2], WM_SETFONT, font as usize, 1);
                let old = EDITOR.with(|c| {
                    let mut state = c.borrow_mut();
                    let s = state.as_mut().unwrap();
                    s.preview_font_key = key;
                    std::mem::replace(&mut s.preview_font, font)
                });
                if !old.is_null() {
                    DeleteObject(old);
                }
            }
        }
        InvalidateRect(controls[1], std::ptr::null(), 0);
        if input_changed {
            RedrawWindow(
                controls[2],
                std::ptr::null(),
                null_mut(),
                RDW_INVALIDATE | RDW_FRAME,
            );
        }
    }
}

pub(super) fn strength_label(field: Field, strength: i32) {
    unsafe {
        let value = GetDlgItem(GetParent(field.value), STRENGTH_VALUE as i32);
        let formatted = strength.to_string();
        if text(value) != formatted {
            SetWindowTextW(value, wide(&formatted).as_ptr());
        }
    }
}

pub(super) fn refresh_font_family(prefs: &Preferences) {
    if let Ok(font) = altrun_probe::preferences::Font::parse(&prefs.get("TitleFont")) {
        let hwnd = EDITOR.with(|c| c.borrow().as_ref().unwrap().hwnd);
        unsafe {
            SetWindowTextW(
                GetDlgItem(hwnd, FONT_FAMILY as i32),
                wide(&font.name).as_ptr(),
            );
        }
    }
}

pub(super) fn choose_panel_font(hwnd: HWND) {
    let draft = EDITOR.with(|c| c.borrow().as_ref().unwrap().draft.clone());
    if let Some(choice) = pickers::choose(hwnd, "TitleFont", &draft.get("TitleFont"))
        && let Ok(font) = altrun_probe::preferences::Font::parse(&choice)
    {
        let prefs = EDITOR.with(|c| {
            let mut editor = c.borrow_mut();
            let draft = &mut editor.as_mut().unwrap().draft;
            for key in ["TitleFont", "KeywordFont", "ListFont"] {
                let value = draft.get(key);
                if let Some((_, rest)) = value.split_once('|') {
                    let _ = draft.set(key, &format!("{}|{rest}", font.name));
                }
            }
            draft.clone()
        });
        refresh_font_family(&prefs);
        update_preview();
    }
}

pub(super) fn adjust_number(hwnd: HWND, id: usize, delta: i32) {
    let (control, fallback, min, max) = if id == STRENGTH_VALUE {
        let slider = field("BGStrength").value;
        let value = unsafe { SendMessageW(slider, TBM_GETPOS, 0, 0) } as i32;
        (unsafe { GetDlgItem(hwnd, id as i32) }, value, 0, 100)
    } else {
        let Some(index) = id.checked_sub(FIELD) else {
            return;
        };
        let Some(spec) = SPECS.get(index) else { return };
        let value = EDITOR.with(|state| state.borrow().as_ref().unwrap().draft.get(spec.key));
        let (fallback, min, max) = match spec.kind {
            Kind::Font => (
                altrun_probe::preferences::Font::parse(&value).map_or(13, |font| font.size_dip()),
                6,
                128,
            ),
            Kind::Number(min, max) => (value.parse().unwrap_or(min as i32), min as i32, max as i32),
            _ => return,
        };
        (fields()[index].value, fallback, min, max)
    };
    let old = text(control);
    let value = old
        .trim()
        .parse::<i32>()
        .unwrap_or(fallback)
        .saturating_add(delta)
        .clamp(min, max)
        .to_string();
    if old != value {
        unsafe {
            SetWindowTextW(control, wide(&value).as_ptr());
        }
    }
}

pub(super) fn refresh_style(prefs: &Preferences) {
    theme::install_chrome(prefs);
    let colours = chrome_colors();
    let hwnd = EDITOR.with(|c| {
        let mut state = c.borrow_mut();
        let s = state.as_mut().unwrap();
        if s.chrome_colors == colours {
            return None;
        }
        s.chrome_colors = colours;
        Some(s.hwnd)
    });
    let Some(hwnd) = hwnd else { return };
    theme::frame(hwnd);
    unsafe {
        let background = CreateSolidBrush(crate::theme::surface());
        let paper = CreateSolidBrush(crate::theme::paper());
        EDITOR.with(|c| {
            let mut s = c.borrow_mut();
            let s = s.as_mut().unwrap();
            DeleteObject(std::mem::replace(&mut s.background, background));
            DeleteObject(std::mem::replace(&mut s.paper, paper));
            DeleteObject(std::mem::replace(
                &mut s.rail,
                CreateSolidBrush(theme::chrome()),
            ));
        });
        RedrawWindow(
            hwnd,
            std::ptr::null(),
            null_mut(),
            RDW_INVALIDATE | RDW_ALLCHILDREN | RDW_NOERASE,
        );
    }
}

pub(super) fn chrome_colors() -> [u32; 5] {
    [
        theme::surface(),
        theme::paper(),
        theme::chrome(),
        theme::text(),
        theme::accent(),
    ]
}
