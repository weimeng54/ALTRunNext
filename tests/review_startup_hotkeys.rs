#![cfg(windows)]
mod native_snapshot;
mod review_native_support;

use review_native_support::{Running, command, has_text, wait, window};
use std::{process::Command, ptr::null_mut};
use windows_sys::Win32::UI::{Input::KeyboardAndMouse::*, WindowsAndMessaging::*};

struct ReservedHotkey;

impl Drop for ReservedHotkey {
    fn drop(&mut self) {
        unsafe {
            UnregisterHotKey(null_mut(), 1);
        }
    }
}

#[test]
fn startup_shows_launcher_and_exits_cleanly_when_hotkey_is_taken() {
    let folder = std::env::current_dir()
        .unwrap()
        .join("target/review-startup-hotkeys");
    std::fs::create_dir_all(&folder).unwrap();
    let data = folder.join("shortcuts.tsv");
    std::fs::write(&data, "test\tFixture\tnotepad.exe\t0\n").unwrap();
    std::fs::write(
        data.with_extension("settings.ini"),
        "[Config]\nLang=zh-Hans\nHotKey1=Ctrl+Alt+F10\nLastItemHotKey=\nShowMeWhenStart=0\n",
    )
    .unwrap();
    assert_ne!(
        unsafe { RegisterHotKey(null_mut(), 1, MOD_CONTROL | MOD_ALT, VK_F10 as u32) },
        0
    );
    let _key = ReservedHotkey;
    let exe = std::env::var_os("ALTRUN_RELEASE_SMOKE_EXE")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_ALTRunNext").into());
    let mut process = Running(
        Command::new(exe)
            .args(["--safe-mode", "--data"])
            .arg(&data)
            .spawn()
            .unwrap(),
    );
    let mut launcher = null_mut();
    wait(|| {
        launcher = window("ALTRun.Native.PerformanceProbe", process.0.id());
        !launcher.is_null() && unsafe { IsWindowVisible(launcher) != 0 }
    });
    let mut warning = null_mut();
    wait(|| {
        warning = window("ALTRun.Native.StartupWarning", process.0.id());
        !warning.is_null() && unsafe { IsWindowVisible(warning) != 0 }
    });
    assert!(has_text(warning, "Ctrl+Alt+F10"));
    native_snapshot::save(warning, "startup-hotkey-warning");
    command(warning, 1);
    let mut settings = null_mut();
    wait(|| {
        settings = window("ALTRun.Native.Settings", process.0.id());
        !settings.is_null() && unsafe { IsWindowVisible(settings) != 0 }
    });
    assert_eq!(
        unsafe { SendMessageW(GetDlgItem(settings, 2301), LB_GETCURSEL, 0, 0) },
        1
    );
    command(settings, 2314);
    wait(|| unsafe { IsWindow(settings) == 0 });
    command(launcher, 18002);
    wait(|| process.0.try_wait().unwrap().is_some());
    assert!(process.0.wait().unwrap().success());
}
