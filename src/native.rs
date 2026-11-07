//! Deliberately small performance experiment, not the full launcher.
//! Persistent Win32 EDIT/LISTBOX and GDI share the original's thin rendering
//! path. Direct2D is not initialized merely to draw ten text rows: the hard
//! performance gate takes precedence over the earlier rendering preference.
use altrun_probe::{Entry, SearchIndex, advance, parse_entries};
use std::{cell::RefCell, mem::zeroed, ptr::null_mut};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::LibraryLoader::GetModuleHandleW,
    UI::{Controls::*, HiDpi::*, Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
};

const ID_EDIT: usize = 1001;
const ID_LIST: usize = 1002;
const HOTKEY: i32 = 17001;
const SHOW: u32 = WM_APP + 1;
const QUIT: u32 = WM_APP + 2;
// SS_CENTER from WinUser.h; not projected by the selected windows-sys features.
const STATIC_CENTER: u32 = 0x00000001;

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
    background: HBRUSH,
    selected_brush: HBRUSH,
    old_edit: isize,
    dpi: i32,
    allow_launch: bool,
    composing: bool,
}

thread_local! { static STATE: RefCell<Option<State>> = const { RefCell::new(None) }; }

fn wide(text: &str) -> Vec<u16> { text.encode_utf16().chain(Some(0)).collect() }
fn scaled(value: i32, dpi: i32) -> i32 { value * dpi / 96 }

pub fn show_error(message: &str) {
    unsafe { MessageBoxW(null_mut(), wide(message).as_ptr(), wide("ALTRun performance probe").as_ptr(), MB_OK | MB_ICONERROR); }
}

fn selected_text() -> (HWND, HWND, Vec<u16>, Vec<u16>) {
    STATE.with(|cell| {
        let s = cell.borrow(); let s = s.as_ref().unwrap();
        let e = s.hits.get(s.selected).map(|&i| &s.entries[i]);
        (s.title, s.command, wide(e.map(|e|e.name.as_str()).unwrap_or("")), wide(&e.map(|e|format!("CMD={}",e.command)).unwrap_or_default()))
    })
}

fn update_selection() {
    let (title, command, name, line) = selected_text();
    unsafe { SetWindowTextW(title,name.as_ptr()); SetWindowTextW(command,line.as_ptr()); }
}

fn refresh() {
    let edit = STATE.with(|c|c.borrow().as_ref().unwrap().edit);
    let query = unsafe {
        let mut buffer=vec![0_u16; GetWindowTextLengthW(edit) as usize+1];
        let len=GetWindowTextW(edit,buffer.as_mut_ptr(),buffer.len() as i32);
        String::from_utf16_lossy(&buffer[..len as usize])
    };
    // Release RefCell before calling native controls, which may synchronously
    // reenter WM_DRAWITEM. Never keep a mutable state borrow across SendMessage.
    let (hwnd,list,command,dpi,count) = STATE.with(|c| {
        let mut s=c.borrow_mut();let s=s.as_mut().unwrap();
        s.hits=s.index.query(&query);s.selected=0;
        (s.hwnd,s.list,s.command,s.dpi,s.hits.len().min(10))
    });
    unsafe {
        SendMessageW(list,WM_SETREDRAW,0,0);
        SendMessageW(list,LB_SETCOUNT,count,0);
        SendMessageW(list,LB_SETCURSEL,0,0);
        let height=scaled(count as i32*20,dpi);
        SetWindowPos(list,null_mut(),scaled(5,dpi),scaled(54,dpi),scaled(410,dpi),height,SWP_NOZORDER|SWP_NOACTIVATE);
        SetWindowPos(command,null_mut(),scaled(7,dpi),scaled(58,dpi)+height,scaled(406,dpi),scaled(20,dpi),SWP_NOZORDER|SWP_NOACTIVATE);
        SetWindowPos(hwnd,null_mut(),0,0,scaled(420,dpi),scaled(84,dpi)+height,SWP_NOMOVE|SWP_NOZORDER|SWP_NOACTIVATE);
        SendMessageW(list,WM_SETREDRAW,1,0);
        InvalidateRect(list,std::ptr::null(),0);
    }
    update_selection();
}

