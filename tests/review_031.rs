// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

#![cfg(windows)]
#[path = "review_031/menu_frame.rs"]
mod menu_frame;
mod native_snapshot;
mod review_native_support;

use review_native_support::{
    Running, check_edit_drag_past_right_edge, check_edit_selection, command, set, text, wait,
    window,
};
use std::{path::Path, process::Command, ptr::null_mut, time::Instant};
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, RECT},
    Graphics::Gdi::{GetPixel, GetWindowDC, ReleaseDC},
    UI::{
        Controls::*,
        Input::KeyboardAndMouse::{VK_DELETE, VK_ESCAPE, VK_F2, VK_RETURN},
        WindowsAndMessaging::*,
    },
};

fn start(data: &Path, theme: &str, scale: &str) -> (Running, HWND) {
    std::fs::write(
        data.with_extension("settings.ini"),
        format!("[GUI]\nTheme={theme}\n[Config]\nLang=zh-Hans\nPlayPopupNotify=0\n"),
    )
    .unwrap();
    let executable = std::env::var_os("ALTRUN_RELEASE_SMOKE_EXE")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_ALTRunNext").into());
    let process = Running(
        Command::new(executable)
            .args(["--show", "--safe-mode", "--ui-scale", scale, "--data"])
            .arg(data)
            .spawn()
            .unwrap(),
    );
    let launcher = ready_window(process.0.id(), "ALTRun.Native.PerformanceProbe", 1001);
    (process, launcher)
}

fn ready_window(pid: u32, class: &str, control: i32) -> HWND {
    let mut hwnd = null_mut();
    wait(|| {
        hwnd = window(class, pid);
        unsafe {
            !hwnd.is_null() && IsWindowVisible(hwnd) != 0 && !GetDlgItem(hwnd, control).is_null()
        }
    });
    hwnd
}

fn begin(hwnd: HWND, action: usize) {
    unsafe {
        assert_ne!(PostMessageW(hwnd, WM_COMMAND, action, 0), 0);
    }
}

fn press(hwnd: HWND, key: u16) {
    unsafe {
        assert_ne!(PostMessageW(hwnd, WM_KEYDOWN, key as usize, 0), 0);
    }
}

fn check_popup(owner: HWND, pid: u32, button: usize, capture: &str, theme: &str) {
    use windows_sys::Win32::Graphics::Gdi::{
        CreateRectRgn, DeleteObject, GetWindowRgn, PtInRegion,
    };
    begin(owner, button);
    let mut popup = null_mut();
    unsafe {
        let region = CreateRectRgn(0, 0, 0, 0);
        assert!(!region.is_null());
        wait(|| {
            popup = window("#32768", pid);
            !popup.is_null() && IsWindowVisible(popup) != 0
        });
        eprintln!("Checking the window region of {capture}");
        wait(|| GetWindowRgn(popup, region) > 1);
        assert_eq!(PtInRegion(region, 0, 0), 0);
        assert_ne!(PtInRegion(region, 12, 12), 0);
        if button == 2312 {
            let menu = SendMessageW(popup, MN_GETHMENU, 0, 0) as HMENU;
            assert_eq!(GetMenuItemCount(menu), 2);
            let mut trigger = RECT::default();
            let mut panel = RECT::default();
            GetWindowRect(child(owner, button as i32), &mut trigger);
            GetWindowRect(popup, &mut panel);
            assert_eq!(panel.left, trigger.left);
            assert!(
                (panel.right - trigger.right).abs() <= 1,
                "{capture}: button {}..{} menu {}..{}",
                trigger.left,
                trigger.right,
                panel.left,
                panel.right
            );
        }
        menu_frame::check(popup, region, theme);
        DeleteObject(region);
        native_snapshot::save(popup, capture);
        PostMessageW(owner, WM_CANCELMODE, 0, 0);
        wait(|| IsWindowVisible(popup) == 0);
    }
}

