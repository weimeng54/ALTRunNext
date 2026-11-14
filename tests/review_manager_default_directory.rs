// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

#![cfg(windows)]
mod review_native_support;
use review_native_support::*;
use windows_sys::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_COMMAND};
#[test]
fn missing_path_check_honors_the_configured_default_directory() {
    let root = std::env::current_dir()
        .unwrap()
        .join("target/review-manager-directory");
    let folder = root.join("data");
    let working = root.join("working");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::create_dir_all(working.join("bin")).unwrap();
    std::fs::write(working.join("bin/app.exe"), "fixture").unwrap();
    let data = folder.join("shortcuts.tsv");
    std::fs::write(&data, "app\tFixture\t\".\\bin\\app.exe\" --flag\t0\tnone\n").unwrap();
    std::fs::write(
        data.with_extension("settings.ini"),
        format!(
            "[Config]\nWorkingDirectory={}\nLang=zh-Hans\n",
            working.display()
        ),
    )
    .unwrap();
    let (process, launcher) = start(&data, Some("--safe-mode"));
    let dialog = manager(launcher, process.0.id());
    unsafe { PostMessageW(dialog, WM_COMMAND, 2124, 0) };
    let mut report = std::ptr::null_mut();
    wait(|| {
        report = window("ALTRun.Native.Confirmation", process.0.id());
        !report.is_null() && has_text(report, "可用 1 项 · 失效 0 项")
    });
    command(report, 1);
}
