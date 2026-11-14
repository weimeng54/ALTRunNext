#![cfg(windows)]
use std::{
    process::{Child, Command},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use windows_sys::Win32::{
    Foundation::{HWND, POINT, RECT},
    Graphics::Gdi::*,
    UI::{
        Controls::*,
        HiDpi::GetDpiForWindow,
        Input::KeyboardAndMouse::{
            IsWindowEnabled, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, mouse_event,
        },
        WindowsAndMessaging::*,
    },
};
static DESKTOP: std::sync::Mutex<()> = std::sync::Mutex::new(());
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
fn selected(list: HWND) -> isize {
    unsafe { SendMessageW(list, LVM_GETNEXTITEM, usize::MAX, LVNI_SELECTED as isize) }
}

fn resize_columns(list: HWND, header: HWND, dpi: i32) {
    unsafe {
        let column_width = |column| SendMessageW(list, LVM_GETCOLUMNWIDTH, column, 0) as i32;
        let mouse = |message, button, x: i32| {
            SendMessageW(header, message, button, ((8 << 16) | x) as isize)
        };
        let first = column_width(0);
        let divider = first - 1;
        let mut origin = POINT::default();
        assert_ne!(ClientToScreen(header, &mut origin), 0);
        let point = POINT {
            x: origin.x + divider,
            y: origin.y + 8,
        };
        assert_ne!(SetCursorPos(point.x, point.y), 0);
        assert_eq!(WindowFromPoint(point), header);
        let thread = GetWindowThreadProcessId(header, std::ptr::null_mut());
        let mut gui = GUITHREADINFO {
            cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
            ..Default::default()
        };
        mouse_event(MOUSEEVENTF_LEFTDOWN, 0, 0, 0, 0);
        let deadline = Instant::now() + Duration::from_secs(2);
        while GetGUIThreadInfo(thread, &mut gui) != 0
            && gui.hwndCapture != header
            && Instant::now() < deadline
        {
            thread::sleep(Duration::from_millis(10));
        }
        let captured = gui.hwndCapture == header;
        let moved = SetCursorPos(point.x + dpi, point.y);
        if moved != 0 && captured {
            let deadline = Instant::now() + Duration::from_secs(2);
            while column_width(0) < first + dpi - 2 && Instant::now() < deadline {
                thread::sleep(Duration::from_millis(10));
            }
        }
        mouse_event(MOUSEEVENTF_LEFTUP, 0, 0, 0, 0);
        assert!(
            captured,
            "the table header must capture the mouse at the divider"
        );
        assert_ne!(moved, 0);
        wait(|| column_width(0) >= first + dpi - 2);
        wait(|| GetGUIThreadInfo(thread, &mut gui) != 0 && gui.hwndCapture != header);
        let wider = column_width(0);
        let mut scroll = SCROLLBARINFO {
            cbSize: std::mem::size_of::<SCROLLBARINFO>() as u32,
            ..Default::default()
        };
        assert_ne!(GetScrollBarInfo(list, OBJID_HSCROLL, &mut scroll), 0);
        assert_eq!(scroll.rgstate[0] & STATE_SYSTEM_INVISIBLE, 0);
        SendMessageW(list, WM_HSCROLL, SB_RIGHT as usize, 0);
        assert!(GetScrollPos(list, SB_HORZ) > 0);
        SendMessageW(list, WM_HSCROLL, SB_LEFT as usize, 0);
        assert_eq!(GetScrollPos(list, SB_HORZ), 0);

        mouse(WM_LBUTTONDBLCLK, 1, wider - 1);
        mouse(WM_LBUTTONUP, 0, wider - 1);
        assert!(column_width(0) < wider, "double-click must fit the content");
        SendMessageW(list, LVM_SETCOLUMNWIDTH, 0, first as isize);
    }
}

fn choose_row(list: HWND, row: usize, dpi: i32) {
    // Pointer-bearing ListView messages are local-process only. Click the
    // native row via its screen coordinates to exercise selection end to end.
    unsafe {
        SendMessageW(list, LVM_ENSUREVISIBLE, row, 0);
        let header = SendMessageW(list, LVM_GETHEADER, 0, 0) as HWND;
        let mut bounds = RECT::default();
        GetWindowRect(header, &mut bounds);
        let mut point = POINT {
            x: bounds.left,
            y: bounds.bottom,
        };
        ScreenToClient(list, &mut point);
        let top = SendMessageW(list, LVM_GETTOPINDEX, 0, 0) as usize;
        let row_height = (dpi * 31 + 48) / 96;
        let y = point.y + (row - top) as i32 * row_height + row_height / 2;
        let position = ((y << 16) | 12) as isize;
        SendMessageW(list, WM_LBUTTONDOWN, 1, position);
        SendMessageW(list, WM_LBUTTONUP, 0, position);
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
    let _desktop = DESKTOP.lock().unwrap_or_else(|error| error.into_inner());
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
        let header = SendMessageW(editor_list, LVM_GETHEADER, 0, 0) as HWND;
        assert!(
            !header.is_null(),
            "the editor must expose a native table header"
        );
        assert!(SendMessageW(editor_list, LVM_GETCOLUMNWIDTH, 0, 0) > 0);
        assert!(SendMessageW(editor_list, LVM_GETCOLUMNWIDTH, 1, 0) > 0);
        resize_columns(editor_list, header, dpi);
        let save = GetDlgItem(manager, 2113);
        let alias = GetDlgItem(manager, 2102);
        let parameter_type = GetDlgItem(manager, 2105);
        let ask_parameter = GetDlgItem(manager, 2139);
        assert!(GetDlgItem(manager, 2116).is_null());
        assert_eq!(SendMessageW(parameter_type, CB_GETCOUNT, 0, 0), 3);
        let select_row = |row| choose_row(editor_list, row, dpi);
        select_row(0);
        assert_eq!(read(alias), "web");
        assert_ne!(IsWindowVisible(parameter_type), 0);
        assert_eq!(SendMessageW(parameter_type, CB_GETCURSEL, 0, 0), 1);
        SendMessageW(ask_parameter, BM_CLICK, 0, 0);
        assert_eq!(SendMessageW(ask_parameter, BM_GETCHECK, 0, 0), 1);
        SendMessageW(manager, WM_COMMAND, 2132, 0);
        assert_eq!(selected(editor_list), 1);
        assert_eq!(read(alias), "web");
        assert_eq!(IsWindowEnabled(save), 0);
        SendMessageW(manager, WM_COMMAND, 2135, 0);
        assert_eq!(selected(editor_list), 1);
        assert_eq!(IsWindowEnabled(save), 0);
        select_row(0);
        assert_eq!(read(alias), "np");
        set(alias, "zzz");
        assert_eq!(selected(editor_list), 1);
        assert_eq!(read(alias), "zzz");
        SendMessageW(manager, WM_COMMAND, 2133, 0);
        assert_eq!(read(alias), "zzz");
        SendMessageW(manager, WM_COMMAND, 2130, 0);
        assert_eq!(selected(editor_list), 1);
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
        assert_eq!(SendMessageW(editor_list, LVM_GETITEMCOUNT, 0, 0), 2);
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
        let draft_row = selected(editor_list);
        assert_eq!(draft_row, 2);
        assert!(read(alias).is_empty());
        assert!(read(name).is_empty());
        assert!(read(GetDlgItem(manager, 2104)).is_empty());
        native_snapshot::save(manager, "manager-blank-draft");
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
            SendMessageW(GetDlgItem(manager, 2101), LVM_GETITEMCOUNT, 0, 0),
            3
        );
        answer_confirmation(manager, pid, WM_COMMAND, 2111, IDOK);
        assert_eq!(
            SendMessageW(GetDlgItem(manager, 2101), LVM_GETITEMCOUNT, 0, 0),
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
            SendMessageW(GetDlgItem(manager, 2101), LVM_GETITEMCOUNT, 0, 0),
            2
        );
        SendMessageW(manager, WM_COMMAND, 2114, 0);
        wait(|| IsWindow(manager) == 0);
    }
}