fn move_selection(delta: isize) {
    let (list,selected)=STATE.with(|c| {
        let mut s=c.borrow_mut();let s=s.as_mut().unwrap();
        s.selected=advance(s.selected,s.hits.len().min(10),delta);
        (s.list,s.selected)
    });
    unsafe { SendMessageW(list,LB_SETCURSEL,selected,0); }
    update_selection();
}

fn launch_selected() {
    let (allow,cmd)=STATE.with(|c| {
        let s=c.borrow();let s=s.as_ref().unwrap();
        (s.allow_launch,s.hits.get(s.selected).map(|&i|s.entries[i].command.clone()))
    });
    // Real command parsing belongs after the performance gate. Until then only
    // the explicit safe notepad test is allowed. Imported commands never run.
    if allow && cmd.as_deref()==Some("notepad.exe") {
        let _=std::process::Command::new("notepad.exe").spawn();
    }
    let hwnd=STATE.with(|c|c.borrow().as_ref().unwrap().hwnd);
    unsafe { ShowWindow(hwnd,SW_HIDE); }
}

unsafe extern "system" fn edit_proc(hwnd: HWND,msg:u32,w:WPARAM,l:LPARAM)->LRESULT {
    if msg==WM_IME_STARTCOMPOSITION { STATE.with(|c|c.borrow_mut().as_mut().unwrap().composing=true); }
    if msg==WM_IME_ENDCOMPOSITION { STATE.with(|c|c.borrow_mut().as_mut().unwrap().composing=false); }
    let composing=STATE.with(|c|c.borrow().as_ref().unwrap().composing);
    if msg==WM_KEYDOWN && !composing {
        match w as u16 {
            VK_DOWN => {move_selection(1);return 0;},
            VK_UP => {move_selection(-1);return 0;},
            VK_TAB => {move_selection(if unsafe {GetKeyState(VK_SHIFT as i32)}<0 {-1} else {1});return 0;},
            VK_RETURN => {launch_selected();return 0;},
            VK_ESCAPE => {
                if unsafe {GetWindowTextLengthW(hwnd)}>0 {unsafe {SetWindowTextW(hwnd,wide("").as_ptr());}}
                else {let parent=unsafe {GetParent(hwnd)};unsafe {ShowWindow(parent,SW_HIDE);}}
                return 0;
            },
            _=>{}
        }
    }
    if msg==WM_CHAR && matches!(w as u16, VK_RETURN|VK_ESCAPE|VK_TAB) && !composing { return 0; }
    let old=STATE.with(|c|c.borrow().as_ref().unwrap().old_edit);
    unsafe {CallWindowProcW(Some(std::mem::transmute::<isize,unsafe extern "system" fn(HWND,u32,WPARAM,LPARAM)->LRESULT>(old)),hwnd,msg,w,l)}
}