fn file_name_field(picker: HWND) -> HWND {
    unsafe extern "system" fn find(hwnd: HWND, context: LPARAM) -> i32 {
        let mut name = [0; 32];
        let len = unsafe { GetClassNameW(hwnd, name.as_mut_ptr(), 32) };
        if len == 4
            && String::from_utf16_lossy(&name[..4]) == "Edit"
            && unsafe { IsWindowVisible(hwnd) } != 0
        {
            unsafe { *(context as *mut HWND) = hwnd };
            return 0;
        }
        1
    }
    unsafe {
        let file_name = GetDlgItem(picker, 1152);
        if !file_name.is_null() && IsWindowVisible(file_name) != 0 {
            return file_name;
        }
        let mut input: HWND = null_mut();
        let combo = GetDlgItem(picker, 1148);
        EnumChildWindows(
            if combo.is_null() { picker } else { combo },
            Some(find),
            &mut input as *mut HWND as LPARAM,
        );
        input
    }
}

fn choose_file(pid: u32, file: &Path) -> Instant {
    eprintln!(
        "Waiting for file picker: {}",
        file.file_name().unwrap().display()
    );
    let mut picker = null_mut();
    let mut input = null_mut();
    wait(|| {
        picker = window("#32770", pid);
        if picker.is_null() {
            return false;
        }
        input = file_name_field(picker);
        unsafe { IsWindowVisible(picker) != 0 && !input.is_null() }
    });
    eprintln!(
        "Selecting synthetic file {}",
        file.file_name().unwrap().display()
    );
    let path = file.to_string_lossy().replace('/', "\\");
    let value = review_native_support::wide(&path);
    let submitted = unsafe {
        SendMessageW(input, EM_SETSEL, 0, -1);
        SendMessageW(input, EM_REPLACESEL, 1, value.as_ptr() as LPARAM);
        assert_eq!(text(input), path);
        let submit = child(picker, IDOK);
        let submitted = Instant::now();
        assert_ne!(
            PostMessageW(picker, WM_COMMAND, IDOK as usize, submit as LPARAM),
            0
        );
        submitted
    };
    wait(|| unsafe { IsWindow(picker) == 0 });
    submitted
}

fn check_backup(launcher: HWND, pid: u32, data: &Path, theme: &str, scale: &str) {
    let archive = data.with_file_name(format!("synthetic-{theme}-{scale}.zip"));
    if archive.is_file() {
        std::fs::remove_file(&archive).unwrap();
    }
    let original = std::fs::read(data).unwrap();
    assert_eq!(text(child(launcher, 18003)), "管理启动项");
    assert_eq!(text(child(launcher, 18005)), "设置…");
    assert_eq!(text(child(launcher, 18001)), "隐藏");
    command(launcher, 18005);
    let settings = ready_window(pid, "ALTRun.Native.Settings", 2312);
    check_settings_layout(settings, theme, scale);
    check_popup(
        settings,
        pid,
        2312,
        &format!("menu-backup-{theme}-{scale}"),
        theme,
    );
    native_snapshot::save(settings, &format!("031-backup-settings-{theme}-{scale}"));
    begin(settings, 2365);
    let preview = ready_window(pid, "ALTRun.Native.BackupSelection", 1);
    native_snapshot::save(preview, &format!("031-backup-{theme}-{scale}"));
    eprintln!("Confirming the backup preview");
    press(preview, VK_RETURN);
    wait(|| unsafe { IsWindow(preview) == 0 });
    choose_file(pid, &archive);
    wait(|| altrun_probe::backup::Backup::load(&archive).is_ok_and(|backup| backup.count() == 5));
    wait(|| unsafe {
        windows_sys::Win32::UI::Input::KeyboardAndMouse::IsWindowEnabled(child(settings, 2312)) != 0
    });
    begin(settings, 2366);
    choose_file(pid, &archive);
    let preview = ready_window(pid, "ALTRun.Native.BackupSelection", 1);
    native_snapshot::save(preview, &format!("031-restore-{theme}-{scale}"));
    press(preview, VK_ESCAPE);
    wait(|| unsafe { IsWindow(preview) == 0 });
    assert_eq!(std::fs::read(data).unwrap(), original);
    unsafe { assert_ne!(PostMessageW(settings, WM_CLOSE, 0, 0), 0) };
    wait(|| unsafe { IsWindow(settings) == 0 });
}

