// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use crate::{
    localization,
    native::{make_font, scaled, ui, wide},
    theme, theme_ui,
};
use std::ptr::{null, null_mut};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::LibraryLoader::GetModuleHandleW,
    UI::{Controls::*, Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
};

const CLASS: &str = "ALTRun.Native.StartupWarning";
const WIDTH: i32 = 416;
const HEIGHT: i32 = 192;
const FOOTER: i32 = 140;
const CHANGE: usize = 1;
const CLOSE: usize = 2;
const CAPTION_CLOSE: usize = 3;

struct Warning {
    dpi: i32,
    caption: HFONT,
    change: bool,
}

unsafe extern "system" fn procedure(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    unsafe {
        if msg == WM_NCCREATE {
            SetWindowLongPtrW(
                hwnd,
                GWLP_USERDATA,
                (*(l as *const CREATESTRUCTW)).lpCreateParams as isize,
            );
        }
        let state = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Warning;
        if !state.is_null() {
            match msg {
                WM_ERASEBKGND => return 1,
                WM_PAINT => {
                    let mut ps = PAINTSTRUCT::default();
                    let dc = BeginPaint(hwnd, &mut ps);
                    paint(hwnd, dc, &*state);
                    EndPaint(hwnd, &ps);
                    return 0;
                }
                WM_CTLCOLORSTATIC => {
                    let dc = w as HDC;
                    let color = if GetDlgCtrlID(l as HWND) == 11 {
                        theme::muted()
                    } else {
                        theme::text()
                    };
                    SetTextColor(dc, color);
                    SetBkColor(dc, theme::surface());
                    SetDCBrushColor(dc, theme::surface());
                    return GetStockObject(DC_BRUSH) as isize;
                }
                WM_DRAWITEM if l != 0 => {
                    let item = &*(l as *const DRAWITEMSTRUCT);
                    if item.CtlID as usize == CAPTION_CLOSE {
                        theme_ui::caption_close(item, (*state).dpi);
                    } else {
                        theme_ui::button_on(
                            item,
                            item.CtlID as usize == CHANGE,
                            theme::chrome(),
                            (*state).dpi,
                        );
                    }
                    return 1;
                }
                WM_CLOSE => {
                    DestroyWindow(hwnd);
                    return 0;
                }
                WM_COMMAND if matches!(w & 65535, CHANGE | CLOSE | CAPTION_CLOSE) => {
                    (*state).change = w & 65535 == CHANGE;
                    DestroyWindow(hwnd);
                    return 0;
                }
                _ => {}
            }
        }
        DefWindowProcW(hwnd, msg, w, l)
    }
}

unsafe fn paint(hwnd: HWND, dc: HDC, state: &Warning) {
    unsafe {
        let mut rect = RECT::default();
        GetClientRect(hwnd, &mut rect);
        let px = |value| scaled(value, state.dpi);
        theme_ui::fill(dc, &rect, theme::surface());
        for bar in [
            RECT {
                bottom: px(42),
                ..rect
            },
            RECT {
                top: px(FOOTER),
                ..rect
            },
        ] {
            theme_ui::fill(dc, &bar, theme::chrome());
        }
        theme_ui::fill(
            dc,
            &RECT {
                top: px(FOOTER),
                bottom: px(FOOTER) + 1,
                ..rect
            },
            theme::divider(),
        );
        theme_ui::panel_caption(dc, state.dpi, state.caption, "ui.startup_warning_title");
        let amber = if theme::high_contrast() {
            theme::text()
        } else if theme::is_dark() {
            theme::rgb(255, 187, 73)
        } else {
            theme::rgb(164, 97, 0)
        };
        let pen = CreatePen(PS_SOLID, px(2).max(1), amber);
        let old_pen = SelectObject(dc, pen);
        let old_brush = SelectObject(dc, GetStockObject(NULL_BRUSH));
        let points = [
            POINT {
                x: px(34),
                y: px(63),
            },
            POINT {
                x: px(21),
                y: px(86),
            },
            POINT {
                x: px(47),
                y: px(86),
            },
        ];
        Polygon(dc, points.as_ptr(), 3);
        theme_ui::fill(
            dc,
            &RECT {
                left: px(33),
                top: px(71),
                right: px(35),
                bottom: px(78),
            },
            amber,
        );
        theme_ui::fill(
            dc,
            &RECT {
                left: px(33),
                top: px(81),
                right: px(35),
                bottom: px(83),
            },
            amber,
        );
        SelectObject(dc, old_brush);
        SelectObject(dc, old_pen);
        DeleteObject(pen);
    }
}

