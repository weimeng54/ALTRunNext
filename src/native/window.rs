// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::*;

pub(super) unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: u32,
    w: WPARAM,
    l: LPARAM,
) -> LRESULT {
    let _panel = if matches!(
        msg,
        WM_PAINT | WM_CTLCOLORSTATIC | WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX
    ) || (msg == WM_DRAWITEM
        && l != 0
        && unsafe { (*(l as *const DRAWITEMSTRUCT)).CtlType } != ODT_MENU)
    {
        super::panel::panel_colours()
    } else {
        None
    };
    if RESTORING_BACKUP.get() {
        match msg {
            WM_DROPFILES => {
                unsafe {
                    DragFinish(w as _);
                }
                return 0;
            }
            WM_COPYDATA
            | WM_HOTKEY
            | SHOW
            | WM_COMMAND
            | WM_CONTEXTMENU
            | TRAY
            | QUIT
            | crate::manager::GEOMETRY_CHANGED
            | WM_EXITSIZEMOVE => return 0,
            _ => {}
        }
    }
    if CONFIRMING_DELETE.get()
        && matches!(msg, WM_HOTKEY | SHOW | WM_COMMAND | WM_CONTEXTMENU | TRAY)
    {
        return 0;
    }
    if execution::parameter_open() {
        match msg {
            WM_HOTKEY | SHOW => {
                execution::focus_parameter();
                return 0;
            }
            WM_COMMAND | WM_CONTEXTMENU | TRAY => return 0,
            _ => {}
        }
    }
    match msg {
        WM_COPYDATA => match crate::instance::decode(l) {
            Ok(paths) => {
                if !paths.is_empty() {
                    crate::manager::enqueue(paths, None);
                    if !execution::defer_intake() {
                        open_manager_window(false);
                    }
                } else {
                    unsafe {
                        PostMessageW(hwnd, SHOW, 0, 0);
                    }
                }
                1
            }
            Err(_) => 0,
        },
        WM_DROPFILES => {
            let paths = crate::windows::file_picker::drop_paths(w as _);
            crate::manager::enqueue(paths, None);
            if !execution::defer_intake() {
                open_manager_window(false);
            }
            0
        }
        WM_HOTKEY | SHOW => {
            // The previous keys stay reserved for rollback while disk I/O is in flight.
            if msg == WM_HOTKEY
                && STATE.with(|c| {
                    let state = c.borrow();
                    let s = state.as_ref().unwrap();
                    s.pending_settings.is_some()
                        && !s
                            .hotkeys
                            .iter()
                            .any(|key| key.0 == w as i32 && key.1.is_some())
                })
            {
                return 0;
            }
            let is_recent = msg == WM_HOTKEY
                && STATE.with(|c| {
                    c.borrow()
                        .as_ref()
                        .unwrap()
                        .hotkeys
                        .get(2)
                        .is_some_and(|h| h.0 == w as i32)
                });
            if msg == WM_HOTKEY && !is_recent && unsafe { IsWindowVisible(hwnd) != 0 } {
                unsafe {
                    ShowWindow(hwnd, SW_HIDE);
                }
                return 0;
            }
            capture_context();
            if is_recent {
                recent();
            } else {
                reset_search();
            }
            position_panel();
            let edit = STATE.with(|c| c.borrow().as_ref().unwrap().edit);
            unsafe {
                super::panel::show_focused(hwnd, edit);
                popup_sound();
                let delay =
                    STATE.with(|c| c.borrow().as_ref().unwrap().preferences.number("HideDelay"));
                if delay > 0 {
                    SetTimer(hwnd, AUTO_HIDE, delay as u32 * 1000, None);
                }
            }
            update_search_ime(true);
            0
        }
        SEARCH_IME_UPDATE => {
            update_search_ime(true);
            0
        }
        WM_SHOWWINDOW if w == 0 => {
            update_search_ime(false);
            unsafe { DefWindowProcW(hwnd, msg, w, l) }
        }
        WM_CONTEXTMENU => {
            let list = STATE.with(|c| c.borrow().as_ref().unwrap().list);
            let allow_selection = if w as HWND == list && l != -1 {
                let mut point = POINT {
                    x: l as i16 as i32,
                    y: (l >> 16) as i16 as i32,
                };
                unsafe {
                    ScreenToClient(list, &mut point);
                }
                let hit = unsafe {
                    SendMessageW(
                        list,
                        LB_ITEMFROMPOINT,
                        0,
                        ((point.y as u16 as u32) << 16 | point.x as u16 as u32) as LPARAM,
                    )
                };
                let row = (hit & 65535) as usize;
                let selected = STATE.with(|c| {
                    let mut state = c.borrow_mut();
                    let s = state.as_mut().unwrap();
                    if hit >> 16 != 0 || row >= s.hits.len() {
                        return false;
                    }
                    s.selected = row;
                    true
                });
                if selected {
                    unsafe {
                        SendMessageW(list, LB_SETCURSEL, row, 0);
                    }
                    update_selection();
                }
                selected
            } else {
                true
            };
            let anchor = if l == -1 {
                let mut bounds = RECT::default();
                unsafe {
                    GetWindowRect(if w == 0 { hwnd } else { w as HWND }, &mut bounds);
                }
                crate::ui::menu_style::Anchor::Below(bounds)
            } else {
                crate::ui::menu_style::Anchor::Pointer(POINT {
                    x: l as i16 as i32,
                    y: (l >> 16) as i16 as i32,
                })
            };
            show_menu(hwnd, false, allow_selection, anchor);
            0
        }
        TRAY => {
            if l as u32 == WM_RBUTTONUP {
                let mut point = POINT::default();
                unsafe {
                    GetCursorPos(&mut point);
                }
                show_menu(
                    hwnd,
                    true,
                    true,
                    crate::ui::menu_style::Anchor::Pointer(point),
                );
            } else if l as u32 == WM_LBUTTONUP {
                unsafe {
                    SendMessageW(hwnd, SHOW, 0, 0);
                }
            }
            0
        }
        LAUNCH_DONE => {
            execution::finish_launch();
            0
        }
        PARAMETER_OPEN => {
            execution::open_parameter(hwnd);
            0
        }
        crate::settings::UPDATED => {
            apply_settings_messages();
            0
        }
        STATE_ERROR => {
            let errors = STATE.with(|c| {
                c.borrow()
                    .as_ref()
                    .unwrap()
                    .usage_writer
                    .errors
                    .try_iter()
                    .collect::<Vec<_>>()
            });
            if let Some(error) = errors.last() {
                show_inline_error(&crate::localization::message(
                    "ui.could_not_save_launch_history_error",
                    &[("error", &crate::localization::tr(error))],
                ));
            }
            0
        }
        SKIN_DONE => {
            let mut errors = Vec::new();
            STATE.with(|c| {
                let mut s = c.borrow_mut();
                let s = s.as_mut().unwrap();
                while let Ok((request, result)) = s.skin_results.try_recv() {
                    if s.skin_request.as_ref() == Some(&request) {
                        match result {
                            Ok(bitmap) => s.skin = Some(bitmap),
                            Err(error) => errors.push(error),
                        }
                    }
                }
            });
            unsafe {
                RedrawWindow(
                    hwnd,
                    std::ptr::null(),
                    null_mut(),
                    RDW_INVALIDATE | RDW_ALLCHILDREN,
                );
            }
            if let Some(e) = errors.pop() {
                show_inline_error(&e);
            }
            0
        }
        crate::ui::language::CHANGED => {
            if let Some((edit, empty, hints)) = STATE.with(|c| {
                c.borrow()
                    .as_ref()
                    .map(|s| (s.edit, s.empty, s.preferences.flag("ShowOperationHint")))
            }) {
                unsafe {
                    SetWindowTextW(empty, ui("ui.press_enter_to_add_launch_item").as_ptr());
                    SendMessageW(
                        edit,
                        EM_SETCUEBANNER,
                        0,
                        search_hint(hints).as_ptr() as LPARAM,
                    );
                }
                update_caption_button_labels(hwnd);
                super::search::refresh_selection_text();
            }
            0
        }
        theme::CHANGED => {
            super::panel::refresh_colours();
            0
        }
        WM_DPICHANGED => {
            STATE.with(|c| c.borrow_mut().as_mut().unwrap().dpi = (w & 65535) as i32);
            if l != 0 {
                let suggested = unsafe { &*(l as *const RECT) };
                unsafe {
                    SetWindowPos(
                        hwnd,
                        null_mut(),
                        suggested.left,
                        suggested.top,
                        0,
                        0,
                        SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
                    );
                }
            }
            apply_appearance();
            render_hits();
            0
        }
        WM_NCHITTEST => {
            let result = unsafe { DefWindowProcW(hwnd, msg, w, l) };
            if result != HTCLIENT as isize {
                return result;
            }
            let dpi = STATE.with(|c| c.borrow().as_ref().map_or(96, |s| s.dpi));
            let mut point = POINT {
                x: l as i16 as i32,
                y: (l >> 16) as i16 as i32,
            };
            if unsafe { ScreenToClient(hwnd, &mut point) } != 0
                && (0..scaled(TITLE_STRIP, dpi)).contains(&point.y)
            {
                HTCAPTION as isize
            } else {
                result
            }
        }
        WM_ENTERSIZEMOVE => {
            STATE.with(|c| c.borrow_mut().as_mut().unwrap().moving = true);
            unsafe {
                KillTimer(hwnd, AUTO_HIDE);
            }
            0
        }
        WM_EXITSIZEMOVE => {
            let mut rect: RECT = unsafe { zeroed() };
            unsafe {
                GetWindowRect(hwnd, &mut rect);
            }
            let delay = STATE.with(|c| {
                let mut s = c.borrow_mut();
                let s = s.as_mut().unwrap();
                s.moving = false;
                if s.preferences.get("PositionMode") == "remember" {
                    let change = UsageChange::LauncherPosition((
                        rect.left,
                        rect.top,
                        rect.right - rect.left,
                        rect.bottom - rect.top,
                    ));
                    change.clone().apply(&mut s.usage);
                    s.usage_writer.publish(change);
                }
                s.preferences.number("HideDelay")
            });
            if delay > 0 && unsafe { IsWindowVisible(hwnd) != 0 } {
                unsafe {
                    SetTimer(hwnd, AUTO_HIDE, delay as u32 * 1000, None);
                }
            }
            0
        }
        WM_TIMER if w == AUTO_HIDE => {
            unsafe {
                KillTimer(hwnd, AUTO_HIDE);
                if !CONFIRMING_DELETE.get()
                    && !execution::parameter_open()
                    && !STATE.with(|c| c.borrow().as_ref().unwrap().moving)
                {
                    ShowWindow(hwnd, SW_HIDE);
                }
            }
            0
        }
        WM_ACTIVATE if w & 65535 == WA_INACTIVE as usize => {
            update_search_ime(false);
            if !CONFIRMING_DELETE.get()
                && !execution::parameter_open()
                && unsafe { IsWindowEnabled(hwnd) != 0 }
                && STATE.with(|c| c.borrow().as_ref().is_some_and(|s| !s.moving))
            {
                unsafe {
                    ShowWindow(hwnd, SW_HIDE);
                }
            }
            0
        }
        WM_MOUSEWHEEL => {
            move_selection(if ((w >> 16) as i16) > 0 { -1 } else { 1 });
            0
        }
        WM_MBUTTONUP => {
            launch_selected();
            0
        }
        WM_ERASEBKGND => {
            let brush = STATE
                .with(|c| c.borrow().as_ref().map(|s| s.background))
                .unwrap_or_else(|| unsafe { GetSysColorBrush(COLOR_WINDOW) });
            let mut rect: RECT = unsafe { zeroed() };
            unsafe {
                GetClientRect(hwnd, &mut rect);
                FillRect(w as HDC, &rect, brush);
            }
            1
        }
        WM_PAINT => {
            paint_panel(hwnd);
            0
        }
        crate::manager::UPDATED => {
            if execution::parameter_open() {
                return 0;
            }
            let entries = STATE.with(|c| c.borrow_mut().as_mut().unwrap().library_updates.take());
            if let Some(entries) = entries {
                STATE.with(|c| {
                    let mut state = c.borrow_mut();
                    let s = state.as_mut().unwrap();
                    s.entries = entries;
                    rebuild_index(s);
                    s.selected = 0;
                });
                refresh();
            }
            0
        }
        crate::manager::GEOMETRY_CHANGED => {
            let geometry = crate::manager::window_state();
            STATE.with(|c| {
                let mut s = c.borrow_mut();
                let s = s.as_mut().unwrap();
                let change = UsageChange::ManagerSelection(geometry.selected);
                change.clone().apply(&mut s.usage);
                s.usage_writer.publish(change);
            });
            0
        }
        WM_COMMAND => handle_command(hwnd, w),
        WM_DRAWITEM => {
            let d = unsafe { &*(l as *const DRAWITEMSTRUCT) };
            if d.CtlID as usize == TITLE {
                paint_title(hwnd, d);
                return 1;
            }
            if d.CtlType == ODT_BUTTON {
                let dpi = STATE.with(|c| c.borrow().as_ref().map_or(96, |s| s.dpi));
                if let Some((_, icon, _)) = CAPTION_BUTTONS
                    .iter()
                    .find(|(id, _, _)| *id == d.CtlID as usize)
                {
                    crate::ui::caption_button(d, theme::surface(), dpi, *icon);
                } else {
                    crate::ui::button_on(d, false, theme::surface(), dpi);
                }
                return 1;
            }
            if d.itemID == u32::MAX {
                return 1;
            }
            STATE.with(|c| {
                let s = c.borrow();
                let Some(s) = s.as_ref() else {
                    return;
                };
                let Some(&i) = s.hits.get(d.itemID as usize) else {
                    return;
                };
                let e = &s.entries[i];
                unsafe {
                    let selected = d.itemState & ODS_SELECTED != 0;
                    FillRect(
                        d.hDC,
                        &d.rcItem,
                        if selected { s.selected_brush } else { s.paper },
                    );
                    s.result_dividers.draw(
                        d.hDC,
                        d.rcItem,
                        [scaled(30, s.dpi), scaled(146, s.dpi)],
                        s.dpi,
                        selected,
                        (d.itemID as usize + 1) < s.hits.len(),
                    );
                    if selected {
                        let rail = RECT {
                            left: 0,
                            top: d.rcItem.top + scaled(3, s.dpi),
                            right: scaled(2, s.dpi),
                            bottom: d.rcItem.bottom - scaled(3, s.dpi),
                        };
                        FillRect(d.hDC, &rail, s.accent);
                    }
                    SetBkMode(d.hDC, TRANSPARENT as i32);
                    SetTextColor(d.hDC, theme::row_ink(selected, theme::muted()));
                    SelectObject(d.hDC, s.font);
                    let number = format!(
                        "{}{}",
                        if e.param_type.needs_input(&e.command) {
                            '*'
                        } else {
                            ' '
                        },
                        if d.itemID >= 10 {
                            ' '
                        } else {
                            char::from(b'0' + ((d.itemID + 1) % 10) as u8)
                        }
                    );
                    let a = wide(&number);
                    let b = wide(&e.alias);
                    let name = wide(&e.name);
                    TextOutW(
                        d.hDC,
                        scaled(8, s.dpi),
                        d.rcItem.top + scaled(3, s.dpi),
                        a.as_ptr(),
                        (a.len() - 1) as i32,
                    );
                    SetTextColor(d.hDC, theme::row_ink(selected, s.font_colours[2]));
                    SelectObject(d.hDC, s.alias_font);
                    let mut rect = RECT {
                        left: scaled(34, s.dpi),
                        top: d.rcItem.top,
                        right: scaled(143, s.dpi),
                        bottom: d.rcItem.bottom,
                    };
                    DrawTextW(
                        d.hDC,
                        b.as_ptr(),
                        -1,
                        &mut rect,
                        DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS | DT_NOPREFIX,
                    );
                    SelectObject(d.hDC, s.font);
                    rect.left = scaled(150, s.dpi);
                    rect.right = d.rcItem.right - scaled(3, s.dpi);
                    DrawTextW(
                        d.hDC,
                        name.as_ptr(),
                        -1,
                        &mut rect,
                        DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS | DT_NOPREFIX,
                    );
                    crate::ui::row_focus(d, &d.rcItem);
                }
            });
            1
        }
        WM_CTLCOLORSTATIC | WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX => STATE.with(|c| {
            let state = c.borrow();
            let Some(s) = state.as_ref() else {
                return 0;
            };
            let dc = w as HDC;
            let paper = l as HWND == s.edit
                || l as HWND == s.list
                || l as HWND == s.command
                || l as HWND == s.inline_error
                || l as HWND == s.empty;
            unsafe {
                SetTextColor(
                    dc,
                    if l as HWND == s.inline_error {
                        theme::error()
                    } else if l as HWND == s.command || l as HWND == s.empty {
                        theme::muted()
                    } else if l as HWND == s.edit {
                        s.font_colours[1]
                    } else if l as HWND == s.title {
                        s.font_colours[0]
                    } else {
                        s.font_colours[2]
                    },
                );
                SetBkColor(
                    dc,
                    if l as HWND == s.edit {
                        theme::keyword()
                    } else if paper {
                        theme::paper()
                    } else {
                        theme::surface()
                    },
                );
            }
            if l as HWND == s.edit {
                s.input_paper as LRESULT
            } else if paper {
                s.paper as LRESULT
            } else {
                s.background as LRESULT
            }
        }),
        WM_CLOSE => {
            unsafe {
                ShowWindow(hwnd, SW_HIDE);
            }
            0
        }
        QUIT => {
            if crate::settings::finish_before_exit() {
                return 0;
            }
            unsafe {
                DestroyWindow(hwnd);
            }
            0
        }
        WM_DESTROY => {
            update_search_ime(false);
            tray_icon(hwnd, NIM_DELETE);
            unsafe {
                STATE.with(|c| {
                    if let Some(s) = c.borrow().as_ref() {
                        for (id, _) in &s.hotkeys {
                            UnregisterHotKey(hwnd, *id);
                        }
                    }
                });
                PostQuitMessage(0);
            }
            0
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, w, l) },
    }
}

