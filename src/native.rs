//! Deliberately small performance experiment, not the full launcher.
//! Persistent Win32 EDIT/LISTBOX and GDI share the original's thin rendering
//! path. Direct2D is not initialized merely to draw ten text rows: the hard
//! performance gate takes precedence over the earlier rendering preference.
use crate::theme;
use altrun_probe::{Entry, SearchIndex, advance, parse_entries};
use std::{cell::RefCell, mem::zeroed, ptr::null_mut};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::{Gdi::*, Dwm::*},
    System::LibraryLoader::GetModuleHandleW,
    UI::{Shell::*, Controls::*, HiDpi::*, Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
};

const ID_EDIT: usize = 1001;
const ID_LIST: usize = 1002;
const HOTKEY: i32 = 17001;
const SHOW: u32 = WM_APP + 1;
const QUIT: u32 = WM_APP + 2;
const LAUNCH_DONE: u32 = WM_APP + 3;
const TRAY: u32 = WM_APP + 4;
const ID_HIDE: usize = 18001;
const ID_EXIT: usize = 18002;
// SS_CENTER from WinUser.h; not projected by the selected windows-sys features.
const STATIC_CENTER: u32 = 0x00000000;

struct State {
    entries: Vec<Entry>,
    index: SearchIndex,
    hits: Vec<usize>,
    selected: usize,
    hwnd: HWND,
    edit: HWND,
    list: HWND,
    title: HWND,
    command: HWND,
    font: HFONT,
    title_font: HFONT,
    input_font: HFONT,
    command_font: HFONT,
    paper: HBRUSH,
    accent: HBRUSH,
    border: HPEN,
    empty: HWND,
    enabled: bool,
    parameter_entry: Option<usize>,
    saved_query: String,
    error: bool,
    launching: bool,
    launch_results: std::sync::mpsc::Receiver<Result<(), String>>,
    background: HBRUSH,
    selected_brush: HBRUSH,
    old_edit: isize,
    dpi: i32,
    allow_launch: bool,
    composing: bool,
}

thread_local! { static STATE: RefCell<Option<State>> = const { RefCell::new(None) }; }

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}
fn scaled(value: i32, dpi: i32) -> i32 {
    value * dpi / 96
}

pub fn show_error(message: &str) {
    unsafe {
        MessageBoxW(
            null_mut(),
            wide(message).as_ptr(),
            wide("ALTRun").as_ptr(),
            MB_OK | MB_ICONERROR,
        );
    }
}

fn selected_text() -> (HWND, HWND, Vec<u16>, Vec<u16>) {
    STATE.with(|cell| {
        let s = cell.borrow();
        let s = s.as_ref().unwrap();
        let e = s.hits.get(s.selected).map(|&i| &s.entries[i]);
        (
            s.title,
            s.command,
            wide(e.map(|e| e.name.as_str()).unwrap_or("没有匹配项")),
            wide(&e.map(|e| format!("CMD  {}", e.command)).unwrap_or_default()),
        )
    })
}

fn update_selection() {
    STATE.with(|c| c.borrow_mut().as_mut().unwrap().error = false);
    let (title, command, name, line) = selected_text();
    unsafe {
        SetWindowTextW(title, name.as_ptr());
        SetWindowTextW(command, line.as_ptr());
        InvalidateRect(command, std::ptr::null(), 1);
    }
}

