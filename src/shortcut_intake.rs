// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

//! Quick-add dialog for file-system targets received from Explorer.
use crate::{
    theme,
    ui::{self, make_font, scaled, ui, wide},
};
use altrun_probe::{Entry, library::OpenTarget};
use std::{cell::Cell, path::Path, ptr::null_mut};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::LibraryLoader::GetModuleHandleW,
    UI::{
        Controls::*,
        Input::{Ime::*, KeyboardAndMouse::*},
        WindowsAndMessaging::*,
    },
};

const SAVE: usize = 1;
const CANCEL: usize = 2;
const ALIAS: usize = 3101;
const NAME: usize = 3102;
const PATH: usize = 3103;
const ROWS: usize = 3104;
const PROGRAM: usize = 3105;
const DEFAULT: usize = 3106;
const TEST: usize = 3107;
const STATUS: usize = 3108;
const OPEN_LABEL: usize = 3109;
const CLOSE: usize = 3110;

thread_local! { static OPEN: Cell<bool> = const { Cell::new(false) }; }

type Save = fn(&[Entry]) -> Result<(), (usize, String)>;
struct Dialog {
    entries: Vec<Entry>,
    selected: usize,
    updating: bool,
    dpi: i32,
    height: i32,
    caption: HFONT,
    launch_enabled: bool,
    save: Save,
}

pub fn is_open() -> bool {
    OPEN.with(Cell::get)
}

pub fn plain_path(text: &str) -> &str {
    let text = text.trim();
    text.strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .unwrap_or(text)
}

pub fn program_name(path: &str) -> &str {
    Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(path)
}

pub fn pick_program(owner: HWND) -> Result<Option<String>, String> {
    let mut filter = ui("ui.executable_programs");
    filter.extend(wide("*.exe"));
    filter.push(0);
    let Some(path) =
        crate::windows::file_picker::pick_path_filtered(owner, false, false, Some(&filter))
    else {
        return Ok(None);
    };
    if !path.is_file() || !altrun_probe::management::is_executable(&path) {
        return Err("ui.select_a_valid_exe_application".into());
    }
    altrun_probe::management::absolute_path(&path).map(Some)
}

fn get(hwnd: HWND, id: usize) -> HWND {
    unsafe { GetDlgItem(hwnd, id as i32) }
}

fn read(hwnd: HWND) -> String {
    unsafe {
        let mut buffer = vec![0; GetWindowTextLengthW(hwnd).max(0) as usize + 1];
        let len = GetWindowTextW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32);
        String::from_utf16_lossy(&buffer[..len.max(0) as usize])
    }
}

fn label(hwnd: HWND, id: usize, text: &str) {
    unsafe {
        SetWindowTextW(get(hwnd, id), wide(text).as_ptr());
    }
}

fn status(hwnd: HWND, message: &str) {
    label(hwnd, STATUS, &crate::localization::tr(message));
}

fn sync(hwnd: HWND, state: *mut Dialog) {
    unsafe {
        let entry = (&(*state).entries)[(*state).selected].clone();
        (*state).updating = true;
        label(hwnd, ALIAS, &entry.alias);
        label(hwnd, NAME, &entry.name);
        label(hwnd, PATH, &entry.command);
        let document = match &entry.open_target {
            Some(OpenTarget::Document(program)) => {
                let name = program.as_deref().map(program_name);
                let caption = name.map_or_else(
                    || crate::localization::tr("ui.system_default"),
                    str::to_owned,
                );
                label(
                    hwnd,
                    OPEN_LABEL,
                    &format!("{}  ·  {caption}", crate::localization::tr("ui.open_with")),
                );
                EnableWindow(get(hwnd, DEFAULT), i32::from(name.is_some()));
                true
            }
            _ => false,
        };
        for id in [OPEN_LABEL, PROGRAM, DEFAULT] {
            ShowWindow(get(hwnd, id), if document { SW_SHOWNA } else { SW_HIDE });
        }
        (*state).updating = false;
        InvalidateRect(get(hwnd, ROWS), std::ptr::null(), 0);
    }
}

fn edit_row(hwnd: HWND, state: *mut Dialog) {
    let alias = read(get(hwnd, ALIAS));
    let name = read(get(hwnd, NAME));
    unsafe {
        let entry = &mut (&mut (*state).entries)[(*state).selected];
        entry.alias = alias;
        entry.name = name;
        let rows = get(hwnd, ROWS);
        if !rows.is_null() {
            InvalidateRect(rows, std::ptr::null(), 0);
        }
    }
}

