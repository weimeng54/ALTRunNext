// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::{Contents, RestoreSelection};
use crate::{
    localization::{message, tr},
    theme,
    ui::{
        self,
        dialogs::{ModalOwner, accept_key},
        make_font, scaled, wide,
    },
};
use std::{cell::Cell, ptr::null_mut};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::LibraryLoader::GetModuleHandleW,
    UI::{Controls::*, Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
};

mod paint;
mod selection;
use paint::{draw_frame, draw_option};
use selection::{details, read, refresh};

const WIDTH: i32 = 620;
const CARD_TOP: i32 = 95;
const ROW: i32 = 62;
const ITEMS: usize = 10;
const SETTINGS: usize = 11;
const HISTORY: usize = 12;
const APPEND: usize = 13;
const REPLACE: usize = 14;
const DETAILS: usize = 15;
const NOTE: usize = 16;
const TOTAL: usize = 17;
const ITEM_INFO: usize = 18;
const SUBMIT: usize = 1;
const CANCEL: usize = 2;
const CLOSE: usize = 4;
const FOOTER: i32 = 61;
const CAPTION: i32 = ui::geometry::CAPTION;
const STATIC_LINE: u32 = ui::STATIC_CENTER_IMAGE | 0x80;

pub(super) struct Options {
    pub heading: String,
    pub available: Contents,
    pub current: usize,
    pub incoming: usize,
    pub conflicts: Vec<(String, &'static str)>,
    pub media: [(&'static str, String); 2],
}

struct Window<'a> {
    dpi: i32,
    height: i32,
    first_row: i32,
    title: &'a str,
    fonts: [HFONT; 3],
    brushes: [HBRUSH; 3],
    event: Cell<usize>,
}

fn rect(dpi: i32, x: i32, y: i32, width: i32, height: i32) -> RECT {
    RECT {
        left: scaled(x, dpi),
        top: scaled(y, dpi),
        right: scaled(x + width, dpi),
        bottom: scaled(y + height, dpi),
    }
}

unsafe extern "system" fn procedure(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    unsafe {
        if msg == WM_NCCREATE {
            let create = &*(l as *const CREATESTRUCTW);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize);
        }
        let state = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const Window<'_>;
        let Some(state) = state.as_ref() else {
            return DefWindowProcW(hwnd, msg, w, l);
        };
        match msg {
            WM_ERASEBKGND => 1,
            WM_PAINT => {
                let mut paint = PAINTSTRUCT::default();
                let dc = BeginPaint(hwnd, &mut paint);
                ui::buffered(dc, paint.rcPaint, |target| draw_frame(state, target));
                EndPaint(hwnd, &paint);
                0
            }
            WM_PRINTCLIENT => {
                draw_frame(state, w as HDC);
                1
            }
            WM_NCHITTEST => {
                let mut point = POINT {
                    x: l as i16 as i32,
                    y: (l >> 16) as i16 as i32,
                };
                ScreenToClient(hwnd, &mut point);
                if (0..scaled(CAPTION, state.dpi)).contains(&point.y)
                    && point.x < scaled(WIDTH - 46, state.dpi)
                {
                    HTCAPTION as isize
                } else {
                    HTCLIENT as isize
                }
            }
            WM_CTLCOLORSTATIC | WM_CTLCOLORBTN => {
                let id = GetDlgCtrlID(l as HWND) as usize;
                let index = if id == TOTAL {
                    2
                } else if (ITEMS..=REPLACE).contains(&id) || (ITEM_INFO..=20).contains(&id) {
                    1
                } else {
                    0
                };
                SetTextColor(
                    w as HDC,
                    if id == TOTAL {
                        theme::text()
                    } else {
                        theme::muted()
                    },
                );
                SetBkColor(
                    w as HDC,
                    [theme::surface(), theme::paper(), theme::chrome()][index],
                );
                state.brushes[index] as isize
            }
            WM_DRAWITEM if l != 0 => {
                let item = &*(l as *const DRAWITEMSTRUCT);
                match item.CtlID as usize {
                    CLOSE => ui::caption_close(item, state.dpi),
                    id => ui::button_on(item, id == SUBMIT, theme::chrome(), state.dpi),
                }
                1
            }
            WM_NOTIFY if l != 0 => {
                let hdr = &*(l as *const NMHDR);
                if hdr.code == NM_CUSTOMDRAW && (ITEMS..=REPLACE).contains(&hdr.idFrom) {
                    let info = &*(l as *const NMCUSTOMDRAW);
                    if info.dwDrawStage == CDDS_PREPAINT {
                        draw_option(state, info);
                        return CDRF_SKIPDEFAULT as isize;
                    }
                }
                0
            }
            WM_COMMAND if (w >> 16) as u32 == BN_CLICKED => {
                state.event.set(w & 0xffff);
                0
            }
            WM_CLOSE => {
                state.event.set(CANCEL);
                0
            }
            _ => DefWindowProcW(hwnd, msg, w, l),
        }
    }
}

pub(super) fn create(owner: HWND, item_count: usize) -> Option<RestoreSelection> {
    open(owner, item_count, None)
}

pub(super) fn restore(owner: HWND, options: Options) -> Option<RestoreSelection> {
    open(owner, options.incoming, Some(options))
}

fn open(owner: HWND, item_count: usize, options: Option<Options>) -> Option<RestoreSelection> {
    let restore = options.is_some();
    let available = options
        .as_ref()
        .map_or(Contents::ALL, |opts| opts.available);
    let show_modes = restore && available.items;
    let first_row = ROW + if show_modes { 43 } else { 0 };
    let note_y = CARD_TOP + first_row + ROW * 2 + 14;
    let height = note_y + if restore { 64 } else { 36 } + FOOTER;
    let title = if restore {
        "backup.preview_restore"
    } else {
        "backup.preview_create"
    };
    unsafe {
        let instance = GetModuleHandleW(std::ptr::null());
        let class = wide("ALTRun.Native.BackupSelection");
        RegisterClassW(&WNDCLASSW {
            lpfnWndProc: Some(procedure),
            hInstance: instance,
            lpszClassName: class.as_ptr(),
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            ..WNDCLASSW::default()
        });
        let mut state = Window {
            dpi: 96,
            height,
            first_row,
            title,
            fonts: [null_mut(); 3],
            brushes: [theme::surface(), theme::paper(), theme::chrome()]
                .map(|c| CreateSolidBrush(c)),
            event: Cell::new(0),
        };
        let hwnd = CreateWindowExW(
            WS_EX_DLGMODALFRAME | WS_EX_CONTROLPARENT,
            class.as_ptr(),
            wide(&tr(title)).as_ptr(),
            WS_POPUP | WS_SYSMENU | WS_CLIPCHILDREN,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            WIDTH,
            height,
            owner,
            null_mut(),
            instance,
            (&mut state as *mut Window<'_>).cast(),
        );
        let result = if hwnd.is_null() {
            None
        } else {
            theme::frame(hwnd);
            state.dpi = ui::geometry::fit(hwnd, WIDTH, height);
            state.fonts = [(15, 400), (15, 600), (13, 400)]
                .map(|(size, weight)| make_font(size, weight, "Microsoft YaHei UI", state.dpi));
            ui::round_panel(
                hwnd,
                scaled(WIDTH, state.dpi),
                scaled(height, state.dpi),
                state.dpi,
            );
            let control = |class: &str,
                           id,
                           value: &str,
                           style,
                           font: usize,
                           position: (i32, i32, i32, i32)| {
                let (x, y, w, h) = position;
                let b = rect(state.dpi, x, y, w, h);
                let child = CreateWindowExW(
                    0,
                    wide(class).as_ptr(),
                    wide(value).as_ptr(),
                    WS_CHILD | WS_VISIBLE | style,
                    b.left,
                    b.top,
                    b.right - b.left,
                    b.bottom - b.top,
                    hwnd,
                    id as HMENU,
                    instance,
                    null_mut(),
                );
                SendMessageW(child, WM_SETFONT, state.fonts[font] as usize, 0);
                child
            };
            let action = |id, label, position| {
                control(
                    "BUTTON",
                    id,
                    &tr(label),
                    WS_TABSTOP | BS_OWNERDRAW as u32,
                    0,
                    position,
                )
            };
            ui::style_caption_close(action(CLOSE, "ui.close", (WIDTH - 46, 0, 46, CAPTION)));
            let heading = options
                .as_ref()
                .map_or_else(|| tr("backup.saved_data"), |opts| opts.heading.clone());
            control(
                "STATIC",
                0,
                &heading,
                STATIC_LINE,
                0,
                (20, 56, WIDTH - 40, 27),
            );
            let sections = [
                (ITEMS, "backup.launch_items", available.items),
                (SETTINGS, "backup.settings", available.settings),
                (HISTORY, "backup.history", available.history),
            ];
            for (row, (id, label, present)) in sections.into_iter().enumerate() {
                let y = match row {
                    0 => CARD_TOP,
                    _ => CARD_TOP + first_row + (row as i32 - 1) * ROW,
                };
                let button = control(
                    "BUTTON",
                    id,
                    &tr(label),
                    WS_TABSTOP | WS_GROUP | BS_AUTOCHECKBOX as u32,
                    1,
                    (36, y + 8, 540, 25),
                );
                if present {
                    SendMessageW(button, BM_SETCHECK, BST_CHECKED as usize, 0);
                } else {
                    EnableWindow(button, 0);
                }
                let subtitle = if !present {
                    tr("backup.not_available")
                } else {
                    match id {
                        ITEMS => {
                            message("backup.items_count", &[("count", &item_count.to_string())])
                        }
                        SETTINGS => tr("backup.settings_contents"),
                        _ => tr(if restore {
                            "backup.history_matches"
                        } else {
                            "backup.history_contents"
                        }),
                    }
                };
                control(
                    "STATIC",
                    ITEM_INFO + row,
                    &subtitle,
                    STATIC_LINE,
                    2,
                    (63, y + 34, 518, 21),
                );
                if id == ITEMS && show_modes {
                    for (mode_id, key, x, flags) in [
                        (APPEND, "ui.import_append_mode", 63, WS_TABSTOP | WS_GROUP),
                        (REPLACE, "ui.import_replace_mode", 318, 0),
                    ] {
                        let mode = control(
                            "BUTTON",
                            mode_id,
                            &tr(key),
                            flags | BS_AUTORADIOBUTTON as u32,
                            0,
                            (x, y + 63, 245, 30),
                        );
                        if mode_id == APPEND {
                            SendMessageW(mode, BM_SETCHECK, BST_CHECKED as usize, 0);
                        }
                    }
                }
            }
            control(
                "STATIC",
                NOTE,
                "",
                0x80,
                2,
                (20, note_y, WIDTH - 40, height - FOOTER - note_y - 7),
            );
            let button_y = height - FOOTER + 13;
            control(
                "STATIC",
                TOTAL,
                "",
                ui::STATIC_CENTER_IMAGE,
                0,
                (20, button_y, 165, 35),
            );
            if restore {
                action(DETAILS, "backup.details", (174, button_y, 121, 35));
            }
            action(CANCEL, "ui.cancel", (301, button_y, 100, 35));
            action(
                SUBMIT,
                if restore {
                    "backup.confirm_restore"
                } else {
                    "backup.confirm_create"
                },
                (411, button_y, 189, 35),
            );
            let modal = ModalOwner::disable(owner);
            let result = run(hwnd, &state, options.as_ref());
            // The owner must be re-enabled before destroying the active popup.
            modal.restore(true);
            DestroyWindow(hwnd);
            result
        };
        for brush in state.brushes {
            DeleteObject(brush);
        }
        for font in state.fonts {
            DeleteObject(font);
        }
        UnregisterClassW(class.as_ptr(), instance);
        result
    }
}

fn run(hwnd: HWND, state: &Window<'_>, options: Option<&Options>) -> Option<RestoreSelection> {
    unsafe {
        refresh(hwnd, options);
        ShowWindow(hwnd, SW_SHOW);
        let primary = GetDlgItem(hwnd, SUBMIT as i32);
        if IsWindowEnabled(primary) != 0 {
            SetFocus(primary);
        } else {
            SetFocus(GetDlgItem(hwnd, CANCEL as i32));
        }
        loop {
            let mut msg = MSG::default();
            let result = GetMessageW(&mut msg, null_mut(), 0, 0);
            if result <= 0 {
                if result == 0 {
                    PostQuitMessage(msg.wParam as i32);
                }
                return None;
            }
            if msg.message == WM_KEYDOWN
                && (msg.hwnd == hwnd || IsChild(hwnd, msg.hwnd) != 0)
                && matches!(msg.wParam as u16, VK_RETURN | VK_ESCAPE)
            {
                if msg.wParam as u16 == VK_ESCAPE {
                    state.event.set(CANCEL);
                } else {
                    accept_key(hwnd, SUBMIT, &[CANCEL, CLOSE, DETAILS]);
                }
            } else if IsDialogMessageW(hwnd, &msg) == 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            match state.event.replace(0) {
                SUBMIT if IsWindowEnabled(primary) != 0 => return Some(read(hwnd)),
                CANCEL | CLOSE => return None,
                DETAILS if IsWindowEnabled(GetDlgItem(hwnd, DETAILS as i32)) != 0 => {
                    if let Some(options) = options {
                        details(hwnd, options);
                    }
                }
                ITEMS..=REPLACE => refresh(hwnd, options),
                _ => {}
            }
        }
    }
}