fn refresh() {
    if STATE.with(|c| c.borrow().as_ref().unwrap().parameter_entry.is_some()) { return; }
    let edit = STATE.with(|c| c.borrow().as_ref().unwrap().edit);
    let query = unsafe {
        let mut buffer = vec![0_u16; GetWindowTextLengthW(edit) as usize + 1];
        let len = GetWindowTextW(edit, buffer.as_mut_ptr(), buffer.len() as i32);
        String::from_utf16_lossy(&buffer[..len as usize])
    };
    // Release RefCell before calling native controls, which may synchronously
    // reenter WM_DRAWITEM. Never keep a mutable state borrow across SendMessage.
    let (hwnd, list, command, dpi, count) = STATE.with(|c| {
        let mut s = c.borrow_mut();
        let s = s.as_mut().unwrap();
        s.hits = s.index.query(&query);
        s.selected = 0;
        (s.hwnd, s.list, s.command, s.dpi, s.hits.len().min(10))
    });
    unsafe {
        SendMessageW(list, WM_SETREDRAW, 0, 0);
        SendMessageW(list, LB_SETCOUNT, count, 0);
        SendMessageW(list, LB_SETCURSEL, 0, 0);
        let height = scaled(count.max(1) as i32 * theme::ROW, dpi);
        SetWindowPos(
            list,
            null_mut(),
            scaled(8, dpi),
            scaled(theme::LIST_Y, dpi),
            scaled(404, dpi),
            height,
            SWP_NOZORDER | SWP_NOACTIVATE,
        );
        SetWindowPos(
            command,
            null_mut(),
            scaled(12, dpi),
            scaled(73, dpi) + height,
            scaled(396, dpi),
            scaled(16, dpi),
            SWP_NOZORDER | SWP_NOACTIVATE,
        );
        SetWindowPos(
            hwnd,
            null_mut(),
            0,
            0,
            scaled(420, dpi),
            scaled(theme::BASE_HEIGHT, dpi) + height,
            SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
        );
        SendMessageW(list, WM_SETREDRAW, 1, 0);
        InvalidateRect(list, std::ptr::null(), 0);
        let empty = STATE.with(|c| c.borrow().as_ref().unwrap().empty);
        ShowWindow(empty, if count == 0 { SW_SHOWNOACTIVATE } else { SW_HIDE });
        InvalidateRect(hwnd, std::ptr::null(), 0);
    }
    update_selection();
}

fn move_selection(delta: isize) {
    if STATE.with(|c| c.borrow().as_ref().unwrap().parameter_entry.is_some()) { return; }
    let (list, selected) = STATE.with(|c| {
        let mut s = c.borrow_mut();
        let s = s.as_mut().unwrap();
        s.selected = advance(s.selected, s.hits.len().min(10), delta);
        (s.list, s.selected)
    });
    unsafe {
        SendMessageW(list, LB_SETCURSEL, selected, 0);
    }
    update_selection();
}

thread_local! { static LAUNCH_SENDER: RefCell<Option<std::sync::mpsc::Sender<Result<(), String>>>> = const { RefCell::new(None) }; }

