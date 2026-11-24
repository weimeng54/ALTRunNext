// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

#![cfg(windows)]

use std::{
    process::{Child, Command},
    thread,
    time::{Duration, Instant},
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{IsWindowEnabled, VK_ESCAPE};
use windows_sys::Win32::UI::WindowsAndMessaging::*;
use windows_sys::Win32::{
    Foundation::{HWND, POINT, RECT},
    Graphics::Gdi::{
        GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint, MonitorFromWindow,
    },
};
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

fn hit_test_near_bottom(hwnd: windows_sys::Win32::Foundation::HWND, child: i32) -> isize {
    use windows_sys::Win32::Foundation::RECT;
    unsafe {
        let mut bounds: RECT = std::mem::zeroed();
        assert_ne!(GetWindowRect(GetDlgItem(hwnd, child), &mut bounds), 0);
        let x = (bounds.left + bounds.right) / 2;
        let y = bounds.bottom - 2;
        let point = ((y as u16 as u32) << 16) | x as u16 as u32;
        SendMessageW(hwnd, WM_NCHITTEST, 0, point as isize)
    }
}

fn centered_work_area(hwnd: HWND) -> (RECT, RECT) {
    unsafe {
        let mut cursor = POINT::default();
        assert_ne!(GetCursorPos(&mut cursor), 0);
        let monitor = MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST);
        assert_eq!(MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST), monitor);
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..std::mem::zeroed()
        };
        assert_ne!(GetMonitorInfoW(monitor, &mut info), 0);
        let mut bounds = RECT::default();
        assert_ne!(GetWindowRect(hwnd, &mut bounds), 0);
        let work = info.rcWork;
        assert!((bounds.left + bounds.right - work.left - work.right).abs() <= 1);
        assert!((bounds.top + bounds.bottom - work.top - work.bottom).abs() <= 1);
        (bounds, work)
    }
}

