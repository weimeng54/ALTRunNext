#![cfg(windows)]
use std::{
    process::{Child, Command},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use windows_sys::Win32::{
    Foundation::{HWND, RECT},
    Graphics::Gdi::*,
    UI::{
        HiDpi::GetDpiForWindow, Input::KeyboardAndMouse::IsWindowEnabled, WindowsAndMessaging::*,
    },
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
fn read(hwnd: HWND) -> String {
    unsafe {
        let length = SendMessageW(hwnd, WM_GETTEXTLENGTH, 0, 0) as usize;
        let mut buffer = vec![0u16; length + 1];
        SendMessageW(hwnd, WM_GETTEXT, buffer.len(), buffer.as_mut_ptr() as isize);
        String::from_utf16(&buffer[..length]).unwrap()
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
    let original = "web\t网页搜索\thttps://example.com/?q={%p}\t2\tutf8_query\nnp\t记事本\tnotepad.exe\t7\tauto\n";
    std::fs::write(&path, original).unwrap();
    let stem = path.file_stem().unwrap().to_str().unwrap();
    std::fs::write(
        path.with_file_name(format!("{stem}.settings.ini")),
        "[Config]\nManWinWidth=500\nManWinHeight=360\n",
    )
    .unwrap();
    std::fs::write(
        path.with_file_name(format!("{stem}.state.tsv")),
        "W\tmanager\t10\t10\t600\t430\n",
    )
    .unwrap();
    let exe = std::env::var_os("ALTRUN_RELEASE_SMOKE_EXE")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_ALTRunNext").into());
    let child = Running(
        Command::new(exe)
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
        // In-window toolbar and the command form stay available at the shared size.
        assert!(GetMenu(manager).is_null());
        let style = GetWindowLongW(manager, GWL_STYLE) as u32;
        assert_eq!(style & WS_THICKFRAME, 0);
        assert_ne!(style & WS_CLIPCHILDREN, 0);
        let mut monitor = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..std::mem::zeroed()
        };
        assert_ne!(
            GetMonitorInfoW(
                MonitorFromWindow(manager, MONITOR_DEFAULTTONEAREST),
                &mut monitor,
            ),
            0
        );
        let work = monitor.rcWork;
        let dpi = (GetDpiForWindow(manager).max(96) as i32)
            .min((work.right - work.left - 48) * 96 / 1102)
            .min((work.bottom - work.top - 80) * 96 / 721)
            .max(48);
        let mut client = RECT::default();
        GetClientRect(manager, &mut client);
        assert_eq!(
            (client.right, client.bottom),
            ((1102 * dpi + 48) / 96, (721 * dpi + 48) / 96),
            "stale size from the profile must not shrink the editor"
        );
        assert_ne!(IsWindowVisible(GetDlgItem(manager, 2140)), 0);
        assert_ne!(IsWindowVisible(GetDlgItem(manager, 2141)), 0);
        let editor_list = GetDlgItem(manager, 2101);
        let save = GetDlgItem(manager, 2113);
        let alias = GetDlgItem(manager, 2102);
        let parameter_type = GetDlgItem(manager, 2105);
        let ask_parameter = GetDlgItem(manager, 2139);
        assert!(GetDlgItem(manager, 2116).is_null());
        assert_eq!(SendMessageW(parameter_type, CB_GETCOUNT, 0, 0), 3);
        let select_row = |row| {
            SendMessageW(editor_list, LB_SETCURSEL, row, 0);
            SendMessageW(
                manager,
                WM_COMMAND,
                (LBN_SELCHANGE as usize) << 16 | 2101,
                editor_list as isize,
            );
        };
        select_row(0);
        assert_eq!(read(alias), "web");
        assert_ne!(IsWindowVisible(parameter_type), 0);
        assert_eq!(SendMessageW(parameter_type, CB_GETCURSEL, 0, 0), 1);
        SendMessageW(ask_parameter, BM_CLICK, 0, 0);
        assert_eq!(SendMessageW(ask_parameter, BM_GETCHECK, 0, 0), 1);
        SendMessageW(manager, WM_COMMAND, 2132, 0);
        assert_eq!(SendMessageW(editor_list, LB_GETCURSEL, 0, 0), 1);
        assert_eq!(read(alias), "web");
        assert_eq!(IsWindowEnabled(save), 0);
        SendMessageW(manager, WM_COMMAND, 2135, 0);
        assert_eq!(SendMessageW(editor_list, LB_GETCURSEL, 0, 0), 1);
        assert_eq!(IsWindowEnabled(save), 0);
        select_row(0);
        assert_eq!(read(alias), "np");
        set(alias, "zzz");
        assert_eq!(SendMessageW(editor_list, LB_GETCURSEL, 0, 0), 1);
        assert_eq!(read(alias), "zzz");
        SendMessageW(manager, WM_COMMAND, 2133, 0);
        assert_eq!(read(alias), "zzz");
        SendMessageW(manager, WM_COMMAND, 2130, 0);
        assert_eq!(SendMessageW(editor_list, LB_GETCURSEL, 0, 0), 1);
        assert_eq!(read(alias), "zzz");
        set(alias, "np");
        SendMessageW(
            manager,
            WM_COMMAND,
            (EN_CHANGE as usize) << 16 | 2102,
            alias as isize,
        );
        assert_eq!(IsWindowEnabled(save), 0);
        select_row(1);
        assert_eq!(IsWindowVisible(parameter_type), 0);
        assert_eq!(SendMessageW(parameter_type, CB_GETCURSEL, 0, 0), 0);
        let command = GetDlgItem(manager, 2104);
        set(command, "notepad.exe {%p}");
        assert_ne!(IsWindowVisible(parameter_type), 0);
        assert_eq!(SendMessageW(parameter_type, CB_GETCURSEL, 0, 0), 0);
        set(command, "notepad.exe");
        assert_eq!(SendMessageW(parameter_type, CB_GETCURSEL, 0, 0), 0);
        assert_eq!(IsWindowVisible(parameter_type), 0);
        assert_eq!(IsWindowEnabled(save), 0);
        let saved_alias = std::fs::read_to_string(&path).unwrap();
        let first_alias = saved_alias
            .lines()
            .nth(1)
            .unwrap()
            .split('\t')
            .next()
            .unwrap();
        set(alias, "temporary-test-edit");
        assert_ne!(IsWindowEnabled(save), 0);
        set(alias, first_alias);
        assert_eq!(IsWindowEnabled(save), 0);
        let row = SendMessageW(editor_list, LB_GETITEMHEIGHT, 0, 0) as i32;
        GetClientRect(editor_list, &mut client);
        assert!(row > 0 && client.bottom % row == 0);
        assert!(GetDlgItem(manager, 2117).is_null());
        assert!(GetDlgItem(manager, 2151).is_null());
        assert_ne!(IsWindowVisible(GetDlgItem(manager, 2104)), 0);
        let alias = GetDlgItem(manager, 2102);
        let name = GetDlgItem(manager, 2103);
        set(alias, "new");
        set(name, "修改后的中文名称");
        SendMessageW(manager, WM_COMMAND, 2113, 0);
        let saved = altrun_probe::library::Snapshot::open(&path).unwrap();
        assert_eq!(saved.entries[0].alias, "web");
        assert_eq!(saved.entries[1].alias, "new");
        assert_eq!(saved.entries[1].name, "修改后的中文名称");
        assert_eq!(saved.entries[1].param_type.name(), "auto");
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
        set(GetDlgItem(manager, 2104), "https://example.com/?q=");
        assert_eq!(IsWindowVisible(parameter_type), 0);
        SendMessageW(ask_parameter, BM_CLICK, 0, 0);
        assert_ne!(IsWindowVisible(parameter_type), 0);
        assert_eq!(SendMessageW(parameter_type, CB_GETCURSEL, 0, 0), 1);
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