fn edit_text(hwnd: HWND) -> String {
    unsafe {
        let mut text = vec![0; GetWindowTextLengthW(hwnd) as usize + 1];
        let n = GetWindowTextW(hwnd, text.as_mut_ptr(), text.len() as i32);
        String::from_utf16_lossy(&text[..n as usize])
    }
}
fn show_inline_error(message: &str) {
    let (hwnd, edit, command) = STATE.with(|c| {
        let mut s = c.borrow_mut(); let s = s.as_mut().unwrap(); s.error = true;
        (s.hwnd, s.edit, s.command)
    });
    unsafe {
        SetWindowTextW(command, wide(message).as_ptr());
        InvalidateRect(command, std::ptr::null(), 1);
        ShowWindow(hwnd, SW_SHOWNOACTIVATE); SetForegroundWindow(hwnd); SetFocus(edit);
    }
}
fn cancel_parameter() -> bool {
    let restore = STATE.with(|c| {
        let mut s = c.borrow_mut(); let s = s.as_mut().unwrap();
        s.parameter_entry.take().map(|_| (s.edit, s.saved_query.clone()))
    });
    if let Some((edit, query)) = restore {
        unsafe { SetWindowTextW(edit, wide(&query).as_ptr()); SendMessageW(edit, EM_SETCUEBANNER, 0, wide("输入快捷字").as_ptr() as LPARAM); }
        refresh(); true
    } else { false }
}
fn launch_selected() {
    let (enabled, allow, busy, selected, parameter_mode, hwnd, edit, title) = STATE.with(|c| {
        let s = c.borrow(); let s = s.as_ref().unwrap();
        (s.enabled, s.allow_launch, s.launching, s.parameter_entry.or_else(|| s.hits.get(s.selected).copied()), s.parameter_entry.is_some(), s.hwnd, s.edit, s.title)
    });
    if busy { return; }
    let Some(index) = selected else { return; };
    let entry = STATE.with(|c| c.borrow().as_ref().unwrap().entries[index].clone());
    if !enabled {
        if allow && entry.command == "notepad.exe" && let Ok(path) = altrun_probe::safe_notepad_path() { let _ = std::process::Command::new(path).spawn(); }
        unsafe { ShowWindow(hwnd, SW_HIDE); } return;
    }
    if entry.command.contains("%p") && !parameter_mode {
        let query = edit_text(edit);
        STATE.with(|c| { let mut s = c.borrow_mut(); let s = s.as_mut().unwrap(); s.saved_query = query; s.parameter_entry = Some(index); });
        unsafe {
            SetWindowTextW(title, wide(&format!("{} · 输入参数", entry.name)).as_ptr());
            SetWindowTextW(edit, wide("").as_ptr());
            SendMessageW(edit, EM_SETCUEBANNER, 0, wide("输入参数，Enter 启动，Esc 返回").as_ptr() as LPARAM);
        }
        return;
    }
    let parameter = if parameter_mode { Some(edit_text(edit)) } else { None };
    let result_sender = LAUNCH_SENDER.with(|s| s.borrow().as_ref().unwrap().clone());
    let command = entry.command;
    STATE.with(|c| c.borrow_mut().as_mut().unwrap().launching = true);
    unsafe { ShowWindow(hwnd, SW_HIDE); }
    let owner = hwnd as usize;
    // All path checks, environment expansion, COM initialization and shell work
    // happen off the resident UI thread. No implicit cmd.exe is introduced.
    std::thread::spawn(move || {
        let result = altrun_probe::launch::resolve(&command, parameter.as_deref()).and_then(|request| execute_request(&request));
        let _ = result_sender.send(result);
        unsafe { PostMessageW(owner as HWND, LAUNCH_DONE, 0, 0); }
    });
}
fn execute_request(request: &altrun_probe::launch::LaunchRequest) -> Result<(), String> {
    use windows_sys::Win32::System::Com::*;
    let file = wide(&request.file); let parameters = wide(&request.parameters);
    unsafe {
        let hr = CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED as u32);
        if hr < 0 { return Err(format!("启动服务初始化失败：0x{:08X}", hr as u32)); }
        let mut info: SHELLEXECUTEINFOW = zeroed();
        info.cbSize = std::mem::size_of_val(&info) as u32;
        info.fMask = SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI;
        // lpVerb must outlive ShellExecuteExW, just like file and parameters.
        let verb = wide("open"); info.lpVerb = verb.as_ptr();
        info.lpFile = file.as_ptr(); info.lpParameters = parameters.as_ptr(); info.nShow = SW_SHOWNORMAL;
        let ok = ShellExecuteExW(&mut info);
        let error = GetLastError(); CoUninitialize();
        if ok == 0 { Err(format!("无法启动（错误 {error}），请检查命令或文件路径")) } else { Ok(()) }
    }
}
fn make_font(size: i32, weight: i32, family: &str, dpi: i32) -> HFONT {
    unsafe { CreateFontW(-scaled(size, dpi), 0, 0, 0, weight, 0, 0, 0, DEFAULT_CHARSET as u32, OUT_DEFAULT_PRECIS as u32, CLIP_DEFAULT_PRECIS as u32, CLEARTYPE_QUALITY as u32, 0, wide(family).as_ptr()) }
}
fn paint_panel(hwnd: HWND) {
    unsafe {
        let mut ps: PAINTSTRUCT = zeroed(); let dc = BeginPaint(hwnd, &mut ps);
        STATE.with(|c| { let s = c.borrow(); let Some(s) = s.as_ref() else { return; };
            let old_brush = SelectObject(dc, s.paper); let old_pen = SelectObject(dc, s.border);
            RoundRect(dc, scaled(10,s.dpi), scaled(29,s.dpi), scaled(410,s.dpi), scaled(60,s.dpi), scaled(8,s.dpi), scaled(8,s.dpi));
            let mut rect: RECT = zeroed(); GetClientRect(hwnd, &mut rect);
            MoveToEx(dc, scaled(12,s.dpi), rect.bottom - scaled(25,s.dpi), std::ptr::null_mut()); LineTo(dc, rect.right - scaled(12,s.dpi), rect.bottom - scaled(25,s.dpi));
            SelectObject(dc, old_brush); SelectObject(dc, old_pen);
        }); EndPaint(hwnd, &ps);
    }
}
fn show_menu(hwnd: HWND) {
    unsafe {
        let menu = CreatePopupMenu(); if menu.is_null() { return; }
        AppendMenuW(menu, MF_STRING, ID_HIDE, wide("隐藏").as_ptr());
        AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
        AppendMenuW(menu, MF_STRING, ID_EXIT, wide("退出 ALTRun").as_ptr());
        let mut point: POINT = zeroed(); GetCursorPos(&mut point); SetForegroundWindow(hwnd);
        TrackPopupMenu(menu, TPM_RIGHTBUTTON, point.x, point.y, 0, hwnd, std::ptr::null());
        DestroyMenu(menu); PostMessageW(hwnd, WM_NULL, 0, 0);
    }
}
fn tray_icon(hwnd: HWND, operation: u32) {
    unsafe {
        let mut icon: NOTIFYICONDATAW = zeroed(); icon.cbSize = std::mem::size_of_val(&icon) as u32;
        icon.hWnd = hwnd; icon.uID = 1; icon.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP; icon.uCallbackMessage = TRAY;
        icon.hIcon = LoadIconW(std::ptr::null_mut(), IDI_APPLICATION);
        let tip = wide("ALTRun · Ctrl+Alt+R"); icon.szTip[..tip.len()].copy_from_slice(&tip);
        Shell_NotifyIconW(operation, &icon);
    }
}

