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
    caption: HFONT,
    title: String,
    done: bool,
    value: Option<String>,
    alternate: bool,
    edit: HWND,
    old_edit: isize,
    composing: bool,
    background: HBRUSH,
    paper: HBRUSH,
    report: Option<Report>,
    preview: bool,
}

pub struct Report {
    pub summary: String,
    pub rows: Vec<(String, String)>,
}

pub struct PreviewLabels<'a> {
    pub columns: [&'a str; 2],
    pub note: &'a str,
}

#[derive(Clone, Copy)]
pub enum Choice {
    Primary,
    Alternate,
    Cancel,
}

#[derive(Default)]
struct Options<'a> {
    action: Option<&'a str>,
    alternate: Option<&'a str>,
    preview: Option<PreviewLabels<'a>>,
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
            WM_NCHITTEST if unsafe { (*state).preview } => unsafe {
                let mut point = POINT {
                    x: l as i16 as i32,
                    y: (l >> 16) as i16 as i32,
                };
                ScreenToClient(hwnd, &mut point);
                let mut client = RECT::default();
                GetClientRect(hwnd, &mut client);
                let dpi = (*state).dpi;
                return if (0..scaled(ui::geometry::CAPTION, dpi)).contains(&point.y)
                    && point.x < client.right - scaled(47, dpi)
                {
                    HTCAPTION as isize
                } else {
                    HTCLIENT as isize
                };
            },
            WM_PAINT if unsafe { (*state).preview } => unsafe {
                let state = &*state;
                let mut paint = PAINTSTRUCT::default();
                let dc = BeginPaint(hwnd, &mut paint);
                let mut client = RECT::default();
                GetClientRect(hwnd, &mut client);
                let caption = scaled(ui::geometry::CAPTION, state.dpi);
                ui::fill(dc, &client, theme::surface());
                ui::fill(
                    dc,
                    &RECT {
                        bottom: caption,
                        ..client
                    },
                    theme::chrome(),
                );
                ui::fill(
                    dc,
                    &RECT {
                        top: caption - 1,
                        bottom: caption,
                        ..client
                    },
                    theme::divider(),
                );
                ui::panel_caption(dc, state.dpi, state.caption, &state.title);
                EndPaint(hwnd, &paint);
                return 0;
            },
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
                } else if item.CtlID == 4 && state.preview {
                    ui::caption_close(item, state.dpi);
                } else {
                    ui::button_on(item, item.CtlID == 1, theme::surface(), state.dpi);
                }
                return 1;
            }
            _ => {}
        }
    }
    if !state.is_null()
        && ((msg == WM_COMMAND && (1..=4).contains(&(w & 65535))) || msg == WM_CLOSE)
    {
        if msg == WM_COMMAND && w & 65535 == 1 {
            unsafe {
                let edit = (*state).edit;
                let mut b = vec![0u16; GetWindowTextLengthW(edit) as usize + 1];
                let n = GetWindowTextW(edit, b.as_mut_ptr(), b.len() as i32);
                (*state).value = Some(String::from_utf16_lossy(&b[..n.max(0) as usize]));
            }
        }
        unsafe {
            (*state).alternate = msg == WM_COMMAND && w & 65535 == 3;
            (*state).done = true;
            DestroyWindow(hwnd);
        }
        return 0;
    }
    unsafe { DefWindowProcW(hwnd, msg, w, l) }
}
pub fn ask(owner: HWND, title: &str) -> Option<String> {
    open(owner, title, None, None, Options::default()).0
}

pub fn confirm(owner: HWND, title: &str, message: &str) -> bool {
    open(owner, title, Some(message), None, Options::default())
        .0
        .is_some()
}

pub fn report(owner: HWND, title: &str, report: Report) -> bool {
    open(owner, title, Some(""), Some(report), Options::default())
        .0
        .is_some()
}

pub fn confirm_details(owner: HWND, title: &str, message: &str, action: &str) -> bool {
    open(
        owner,
        title,
        Some(message),
        None,
        Options {
            action: Some(action),
            ..Options::default()
        },
    )
    .0
    .is_some()
}

