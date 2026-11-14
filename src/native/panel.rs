// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::*;

pub(super) fn show_inline_error(message: &str) {
    let (hwnd, edit, error) = STATE.with(|c| {
        let mut s = c.borrow_mut();
        let s = s.as_mut().unwrap();
        s.error = true;
        (s.hwnd, s.edit, s.inline_error)
    });
    unsafe {
        let message = crate::localization::tr(message)
            .replace("\r\n", "\n")
            .replace('\n', "\r\n");
        SetWindowTextW(error, wide(&message).as_ptr());
        layout_inline_error();
        ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        SetForegroundWindow(hwnd);
        SetFocus(edit);
    }
}

pub(super) fn layout_inline_error() {
    let (hwnd, list, command, error, font, dpi, visible, preview) = STATE.with(|c| {
        let state = c.borrow();
        let s = state.as_ref().unwrap();
        (
            s.hwnd,
            s.list,
            s.command,
            s.inline_error,
            s.error_font,
            s.dpi,
            s.error,
            s.preferences.flag("ShowCommandLine"),
        )
    });
    unsafe {
        ShowWindow(
            command,
            if preview && !visible {
                SW_SHOWNOACTIVATE
            } else {
                SW_HIDE
            },
        );
        ShowWindow(error, SW_HIDE);
        let mut panel = RECT::default();
        let mut rows = RECT::default();
        GetClientRect(hwnd, &mut panel);
        GetClientRect(list, &mut rows);
        let mut origin = POINT {
            x: 0,
            y: rows.bottom,
        };
        ClientToScreen(list, &mut origin);
        ScreenToClient(hwnd, &mut origin);
        let y = origin.y + scaled(8, dpi);
        let inset = scaled(12, dpi);
        let mut height = 0;
        if visible {
            let dc = GetDC(error);
            let old = SelectObject(dc, font);
            let mut measure = RECT {
                right: (panel.right - 2 * inset).max(1),
                ..Default::default()
            };
            let text = wide(&edit_text(error));
            height = DrawTextW(
                dc,
                text.as_ptr(),
                -1,
                &mut measure,
                DT_CALCRECT | DT_WORDBREAK | DT_NOPREFIX | DT_EDITCONTROL,
            )
            .max(scaled(16, dpi));
            SelectObject(dc, old);
            ReleaseDC(error, dc);
            SetWindowPos(
                error,
                null_mut(),
                inset,
                y,
                (panel.right - 2 * inset).max(1),
                height,
                SWP_NOZORDER | SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
        }
        let height = if visible {
            y + height + scaled(5, dpi)
        } else {
            origin.y
                + scaled(
                    if preview {
                        theme::BASE_HEIGHT - theme::LIST_Y
                    } else {
                        70 - theme::LIST_Y
                    },
                    dpi,
                )
        };
        SetWindowPos(
            hwnd,
            null_mut(),
            0,
            0,
            panel.right,
            height,
            SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
        );
        let radius = scaled(16, dpi);
        let region = CreateRoundRectRgn(0, 0, panel.right + 1, height + 1, radius, radius);
        if !region.is_null() && SetWindowRgn(hwnd, region, 0) == 0 {
            DeleteObject(region);
        }
        InvalidateRect(hwnd, std::ptr::null(), 1);
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct PanelLayout {
    width: i32,
    row: i32,
    height: i32,
    dpi: i32,
    show_command: bool,
}

pub(super) fn render_hits() {
    let (hwnd, dpi, count) = STATE.with(|c| {
        let s = c.borrow();
        let s = s.as_ref().unwrap();
        (s.hwnd, s.dpi, s.hits.len())
    });
    let (mut width, row, mut max_rows, show_command) = STATE.with(|c| {
        let s = c.borrow();
        let s = s.as_ref().unwrap();
        (
            s.preferences.number("FormWidth"),
            s.preferences.number("RowHeight"),
            s.preferences.number("MaxRows"),
            s.preferences.flag("ShowCommandLine"),
        )
    });
    let row_pixels = scaled(row, dpi).max(1);
    unsafe {
        let mut monitor: MONITORINFO = zeroed();
        monitor.cbSize = std::mem::size_of_val(&monitor) as u32;
        if GetMonitorInfoW(
            MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST),
            &mut monitor,
        ) != 0
        {
            width =
                width.min(((monitor.rcWork.right - monitor.rcWork.left) * 96 / dpi - 2).max(160));
            let available =
                monitor.rcWork.bottom - monitor.rcWork.top - scaled(theme::BASE_HEIGHT, dpi);
            max_rows = max_rows.min((available / row_pixels).max(1));
        }
    }
    let layout = PanelLayout {
        width,
        row,
        height: (count.max(1) as i32).min(max_rows) * row_pixels,
        dpi,
        show_command,
    };
    let (list, empty, show_empty, labels, resize) = STATE.with(|c| {
        let mut state = c.borrow_mut();
        let s = state.as_mut().unwrap();
        let labels = std::mem::take(&mut s.list_dirty).then(|| {
            s.hits
                .iter()
                .map(|&i| {
                    let entry = &s.entries[i];
                    entry
                        .alias
                        .encode_utf16()
                        .chain(" · ".encode_utf16())
                        .chain(entry.name.encode_utf16())
                        .chain(Some(0))
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>()
        });
        let resize = s.layout != Some(layout);
        s.layout = Some(layout);
        (
            s.list,
            s.empty,
            count == 0 && !s.current_query.is_empty(),
            labels,
            resize,
        )
    });
    unsafe {
        if let Some(labels) = labels {
            SendMessageW(list, WM_SETREDRAW, 0, 0);
            SendMessageW(list, LB_RESETCONTENT, 0, 0);
            if !labels.is_empty() {
                let bytes = labels
                    .iter()
                    .map(|label| label.len() * size_of::<u16>())
                    .sum::<usize>();
                SendMessageW(list, LB_INITSTORAGE, labels.len(), bytes as LPARAM);
            }
            for label in &labels {
                SendMessageW(list, LB_ADDSTRING, 0, label.as_ptr() as LPARAM);
            }
            SendMessageW(list, LB_SETCURSEL, 0, 0);
            SendMessageW(list, WM_SETREDRAW, 1, 0);
            InvalidateRect(list, std::ptr::null(), 0);
        } else if count != 0 {
            // A new query starts at the first result even if its rows are identical.
            if SendMessageW(list, LB_GETCURSEL, 0, 0) != 0 {
                SendMessageW(list, LB_SETCURSEL, 0, 0);
            }
            if SendMessageW(list, LB_GETTOPINDEX, 0, 0) != 0 {
                SendMessageW(list, LB_SETTOPINDEX, 0, 0);
            }
        }
        if resize {
            layout_results(hwnd, layout);
        }
        if (GetWindowLongW(empty, GWL_STYLE) as u32 & WS_VISIBLE != 0) != show_empty {
            ShowWindow(
                empty,
                if show_empty {
                    SW_SHOWNOACTIVATE
                } else {
                    SW_HIDE
                },
            );
        }
    }
    update_selection();
}

fn layout_results(hwnd: HWND, layout: PanelLayout) {
    let PanelLayout {
        width,
        row,
        height,
        dpi,
        show_command,
    } = layout;
    let (list, edit, title, command, empty, buttons) = STATE.with(|c| {
        let state = c.borrow();
        let s = state.as_ref().unwrap();
        (s.list, s.edit, s.title, s.command, s.empty, s.buttons)
    });
    unsafe {
        SetWindowPos(
            empty,
            null_mut(),
            scaled(12, dpi),
            scaled(theme::LIST_Y + 2, dpi),
            scaled(width - 24, dpi),
            scaled(row, dpi),
            SWP_NOZORDER | SWP_NOACTIVATE,
        );
        SetWindowPos(
            edit,
            null_mut(),
            scaled(18, dpi),
            scaled(35, dpi),
            scaled(width - 36, dpi),
            scaled(22, dpi),
            SWP_NOZORDER | SWP_NOACTIVATE,
        );
        let mut offset = 10;
        for button in buttons.into_iter().rev() {
            offset += 24;
            SetWindowPos(
                button,
                null_mut(),
                scaled(width - offset, dpi),
                scaled(4, dpi),
                scaled(22, dpi),
                scaled(22, dpi),
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
        SetWindowPos(
            title,
            null_mut(),
            scaled(12, dpi),
            scaled(6, dpi),
            scaled(width - offset - 20, dpi),
            scaled(20, dpi),
            SWP_NOZORDER | SWP_NOACTIVATE,
        );
        SetWindowPos(
            list,
            null_mut(),
            scaled(8, dpi),
            scaled(theme::LIST_Y, dpi),
            scaled(width - 16, dpi),
            height,
            SWP_NOZORDER | SWP_NOACTIVATE,
        );
        SetWindowPos(
            command,
            null_mut(),
            scaled(12, dpi),
            scaled(73, dpi) + height,
            scaled(width - 24, dpi),
            scaled(16, dpi),
            SWP_NOZORDER | SWP_NOACTIVATE,
        );
        SetWindowPos(
            hwnd,
            null_mut(),
            0,
            0,
            scaled(width, dpi),
            scaled(if show_command { theme::BASE_HEIGHT } else { 70 }, dpi) + height,
            SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
        );
        let diameter = scaled(16, dpi);
        let region = CreateRoundRectRgn(
            0,
            0,
            scaled(width, dpi) + 1,
            scaled(if show_command { theme::BASE_HEIGHT } else { 70 }, dpi) + height + 1,
            diameter,
            diameter,
        );
        if !region.is_null() && SetWindowRgn(hwnd, region, 1) == 0 {
            DeleteObject(region);
        }
        InvalidateRect(hwnd, std::ptr::null(), 0);
    }
}

pub(super) fn paint_title(hwnd: HWND, item: &DRAWITEMSTRUCT) {
    let name = crate::ui::window_text(item.hwndItem);
    STATE.with(|state| {
        let state = state.borrow();
        let Some(s) = state.as_ref() else {
            return;
        };
        let dc = item.hDC;
        unsafe {
            FillRect(dc, &item.rcItem, s.background);
            if let Some(bitmap) = &s.skin {
                let mut origin = POINT { x: 0, y: 0 };
                let mut rect: RECT = zeroed();
                GetClientRect(hwnd, &mut rect);
                MapWindowPoints(item.hwndItem, hwnd, &mut origin, 1);
                OffsetRect(&mut rect, -origin.x, -origin.y);
                let old = SaveDC(dc);
                if old != 0 {
                    IntersectClipRect(
                        dc,
                        item.rcItem.left,
                        item.rcItem.top,
                        item.rcItem.right,
                        item.rcItem.bottom,
                    );
                    bitmap.draw(dc, rect, &s.preferences.get("BGCrop"));
                    RestoreDC(dc, old);
                }
            }
            let font = SelectObject(dc, s.title_font);
            crate::ui::line_text(dc, item.rcItem, &name, s.font_colours[0], 0);
            SelectObject(dc, font);
        }
    });
}

pub(super) fn paint_panel(hwnd: HWND) {
    unsafe {
        let mut ps: PAINTSTRUCT = zeroed();
        let dc = BeginPaint(hwnd, &mut ps);
        STATE.with(|c| {
            let s = c.borrow();
            let Some(s) = s.as_ref() else {
                return;
            };
            if let Some(bitmap) = &s.skin {
                let mut rect: RECT = zeroed();
                GetClientRect(hwnd, &mut rect);
                bitmap.draw(dc, rect, &s.preferences.get("BGCrop"));
            }
            let old_brush = SelectObject(dc, s.input_paper);
            let old_pen = SelectObject(dc, s.border);
            let mut panel: RECT = zeroed();
            GetClientRect(hwnd, &mut panel);
            RoundRect(
                dc,
                scaled(10, s.dpi),
                scaled(29, s.dpi),
                panel.right - scaled(10, s.dpi),
                scaled(60, s.dpi),
                scaled(8, s.dpi),
                scaled(8, s.dpi),
            );
            let mut rect: RECT = zeroed();
            GetClientRect(hwnd, &mut rect);
            if s.preferences.flag("ShowCommandLine") || s.error {
                let mut label = RECT::default();
                if s.error {
                    GetWindowRect(s.inline_error, &mut label);
                    let mut start = POINT {
                        x: label.left,
                        y: label.top,
                    };
                    ScreenToClient(hwnd, &mut start);
                    rect.bottom = start.y + scaled(4, s.dpi);
                }
                MoveToEx(
                    dc,
                    scaled(12, s.dpi),
                    rect.bottom - scaled(if s.error { 8 } else { 25 }, s.dpi),
                    std::ptr::null_mut(),
                );
                LineTo(
                    dc,
                    rect.right - scaled(12, s.dpi),
                    rect.bottom - scaled(if s.error { 8 } else { 25 }, s.dpi),
                );
            }
            SelectObject(dc, old_brush);
            SelectObject(dc, old_pen);
        });
        EndPaint(hwnd, &ps);
    }
}

pub(super) fn apply_appearance() {
    let (hwnd, edit, list, title, command, inline_error, prefs, dpi) = STATE.with(|c| {
        let s = c.borrow();
        let s = s.as_ref().unwrap();
        (
            s.hwnd,
            s.edit,
            s.list,
            s.title,
            s.command,
            s.inline_error,
            s.preferences.clone(),
            s.dpi,
        )
    });
    theme::install(&prefs);
    unsafe {
        let error_font = make_font(12, 400, "Microsoft YaHei UI", dpi);
        if !error_font.is_null() {
            SendMessageW(inline_error, WM_SETFONT, error_font as WPARAM, 0);
            STATE.with(|c| {
                DeleteObject(std::mem::replace(
                    &mut c.borrow_mut().as_mut().unwrap().error_font,
                    error_font,
                ))
            });
        }
        STATE.with(|c| {
            let mut s = c.borrow_mut();
            let s = s.as_mut().unwrap();
            let background = CreateSolidBrush(theme::surface());
            for (slot, new) in [
                (&mut s.background, background),
                (&mut s.paper, CreateSolidBrush(theme::paper())),
                (&mut s.selected_brush, CreateSolidBrush(theme::selection())),
                (&mut s.accent, CreateSolidBrush(theme::accent())),
                (&mut s.input_paper, CreateSolidBrush(theme::keyword())),
            ] {
                let old = std::mem::replace(slot, new);
                DeleteObject(old);
            }
            let old = std::mem::replace(&mut s.border, CreatePen(PS_SOLID, 1, theme::border()));
            DeleteObject(old);
        });
        STATE.with(|c| {
            let mut s = c.borrow_mut();
            let s = s.as_mut().unwrap();
            s.font_colours = [
                theme::font_colour(&prefs.get("TitleFont")),
                theme::font_colour(&prefs.get("KeywordFont")),
                theme::font_colour(&prefs.get("ListFont")),
            ];
        });
        let definitions = [
            ("ListFont", list),
            ("TitleFont", title),
            ("KeywordFont", edit),
        ];
        let mut fonts = Vec::new();
        for (key, control) in definitions {
            if let Ok(f) = altrun_probe::preferences::Font::parse(&prefs.get(key)) {
                let font = CreateFontW(
                    -scaled(f.size_dip(), dpi),
                    0,
                    0,
                    0,
                    f.weight,
                    u32::from(f.italic),
                    u32::from(f.underline),
                    u32::from(f.strike_out),
                    u32::from(f.charset),
                    0,
                    0,
                    CLEARTYPE_QUALITY as u32,
                    0,
                    wide(&f.name).as_ptr(),
                );
                if !font.is_null() {
                    SendMessageW(control, WM_SETFONT, font as usize, 1);
                    if key == "ListFont" {
                        let empty = STATE.with(|c| c.borrow().as_ref().unwrap().empty);
                        SendMessageW(empty, WM_SETFONT, font as usize, 1);
                    }
                    fonts.push((key, font));
                }
            }
        }
        STATE.with(|c| {
            let mut s = c.borrow_mut();
            let s = s.as_mut().unwrap();
            for (key, font) in fonts {
                let slot = match key {
                    "ListFont" => &mut s.font,
                    "TitleFont" => &mut s.title_font,
                    _ => &mut s.input_font,
                };
                let old = std::mem::replace(slot, font);
                DeleteObject(old);
            }
        });
        if let Ok(font) = altrun_probe::preferences::Font::parse(&prefs.get("ListFont")) {
            let alias = make_font(font.size_dip(), 600, &font.name, dpi);
            let preview = make_font(12, 400, "Consolas", dpi);
            SendMessageW(command, WM_SETFONT, preview as usize, 1);
            STATE.with(|c| {
                let mut s = c.borrow_mut();
                let s = s.as_mut().unwrap();
                DeleteObject(std::mem::replace(&mut s.alias_font, alias));
                DeleteObject(std::mem::replace(&mut s.command_font, preview));
            });
        }
        let hint = search_hint(prefs.flag("ShowOperationHint"));
        SendMessageW(edit, EM_SETCUEBANNER, 0, hint.as_ptr() as isize);
        ShowWindow(
            command,
            if prefs.flag("ShowCommandLine") {
                SW_SHOWNOACTIVATE
            } else {
                SW_HIDE
            },
        );
        let has_hotkey = STATE.with(|c| {
            let state = c.borrow();
            state
                .as_ref()
                .unwrap()
                .hotkeys
                .iter()
                .any(|h| h.1.is_some())
        });
        tray_icon(
            hwnd,
            if prefs.flag("ShowTrayIcon") || !has_hotkey {
                NIM_ADD
            } else {
                NIM_DELETE
            },
        );
        SendMessageW(
            list,
            LB_SETITEMHEIGHT,
            0,
            scaled(prefs.number("RowHeight"), dpi) as isize,
        );
    }
    load_skin();
    unsafe {
        RedrawWindow(
            hwnd,
            std::ptr::null(),
            null_mut(),
            RDW_INVALIDATE | RDW_ALLCHILDREN,
        );
    }
}

fn load_skin() {
    let task = STATE.with(|c| {
        let mut state = c.borrow_mut();
        let s = state.as_mut().unwrap();
        let request =
            crate::skin::Request::for_settings(&s.preferences, &s.data_path, theme::surface());
        if request == s.skin_request {
            return None;
        }
        s.skin = None;
        s.skin_request = request.clone();
        request.map(|request| (request, s.skin_sender.clone(), s.hwnd as usize))
    });
    if let Some((request, sender, owner)) = task {
        std::thread::spawn(move || {
            let result = crate::skin::load(&request);
            if sender.send((request, result)).is_ok() {
                unsafe {
                    PostMessageW(owner as HWND, SKIN_DONE, 0, 0);
                }
            }
        });
    }
}

pub(super) unsafe extern "system" fn list_proc(
    hwnd: HWND,
    msg: u32,
    w: WPARAM,
    l: LPARAM,
) -> LRESULT {
    if msg == WM_MBUTTONUP {
        launch_selected();
        return 0;
    }
    if msg == WM_MOUSEWHEEL {
        move_selection(if ((w >> 16) as i16) > 0 { -1 } else { 1 });
        return 0;
    }
    let old = STATE.with(|c| c.borrow().as_ref().unwrap().old_list);
    unsafe {
        CallWindowProcW(
            Some(std::mem::transmute::<
                isize,
                unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT,
            >(old)),
            hwnd,
            msg,
            w,
            l,
        )
    }
}
