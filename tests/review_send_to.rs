// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

#![cfg(windows)]
mod native_snapshot;
mod review_native_support;
use altrun_probe::library::{OpenTarget, Snapshot};
use review_native_support::*;
use std::process::Command;
use windows_sys::Win32::{Graphics::Gdi::*, UI::WindowsAndMessaging::*};

fn assert_surface(dialog: windows_sys::Win32::Foundation::HWND, expected: u32) {
    unsafe {
        RedrawWindow(
            dialog,
            std::ptr::null(),
            std::ptr::null_mut(),
            RDW_INVALIDATE | RDW_ALLCHILDREN | RDW_UPDATENOW,
        );
        let status = GetDlgItem(dialog, 3108);
        let dc = GetDC(status);
        let actual = GetPixel(dc, 4, 4);
        ReleaseDC(status, dc);
        assert_eq!(
            actual, expected,
            "the empty status area must match the form background"
        );
    }
}

fn send_to(exe: &std::ffi::OsStr, data: &std::path::Path, paths: &[&std::path::Path]) {
    let mut process = Running(
        Command::new(exe)
            .arg("--data")
            .arg(data)
            .arg("--add")
            .args(paths)
            .spawn()
            .unwrap(),
    );
    wait(|| process.0.try_wait().unwrap().is_some());
    assert!(process.0.wait().unwrap().success());
}

#[test]
fn send_to_add_dialog_saves_a_batch_without_overwriting_an_existing_alias() {
    let root = std::env::current_dir()
        .unwrap()
        .join("target")
        .join("send-to-ui");
    std::fs::create_dir_all(&root).unwrap();
    let data = root.join("shortcuts.tsv");
    let original = "已有\tExisting\tnotepad.exe\t0\n";
    std::fs::write(&data, original).unwrap();
    let settings = data.with_extension("settings.ini");
    std::fs::write(&settings, "[GUI]\nLang=zh-Hans\nTheme=light\n").unwrap();
    let file = root.join("已有.txt");
    let folder = root.join("资料 文件夹");
    std::fs::write(&file, "fixture").unwrap();
    std::fs::create_dir_all(&folder).unwrap();
    let exe = std::env::var_os("ALTRUN_RELEASE_SMOKE_EXE")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_ALTRunNext").into());
    let process = Running(
        Command::new(&exe)
            .args(["--show", "--safe-mode", "--ui-scale", "150", "--data"])
            .arg(&data)
            .spawn()
            .unwrap(),
    );
    wait(|| {
        let launcher = window("ALTRun.Native.PerformanceProbe", process.0.id());
        !launcher.is_null() && unsafe { IsWindowVisible(launcher) != 0 }
    });
    send_to(&exe, &data, &[&file, &folder]);
    let mut dialog = std::ptr::null_mut();
    wait(|| {
        dialog = window("ALTRun.Native.AddShortcut", process.0.id());
        !dialog.is_null() && unsafe { IsWindowVisible(dialog) != 0 }
    });
    native_snapshot::save(dialog, "send-to-light");
    assert_surface(dialog, 245 | (247 << 8) | (251 << 16));
    unsafe {
        assert_eq!(text(GetDlgItem(dialog, 3103)), file.to_str().unwrap());
        assert_eq!(SendMessageW(GetDlgItem(dialog, 3104), LB_GETCOUNT, 0, 0), 2);
        command(dialog, 1);
        assert_eq!(std::fs::read_to_string(&data).unwrap(), original);
        assert!(text(GetDlgItem(dialog, 3108)).contains("已存在"));
        set(GetDlgItem(dialog, 3101), "notes");
        command(dialog, 1);
        wait(|| IsWindow(dialog) == 0);
    }
    let entries = Snapshot::open(&data).unwrap().entries;
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[1].alias, "notes");
    assert_eq!(entries[1].command, file.to_str().unwrap());
    assert_eq!(entries[1].open_target, Some(OpenTarget::Document(None)));
    assert_eq!(entries[2].command, folder.to_str().unwrap());
    assert_eq!(entries[2].open_target, Some(OpenTarget::Path));

    let executable = root.join("Actual Program.exe");
    std::fs::copy(altrun_probe::safe_notepad_path().unwrap(), &executable).unwrap();
    let link = root.join("Different Shortcut.lnk");
    let document = root.join("Actual Document.txt");
    let document_link = root.join("Document Shortcut.lnk");
    std::fs::write(&document, "fixture").unwrap();
    let setup = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", concat!(
            "$ErrorActionPreference='Stop';$s=New-Object -ComObject WScript.Shell;",
            "$l=$s.CreateShortcut($env:ALTRUN_TEST_LINK);$l.TargetPath=$env:ALTRUN_TEST_TARGET;",
            "$l.Arguments=$env:ALTRUN_TEST_ARGUMENTS;$l.Save();",
            "$l=$s.CreateShortcut($env:ALTRUN_TEST_DOCUMENT_LINK);",
            "$l.TargetPath=$env:ALTRUN_TEST_DOCUMENT;$l.Save()"
        )])
        .env("ALTRUN_TEST_LINK", &link)
        .env("ALTRUN_TEST_TARGET", &executable)
        .env("ALTRUN_TEST_DOCUMENT_LINK", &document_link)
        .env("ALTRUN_TEST_DOCUMENT", &document)
        .env("ALTRUN_TEST_ARGUMENTS", "--name \"two words\" %p")
        .status()
        .unwrap();
    assert!(setup.success());
    std::fs::write(&settings, "[GUI]\nLang=zh-Hans\nTheme=dark\n").unwrap();
    send_to(&exe, &data, &[&link, &document_link]);
    wait(|| {
        dialog = window("ALTRun.Native.AddShortcut", process.0.id());
        !dialog.is_null() && unsafe { IsWindowVisible(dialog) != 0 }
    });
    unsafe {
        assert_eq!(text(GetDlgItem(dialog, 3101)), "Actual Program");
        assert_eq!(text(GetDlgItem(dialog, 3102)), "Actual Program");
        assert_eq!(text(GetDlgItem(dialog, 3103)), executable.to_str().unwrap());
        assert_eq!(IsWindowVisible(GetDlgItem(dialog, 3105)), 0);
        assert_surface(dialog, 25 | (30 << 8) | (37 << 16));
        native_snapshot::save(dialog, "send-to-dark-link");
        let list = GetDlgItem(dialog, 3104);
        SendMessageW(list, LB_SETCURSEL, 1, 0);
        SendMessageW(
            dialog,
            WM_COMMAND,
            3104 | ((LBN_SELCHANGE as usize) << 16),
            list as isize,
        );
        assert_eq!(text(GetDlgItem(dialog, 3101)), "Actual Document");
        assert_eq!(text(GetDlgItem(dialog, 3102)), "Actual Document.txt");
        assert_eq!(text(GetDlgItem(dialog, 3103)), document.to_str().unwrap());
        assert_ne!(IsWindowVisible(GetDlgItem(dialog, 3105)), 0);
        command(dialog, 1);
    }
    wait(|| unsafe { IsWindow(dialog) == 0 });
    let entries = Snapshot::open(&data).unwrap().entries;
    let (request, _, _) =
        altrun_probe::launch::resolve_entry(&entries[3], None, &Default::default()).unwrap();
    assert_eq!(request.file, executable.to_str().unwrap());
    assert_eq!(request.parameters, "--name \"two words\" %p");
    assert_eq!(entries[4].command, document.to_str().unwrap());
}