pub fn confirm_preview(
    owner: HWND,
    title: &str,
    report: Report,
    labels: PreviewLabels<'_>,
    action: &str,
) -> bool {
    open(
        owner,
        title,
        Some(""),
        Some(report),
        Options {
            action: Some(action),
            preview: Some(labels),
            ..Options::default()
        },
    )
    .0
    .is_some()
}

pub fn choose(owner: HWND, title: &str, message: &str, action: &str, alternate: &str) -> Choice {
    match open(
        owner,
        title,
        Some(message),
        None,
        Options {
            action: Some(action),
            alternate: Some(alternate),
            preview: None,
        },
    ) {
        (Some(_), _) => Choice::Primary,
        (_, true) => Choice::Alternate,
        _ => Choice::Cancel,
    }
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
        if state.preview {
            let line = RECT {
                top: rect.bottom - 1,
                ..rect
            };
            ui::fill(item.hDC, &line, theme::divider());
        }
        if item.itemState & ODS_FOCUS != 0 {
            DrawFocusRect(item.hDC, &rect);
        }
    }
}

fn open(
    owner: HWND,
    title: &str,
    message: Option<&str>,
    report: Option<Report>,
    options: Options<'_>,
) -> (Option<String>, bool) {
    unsafe {
        let Options {
            action,
            alternate,
            preview,
        } = options;
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
            caption: null_mut(),
            title: title.to_owned(),
            done: false,
            value: None,
            alternate: false,
            edit: null_mut(),
            old_edit: 0,
            composing: false,
            background,
            paper,
            report,
            preview: preview.is_some(),
        }));
        let hwnd = CreateWindowExW(
            WS_EX_DLGMODALFRAME | WS_EX_CONTROLPARENT,
            class.as_ptr(),
            ui(title).as_ptr(),
            (if preview.is_some() {
                WS_POPUP
            } else {
                WS_OVERLAPPED | WS_CAPTION
            }) | WS_SYSMENU
                | WS_CLIPCHILDREN,
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
            return (None, false);
        }
        theme::frame(hwnd);
        let long =
            message.is_some_and(|message| message.chars().count() > 84 || message.contains('\n'));
        let report = (*state).report.as_ref();
        let has_rows = report.is_some_and(|r| !r.rows.is_empty());
        let width = if report.is_some() || action.is_some() {
            600
        } else {
            420
        };
        let summary_height = if report.is_some_and(|report| report.summary.contains('\n')) {
            62
        } else {
            34
        };
        let table_top = 16 + summary_height + 30;
        let row_height = 32;
        let table_height = report.map_or(0, |r| r.rows.len() as i32 * row_height);
        let note_top = table_top + table_height + 14;
        let note_height = if preview.as_ref().is_some_and(|p| !p.note.is_empty()) {
            38
        } else {
            0
        };
        let start_y = if preview.is_some() {
            ui::geometry::CAPTION
        } else {
            0
        };
        let (height, content_height, action_y) = if preview.is_some() {
            let action_y = note_top + note_height;
            (action_y + 48, summary_height, action_y)
        } else if has_rows {
            (408, 78, 360)
        } else if report.is_some() {
            (168, 96, 120)
        } else if action.is_some() && long {
            (298, 218, 249)
        } else if long {
            (278, 203, 230)
        } else if message.is_some() {
            (135, 55, 88)
        } else {
            (110, 28, 66)
        };
        let dpi = crate::ui::geometry::fit(hwnd, width, height + start_y);
        (*state).dpi = dpi;
        let font = make_font(15, 400, "Microsoft YaHei UI", dpi);
        let y = |coordinate| scaled(start_y + coordinate, dpi);
        if preview.is_some() {
            (*state).caption = make_font(14, 600, "Microsoft YaHei UI", dpi);
            ui::round_panel(hwnd, scaled(width, dpi), scaled(height + start_y, dpi), dpi);
            let close = CreateWindowExW(
                0,
                wide("BUTTON").as_ptr(),
                ui("ui.close").as_ptr(),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_OWNERDRAW as u32,
                scaled(width - 46, dpi),
                0,
                scaled(46, dpi),
                scaled(start_y, dpi),
                hwnd,
                4usize as HMENU,
                instance,
                null_mut(),
            );
            ui::style_caption_close(close);
        }
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
            y(16),
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
            return (None, false);
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
            let [left, right] = preview
                .as_ref()
                .map_or(["ui.alias", "ui.name"], |p| p.columns);
            for (label, x, w) in [(left, 26, 170), (right, 216, 350)] {
                let header = CreateWindowExW(
                    0,
                    wide("STATIC").as_ptr(),
                    ui(label).as_ptr(),
                    WS_CHILD | WS_VISIBLE,
                    scaled(x, dpi),
                    y(if preview.is_some() {
                        table_top - 26
                    } else {
                        105
                    }),
                    scaled(w, dpi),
                    scaled(22, dpi),
                    hwnd,
                    null_mut(),
                    instance,
                    std::ptr::null(),
                );
                SendMessageW(header, WM_SETFONT, font as usize, 0);
            }
            let table_style = if preview.is_some() {
                LBS_NOSEL as u32
            } else {
                WS_TABSTOP | WS_VSCROLL
            };
            let table = CreateWindowExW(
                0,
                wide("LISTBOX").as_ptr(),
                std::ptr::null(),
                WS_CHILD
                    | WS_VISIBLE
                    | table_style
                    | WS_CLIPSIBLINGS
                    | (LBS_NOINTEGRALHEIGHT | LBS_OWNERDRAWFIXED | LBS_HASSTRINGS) as u32,
                scaled(16, dpi),
                y(if preview.is_some() { table_top } else { 132 }),
                scaled(width - 32, dpi),
                if preview.is_some() {
                    report.rows.len() as i32 * scaled(row_height, dpi)
                } else {
                    7 * scaled(29, dpi)
                },
                hwnd,
                11usize as HMENU,
                instance,
                std::ptr::null(),
            );
            SendMessageW(table, WM_SETFONT, font as usize, 0);
            SendMessageW(
                table,
                LB_SETITEMHEIGHT,
                0,
                scaled(if preview.is_some() { row_height } else { 29 }, dpi) as isize,
            );
            for (alias, name) in &report.rows {
                SendMessageW(
                    table,
                    LB_ADDSTRING,
                    0,
                    wide(&format!("{alias} · {name}")).as_ptr() as LPARAM,
                );
            }
            if let Some(preview) = &preview {
                if !preview.note.is_empty() {
                    let note = CreateWindowExW(
                        0,
                        wide("STATIC").as_ptr(),
                        ui(preview.note).as_ptr(),
                        WS_CHILD | WS_VISIBLE,
                        scaled(16, dpi),
                        y(note_top),
                        scaled(width - 32, dpi),
                        scaled(note_height, dpi),
                        hwnd,
                        null_mut(),
                        instance,
                        null_mut(),
                    );
                    SendMessageW(note, WM_SETFONT, font as usize, 0);
                }
            } else {
                ui::style_vertical_scrollbar(table, 12, dpi);
            }
        }
        let mut focus = content;
        let actions: &[(&str, usize, i32, i32)] =
            if let (Some(action), Some(alternate)) = (action, alternate) {
                &[
                    (action, 1, 424, 160),
                    (alternate, 3, 234, 180),
                    ("ui.cancel", 2, 140, 84),
                ]
            } else if let Some(action) = action {
                &[(action, 1, 404, 180), ("ui.cancel", 2, 310, 84)]
            } else if has_rows {
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
                y(action_y),
                scaled(width, dpi),
                scaled(28, dpi),
                hwnd,
                id as HMENU,
                instance,
                std::ptr::null(),
            );
            SendMessageW(button, WM_SETFONT, font as usize, 0);
            if message.is_some() && id == 1 {
                focus = button;
            }
        }
        let modal_owner = super::ModalOwner::disable(owner);
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
                if msg.wParam as u16 == VK_ESCAPE {
                    SendMessageW(hwnd, WM_COMMAND, 2, 0);
                } else {
                    super::accept_key(hwnd, 1, &[2, 3, 4]);
                }
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
        modal_owner.restore(true);
        DeleteObject(font);
        if !(*state).caption.is_null() {
            DeleteObject((*state).caption);
        }
        UnregisterClassW(class.as_ptr(), instance);
        DeleteObject(background);
        DeleteObject(paper);
        let state = Box::from_raw(state);
        (state.value, state.alternate)
    }
}