fn check_open_report_theme(
    launcher: HWND,
    pid: u32,
    manager_list: HWND,
    report_list: HWND,
    data: &Path,
) {
    command(launcher, 18005);
    let settings = ready_window(pid, "ALTRun.Native.Settings", 2310);
    let tabs = child(settings, 2301);
    unsafe {
        SendMessageW(tabs, LB_SETCURSEL, 3, 0);
        SendMessageW(
            settings,
            WM_COMMAND,
            (LBN_SELCHANGE as usize) << 16 | 2301,
            tabs as LPARAM,
        );
    }
    let selection =
        |list| unsafe { SendMessageW(list, LVM_GETNEXTITEM, usize::MAX, LVNI_SELECTED as LPARAM) };
    let paper = |list| unsafe { SendMessageW(list, LVM_GETBKCOLOR, 0, 0) };
    let selected = (selection(manager_list), selection(report_list));
    let light = paper(report_list);
    let ini = data.with_extension("settings.ini");
    let caption = |hwnd| unsafe {
        let dc = GetWindowDC(hwnd);
        let color = GetPixel(dc, 360, 12);
        ReleaseDC(hwnd, dc);
        color
    };
    let light_chrome = 236 | (240 << 8) | (246 << 16);
    let dark_chrome = 21 | (26 << 8) | (32 << 16);
    assert_eq!(paper(manager_list), light);
    for (segment, chrome, expected, mode) in [
        (2332, dark_chrome, 23 | (29 << 8) | (36 << 16), "dark"),
        (2331, light_chrome, light, "light"),
    ] {
        let before = paper(report_list);
        let saved = std::fs::read(&ini).unwrap();
        command(settings, segment);
        wait(|| caption(settings) == chrome);
        assert_eq!(paper(report_list), before);
        assert_eq!(paper(manager_list), before);
        assert_eq!(std::fs::read(&ini).unwrap(), saved);
        command(settings, 2310);
        wait(|| paper(manager_list) == expected && paper(report_list) == expected);
        let (_, text) = altrun_probe::state_store::TextFile::open(&ini).unwrap();
        assert_eq!(
            altrun_probe::preferences::Preferences::parse(&text)
                .unwrap()
                .get("Theme"),
            mode
        );
        assert_eq!((selection(manager_list), selection(report_list)), selected);
    }
    let saved = std::fs::read(&ini).unwrap();
    command(settings, 2332);
    wait(|| caption(settings) == dark_chrome);
    unsafe { SendMessageW(settings, WM_SETTINGCHANGE, 0, 0) };
    unsafe { PostMessageW(settings, WM_CLOSE, 0, 0) };
    let confirm = ready_window(pid, "ALTRun.Native.Confirmation", 1);
    command(confirm, 2);
    wait(|| unsafe { IsWindow(confirm) == 0 } && caption(settings) == dark_chrome);
    unsafe { PostMessageW(settings, WM_CLOSE, 0, 0) };
    let confirm = ready_window(pid, "ALTRun.Native.Confirmation", 1);
    command(confirm, 1);
    wait(|| unsafe { IsWindow(settings) == 0 });
    assert_eq!(paper(report_list), light);
    assert_eq!(std::fs::read(&ini).unwrap(), saved);
    wait(|| {
        command(launcher, 18005);
        !window("ALTRun.Native.Settings", pid).is_null()
    });
    let reopened = ready_window(pid, "ALTRun.Native.Settings", 2310);
    assert_eq!(settings_caption_color(reopened), light_chrome);
    unsafe { PostMessageW(reopened, WM_CLOSE, 0, 0) };
    wait(|| unsafe { IsWindow(reopened) == 0 });
}

// Ask the target process to paint its client area even if another window
// overlaps the settings caption on the test desktop.
fn settings_caption_color(hwnd: HWND) -> u32 {
    use windows_sys::Win32::{
        Graphics::Gdi::{
            CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, SelectObject,
        },
        Storage::Xps::{PW_CLIENTONLY, PrintWindow},
    };
    unsafe {
        let screen = GetWindowDC(hwnd);
        let dc = CreateCompatibleDC(screen);
        let bitmap = CreateCompatibleBitmap(screen, 361, 13);
        let previous = SelectObject(dc, bitmap);
        let printed = PrintWindow(hwnd, dc, PW_CLIENTONLY);
        let color = GetPixel(dc, 360, 12);
        SelectObject(dc, previous);
        DeleteObject(bitmap);
        DeleteDC(dc);
        ReleaseDC(hwnd, screen);
        assert_ne!(printed, 0);
        color
    }
}

