#![cfg(windows)]
use std::os::windows::io::AsRawHandle;
use std::{
    process::{Child, Command},
    ptr::null_mut,
    thread,
    time::{Duration, Instant},
};
use windows_sys::Win32::System::{DataExchange::*, Memory::*};
use windows_sys::Win32::UI::HiDpi::GetDpiForWindow;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    IsWindowEnabled, RegisterHotKey, UnregisterHotKey,
};
use windows_sys::Win32::{Foundation::*, Graphics::Gdi::*, UI::WindowsAndMessaging::*};
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
    unsafe {
        let tabs = GetDlgItem(hwnd, 2301);
        assert_eq!(SendMessageW(tabs, LB_SETCURSEL, page, 0) as usize, page);
        SendMessageW(
            hwnd,
            WM_COMMAND,
            (LBN_SELCHANGE as usize) << 16 | 2301,
            tabs as isize,
        );
    }
}

fn bounds(hwnd: HWND) -> RECT {
    let mut rect = RECT::default();
    assert_ne!(unsafe { GetWindowRect(hwnd, &mut rect) }, 0);
    rect
}

fn pixel(hwnd: HWND, x: i32, y: i32) -> u32 {
    unsafe {
        let dc = GetWindowDC(hwnd);
        let result = GetPixel(dc, x, y);
        ReleaseDC(hwnd, dc);
        result
    }
}

fn click_tab_and_wait_for_pixels(hwnd: HWND, page: usize, field: &str, screenshot: &str) {
    unsafe {
        let tabs = GetDlgItem(hwnd, 2301);
        let mut row = RECT::default();
        assert_ne!(
            SendMessageW(tabs, LB_GETITEMRECT, page, &mut row as *mut RECT as isize),
            -1
        );
        let point = ((row.top + row.bottom) / 2) << 16 | 20;
        assert_ne!(PostMessageW(tabs, WM_LBUTTONDOWN, 1, point as isize), 0);
        assert_ne!(PostMessageW(tabs, WM_LBUTTONUP, 0, point as isize), 0);
        let control = setting(hwnd, field);
        let dark_accent = 0x00ffc79a;
        wait(|| {
            let window = bounds(hwnd);
            let rect = bounds(control);
            let inset = (window.right - window.left) * 33 / 1102;
            SendMessageW(tabs, LB_GETCURSEL, 0, 0) == page as isize
                && IsWindowVisible(control) != 0
                && pixel(
                    control,
                    rect.right - rect.left - inset,
                    (rect.bottom - rect.top) / 2,
                ) == dark_accent
        });
        native_snapshot::save_painted(hwnd, screenshot);
    }
}

