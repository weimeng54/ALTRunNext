#![cfg(windows)]
use std::{process::{Child,Command},thread,time::{Duration,Instant,SystemTime,UNIX_EPOCH}};
use windows_sys::Win32::{Foundation::HWND, UI::WindowsAndMessaging::*};
#[path="native_snapshot/mod.rs"] mod native_snapshot;
fn wide(s:&str)->Vec<u16>{s.encode_utf16().chain(Some(0)).collect()}
struct Running(Child);impl Drop for Running{fn drop(&mut self){let _=self.0.kill();let _=self.0.wait();}}
fn wait(mut condition:impl FnMut()->bool){let deadline=Instant::now()+Duration::from_secs(10);while !condition(){assert!(Instant::now()<deadline,"native management action timed out");thread::sleep(Duration::from_millis(20));}}
fn window(class:&str,pid:u32)->HWND{unsafe{let hwnd=FindWindowW(wide(class).as_ptr(),std::ptr::null());let mut owner=0;GetWindowThreadProcessId(hwnd,&mut owner);if owner==pid{hwnd}else{std::ptr::null_mut()}}}
fn set(hwnd:HWND,value:&str){unsafe{assert_ne!(SendMessageW(hwnd,WM_SETTEXT,0,wide(value).as_ptr() as isize),0);}}
#[test]
fn management_save_persists_drafts_and_refreshes_resident_search(){
    let stamp=SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let path=std::env::current_dir().unwrap().join(format!("target/manager-fixture-{stamp}.tsv"));
    let original="np\t记事本\tnotepad.exe\t7\tnone\nweb\t网页搜索\thttps://example.com/?q={%p}\t2\tutf8_query\n";
    std::fs::write(&path,original).unwrap();
    let child=Running(Command::new(env!("CARGO_BIN_EXE_altrun-probe")).arg("--show").arg("--data").arg(&path).spawn().unwrap());
    let pid=child.0.id();let mut root=std::ptr::null_mut();
    wait(||{root=window("ALTRun.Native.PerformanceProbe",pid);!root.is_null()&&unsafe{!GetDlgItem(root,1001).is_null()}});
    unsafe{SendMessageW(root,WM_COMMAND,18003,0);}
    let mut manager=std::ptr::null_mut();
    wait(||{manager=window("ALTRun.Native.ShortcutManager",pid);!manager.is_null()&&unsafe{IsWindowVisible(manager)!=0&&!GetDlgItem(manager,2102).is_null()}});
    unsafe{
        let alias=GetDlgItem(manager,2102);let name=GetDlgItem(manager,2103);
        set(alias,"new");set(name,"修改后的中文名称");
        SendMessageW(manager,WM_COMMAND,2113,0);
        let saved=altrun_probe::library::Snapshot::open(&path).unwrap();assert_eq!(saved.entries[0].alias,"new");assert_eq!(saved.entries[0].name,"修改后的中文名称");
        assert_eq!(std::fs::read_to_string(path.with_extension("tsv.bak")).unwrap(),original);
        let edit=GetDlgItem(root,1001);let list=GetDlgItem(root,1002);set(edit,"new");wait(||SendMessageW(list,LB_GETCOUNT,0,0)==1);
        SendMessageW(manager,WM_COMMAND,2110,0);
        set(alias,"added");set(name,"新添加的快捷项");set(GetDlgItem(manager,2104),"notepad.exe");
        SendMessageW(manager,WM_COMMAND,2113,0);
        assert_eq!(altrun_probe::library::Snapshot::open(&path).unwrap().entries.len(),3);
        set(edit,"added");wait(||SendMessageW(list,LB_GETCOUNT,0,0)==1);
        native_snapshot::save(manager,"manager");
        SendMessageW(manager,WM_COMMAND,2114,0);wait(||IsWindow(manager)==0);
        SendMessageW(root,WM_COMMAND,18003,0);
        wait(||{manager=window("ALTRun.Native.ShortcutManager",pid);!manager.is_null()&&IsWindowVisible(manager)!=0});
        assert_eq!(SendMessageW(GetDlgItem(manager,2101),LB_GETCOUNT,0,0),3);
        SendMessageW(manager,WM_COMMAND,2114,0);wait(||IsWindow(manager)==0);
    }
}
