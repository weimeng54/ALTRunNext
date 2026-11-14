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

fn swatch_colour(key: &str, value: &str) -> u32 {
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
    if !hwnd.is_null() {
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

pub(super) fn draw(item: &DRAWITEMSTRUCT) {
    if EDITOR.with(|c| c.borrow().is_none()) {
        return;
    }
    if item.CtlType == ODT_BUTTON {
        let id = item.CtlID as usize;
        if (SEGMENT..SEGMENT + 5).contains(&id) {
            let selected = if id < SEGMENT + 3 {
                let selected = unsafe { SendMessageW(field("Theme").value, CB_GETCURSEL, 0, 0) };
                selected == [2, 0, 1][id - SEGMENT]
            } else {
                let image =
                    unsafe { SendMessageW(field("ShowSkin").value, BM_GETCHECK, 0, 0) != 0 };
                image == (id == SEGMENT + 4)
            };
            let focused = item.itemState & ODS_FOCUS != 0;
            let color = if selected {
                theme::selection()
            } else {
                theme::paper()
            };
            ui::fill(item.hDC, &item.rcItem, theme::paper());
            let dpi = EDITOR.with(|c| c.borrow().as_ref().map_or(96, |e| e.dpi));
            ui::rounded(
                item.hDC,
                item.rcItem,
                scaled(8, dpi),
                color,
                if focused {
                    theme::accent()
                } else if selected {
                    theme::border()
                } else {
                    color
                },
            );
            draw_button_text(
                item,
                item.rcItem,
                if selected {
                    theme::text()
                } else {
                    theme::muted()
                },
                DT_CENTER,
            );
        } else if let Some((key, preset_index)) = if (SOLID_DEFAULT..=SOLID_PICK).contains(&id) {
            Some((
                "TitleColor",
                (id < SOLID_PICK).then_some(id - SOLID_DEFAULT),
            ))
        } else {
            id.checked_sub(PICK)
                .and_then(|i| SPECS.get(i))
                .filter(|spec| spec.key.ends_with("Color"))
                .map(|spec| (spec.key, None))
        } {
            let current = text(field(key).value);
            let value = preset_index.map_or_else(|| current.clone(), preset);
            let color = swatch_colour(key, &value);
            let rect = item.rcItem;
            ui::fill(item.hDC, &rect, theme::control_surface());
            let dpi = EDITOR.with(|c| c.borrow().as_ref().unwrap().dpi);
            ui::rounded(
                item.hDC,
                rect,
                scaled(10, dpi),
                theme::paper(),
                if item.itemState & ODS_FOCUS != 0 || (preset_index.is_some() && current == value) {
                    theme::accent()
                } else {
                    theme::border()
                },
            );
            let mut swatch = rect;
            unsafe {
                InflateRect(&mut swatch, -scaled(5, dpi), -scaled(5, dpi));
            }
            if id == SOLID_PICK {
                swatch.right = swatch.left + scaled(16, dpi);
                draw_button_text(
                    item,
                    RECT {
                        left: swatch.right + scaled(4, dpi),
                        ..rect
                    },
                    theme::text(),
                    DT_CENTER,
                );
            }
            ui::rounded(item.hDC, swatch, scaled(4, dpi), color, color);
        } else if let Some(index) = id.checked_sub(FIELD).filter(|index| {
            SPECS
                .get(*index)
                .is_some_and(|spec| matches!(spec.kind, Kind::Hotkey))
        }) {
            let dpi = EDITOR.with(|c| c.borrow().as_ref().unwrap().dpi);
            let rect = item.rcItem;
            let active = CAPTURE.get().is_some_and(|capture| capture.index == index);
            ui::fill(item.hDC, &rect, theme::control_surface());
            ui::rounded(
                item.hDC,
                rect,
                scaled(12, dpi),
                if active {
                    theme::selection()
                } else {
                    theme::paper()
                },
                if active || item.itemState & ODS_FOCUS != 0 {
                    theme::accent()
                } else {
                    theme::border()
                },
            );
            let value = text(item.hwndItem);
            let label = if value.is_empty() {
                crate::localization::tr("ui.not_set")
            } else {
                value
            };
            unsafe {
                let font = SendMessageW(item.hwndItem, WM_GETFONT, 0, 0) as HFONT;
                let old = SelectObject(item.hDC, font);
                ui::line_text(item.hDC, rect, &label, theme::text(), DT_CENTER);
                SelectObject(item.hDC, old);
            }
        } else if id == FONT_FAMILY {
            let dpi = EDITOR.with(|c| c.borrow().as_ref().map_or(96, |e| e.dpi));
            let rect = item.rcItem;
            ui::fill(item.hDC, &rect, theme::control_surface());
            ui::rounded(
                item.hDC,
                rect,
                scaled(12, dpi),
                if item.itemState & ODS_SELECTED != 0 {
                    theme::selection()
                } else {
                    theme::paper()
                },
                if item.itemState & ODS_FOCUS != 0 {
                    theme::accent()
                } else {
                    theme::border()
                },
            );
            draw_button_text(
                item,
                RECT {
                    left: rect.left + scaled(9, dpi),
                    right: rect.right - scaled(9, dpi),
                    ..rect
                },
                theme::text(),
                0,
            );
        } else if id == TITLE_CLOSE {
            ui::caption_close(item, EDITOR.with(|c| c.borrow().as_ref().unwrap().dpi));
        } else if id == MORE_COLORS {
            let (dpi, expanded) = EDITOR.with(|c| {
                let state = c.borrow();
                let s = state.as_ref().unwrap();
                (s.dpi, s.more_colors)
            });
            let rect = item.rcItem;
            let pressed = item.itemState & ODS_SELECTED != 0;
            let focused = item.itemState & ODS_FOCUS != 0 && item.itemState & ODS_NOFOCUSRECT == 0;
            ui::fill(item.hDC, &rect, theme::surface());
            ui::rounded(
                item.hDC,
                rect,
                scaled(10, dpi),
                if pressed {
                    theme::selection()
                } else {
                    theme::control_surface()
                },
                if focused {
                    theme::accent()
                } else {
                    theme::divider()
                },
            );
            let mut label = rect;
            label.left += scaled(31, dpi);
            draw_button_text(item, label, theme::text(), 0);
            ui::chevron(
                item.hDC,
                POINT {
                    x: rect.left + scaled(19, dpi),
                    y: (rect.top + rect.bottom) / 2,
                },
                if expanded {
                    ui::Chevron::Down
                } else {
                    ui::Chevron::Right
                },
                dpi,
                theme::muted(),
            );
        } else {
            let background = if (2310..=2314).contains(&id) {
                theme::chrome()
            } else {
                theme::control_surface()
            };
            let dpi = EDITOR.with(|c| c.borrow().as_ref().map_or(96, |s| s.dpi));
            ui::button_on(item, matches!(id, 2310 | OPEN_RELEASES), background, dpi);
        }
        return;
    }
    if item.CtlType == ODT_COMBOBOX {
        let dpi = EDITOR.with(|c| c.borrow().as_ref().map_or(96, |s| s.dpi));
        ui::combo(item, dpi);
        return;
    }
    let query = if item.CtlID as usize == PREVIEW {
        let input = EDITOR.with(|c| c.borrow().as_ref().map(|e| e.preview_controls[2]));
        input.map(text).unwrap_or_default()
    } else {
        String::new()
    };
    EDITOR.with(|c| {
        let state = c.borrow();
        let Some(s) = state.as_ref() else {
            return;
        };
        if item.CtlID as usize == PREVIEW {
            unsafe {
                let input = s.preview_controls[2];
                let mut input_rect: RECT = std::mem::zeroed();
                let clip = SaveDC(item.hDC);
                if clip != 0
                    && IsWindowVisible(input) != 0
                    && GetWindowRect(input, &mut input_rect) != 0
                {
                    MapWindowPoints(
                        null_mut(),
                        item.hwndItem,
                        &mut input_rect as *mut RECT as *mut POINT,
                        2,
                    );
                    ExcludeClipRect(
                        item.hDC,
                        input_rect.left,
                        input_rect.top,
                        input_rect.right,
                        input_rect.bottom,
                    );
                }
                s.preview.paint(item, s.dpi, &query);
                if clip != 0 {
                    RestoreDC(item.hDC, clip);
                }
            }
        } else if item.CtlID as usize == STATUS
            || item.CtlID as usize == FIELD + spec_index("BGFileName")
        {
            let id = item.CtlID as usize;
            let face = if id == STATUS {
                theme::chrome()
            } else {
                theme::control_surface()
            };
            ui::fill(item.hDC, &item.rcItem, face);
            let value = text(item.hwndItem);
            let value = if id == FIELD + spec_index("BGFileName") {
                if value.is_empty() {
                    crate::localization::tr("ui.no_image_selected")
                } else {
                    std::path::Path::new(&value)
                        .file_name()
                        .map_or(value.clone(), |name| name.to_string_lossy().into_owned())
                }
            } else {
                value
            };
            unsafe {
                let old = SelectObject(item.hDC, s.small_font);
                if id == STATUS {
                    let mut rect = item.rcItem;
                    SetBkMode(item.hDC, TRANSPARENT as i32);
                    SetTextColor(item.hDC, theme::muted());
                    DrawTextW(
                        item.hDC,
                        wide(&value).as_ptr(),
                        -1,
                        &mut rect,
                        DT_WORDBREAK | DT_NOPREFIX | DT_END_ELLIPSIS,
                    );
                } else {
                    ui::line_text(item.hDC, item.rcItem, &value, theme::muted(), 0);
                }
                SelectObject(item.hDC, old);
            }
        } else if item.CtlID as usize == IMAGE_THUMB {
            s.preview.paint_thumb(item, s.dpi);
        } else if item.CtlID as usize == TAB
            && let Some(page) = PAGES.get(item.itemID as usize)
        {
            let selected = item.itemState & ODS_SELECTED != 0;
            let mut bounds = item.rcItem;
            ui::fill(item.hDC, &bounds, theme::chrome());
            bounds.bottom -= scaled(5, s.dpi);
            if selected {
                ui::rounded(
                    item.hDC,
                    bounds,
                    scaled(12, s.dpi),
                    theme::selection(),
                    theme::selection(),
                );
                let rail = RECT {
                    left: bounds.left + scaled(3, s.dpi),
                    right: bounds.left + scaled(6, s.dpi),
                    top: bounds.top + scaled(10, s.dpi),
                    bottom: bounds.bottom - scaled(10, s.dpi),
                };
                ui::rounded(
                    item.hDC,
                    rail,
                    scaled(4, s.dpi),
                    theme::accent(),
                    theme::accent(),
                );
            }
            bounds.left += scaled(15, s.dpi);
            unsafe {
                let old = SelectObject(item.hDC, s.font);
                ui::line_text(
                    item.hDC,
                    bounds,
                    &crate::localization::tr(page.name),
                    theme::text(),
                    0,
                );
                SelectObject(item.hDC, old);
            }
        }
    });
}

fn draw_button_text(item: &DRAWITEMSTRUCT, rect: RECT, color: u32, flags: u32) {
    unsafe {
        let font = SendMessageW(item.hwndItem, WM_GETFONT, 0, 0) as HFONT;
        let old = SelectObject(item.hDC, font);
        ui::line_text(item.hDC, rect, &text(item.hwndItem), color, flags);
        SelectObject(item.hDC, old);
    }
}

pub(super) fn paint_settings(hwnd: HWND, dc: HDC) {
    let state = EDITOR.with(|c| {
        c.borrow().as_ref().map(|s| {
            (
                s.background,
                s.dpi,
                s.height,
                s.page,
                s.more_colors,
                s.caption_font,
            )
        })
    });
    let mut rect: RECT = unsafe { std::mem::zeroed() };
    unsafe {
        GetClientRect(hwnd, &mut rect);
    }
    let Some((background, dpi, height, page, more_colors, caption)) = state else {
        ui::fill(dc, &rect, theme::surface());
        return;
    };
    let px = |v| scaled(v, dpi);
    let divider = theme::divider();
    let foot = height - px(layout::FOOTER);
    let rail = px(layout::RAIL);
    unsafe {
        FillRect(dc, &rect, background);
    }
    ui::fill(
        dc,
        &RECT {
            bottom: px(layout::CAPTION),
            ..rect
        },
        theme::chrome(),
    );
    ui::fill(
        dc,
        &RECT {
            top: px(layout::CAPTION),
            right: rail,
            bottom: foot,
            ..rect
        },
        theme::chrome(),
    );
    ui::fill(dc, &RECT { top: foot, ..rect }, theme::chrome());
    for line in [
        RECT {
            top: px(layout::CAPTION),
            bottom: px(layout::CAPTION) + 1,
            ..rect
        },
        RECT {
            top: foot,
            bottom: foot + 1,
            ..rect
        },
        RECT {
            left: rail - 1,
            right: rail,
            top: px(layout::CAPTION),
            bottom: foot,
        },
    ] {
        ui::fill(dc, &line, divider);
    }
    ui::panel_caption(dc, dpi, caption, "ui.altrunnext_settings");
    if page == layout::ABOUT_PAGE {
        for (bounds, rules) in [
            (layout::ABOUT_PROFILE, &[80, 133][..]),
            (layout::ABOUT_LINKS, &[68, 136][..]),
        ] {
            let rect = bounds.pixels(dpi);
            ui::rounded(dc, rect, px(18), theme::control_surface(), divider);
            for &offset in rules {
                let y = px(bounds.y + offset);
                ui::fill(
                    dc,
                    &RECT {
                        left: rect.left + 1,
                        top: y,
                        right: rect.right - 1,
                        bottom: y + 1,
                    },
                    divider,
                );
            }
        }
        ui::draw_app_icon(dc, BoxRect::new(230, 151, 42, 42).pixels(dpi));
    }
    unsafe {
        let clip = SaveDC(dc);
        if clip != 0 {
            IntersectClipRect(dc, px(layout::CONTENT) - 1, px(150), rect.right, foot);
        }
        for card in layout::CARDS.iter().filter(|card| card.page == page) {
            let mut bounds = card.bounds;
            if page == 3 && bounds.y == 284 && !background_image() {
                bounds.h = 120;
            }
            let r = bounds.pixels(dpi);
            ui::rounded(dc, r, px(16), theme::control_surface(), divider);
            for &y in card.rules {
                let top = px(bounds.y + y);
                ui::fill(
                    dc,
                    &RECT {
                        left: r.left + 1,
                        top,
                        right: r.right - 1,
                        bottom: top + 1,
                    },
                    divider,
                );
            }
            if page == 3 && bounds.x == layout::RIGHT {
                ui::fill(dc, &BoxRect::new(798, 487, 1, 80).pixels(dpi), divider);
            }
        }
        if page == 3 {
            for segment in [
                BoxRect::new(226, 191, 266, 38),
                BoxRect::new(226, 295, 266, 38),
            ] {
                ui::rounded(
                    dc,
                    segment.pixels(dpi),
                    px(14),
                    theme::paper(),
                    theme::border(),
                );
            }
        }
        if page == 4 && more_colors {
            let r = layout::EXTRA_COLORS.pixels(dpi);
            ui::rounded(dc, r, px(16), theme::control_surface(), divider);
            for y in [518, 558, 598] {
                ui::fill(dc, &BoxRect::new(525, y, 547, 1).pixels(dpi), divider);
            }
        }
        if clip != 0 {
            RestoreDC(dc, clip);
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
