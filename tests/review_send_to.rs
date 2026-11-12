#![cfg(windows)]
mod review_native_support;
use altrun_probe::library::{OpenTarget, Snapshot};
use review_native_support::*;
use std::process::Command;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

#[test]
fn send_to_add_dialog_saves_a_batch_without_overwriting_an_existing_alias() {
    let root = std::env::current_dir().unwrap().join("target/send-to-ui");
    std::fs::create_dir_all(&root).unwrap();
    let data = root.join("shortcuts.tsv");
    let original = "已有\tExisting\tnotepad.exe\t0\n";
    std::fs::write(&data, original).unwrap();
    let file = root.join("已有.txt");
    let folder = root.join("资料 文件夹");
    std::fs::write(&file, "fixture").unwrap();
    std::fs::create_dir_all(&folder).unwrap();
    let (process, _) = start(&data, Some("--safe-mode"));
    let second = Command::new(env!("CARGO_BIN_EXE_altrun-probe"))
        .arg("--data")
        .arg(&data)
        .arg("--add")
        .args([&file, &folder])
        .status()
        .unwrap();
    assert!(second.success());
    let mut dialog = std::ptr::null_mut();
    wait(|| {
        dialog = window("ALTRun.Native.AddShortcut", process.0.id());
        !dialog.is_null() && unsafe { IsWindowVisible(dialog) != 0 }
    });
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
}
