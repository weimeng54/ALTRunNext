// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

#![cfg(windows)]
mod native_snapshot;
mod review_native_support;

use review_native_support::{Running, command, set, text, wait, window};
use std::{path::Path, process::Command, ptr::null_mut};
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, RECT},
    Graphics::Gdi::{GetPixel, GetWindowDC, ReleaseDC},
    UI::{
        Controls::*,
        Input::KeyboardAndMouse::{VK_ESCAPE, VK_F2, VK_RETURN},
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

fn choose_file(pid: u32, file: &Path) {
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
    unsafe {
        SendMessageW(input, EM_SETSEL, 0, -1);
        SendMessageW(input, EM_REPLACESEL, 1, value.as_ptr() as LPARAM);
        assert_eq!(text(input), path);
        let submit = child(picker, IDOK);
        assert_ne!(
            PostMessageW(picker, WM_COMMAND, IDOK as usize, submit as LPARAM),
            0
        );
    }
    wait(|| unsafe { IsWindow(picker) == 0 });
}

fn check_backup(launcher: HWND, pid: u32, data: &Path, theme: &str, scale: &str) {
    let archive = data.with_file_name(format!("synthetic-{theme}-{scale}.zip"));
    if archive.is_file() {
        std::fs::remove_file(&archive).unwrap();
    }
    let original = std::fs::read(data).unwrap();
    command(launcher, 18005);
    let settings = ready_window(pid, "ALTRun.Native.Settings", 2365);
    check_settings_layout(settings, theme, scale);
    native_snapshot::save(settings, &format!("031-backup-settings-{theme}-{scale}"));
    begin(settings, 2365);
    let preview = ready_window(pid, "ALTRun.Native.Confirmation", 1);
    native_snapshot::save(preview, &format!("031-backup-{theme}-{scale}"));
    eprintln!("Confirming the backup preview");
    press(preview, VK_RETURN);
    wait(|| unsafe { IsWindow(preview) == 0 });
    choose_file(pid, &archive);
    wait(|| altrun_probe::backup::Backup::load(&archive).is_ok_and(|backup| backup.count() == 5));
    wait(|| unsafe {
        windows_sys::Win32::UI::Input::KeyboardAndMouse::IsWindowEnabled(child(settings, 2366)) != 0
    });
    begin(settings, 2366);
    choose_file(pid, &archive);
    let preview = ready_window(pid, "ALTRun.Native.Confirmation", 1);
    native_snapshot::save(preview, &format!("031-restore-{theme}-{scale}"));
    press(preview, VK_ESCAPE);
    wait(|| unsafe { IsWindow(preview) == 0 });
    assert_eq!(std::fs::read(data).unwrap(), original);
    unsafe { assert_ne!(PostMessageW(settings, WM_CLOSE, 0, 0), 0) };
    wait(|| unsafe { IsWindow(settings) == 0 });
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
        assert_eq!(text(preview), "cl");
        assert!(!review_native_support::has_text(settings, "例如 db"));
        native_snapshot::save(
            settings,
            &format!("settings-clean-{section}-{theme}-{scale}"),
        );
    }
    select(3);
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
    let settings = ready_window(pid, "ALTRun.Native.Settings", 2366);
    begin(settings, 2366);
    choose_file(pid, &data.with_file_name("synthetic-dark-200.zip"));
    let preview = ready_window(pid, "ALTRun.Native.Confirmation", 1);
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
        command(launcher, 18003);
        let manager = ready_window(pid, "ALTRun.Native.ShortcutManager", 2113);
        let list = child(manager, 2101);
        let query = child(manager, 2106);
        let cmd = child(manager, 2104);
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
}