#[test]
fn shortcut_table_preserves_theme_and_selection_when_resized() {
    let _desktop = DESKTOP.lock().unwrap_or_else(|error| error.into_inner());
    let exe = std::env::var_os("ALTRUN_RELEASE_SMOKE_EXE")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_ALTRunNext").into());
    for (theme, scale, header_color) in [("light", "100", 0xffffff), ("dark", "150", 0x322922)] {
        let data = std::env::current_dir()
            .unwrap()
            .join(format!("target/manager-table-{theme}.tsv"));
        let mut shortcuts =
            String::from("codex-plus-plus-manager\t快捷项管理工具\tnotepad.exe\t0\n");
        for row in 1..24 {
            shortcuts.push_str(&format!(
                "example-{row:02}\t示例快捷项 {row:02}\tnotepad.exe\t0\n"
            ));
        }
        std::fs::write(&data, &shortcuts).unwrap();
        std::fs::write(
            data.with_extension("settings.ini"),
            format!("[GUI]\nLang=zh-Hans\nTheme={theme}\n"),
        )
        .unwrap();
        let child = Running(
            Command::new(&exe)
                .args(["--show", "--safe-mode", "--ui-scale", scale, "--data"])
                .arg(&data)
                .spawn()
                .unwrap(),
        );
        let pid = child.0.id();
        let mut launcher = std::ptr::null_mut();
        let mut manager = std::ptr::null_mut();
        wait(|| {
            launcher = window("ALTRun.Native.PerformanceProbe", pid);
            !launcher.is_null() && unsafe { IsWindowVisible(launcher) != 0 }
        });
        unsafe {
            SendMessageW(launcher, WM_COMMAND, 18003, 0);
            wait(|| {
                manager = window("ALTRun.Native.ShortcutManager", pid);
                !manager.is_null() && IsWindowVisible(manager) != 0
            });
            let list = GetDlgItem(manager, 2101);
            let header = SendMessageW(list, LVM_GETHEADER, 0, 0) as HWND;
            let alias = GetDlgItem(manager, 2102);
            let save = GetDlgItem(manager, 2113);
            assert_eq!(selected(list), 0);
            assert_eq!(read(alias), "codex-plus-plus-manager");
            assert_eq!(IsWindowEnabled(save), 0);
            let width = SendMessageW(list, LVM_GETCOLUMNWIDTH, 0, 0);
            assert_ne!(SendMessageW(list, LVM_SETCOLUMNWIDTH, 0, width + 75), 0);
            native_snapshot::save(manager, &format!("manager-table-{theme}-{scale}"));
            let dc = GetDC(header);
            let color = GetPixel(dc, 4, 4);
            ReleaseDC(header, dc);
            assert_eq!(color, header_color, "the header must use the manager theme");
            assert_eq!(selected(list), 0);
            assert_eq!(read(alias), "codex-plus-plus-manager");
            assert_eq!(IsWindowEnabled(save), 0);
            assert_eq!(std::fs::read_to_string(data).unwrap(), shortcuts);
            SendMessageW(manager, WM_COMMAND, 2114, 0);
            wait(|| IsWindow(manager) == 0);
        }
    }
}

