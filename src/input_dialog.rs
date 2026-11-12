use crate::{
    native::{make_font, scaled, ui, wide},
    theme, theme_ui,
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
            WM_CTLCOLORSTATIC | WM_CTLCOLOREDIT => unsafe {
                let (color, brush) = if msg == WM_CTLCOLOREDIT {
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
                theme_ui::button_on(item, item.CtlID == 1, theme::surface(), unsafe {
                    (*state).dpi
                });
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
    open(owner, title, None)
}

pub fn confirm(owner: HWND, title: &str, message: &str) -> bool {
    open(owner, title, Some(message)).is_some()
}

fn open(owner: HWND, title: &str, message: Option<&str>) -> Option<String> {
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
        }));
        let hwnd = CreateWindowExW(
            WS_EX_DLGMODALFRAME | WS_EX_CONTROLPARENT,
            class.as_ptr(),
            ui(title).as_ptr(),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU,
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
        let (height, content_height, action_y) = if long {
            (278, 203, 230)
        } else if message.is_some() {
            (135, 55, 88)
        } else {
            (110, 28, 66)
        };
        let dpi = crate::window_geometry::fit(hwnd, 420, height, None);
        (*state).dpi = dpi;
        let font = make_font(15, 400, "Microsoft YaHei UI", dpi);
        let content_style = if long {
            WS_TABSTOP | WS_VSCROLL | WS_BORDER | (ES_MULTILINE | ES_READONLY) as u32
        } else if message.is_none() {
            WS_BORDER | WS_TABSTOP | ES_AUTOHSCROLL as u32
        } else {
            0
        };
        let content = CreateWindowExW(
            0,
            wide(if message.is_some() && !long {
                "STATIC"
            } else {
                "EDIT"
            })
            .as_ptr(),
            ui(message.unwrap_or("")).as_ptr(),
            WS_CHILD | WS_VISIBLE | content_style,
            scaled(16, dpi),
            scaled(16, dpi),
            scaled(388, dpi),
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
        let mut focus = content;
        let actions = if title == "关闭设置" {
            [("放弃并关闭", 1, 156, 120), ("继续编辑", 2, 284, 120)]
        } else {
            [("确定", 1, 224, 84), ("取消", 2, 320, 84)]
        };
        for (text, id, x, width) in actions {
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
            if message.is_some() && id == 2 {
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
                SendMessageW(
                    hwnd,
                    WM_COMMAND,
                    if msg.wParam as u16 == VK_RETURN && GetDlgCtrlID(GetFocus()) != 2 {
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