unsafe extern "system" fn edit_proc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    if msg == WM_IME_STARTCOMPOSITION {
        STATE.with(|c| c.borrow_mut().as_mut().unwrap().composing = true);
    }
    if msg == WM_IME_ENDCOMPOSITION {
        STATE.with(|c| c.borrow_mut().as_mut().unwrap().composing = false);
    }
    let composing = STATE.with(|c| c.borrow().as_ref().unwrap().composing);
    if msg == WM_KEYDOWN && !composing {
        match w as u16 {
            VK_DOWN => {
                move_selection(1);
                return 0;
            }
            VK_UP => {
                move_selection(-1);
                return 0;
            }
            VK_TAB => {
                move_selection(if unsafe { GetKeyState(VK_SHIFT as i32) } < 0 {
                    -1
                } else {
                    1
                });
                return 0;
            }
            VK_RETURN => {
                if unsafe { GetKeyState(VK_CONTROL as i32) } < 0 && unsafe { GetKeyState(VK_SHIFT as i32) } < 0 {
                    let parent = unsafe { GetParent(hwnd) }; unsafe { PostMessageW(parent, QUIT, 0, 0); }
                } else { launch_selected(); }
                return 0;
            }
            VK_ESCAPE => {
                if cancel_parameter() { return 0; }
                if unsafe { GetWindowTextLengthW(hwnd) } > 0 {
                    unsafe {
                        SetWindowTextW(hwnd, wide("").as_ptr());
                    }
                } else {
                    let parent = unsafe { GetParent(hwnd) };
                    unsafe {
                        ShowWindow(parent, SW_HIDE);
                    }
                }
                return 0;
            }
            _ => {}
        }
    }
    if msg == WM_CHAR && matches!(w as u16, VK_RETURN | VK_ESCAPE | VK_TAB) && !composing {
        return 0;
    }
    let old = STATE.with(|c| c.borrow().as_ref().unwrap().old_edit);
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

