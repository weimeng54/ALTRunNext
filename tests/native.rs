#![cfg(windows)]

use std::{
    process::{Child, Command},
    thread,
    time::{Duration, Instant},
};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

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
    let _child = Running(
        Command::new(env!("CARGO_BIN_EXE_altrun-probe"))
            .arg("--show")
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
    }
}