fn draw_row(item: &DRAWITEMSTRUCT, state: &Dialog) {
    let rect = item.rcItem;
    let selected = item.itemState & ODS_SELECTED != 0;
    let colour = if selected {
        theme::selection()
    } else {
        theme::paper()
    };
    ui::fill(item.hDC, &rect, colour);
    let Some(entry) = state.entries.get(item.itemID as usize) else {
        return;
    };
    let px = |value| scaled(value, state.dpi);
    for (value, left, right) in [
        (
            entry.alias.as_str(),
            rect.left + px(12),
            rect.left + px(178),
        ),
        (
            entry.name.as_str(),
            rect.left + px(190),
            rect.left + px(402),
        ),
        (
            entry.command.as_str(),
            rect.left + px(414),
            rect.right - px(12),
        ),
    ] {
        let mut bounds = RECT {
            left,
            right,
            ..rect
        };
        unsafe {
            SetBkMode(item.hDC, TRANSPARENT as i32);
            SetTextColor(item.hDC, theme::text());
            DrawTextW(
                item.hDC,
                wide(value).as_ptr(),
                -1,
                &mut bounds,
                DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS | DT_NOPREFIX,
            );
        }
    }
    if item.itemState & ODS_FOCUS != 0 {
        unsafe {
            DrawFocusRect(item.hDC, &rect);
        }
    }
}

unsafe extern "system" fn procedure(hwnd: HWND, message: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    unsafe {
        if message == WM_NCCREATE {
            let create = &*(l as *const CREATESTRUCTW);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize);
        }
        let state = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Dialog;
        if state.is_null() {
            return DefWindowProcW(hwnd, message, w, l);
        }
        match message {
            WM_CLOSE => {
                DestroyWindow(hwnd);
                return 0;
            }
            WM_NCHITTEST => {
                let mut point = POINT {
                    x: l as i16 as i32,
                    y: (l >> 16) as i16 as i32,
                };
                ScreenToClient(hwnd, &mut point);
                let mut rect = RECT::default();
                GetClientRect(hwnd, &mut rect);
                if (0..scaled(42, (*state).dpi)).contains(&point.y)
                    && (0..rect.right - scaled(47, (*state).dpi)).contains(&point.x)
                {
                    return HTCAPTION as isize;
                }
            }
            WM_ERASEBKGND => return 1,
            WM_PAINT => {
                let mut paint = PAINTSTRUCT::default();
                let dc = BeginPaint(hwnd, &mut paint);
                let mut rect = RECT::default();
                GetClientRect(hwnd, &mut rect);
                let px = |v| scaled(v, (*state).dpi);
                ui::fill(dc, &rect, theme::surface());
                for bar in [
                    RECT {
                        bottom: px(42),
                        ..rect
                    },
                    RECT {
                        top: px((*state).height - 59),
                        ..rect
                    },
                ] {
                    ui::fill(dc, &bar, theme::chrome());
                }
                ui::panel_caption(dc, (*state).dpi, (*state).caption, "ui.add_shortcut");
                EndPaint(hwnd, &paint);
                return 0;
            }
            WM_CTLCOLOREDIT | WM_CTLCOLORSTATIC | WM_CTLCOLORLISTBOX => {
                let id = GetDlgCtrlID(l as HWND) as usize;
                let background = if matches!(id, ALIAS | NAME | PATH | ROWS) {
                    theme::paper()
                } else {
                    theme::surface()
                };
                SetTextColor(
                    w as HDC,
                    if id == STATUS {
                        theme::error()
                    } else {
                        theme::text()
                    },
                );
                SetBkColor(w as HDC, background);
                SetDCBrushColor(w as HDC, background);
                return GetStockObject(DC_BRUSH) as isize;
            }
            WM_DRAWITEM if l != 0 => {
                let item = &*(l as *const DRAWITEMSTRUCT);
                if item.CtlID as usize == ROWS {
                    draw_row(item, &*state);
                } else if item.CtlID as usize == CLOSE {
                    ui::caption_close(item, (*state).dpi);
                } else {
                    let background = if matches!(item.CtlID as usize, PROGRAM | DEFAULT) {
                        theme::surface()
                    } else {
                        theme::chrome()
                    };
                    ui::button_on(item, item.CtlID as usize == SAVE, background, (*state).dpi);
                }
                return 1;
            }
            WM_COMMAND => {
                let id = w & 65535;
                let notification = (w >> 16) & 65535;
                if (*state).updating {
                    return 0;
                }
                match id {
                    ALIAS | NAME if notification == EN_CHANGE as usize => edit_row(hwnd, state),
                    ROWS if notification == LBN_SELCHANGE as usize => {
                        let row = SendMessageW(get(hwnd, ROWS), LB_GETCURSEL, 0, 0);
                        let entries = &(*state).entries;
                        if row >= 0 && (row as usize) < entries.len() {
                            (*state).selected = row as usize;
                            sync(hwnd, state);
                            status(hwnd, "");
                        }
                    }
                    PROGRAM | DEFAULT => {
                        let program = if id == DEFAULT {
                            None
                        } else {
                            match pick_program(hwnd) {
                                Ok(Some(path)) => Some(path),
                                Ok(None) => return 0,
                                Err(error) => {
                                    status(hwnd, &error);
                                    return 0;
                                }
                            }
                        };
                        if let Some(OpenTarget::Document(current)) =
                            &mut (&mut (*state).entries)[(*state).selected].open_target
                        {
                            *current = program;
                        }
                        sync(hwnd, state);
                        status(hwnd, "");
                    }
                    TEST if (*state).launch_enabled => {
                        let entry = (&(*state).entries)[(*state).selected].clone();
                        let owner = hwnd as usize;
                        std::thread::spawn(move || {
                            let result = altrun_probe::launch::resolve_entry(
                                &entry,
                                None,
                                &Default::default(),
                            )
                            .and_then(|(request, mode, directory)| {
                                crate::windows::shell_execute::execute_request_with(
                                    &request,
                                    mode,
                                    directory.as_deref(),
                                )
                            });
                            if let Err(error) = result {
                                let owner = owner as HWND;
                                if IsWindow(owner) != 0 {
                                    MessageBoxW(
                                        owner,
                                        ui(&error).as_ptr(),
                                        ui("ui.try_opening").as_ptr(),
                                        MB_OK | MB_ICONERROR,
                                    );
                                }
                            }
                        });
                    }
                    SAVE => {
                        let mut entries = Vec::clone(&(*state).entries);
                        for entry in &mut entries {
                            entry.alias = entry.alias.trim().to_owned();
                            entry.name = entry.name.trim().to_owned();
                            if entry.name.is_empty() {
                                entry.name = entry.alias.clone();
                            }
                        }
                        match ((*state).save)(&entries) {
                            Ok(()) => {
                                DestroyWindow(hwnd);
                            }
                            Err((row, error)) => {
                                (*state).selected = row;
                                let list = get(hwnd, ROWS);
                                if !list.is_null() {
                                    SendMessageW(list, LB_SETCURSEL, row, 0);
                                }
                                sync(hwnd, state);
                                status(hwnd, &error);
                                let field = get(hwnd, ALIAS);
                                SetFocus(field);
                                SendMessageW(field, EM_SETSEL, 0, -1);
                            }
                        }
                    }
                    CANCEL | CLOSE => {
                        DestroyWindow(hwnd);
                    }
                    _ => {}
                }
                return 0;
            }
            _ => {}
        }
        DefWindowProcW(hwnd, message, w, l)
    }
}

