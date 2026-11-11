//! Shared helpers for independent native regression test binaries.
#![allow(dead_code)]
use std::{
    path::Path,
    process::{Child, Command},
    ptr::null_mut,
    thread,
    time::{Duration, Instant},
};
use windows_sys::Win32::{Foundation::*, UI::WindowsAndMessaging::*};
pub fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}
pub struct Running(pub Child);
impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
pub fn wait(mut condition: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(10);
    while !condition() {
        assert!(Instant::now() < end, "native regression timed out");
        thread::sleep(Duration::from_millis(10));
    }
}
pub fn window(class: &str, pid: u32) -> HWND {
    unsafe {
        let mut h = FindWindowExW(
            null_mut(),
            null_mut(),
            wide(class).as_ptr(),
            std::ptr::null(),
        );
        while !h.is_null() {
            let mut owner = 0;
            GetWindowThreadProcessId(h, &mut owner);
            if owner == pid {
                return h;
            }
            h = FindWindowExW(null_mut(), h, wide(class).as_ptr(), std::ptr::null());
        }
        null_mut()
    }
}
pub fn start(data: &Path, mode: Option<&str>) -> (Running, HWND) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_altrun-probe"));
    command.args(["--show", "--data"]).arg(data);
    if let Some(mode) = mode {
        command.arg(mode);
    }
    let process = Running(command.spawn().unwrap());
    let mut root = null_mut();
    wait(|| {
        root = window("ALTRun.Native.PerformanceProbe", process.0.id());
        !root.is_null() && unsafe { !GetDlgItem(root, 1001).is_null() }
    });
    (process, root)
}
pub fn manager(root: HWND, pid: u32) -> HWND {
    command(root, 18003);
    let mut dialog = null_mut();
    wait(|| {
        dialog = window("ALTRun.Native.ShortcutManager", pid);
        !dialog.is_null() && unsafe { !GetDlgItem(dialog, 2101).is_null() }
    });
    dialog
}
pub fn command(hwnd: HWND, id: usize) {
    unsafe {
        SendMessageW(hwnd, WM_COMMAND, id, 0);
    }
}
pub fn set(hwnd: HWND, value: &str) {
    unsafe {
        SendMessageW(hwnd, WM_SETTEXT, 0, wide(value).as_ptr() as isize);
    }
}
pub fn text(hwnd: HWND) -> String {
    unsafe {
        let n = SendMessageW(hwnd, WM_GETTEXTLENGTH, 0, 0).max(0) as usize;
        let mut b = vec![0u16; n + 1];
        let n = SendMessageW(hwnd, WM_GETTEXT, b.len(), b.as_mut_ptr() as isize).max(0) as usize;
        String::from_utf16_lossy(&b[..n])
    }
}
pub fn shell() -> std::path::PathBuf {
    altrun_probe::safe_notepad_path()
        .unwrap()
        .with_file_name("cmd.exe")
}
pub fn has_text(hwnd: HWND, needle: &str) -> bool {
    struct Search<'a> {
        needle: &'a str,
        found: bool,
    }
    unsafe extern "system" fn visit(child: HWND, value: LPARAM) -> i32 {
        let search = unsafe { &mut *(value as *mut Search<'_>) };
        if text(child).contains(search.needle) {
            search.found = true;
            0
        } else {
            1
        }
    }
    let mut search = Search {
        needle,
        found: false,
    };
    unsafe {
        EnumChildWindows(hwnd, Some(visit), &mut search as *mut _ as isize);
    }
    search.found
}
