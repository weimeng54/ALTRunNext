//! On-demand shortcut editor, isolated from the resident launcher's UI thread.
use altrun_probe::{Entry, library::{self, ParamType, Snapshot}};
use crate::{native::{make_font, scaled, wide}, theme};
use std::{cell::RefCell, mem::zeroed, path::PathBuf, ptr::null_mut, sync::{atomic::{AtomicBool, AtomicUsize, Ordering}, mpsc::Sender}};
use windows_sys::Win32::{Foundation::*, Graphics::Gdi::*, System::LibraryLoader::GetModuleHandleW, UI::{Controls::Dialogs::*, Controls::*, HiDpi::*, Input::KeyboardAndMouse::*, WindowsAndMessaging::*}};

const CLASS: &str = "ALTRun.Native.ShortcutManager";
const FOCUS: u32 = WM_APP + 1;
pub const UPDATED: u32 = WM_APP + 6;
const LIST: usize = 2101;
const ALIAS: usize = 2102;
const NAME: usize = 2103;
const COMMAND: usize = 2104;
const KIND: usize = 2105;
const ADD: usize = 2110;
const DELETE: usize = 2111;
const IMPORT: usize = 2112;
const SAVE: usize = 2113;
const CLOSE: usize = 2114;
static OPEN: AtomicBool = AtomicBool::new(false);
static WINDOW: AtomicUsize = AtomicUsize::new(0);

struct Editor {
    file: Snapshot, draft: Vec<Entry>, selected: usize, dirty: bool, loading: bool,
    list: HWND, alias: HWND, name: HWND, command: HWND, kind: HWND,
    status: HWND, save: HWND, delete: HWND, font: HFONT, heading: HFONT,
    background: HBRUSH, paper: HBRUSH, selection: HBRUSH, dpi: i32, error: bool,
    parent: usize, sender: Sender<Vec<Entry>>,
}
thread_local! { static EDITOR: RefCell<Option<Editor>> = const { RefCell::new(None) }; }