unsafe extern "system" fn window_proc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    match msg {
        WM_HOTKEY | SHOW => {
            let edit = STATE.with(|c| c.borrow().as_ref().unwrap().edit);
            unsafe {
                if msg == WM_HOTKEY && IsWindowVisible(hwnd) != 0 {
                    ShowWindow(hwnd, SW_HIDE);
                } else {
                    ShowWindow(hwnd, SW_SHOWNOACTIVATE);
                    SetForegroundWindow(hwnd);
                    SetFocus(edit);
                }
            }
            0
        }
        WM_CONTEXTMENU => { show_menu(hwnd); 0 }
        TRAY => {
            if l as u32 == WM_RBUTTONUP { show_menu(hwnd); }
            else if l as u32 == WM_LBUTTONUP { unsafe { SendMessageW(hwnd, SHOW, 0, 0); } }
            0
        }
        LAUNCH_DONE => {
            let result = STATE.with(|c| c.borrow().as_ref().unwrap().launch_results.try_recv().ok());
            STATE.with(|c| c.borrow_mut().as_mut().unwrap().launching = false);
            match result { Some(Err(message)) => show_inline_error(&message), Some(Ok(())) => { cancel_parameter(); }, None => {} }
            0
        }
        WM_PAINT => { paint_panel(hwnd); 0 }
        WM_COMMAND => {
            if w & 0xffff == ID_EXIT { unsafe { PostMessageW(hwnd, QUIT, 0, 0); } return 0; }
            if w & 0xffff == ID_HIDE { unsafe { ShowWindow(hwnd, SW_HIDE); } return 0; }
            let id = w & 0xffff;
            let code = (w >> 16) & 0xffff;
            if id == ID_EDIT && code == EN_CHANGE as usize {
                refresh();
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
        WM_DRAWITEM => {
            let d = unsafe { &*(l as *const DRAWITEMSTRUCT) };
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
                    FillRect(
                        d.hDC,
                        &d.rcItem,
                        if d.itemState & ODS_SELECTED != 0 {
                            s.selected_brush
                        } else {
                            s.paper
                        },
                    );
                    if d.itemState & ODS_SELECTED != 0 {
                        let rail = RECT { left: 0, top: d.rcItem.top + scaled(3, s.dpi), right: scaled(2, s.dpi), bottom: d.rcItem.bottom - scaled(3, s.dpi) };
                        FillRect(d.hDC, &rail, s.accent);
                    }
                    SetBkMode(d.hDC, TRANSPARENT as i32);
                    SetTextColor(d.hDC, theme::MUTED);
                    SelectObject(d.hDC, s.font);
                    let number = format!(
                        "{}{}",
                        if e.command.contains("%p") { '*' } else { ' ' },
                        (d.itemID + 1) % 10
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
                    SetTextColor(d.hDC, theme::TEXT);
                    SelectObject(d.hDC, s.title_font);
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
                }
            });
            1
        }
        WM_CTLCOLORSTATIC | WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX => {
            STATE.with(|c| {
                let state = c.borrow(); let Some(s) = state.as_ref() else { return 0; };
                let dc = w as HDC;
                let paper = l as HWND == s.edit || l as HWND == s.list;
                unsafe {
                    SetTextColor(dc, if l as HWND == s.command && s.error { theme::ERROR } else if l as HWND == s.command || l as HWND == s.empty { theme::MUTED } else { theme::TEXT });
                    SetBkColor(dc, if paper { theme::PAPER } else { theme::SURFACE });
                }
                if paper { s.paper as LRESULT } else { s.background as LRESULT }
            })
        }
        WM_CLOSE => {
            unsafe {
                ShowWindow(hwnd, SW_HIDE);
            }
            0
        }
        QUIT => {
            unsafe {
                DestroyWindow(hwnd);
            }
            0
        }
        WM_DESTROY => {
            tray_icon(hwnd, NIM_DELETE);
            unsafe {
                UnregisterHotKey(hwnd, HOTKEY);
                PostQuitMessage(0);
            }
            0
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, w, l) },
    }
}

