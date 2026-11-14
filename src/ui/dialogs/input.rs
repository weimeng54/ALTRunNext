// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use crate::{
    theme,
    ui::{self, make_font, scaled, ui, wide},
};
use std::ptr::null_mut;
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::LibraryLoader::GetModuleHandleW,
    UI::{Controls::*, Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
};
struct Input {
    dpi: i32,
    done: bool,
    value: Option<String>,
    edit: HWND,
    old_edit: isize,
    composing: bool,
    background: HBRUSH,
    paper: HBRUSH,
    report: Option<Report>,
}

pub struct Report {
    pub summary: String,
    pub rows: Vec<(String, String)>,
}

unsafe extern "system" fn edit_procedure(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    let state = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut Input;
    unsafe {
        match msg {
            WM_IME_STARTCOMPOSITION => (*state).composing = true,
            WM_IME_ENDCOMPOSITION => (*state).composing = false,
            _ => {}
        }
        CallWindowProcW(
            Some(std::mem::transmute::<
                isize,
                unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT,
            >((*state).old_edit)),
            hwnd,
            msg,
            w,
            l,
        )
    }
}

unsafe extern "system" fn procedure(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    if msg == WM_NCCREATE {
        let create = unsafe { &*(l as *const CREATESTRUCTW) };
        unsafe {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize);
        }
    }
    let state = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut Input;
    if !state.is_null() {
        match msg {
            WM_ERASEBKGND => unsafe {
                let mut rect = std::mem::zeroed();
                GetClientRect(hwnd, &mut rect);
                FillRect(w as HDC, &rect, (*state).background);
                return 1;
            },
            WM_CTLCOLORSTATIC | WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX => unsafe {
                let control = l as HWND;
                let read_only_edit = GetDlgCtrlID(control) == 10
                    && GetWindowLongW(control, GWL_STYLE) as u32 & ES_READONLY as u32 != 0;
                let (color, brush) = if msg != WM_CTLCOLORSTATIC || read_only_edit {
                    (theme::paper(), (*state).paper)
                } else {
                    (theme::surface(), (*state).background)
                };
                SetTextColor(w as HDC, theme::text());
                SetBkColor(w as HDC, color);
                return brush as isize;
            },
            WM_DRAWITEM if l != 0 => {
                let item = unsafe { &*(l as *const DRAWITEMSTRUCT) };
                let state = unsafe { &*state };
                if item.CtlType == ODT_LISTBOX {
                    draw_report_row(item, state);
                } else {
                    ui::button_on(item, item.CtlID == 1, theme::surface(), state.dpi);
                }
                return 1;
            }
            _ => {}
        }
    }
    if !state.is_null() && ((msg == WM_COMMAND && matches!(w & 65535, 1 | 2)) || msg == WM_CLOSE) {
        if msg == WM_COMMAND && w & 65535 == 1 {
            unsafe {
                let edit = (*state).edit;
                let mut b = vec![0u16; GetWindowTextLengthW(edit) as usize + 1];
                let n = GetWindowTextW(edit, b.as_mut_ptr(), b.len() as i32);
                (*state).value = Some(String::from_utf16_lossy(&b[..n.max(0) as usize]));
            }
        }
        unsafe {
            (*state).done = true;
            DestroyWindow(hwnd);
        }
        return 0;
    }
    unsafe { DefWindowProcW(hwnd, msg, w, l) }
}
pub fn ask(owner: HWND, title: &str) -> Option<String> {
    open(owner, title, None, None)
}

pub fn confirm(owner: HWND, title: &str, message: &str) -> bool {
    open(owner, title, Some(message), None).is_some()
}

pub fn report(owner: HWND, title: &str, report: Report) -> bool {
    open(owner, title, Some(""), Some(report)).is_some()
}

fn draw_report_row(item: &DRAWITEMSTRUCT, state: &Input) {
    let rect = item.rcItem;
    let selected = item.itemState & ODS_SELECTED != 0;
    ui::fill(
        item.hDC,
        &rect,
        if selected {
            theme::selection()
        } else {
            theme::paper()
        },
    );
    let Some((alias, name)) = state
        .report
        .as_ref()
        .and_then(|r| r.rows.get(item.itemID as usize))
    else {
        return;
    };
    let inset = scaled(10, state.dpi);
    let split = scaled(190, state.dpi);
    unsafe {
        let font = SendMessageW(item.hwndItem, WM_GETFONT, 0, 0) as HFONT;
        let previous = SelectObject(item.hDC, font);
        for (value, left, right) in [
            (alias, rect.left + inset, rect.left + split - inset),
            (name, rect.left + split + inset, rect.right - inset),
        ] {
            ui::line_text(
                item.hDC,
                RECT {
                    left,
                    right,
                    ..rect
                },
                value,
                theme::text(),
                0,
            );
        }
        SelectObject(item.hDC, previous);
        if item.itemState & ODS_FOCUS != 0 {
            DrawFocusRect(item.hDC, &rect);
        }
    }
}