pub fn show(owner: HWND, binding: &str, tray: bool) -> bool {
    unsafe {
        let instance = GetModuleHandleW(null());
        let class = wide(CLASS);
        let title = ui("ui.startup_warning_title");
        let mut state = Warning {
            dpi: 96,
            caption: null_mut(),
            change: false,
        };
        let window_class = WNDCLASSW {
            lpfnWndProc: Some(procedure),
            hInstance: instance,
            lpszClassName: class.as_ptr(),
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            ..Default::default()
        };
        if RegisterClassW(&window_class) == 0 {
            return false;
        }
        let hwnd = CreateWindowExW(
            WS_EX_CONTROLPARENT,
            class.as_ptr(),
            title.as_ptr(),
            WS_POPUP | WS_SYSMENU | WS_CLIPCHILDREN,
            0,
            0,
            WIDTH,
            HEIGHT,
            owner,
            null_mut(),
            instance,
            (&mut state as *mut Warning).cast(),
        );
        if hwnd.is_null() {
            UnregisterClassW(class.as_ptr(), instance);
            return false;
        }
        theme::frame(hwnd);
        let dpi = crate::window_geometry::fit(hwnd, WIDTH, HEIGHT);
        state.dpi = dpi;
        let px = |value| scaled(value, dpi);
        theme_ui::round_panel(hwnd, px(WIDTH), px(HEIGHT), dpi);
        let caption = make_font(13, 600, "Microsoft YaHei UI", dpi);
        state.caption = caption;
        let heading = make_font(17, 700, "Microsoft YaHei UI", dpi);
        let body = make_font(13, 400, "Microsoft YaHei UI", dpi);
        let control = |class: &str,
                       label: &str,
                       (x, y, width, height): (i32, i32, i32, i32),
                       id: usize,
                       style,
                       font| {
            let child = CreateWindowExW(
                0,
                wide(class).as_ptr(),
                wide(label).as_ptr(),
                WS_CHILD | WS_VISIBLE | style,
                px(x),
                px(y),
                px(width),
                px(height),
                hwnd,
                id as HMENU,
                instance,
                null(),
            );
            SendMessageW(child, WM_SETFONT, font, 0);
            child
        };
        let buttons = WS_TABSTOP | BS_OWNERDRAW as u32;
        let close = localization::tr("ui.close");
        let caption_close = control(
            "BUTTON",
            &close,
            (WIDTH - 47, 0, 46, 42),
            CAPTION_CLOSE,
            BS_OWNERDRAW as u32,
            body as usize,
        );
        theme_ui::style_caption_close(caption_close);
        control(
            "STATIC",
            &localization::tr("ui.startup_hotkey_warning"),
            (64, 57, 332, 25),
            10,
            0,
            heading as usize,
        );
        let unavailable =
            localization::message("ui.startup_hotkey_message", &[("binding", binding)]);
        let help = localization::tr(if tray {
            "ui.startup_hotkey_help_tray"
        } else {
            "ui.startup_hotkey_help"
        });
        control(
            "STATIC",
            &format!("{unavailable}\r\n{help}"),
            (64, 89, 332, 49),
            11,
            0x80,
            body as usize,
        );
        control(
            "BUTTON",
            &close,
            (WIDTH - 216, 150, 78, 32),
            CLOSE,
            buttons,
            body as usize,
        );
        let change = control(
            "BUTTON",
            &localization::tr("ui.change_hotkey"),
            (WIDTH - 129, 150, 112, 32),
            CHANGE,
            buttons,
            body as usize,
        );
        EnableWindow(owner, 0);
        ShowWindow(hwnd, SW_SHOW);
        SetForegroundWindow(hwnd);
        SetFocus(change);
        let mut msg = MSG::default();
        while IsWindow(hwnd) != 0 {
            let result = GetMessageW(&mut msg, null_mut(), 0, 0);
            if result <= 0 {
                if result == 0 {
                    PostQuitMessage(msg.wParam as i32);
                }
                break;
            }
            if msg.message == WM_KEYDOWN && (msg.hwnd == hwnd || IsChild(hwnd, msg.hwnd) != 0) {
                if msg.wParam == VK_ESCAPE as usize {
                    SendMessageW(hwnd, WM_CLOSE, 0, 0);
                    continue;
                }
                if msg.wParam == VK_RETURN as usize {
                    let id = GetDlgCtrlID(GetFocus()) as usize;
                    SendMessageW(
                        hwnd,
                        WM_COMMAND,
                        if id == CLOSE { CLOSE } else { CHANGE },
                        0,
                    );
                    continue;
                }
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
        if !state.change && IsWindowVisible(owner) != 0 {
            SetForegroundWindow(owner);
            SetFocus(GetDlgItem(owner, 1001));
        }
        for font in [caption, heading, body] {
            DeleteObject(font);
        }
        UnregisterClassW(class.as_ptr(), instance);
        state.change
    }
}