fn check_settings_layout(settings: HWND, theme: &str, scale: &str) {
    let tabs = child(settings, 2301);
    let select = |page| unsafe {
        SendMessageW(tabs, LB_SETCURSEL, page, 0);
        SendMessageW(
            settings,
            WM_COMMAND,
            (LBN_SELCHANGE as usize) << 16 | 2301,
            tabs as LPARAM,
        );
    };
    let preview = child(settings, 2306);
    for (page, section) in [(3, "appearance"), (4, "fonts")] {
        select(page);
        assert!(text(preview).is_empty());
        assert!(!review_native_support::has_text(settings, "例如 db"));
        native_snapshot::save(
            settings,
            &format!("settings-clean-{section}-{theme}-{scale}"),
        );
    }
    set(preview, "cl");
    select(3);
    assert_eq!(text(preview), "cl");
    set(preview, "");
    let row_limit = altrun_probe::preferences::SPECS
        .iter()
        .position(|spec| spec.key == "MaxRows")
        .unwrap();
    set(child(settings, 2400 + row_limit as i32), "9");
    let save = child(settings, 2310);
    command(settings, 2310);
    let status = child(settings, 2303);
    wait(|| text(status) == "已保存并应用。");
    unsafe {
        let mut action = RECT::default();
        let mut message = RECT::default();
        GetWindowRect(save, &mut action);
        GetWindowRect(status, &mut message);
        assert_eq!((message.top, message.bottom), (action.top, action.bottom));
    }
    select(0);
    assert!(!review_native_support::has_text(settings, "管理启动方式"));
    native_snapshot::save(settings, &format!("settings-clean-saved-{theme}-{scale}"));
}

fn check_restore_and_restart(running: &mut Running, launcher: HWND, data: &Path) {
    use review_native_support::wide;

    let pid = running.0.id();
    let original = std::fs::read(data).unwrap();
    let mut changed = original.clone();
    changed.extend_from_slice(b"extra\tExtra launch item\texplorer.exe\t0\n");
    std::fs::write(data, changed).unwrap();
    command(launcher, 18005);
    let settings = ready_window(pid, "ALTRun.Native.Settings", 2312);
    begin(settings, 2366);
    choose_file(pid, &data.with_file_name("synthetic-dark-200.zip"));
    let preview = ready_window(pid, "ALTRun.Native.BackupSelection", 1);
    unsafe {
        SendMessageW(child(preview, 14), BM_CLICK, 0, 0);
    }
    press(preview, VK_RETURN);
    wait(|| running.0.try_wait().unwrap().is_some());
    assert!(running.0.wait().unwrap().success());
    assert_eq!(std::fs::read(data).unwrap(), original);

    let class = wide("ALTRun.Native.PerformanceProbe");
    let mut restarted = null_mut();
    wait(|| unsafe {
        restarted = FindWindowW(class.as_ptr(), null_mut());
        !restarted.is_null() && !GetDlgItem(restarted, 1002).is_null()
    });
    let mut restarted_pid = 0;
    unsafe {
        GetWindowThreadProcessId(restarted, &mut restarted_pid);
    }
    assert_ne!(restarted_pid, pid);
    command(restarted, 18002);
    wait(|| unsafe { IsWindow(restarted) == 0 });
    let previous = std::fs::read_dir(data.parent().unwrap())
        .unwrap()
        .flatten()
        .filter(|file| {
            file.file_name()
                .to_string_lossy()
                .starts_with("ALTRunNext-before-restore-")
        });
    assert!(previous.into_iter().any(|file| {
        altrun_probe::backup::Backup::load(&file.path()).is_ok_and(|backup| backup.count() == 6)
    }));
}

fn check_legacy_import(manager: HWND, pid: u32, dir: &Path, list: HWND) {
    let invalid = dir.join("unreadable.txt");
    std::fs::write(&invalid, "This file does not contain launch items.").unwrap();
    begin(manager, 2129);
    choose_file(pid, &invalid);
    let error = ready_window(pid, "ALTRun.Native.ImportPreview", 2913);
    assert!(!text(child(error, 2913)).is_empty());
    press(error, VK_RETURN);
    wait(|| unsafe { IsWindow(error) == 0 });

    let legacy = dir.join("ShortCutList.txt");
    let legacy_text = "F7|记事本|笔记|notepad.exe\r\n";
    let bytes = [0xfeff_u16]
        .into_iter()
        .chain(legacy_text.encode_utf16())
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    std::fs::write(&legacy, bytes).unwrap();
    choose_file(pid, &legacy);
    let preview = ready_window(pid, "ALTRun.Native.ImportPreview", 2906);
    assert_eq!(count(child(preview, 2901)), 1);
    command(preview, 2906);
    assert!(text(child(preview, 2912)).contains('3'));
    press(preview, VK_ESCAPE);
    wait(|| unsafe { IsWindow(preview) == 0 });
    assert_eq!(count(list), 3);
}