fn handle_command(hwnd: HWND, w: WPARAM) -> LRESULT {
    if w & 65535 == ID_RUN_AS_ADMIN {
        execution::launch_selected_with(true);
        return 0;
    }
    if w & 65535 == ID_EDIT_COMMAND {
        edit_selected_command();
        return 0;
    }
    if w & 65535 == ID_DELETE_COMMAND {
        delete_selected_command(hwnd);
        return 0;
    }
    if w & 65535 == ID_COPY {
        selected_action(true);
        return 0;
    }
    if w & 65535 == ID_OPEN_FOLDER {
        selected_action(false);
        return 0;
    }
    if w & 0xffff == ID_MANAGE {
        open_manager();
        return 0;
    }
    if w & 0xffff == ID_SETTINGS {
        open_settings();
        return 0;
    }
    if w & 0xffff == ID_RECENT {
        recent();
        return 0;
    }
    if w & 0xffff == ID_ABOUT {
        about();
        return 0;
    }
    if w & 0xffff == ID_RESTART || w & 0xffff == ID_CLEAN {
        lifecycle(w & 0xffff == ID_CLEAN);
        return 0;
    }
    if w & 0xffff == ID_SHOW {
        unsafe {
            SendMessageW(hwnd, SHOW, 0, 0);
        }
        return 0;
    }
    if w & 0xffff == ID_EXIT {
        unsafe {
            PostMessageW(hwnd, QUIT, 0, 0);
        }
        return 0;
    }
    if w & 0xffff == ID_HIDE {
        unsafe {
            ShowWindow(hwnd, SW_HIDE);
        }
        return 0;
    }
    let id = w & 0xffff;
    let code = (w >> 16) & 0xffff;
    if id == ID_EDIT && code == EN_CHANGE as usize {
        let (previous, previous_selected, key, edit, number, composing) = STATE.with(|c| {
            let mut s = c.borrow_mut();
            let s = s.as_mut().unwrap();
            let key = std::mem::take(&mut s.last_key);
            let number = s.preferences.flag("NumberKey");
            (
                if !s.composing
                    && (key == VK_SPACE || (number && matches!(key, 0x30..=0x39 | 0x60..=0x69)))
                {
                    s.hits.clone()
                } else {
                    Vec::new()
                },
                s.selected,
                key,
                s.edit,
                number,
                s.composing,
            )
        });
        let query = edit_text(edit);
        refresh();
        let none = STATE.with(|c| {
            let s = c.borrow();
            let s = s.as_ref().unwrap();
            s.hits.is_empty()
        });
        if none && !composing && !previous.is_empty() {
            let digit = match key {
                0x30..=0x39 => Some((key - 0x30) as usize),
                0x60..=0x69 => Some((key - 0x60) as usize),
                _ => None,
            };
            if (number && digit.is_some()) || key == VK_SPACE {
                let trimmed = query
                    .char_indices()
                    .next_back()
                    .map(|(i, _)| &query[..i])
                    .unwrap_or("");
                unsafe {
                    SetWindowTextW(edit, wide(trimmed).as_ptr());
                }
                STATE.with(|c| {
                    let mut s = c.borrow_mut();
                    let s = s.as_mut().unwrap();
                    s.hits = previous;
                    s.selected = previous_selected.min(s.hits.len().saturating_sub(1));
                });
                if let Some(digit) = digit {
                    choose_number(digit);
                } else {
                    launch_selected();
                }
            }
        }
    }
    if id == ID_LIST && code == LBN_SELCHANGE as usize {
        let list = STATE.with(|c| c.borrow().as_ref().unwrap().list);
        let selected = unsafe { SendMessageW(list, LB_GETCURSEL, 0, 0) };
        if selected >= 0 {
            STATE.with(|c| c.borrow_mut().as_mut().unwrap().selected = selected as usize);
            update_selection();
            let edit = STATE.with(|c| c.borrow().as_ref().unwrap().edit);
            unsafe {
                SetFocus(edit);
            }
        }
    }
    if id == ID_LIST && code == LBN_DBLCLK as usize {
        launch_selected();
    }
    0
}