fn open(owner: HWND, title: &str, message: Option<&str>, report: Option<Report>) -> Option<String> {
    unsafe {
        let instance = GetModuleHandleW(std::ptr::null());
        let class = wide(if message.is_some() {
            "ALTRun.Native.Confirmation"
        } else {
            "ALTRun.Native.ParameterDialog"
        });
        let mut wc: WNDCLASSW = std::mem::zeroed();
        wc.lpfnWndProc = Some(procedure);
        wc.hInstance = instance;
        wc.lpszClassName = class.as_ptr();
        let background = CreateSolidBrush(theme::surface());
        let paper = CreateSolidBrush(theme::paper());
        wc.hCursor = LoadCursorW(null_mut(), IDC_ARROW);
        RegisterClassW(&wc);
        let state = Box::into_raw(Box::new(Input {
            dpi: 96,
            done: false,
            value: None,
            edit: null_mut(),
            old_edit: 0,
            composing: false,
            background,
            paper,
            report,
        }));
        let hwnd = CreateWindowExW(
            WS_EX_DLGMODALFRAME | WS_EX_CONTROLPARENT,
            class.as_ptr(),
            ui(title).as_ptr(),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_CLIPCHILDREN,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            420,
            160,
            owner,
            null_mut(),
            instance,
            state as _,
        );
        if hwnd.is_null() {
            UnregisterClassW(class.as_ptr(), instance);
            DeleteObject(background);
            DeleteObject(paper);
            drop(Box::from_raw(state));
            return None;
        }
        theme::frame(hwnd);
        let long =
            message.is_some_and(|message| message.chars().count() > 84 || message.contains('\n'));
        let report = (*state).report.as_ref();
        let has_rows = report.is_some_and(|r| !r.rows.is_empty());
        let width = if report.is_some() { 600 } else { 420 };
        let (height, content_height, action_y) = if has_rows {
            (408, 78, 360)
        } else if report.is_some() {
            (168, 96, 120)
        } else if long {
            (278, 203, 230)
        } else if message.is_some() {
            (135, 55, 88)
        } else {
            (110, 28, 66)
        };
        let dpi = crate::ui::geometry::fit(hwnd, width, height);
        (*state).dpi = dpi;
        let font = make_font(15, 400, "Microsoft YaHei UI", dpi);
        let content_style = if long {
            WS_TABSTOP
                | WS_VSCROLL
                | WS_BORDER
                | WS_CLIPSIBLINGS
                | (ES_MULTILINE | ES_READONLY) as u32
        } else if message.is_none() {
            WS_BORDER | WS_TABSTOP | ES_AUTOHSCROLL as u32
        } else {
            0
        };
        let content_text = report
            .map_or_else(
                || crate::localization::tr(message.unwrap_or("")),
                |r| r.summary.clone(),
            )
            .replace("\r\n", "\n")
            .replace('\n', "\r\n");
        let content = CreateWindowExW(
            0,
            wide(if message.is_some() && !long {
                "STATIC"
            } else {
                "EDIT"
            })
            .as_ptr(),
            wide(&content_text).as_ptr(),
            WS_CHILD | WS_VISIBLE | content_style,
            scaled(16, dpi),
            scaled(16, dpi),
            scaled(width - 32, dpi),
            scaled(content_height, dpi),
            hwnd,
            10usize as HMENU,
            instance,
            std::ptr::null(),
        );
        if content.is_null() {
            DestroyWindow(hwnd);
            DeleteObject(font);
            UnregisterClassW(class.as_ptr(), instance);
            DeleteObject(background);
            DeleteObject(paper);
            drop(Box::from_raw(state));
            return None;
        }
        if message.is_none() {
            (*state).edit = content;
            SetWindowLongPtrW(content, GWLP_USERDATA, state as isize);
            (*state).old_edit =
                SetWindowLongPtrW(content, GWLP_WNDPROC, edit_procedure as *const () as isize);
        }
        SendMessageW(content, WM_SETFONT, font as usize, 0);
        if long {
            let inset = scaled(8, dpi) as isize;
            SendMessageW(
                content,
                EM_SETMARGINS,
                (EC_LEFTMARGIN | EC_RIGHTMARGIN) as usize,
                inset | (inset << 16),
            );
            ui::style_vertical_scrollbar(content, 12, dpi);
        }
        if let Some(report) = report.filter(|r| !r.rows.is_empty()) {
            for (label, x, w) in [("ui.alias", 26, 170), ("ui.name", 216, 350)] {
                let header = CreateWindowExW(
                    0,
                    wide("STATIC").as_ptr(),
                    ui(label).as_ptr(),
                    WS_CHILD | WS_VISIBLE,
                    scaled(x, dpi),
                    scaled(105, dpi),
                    scaled(w, dpi),
                    scaled(22, dpi),
                    hwnd,
                    null_mut(),
                    instance,
                    std::ptr::null(),
                );
                SendMessageW(header, WM_SETFONT, font as usize, 0);
            }
            let table = CreateWindowExW(
                0,
                wide("LISTBOX").as_ptr(),
                std::ptr::null(),
                WS_CHILD
                    | WS_VISIBLE
                    | WS_TABSTOP
                    | WS_VSCROLL
                    | WS_CLIPSIBLINGS
                    | (LBS_NOINTEGRALHEIGHT | LBS_OWNERDRAWFIXED | LBS_HASSTRINGS) as u32,
                scaled(16, dpi),
                scaled(132, dpi),
                scaled(width - 32, dpi),
                7 * scaled(29, dpi),
                hwnd,
                11usize as HMENU,
                instance,
                std::ptr::null(),
            );
            SendMessageW(table, WM_SETFONT, font as usize, 0);
            SendMessageW(table, LB_SETITEMHEIGHT, 0, scaled(29, dpi) as isize);
            for (alias, name) in &report.rows {
                SendMessageW(
                    table,
                    LB_ADDSTRING,
                    0,
                    wide(&format!("{alias} · {name}")).as_ptr() as LPARAM,
                );
            }
            ui::style_vertical_scrollbar(table, 12, dpi);
        }
        let mut focus = content;
        let actions: &[(&str, usize, i32, i32)] = if has_rows {
            &[
                ("ui.remove_missing", 1, 372, 120),
                ("ui.cancel", 2, 500, 84),
            ]
        } else if report.is_some() {
            &[("ui.close", 1, 500, 84)]
        } else if title == "ui.close_settings" {
            &[
                ("ui.discard_and_close", 1, 156, 120),
                ("ui.keep_editing", 2, 284, 120),
            ]
        } else {
            &[("ui.ok", 1, 224, 84), ("ui.cancel", 2, 320, 84)]
        };
        for &(text, id, x, width) in actions {
            let button = CreateWindowExW(
                0,
                wide("BUTTON").as_ptr(),
                ui(text).as_ptr(),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_OWNERDRAW as u32,
                scaled(x, dpi),
                scaled(action_y, dpi),
                scaled(width, dpi),
                scaled(28, dpi),
                hwnd,
                id as HMENU,
                instance,
                std::ptr::null(),
            );
            SendMessageW(button, WM_SETFONT, font as usize, 0);
            if message.is_some() && (id == 2 || actions.len() == 1) {
                focus = button;
            }
        }
        EnableWindow(owner, 0);
        ShowWindow(hwnd, SW_SHOW);
        SetFocus(focus);
        loop {
            if (*state).done {
                break;
            }
            let mut msg: MSG = std::mem::zeroed();
            let status = GetMessageW(&mut msg, null_mut(), 0, 0);
            if status <= 0 {
                if status == 0 {
                    PostQuitMessage(msg.wParam as i32);
                }
                break;
            }
            if msg.message == WM_KEYDOWN
                && (msg.hwnd == hwnd || IsChild(hwnd, msg.hwnd) != 0)
                && matches!(msg.wParam as u16, VK_RETURN | VK_ESCAPE)
            {
                if (*state).composing {
                    DispatchMessageW(&msg);
                    continue;
                }
                let focused = GetDlgCtrlID(GetFocus());
                if has_rows && msg.wParam as u16 == VK_RETURN && focused != 1 && focused != 2 {
                    continue;
                }
                SendMessageW(
                    hwnd,
                    WM_COMMAND,
                    if msg.wParam as u16 == VK_RETURN && focused != 2 {
                        1
                    } else {
                        2
                    },
                    0,
                );
                continue;
            }
            if IsDialogMessageW(hwnd, &msg) == 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        if IsWindow(hwnd) != 0 {
            DestroyWindow(hwnd);
        }
        EnableWindow(owner, 1);
        SetForegroundWindow(owner);
        DeleteObject(font);
        UnregisterClassW(class.as_ptr(), instance);
        DeleteObject(background);
        DeleteObject(paper);
        Box::from_raw(state).value.take()
    }
}