fn child(hwnd: HWND, id: i32) -> HWND {
    let control = unsafe { GetDlgItem(hwnd, id) };
    assert!(!control.is_null(), "missing control {id}");
    control
}

// Run only on the disposable workflow desktop, where changing a system preference is safe.
fn check_system_contrast(data: &Path) {
    use windows_sys::Win32::{
        Graphics::Gdi::{COLOR_WINDOW, COLOR_WINDOWTEXT, GetSysColor},
        UI::Accessibility::{HCF_HIGHCONTRASTON, HIGHCONTRASTW},
    };
    if std::env::var("ALTRUN_CONTRAST_DESKTOP").as_deref() != Ok("github-hosted") {
        return;
    }
    let size = size_of::<HIGHCONTRASTW>() as u32;
    let mut previous = HIGHCONTRASTW {
        cbSize: size,
        ..Default::default()
    };
    unsafe {
        assert_ne!(
            SystemParametersInfoW(
                SPI_GETHIGHCONTRAST,
                size,
                (&mut previous as *mut HIGHCONTRASTW).cast(),
                0
            ),
            0
        );
    }
    let (running, launcher) = start(data, "dark", "100");
    let pid = running.0.id();
    command(launcher, 18003);
    let manager = ready_window(pid, "ALTRun.Native.ShortcutManager", 2101);
    let list = child(manager, 2101);
    let set = |flags| unsafe {
        let mut state = HIGHCONTRASTW {
            cbSize: size,
            dwFlags: flags,
            lpszDefaultScheme: null_mut(),
        };
        SystemParametersInfoW(
            SPI_SETHIGHCONTRAST,
            size,
            (&mut state as *mut HIGHCONTRASTW).cast(),
            SPIF_SENDCHANGE,
        )
    };
    assert_ne!(set(previous.dwFlags | HCF_HIGHCONTRASTON), 0);
    let result = std::panic::catch_unwind(|| unsafe {
        wait(|| {
            SendMessageW(list, LVM_GETBKCOLOR, 0, 0) == GetSysColor(COLOR_WINDOW) as isize
                && SendMessageW(list, LVM_GETTEXTCOLOR, 0, 0)
                    == GetSysColor(COLOR_WINDOWTEXT) as isize
        });
        native_snapshot::save(manager, "manager-system-contrast");
        check_popup(manager, pid, 2140, "menu-new-system-contrast", "system");
    });
    assert_ne!(set(previous.dwFlags), 0);
    if let Err(failure) = result {
        std::panic::resume_unwind(failure);
    }
}

fn count(list: HWND) -> isize {
    unsafe { SendMessageW(list, LVM_GETITEMCOUNT, 0, 0) }
}

fn check_launcher_scrollbar(data: &Path, theme: &str, scale: &str) {
    let rows = (0..26)
        .map(|i| format!("test{i}\tSynthetic {i}\tnotepad.exe\t0\n"))
        .collect::<String>();
    std::fs::write(data, rows).unwrap();
    let (mut process, _) = start(data, theme, scale);
    let launcher = ready_window(process.0.id(), "ALTRun.Native.PerformanceProbe", 1006);
    let list = child(launcher, 1002);
    let track = child(launcher, 1006);
    wait(|| unsafe { IsWindowVisible(track) != 0 });
    unsafe {
        SendMessageW(list, WM_VSCROLL, SB_LINEDOWN as usize, 0);
        assert_eq!(SendMessageW(list, LB_GETTOPINDEX, 0, 0), 1);
        SendMessageW(list, WM_NCPAINT, 1, 0);
    }
    native_snapshot::save(launcher, &format!("ui-scrollbar-{theme}-{scale}"));
    let search = child(launcher, 1001);
    set(search, "test25");
    wait(|| unsafe { IsWindowVisible(track) == 0 });
    set(search, "");
    wait(|| unsafe { IsWindowVisible(track) != 0 });
    command(launcher, 18002);
    wait(|| process.0.try_wait().unwrap().is_some());
}

