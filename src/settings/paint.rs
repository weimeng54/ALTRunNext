// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::{appearance::swatch_colour, *};

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
                    let words = wide(&value);
                    let flags = DT_WORDBREAK | DT_NOPREFIX | DT_END_ELLIPSIS;
                    let mut rect = item.rcItem;
                    let mut measured = rect;
                    SetBkMode(item.hDC, TRANSPARENT as i32);
                    SetTextColor(item.hDC, theme::muted());
                    let height = DrawTextW(
                        item.hDC,
                        words.as_ptr(),
                        -1,
                        &mut measured,
                        flags | DT_CALCRECT,
                    );
                    rect.top += ((rect.bottom - rect.top - height) / 2).max(0);
                    DrawTextW(item.hDC, words.as_ptr(), -1, &mut rect, flags);
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