fn settings_design(hwnd: HWND) {
    let pages = [
        "general",
        "hotkeys",
        "search",
        "appearance",
        "type",
        "position",
        "about",
    ];
    for (theme, segment) in [("light", 2331), ("dark", 2332)] {
        select_page(hwnd, 3);
        command(hwnd, segment);
        unsafe {
            command(hwnd, 2333);
            assert_ne!(IsWindowVisible(GetDlgItem(hwnd, 2351)), 0);
            assert_eq!(IsWindowVisible(setting(hwnd, "BGStrength")), 0);
            assert_eq!(IsWindowVisible(setting(hwnd, "BGCrop")), 0);
            command(hwnd, 2351);
            assert_eq!(
                text(setting(hwnd, "TitleColor")),
                if theme == "dark" {
                    "#263B53"
                } else {
                    "#DCE7F4"
                },
            );
            command(hwnd, 2350);
            command(hwnd, 2334);
            assert_eq!(IsWindowVisible(GetDlgItem(hwnd, 2351)), 0);
            assert_ne!(IsWindowVisible(setting(hwnd, "BGStrength")), 0);
            assert_ne!(IsWindowVisible(setting(hwnd, "BGCrop")), 0);
            native_snapshot::save(hwnd, &format!("settings-image-{theme}"));
            command(hwnd, 2333);
            select_page(hwnd, 4);
            let title = setting(hwnd, "TitleFont");
            let size = text(title).parse::<i32>().unwrap();
            SendMessageW(title, WM_KEYDOWN, 0x26, 0);
            assert_eq!(text(title), (size + 1).to_string());
            SendMessageW(title, WM_KEYDOWN, 0x28, 0);
            assert_eq!(text(title), size.to_string());
            command(hwnd, 2343);
            assert_eq!(IsWindowVisible(setting(hwnd, "TitleColor")), 0);
            assert_ne!(IsWindowVisible(setting(hwnd, "UseCustomTextColor")), 0);
            assert_ne!(
                IsWindowVisible(GetDlgItem(
                    hwnd,
                    (2500 + setting_index("TitleColor")) as i32
                )),
                0
            );
            native_snapshot::save(hwnd, &format!("settings-colors-{theme}"));
            command(hwnd, 2343);
            let sidebar = GetDlgItem(hwnd, 2301);
            let sidebar_bounds = bounds(sidebar);
            let row_height = SendMessageW(sidebar, LB_GETITEMHEIGHT, 0, 0);
            select_page(hwnd, 5);
            let position_sidebar = bounds(sidebar);
            assert_eq!(
                (
                    position_sidebar.left,
                    position_sidebar.top,
                    position_sidebar.right,
                    position_sidebar.bottom
                ),
                (
                    sidebar_bounds.left,
                    sidebar_bounds.top,
                    sidebar_bounds.right,
                    sidebar_bounds.bottom
                ),
            );
            assert_eq!(SendMessageW(sidebar, LB_GETITEMHEIGHT, 0, 0), row_height);
            let switch = setting(hwnd, "PositionMode");
            assert_eq!(SendMessageW(switch, BM_GETCHECK, 0, 0), 0);
            let rect = bounds(switch);
            let height = rect.bottom - rect.top;
            let switch_center = rect.right - rect.left - height / 3;
            let click = |from, to| {
                let center_y = (height / 2) << 16;
                SendMessageW(switch, WM_LBUTTONDOWN, 1, (from | center_y) as isize);
                SendMessageW(switch, WM_LBUTTONUP, 0, (to | center_y) as isize);
            };
            let checked = SendMessageW(switch, BM_GETCHECK, 0, 0);
            click(8, 8);
            assert_eq!(SendMessageW(switch, BM_GETCHECK, 0, 0), checked);
            click(switch_center, 8);
            assert_eq!(SendMessageW(switch, BM_GETCHECK, 0, 0), checked);
            click(switch_center, switch_center);
            assert_ne!(SendMessageW(switch, BM_GETCHECK, 0, 0), checked);
            SendMessageW(switch, BM_CLICK, 0, 0);
            assert_eq!(SendMessageW(switch, BM_GETCHECK, 0, 0), checked);
        }
        for (page, name) in pages.iter().enumerate() {
            select_page(hwnd, page);
            native_snapshot::save(hwnd, &format!("settings-design-{theme}-{name}"));
        }
    }
    select_page(hwnd, 0);
    let language = bounds(setting(hwnd, "Lang"));
    let sound = bounds(setting(hwnd, "PlayPopupNotify"));
    assert!(language.left > sound.right && (language.top - sound.top).abs() < 20);
    select_page(hwnd, 2);
    let last_input = bounds(setting(hwnd, "RememberFavouratMatch"));
    let first_result = bounds(setting(hwnd, "NumberKey"));
    assert!(
        first_result.top - last_input.bottom > 24,
        "search settings need separate cards"
    );
    select_page(hwnd, 3);
    unsafe {
        assert!(GetDlgItem(hwnd, 2340).is_null());
        let footer = bounds(GetDlgItem(hwnd, 2310));
        let option = setting(hwnd, "ShowOperationHint");
        assert_ne!(IsWindowVisible(option), 0);
        assert!(
            bounds(option).bottom < footer.top,
            "operation hint overlaps footer"
        );
    }
    command(hwnd, 2331);
    select_page(hwnd, 0);
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
        "[Config]\nLastItemHotKey=Ctrl+Alt+F9\nTheme=dark\nMaxRows=2\nLang=简体中文\n[Legacy]\nExtraSetting=keep\n",
    )
    .unwrap();
    let shell = altrun_probe::safe_notepad_path()
        .unwrap()
        .with_file_name("cmd.exe");
    std::fs::write(&data,format!("self\tSynthetic launch\t\"{}\" /D /C echo ok>\"{}\"\t0\nabab\tRepeated alias\t__missing__.exe\t0\nparam\tParameter fixture\t__missing__.exe %p\t0\n",shell.display(),marker.display())).unwrap();
    let exe = std::env::var_os("ALTRUN_RELEASE_SMOKE_EXE")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_ALTRunNext").into());
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
        let original_dpi = GetDpiForWindow(launcher) as usize;
        let mut suggested = bounds(launcher);
        SendMessageW(
            launcher,
            WM_DPICHANGED,
            120 | (120 << 16),
            &mut suggested as *mut RECT as LPARAM,
        );
        let mut list_client = RECT::default();
        assert_ne!(GetClientRect(list, &mut list_client), 0);
        assert_eq!(SendMessageW(list, LB_GETITEMHEIGHT, 0, 0), 28);
        assert_eq!(
            list_client.bottom, 56,
            "125% should display two complete rows"
        );
        SendMessageW(
            launcher,
            WM_DPICHANGED,
            original_dpi | (original_dpi << 16),
            &mut suggested as *mut RECT as LPARAM,
        );
        let button = GetDlgItem(launcher, 18001);
        for id in [18003, 18005, 18001] {
            assert_ne!(
                IsWindowVisible(GetDlgItem(launcher, id)),
                0,
                "default button {id}"
            );
        }
        assert_eq!(
            GetWindowLongW(button, GWL_STYLE) as u32 & 15,
            BS_OWNERDRAW as u32
        );
        let mut bar = SCROLLBARINFO {
            cbSize: std::mem::size_of::<SCROLLBARINFO>() as u32,
            ..Default::default()
        };
        assert_ne!(GetScrollBarInfo(list, OBJID_VSCROLL, &mut bar), 0);
        let list_bounds = bounds(list);
        let x = (bar.rcScrollBar.left + bar.rcScrollBar.right) / 2 - list_bounds.left;
        let y = bar.rcScrollBar.top + 2 - list_bounds.top;
        wait(|| {
            let colour = pixel(list, x, y);
            matches!(colour, 0x00241d17 | 0x00251e19)
        });
        SendMessageW(list, WM_VSCROLL, SB_LINEDOWN as usize, 0);
        assert_eq!(SendMessageW(list, LB_GETTOPINDEX, 0, 0), 1);
        native_snapshot::save_painted(launcher, "launcher-dark-scrollbar");
        set(edit, "^(ab)\\1$");
        assert_eq!(SendMessageW(list, LB_GETCOUNT, 0, 0), 0);
        set(edit, "ab");
        assert_eq!(SendMessageW(list, LB_GETCOUNT, 0, 0), 1);
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
        assert_eq!(
            SendMessageW(setting(dialog, "PlayPopupNotify"), BM_GETCHECK, 0, 0),
            1
        );
        assert_eq!(text(setting(dialog, "PopupFileName")), "Popup.wav");
        assert_eq!(IsWindowVisible(setting(dialog, "HotKey1")), 0);
        assert!(SendMessageW(setting(dialog, "Lang"), CB_GETCOUNT, 0, 0) >= 3);
        let size = |rect: RECT| (rect.right - rect.left, rect.bottom - rect.top);
        let initial_size = size(bounds(dialog));
        for page in 1..=6 {
            select_page(dialog, page);
            assert_eq!(size(bounds(dialog)), initial_size, "settings page {page}");
            assert_eq!(
                SendMessageW(GetDlgItem(dialog, 2301), LB_GETTOPINDEX, 0, 0),
                0
            );
        }
        assert_eq!(SendMessageW(GetDlgItem(dialog, 2301), LB_GETCOUNT, 0, 0), 7);
        assert_eq!(IsWindowEnabled(GetDlgItem(dialog, 2310)), 0);
        for id in 2360..=2363 {
            assert_ne!(IsWindowVisible(GetDlgItem(dialog, id)), 0);
        }
        let email = GetDlgItem(dialog, 2360);
        let mut class = [0u16; 16];
        let len = GetClassNameW(email, class.as_mut_ptr(), class.len() as i32);
        assert_eq!(String::from_utf16_lossy(&class[..len as usize]), "SysLink");
        assert!(text(email).contains("mailto:weimeng54@qq.com"));
        let releases = FindWindowExW(
            dialog,
            null_mut(),
            wide("Static").as_ptr(),
            wide("https://github.com/weimeng54/ALTRunNext/releases").as_ptr(),
        );
        assert!(!releases.is_null() && IsWindowVisible(releases) != 0);
        select_page(dialog, 0);
        assert_eq!(IsWindowVisible(GetDlgItem(dialog, 2360)), 0);
        command(launcher, 18007);
        wait(|| SendMessageW(GetDlgItem(dialog, 2301), LB_GETCURSEL, 0, 0) == 6);
        for key in [
            "WorkingDirectory",
            "ParamHistoryLimit",
            "AllowDynamicTarget",
            "TraceEnable",
        ] {
            assert!(setting(dialog, key).is_null(), "{key}");
        }
        let region = CreateRectRgn(0, 0, 1, 1);
        wait(|| GetWindowRgn(dialog, region) != 0);
        assert_eq!(PtInRegion(region, 0, 0), 0);
        assert_ne!(PtInRegion(region, 60, 20), 0);
        DeleteObject(region);
        command(dialog, 2314);
        wait(|| IsWindow(dialog) == 0);
        wait(|| {
            dialog = window("ALTRun.Native.Settings", pid);
            if dialog.is_null() {
                // A closed window's UI thread can still be releasing its slot.
                command(launcher, 18005);
                false
            } else {
                IsWindowVisible(dialog) != 0
            }
        });
        select_page(dialog, 0);
        click_tab_and_wait_for_pixels(dialog, 2, "NumberKey", "settings-live-search");
        click_tab_and_wait_for_pixels(dialog, 3, "ShowCommandLine", "settings-live-appearance");
        settings_design(dialog);
        native_snapshot::save(dialog, "settings-general");
        select_page(dialog, 3);
        let preview = GetDlgItem(dialog, 2306);
        assert_ne!(IsWindowVisible(preview), 0);
        set(preview, "db");
        let theme = setting(dialog, "Theme");
        SendMessageW(theme, CB_SETCURSEL, 1, 0);
        SendMessageW(
            dialog,
            WM_COMMAND,
            (CBN_SELCHANGE as usize) << 16 | GetDlgCtrlID(theme) as usize,
            theme as isize,
        );
        native_snapshot::save(dialog, "settings-dark-appearance");
        SendMessageW(theme, CB_SETCURSEL, 0, 0);
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
        SendMessageW(setting(dialog, "MatchAnywhere"), BM_CLICK, 0, 0);
        command(dialog, 2310);
        assert!(
            std::fs::read_to_string(&settings)
                .unwrap()
                .contains("MatchAnywhere=0")
        );
        SendMessageW(setting(dialog, "MatchAnywhere"), BM_CLICK, 0, 0);
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
        assert!(GetDlgItem(dialog, (2500 + setting_index("HotKey1")) as i32).is_null());
        let previous = text(main_key);
        SendMessageW(main_key, WM_SETFOCUS, 0, 0);
        assert_eq!(text(main_key), previous);
        SendMessageW(dialog, WM_KEYDOWN, 0x10, 0);
        assert_eq!(text(main_key), previous);
        SendMessageW(dialog, WM_KEYDOWN, 0x77, 0);
        assert_eq!(text(main_key), "F8");
        SendMessageW(main_key, 0x02a3, 0, 0);
        SendMessageW(dialog, WM_KEYDOWN, 0x76, 0);
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
        wait(|| std::fs::read_to_string(&state).is_ok_and(|t| t.contains("F\titem-")));
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
            3
        );
        assert!(GetDlgItem(manager, 2151).is_null());
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

        command(launcher, 18005);
        let mut settings_window = null_mut();
        wait(|| {
            settings_window = window("ALTRun.Native.Settings", pid);
            !settings_window.is_null() && IsWindowVisible(settings_window) != 0
        });
        SendMessageW(setting(settings_window, "Lang"), CB_SETCURSEL, 1, 0);
        command(settings_window, 2310);
        assert!(
            std::fs::read_to_string(&settings)
                .unwrap()
                .contains("Lang=en")
        );
        command(settings_window, 2314);
        wait(|| IsWindow(settings_window) == 0);
        wait(|| {
            settings_window = window("ALTRun.Native.Settings", pid);
            if settings_window.is_null() {
                PostMessageW(launcher, WM_COMMAND, 18005, 0);
            }
            !settings_window.is_null() && IsWindowVisible(settings_window) != 0
        });
        assert_eq!(text(settings_window), "ALTRunNext · Settings");
        assert_eq!(text(GetDlgItem(settings_window, 2310)), "Save and apply");
        command(settings_window, 2314);
        wait(|| IsWindow(settings_window) == 0);
    }
}