fn check_launcher(launcher: HWND, theme: &str, scale: &str) {
    let input = child(launcher, 1001);
    let rows = child(launcher, 1002);
    let hotkey =
        || unsafe { SendMessageW(launcher, WM_HOTKEY, 17001, (isize::from(b'R') << 16) | 1) };
    hotkey();
    assert_eq!(unsafe { IsWindowVisible(launcher) }, 0);
    hotkey();
    assert!(text(input).is_empty());
    assert_eq!(unsafe { SendMessageW(rows, LB_GETCOUNT, 0, 0) }, 2);
    unsafe {
        for ch in b"cl" {
            SendMessageW(input, WM_CHAR, usize::from(*ch), 1);
        }
    }
    assert_eq!(text(input), "cl");
    assert_eq!(unsafe { SendMessageW(rows, LB_GETCOUNT, 0, 0) }, 1);
    assert!(text(child(launcher, 1003)).contains("calc.exe"));
    native_snapshot::save(launcher, &format!("031-launcher-{theme}-{scale}"));
    set(input, "计算器");
    assert_eq!(unsafe { SendMessageW(rows, LB_GETCOUNT, 0, 0) }, 0);
    set(input, "");
}

fn check_direct_delete(launcher: HWND, pid: u32, data: &Path) {
    let edit = child(launcher, 1001);
    let results = child(launcher, 1002);
    let saved = std::fs::read_to_string(data).unwrap();
    set(edit, "calc");
    press(results, VK_DELETE);
    let dialog = ready_window(pid, "ALTRun.Native.Confirmation", 1);
    press(dialog, VK_ESCAPE);
    wait(|| unsafe { IsWindow(dialog) == 0 });
    assert_eq!(std::fs::read_to_string(data).unwrap(), saved);

    begin(launcher, 18014);
    let dialog = ready_window(pid, "ALTRun.Native.Confirmation", 1);
    press(dialog, VK_RETURN);
    wait(|| unsafe { SendMessageW(results, LB_GETCOUNT, 0, 0) == 0 });
    let entries = altrun_probe::library::Snapshot::open(data).unwrap().entries;
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].alias, "edit");
    assert_eq!(text(edit), "calc");
    assert_eq!(unsafe { IsWindowVisible(launcher) }, 1);
    assert!(window("ALTRun.Native.ShortcutManager", pid).is_null());

    set(edit, "edit");
    press(edit, VK_DELETE);
    let dialog = ready_window(pid, "ALTRun.Native.Confirmation", 1);
    press(dialog, VK_RETURN);
    wait(|| unsafe { SendMessageW(results, LB_GETCOUNT, 0, 0) == 0 });
    assert!(
        altrun_probe::library::Snapshot::open(data)
            .unwrap()
            .entries
            .is_empty()
    );
    std::fs::write(data, saved).unwrap();
}

fn check_large_bookmark_preview(manager: HWND, pid: u32, file: &Path) {
    use std::fmt::Write;

    let mut html = String::from("<!DOCTYPE NETSCAPE-Bookmark-file-1><DL>");
    for i in 0..10_000 {
        let group = if i < 6_000 { "reference" } else { "guide" };
        write!(
            html,
            r#"<DT><A HREF="https://example.com/{group}/{i}">{group}-{i}</A>"#
        )
        .unwrap();
    }
    html.push_str("</DL>");
    std::fs::write(file, html).unwrap();
    begin(manager, 2138);
    let opened = choose_file(pid, file);
    let preview = ready_window(pid, "ALTRun.Native.ImportPreview", 2907);
    let opening = opened.elapsed();
    let grid = child(preview, 2901);
    let filter = child(preview, 2907);
    assert_eq!(count(grid), 10_000);
    let mut timings = Vec::new();
    for _ in 0..7 {
        let start = Instant::now();
        set(filter, "reference");
        assert_eq!(count(grid), 6_000);
        set(filter, "");
        assert_eq!(count(grid), 10_000);
        timings.push(start.elapsed());
    }
    timings.sort_unstable();
    eprintln!(
        "10k bookmark preview open: {opening:?}; filter + clear median: {:?}",
        timings[3]
    );
    assert_ne!(
        unsafe { GetWindowLongW(grid, GWL_STYLE) as u32 & LVS_OWNERDATA },
        0
    );
    press(preview, VK_ESCAPE);
    wait(|| unsafe { IsWindow(preview) == 0 });
}

