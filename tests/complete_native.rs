#![cfg(windows)]
use std::os::windows::io::AsRawHandle;
use std::{
    process::{Child, Command},
    ptr::null_mut,
    thread,
    time::{Duration, Instant},
};
use windows_sys::Win32::System::{DataExchange::*, Memory::*};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{RegisterHotKey, UnregisterHotKey};
use windows_sys::Win32::{Foundation::*, UI::WindowsAndMessaging::*};
#[repr(C)]
#[derive(Default)]
struct Memory {
    size: u32,
    faults: u32,
    peak_working: usize,
    working: usize,
    peak_paged: usize,
    paged: usize,
    peak_nonpaged: usize,
    nonpaged: usize,
    pagefile: usize,
    peak_pagefile: usize,
    private: usize,
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn K32GetProcessMemoryInfo(process: HANDLE, memory: *mut Memory, size: u32) -> i32;
}
#[link(name = "user32")]
unsafe extern "system" {
    fn GetGuiResources(process: HANDLE, flags: u32) -> u32;
}
fn resources(child: &Child) -> (usize, u32, u32) {
    unsafe {
        let process = child.as_raw_handle() as HANDLE;
        let mut memory = Memory {
            size: std::mem::size_of::<Memory>() as u32,
            ..Default::default()
        };
        assert_ne!(
            K32GetProcessMemoryInfo(process, &mut memory, std::mem::size_of::<Memory>() as u32),
            0
        );
        (
            memory.private,
            GetGuiResources(process, 0),
            GetGuiResources(process, 1),
        )
    }
}
mod native_snapshot;
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
#[track_caller]
fn wait(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !condition() {
        assert!(Instant::now() < deadline, "native operation timed out");
        thread::sleep(Duration::from_millis(15));
    }
}
struct Running(Child);
impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn window(class: &str, pid: u32) -> HWND {
    unsafe {
        let mut hwnd = FindWindowExW(
            null_mut(),
            null_mut(),
            wide(class).as_ptr(),
            std::ptr::null(),
        );
        while !hwnd.is_null() {
            let mut owner = 0;
            GetWindowThreadProcessId(hwnd, &mut owner);
            if owner == pid {
                return hwnd;
            }
            hwnd = FindWindowExW(null_mut(), hwnd, wide(class).as_ptr(), std::ptr::null());
        }
        null_mut()
    }
}
fn set(hwnd: HWND, value: &str) {
    unsafe {
        SendMessageW(hwnd, WM_SETTEXT, 0, wide(value).as_ptr() as isize);
    }
}
fn text(hwnd: HWND) -> String {
    unsafe {
        let n = SendMessageW(hwnd, WM_GETTEXTLENGTH, 0, 0).max(0) as usize;
        let mut b = vec![0u16; n + 1];
        let n = SendMessageW(hwnd, WM_GETTEXT, b.len(), b.as_mut_ptr() as isize);
        String::from_utf16_lossy(&b[..n.max(0) as usize])
    }
}
fn command(hwnd: HWND, id: usize) {
    unsafe {
        SendMessageW(hwnd, WM_COMMAND, id, 0);
    }
}
fn setting_index(key: &str) -> usize {
    altrun_probe::preferences::SPECS
        .iter()
        .position(|spec| spec.key == key)
        .unwrap()
}
fn setting(hwnd: HWND, key: &str) -> HWND {
    unsafe { GetDlgItem(hwnd, (2400 + setting_index(key)) as i32) }
}
fn select_page(hwnd: HWND, page: usize) {
    use windows_sys::Win32::UI::Controls::TCM_GETCURSEL;
    unsafe {
        let tabs = GetDlgItem(hwnd, 2301);
        let current = SendMessageW(tabs, TCM_GETCURSEL, 0, 0) as usize;
        let key = if page < current { 0x25 } else { 0x27 };
        for _ in 0..page.abs_diff(current) {
            SendMessageW(tabs, WM_KEYDOWN, key, 0);
        }
        assert_eq!(SendMessageW(tabs, TCM_GETCURSEL, 0, 0) as usize, page);
    }
}
#[test]
fn complete_native_settings_history_matching_and_instance_regression() {
    let root = std::env::current_dir()
        .unwrap()
        .join("target/complete-native");
    std::fs::create_dir_all(&root).unwrap();
    let data = root.join("shortcuts.tsv");
    let state = data.with_extension("state.tsv");
    let settings = data.with_extension("settings.ini");
    let marker = root.join("launched.txt");
    for p in [&state, &settings, &marker] {
        let _ = std::fs::remove_file(p);
    }
    // Reserve the secondary action before launch to reproduce the user's
    // unrelated occupied hotkey while changing appearance and primary hotkey.
    assert_ne!(unsafe { RegisterHotKey(null_mut(), 22221, 3, 0x78) }, 0);
    std::fs::write(
        &settings,
        "[Config]\nLastItemHotKey=Ctrl+Alt+F9\n[Legacy]\nExtraSetting=keep\n",
    )
    .unwrap();
    let shell = altrun_probe::safe_notepad_path()
        .unwrap()
        .with_file_name("cmd.exe");
    std::fs::write(&data,format!("self\tSynthetic launch\t\"{}\" /D /C echo ok>\"{}\"\t0\nabab\tRepeated alias\t__missing__.exe\t0\nparam\tParameter fixture\t__missing__.exe %p\t0\n",shell.display(),marker.display())).unwrap();
    let exe = std::env::var_os("ALTRUN_RELEASE_SMOKE_EXE")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_altrun-probe").into());
    let child = Running(
        Command::new(&exe)
            .args(["--show", "--enable-launch", "--data"])
            .arg(&data)
            .spawn()
            .unwrap(),
    );
    let pid = child.0.id();
    let mut launcher = null_mut();
    wait(|| {
        launcher = window("ALTRun.Native.PerformanceProbe", pid);
        !launcher.is_null() && unsafe { IsWindowVisible(launcher) != 0 }
    });
    unsafe {
        let edit = GetDlgItem(launcher, 1001);
        let list = GetDlgItem(launcher, 1002);
        set(edit, "^(ab)\\1$");
        wait(|| SendMessageW(list, LB_GETCOUNT, 0, 0) == 1);
        set(edit, "^(a+)\\1$");
        set(edit, "self");
        thread::sleep(Duration::from_millis(250));
        assert_eq!(SendMessageW(list, LB_GETCOUNT, 0, 0), 1);
        assert!(text(GetDlgItem(launcher, 1003)).contains("cmd.exe"));
        command(launcher, 18010);
        assert_ne!(OpenClipboard(null_mut()), 0);
        let handle = GetClipboardData(13);
        let copied = if handle.is_null() {
            None
        } else {
            let p = GlobalLock(handle) as *const u16;
            if p.is_null() {
                None
            } else {
                let data = std::slice::from_raw_parts(p, GlobalSize(handle) / 2);
                let s = String::from_utf16(data.split(|c| *c == 0).next().unwrap_or_default()).ok();
                GlobalUnlock(handle);
                s
            }
        };
        CloseClipboard();
        assert!(
            copied.is_some_and(|s| s.contains("cmd.exe")),
            "copy command must transfer Unicode text to the clipboard"
        );
        command(launcher, 18005);
        let mut dialog = null_mut();
        wait(|| {
            dialog = window("ALTRun.Native.Settings", pid);
            !dialog.is_null() && IsWindowVisible(dialog) != 0
        });
        assert_ne!(IsWindowVisible(setting(dialog, "AddToSendTo")), 0);
        assert_eq!(IsWindowVisible(setting(dialog, "HotKey1")), 0);
        native_snapshot::save(dialog, "settings-general");
        select_page(dialog, 3);
        assert_ne!(IsWindowVisible(setting(dialog, "RowHeight")), 0);
        set(setting(dialog, "RowHeight"), "024 ");
        command(dialog, 2310);
        wait(|| SendMessageW(list, LB_GETITEMHEIGHT, 0, 0) == 24);
        assert!(
            std::fs::read_to_string(&settings)
                .unwrap()
                .contains("RowHeight=24")
        );
        native_snapshot::save(dialog, "settings");
        select_page(dialog, 2);
        SendMessageW(setting(dialog, "Regex"), BM_CLICK, 0, 0);
        command(dialog, 2310);
        assert!(
            std::fs::read_to_string(&settings)
                .unwrap()
                .contains("Regex=0")
        );
        SendMessageW(setting(dialog, "Regex"), BM_CLICK, 0, 0);
        command(dialog, 2310);
        assert_ne!(RegisterHotKey(null_mut(), 22222, 3, 0x7b), 0);
        select_page(dialog, 1);
        let main_key = setting(dialog, "HotKey1");
        assert_ne!(IsWindowVisible(setting(dialog, "LastItemHotKey")), 0);
        native_snapshot::save(dialog, "settings-hotkeys");
        set(main_key, "Ctrl+Alt+F12");
        command(dialog, 2310);
        assert!(text(GetDlgItem(dialog, 2303)).contains("占用"));
        assert!(
            !std::fs::read_to_string(&settings)
                .unwrap()
                .contains("Ctrl+Alt+F12")
        );
        UnregisterHotKey(null_mut(), 22222);
        command(dialog, 2500 + setting_index("HotKey1"));
        SendMessageW(dialog, WM_KEYDOWN, 0x77, 0);
        assert_eq!(text(main_key), "F8");
        command(dialog, 2310);
        assert!(
            std::fs::read_to_string(&settings)
                .unwrap()
                .contains("HotKey1=F8")
        );
        assert!(
            std::fs::read_to_string(&settings)
                .unwrap()
                .contains("LastItemHotKey=Ctrl+Alt+F9")
        );
        assert!(
            std::fs::read_to_string(&settings)
                .unwrap()
                .contains("ExtraSetting=keep")
        );
        select_page(dialog, 0);
        let integration = altrun_probe::integration::Integration::for_data(&data).unwrap();
        assert!(!integration.send_to.exists());
        SendMessageW(setting(dialog, "AddToSendTo"), BM_CLICK, 0, 0);
        command(dialog, 2310);
        assert!(
            integration.send_to.exists(),
            "Send to was not added: {}",
            text(GetDlgItem(dialog, 2303))
        );
        SendMessageW(setting(dialog, "AddToSendTo"), BM_CLICK, 0, 0);
        command(dialog, 2310);
        assert!(!integration.send_to.exists());
        command(dialog, 2314);
        wait(|| IsWindow(dialog) == 0);
        UnregisterHotKey(null_mut(), 22221);
        let mut second = Command::new(&exe).arg("--data").arg(&data).spawn().unwrap();
        wait(|| second.try_wait().unwrap().is_some());
        assert!(second.wait().unwrap().success());
        wait(|| IsWindowVisible(launcher) != 0);
        assert!(text(edit).is_empty());
        assert_eq!(SendMessageW(list, LB_GETCOUNT, 0, 0), 3);
        set(edit, "self");
        SendMessageW(edit, WM_KEYDOWN, 13, 0);
        wait(|| marker.exists());
        wait(|| std::fs::read_to_string(&state).is_ok_and(|t| t.contains("F\tlegacy-")));
        command(launcher, 18006);
        assert_eq!(SendMessageW(list, LB_GETCOUNT, 0, 0), 1);
        command(launcher, 18012);
        assert!(text(edit).is_empty());
        assert_eq!(SendMessageW(list, LB_GETCOUNT, 0, 0), 3);
        command(launcher, 18006);
        assert_eq!(SendMessageW(list, LB_GETCOUNT, 0, 0), 1);
        command(launcher, 18003);
        let mut manager = null_mut();
        wait(|| {
            manager = window("ALTRun.Native.ShortcutManager", pid);
            !manager.is_null() && !GetDlgItem(manager, 2115).is_null()
        });
        command(manager, 2122);
        assert_eq!(
            SendMessageW(GetDlgItem(manager, 2101), LB_GETCOUNT, 0, 0),
            4
        );
        command(manager, 2113);
        assert_eq!(
            altrun_probe::library::Snapshot::open(&data)
                .unwrap()
                .entries[3]
                .kind,
            altrun_probe::library::EntryKind::Separator
        );
        command(manager, 2114);
        wait(|| IsWindow(manager) == 0);
        for i in 0..100 {
            set(edit, if i % 2 == 0 { "ab" } else { "self" });
        }
        let before = resources(&child.0);
        for i in 0..500 {
            set(edit, if i % 2 == 0 { "ab" } else { "self" });
        }
        assert_eq!(SendMessageW(list, LB_GETCOUNT, 0, 0), 1);
        assert_eq!(SendMessageW(launcher, WM_NULL, 0, 0), 0);
        let after = resources(&child.0);
        assert!(
            after.0 <= before.0 + 8 * 1024 * 1024,
            "private commit grew during ordinary search: {before:?} -> {after:?}"
        );
        assert!(
            after.1 <= before.1 + 8,
            "GDI resource leak: {before:?} -> {after:?}"
        );
        assert!(
            after.2 <= before.2 + 4,
            "USER resource leak: {before:?} -> {after:?}"
        );
        std::fs::create_dir_all("target/ui").unwrap();
        std::fs::write(
            "target/ui/resident-resources.txt",
            format!(
                "synthetic fixture; private commit, GDI, USER\nbefore={before:?}\nafter={after:?}\n"
            ),
        )
        .unwrap();
    }
}
