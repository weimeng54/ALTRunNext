// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

#![cfg(windows)]
mod review_native_support;
use review_native_support::*;
use windows_sys::Win32::UI::{
    Controls::EM_SETSEL, Input::KeyboardAndMouse::*, WindowsAndMessaging::*,
};
#[test]
fn parameter_edit_keeps_apostrophes_semicolons_delete_and_frozen_target() {
    let root = std::env::current_dir()
        .unwrap()
        .join("target/review-parameter-edit");
    std::fs::create_dir_all(&root).unwrap();
    let data = root.join("shortcuts.tsv");
    let marker = root.join("parameter.txt");
    let _ = std::fs::remove_file(&marker);
    std::fs::write(
        &data,
        format!(
            "param\tSynthetic parameter\t\"{}\" /D /C echo {{%p}}>\"{}\"\t0\traw\n",
            shell().display(),
            marker.display()
        ),
    )
    .unwrap();
    let (process, launcher) = start(&data, Some("--enable-launch"));
    unsafe {
        let search = GetDlgItem(launcher, 1001);
        set(search, "param");
        SendMessageW(search, WM_KEYDOWN, VK_RETURN as usize, 0);
        let mut dialog = std::ptr::null_mut();
        wait(|| {
            dialog = window("ALTRun.Native.ParameterDialog", process.0.id());
            !dialog.is_null() && IsWindowVisible(dialog) != 0
        });
        let edit = GetDlgItem(dialog, 10);
        for c in "O'Brien;a".encode_utf16() {
            SendMessageW(edit, WM_CHAR, c as usize, 0);
        }
        assert_eq!(text(edit), "O'Brien;a");
        check_edit_selection(dialog, 10);
        SendMessageW(edit, EM_SETSEL, 0, 1);
        SendMessageW(edit, WM_KEYDOWN, VK_DELETE as usize, 0);
        assert_eq!(text(edit), "'Brien;a");
        assert!(window("ALTRun.Native.ShortcutManager", process.0.id()).is_null());
        assert_eq!(text(search), "param");
        PostMessageW(edit, WM_KEYDOWN, VK_RETURN as usize, 0);
    }
    // The child shell creates the redirection target before writing the line.
    wait(|| std::fs::read_to_string(&marker).is_ok_and(|text| text.ends_with('\n')));
    assert_eq!(std::fs::read_to_string(marker).unwrap().trim(), "'Brien;a");
}
