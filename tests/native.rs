#![cfg(windows)]

use std::{
    process::{Child, Command},
    thread,
    time::{Duration, Instant},
};
use windows_sys::Win32::UI::WindowsAndMessaging::*;
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

#[test]
fn safe_notepad_uses_an_absolute_system_path() {
    let path = altrun_probe::safe_notepad_path().unwrap();
    assert!(
        path.is_absolute(),
        "must not search the executable directory or PATH"
    );
    assert_eq!(path.file_name().unwrap(), "notepad.exe");
}

#[test]
fn clicking_a_result_keeps_the_search_input_ready_for_typing() {
    let data = std::env::current_dir().unwrap().join("target/ui-fixture.tsv");
    std::fs::write(&data, concat!(
        "np\t记事本 · 日常文本与临时笔记\t__altrun_missing_test__.exe\t10\n",
        "param\t参数命令 · 中文文件名与带空格路径\t__altrun_missing_test__.exe %p\t9\n",
        "explorer\t资源管理器\texplorer.exe\t8\n",
        "browser\t浏览器\thttps://example.com/\t7\n",
        "long-shortcut-alias-for-ellipsis\t这是一个很长的中文名称，用于检查名称列截断和紧凑排版\tD:\\A long synthetic path\\application.exe\t6\n"
    )).unwrap();
    for scale in ["100", "150"] {
    let _child = Running(
        Command::new(env!("CARGO_BIN_EXE_altrun-probe"))
            .arg("--show")
            .arg("--enable-launch")
            .arg("--ui-scale").arg(scale)
            .arg("--data").arg(&data)
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(8);
    let (hwnd, edit, list) = loop {
        let hwnd = unsafe {
            FindWindowW(
                wide("ALTRun.Native.PerformanceProbe").as_ptr(),
                std::ptr::null(),
            )
        };
        if !hwnd.is_null() {
            let edit = unsafe { GetDlgItem(hwnd, 1001) };
            let list = unsafe { GetDlgItem(hwnd, 1002) };
            if !edit.is_null()
                && !list.is_null()
                && unsafe { SendMessageW(list, LB_GETCOUNT, 0, 0) } >= 2
            {
                break (hwnd, edit, list);
            }
        }
        assert!(Instant::now() < deadline, "native window was not created");
        thread::sleep(Duration::from_millis(20));
    };
    unsafe {
        // WM_NULL is a readiness barrier, not a latency measurement.
        let mut response = 0;
        assert_ne!(
            SendMessageTimeoutW(hwnd, WM_NULL, 0, 0, SMTO_ABORTIFHUNG, 5000, &mut response),
            0
        );
        assert!(!edit.is_null() && !list.is_null());
        let row_height = SendMessageW(list, LB_GETITEMHEIGHT, 0, 0) as u32;
        let click = (((row_height + row_height / 2) << 16) | 5) as isize;
        SendMessageW(list, WM_LBUTTONDOWN, 1, click);
        SendMessageW(list, WM_LBUTTONUP, 0, click);
        let tid = GetWindowThreadProcessId(hwnd, std::ptr::null_mut());
        let mut info: GUITHREADINFO = std::mem::zeroed();
        info.cbSize = std::mem::size_of::<GUITHREADINFO>() as u32;
        assert_ne!(GetGUIThreadInfo(tid, &mut info), 0);
        assert_eq!(
            info.hwndFocus, edit,
            "list selection must not steal subsequent Enter/Esc/text input"
        );
        native_snapshot::save(hwnd, &format!("normal-{scale}"));
        SendMessageW(edit, WM_IME_STARTCOMPOSITION, 0, 0);
        SendMessageW(edit, WM_KEYDOWN, 13, 0);
        assert_ne!(IsWindowVisible(hwnd), 0, "IME Enter must not launch");
        SendMessageW(edit, WM_IME_ENDCOMPOSITION, 0, 0);
        SetWindowTextW(edit, wide("param").as_ptr());
        SendMessageW(edit, WM_KEYDOWN, 13, 0);
        assert_eq!(GetWindowTextLengthW(edit), 0, "parameter mode accepts a separate value");
        native_snapshot::save(hwnd, &format!("parameter-{scale}"));
        SendMessageW(edit, WM_KEYDOWN, 27, 0);
        let mut restored = [0_u16; 32]; GetWindowTextW(edit, restored.as_mut_ptr(), 32);
        assert_eq!(String::from_utf16_lossy(&restored[..5]), "param");
        SetWindowTextW(edit, wide("np").as_ptr());
        SendMessageW(edit, WM_KEYDOWN, 13, 0);
        let failure_deadline = Instant::now() + Duration::from_secs(8);
        while IsWindowVisible(hwnd) == 0 {
            assert!(Instant::now() < failure_deadline, "failed launch must return for correction");
            thread::sleep(Duration::from_millis(20));
        }
        let command = GetDlgItem(hwnd, 1003);
        let mut error = [0_u16; 256]; let length = GetWindowTextW(command, error.as_mut_ptr(), 256);
        assert!(String::from_utf16_lossy(&error[..length as usize]).contains("无法启动"));
        native_snapshot::save(hwnd, &format!("error-{scale}"));
        SetWindowTextW(edit, wide("no-matching-synthetic-shortcut").as_ptr());
        assert_eq!(SendMessageW(list, LB_GETCOUNT, 0, 0), 0);
        assert_ne!(IsWindowVisible(GetDlgItem(hwnd, 1004)), 0);
        SendMessageW(edit, WM_KEYDOWN, 13, 0);
        assert_ne!(IsWindowVisible(hwnd), 0, "empty result must not launch or hide");
        native_snapshot::save(hwnd, &format!("empty-{scale}"));
    }
    }
}
