//! Deliberately small performance experiment, not the full launcher.
//! Persistent Win32 EDIT/LISTBOX and GDI share the original's thin rendering
//! path. Direct2D is not initialized merely to draw ten text rows: the hard
//! performance gate takes precedence over the earlier rendering preference.
use crate::theme;
use altrun_probe::{Entry, SearchIndex, advance, parse_entries};
use std::{cell::RefCell, mem::zeroed, ptr::null_mut};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::{Dwm::*, Gdi::*},
    System::LibraryLoader::GetModuleHandleW,
    UI::{Controls::*, HiDpi::*, Input::KeyboardAndMouse::*, Shell::*, WindowsAndMessaging::*},
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
const ID_MANAGE: usize = 18003;
const ID_RELOAD: usize = 18004;
const ID_SETTINGS:usize=18005;
const ID_RECENT:usize=18006;
const ID_ABOUT:usize=18007;
const MATCH_DONE:u32=WM_APP+13;
const AUTO_HIDE:usize=17030;
// SS_CENTER from WinUser.h; not projected by the selected windows-sys features.
const STATIC_CENTER: u32 = 0x00000000;

struct State {
    preferences:altrun_probe::preferences::Preferences,
    usage:altrun_probe::usage::Usage,
    usage_writer:altrun_probe::state_store::Writer,
    settings_sender:std::sync::mpsc::Sender<crate::settings::Change>,
    settings_updates:std::sync::mpsc::Receiver<crate::settings::Change>,
    hotkeys:Vec<(i32,Option<altrun_probe::preferences::Hotkey>)>,
    pending_settings:Option<(u64,altrun_probe::preferences::Preferences,Vec<(i32,Option<altrun_probe::preferences::Hotkey>)>)>,
    generation:u64,
    match_sender:std::sync::mpsc::Sender<(u64,String,Vec<String>,bool)>,
    match_results:std::sync::mpsc::Receiver<(u64,Result<Vec<Option<usize>>,String>)>,
    current_query:String,
    matching:bool,
    last_key:u16,
    context:altrun_probe::launch::LaunchContext,
    successful:Option<(String,String,Option<String>)>,
    history_position:usize,
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
    data_path: std::path::PathBuf,
    library_sender: altrun_probe::library_updates::LibraryWriter,
    library_updates: altrun_probe::library_updates::LibraryMailbox,
}

thread_local! { static STATE: RefCell<Option<State>> = const { RefCell::new(None) }; }

pub(crate) fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}
pub(crate) fn scaled(value: i32, dpi: i32) -> i32 {
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
    if STATE.with(|c| c.borrow().as_ref().unwrap().parameter_entry.is_some()) {
        return;
    }
    let edit = STATE.with(|c| c.borrow().as_ref().unwrap().edit);
    let query = unsafe {
        let mut buffer = vec![0_u16; GetWindowTextLengthW(edit) as usize + 1];
        let len = GetWindowTextW(edit, buffer.as_mut_ptr(), buffer.len() as i32);
        String::from_utf16_lossy(&buffer[..len as usize])
    };
    // Release RefCell before calling native controls, which may synchronously
    // reenter WM_DRAWITEM. Never keep a mutable state borrow across SendMessage.
    STATE.with(|c| {
        let mut s = c.borrow_mut();
        let s = s.as_mut().unwrap();
        s.generation=s.generation.wrapping_add(1);s.current_query=query.clone();s.matching=false;
        s.hits=match s.index.query_options(&query,s.preferences.flag("Regex"),s.preferences.flag("MatchAnywhere"),s.preferences.flag("StandardRegex")){
            Ok(hits)=>hits,
            Err(e) if e=="advanced"=>{let pattern=if s.preferences.flag("StandardRegex"){query.clone()}else{altrun_probe::matching::prepare_legacy(&query)};let _=s.match_sender.send((s.generation,pattern,s.index.aliases().to_vec(),!s.preferences.flag("MatchAnywhere")));s.matching=true;Vec::new()},
            Err(_)=>Vec::new()
        };
        if s.preferences.flag("RememberFavouratMatch")&&let Some(id)=s.usage.favourites.get(&query)&&let Some(pos)=s.hits.iter().position(|i|&s.entries[*i].id==id){let hit=s.hits.remove(pos);s.hits.insert(0,hit);}
        s.selected = 0;

    });
    render_hits();
}

fn render_hits(){
    let (hwnd,list,command,dpi,count)=STATE.with(|c|{let s=c.borrow();let s=s.as_ref().unwrap();(s.hwnd,s.list,s.command,s.dpi,visible_count(s))});
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
        ShowWindow(
            empty,
            if count == 0 {
                SW_SHOWNOACTIVATE
            } else {
                SW_HIDE
            },
        );
        InvalidateRect(hwnd, std::ptr::null(), 0);
    }
    update_selection();
}