pub fn run() -> Result<(), String> {
    let args: Vec<_> = std::env::args().collect();
    let input = if let Some(i) = args.iter().position(|a| a == "--data") {
        let p = args.get(i + 1).ok_or("--data needs a UTF-8 TSV path")?;
        std::fs::read_to_string(p).map_err(|e| format!("Cannot read private dataset: {e}"))?
    } else {
        "notepad\t记事本\tnotepad.exe\t7\nCalc\t计算器\tcalc\t5\nExplorer\t资源管理器\texplorer.exe\t4\ncmd\t命令提示符\tcmd /k %p\t6\nb\t百度搜索\thttps://www.baidu.com/s?wd=%p\t12\ng\tGoogle 搜索\thttps://www.google.com/search?q=%p\t14\n".into()
    };
    let entries = parse_entries(&input)?;
    let index = SearchIndex::new(&entries);
    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let instance = GetModuleHandleW(std::ptr::null());
        let class = wide("ALTRun.Native.PerformanceProbe");
        let background = CreateSolidBrush(theme::SURFACE);
        let selected_brush = CreateSolidBrush(theme::SELECTION);
        let paper = CreateSolidBrush(theme::PAPER);
        let accent = CreateSolidBrush(theme::ACCENT);
        let border = CreatePen(PS_SOLID, 1, theme::BORDER);
        let mut wc: WNDCLASSW = zeroed();
        wc.lpfnWndProc = Some(window_proc);
        wc.hInstance = instance;
        wc.lpszClassName = class.as_ptr();
        wc.hbrBackground = background;
        wc.hCursor = LoadCursorW(null_mut(), IDC_ARROW);
        if RegisterClassW(&wc) == 0 {
            return Err("Cannot register native window".into());
        }
        let hwnd = CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
            class.as_ptr(),
            wide("ALTRun").as_ptr(),
            WS_POPUP | WS_BORDER | WS_CLIPCHILDREN,
            400,
            220,
            420,
            280,
            null_mut(),
            null_mut(),
            instance,
            std::ptr::null(),
        );
        if hwnd.is_null() {
            return Err("Cannot create native window".into());
        }
        let dpi = if let Some(i) = args.iter().position(|a| a == "--ui-scale") {
            match args.get(i + 1).map(String::as_str) { Some("150") => 144, Some("100") => 96, _ => return Err("--ui-scale accepts 100 or 150".into()) }
        } else { GetDpiForWindow(hwnd).max(96) as i32 };
        let rounded = 2_i32;
        DwmSetWindowAttribute(hwnd, 33, &rounded as *const _ as *const _, 4);
        DwmSetWindowAttribute(hwnd, 34, &theme::BORDER as *const _ as *const _, 4);
        let font = CreateFontW(
            -scaled(13, dpi),
            0,
            0,
            0,
            400,
            0,
            0,
            0,
            DEFAULT_CHARSET as u32,
            OUT_DEFAULT_PRECIS as u32,
            CLIP_DEFAULT_PRECIS as u32,
            CLEARTYPE_QUALITY as u32,
            0,
            wide("Microsoft YaHei UI").as_ptr(),
        );
        let title_font = make_font(13, 600, "Microsoft YaHei UI", dpi);
        let input_font = make_font(16, 400, "Microsoft YaHei UI", dpi);
        let command_font = make_font(11, 400, "Consolas", dpi);
        let control = |class_name: &str, style: u32, x, y, width, height, id: usize| {
            CreateWindowExW(
                0,
                wide(class_name).as_ptr(),
                wide("").as_ptr(),
                WS_CHILD | WS_VISIBLE | style,
                scaled(x, dpi),
                scaled(y, dpi),
                scaled(width, dpi),
                scaled(height, dpi),
                hwnd,
                id as HMENU,
                instance,
                std::ptr::null(),
            )
        };
        let title = control("STATIC", STATIC_CENTER, 12, 6, 396, 20, 1000);
        let edit = control(
            "EDIT",
            ES_AUTOHSCROLL as u32,
            18,
            35,
            384,
            22,
            ID_EDIT,
        );
        let list = control(
            "LISTBOX",
            LBS_NOTIFY as u32
                | LBS_OWNERDRAWFIXED as u32
                | LBS_NODATA as u32
                | LBS_NOINTEGRALHEIGHT as u32,
            8,
            theme::LIST_Y,
            404,
            120,
            ID_LIST,
        );
        let command = control(
            "EDIT",
            ES_AUTOHSCROLL as u32 | ES_READONLY as u32,
            12,
            178,
            396,
            16,
            1003,
        );
        let empty = control("STATIC", 1, 12, theme::LIST_Y + 2, 396, 20, 1004);
        SetWindowTextW(empty, wide("没有匹配项，换个快捷字试试").as_ptr());
        if [title, edit, list, command, empty].iter().any(|h| h.is_null()) {
            return Err("Cannot create native controls".into());
        }
        for child in [title, edit, list, command, empty] {
            SendMessageW(child, WM_SETFONT, font as WPARAM, 0);
        }
        SendMessageW(title, WM_SETFONT, title_font as WPARAM, 0);
        SendMessageW(edit, WM_SETFONT, input_font as WPARAM, 0);
        SendMessageW(edit, EM_SETCUEBANNER, 0, wide("输入快捷字").as_ptr() as LPARAM);
        SendMessageW(command, WM_SETFONT, command_font as WPARAM, 0);
        SendMessageW(list, LB_SETITEMHEIGHT, 0, scaled(theme::ROW, dpi) as LPARAM);
        let (launch_sender, launch_results) = std::sync::mpsc::channel();
        LAUNCH_SENDER.with(|s| *s.borrow_mut() = Some(launch_sender));
        let old_edit = SetWindowLongPtrW(edit, GWLP_WNDPROC, edit_proc as *const () as isize);
        STATE.with(|c| {
            *c.borrow_mut() = Some(State {
                entries,
                index,
                hits: Vec::new(),
                selected: 0,
                hwnd,
                edit,
                list,
                title,
                command,
                font,
                title_font, input_font, command_font, paper, accent, border, empty,
                enabled: args.iter().any(|a| a == "--enable-launch"),
                parameter_entry: None, saved_query: String::new(), error: false, launching: false, launch_results,
                background,
                selected_brush,
                old_edit,
                dpi,
                allow_launch: args.iter().any(|a| a == "--allow-safe-notepad"),
                composing: false,
            })
        });
        refresh();
        tray_icon(hwnd, NIM_ADD);
        // Deliberately separate from the original's Alt+R / Pause binding.
        if RegisterHotKey(
            hwnd,
            HOTKEY,
            MOD_CONTROL | MOD_ALT | MOD_NOREPEAT,
            b'R' as u32,
        ) == 0
        {
            DestroyWindow(hwnd);
            return Err("Ctrl+Alt+R is in use; original hotkeys were not changed.".into());
        }
        if args.iter().any(|a| a == "--show") {
            SendMessageW(hwnd, SHOW, 0, 0);
        }
        let mut message: MSG = zeroed();
        loop {
            let result = GetMessageW(&mut message, null_mut(), 0, 0);
            if result <= 0 {
                break;
            }
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
        STATE.with(|c| {
            if let Some(s) = c.borrow_mut().take() {
                for font in [s.font, s.title_font, s.input_font, s.command_font] { DeleteObject(font); }
                DeleteObject(s.paper); DeleteObject(s.accent); DeleteObject(s.border);
                DeleteObject(s.selected_brush);
                DeleteObject(s.background);
            }
        });
        UnregisterClassW(class.as_ptr(), instance);
    }
    Ok(())
}
