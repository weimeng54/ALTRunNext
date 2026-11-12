#![cfg(windows)]
use std::{
    process::{Child, Command},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use windows_sys::Win32::{
    Foundation::{HWND, RECT},
    Graphics::Gdi::*,
    UI::WindowsAndMessaging::*,
};
#[path = "native_snapshot/mod.rs"]
mod native_snapshot;
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
struct Running(Child);
impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn wait(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !condition() {
        assert!(
            Instant::now() < deadline,
            "native management action timed out"
        );
        thread::sleep(Duration::from_millis(20));
    }
}
fn window(class: &str, pid: u32) -> HWND {
    unsafe {
        let mut hwnd = std::ptr::null_mut();
        loop {
            hwnd = FindWindowExW(
                std::ptr::null_mut(),
                hwnd,
                wide(class).as_ptr(),
                std::ptr::null(),
            );
            if hwnd.is_null() {
                return hwnd;
            }
            let mut owner = 0;
            GetWindowThreadProcessId(hwnd, &mut owner);
            if owner == pid {
                return hwnd;
            }
        }
    }
}

fn set(hwnd: HWND, value: &str) {
    unsafe {
        assert_ne!(
            SendMessageW(hwnd, WM_SETTEXT, 0, wide(value).as_ptr() as isize),
            0
        );
    }
}
fn answer_confirmation(owner: HWND, pid: u32, message: u32, command: usize, answer: i32) {
    let address = owner as usize;
    let pending =
        thread::spawn(move || unsafe { SendMessageW(address as HWND, message, command, 0) });
    let mut dialog = std::ptr::null_mut();
    wait(|| {
        dialog = window("ALTRun.Native.Confirmation", pid);
        !dialog.is_null()
            && unsafe { GetWindow(dialog, GW_OWNER) == owner && IsWindowVisible(dialog) != 0 }
    });
    unsafe {
        let button = GetDlgItem(dialog, answer);
        assert!(!button.is_null());
        SendMessageW(button, BM_CLICK, 0, 0);
    }
    pending.join().unwrap();
    wait(|| unsafe { IsWindow(dialog) == 0 });
}

#[test]
fn management_save_persists_drafts_and_refreshes_resident_search() {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::current_dir()
        .unwrap()
        .join(format!("target/manager-fixture-{stamp}.tsv"));
    let original = "np\t记事本\tnotepad.exe\t7\tnone\nweb\t网页搜索\thttps://example.com/?q={%p}\t2\tutf8_query\n";
    std::fs::write(&path, original).unwrap();
    let child = Running(
        Command::new(env!("CARGO_BIN_EXE_altrun-probe"))
            .arg("--show")
            .arg("--data")
            .arg(&path)
            .spawn()
            .unwrap(),
    );
    let pid = child.0.id();
    let mut root = std::ptr::null_mut();
    wait(|| {
        root = window("ALTRun.Native.PerformanceProbe", pid);
        !root.is_null() && unsafe { !GetDlgItem(root, 1001).is_null() }
    });
    unsafe {
        SendMessageW(root, WM_COMMAND, 18003, 0);
    }
    let mut manager = std::ptr::null_mut();
    wait(|| {
        manager = window("ALTRun.Native.ShortcutManager", pid);
        !manager.is_null()
            && unsafe { IsWindowVisible(manager) != 0 && !GetDlgItem(manager, 2102).is_null() }
    });
    unsafe {
        // In-window toolbar and type segments keep the existing draft controls usable.
        assert!(GetMenu(manager).is_null());
        assert_ne!(IsWindowVisible(GetDlgItem(manager, 2140)), 0);
        assert_ne!(IsWindowVisible(GetDlgItem(manager, 2141)), 0);
        SendMessageW(manager, WM_COMMAND, 2152, 0);
        assert_eq!(IsWindowVisible(GetDlgItem(manager, 2104)), 0);
        assert_ne!(IsWindowVisible(GetDlgItem(manager, 2103)), 0);
        assert_eq!(
            SendMessageW(GetDlgItem(manager, 2117), CB_GETCURSEL, 0, 0),
            2
        );
        SendMessageW(manager, WM_COMMAND, 2151, 0);
        assert_eq!(IsWindowVisible(GetDlgItem(manager, 2103)), 0);
        assert_ne!(IsWindowVisible(GetDlgItem(manager, 2162)), 0);
        SendMessageW(manager, WM_COMMAND, 2150, 0);
        assert_ne!(IsWindowVisible(GetDlgItem(manager, 2104)), 0);
        let alias = GetDlgItem(manager, 2102);
        let name = GetDlgItem(manager, 2103);
        set(alias, "new");
        set(name, "修改后的中文名称");
        SendMessageW(manager, WM_COMMAND, 2113, 0);
        let saved = altrun_probe::library::Snapshot::open(&path).unwrap();
        assert_eq!(saved.entries[0].alias, "new");
        assert_eq!(saved.entries[0].name, "修改后的中文名称");
        assert_eq!(
            std::fs::read_to_string(path.with_extension("tsv.bak")).unwrap(),
            original
        );
        let edit = GetDlgItem(root, 1001);
        let list = GetDlgItem(root, 1002);
        set(edit, "new");
        wait(|| SendMessageW(list, LB_GETCOUNT, 0, 0) == 1);
        SendMessageW(manager, WM_COMMAND, 2110, 0);
        set(alias, "added");
        set(name, "新添加的快捷项");
        set(GetDlgItem(manager, 2104), "notepad.exe");
        SendMessageW(manager, WM_COMMAND, 2113, 0);
        assert_eq!(
            altrun_probe::library::Snapshot::open(&path)
                .unwrap()
                .entries
                .len(),
            3
        );
        set(edit, "added");
        wait(|| SendMessageW(list, LB_GETCOUNT, 0, 0) == 1);
        native_snapshot::save(manager, "manager");
        let mut bounds: RECT = std::mem::zeroed();
        GetWindowRect(manager, &mut bounds);
        let mut monitor: MONITORINFO = std::mem::zeroed();
        monitor.cbSize = std::mem::size_of_val(&monitor) as u32;
        assert_ne!(
            GetMonitorInfoW(
                MonitorFromWindow(manager, MONITOR_DEFAULTTONEAREST),
                &mut monitor
            ),
            0
        );
        assert!(
            bounds.left >= monitor.rcWork.left
                && bounds.right <= monitor.rcWork.right
                && bounds.top >= monitor.rcWork.top
                && bounds.bottom <= monitor.rcWork.bottom,
            "manager controls must fit the work area"
        );
        answer_confirmation(manager, pid, WM_COMMAND, 2111, IDCANCEL);
        assert_eq!(
            SendMessageW(GetDlgItem(manager, 2101), LB_GETCOUNT, 0, 0),
            3
        );
        answer_confirmation(manager, pid, WM_COMMAND, 2111, IDOK);
        assert_eq!(
            SendMessageW(GetDlgItem(manager, 2101), LB_GETCOUNT, 0, 0),
            2
        );
        assert_eq!(
            altrun_probe::library::Snapshot::open(&path)
                .unwrap()
                .entries
                .len(),
            3
        );
        answer_confirmation(manager, pid, WM_CLOSE, 0, IDCANCEL);
        assert_ne!(IsWindowVisible(manager), 0);
        SendMessageW(manager, WM_COMMAND, 2113, 0);
        assert_eq!(
            altrun_probe::library::Snapshot::open(&path)
                .unwrap()
                .entries
                .len(),
            2
        );
        wait(|| SendMessageW(list, LB_GETCOUNT, 0, 0) == 0);
        SendMessageW(manager, WM_COMMAND, 2114, 0);
        wait(|| IsWindow(manager) == 0);
        SendMessageW(root, WM_COMMAND, 18003, 0);
        wait(|| {
            manager = window("ALTRun.Native.ShortcutManager", pid);
            !manager.is_null() && IsWindowVisible(manager) != 0
        });
        assert_eq!(
            SendMessageW(GetDlgItem(manager, 2101), LB_GETCOUNT, 0, 0),
            2
        );
        SendMessageW(manager, WM_COMMAND, 2114, 0);
        wait(|| IsWindow(manager) == 0);
    }
}
