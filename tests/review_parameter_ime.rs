#![cfg(windows)]
mod review_native_support;
use review_native_support::*;
use windows_sys::Win32::{
    Foundation::HWND,
    UI::{Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
};
#[test]
fn manager_parameter_dialog_does_not_submit_an_ime_candidate_commit() {
    let root = std::env::current_dir()
        .unwrap()
        .join("target/review-parameter-ime");
    std::fs::create_dir_all(&root).unwrap();
    let data = root.join("shortcuts.tsv");
    let marker = root.join("launched.txt");
    let _ = std::fs::remove_file(&marker);
    std::fs::write(
        &data,
        format!(
            "param\tSynthetic IME\t\"{}\" /D /C echo ok>\"{}\"\t0\traw\n",
            shell().display(),
            marker.display()
        ),
    )
    .unwrap();
    let (process, launcher) = start(&data, Some("--enable-launch"));
    let manager = manager(launcher, process.0.id());
    let owner = manager as usize;
    let pending = std::thread::spawn(move || command(owner as HWND, 2137));
    let mut dialog = std::ptr::null_mut();
    wait(|| {
        dialog = window("ALTRun.Native.ParameterDialog", process.0.id());
        !dialog.is_null() && unsafe { IsWindowVisible(dialog) != 0 }
    });
    unsafe {
        let edit = GetDlgItem(dialog, 10);
        let ok = GetDlgItem(dialog, 1);
        set(edit, "中文");
        SendMessageW(edit, WM_IME_STARTCOMPOSITION, 0, 0);
        PostMessageW(edit, WM_KEYDOWN, VK_RETURN as usize, 0);
        PostMessageW(ok, BM_SETSTATE, 1, 0);
        wait(|| IsWindow(dialog) == 0 || SendMessageW(ok, BM_GETSTATE, 0, 0) & 4 != 0);
        assert_ne!(
            IsWindow(dialog),
            0,
            "IME candidate Enter submitted the parameter dialog"
        );
        assert!(!marker.exists());
        SendMessageW(ok, BM_SETSTATE, 0, 0);
        SendMessageW(edit, WM_IME_ENDCOMPOSITION, 0, 0);
        PostMessageW(edit, WM_KEYDOWN, VK_RETURN as usize, 0);
        wait(|| IsWindow(dialog) == 0);
    }
    pending.join().unwrap();
    wait(|| marker.exists());
}