unsafe extern "system" fn window_proc(hwnd: HWND,msg:u32,w:WPARAM,l:LPARAM)->LRESULT {
    match msg {
        WM_HOTKEY|SHOW => {
            let edit=STATE.with(|c|c.borrow().as_ref().unwrap().edit);
            unsafe {
                if msg==WM_HOTKEY && IsWindowVisible(hwnd)!=0 {ShowWindow(hwnd,SW_HIDE);}
                else {ShowWindow(hwnd,SW_SHOWNOACTIVATE);SetForegroundWindow(hwnd);SetFocus(edit);}
            }
            0
        },
        WM_COMMAND => {
            let id=w&0xffff;let code=(w>>16)&0xffff;
            if id==ID_EDIT && code==EN_CHANGE as usize {refresh();}
            if id==ID_LIST && code==LBN_SELCHANGE as usize {
                let list=STATE.with(|c|c.borrow().as_ref().unwrap().list);
                let selected=unsafe {SendMessageW(list,LB_GETCURSEL,0,0)};
                if selected>=0 {STATE.with(|c|c.borrow_mut().as_mut().unwrap().selected=selected as usize);update_selection();}
            }
            if id==ID_LIST && code==LBN_DBLCLK as usize {launch_selected();}
            0
        },
        WM_DRAWITEM => {
            let d=unsafe {&*(l as *const DRAWITEMSTRUCT)};
            if d.itemID==u32::MAX {return 1;}
            STATE.with(|c| {
                let s=c.borrow();let Some(s)=s.as_ref() else {return;};
                let Some(&i)=s.hits.get(d.itemID as usize) else {return;};
                let e=&s.entries[i];
                unsafe {
                    FillRect(d.hDC,&d.rcItem,if d.itemState&ODS_SELECTED!=0 {s.selected_brush}else{s.background});
                    SetBkMode(d.hDC,TRANSPARENT as i32);SetTextColor(d.hDC,0x00362a20);SelectObject(d.hDC,s.font);
                    let number=format!("{}{}",if e.command.contains("%p") {'*'} else {' '},(d.itemID+1)%10);
                    let a=wide(&number);let b=wide(&e.alias);let name=wide(&e.name);
                    TextOutW(d.hDC,scaled(3,s.dpi),d.rcItem.top+scaled(2,s.dpi),a.as_ptr(),(a.len()-1) as i32);
                    let mut rect=RECT{left:scaled(30,s.dpi),top:d.rcItem.top,right:scaled(145,s.dpi),bottom:d.rcItem.bottom};
                    DrawTextW(d.hDC,b.as_ptr(),-1,&mut rect,DT_SINGLELINE|DT_VCENTER|DT_END_ELLIPSIS|DT_NOPREFIX);
                    rect.left=scaled(150,s.dpi);rect.right=d.rcItem.right-scaled(3,s.dpi);
                    DrawTextW(d.hDC,name.as_ptr(),-1,&mut rect,DT_SINGLELINE|DT_VCENTER|DT_END_ELLIPSIS|DT_NOPREFIX);
                }
            });
            1
        },
        WM_CTLCOLORSTATIC|WM_CTLCOLOREDIT|WM_CTLCOLORLISTBOX => {
            let brush=STATE.with(|c|c.borrow().as_ref().map(|s|s.background).unwrap_or(null_mut()));
            unsafe {SetTextColor(w as HDC,0x00362a20);SetBkColor(w as HDC,0x00ffffff);}
            brush as LRESULT
        },
        WM_CLOSE => {unsafe {ShowWindow(hwnd,SW_HIDE);}0},
        QUIT => {unsafe {DestroyWindow(hwnd);}0},
        WM_DESTROY => {unsafe {UnregisterHotKey(hwnd,HOTKEY);PostQuitMessage(0);}0},
        _=>unsafe {DefWindowProcW(hwnd,msg,w,l)},
    }
}