fn check_centered_editor(launcher: HWND, pid: u32, action: usize, class: &str, scale: &str) {
    unsafe {
        SendMessageW(launcher, WM_COMMAND, action, 0);
        let deadline = Instant::now() + Duration::from_secs(8);
        let class = wide(class);
        let hwnd = loop {
            let mut window = std::ptr::null_mut();
            loop {
                window = FindWindowExW(
                    std::ptr::null_mut(),
                    window,
                    class.as_ptr(),
                    std::ptr::null(),
                );
                if window.is_null() {
                    break;
                }
                let mut owner = 0;
                GetWindowThreadProcessId(window, &mut owner);
                if owner == pid && IsWindowVisible(window) != 0 {
                    break;
                }
            }
            if !window.is_null() {
                break window;
            }
            assert!(Instant::now() < deadline, "editor was not shown");
            thread::sleep(Duration::from_millis(20));
        };
        let (bounds, work) = centered_work_area(hwnd);
        let dpi = (scale.parse::<i32>().unwrap() * 96 / 100)
            .min((work.right - work.left - 48) * 96 / 1102)
            .min((work.bottom - work.top - 80) * 96 / 721)
            .max(48);
        assert_eq!(bounds.right - bounds.left, (1102 * dpi + 48) / 96);
        assert_eq!(bounds.bottom - bounds.top, (721 * dpi + 48) / 96);
        native_snapshot::save(hwnd, &format!("editor-{action}-{scale}"));
        assert_ne!(PostMessageW(hwnd, WM_KEYDOWN, VK_ESCAPE as usize, 0), 0);
        let close_deadline = Instant::now() + Duration::from_secs(8);
        while IsWindow(hwnd) != 0 {
            assert!(
                Instant::now() < close_deadline,
                "Esc did not close the editor"
            );
            thread::sleep(Duration::from_millis(20));
        }
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
    let data = std::env::current_dir()
        .unwrap()
        .join("target/ui-fixture.tsv");
    let marker = std::env::current_dir()
        .unwrap()
        .join("target/ui-launch-marker.txt");
    let shell = altrun_probe::safe_notepad_path()
        .unwrap()
        .with_file_name("cmd.exe");
    std::fs::write(&data, concat!(
        "np\t记事本 · 日常文本与临时笔记\t__altrun_missing_test__.exe\t10\n",
        "param\t参数命令 · 中文文件名与带空格路径\t__altrun_missing_test__.exe %p\t9\n",
        "explorer\t资源管理器\texplorer.exe\t8\n",
        "browser\t浏览器\thttps://example.com/\t7\n",
        "long-shortcut-alias-for-ellipsis\t这是一个很长的中文名称，用于检查名称列截断和紧凑排版\tD:\\A long synthetic path\\application.exe\t6\n"
    )).unwrap();
    let success_command = format!(
        "self\t启动成功自检\t\"{}\" /D /C echo ok>\"{}\"\t0\n",
        shell.display(),
        marker.display()
    );
    let mut fixture = std::fs::read_to_string(&data).unwrap();
    fixture.push_str("r\t运行\t{%p}\t0\traw\n");
    fixture.push_str(&success_command);
    for i in 0..11 {
        fixture.push_str(&format!("z{i}\tExtra command {i}\t__missing__.exe\t0\n"));
    }
    std::fs::write(&data, fixture).unwrap();
    std::fs::write(data.with_extension("settings.ini"), "[GUI]\nLang=zh-Hans\n").unwrap();
    let executable = std::env::var_os("ALTRUN_RELEASE_SMOKE_EXE")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_ALTRunNext").into());
    for scale in ["100", "125", "150", "200"] {
        let _child = Running(
            Command::new(&executable)
                .arg("--show")
                .arg("--enable-launch")
                .arg("--ui-scale")
                .arg(scale)
                .arg("--data")
                .arg(&data)
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
                if !edit.is_null() && !list.is_null() && unsafe { IsWindowVisible(hwnd) } != 0 {
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
            centered_work_area(hwnd);
            assert_eq!(hit_test_near_bottom(hwnd, 1000), HTCAPTION as isize);
            assert_eq!(hit_test_near_bottom(hwnd, 1001), HTCLIENT as isize);
            SendMessageW(hwnd, WM_ENTERSIZEMOVE, 0, 0);
            SendMessageW(hwnd, WM_TIMER, 17030, 0);
            assert_ne!(IsWindowVisible(hwnd), 0, "drag must defer auto-hide");
            SendMessageW(hwnd, WM_EXITSIZEMOVE, 0, 0);
            assert_eq!(SendMessageW(edit, WM_GETTEXTLENGTH, 0, 0), 0);
            assert_eq!(SendMessageW(list, LB_GETCOUNT, 0, 0), 18);
            SendMessageW(edit, WM_SETTEXT, 0, wide("*").as_ptr() as isize);
            assert_eq!(SendMessageW(list, LB_GETCOUNT, 0, 0), 0);
            SendMessageW(edit, WM_SETTEXT, 0, wide("z").as_ptr() as isize);
            assert_eq!(SendMessageW(list, LB_GETCOUNT, 0, 0), 11);
            SendMessageW(edit, WM_KEYDOWN, 0x26, 0);
            assert_eq!(SendMessageW(list, LB_GETCURSEL, 0, 0), 10);
            assert!(SendMessageW(list, LB_GETTOPINDEX, 0, 0) > 0);
            SendMessageW(edit, WM_SETTEXT, 0, wide("Z").as_ptr() as isize);
            assert_eq!(SendMessageW(list, LB_GETCURSEL, 0, 0), 0);
            assert_eq!(SendMessageW(list, LB_GETTOPINDEX, 0, 0), 0);
            SendMessageW(edit, WM_SETTEXT, 0, [0_u16].as_ptr() as isize);
            assert_eq!(SendMessageW(list, LB_GETCOUNT, 0, 0), 18);
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
            SendMessageW(edit, WM_SETTEXT, 0, wide("np").as_ptr() as isize);
            // "np" is exact and the long alias also contains n before p.
            assert_eq!(SendMessageW(list, LB_GETCOUNT, 0, 0), 2);
            SendMessageW(edit, WM_IME_STARTCOMPOSITION, 0, 0);
            SendMessageW(edit, WM_KEYDOWN, 13, 0);
            assert_ne!(IsWindowVisible(hwnd), 0, "IME Enter must not launch");
            SendMessageW(edit, WM_IME_ENDCOMPOSITION, 0, 0);
            SendMessageW(edit, WM_SETTEXT, 0, wide("r").as_ptr() as isize);
            let original_count = SendMessageW(list, LB_GETCOUNT, 0, 0);
            let original_row = SendMessageW(list, LB_GETCURSEL, 0, 0);
            SendMessageW(edit, WM_KEYDOWN, 13, 0);
            let prompt_deadline = Instant::now() + Duration::from_secs(8);
            let prompt = loop {
                let prompt = FindWindowW(
                    wide("ALTRun.Native.ParameterDialog").as_ptr(),
                    wide("运行").as_ptr(),
                );
                if !prompt.is_null() && IsWindowVisible(prompt) != 0 {
                    break prompt;
                }
                assert!(
                    Instant::now() < prompt_deadline,
                    "parameter prompt did not appear"
                );
                thread::sleep(Duration::from_millis(15));
            };
            let parameter = GetDlgItem(prompt, 10);
            assert_ne!(parameter, edit);
            assert_eq!(SendMessageW(edit, WM_GETTEXTLENGTH, 0, 0), 1);
            assert_eq!(IsWindowEnabled(hwnd), 0);
            let output = GetDlgItem(prompt, 12);
            SendMessageW(parameter, WM_IME_STARTCOMPOSITION, 0, 0);
            PostMessageW(parameter, WM_KEYDOWN, 13, 0);
            let ok = GetDlgItem(prompt, 1);
            PostMessageW(ok, BM_SETSTATE, 1, 0);
            let ime_deadline = Instant::now() + Duration::from_secs(5);
            while SendMessageW(ok, BM_GETSTATE, 0, 0) & 4 == 0 {
                assert!(IsWindow(prompt) != 0 && Instant::now() < ime_deadline);
                thread::sleep(Duration::from_millis(15));
            }
            assert_eq!(SendMessageW(output, WM_GETTEXTLENGTH, 0, 0), 0);
            SendMessageW(ok, BM_SETSTATE, 0, 0);
            SendMessageW(parameter, WM_IME_ENDCOMPOSITION, 0, 0);
            SendMessageW(hwnd, WM_HOTKEY, 17001, 0);
            assert_ne!(GetGUIThreadInfo(tid, &mut info), 0);
            assert_eq!(info.hwndFocus, parameter);
            assert_ne!(IsWindowVisible(prompt), 0);
            SendMessageW(
                parameter,
                WM_SETTEXT,
                0,
                wide("__altrun_missing_run_test__.exe").as_ptr() as isize,
            );
            PostMessageW(parameter, WM_KEYDOWN, 13, 0);
            let failure_deadline = Instant::now() + Duration::from_secs(8);
            while SendMessageW(output, WM_GETTEXTLENGTH, 0, 0) == 0 {
                assert!(IsWindow(prompt) != 0 && Instant::now() < failure_deadline);
                thread::sleep(Duration::from_millis(15));
            }
            let mut value = [0_u16; 64];
            let length = SendMessageW(
                parameter,
                WM_GETTEXT,
                value.len(),
                value.as_mut_ptr() as isize,
            );
            assert_eq!(
                String::from_utf16_lossy(&value[..length as usize]),
                "__altrun_missing_run_test__.exe"
            );
            native_snapshot::save(prompt, &format!("parameter-{scale}"));
            PostMessageW(parameter, WM_KEYDOWN, 27, 0);
            let cancel_deadline = Instant::now() + Duration::from_secs(5);
            while IsWindowEnabled(hwnd) == 0 {
                assert!(Instant::now() < cancel_deadline);
                thread::sleep(Duration::from_millis(15));
            }
            assert_eq!(IsWindow(prompt), 0);
            let mut restored = [0_u16; 32];
            SendMessageW(edit, WM_GETTEXT, 32, restored.as_mut_ptr() as isize);
            assert_eq!(String::from_utf16_lossy(&restored[..1]), "r");
            assert_eq!(SendMessageW(list, LB_GETCOUNT, 0, 0), original_count);
            assert_eq!(SendMessageW(list, LB_GETCURSEL, 0, 0), original_row);
            SendMessageW(hwnd, WM_HOTKEY, 17001, 0);
            assert_eq!(IsWindowVisible(hwnd), 0);
            SendMessageW(hwnd, WM_HOTKEY, 17001, 0);
            assert_ne!(IsWindowVisible(hwnd), 0);
            assert_eq!(SendMessageW(edit, WM_GETTEXTLENGTH, 0, 0), 0);
            assert_eq!(SendMessageW(list, LB_GETCOUNT, 0, 0), 18);
            assert_ne!(
                SendMessageW(GetDlgItem(hwnd, 1003), WM_GETTEXTLENGTH, 0, 0),
                0
            );
            assert_eq!(IsWindowVisible(GetDlgItem(hwnd, 1004)), 0);
            SendMessageW(edit, WM_SETTEXT, 0, wide("np").as_ptr() as isize);
            SendMessageW(edit, WM_KEYDOWN, 13, 0);
            let failure_deadline = Instant::now() + Duration::from_secs(8);
            while IsWindowVisible(hwnd) == 0 {
                assert!(
                    Instant::now() < failure_deadline,
                    "failed launch must return for correction"
                );
                thread::sleep(Duration::from_millis(20));
            }
            let command = GetDlgItem(hwnd, 1005);
            let mut error = [0_u16; 256];
            let length = SendMessageW(command, WM_GETTEXT, 256, error.as_mut_ptr() as isize);
            assert!(String::from_utf16_lossy(&error[..length as usize]).contains("无法启动"));
            native_snapshot::save(hwnd, &format!("error-{scale}"));
            SendMessageW(
                edit,
                WM_SETTEXT,
                0,
                wide("no-matching-synthetic-shortcut").as_ptr() as isize,
            );
            assert_eq!(SendMessageW(list, LB_GETCOUNT, 0, 0), 0);
            assert_ne!(IsWindowVisible(GetDlgItem(hwnd, 1004)), 0);
            let target = edit as usize;
            let pending = thread::spawn(move || {
                SendMessageW(target as _, WM_KEYDOWN, 13, 0);
            });
            let prompt_deadline = Instant::now() + Duration::from_secs(8);
            let prompt_class = wide("ALTRun.Native.Confirmation");
            loop {
                let mut prompt = FindWindowExW(
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    prompt_class.as_ptr(),
                    std::ptr::null(),
                );
                let mut found = false;
                while !prompt.is_null() {
                    let mut pid = 0;
                    GetWindowThreadProcessId(prompt, &mut pid);
                    if pid == _child.0.id() && IsWindowVisible(prompt) != 0 {
                        assert!(!GetDlgItem(prompt, IDOK).is_null());
                        assert!(!GetDlgItem(prompt, IDCANCEL).is_null());
                        SendMessageW(prompt, WM_COMMAND, IDCANCEL as usize, 0);
                        found = true;
                        break;
                    }
                    prompt = FindWindowExW(
                        std::ptr::null_mut(),
                        prompt,
                        prompt_class.as_ptr(),
                        std::ptr::null(),
                    );
                }
                if found {
                    break;
                }
                assert!(
                    Instant::now() < prompt_deadline,
                    "empty-result confirmation did not appear"
                );
                thread::sleep(Duration::from_millis(15));
            }
            pending.join().unwrap();
            assert_ne!(
                IsWindowVisible(hwnd),
                0,
                "empty result must not launch or hide"
            );
            native_snapshot::save(hwnd, &format!("empty-{scale}"));
            let _ = std::fs::remove_file(&marker);
            SendMessageW(edit, WM_SETTEXT, 0, wide("r").as_ptr() as isize);
            SendMessageW(edit, WM_KEYDOWN, 13, 0);
            let prompt_deadline = Instant::now() + Duration::from_secs(8);
            loop {
                let prompt = FindWindowW(
                    wide("ALTRun.Native.ParameterDialog").as_ptr(),
                    wide("运行").as_ptr(),
                );
                if !prompt.is_null() && IsWindowVisible(prompt) != 0 {
                    let edit = GetDlgItem(prompt, 10);
                    let command = format!(
                        r#""{}" /D /C echo ok>"{}""#,
                        shell.display(),
                        marker.display()
                    );
                    SendMessageW(edit, WM_SETTEXT, 0, wide(&command).as_ptr() as isize);
                    PostMessageW(edit, WM_KEYDOWN, 13, 0);
                    break;
                }
                assert!(Instant::now() < prompt_deadline);
                thread::sleep(Duration::from_millis(15));
            }
            let launch_deadline = Instant::now() + Duration::from_secs(10);
            while std::fs::read_to_string(&marker).unwrap_or_default().trim() != "ok"
                || IsWindowVisible(hwnd) != 0
            {
                assert!(
                    Instant::now() < launch_deadline,
                    "selected command did not run"
                );
                thread::sleep(Duration::from_millis(20));
            }
            assert_eq!(std::fs::read_to_string(&marker).unwrap().trim(), "ok");
            assert_eq!(IsWindowVisible(hwnd), 0);
            check_centered_editor(hwnd, _child.0.id(), 18005, "ALTRun.Native.Settings", scale);
            check_centered_editor(
                hwnd,
                _child.0.id(),
                18003,
                "ALTRun.Native.ShortcutManager",
                scale,
            );
        }
    }
}
