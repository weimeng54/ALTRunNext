use crate::native::{make_font, scaled, ui, wide};
use std::ptr::null_mut;
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::LibraryLoader::GetModuleHandleW,
    UI::{Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
};
struct Input {
    done: bool,
    value: Option<String>,
    edit: HWND,
    old_edit: isize,
    composing: bool,
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
    unsafe {
        let instance = GetModuleHandleW(std::ptr::null());
        let class = wide("ALTRun.Native.ParameterDialog");
        let mut wc: WNDCLASSW = std::mem::zeroed();
        wc.lpfnWndProc = Some(procedure);
        wc.hInstance = instance;
        wc.lpszClassName = class.as_ptr();
        wc.hbrBackground = (COLOR_WINDOW + 1) as HBRUSH;
        wc.hCursor = LoadCursorW(null_mut(), IDC_ARROW);
        RegisterClassW(&wc);
        let state = Box::into_raw(Box::new(Input {
            done: false,
            value: None,
            edit: null_mut(),
            old_edit: 0,
            composing: false,
        }));
        let hwnd = CreateWindowExW(
            WS_EX_DLGMODALFRAME | WS_EX_CONTROLPARENT,
            class.as_ptr(),
            wide(title).as_ptr(),
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
            drop(Box::from_raw(state));
            return None;
        }
        let dpi = crate::window_geometry::fit(hwnd, 420, 110, None);
        let font = make_font(14, 400, "Microsoft YaHei UI", dpi);
        (*state).edit = CreateWindowExW(
            0,
            wide("EDIT").as_ptr(),
            wide("").as_ptr(),
            WS_CHILD | WS_VISIBLE | WS_BORDER | WS_TABSTOP | ES_AUTOHSCROLL as u32,
            scaled(16, dpi),
            scaled(16, dpi),
            scaled(388, dpi),
            scaled(28, dpi),
            hwnd,
            10usize as HMENU,
            instance,
            std::ptr::null(),
        );
        if (*state).edit.is_null() {
            DestroyWindow(hwnd);
            DeleteObject(font);
            UnregisterClassW(class.as_ptr(), instance);
            drop(Box::from_raw(state));
            return None;
        }
        SetWindowLongPtrW((*state).edit, GWLP_USERDATA, state as isize);
        (*state).old_edit =
            SetWindowLongPtrW((*state).edit, GWLP_WNDPROC, edit_procedure as *const () as isize);
        SendMessageW((*state).edit, WM_SETFONT, font as usize, 0);
        for (text, id, x) in [("确定", 1, 224), ("取消", 2, 320)] {
            let button = CreateWindowExW(
                0,
                wide("BUTTON").as_ptr(),
                ui(text).as_ptr(),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                scaled(x, dpi),
                scaled(66, dpi),
                scaled(84, dpi),
                scaled(28, dpi),
                hwnd,
                id as HMENU,
                instance,
                std::ptr::null(),
            );
            SendMessageW(button, WM_SETFONT, font as usize, 0);
        }
        EnableWindow(owner, 0);
        ShowWindow(hwnd, SW_SHOW);
        SetFocus((*state).edit);
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
        Box::from_raw(state).value.take()
    }
}