pub fn run()->Result<(),String> {
    let args:Vec<_>=std::env::args().collect();
    let input=if let Some(i)=args.iter().position(|a|a=="--data") {
        let p=args.get(i+1).ok_or("--data needs a UTF-8 TSV path")?;
        std::fs::read_to_string(p).map_err(|e|format!("Cannot read private dataset: {e}"))?
    }else {
        "notepad\t记事本\tnotepad.exe\t7\nCalc\t计算器\tcalc\t5\nExplorer\t资源管理器\texplorer.exe\t4\ncmd\t命令提示符\tcmd /k %p\t6\nb\t百度搜索\thttps://www.baidu.com/s?wd=%p\t12\ng\tGoogle 搜索\thttps://www.google.com/search?q=%p\t14\n".into()
    };
    let entries=parse_entries(&input)?;let index=SearchIndex::new(&entries);
    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let instance=GetModuleHandleW(std::ptr::null());
        let class=wide("ALTRun.Native.PerformanceProbe");
        let background=CreateSolidBrush(0x00ffffff);let selected_brush=CreateSolidBrush(0x00fceee5);
        let mut wc:WNDCLASSW=zeroed();wc.lpfnWndProc=Some(window_proc);wc.hInstance=instance;
        wc.lpszClassName=class.as_ptr();wc.hbrBackground=background;wc.hCursor=LoadCursorW(null_mut(),IDC_ARROW);
        if RegisterClassW(&wc)==0 {return Err("Cannot register native window".into());}
        let hwnd=CreateWindowExW(WS_EX_TOOLWINDOW|WS_EX_TOPMOST,class.as_ptr(),wide("ALTRun performance probe (safe mode)").as_ptr(),WS_POPUP|WS_BORDER|WS_CLIPCHILDREN,400,220,420,280,null_mut(),null_mut(),instance,std::ptr::null());
        if hwnd.is_null(){return Err("Cannot create native window".into());}
        let dpi=GetDpiForWindow(hwnd).max(96) as i32;
        let font=CreateFontW(-scaled(13,dpi),0,0,0,400,0,0,0,DEFAULT_CHARSET as u32,OUT_DEFAULT_PRECIS as u32,CLIP_DEFAULT_PRECIS as u32,CLEARTYPE_QUALITY as u32,0,wide("Microsoft YaHei UI").as_ptr());
        let control=|class_name:&str,style:u32,x,y,width,height,id:usize|CreateWindowExW(0,wide(class_name).as_ptr(),wide("").as_ptr(),WS_CHILD|WS_VISIBLE|style,scaled(x,dpi),scaled(y,dpi),scaled(width,dpi),scaled(height,dpi),hwnd,id as HMENU,instance,std::ptr::null());
        let title=control("STATIC",STATIC_CENTER,5,3,410,22,1000);
        let edit=control("EDIT",WS_BORDER|ES_AUTOHSCROLL as u32,5,27,410,25,ID_EDIT);
        let list=control("LISTBOX",LBS_NOTIFY as u32|LBS_OWNERDRAWFIXED as u32|LBS_NODATA as u32|LBS_NOINTEGRALHEIGHT as u32,5,54,410,120,ID_LIST);
        let command=control("EDIT",ES_AUTOHSCROLL as u32|ES_READONLY as u32,7,178,406,20,1003);
        if [title,edit,list,command].iter().any(|h|h.is_null()){return Err("Cannot create native controls".into());}
        for child in [title,edit,list,command] {SendMessageW(child,WM_SETFONT,font as WPARAM,0);}
        SendMessageW(list,LB_SETITEMHEIGHT,0,scaled(20,dpi) as LPARAM);
        let old_edit=SetWindowLongPtrW(edit,GWLP_WNDPROC,edit_proc as *const () as isize);
        STATE.with(|c|*c.borrow_mut()=Some(State{entries,index,hits:Vec::new(),selected:0,hwnd,edit,list,title,command,font,background,selected_brush,old_edit,dpi,allow_launch:args.iter().any(|a|a=="--allow-safe-notepad"),composing:false}));
        refresh();
        // Deliberately separate from the original's Alt+R / Pause binding.
        if RegisterHotKey(hwnd,HOTKEY,MOD_CONTROL|MOD_ALT|MOD_NOREPEAT,b'R' as u32)==0 {
            DestroyWindow(hwnd);return Err("Ctrl+Alt+R is in use; original hotkeys were not changed.".into());
        }
        if args.iter().any(|a|a=="--show") {SendMessageW(hwnd,SHOW,0,0);}
        let mut message:MSG=zeroed();
        loop {
            let result=GetMessageW(&mut message,null_mut(),0,0);
            if result<=0 {break;}
            TranslateMessage(&message);DispatchMessageW(&message);
        }
        STATE.with(|c|{if let Some(s)=c.borrow_mut().take(){DeleteObject(s.font);DeleteObject(s.selected_brush);DeleteObject(s.background);}});
        UnregisterClassW(class.as_ptr(),instance);
    }
    Ok(())
}