#[test]
fn edit_command_targets_the_selected_id_and_preserves_existing_drafts() {
    let _desktop = DESKTOP.lock().unwrap_or_else(|error| error.into_inner());
    let data = std::env::current_dir()
        .unwrap()
        .join("target/edit-command-fixture.tsv");
    let original = "same\tExample\tnotepad.exe\t0\nsame\tExample\tcalc.exe\t0\n";
    std::fs::write(&data, original).unwrap();
    let exe = std::env::var_os("ALTRUN_RELEASE_SMOKE_EXE")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_ALTRunNext").into());
    let child = Running(
        Command::new(exe)
            .args(["--show", "--safe-mode", "--data"])
            .arg(&data)
            .spawn()
            .unwrap(),
    );
    let pid = child.0.id();
    let mut launcher = std::ptr::null_mut();
    wait(|| {
        launcher = window("ALTRun.Native.PerformanceProbe", pid);
        !launcher.is_null() && unsafe { IsWindowVisible(launcher) != 0 }
    });
    unsafe {
        SendMessageW(launcher, WM_COMMAND, 18003, 0);
        let mut manager = std::ptr::null_mut();
        wait(|| {
            manager = window("ALTRun.Native.ShortcutManager", pid);
            !manager.is_null() && IsWindowVisible(manager) != 0
        });
        let command = GetDlgItem(manager, 2104);
        assert_eq!(read(command), "notepad.exe");
        set(command, "notepad.exe --draft");
        // Reopen the launcher, select the second of two identical aliases,
        // and dispatch the command used by its context menu.
        SendMessageW(launcher, WM_COMMAND, 18012, 0);
        let edit = GetDlgItem(launcher, 1001);
        SendMessageW(edit, WM_KEYDOWN, 0x28, 0);
        SendMessageW(launcher, WM_COMMAND, 18013, 0);
        wait(|| read(command) == "calc.exe");
        assert_eq!(selected(GetDlgItem(manager, 2101)), 1);
        assert_ne!(IsWindowEnabled(GetDlgItem(manager, 2113)), 0);
        let mut gui = GUITHREADINFO {
            cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
            ..Default::default()
        };
        let thread = GetWindowThreadProcessId(manager, std::ptr::null_mut());
        wait(|| GetGUIThreadInfo(thread, &mut gui) != 0 && gui.hwndFocus == command);
        SendMessageW(launcher, WM_COMMAND, 18012, 0);
        SendMessageW(edit, WM_KEYDOWN, 0x71, 0);
        wait(|| read(command) == "notepad.exe --draft");
        assert_eq!(std::fs::read_to_string(data).unwrap(), original);
    }
}