fn move_selection(delta: isize) {
    if STATE.with(|c| c.borrow().as_ref().unwrap().parameter_entry.is_some()) {
        return;
    }
    let (list, selected) = STATE.with(|c| {
        let mut s = c.borrow_mut();
        let s = s.as_mut().unwrap();
        s.selected = advance(s.selected, visible_count(s), delta);
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
        let mut s = c.borrow_mut();
        let s = s.as_mut().unwrap();
        s.error = true;
        (s.hwnd, s.edit, s.command)
    });
    unsafe {
        SetWindowTextW(command, wide(message).as_ptr());
        InvalidateRect(command, std::ptr::null(), 1);
        ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        SetForegroundWindow(hwnd);
        SetFocus(edit);
    }
}
fn cancel_parameter() -> bool {
    let restore = STATE.with(|c| {
        let mut s = c.borrow_mut();
        let s = s.as_mut().unwrap();
        s.parameter_entry
            .take()
            .map(|_| (s.edit, s.list, s.saved_query.clone()))
    });
    if let Some((edit, list, query)) = restore {
        unsafe {
            EnableWindow(list, 1);
            SetWindowTextW(edit, wide(&query).as_ptr());
            SendMessageW(
                edit,
                EM_SETCUEBANNER,
                0,
                wide("输入快捷字").as_ptr() as LPARAM,
            );
        }
        refresh();
        true
    } else {
        false
    }
}
fn launch_selected() {
    let (enabled, allow, busy, selected, parameter_mode, hwnd, edit, title) = STATE.with(|c| {
        let s = c.borrow();
        let s = s.as_ref().unwrap();
        (
            s.enabled,
            s.allow_launch,
            s.launching,
            s.parameter_entry
                .or_else(|| s.hits.get(s.selected).copied()),
            s.parameter_entry.is_some(),
            s.hwnd,
            s.edit,
            s.title,
        )
    });
    if busy {
        return;
    }
    let Some(index) = selected else {
        return;
    };
    let entry = STATE.with(|c| c.borrow().as_ref().unwrap().entries[index].clone());
    if !enabled {
        if allow
            && entry.command == "notepad.exe"
            && let Ok(path) = altrun_probe::safe_notepad_path()
        {
            let _ = std::process::Command::new(path).spawn();
        }
        unsafe {
            ShowWindow(hwnd, SW_HIDE);
        }
        return;
    }
    if entry.param_type.needs_input(&entry.command) && !parameter_mode && (altrun_probe::launch::has_parameter(&entry.command)||!["%c","%wd","%wt","%wc"].iter().any(|t|entry.command.contains(t))) {
        let query = edit_text(edit);
        let list = STATE.with(|c| c.borrow().as_ref().unwrap().list);
        STATE.with(|c| {
            let mut s = c.borrow_mut();
            let s = s.as_mut().unwrap();
            s.saved_query = query;
            s.parameter_entry = Some(index);
        });
        unsafe {
            EnableWindow(list, 0);
            SetWindowTextW(title, wide(&format!("{} · 输入参数", entry.name)).as_ptr());
            SetWindowTextW(edit, wide("").as_ptr());
            SendMessageW(
                edit,
                EM_SETCUEBANNER,
                0,
                wide("输入参数，Enter 启动，Esc 返回").as_ptr() as LPARAM,
            );
        }
        return;
    }
    let parameter = if parameter_mode {
        Some(edit_text(edit))
    } else {
        None
    };
    let result_sender = LAUNCH_SENDER.with(|s| s.borrow().as_ref().unwrap().clone());
    let param_type = entry.param_type;
    let query=if parameter_mode{STATE.with(|c|c.borrow().as_ref().unwrap().saved_query.clone())}else{edit_text(edit)};
    let mut context=STATE.with(|c|c.borrow().as_ref().unwrap().context.clone());
    let working_directory=entry.working_directory;
    STATE.with(|c|c.borrow_mut().as_mut().unwrap().successful=Some((entry.id,query,parameter.clone())));
    let command = entry.command;
    STATE.with(|c| c.borrow_mut().as_mut().unwrap().launching = true);
    unsafe {
        ShowWindow(hwnd, SW_HIDE);
    }
    let owner = hwnd as usize;
    // All path checks, environment expansion, COM initialization and shell work
    // happen off the resident UI thread. No implicit cmd.exe is introduced.
    std::thread::spawn(move || {
        if command.contains("%c"){context.clipboard=clipboard_text();}
        let result =altrun_probe::launch::resolve_context(&command,parameter.as_deref(),param_type,&working_directory,&context).and_then(|(request,mode,directory)|execute_request_with(&request,mode,directory.as_deref()));
        let _ = result_sender.send(result);
        unsafe {
            PostMessageW(owner as HWND, LAUNCH_DONE, 0, 0);
        }
    });
}
fn execute_request(request: &altrun_probe::launch::LaunchRequest) -> Result<(), String> {
    execute_request_with(request,altrun_probe::launch::ShowMode::Normal,None)
}
fn execute_request_with(request:&altrun_probe::launch::LaunchRequest,mode:altrun_probe::launch::ShowMode,directory:Option<&str>)->Result<(),String>{
    use windows_sys::Win32::System::Com::*;
    let file = wide(&request.file);
    let parameters = wide(&request.parameters);
    let directory=directory.map(wide);
    unsafe {
        let hr = CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED as u32);
        if hr < 0 {
            return Err(format!("启动服务初始化失败：0x{:08X}", hr as u32));
        }
        let mut info: SHELLEXECUTEINFOW = zeroed();
        info.cbSize = std::mem::size_of_val(&info) as u32;
        info.fMask = SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI;
        // lpVerb must outlive ShellExecuteExW, just like file and parameters.
        let verb = wide("open");
        info.lpVerb = verb.as_ptr();
        info.lpFile = file.as_ptr();
        info.lpParameters = parameters.as_ptr();
        info.lpDirectory=directory.as_ref().map_or(std::ptr::null(),|d|d.as_ptr());
        info.nShow =match mode{altrun_probe::launch::ShowMode::Normal=>SW_SHOWNORMAL,altrun_probe::launch::ShowMode::Maximized=>SW_SHOWMAXIMIZED,altrun_probe::launch::ShowMode::Minimized=>SW_SHOWMINNOACTIVE,altrun_probe::launch::ShowMode::Hidden=>SW_HIDE};
        let ok = ShellExecuteExW(&mut info);
        let error = GetLastError();
        CoUninitialize();
        if ok == 0 {
            Err(format!("无法启动（错误 {error}），请检查命令或文件路径"))
        } else {
            Ok(())
        }
    }
}
pub(crate) fn make_font(size: i32, weight: i32, family: &str, dpi: i32) -> HFONT {
    unsafe {
        CreateFontW(
            -scaled(size, dpi),
            0,
            0,
            0,
            weight,
            0,
            0,
            0,
            DEFAULT_CHARSET as u32,
            OUT_DEFAULT_PRECIS as u32,
            CLIP_DEFAULT_PRECIS as u32,
            CLEARTYPE_QUALITY as u32,
            0,
            wide(family).as_ptr(),
        )
    }
}
fn paint_panel(hwnd: HWND) {
    unsafe {
        let mut ps: PAINTSTRUCT = zeroed();
        let dc = BeginPaint(hwnd, &mut ps);
        STATE.with(|c| {
            let s = c.borrow();
            let Some(s) = s.as_ref() else {
                return;
            };
            let old_brush = SelectObject(dc, s.paper);
            let old_pen = SelectObject(dc, s.border);
            RoundRect(
                dc,
                scaled(10, s.dpi),
                scaled(29, s.dpi),
                scaled(410, s.dpi),
                scaled(60, s.dpi),
                scaled(8, s.dpi),
                scaled(8, s.dpi),
            );
            let mut rect: RECT = zeroed();
            GetClientRect(hwnd, &mut rect);
            MoveToEx(
                dc,
                scaled(12, s.dpi),
                rect.bottom - scaled(25, s.dpi),
                std::ptr::null_mut(),
            );
            LineTo(
                dc,
                rect.right - scaled(12, s.dpi),
                rect.bottom - scaled(25, s.dpi),
            );
            SelectObject(dc, old_brush);
            SelectObject(dc, old_pen);
        });
        EndPaint(hwnd, &ps);
    }
}
fn show_menu(hwnd: HWND) {
    unsafe {
        let menu = CreatePopupMenu();
        if menu.is_null() {
            return;
        }
        AppendMenuW(
            menu,
            MF_STRING,
            ID_MANAGE,
            wide("管理快捷项\tAlt+S").as_ptr(),
        );
        AppendMenuW(menu, MF_STRING, ID_RELOAD, wide("重新载入快捷项").as_ptr());
        AppendMenuW(menu, MF_STRING, ID_SETTINGS, wide("设置…").as_ptr());
        AppendMenuW(menu, MF_STRING, ID_RECENT, wide("最近启动\tCtrl+L").as_ptr());
        AppendMenuW(menu, MF_STRING, ID_ABOUT, wide("帮助和关于\tF1").as_ptr());
        AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
        AppendMenuW(menu, MF_STRING, ID_HIDE, wide("隐藏").as_ptr());
        AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
        AppendMenuW(menu, MF_STRING, ID_EXIT, wide("退出 ALTRun").as_ptr());
        let mut point: POINT = zeroed();
        GetCursorPos(&mut point);
        SetForegroundWindow(hwnd);
        TrackPopupMenu(
            menu,
            TPM_RIGHTBUTTON,
            point.x,
            point.y,
            0,
            hwnd,
            std::ptr::null(),
        );
        DestroyMenu(menu);
        PostMessageW(hwnd, WM_NULL, 0, 0);
    }
}
fn tray_icon(hwnd: HWND, operation: u32) {
    unsafe {
        let mut icon: NOTIFYICONDATAW = zeroed();
        icon.cbSize = std::mem::size_of_val(&icon) as u32;
        icon.hWnd = hwnd;
        icon.uID = 1;
        icon.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
        icon.uCallbackMessage = TRAY;
        icon.hIcon = LoadIconW(std::ptr::null_mut(), IDI_APPLICATION);
        let tip = wide("ALTRun · Ctrl+Alt+R");
        icon.szTip[..tip.len()].copy_from_slice(&tip);
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
        let control=unsafe{GetKeyState(VK_CONTROL as i32)}<0;let alt=unsafe{GetKeyState(VK_MENU as i32)}<0;
        STATE.with(|c|c.borrow_mut().as_mut().unwrap().last_key=w as u16);
        if control||alt{let digit=match w as u16{0x30..=0x39=>Some(w-0x30),0x60..=0x69=>Some(w-0x60),_=>None};if let Some(digit)=digit{choose_number(digit);return 0;}}
        if control{match w as u16{0x43=>{selected_action(true);return 0;},0x44=>{selected_action(false);return 0;},0x4c=>{recent();return 0;},VK_UP|VK_DOWN if parameter_history(if w as u16==VK_UP{-1}else{1})=>return 0,_=>{}}}
        match w as u16 {
            VK_F1=>{about();return 0;}
            VK_F2|VK_INSERT|VK_DELETE=>{open_manager();return 0;}
            VK_PRIOR=>{move_selection(-10);return 0;}
            VK_NEXT=>{move_selection(10);return 0;}
            0xba|0xde=>{choose_number(if w==0xba{2}else{3});return 0;}
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
                launch_selected();
                return 0;
            }
            VK_ESCAPE => {
                if cancel_parameter() {
                    return 0;
                }
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
        let(delay,parent)=STATE.with(|c|{let s=c.borrow();let s=s.as_ref().unwrap();(s.preferences.number("HideDelay"),s.hwnd)});if delay>0{unsafe{SetTimer(parent,AUTO_HIDE,delay as u32*1000,None);}}
    }
    if msg==WM_CHAR&&unsafe{GetKeyState(VK_CONTROL as i32)}<0&&matches!(w,3|4|12){return 0;}
    if msg==WM_CHAR&&matches!(w,59|39)&&!composing{return 0;}
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
            capture_context();
            if msg==WM_HOTKEY&&STATE.with(|c|c.borrow().as_ref().unwrap().hotkeys.get(2).is_some_and(|h|h.0==w as i32)){recent();}
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
        WM_CONTEXTMENU => {
            show_menu(hwnd);
            0
        }
        TRAY => {
            if l as u32 == WM_RBUTTONUP {
                show_menu(hwnd);
            } else if l as u32 == WM_LBUTTONUP {
                unsafe {
                    SendMessageW(hwnd, SHOW, 0, 0);
                }
            }
            0
        }
        LAUNCH_DONE => {
            let result =
                STATE.with(|c| c.borrow().as_ref().unwrap().launch_results.try_recv().ok());
            STATE.with(|c| c.borrow_mut().as_mut().unwrap().launching = false);
            match result {
                Some(Err(message)) => show_inline_error(&message),
                Some(Ok(())) => {
                    STATE.with(|c|{let mut s=c.borrow_mut();let s=s.as_mut().unwrap();if let Some((id,query,param))=s.successful.take(){s.usage.record(&id,&query,param.as_deref(),s.preferences.number("ParamHistoryLimit") as usize);s.usage_writer.publish(s.usage.text());rebuild_index(s);}});
                    cancel_parameter();
                    if STATE.with(|c|c.borrow().as_ref().unwrap().preferences.flag("ExitWhenExecute")){unsafe{PostMessageW(hwnd,QUIT,0,0);}}
                }
                None => {}
            }
            0
        }
        crate::settings::UPDATED=>{apply_settings_messages();0}
        MATCH_DONE=>{let result=STATE.with(|c|{let mut s=c.borrow_mut();let s=s.as_mut().unwrap();let mut latest=None;while let Ok((generation,result))=s.match_results.try_recv(){if generation==s.generation{latest=Some(result);}}if let Some(result)=latest{s.matching=false;s.hits=result.as_ref().map(|p|s.index.rank_positions(p,&s.current_query)).unwrap_or_default();s.selected=0;Some(result.err())}else{None}});if let Some(error)=result{render_hits();if let Some(error)=error{show_inline_error(&error);}}0}
        WM_TIMER if w==AUTO_HIDE=>{unsafe{KillTimer(hwnd,AUTO_HIDE);ShowWindow(hwnd,SW_HIDE);}0}
        WM_ACTIVATE if w&65535==WA_INACTIVE as usize=>{if STATE.with(|c|c.borrow().as_ref().is_some_and(|s|s.preferences.number("HideDelay")>0)){unsafe{ShowWindow(hwnd,SW_HIDE);}}0}
        WM_MOUSEWHEEL=>{move_selection(if ((w>>16)as i16)>0{-1}else{1});0}
        WM_MBUTTONUP=>{launch_selected();0}
        WM_PAINT => {
            paint_panel(hwnd);
            0
        }
        crate::manager::UPDATED => {
            let entries = STATE.with(|c| c.borrow_mut().as_mut().unwrap().library_updates.take());
            if let Some(entries) = entries {
                cancel_parameter();
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
        WM_COMMAND => {
            if w & 0xffff == ID_MANAGE {
                open_manager();
                return 0;
            }
            if w&0xffff==ID_SETTINGS{open_settings();return 0;}
            if w&0xffff==ID_RECENT{recent();return 0;}
            if w&0xffff==ID_ABOUT{about();return 0;}
            if w & 0xffff == ID_RELOAD {
                reload_library();
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
            if id == ID_LIST
                && STATE.with(|c| c.borrow().as_ref().unwrap().parameter_entry.is_some())
            {
                return 0;
            }
            if id == ID_EDIT && code == EN_CHANGE as usize {
                let(previous,key,query,number,composing)=STATE.with(|c|{let mut s=c.borrow_mut();let s=s.as_mut().unwrap();let key=std::mem::take(&mut s.last_key);(s.hits.clone(),key,edit_text(s.edit),s.preferences.flag("NumberKey"),s.composing)});
                refresh();
                let none=STATE.with(|c|{let s=c.borrow();let s=s.as_ref().unwrap();s.hits.is_empty()&&!s.matching&&s.parameter_entry.is_none()});
                if none&&!composing&&!previous.is_empty(){let digit=match key{0x30..=0x39=>Some((key-0x30)as usize),0x60..=0x69=>Some((key-0x60)as usize),_=>None};if (number&&digit.is_some())||key==VK_SPACE{STATE.with(|c|{let mut s=c.borrow_mut();let s=s.as_mut().unwrap();s.hits=previous;});if let Some(digit)=digit{choose_number(digit);}else{launch_selected();}}}
                let unique=STATE.with(|c|{let s=c.borrow();let s=s.as_ref().unwrap();s.enabled&&s.preferences.flag("ExecuteIfOnlyOne")&&s.hits.len()==1&&!s.matching&&s.parameter_entry.is_none()});if unique&&!composing&&key!=0&&!query.is_empty(){launch_selected();}
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
                        let rail = RECT {
                            left: 0,
                            top: d.rcItem.top + scaled(3, s.dpi),
                            right: scaled(2, s.dpi),
                            bottom: d.rcItem.bottom - scaled(3, s.dpi),
                        };
                        FillRect(d.hDC, &rail, s.accent);
                    }
                    SetBkMode(d.hDC, TRANSPARENT as i32);
                    SetTextColor(d.hDC, theme::MUTED);
                    SelectObject(d.hDC, s.font);
                    let number = format!(
                        "{}{}",
                        if e.param_type.needs_input(&e.command) {
                            '*'
                        } else {
                            ' '
                        },
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
        WM_CTLCOLORSTATIC | WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX => STATE.with(|c| {
            let state = c.borrow();
            let Some(s) = state.as_ref() else {
                return 0;
            };
            let dc = w as HDC;
            let paper = l as HWND == s.edit || l as HWND == s.list;
            unsafe {
                SetTextColor(
                    dc,
                    if l as HWND == s.command && s.error {
                        theme::ERROR
                    } else if l as HWND == s.command || l as HWND == s.empty {
                        theme::MUTED
                    } else {
                        theme::TEXT
                    },
                );
                SetBkColor(dc, if paper { theme::PAPER } else { theme::SURFACE });
            }
            if paper {
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
            if crate::manager::is_open()||crate::settings::is_open() {
                crate::manager::focus();
                unsafe {
                    MessageBoxW(
                        hwnd,
                        wide("请先保存或关闭快捷项管理窗口，再退出启动器。").as_ptr(),
                        wide("ALTRun").as_ptr(),
                        MB_OK | MB_ICONINFORMATION,
                    );
                }
                return 0;
            }
            unsafe {
                DestroyWindow(hwnd);
            }
            0
        }
        WM_DESTROY => {
            tray_icon(hwnd, NIM_DELETE);
            unsafe {
                STATE.with(|c|{if let Some(s)=c.borrow().as_ref(){for(id,_)in &s.hotkeys{UnregisterHotKey(hwnd,*id);}}});
                PostQuitMessage(0);
            }
            0
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, w, l) },
    }
}

pub fn run() -> Result<(), String> {
    let args: Vec<_> = std::env::args().collect();
    let data_path = if let Some(i) = args.iter().position(|a| a == "--data") {
        std::path::absolute(args.get(i + 1).ok_or("--data needs a path")?)
            .map_err(|e| e.to_string())?
    } else {
        std::env::current_exe()
            .map_err(|e| e.to_string())?
            .with_file_name("shortcuts.tsv")
    };
    let entries = if args.iter().any(|a| a == "--data") || data_path.exists() {
        altrun_probe::library::Snapshot::open(&data_path)?.entries
    } else {
        parse_entries(
            "notepad\t记事本\tnotepad.exe\t7\nCalc\t计算器\tcalc\t5\nExplorer\t资源管理器\texplorer.exe\t4\ncmd\t命令提示符\tcmd /k %p\t6\nb\t百度搜索\thttps://www.baidu.com/s?wd=%p\t12\n",
        )?
    };
    let index = SearchIndex::new(&entries);
    let(settings_file,settings_text)=altrun_probe::state_store::TextFile::open(&data_path.with_extension("settings.ini"))?;
    drop(settings_file);
    let preferences=altrun_probe::preferences::Preferences::parse(&settings_text)?;
    let(state_file,state_text)=altrun_probe::state_store::TextFile::open(&data_path.with_extension("state.tsv"))?;
    let usage=altrun_probe::usage::Usage::parse(&state_text)?;
    let usage_writer=altrun_probe::state_store::Writer::start(state_file);
    let(settings_sender,settings_updates)=std::sync::mpsc::channel();
    let(match_sender,match_tasks)=std::sync::mpsc::channel::<(u64,String,Vec<String>,bool)>();
    let(match_sender_result,match_results)=std::sync::mpsc::channel();
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
            match args.get(i + 1).map(String::as_str) {
                Some("150") => 144,
                Some("100") => 96,
                _ => return Err("--ui-scale accepts 100 or 150".into()),
            }
        } else {
            GetDpiForWindow(hwnd).max(96) as i32
        };
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
        let edit = control("EDIT", ES_AUTOHSCROLL as u32, 18, 35, 384, 22, ID_EDIT);
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
        if [title, edit, list, command, empty]
            .iter()
            .any(|h| h.is_null())
        {
            return Err("Cannot create native controls".into());
        }
        for child in [title, edit, list, command, empty] {
            SendMessageW(child, WM_SETFONT, font as WPARAM, 0);
        }
        SendMessageW(title, WM_SETFONT, title_font as WPARAM, 0);
        SendMessageW(edit, WM_SETFONT, input_font as WPARAM, 0);
        SendMessageW(
            edit,
            EM_SETCUEBANNER,
            0,
            wide("输入快捷字").as_ptr() as LPARAM,
        );
        SendMessageW(command, WM_SETFONT, command_font as WPARAM, 0);
        SendMessageW(list, LB_SETITEMHEIGHT, 0, scaled(theme::ROW, dpi) as LPARAM);
        let (launch_sender, launch_results) = std::sync::mpsc::channel();
        let (library_sender, library_updates) = altrun_probe::library_updates::channel();
        LAUNCH_SENDER.with(|s| *s.borrow_mut() = Some(launch_sender));
        let old_edit = SetWindowLongPtrW(edit, GWLP_WNDPROC, edit_proc as *const () as isize);
        STATE.with(|c| {
            *c.borrow_mut() = Some(State {
                preferences,usage,usage_writer,settings_sender,settings_updates,hotkeys:Vec::new(),pending_settings:None,generation:0,match_sender,match_results,current_query:String::new(),matching:false,last_key:0,context:altrun_probe::launch::LaunchContext{base_directory:std::env::current_dir().unwrap_or_default().to_string_lossy().into_owned(),..Default::default()},successful:None,history_position:0,
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
                title_font,
                input_font,
                command_font,
                paper,
                accent,
                border,
                empty,
                enabled: args.iter().any(|a| a == "--enable-launch"),
                parameter_entry: None,
                saved_query: String::new(),
                error: false,
                launching: false,
                launch_results,
                background,
                selected_brush,
                old_edit,
                dpi,
                allow_launch: args.iter().any(|a| a == "--allow-safe-notepad"),
                composing: false,
                data_path,
                library_sender,
                library_updates,
            })
        });
        let owner=hwnd as usize;
        std::thread::spawn(move||{while let Ok(mut task)=match_tasks.recv(){while let Ok(newer)=match_tasks.try_recv(){task=newer;}let(generation,pattern,aliases,prefix)=task;let result=std::env::current_exe().map_err(|e|e.to_string()).and_then(|exe|altrun_probe::isolated_matching::run(&exe,&pattern,&aliases,prefix,std::time::Duration::from_millis(100)));if match_sender_result.send((generation,result)).is_err(){break;}PostMessageW(owner as HWND,MATCH_DONE,0,0);}});
        STATE.with(|c|rebuild_index(c.borrow_mut().as_mut().unwrap()));
        refresh();
        if STATE.with(|c|c.borrow().as_ref().unwrap().preferences.flag("ShowTrayIcon")){tray_icon(hwnd, NIM_ADD);}
        // Deliberately separate from the original's Alt+R / Pause binding.
        let prefs=STATE.with(|c|c.borrow().as_ref().unwrap().preferences.clone());
        match register_bindings(hwnd,&prefs,&[]){Ok(keys)=>STATE.with(|c|c.borrow_mut().as_mut().unwrap().hotkeys=keys),Err(error)=>{DestroyWindow(hwnd);return Err(error);}}
        apply_appearance();
        if args.iter().any(|a| a == "--show")||STATE.with(|c|c.borrow().as_ref().unwrap().preferences.flag("ShowMeWhenStart")) {
            SendMessageW(hwnd, SHOW, 0, 0);
        }
        let accelerator = ACCEL {
            fVirt: FVIRTKEY | FALT,
            key: b'S' as u16,
            cmd: ID_MANAGE as u16,
        };
        let table = CreateAcceleratorTableW(&accelerator, 1);
        let mut message: MSG = zeroed();
        loop {
            let result = GetMessageW(&mut message, null_mut(), 0, 0);
            if result <= 0 {
                break;
            }
            if TranslateAcceleratorW(hwnd, table, &message) != 0 {
                continue;
            }
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
        if !table.is_null() {
            DestroyAcceleratorTable(table);
        }
        STATE.with(|c| {
            if let Some(s) = c.borrow_mut().take() {
                for font in [s.font, s.title_font, s.input_font, s.command_font] {
                    DeleteObject(font);
                }
                DeleteObject(s.paper);
                DeleteObject(s.accent);
                DeleteObject(s.border);
                DeleteObject(s.selected_brush);
                DeleteObject(s.background);
            }
        });
        UnregisterClassW(class.as_ptr(), instance);
    }
    Ok(())
}

fn open_manager() {
    let (path, entries, hwnd, sender) = STATE.with(|c| {
        let state = c.borrow();
        let s = state.as_ref().unwrap();
        (
            s.data_path.clone(),
            s.entries.clone(),
            s.hwnd,
            s.library_sender.clone(),
        )
    });
    crate::manager::open(path, entries, hwnd, sender);
    unsafe{ShowWindow(hwnd,SW_HIDE);}
}
fn reload_library() {
    let (path, sender, hwnd) = STATE.with(|c| {
        let state = c.borrow();
        let s = state.as_ref().unwrap();
        (
            s.data_path.clone(),
            s.library_sender.clone(),
            s.hwnd as usize,
        )
    });
    let revision = sender.begin_read();
    std::thread::spawn(move || match altrun_probe::library::Snapshot::open(&path) {
        Ok(snapshot) => {
            sender.read_complete(revision, snapshot.entries);
            unsafe {
                PostMessageW(hwnd as HWND, crate::manager::UPDATED, 0, 0);
            }
        }
        Err(error) => show_error(&error),
    });
}

fn visible_count(s:&State)->usize{if s.preferences.flag("ShowTopTen"){s.hits.len().min(10)}else{s.hits.len()}}
fn rebuild_index(s:&mut State){let mut rows=s.entries.clone();for entry in &mut rows{entry.frequency=entry.frequency.saturating_add(s.usage.counts.get(&entry.id).copied().unwrap_or(0).min(i64::MAX as u64) as i64);}s.index=SearchIndex::new(&rows);}
fn clipboard_text()->Option<String>{
    use windows_sys::Win32::System::{DataExchange::*,Memory::*};
    unsafe{if OpenClipboard(null_mut())==0{return None;}let handle=GetClipboardData(13);let result=if handle.is_null(){None}else{let pointer=GlobalLock(handle) as *const u16;if pointer.is_null(){None}else{let length=GlobalSize(handle)/2;let slice=std::slice::from_raw_parts(pointer,length.min(1024*1024));let result=String::from_utf16(slice.split(|v|*v==0).next().unwrap_or_default()).ok();GlobalUnlock(handle);result}};CloseClipboard();result}
}
fn copy_text(value:&str)->Result<(),String>{
    use windows_sys::Win32::System::{DataExchange::*,Memory::*};let bytes=wide(value);
    unsafe{let handle=GlobalAlloc(GMEM_MOVEABLE,bytes.len()*2);if handle.is_null(){return Err("无法分配剪贴板内存".into());}let pointer=GlobalLock(handle) as *mut u16;if pointer.is_null(){GlobalFree(handle);return Err("无法访问剪贴板内存".into());}std::ptr::copy_nonoverlapping(bytes.as_ptr(),pointer,bytes.len());GlobalUnlock(handle);if OpenClipboard(null_mut())==0{GlobalFree(handle);return Err("剪贴板正被其他程序使用".into());}EmptyClipboard();let success=!SetClipboardData(13,handle).is_null();CloseClipboard();if !success{GlobalFree(handle);return Err("写入剪贴板失败".into());}}Ok(())
}
fn capture_context(){unsafe{let foreground=GetForegroundWindow();if foreground.is_null(){return;}let own=STATE.with(|c|c.borrow().as_ref().unwrap().hwnd);if foreground==own{return;}let title=edit_text(foreground);let mut class=[0u16;256];let n=GetClassNameW(foreground,class.as_mut_ptr(),class.len() as i32);STATE.with(|c|{let mut s=c.borrow_mut();let s=s.as_mut().unwrap();s.context.window_id=foreground as usize;s.context.window_title=title;s.context.window_class=String::from_utf16_lossy(&class[..n.max(0) as usize]);s.context.allow_dynamic_target=s.preferences.flag("AllowDynamicTarget");let dir=s.preferences.get("WorkingDirectory");if !dir.is_empty(){s.context.base_directory=dir;}});}}
fn recent(){cancel_parameter();STATE.with(|c|{let mut s=c.borrow_mut();let s=s.as_mut().unwrap();s.generation=s.generation.wrapping_add(1);s.matching=false;s.hits=s.usage.recent.iter().filter_map(|id|s.entries.iter().position(|e|&e.id==id&&e.enabled&&e.kind==altrun_probe::library::EntryKind::Command)).collect();s.selected=0;});render_hits();}
fn about(){unsafe{MessageBoxW(null_mut(),wide("ALTRun Modern 0.1\n原生 Windows 快速启动器\n\nCtrl+Alt+R：呼出 / 隐藏\nEnter：执行；Esc：清空 / 隐藏\n↑↓ / Tab：选择；Ctrl+L：最近项目\nCtrl+数字：执行对应项\nCtrl+C：复制命令；Ctrl+D：打开目录\nF2 / Insert / Delete：管理快捷项\nAlt+S：快捷项管理\n\n数据与设置保存在快捷项文件旁；无网络搜索。\n项目：https://github.com/weimeng54/altrun").as_ptr(),wide("ALTRun · 帮助和关于").as_ptr(),MB_OK|MB_ICONINFORMATION);}}
fn open_settings(){let(path,prefs,hwnd,sender)=STATE.with(|c|{let s=c.borrow();let s=s.as_ref().unwrap();(s.data_path.with_extension("settings.ini"),s.preferences.clone(),s.hwnd,s.settings_sender.clone())});crate::settings::open(path,prefs,hwnd,sender);unsafe{ShowWindow(hwnd,SW_HIDE);}}
type Bindings=Vec<(i32,Option<altrun_probe::preferences::Hotkey>)>;
fn register_bindings(hwnd:HWND,prefs:&altrun_probe::preferences::Preferences,old:&Bindings)->Result<Bindings,String>{
    let mut next=Vec::new();let mut added=Vec::new();
    for(i,key)in ["HotKey1","HotKey2","LastItemHotKey"].iter().enumerate(){let value=altrun_probe::preferences::parse_hotkey(&prefs.get(key))?;
        if let Some(existing)=old.get(i)&&existing.1==value{next.push(*existing);continue;}
        let id=if old.get(i).is_some_and(|v|v.0==HOTKEY+i as i32){HOTKEY+10+i as i32}else{HOTKEY+i as i32};
        if let Some(value)=value{if unsafe{RegisterHotKey(hwnd,id,value.modifiers|MOD_NOREPEAT,value.key)}==0{for id in added{unsafe{UnregisterHotKey(hwnd,id);}}return Err(format!("快捷键 {} 已被占用，原快捷键保持可用",prefs.get(key)));}added.push(id);}
        next.push((id,value));
    }Ok(next)
}
fn apply_settings_messages(){
    loop{let change=STATE.with(|c|c.borrow().as_ref().unwrap().settings_updates.try_recv().ok());let Some(change)=change else{break;};
        match change{
            crate::settings::Change::Prepare(token,prefs,reply)=>{let(hwnd,old,pending)=STATE.with(|c|{let s=c.borrow();let s=s.as_ref().unwrap();(s.hwnd,s.hotkeys.clone(),s.pending_settings.is_some())});if pending{let _=reply.send(Err("另一次设置修改尚未完成".into()));continue;}match register_bindings(hwnd,&prefs,&old){Ok(keys)=>{STATE.with(|c|c.borrow_mut().as_mut().unwrap().pending_settings=Some((token,prefs,keys)));let _=reply.send(Ok(()));},Err(e)=>{let _=reply.send(Err(e));}}},
            crate::settings::Change::Commit(token)=>{let accepted=STATE.with(|c|{let mut s=c.borrow_mut();let s=s.as_mut().unwrap();if !s.pending_settings.as_ref().is_some_and(|p|p.0==token){return false;}let(_,prefs,keys)=s.pending_settings.take().unwrap();for(id,value)in &s.hotkeys{if value.is_some()&&!keys.iter().any(|v|v.0==*id){unsafe{UnregisterHotKey(s.hwnd,*id);}}}s.preferences=prefs;s.hotkeys=keys;true});if accepted{apply_appearance();refresh();}},
            crate::settings::Change::Cancel(token)=>{STATE.with(|c|{let mut s=c.borrow_mut();let s=s.as_mut().unwrap();if s.pending_settings.as_ref().is_some_and(|p|p.0==token){let(_,_,keys)=s.pending_settings.take().unwrap();for(id,value)in keys{if value.is_some()&&!s.hotkeys.iter().any(|v|v.0==id){unsafe{UnregisterHotKey(s.hwnd,id);}}}}});}
        }
    }
}
fn apply_appearance(){
    let(hwnd,edit,list,title,command,prefs,dpi)=STATE.with(|c|{let s=c.borrow();let s=s.as_ref().unwrap();(s.hwnd,s.edit,s.list,s.title,s.command,s.preferences.clone(),s.dpi)});
    unsafe{
        let definitions=[("ListFont",list),("TitleFont",title),("KeywordFont",edit)];let mut fonts=Vec::new();
        for(key,control)in definitions{if let Ok(f)=altrun_probe::preferences::Font::parse(&prefs.get(key)){let font=CreateFontW(-f.points*dpi/72,0,0,0,f.weight,u32::from(f.italic),u32::from(f.underline),0,DEFAULT_CHARSET as u32,0,0,CLEARTYPE_QUALITY as u32,0,wide(&f.name).as_ptr());if !font.is_null(){SendMessageW(control,WM_SETFONT,font as usize,1);fonts.push((key,font));}}}
        STATE.with(|c|{let mut s=c.borrow_mut();let s=s.as_mut().unwrap();for(key,font)in fonts{let slot=match key{"ListFont"=>&mut s.font,"TitleFont"=>&mut s.title_font,_=>&mut s.input_font};let old=std::mem::replace(slot,font);DeleteObject(old);}});
        let alpha=prefs.number("Alpha").clamp(40,255) as u8;let style=GetWindowLongW(hwnd,GWL_EXSTYLE) as u32;SetWindowLongW(hwnd,GWL_EXSTYLE,(style|WS_EX_LAYERED) as i32);SetLayeredWindowAttributes(hwnd,0,alpha,LWA_ALPHA);
        ShowWindow(command,if prefs.flag("ShowCommandLine"){SW_SHOWNOACTIVATE}else{SW_HIDE});
        tray_icon(hwnd,if prefs.flag("ShowTrayIcon"){NIM_ADD}else{NIM_DELETE});
        SendMessageW(list,LB_SETITEMHEIGHT,0,scaled(prefs.number("RowHeight"),dpi) as isize);
    }
}
fn choose_number(digit:usize){let chosen=STATE.with(|c|{let mut s=c.borrow_mut();let s=s.as_mut().unwrap();if s.parameter_entry.is_some()||s.matching{return false;}let index=if s.preferences.flag("IndexFrom0to9"){digit}else{(digit+9)%10};if index<visible_count(s){s.selected=index;true}else{false}});if chosen{launch_selected();}}
fn parameter_history(delta:isize)->bool{let value=STATE.with(|c|{let mut s=c.borrow_mut();let s=s.as_mut().unwrap();let index=s.parameter_entry?;let history=s.usage.parameters.get(&s.entries[index].id).or_else(||s.usage.parameters.get("*"))?;if history.is_empty(){return None;}s.history_position=advance(s.history_position,history.len(),delta);Some((s.edit,history[s.history_position].clone()))});if let Some((edit,value))=value{unsafe{SetWindowTextW(edit,wide(&value).as_ptr());SendMessageW(edit,EM_SETSEL,0,-1);}true}else{false}}
fn selected_action(copy:bool){let entry=STATE.with(|c|{let s=c.borrow();let s=s.as_ref().unwrap();s.hits.get(s.selected).map(|i|s.entries[*i].clone())});if let Some(entry)=entry{if copy{if let Err(e)=copy_text(&entry.command){show_inline_error(&e);}}else{std::thread::spawn(move||{let context=altrun_probe::launch::LaunchContext::default();match altrun_probe::launch::resolve_context(&entry.command,None,entry.param_type,&entry.working_directory,&context){Ok((request,_,_))=>{if let Some(parent)=std::path::Path::new(&request.file).parent().filter(|p|!p.as_os_str().is_empty()){let _=execute_request(&altrun_probe::launch::LaunchRequest{file:parent.to_string_lossy().into_owned(),parameters:String::new()});}},Err(e)=>crate::native::show_error(&e)}});}}}

