//! Common legacy WinCtl actions, implemented with documented Win32 messages.
use std::sync::Mutex;
use windows_sys::Win32::{Foundation::*,UI::WindowsAndMessaging::*};
static HIDDEN:Mutex<Vec<usize>>=Mutex::new(Vec::new());
static MINIMIZED:Mutex<Vec<usize>>=Mutex::new(Vec::new());
pub fn execute(file:&str,parameters:&str,foreground:usize)->Option<Result<(),String>>{
    if !file.rsplit(['\\','/']).next().is_some_and(|name|name.eq_ignore_ascii_case("winctl.exe")){return None;}
    let verb=parameters.split_whitespace().next().unwrap_or("").to_ascii_lowercase();
    unsafe{match verb.as_str(){
        "minall"=>{unsafe extern "system" fn minimize(hwnd:HWND,value:LPARAM)->i32{unsafe{let mut pid=0;GetWindowThreadProcessId(hwnd,&mut pid);if IsWindowVisible(hwnd)!=0&&IsIconic(hwnd)==0&&pid!=std::process::id()&&GetWindowLongW(hwnd,GWL_STYLE)as u32&WS_CAPTION!=0&&GetWindow(hwnd,GW_OWNER).is_null(){let list=&mut *(value as *mut Vec<usize>);list.push(hwnd as usize);ShowWindowAsync(hwnd,SW_MINIMIZE);}}1}let mut windows=Vec::<usize>::new();EnumWindows(Some(minimize),&mut windows as *mut _ as isize);if let Ok(mut list)=MINIMIZED.lock(){*list=windows;}Some(Ok(()))},
        "undominall"=>{if let Ok(mut list)=MINIMIZED.lock(){for hwnd in list.drain(..){if IsWindow(hwnd as HWND)!=0{ShowWindowAsync(hwnd as HWND,SW_RESTORE);}}}Some(Ok(()))},
        "hide"=>{let hwnd=foreground as HWND;let mut pid=0;GetWindowThreadProcessId(hwnd,&mut pid);if hwnd.is_null()||IsWindow(hwnd)==0||pid==std::process::id(){return Some(Err("没有可隐藏的外部窗口".into()));}ShowWindowAsync(hwnd,SW_HIDE);if let Ok(mut hidden)=HIDDEN.lock(){hidden.retain(|h|*h!=foreground);hidden.push(foreground);}Some(Ok(()))},
        "unhide"|"show" if parameters.split_whitespace().count()==1=>{if let Ok(mut hidden)=HIDDEN.lock(){hidden.retain(|handle|IsWindow(*handle as HWND)!=0);if let Some(hwnd)=hidden.pop(){ShowWindowAsync(hwnd as HWND,SW_SHOW);SetForegroundWindow(hwnd as HWND);}}Some(Ok(()))},
        // Unknown external helper commands retain their original executable and
        // parameters, rather than guessing a destructive window operation.
        _=>None
    }}
}