fn position_panel() {
    use crate::ui::geometry::{cursor_monitor, work_area};

    let (hwnd, remembered) = STATE.with(|c| {
        let s = c.borrow();
        let s = s.as_ref().unwrap();
        let remembered = if s.preferences.get("PositionMode") == "remember" {
            s.usage
                .positions
                .get("launcher")
                .map(|&(x, y, _, _)| POINT { x, y })
        } else {
            None
        };
        (s.hwnd, remembered)
    });
    unsafe {
        let saved = remembered.and_then(|point| {
            let monitor = MonitorFromPoint(point, MONITOR_DEFAULTTONULL);
            let work = work_area(monitor)?;
            (PtInRect(&work, point) != 0).then_some((monitor, work, point))
        });
        let (monitor, work, anchor) = match saved {
            Some((monitor, work, point)) => (monitor, work, Some(point)),
            None => {
                let monitor = cursor_monitor(hwnd);
                let Some(work) = work_area(monitor) else {
                    return;
                };
                (monitor, work, None)
            }
        };
        let flags = SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE;
        if monitor != MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) {
            // Move the hidden panel first so layout uses the destination monitor.
            SetWindowPos(hwnd, null_mut(), work.left, work.top, 0, 0, flags);
            render_hits();
        }
        let mut rect = RECT::default();
        GetWindowRect(hwnd, &mut rect);
        let right = (work.right - (rect.right - rect.left)).max(work.left);
        let bottom = (work.bottom - (rect.bottom - rect.top)).max(work.top);
        let (x, y) = anchor.map_or(
            (
                work.left + (right - work.left) / 2,
                work.top + (bottom - work.top) / 2,
            ),
            |p| (p.x.clamp(work.left, right), p.y.clamp(work.top, bottom)),
        );
        if x != rect.left || y != rect.top {
            SetWindowPos(hwnd, null_mut(), x, y, 0, 0, flags);
        }
    }
}

pub(super) fn open_manager() {
    open_manager_window(true);
}

pub(super) fn open_manager_window(show: bool) {
    let (context, english) = STATE.with(|c| {
        let s = c.borrow();
        let s = s.as_ref().unwrap();
        (s.context.clone(), s.preferences.flag("EnglishSearch"))
    });
    crate::manager::configure_context(context, english);
    let geometry = STATE.with(|c| {
        let s = c.borrow();
        let s = s.as_ref().unwrap();
        crate::manager::WindowState {
            selected: s
                .usage
                .selection
                .get("manager")
                .cloned()
                .unwrap_or_default(),
        }
    });
    crate::manager::configure(geometry);
    let (path, entries, hwnd, sender, enabled) = STATE.with(|c| {
        let state = c.borrow();
        let s = state.as_ref().unwrap();
        (
            s.data_path.clone(),
            s.entries.clone(),
            s.hwnd,
            s.library_sender.clone(),
            s.enabled,
        )
    });
    crate::manager::open(path, entries, hwnd, sender, enabled, show);
    unsafe {
        ShowWindow(hwnd, SW_HIDE);
    }
}