pub fn open(owner: HWND, entries: Vec<Entry>, launch_enabled: bool, save: Save) {
    if entries.is_empty() {
        return;
    }
    OPEN.with(|open| open.set(true));
    unsafe {
        let instance = GetModuleHandleW(std::ptr::null());
        let class = wide("ALTRun.Native.AddShortcut");
        let batch = entries.len() > 1;
        let (width, height, form) = if batch {
            (840, 542, 246)
        } else {
            (700, 354, 58)
        };
        let mut state = Dialog {
            entries,
            selected: 0,
            updating: true,
            dpi: 96,
            height,
            caption: null_mut(),
            launch_enabled,
            save,
        };
        RegisterClassW(&WNDCLASSW {
            lpfnWndProc: Some(procedure),
            hInstance: instance,
            lpszClassName: class.as_ptr(),
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            ..Default::default()
        });
        let mut rect = RECT::default();
        GetWindowRect(owner, &mut rect);
        let hwnd = CreateWindowExW(
            WS_EX_CONTROLPARENT,
            class.as_ptr(),
            ui("ui.add_shortcut").as_ptr(),
            WS_POPUP | WS_SYSMENU | WS_CLIPCHILDREN,
            rect.left,
            rect.top,
            1,
            1,
            owner,
            null_mut(),
            instance,
            &mut state as *mut Dialog as _,
        );
        if hwnd.is_null() {
            UnregisterClassW(class.as_ptr(), instance);
            OPEN.with(|open| open.set(false));
            crate::ui::show_error("ui.could_not_open_the_add_shortcut_window");
            return;
        }
        state.dpi = crate::ui::geometry::fit(hwnd, width, height);
        let dpi = state.dpi;
        let px = |value| scaled(value, dpi);
        state.caption = make_font(13, 600, "Microsoft YaHei UI", dpi);
        let font = make_font(14, 400, "Microsoft YaHei UI", dpi);
        theme::frame(hwnd);
        ui::round_panel(hwnd, px(width), px(height), dpi);
        let control = |class: &str, text: &str, id, (x, y, w, h), style| {
            let child = CreateWindowExW(
                0,
                wide(class).as_ptr(),
                ui(text).as_ptr(),
                WS_CHILD | WS_VISIBLE | style,
                px(x),
                px(y),
                px(x + w) - px(x),
                px(y + h) - px(y),
                hwnd,
                id as HMENU,
                instance,
                std::ptr::null(),
            );
            SendMessageW(child, WM_SETFONT, font as usize, 0);
            child
        };
        let button = |text, id, bounds| {
            control("BUTTON", text, id, bounds, WS_TABSTOP | BS_OWNERDRAW as u32)
        };
        ui::style_caption_close(button("ui.close", CLOSE, (width - 47, 1, 46, 41)));
        if batch {
            for (caption, x, w) in [
                ("ui.alias", 34, 166),
                ("ui.name", 212, 212),
                ("ui.target_path", 436, width - 460),
            ] {
                control("STATIC", caption, 0, (x, 55, w, 24), 0);
            }
            let list = control(
                "LISTBOX",
                "",
                ROWS,
                (22, 84, width - 44, 145),
                WS_TABSTOP
                    | WS_VSCROLL
                    | LBS_NOTIFY as u32
                    | LBS_OWNERDRAWFIXED as u32
                    | LBS_HASSTRINGS as u32
                    | LBS_NOINTEGRALHEIGHT as u32,
            );
            SendMessageW(list, LB_SETITEMHEIGHT, 0, px(29) as LPARAM);
            for entry in &state.entries {
                let accessible = format!("{} · {} · {}", entry.alias, entry.name, entry.command);
                SendMessageW(list, LB_ADDSTRING, 0, wide(&accessible).as_ptr() as LPARAM);
            }
            SendMessageW(list, LB_SETCURSEL, 0, 0);
            ui::style_vertical_scrollbar(list, 12, dpi);
        }
        let label_style = ui::STATIC_CENTER_IMAGE;
        for (caption, id, dy) in [
            ("ui.alias", ALIAS, 0),
            ("ui.name", NAME, 48),
            ("ui.target_path", PATH, 96),
        ] {
            control("STATIC", caption, 0, (22, form + dy, 113, 36), label_style);
            let style = WS_TABSTOP
                | ES_AUTOHSCROLL as u32
                | if id == PATH { ES_READONLY as u32 } else { 0 };
            ui::style_edit_with_colours(
                control("EDIT", "", id, (140, form + dy, width - 162, 36), style),
                dpi,
                || (theme::surface(), theme::paper()),
            );
        }
        control(
            "STATIC",
            "ui.open_with",
            OPEN_LABEL,
            (22, form + 154, width - 326, 36),
            label_style,
        );
        button(
            "ui.use_system_default",
            DEFAULT,
            (width - 299, form + 154, 132, 36),
        );
        button("ui.choose_app", PROGRAM, (width - 156, form + 154, 134, 36));
        control(
            "STATIC",
            "",
            STATUS,
            (22, height - 99, width - 44, 34),
            label_style,
        );
        let test = button("ui.try_opening", TEST, (22, height - 48, 100, 36));
        EnableWindow(test, i32::from(launch_enabled));
        button("ui.cancel", CANCEL, (width - 257, height - 48, 86, 36));
        button("ui.add_shortcut", SAVE, (width - 159, height - 48, 137, 36));
        sync(hwnd, &mut state);
        EnableWindow(owner, 0);
        ShowWindow(hwnd, SW_SHOW);
        SetForegroundWindow(hwnd);
        SetFocus(get(hwnd, ALIAS));
        SendMessageW(get(hwnd, ALIAS), EM_SETSEL, 0, -1);
        while IsWindow(hwnd) != 0 {
            let mut msg = MSG::default();
            let result = GetMessageW(&mut msg, null_mut(), 0, 0);
            if result <= 0 {
                if result == 0 {
                    PostQuitMessage(msg.wParam as i32);
                }
                break;
            }
            if msg.message == WM_KEYDOWN
                && (msg.hwnd == hwnd || IsChild(hwnd, msg.hwnd) != 0)
                && matches!(msg.wParam as u16, VK_RETURN | VK_ESCAPE)
            {
                let focus = GetFocus();
                let ime = ImmGetContext(focus);
                let composing =
                    !ime.is_null() && ImmGetCompositionStringW(ime, GCS_COMPSTR, null_mut(), 0) > 0;
                if !ime.is_null() {
                    ImmReleaseContext(focus, ime);
                }
                if composing {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                    continue;
                }
                if msg.wParam as u16 == VK_ESCAPE {
                    SendMessageW(hwnd, WM_COMMAND, CANCEL, 0);
                } else {
                    ui::dialogs::accept_key(hwnd, SAVE, &[PROGRAM, DEFAULT, TEST, CANCEL, CLOSE]);
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
        EnableWindow(owner, 1);
        if IsWindowVisible(owner) != 0 {
            SetForegroundWindow(owner);
        }
        DeleteObject(font);
        DeleteObject(state.caption);
        UnregisterClassW(class.as_ptr(), instance);
    }
    OPEN.with(|open| open.set(false));
}