#[test]
fn windows_031_management_and_import_previews() {
    let dir = std::env::current_dir()
        .unwrap()
        .join("target")
        .join("review-031");
    std::fs::create_dir_all(&dir).unwrap();
    let data = dir.join("shortcuts.tsv");
    let available = dir.join("existing file.txt");
    std::fs::write(&available, "Synthetic target").unwrap();
    let missing = dir.join("no-such-031.txt");
    let original = format!(
        "calc\t计算器\tcalc.exe\t5\tnone\nedit\t文本编辑器\t{}\t0\n",
        missing.display()
    );
    let import = dir.join("incoming.tsv");
    std::fs::write(
        &import,
        "calc\tCalculator\tcalc.exe\t0\npaint\tPaint\tmspaint.exe\t0\n",
    )
    .unwrap();
    let bookmarks = dir.join("bookmarks.html");
    std::fs::write(
        &bookmarks,
        concat!(
            "<!DOCTYPE NETSCAPE-Bookmark-file-1><DL><p><DT><H3>参考</H3><DL><p>",
            r#"<DT><A HREF="https://example.com/calculator?q=a%20b">计算器资料</A>"#,
            r#"<DT><A HREF="https://example.org/rust">Rust</A>"#,
            r#"<DT><A HREF="https://example.org/rust">Duplicate</A>"#,
            r#"<DT><A HREF="about:blank">Unsupported</A>"#,
            "</DL></DL>",
        ),
    )
    .unwrap();
    for (theme, scale) in [
        ("light", "100"),
        ("dark", "125"),
        ("light", "150"),
        ("light", "125"),
        ("light", "200"),
        ("dark", "100"),
        ("dark", "150"),
        ("dark", "200"),
    ] {
        check_launcher_scrollbar(&dir.join("scrollbar.tsv"), theme, scale);
        std::fs::write(&data, &original).unwrap();
        let (mut running, launcher) = start(&data, theme, scale);
        let pid = running.0.id();
        check_launcher(launcher, theme, scale);
        if theme == "light" && scale == "100" {
            check_direct_delete(launcher, pid, &data);
            command(launcher, 18002);
            wait(|| running.0.try_wait().unwrap().is_some());
            (running, _) = start(&data, theme, scale);
        }
        let pid = running.0.id();
        let launcher = ready_window(pid, "ALTRun.Native.PerformanceProbe", 1001);
        command(launcher, 18003);
        let manager = ready_window(pid, "ALTRun.Native.ShortcutManager", 2113);
        check_popup(
            manager,
            pid,
            2140,
            &format!("menu-new-{theme}-{scale}"),
            theme,
        );
        let list = child(manager, 2101);
        let query = child(manager, 2106);
        let cmd = child(manager, 2104);
        check_edit_selection(manager, 2104);
        if theme == "light" && scale == "100" {
            let saved_command = text(cmd);
            set(
                cmd,
                &format!("C:\\Synthetic Path\\{}.exe", "folder ".repeat(32)),
            );
            check_edit_drag_past_right_edge(cmd);
            set(cmd, &saved_command);
        }
        set(query, "文本");
        wait(|| count(list) == 1);
        assert_eq!(text(cmd), "calc.exe");
        native_snapshot::save(manager, &format!("031-manager-filter-{theme}-{scale}"));
        set(child(launcher, 1001), "cl");
        unsafe {
            SendMessageW(child(launcher, 1001), WM_KEYDOWN, VK_F2 as usize, 0);
        }
        wait(|| text(query).is_empty() && count(list) == 2);

        let original_entries = altrun_probe::parse_entries(&original).unwrap();
        begin(manager, 2124);
        let report = ready_window(pid, "ALTRun.Native.MissingPaths", 2180);
        assert_eq!(count(child(report, 2180)), 1);
        if theme == "light" && scale == "100" {
            check_open_report_theme(launcher, pid, list, child(report, 2180), &data);
        }
        native_snapshot::save(report, &format!("031-missing-paths-{theme}-{scale}"));
        press(report, VK_RETURN);
        choose_file(pid, &available);
        wait(|| text(cmd).contains("existing file.txt"));
        assert_eq!(std::fs::read_to_string(&data).unwrap(), original);
        native_snapshot::save(manager, &format!("031-path-repair-{theme}-{scale}"));

        begin(manager, 2129);
        choose_file(pid, &import);
        let preview = ready_window(pid, "ALTRun.Native.ImportPreview", 2902);
        let grid = child(preview, 2901);
        let width = unsafe { SendMessageW(grid, LVM_GETCOLUMNWIDTH, 0, 0) };
        assert!(width > 0);
        unsafe { assert_ne!(SendMessageW(grid, LVM_SETCOLUMNWIDTH, 0, width + 25), 0) };
        native_snapshot::save(preview, &format!("031-import-append-{theme}-{scale}"));
        command(preview, 2906);
        native_snapshot::save(preview, &format!("031-import-replace-{theme}-{scale}"));
        command(preview, 2905);
        press(preview, VK_RETURN);
        wait(|| count(list) == 3);
        assert_eq!(std::fs::read_to_string(&data).unwrap(), original);

        if theme == "light" && scale == "100" {
            check_legacy_import(manager, pid, &dir, list);
            check_large_bookmark_preview(manager, pid, &dir.join("large-bookmarks.html"));
        }

        begin(manager, 2138);
        choose_file(pid, &bookmarks);
        let preview = ready_window(pid, "ALTRun.Native.ImportPreview", 2907);
        let grid = child(preview, 2901);
        assert_eq!(count(grid), 4);
        native_snapshot::save(preview, &format!("ui-bookmark-search-{theme}-{scale}"));
        set(child(preview, 2907), "EXAMPLE.ORG");
        assert_eq!(count(grid), 2);
        set(child(preview, 2907), "参考");
        assert_eq!(count(grid), 4);
        set(child(preview, 2907), "计算器");
        assert_eq!(count(grid), 1);
        unsafe {
            SendMessageW(grid, WM_KEYDOWN, VK_F2 as usize, 0);
        }
        let alias = child(preview, 2911);
        wait(|| unsafe { IsWindowVisible(alias) != 0 });
        native_snapshot::save(preview, &format!("ui-bookmark-alias-{theme}-{scale}"));
        unsafe {
            let mut bounds = RECT::default();
            GetWindowRect(alias, &mut bounds);
            let dc = GetWindowDC(preview);
            let mut preview_bounds = RECT::default();
            GetWindowRect(preview, &mut preview_bounds);
            let corner = GetPixel(
                dc,
                bounds.left - preview_bounds.left,
                bounds.top - preview_bounds.top,
            );
            let row_background = GetPixel(
                dc,
                bounds.left - preview_bounds.left - 2,
                bounds.top - preview_bounds.top,
            );
            ReleaseDC(preview, dc);
            assert_eq!(
                corner, row_background,
                "alias corner should blend with the selected row"
            );
        }
        set(alias, "calculator-links");
        press(alias, VK_RETURN);
        wait(|| unsafe { IsWindowVisible(alias) == 0 });
        command(preview, 2910);
        assert_eq!(
            unsafe {
                windows_sys::Win32::UI::Input::KeyboardAndMouse::IsWindowEnabled(child(
                    preview, 2902,
                ))
            },
            0
        );
        command(preview, 2909);
        set(child(preview, 2907), "");
        native_snapshot::save(preview, &format!("031-browser-bookmarks-{theme}-{scale}"));
        press(preview, VK_RETURN);
        wait(|| count(list) == 5);
        assert_eq!(std::fs::read_to_string(&data).unwrap(), original);
        command(manager, 2113);
        wait(|| std::fs::read_to_string(&data).is_ok_and(|doc| doc.contains("example.org/rust")));
        let saved = altrun_probe::library::Snapshot::open(&data)
            .unwrap()
            .entries;
        assert_eq!(saved.len(), 5);
        assert!(saved.iter().any(|entry| entry.alias == "calculator-links"));
        let repaired = saved.iter().find(|entry| entry.alias == "edit").unwrap();
        assert_eq!(repaired.id, original_entries[1].id);
        assert!(repaired.command.contains("existing file.txt"));
        command(manager, 2114);
        wait(|| unsafe { IsWindow(manager) == 0 });
        check_backup(launcher, pid, &data, theme, scale);
        if theme == "dark" && scale == "200" {
            check_restore_and_restart(&mut running, launcher, &data);
        } else {
            command(launcher, 18002);
            wait(|| running.0.try_wait().unwrap().is_some());
        }
    }
    check_system_contrast(&data);
}