pub fn is_open() -> bool { OPEN.load(Ordering::Acquire) }
pub fn focus() { let hwnd = WINDOW.load(Ordering::Acquire); if hwnd != 0 { unsafe { PostMessageW(hwnd as HWND, FOCUS, 0, 0); } } }
pub fn open(path: PathBuf, initial: Vec<Entry>, parent: HWND, sender: Sender<Vec<Entry>>) {
    if OPEN.swap(true, Ordering::AcqRel) { focus(); return; }
    let owner = parent as usize;
    std::thread::spawn(move || {
        if let Err(error) = run(path, initial, owner, sender) {
            unsafe { MessageBoxW(null_mut(), wide(&error).as_ptr(), wide("无法打开快捷项管理").as_ptr(), MB_OK | MB_ICONERROR); }
        }
        WINDOW.store(0, Ordering::Release); OPEN.store(false, Ordering::Release);
    });
}
fn text(hwnd: HWND) -> String {
    unsafe { let mut buffer = vec![0; GetWindowTextLengthW(hwnd) as usize + 1]; let n = GetWindowTextW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32); String::from_utf16_lossy(&buffer[..n as usize]) }
}
fn status(message: &str, error: bool) {
    let hwnd = EDITOR.with(|c| { let mut state = c.borrow_mut(); let s = state.as_mut().unwrap(); s.error = error; s.status });
    unsafe { SetWindowTextW(hwnd, wide(message).as_ptr()); InvalidateRect(hwnd, std::ptr::null(), 1); }
}
fn sync_fields() {
    let (list, alias, name, command, kind, save, delete, entry, dirty) = EDITOR.with(|c| {
        let mut state = c.borrow_mut(); let s = state.as_mut().unwrap(); s.loading = true;
        (s.list,s.alias,s.name,s.command,s.kind,s.save,s.delete,s.draft.get(s.selected).cloned(),s.dirty)
    });
    unsafe {
        for hwnd in [alias,name,command,kind,delete] { EnableWindow(hwnd, i32::from(entry.is_some())); }
        SetWindowTextW(alias, wide(entry.as_ref().map_or("", |e| e.alias.as_str())).as_ptr());
        SetWindowTextW(name, wide(entry.as_ref().map_or("", |e| e.name.as_str())).as_ptr());
        SetWindowTextW(command, wide(entry.as_ref().map_or("", |e| e.command.as_str())).as_ptr());
        let index = entry.map_or(0, |e| e.param_type as usize);
        SendMessageW(kind, CB_SETCURSEL, index, 0); EnableWindow(save, i32::from(dirty));
        InvalidateRect(list, std::ptr::null(), 0);
    }
    EDITOR.with(|c| c.borrow_mut().as_mut().unwrap().loading = false);
}
fn refresh_list() {
    let (list, count, selected) = EDITOR.with(|c| { let s = c.borrow(); let s = s.as_ref().unwrap(); (s.list,s.draft.len(),s.selected) });
    unsafe { SendMessageW(list, LB_SETCOUNT, count, 0); if count != 0 { SendMessageW(list, LB_SETCURSEL, selected, 0); } }
    sync_fields();
}
fn update_draft() {
    let handles = EDITOR.with(|c| { let s = c.borrow(); let s = s.as_ref()?; if s.loading { None } else { Some((s.alias,s.name,s.command,s.kind)) } });
    let Some((alias,name,command,kind)) = handles else { return; };
    let values = (text(alias),text(name),text(command),unsafe { SendMessageW(kind, CB_GETCURSEL, 0, 0) });
    let (list, save) = EDITOR.with(|c| {
        let mut state = c.borrow_mut(); let s = state.as_mut().unwrap();
        if let Some(entry) = s.draft.get_mut(s.selected) {
            entry.alias = values.0; entry.name = values.1; entry.command = values.2;
            entry.param_type = match values.3 { 1 => ParamType::None, 2 => ParamType::Raw, 3 => ParamType::LegacyUrl, 4 => ParamType::Utf8Url, _ => ParamType::Auto };
            s.dirty = true;
        }
        (s.list,s.save)
    });
    unsafe { EnableWindow(save, 1); InvalidateRect(list, std::ptr::null(), 0); }
    status("尚未保存。保存后立即更新启动面板。",false);
}
fn save() {
    status("正在保存…",false);
    let result = EDITOR.with(|c| {
        let mut state = c.borrow_mut(); let s = state.as_mut().unwrap();
        match s.file.save(&s.draft) {
            Ok(()) => { s.dirty = false; Ok((s.sender.clone(),s.draft.clone(),s.parent)) },
            Err(error) => Err(error),
        }
    });
    match result {
        Ok((sender,entries,parent)) => {
            let _ = sender.send(entries); unsafe { PostMessageW(parent as HWND, UPDATED, 0, 0); }
            status("已保存。启动面板已更新，上一版保存在 .bak 文件。",false); sync_fields();
        }
        Err(error) => status(&error,true),
    }
}
fn confirm(hwnd: HWND, message: &str) -> bool {
    unsafe { MessageBoxW(hwnd, wide(message).as_ptr(), wide("管理快捷项").as_ptr(), MB_OKCANCEL | MB_ICONWARNING | MB_DEFBUTTON2) == IDOK }
}
fn import(hwnd: HWND) {
    unsafe {
        let mut file = vec![0_u16; 32768]; let filter = wide("快捷项文件 (*.txt;*.tsv)\0*.txt;*.tsv\0所有文件\0*.*\0");
        let mut dialog: OPENFILENAMEW = zeroed(); dialog.lStructSize = std::mem::size_of_val(&dialog) as u32;
        dialog.hwndOwner = hwnd; dialog.lpstrFilter = filter.as_ptr(); dialog.lpstrFile = file.as_mut_ptr(); dialog.nMaxFile = file.len() as u32;
        dialog.Flags = OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR;
        if GetOpenFileNameW(&mut dialog) == 0 { return; }
        let length = file.iter().position(|&c| c == 0).unwrap_or(file.len());
        let path = PathBuf::from(String::from_utf16_lossy(&file[..length]));
        match std::fs::read(&path).map_err(|e| e.to_string()).and_then(|bytes| library::decode(&bytes)).and_then(|text| library::import_legacy(&text)) {
            Ok(imported) => {
                if imported.entries.is_empty() { status("文件中没有可导入的快捷项。",true); return; }
                if !confirm(hwnd,&format!("用导入的 {} 个快捷项替换当前草稿？保存前不会修改数据文件。",imported.entries.len())) { return; }
                let note = format!("已导入 {} 项，跳过 {} 行备注或空行；尚未保存。",imported.entries.len(),imported.ignored_lines);
                EDITOR.with(|c| { let mut state = c.borrow_mut(); let s = state.as_mut().unwrap(); s.draft = imported.entries; s.selected = 0; s.dirty = true; });
                refresh_list(); status(&note,false);
            }
            Err(error) => status(&error,true),
        }
    }
}
unsafe extern "system" fn procedure(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    if msg == WM_DESTROY { unsafe { PostQuitMessage(0); } return 0; }
    if msg == FOCUS { unsafe { ShowWindow(hwnd,SW_SHOWNORMAL); SetForegroundWindow(hwnd); } return 0; }
    if msg == WM_CLOSE {
        let dirty = EDITOR.with(|c| c.borrow().as_ref().is_some_and(|s| s.dirty));
        if !dirty || confirm(hwnd,"有尚未保存的修改。放弃修改并关闭？") { unsafe { DestroyWindow(hwnd); } }
        return 0;
    }
    let ready = EDITOR.with(|c| c.borrow().is_some());
    if !ready { return unsafe { DefWindowProcW(hwnd,msg,w,l) }; }
    match msg {
        WM_COMMAND => {
            let id = w & 0xffff; let notification = (w >> 16) & 0xffff;
            if id == LIST && notification == LBN_SELCHANGE as usize {
                let list = EDITOR.with(|c| c.borrow().as_ref().unwrap().list);
                let selected = unsafe { SendMessageW(list,LB_GETCURSEL,0,0) };
                if selected >= 0 { EDITOR.with(|c| c.borrow_mut().as_mut().unwrap().selected = selected as usize); sync_fields(); }
            } else if [ALIAS,NAME,COMMAND].contains(&id) && notification == EN_CHANGE as usize || id == KIND && notification == CBN_SELCHANGE as usize { update_draft(); }
            else if notification == BN_CLICKED as usize {
                match id {
                    ADD => {
                        EDITOR.with(|c| { let mut state = c.borrow_mut(); let s = state.as_mut().unwrap(); s.draft.push(Entry { alias:String::new(),name:String::new(),command:String::new(),frequency:0,param_type:ParamType::None }); s.selected=s.draft.len()-1; s.dirty=true; });
                        refresh_list(); let field = EDITOR.with(|c| c.borrow().as_ref().unwrap().alias); unsafe { SetFocus(field); }
                    }
                    DELETE => {
                        let name = EDITOR.with(|c| { let s=c.borrow(); let s=s.as_ref().unwrap(); s.draft.get(s.selected).map(|e|format!("{} · {}",e.alias,e.name)) });
                        if let Some(name)=name && confirm(hwnd,&format!("从草稿中删除“{name}”？保存后生效。")) {
                            EDITOR.with(|c| { let mut state=c.borrow_mut(); let s=state.as_mut().unwrap(); s.draft.remove(s.selected); s.selected=s.selected.min(s.draft.len().saturating_sub(1)); s.dirty=true; }); refresh_list(); status("已从草稿删除，尚未保存。",false);
                        }
                    }
                    IMPORT => import(hwnd), SAVE => save(), CLOSE => { unsafe { PostMessageW(hwnd,WM_CLOSE,0,0); } }
                    _ => {}
                }
            }
            0
        }
        WM_DRAWITEM => {
            let d=unsafe { &*(l as *const DRAWITEMSTRUCT) };
            EDITOR.with(|c| { let state=c.borrow(); let s=state.as_ref().unwrap(); let Some(entry)=s.draft.get(d.itemID as usize) else {return;};
                unsafe {
                    FillRect(d.hDC,&d.rcItem,if d.itemState & ODS_SELECTED !=0 {s.selection}else{s.paper}); SetBkMode(d.hDC,TRANSPARENT as i32);SetTextColor(d.hDC,theme::TEXT);SelectObject(d.hDC,s.font);
                    let mut rect=d.rcItem;rect.left+=scaled(8,s.dpi);rect.right=scaled(100,s.dpi);
                    DrawTextW(d.hDC,wide(if entry.alias.is_empty(){"新快捷项"}else{&entry.alias}).as_ptr(),-1,&mut rect,DT_SINGLELINE|DT_VCENTER|DT_END_ELLIPSIS|DT_NOPREFIX);
                    rect.left=scaled(106,s.dpi);rect.right=d.rcItem.right-scaled(6,s.dpi);SetTextColor(d.hDC,theme::MUTED);
                    DrawTextW(d.hDC,wide(&entry.name).as_ptr(),-1,&mut rect,DT_SINGLELINE|DT_VCENTER|DT_END_ELLIPSIS|DT_NOPREFIX);
                }
            }); 1
        }
        WM_CTLCOLORSTATIC | WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX => EDITOR.with(|c| {let state=c.borrow();let s=state.as_ref().unwrap();unsafe{SetTextColor(w as HDC,if l as HWND==s.status && s.error{theme::ERROR}else{theme::TEXT});SetBkColor(w as HDC,if msg==WM_CTLCOLORSTATIC{theme::SURFACE}else{theme::PAPER});} if msg==WM_CTLCOLORSTATIC{s.background as LRESULT}else{s.paper as LRESULT}}),
        _ => unsafe { DefWindowProcW(hwnd,msg,w,l) }
    }
}
fn run(path: PathBuf, initial: Vec<Entry>, parent: usize, sender: Sender<Vec<Entry>>) -> Result<(),String> {
    let file = if path.exists() { Snapshot::open(&path)? } else { Snapshot::new(path,initial)? };
    unsafe {
        let instance=GetModuleHandleW(std::ptr::null()); let background=CreateSolidBrush(theme::SURFACE);
        let mut class:WNDCLASSW=zeroed();class.lpfnWndProc=Some(procedure);class.hInstance=instance;
        let class_name=wide(CLASS);class.lpszClassName=class_name.as_ptr();class.hbrBackground=background;class.hCursor=LoadCursorW(null_mut(),IDC_ARROW);RegisterClassW(&class);
        let hwnd=CreateWindowExW(WS_EX_CONTROLPARENT,class_name.as_ptr(),wide("ALTRun · 管理快捷项").as_ptr(),WS_OVERLAPPED|WS_CAPTION|WS_SYSMENU|WS_MINIMIZEBOX,450,180,760,440,null_mut(),null_mut(),instance,std::ptr::null());
        if hwnd.is_null(){DeleteObject(background);return Err("无法创建管理窗口".into());}
        let dpi=GetDpiForWindow(hwnd).max(96) as i32;
        let mut outer=RECT{left:0,top:0,right:scaled(760,dpi),bottom:scaled(420,dpi)};AdjustWindowRectEx(&mut outer,WS_OVERLAPPED|WS_CAPTION|WS_SYSMENU|WS_MINIMIZEBOX,0,WS_EX_CONTROLPARENT);
        SetWindowPos(hwnd,null_mut(),0,0,outer.right-outer.left,outer.bottom-outer.top,SWP_NOMOVE|SWP_NOZORDER|SWP_NOACTIVATE);
        let font=make_font(13,400,"Microsoft YaHei UI",dpi);let heading=make_font(16,600,"Microsoft YaHei UI",dpi);
        let control=|class:&str,label:&str,style:u32,x,y,width,height,id:usize|{
            let child=CreateWindowExW(0,wide(class).as_ptr(),wide(label).as_ptr(),WS_CHILD|WS_VISIBLE|style,scaled(x,dpi),scaled(y,dpi),scaled(width,dpi),scaled(height,dpi),hwnd,id as HMENU,instance,std::ptr::null());
            SendMessageW(child,WM_SETFONT,font as WPARAM,0);child
        };
        let title=control("STATIC","管理快捷项",0,12,12,300,25,0);SendMessageW(title,WM_SETFONT,heading as WPARAM,0);
        let list=control("LISTBOX","",WS_BORDER|WS_VSCROLL|WS_TABSTOP|LBS_NOTIFY as u32|LBS_OWNERDRAWFIXED as u32|LBS_NODATA as u32|LBS_NOINTEGRALHEIGHT as u32,12,52,250,304,LIST);SendMessageW(list,LB_SETITEMHEIGHT,0,scaled(22,dpi) as LPARAM);
        control("STATIC","快捷字",0,280,52,210,20,0); control("STATIC","名称",0,512,52,232,20,0);
        let alias=control("EDIT","",WS_BORDER|WS_TABSTOP|ES_AUTOHSCROLL as u32,280,76,216,28,ALIAS);
        let name=control("EDIT","",WS_BORDER|WS_TABSTOP|ES_AUTOHSCROLL as u32,512,76,232,28,NAME);
        control("STATIC","命令或路径",0,280,118,464,20,0);
        let command=control("EDIT","",WS_BORDER|WS_TABSTOP|ES_AUTOHSCROLL as u32,280,142,464,28,COMMAND);
        control("STATIC","参数类型",0,280,184,464,20,0);
        let kind=control("COMBOBOX","",WS_TABSTOP|CBS_DROPDOWNLIST as u32|WS_VSCROLL,280,208,300,220,KIND);
        for label in ["自动（旧体验数据）","无参数","原样参数","旧式 URL 编码","UTF-8 URL 编码"] {SendMessageW(kind,CB_ADDSTRING,0,wide(label).as_ptr() as LPARAM);}
        control("STATIC","带空格的程序路径请加双引号。\r\n使用 %p 或 {%p} 填入参数；UTF-8 URL 适用于网页搜索。",0,280,252,464,48,0);
        let status=control("STATIC","修改后点击保存。原版配置不会被改写。",0,280,318,464,40,0);
        control("BUTTON","新增",WS_TABSTOP,12,374,72,30,ADD);let delete=control("BUTTON","删除",WS_TABSTOP,92,374,72,30,DELETE);
        control("BUTTON","导入旧版…",WS_TABSTOP,172,374,90,30,IMPORT);
        let save=control("BUTTON","保存",WS_TABSTOP|BS_DEFPUSHBUTTON as u32,560,374,88,30,SAVE);
        control("BUTTON","关闭",WS_TABSTOP,656,374,88,30,CLOSE);
        let draft=file.entries.clone();let paper=CreateSolidBrush(theme::PAPER);let selection=CreateSolidBrush(theme::SELECTION);
        EDITOR.with(|c|*c.borrow_mut()=Some(Editor{file,draft,selected:0,dirty:false,loading:false,list,alias,name,command,kind,status,save,delete,font,heading,background,paper,selection,dpi,error:false,parent,sender}));
        refresh_list();WINDOW.store(hwnd as usize,Ordering::Release);ShowWindow(hwnd,SW_SHOWNORMAL);SetForegroundWindow(hwnd);SetFocus(list);
        let mut message:MSG=zeroed();while GetMessageW(&mut message,null_mut(),0,0)>0 {
            if message.message==WM_KEYDOWN && message.wParam==VK_TAB as usize && IsDialogMessageW(hwnd,&message)!=0 {continue;}
            TranslateMessage(&message);DispatchMessageW(&message);
        }
        EDITOR.with(|c|{if let Some(s)=c.borrow_mut().take(){for object in [s.font,s.heading,s.background,s.paper,s.selection]{DeleteObject(object);}}});
        UnregisterClassW(class_name.as_ptr(),instance);
    } Ok(())
}
