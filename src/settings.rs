//! Lazy native property editor, running independently of the launch window.
use crate::native::{wide,make_font};
use altrun_probe::{preferences::{Preferences,SPECS,Kind},state_store::TextFile};
use std::{cell::RefCell,path::PathBuf,ptr::null_mut,sync::{mpsc,atomic::{AtomicUsize,Ordering}}};
use windows_sys::Win32::{Foundation::*,Graphics::Gdi::*,System::LibraryLoader::GetModuleHandleW,UI::{WindowsAndMessaging::*,Controls::*,Controls::Dialogs::*}};
pub const UPDATED:u32=WM_APP+12;
pub enum Change{Prepare(u64,Preferences,mpsc::Sender<Result<(),String>>),Commit(u64),Cancel(u64)}
static WINDOW:AtomicUsize=AtomicUsize::new(0);
struct Editor{prefs:Preferences,file:TextFile,list:HWND,value:HWND,status:HWND,index:usize,dirty:bool,loading:bool,font:HFONT,sender:mpsc::Sender<Change>,owner:usize}
thread_local!{static EDITOR:RefCell<Option<Editor>>=const{RefCell::new(None)};}
fn text(hwnd:HWND)->String{unsafe{let mut b=vec![0;GetWindowTextLengthW(hwnd) as usize+1];let n=GetWindowTextW(hwnd,b.as_mut_ptr(),b.len() as i32);String::from_utf16_lossy(&b[..n as usize])}}
fn status(message:&str){let h=EDITOR.with(|c|c.borrow().as_ref().unwrap().status);unsafe{SetWindowTextW(h,wide(message).as_ptr());}}
fn select(){let(value,index,prefs)=EDITOR.with(|c|{let mut s=c.borrow_mut();let s=s.as_mut().unwrap();s.loading=true;(s.value,s.index,s.prefs.clone())});unsafe{SetWindowTextW(value,wide(&prefs.get(SPECS[index].key)).as_ptr());}EDITOR.with(|c|c.borrow_mut().as_mut().unwrap().loading=false);let spec=&SPECS[index];status(&format!("{} · {}",spec.key,match spec.kind{Kind::Bool=>"0 = 关闭，1 = 开启",Kind::Hotkey=>"例如 Ctrl+Alt+R；留空可关闭",Kind::Font=>"字体|[fsBold,fsItalic]|字号|颜色|字符集",Kind::Number(_,_)=>"输入整数",Kind::Text=>"输入文本；颜色格式 #RRGGBB"}));}
fn store_value()->Result<(),String>{let(h,index,loading)=EDITOR.with(|c|{let s=c.borrow();let s=s.as_ref().unwrap();(s.value,s.index,s.loading)});if loading{return Ok(());}let value=text(h);EDITOR.with(|c|{let mut s=c.borrow_mut();let s=s.as_mut().unwrap();s.prefs.set(SPECS[index].key,&value)?;s.dirty=true;Ok(())})}
fn apply(){
    if let Err(e)=store_value(){status(&e);return;}
    let(prefs,sender,owner)=EDITOR.with(|c|{let s=c.borrow();let s=s.as_ref().unwrap();(s.prefs.clone(),s.sender.clone(),s.owner)});
    static NEXT:AtomicUsize=AtomicUsize::new(1);let token=NEXT.fetch_add(1,Ordering::Relaxed) as u64;let(tx,rx)=mpsc::channel();
    let _=sender.send(Change::Prepare(token,prefs.clone(),tx));unsafe{PostMessageW(owner as HWND,UPDATED,0,0);}
    let prepared=rx.recv_timeout(std::time::Duration::from_secs(5)).map_err(|_|"启动面板暂时未响应".to_owned()).and_then(|v|v);
    let result=prepared.and_then(|_|EDITOR.with(|c|{let mut s=c.borrow_mut();let s=s.as_mut().unwrap();s.file.save(&prefs.text())}));
    match result{Ok(())=>{let _=sender.send(Change::Commit(token));EDITOR.with(|c|c.borrow_mut().as_mut().unwrap().dirty=false);status("已保存并应用。热键冲突会保留原有可用设置。");},Err(e)=>{let _=sender.send(Change::Cancel(token));status(&e);}}
    unsafe{PostMessageW(owner as HWND,UPDATED,0,0);}
}
fn choose_file(save:bool)->Option<PathBuf>{unsafe{let mut b=vec![0u16;32768];let mut d:OPENFILENAMEW=std::mem::zeroed();d.lStructSize=std::mem::size_of_val(&d) as u32;d.hwndOwner=WINDOW.load(Ordering::Acquire) as HWND;d.lpstrFile=b.as_mut_ptr();d.nMaxFile=b.len() as u32;let filter=wide("设置文件\0*.ini\0所有文件\0*.*\0");d.lpstrFilter=filter.as_ptr();d.Flags=OFN_EXPLORER|OFN_NOCHANGEDIR|if save{OFN_OVERWRITEPROMPT}else{OFN_FILEMUSTEXIST};let ok=if save{GetSaveFileNameW(&mut d)}else{GetOpenFileNameW(&mut d)};if ok==0{None}else{Some(PathBuf::from(String::from_utf16_lossy(&b[..b.iter().position(|c|*c==0).unwrap_or(b.len())])))}}}
unsafe extern "system" fn procedure(hwnd:HWND,msg:u32,w:WPARAM,l:LPARAM)->LRESULT{
    match msg{
        WM_COMMAND=>{let id=w&65535;let code=w>>16;
            if id==2301&&code==LBN_SELCHANGE as usize{if let Err(e)=store_value(){status(&e);let(list,index)=EDITOR.with(|c|{let s=c.borrow();let s=s.as_ref().unwrap();(s.list,s.index)});unsafe{SendMessageW(list,LB_SETCURSEL,index,0);}return 0;}let list=EDITOR.with(|c|c.borrow().as_ref().unwrap().list);let n=unsafe{SendMessageW(list,LB_GETCURSEL,0,0)};if n>=0{EDITOR.with(|c|c.borrow_mut().as_mut().unwrap().index=n as usize);select();}}
            match id{2310=>apply(),2311=>{if unsafe{MessageBoxW(hwnd,wide("将恢复默认设置；点击保存后才生效。").as_ptr(),wide("恢复默认").as_ptr(),MB_OKCANCEL)}==IDOK{EDITOR.with(|c|{let mut s=c.borrow_mut();let s=s.as_mut().unwrap();s.prefs=Preferences::default();s.dirty=true;});select();}},2312=>{if let Some(path)=choose_file(false){let result=std::fs::read(&path).map_err(|e|e.to_string()).and_then(|b|altrun_probe::library::decode(&b)).and_then(|t|Preferences::parse(&t));match result{Ok(mut prefs)=>{prefs.set("AutoRun","0").unwrap();prefs.set("AddToSendTo","0").unwrap();EDITOR.with(|c|{let mut s=c.borrow_mut();let s=s.as_mut().unwrap();s.prefs=prefs;s.dirty=true;});select();status("设置已载入草稿，尚未应用。系统集成默认关闭。");},Err(e)=>status(&e)}}},2313=>{if store_value().is_ok()&&let Some(path)=choose_file(true){let t=EDITOR.with(|c|c.borrow().as_ref().unwrap().prefs.text());match TextFile::open(&path).and_then(|(mut f,_)|f.save(&t)){Ok(())=>status("设置已导出。"),Err(e)=>status(&e)}}},2314=>unsafe{SendMessageW(hwnd,WM_CLOSE,0,0);},_=>{}}0
        },
        WM_CLOSE=>{let dirty=EDITOR.with(|c|c.borrow().as_ref().is_some_and(|s|s.dirty||text(s.value)!=s.prefs.get(SPECS[s.index].key)));if dirty&&unsafe{MessageBoxW(hwnd,wide("放弃尚未保存的设置？").as_ptr(),wide("关闭设置").as_ptr(),MB_OKCANCEL|MB_DEFBUTTON2)}!=IDOK{return 0;}unsafe{DestroyWindow(hwnd);}0},
        WM_DESTROY=>{unsafe{PostQuitMessage(0);}0},_=>unsafe{DefWindowProcW(hwnd,msg,w,l)}
    }
}
pub fn open(path:PathBuf,prefs:Preferences,owner:HWND,sender:mpsc::Sender<Change>){
    if WINDOW.compare_exchange(0,1,Ordering::AcqRel,Ordering::Acquire).is_err(){let h=WINDOW.load(Ordering::Acquire);if h>1{unsafe{ShowWindow(h as HWND,SW_SHOW);SetForegroundWindow(h as HWND);}}return;}
    let owner=owner as usize;
    std::thread::spawn(move||{let result=(||->Result<(),String>{let(file,_)=TextFile::open(&path)?;unsafe{let instance=GetModuleHandleW(std::ptr::null());let class=wide("ALTRun.Native.Settings");let mut wc:WNDCLASSW=std::mem::zeroed();wc.lpfnWndProc=Some(procedure);wc.hInstance=instance;wc.lpszClassName=class.as_ptr();wc.hCursor=LoadCursorW(null_mut(),IDC_ARROW);wc.hbrBackground=(COLOR_WINDOW+1) as HBRUSH;RegisterClassW(&wc);let hwnd=CreateWindowExW(0,class.as_ptr(),wide("ALTRun · 设置").as_ptr(),WS_OVERLAPPED|WS_CAPTION|WS_SYSMENU|WS_MINIMIZEBOX, CW_USEDEFAULT,CW_USEDEFAULT,700,490,null_mut(),null_mut(),instance,std::ptr::null());if hwnd.is_null(){return Err("无法创建设置窗口".into());}WINDOW.store(hwnd as usize,Ordering::Release);let font=make_font(14,400,"Microsoft YaHei UI",96);let control=|class:&str,label:&str,style:u32,x,y,width,height,id|{let h=CreateWindowExW(0,wide(class).as_ptr(),wide(label).as_ptr(),WS_CHILD|WS_VISIBLE|style,x,y,width,height,hwnd,id as HMENU,instance,std::ptr::null());SendMessageW(h,WM_SETFONT,font as usize,0);h};
        let list=control("LISTBOX","",LBS_NOTIFY as u32|WS_VSCROLL|WS_BORDER,16,16,276,364,2301);for spec in SPECS{SendMessageW(list,LB_ADDSTRING,0,wide(spec.label).as_ptr() as isize);}control("STATIC","选中设置的值",0,310,20,350,24,0);let value=control("EDIT","",ES_AUTOHSCROLL as u32|WS_BORDER|WS_TABSTOP,310,52,350,30,2302);let status=control("STATIC","",0,310,100,350,245,2303);
        for(label,id,x,width)in[("保存并应用",2310,16,120),("恢复默认",2311,146,110),("导入…",2312,266,90),("导出…",2313,366,90),("关闭",2314,558,100)]{control("BUTTON",label,WS_TABSTOP,x,396,width,30,id);}
        EDITOR.with(|c|*c.borrow_mut()=Some(Editor{prefs,file,list,value,status,index:0,dirty:false,loading:false,font,sender,owner}));SendMessageW(list,LB_SETCURSEL,0,0);select();ShowWindow(hwnd,SW_SHOW);let mut msg:MSG=std::mem::zeroed();while GetMessageW(&mut msg,null_mut(),0,0)>0{if IsDialogMessageW(hwnd,&msg)==0{TranslateMessage(&msg);DispatchMessageW(&msg);}}EDITOR.with(|c|{if let Some(e)=c.borrow_mut().take(){DeleteObject(e.font);}});UnregisterClassW(class.as_ptr(),instance);}Ok(())})();if let Err(e)=result{crate::native::show_error(&e);}WINDOW.store(0,Ordering::Release);});
}
pub fn is_open()->bool{WINDOW.load(Ordering::Acquire)!=0}
