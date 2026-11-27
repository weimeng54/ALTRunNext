// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::*;

pub(super) fn slow_writes_and_conflict() {
    let root = std::env::current_dir()
        .unwrap()
        .join("target/complete-native-slow-write");
    std::fs::create_dir_all(&root).unwrap();
    let data = std::path::absolute(root.join("shortcuts.tsv")).unwrap();
    let ini = data.with_extension("settings.ini");
    std::fs::write(
        &data,
        "alpha\tSynthetic alpha\t__missing__.exe\t0\nbeta\tSynthetic beta\t__missing__.exe\t0\n",
    )
    .unwrap();
    let original = "[GUI]\nRowHeight=22\nLang=zh-Hans\n[Config]\nHotKey1=\nPlayPopupNotify=0\n";
    std::fs::write(&ini, original).unwrap();
    let executable = std::env::var_os("ALTRUN_RELEASE_SMOKE_EXE")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_ALTRunNext").into());
    let process = Running(
        Command::new(executable)
            .args(["--safe-mode", "--show", "--ui-scale", "100", "--data"])
            .arg(&data)
            .env("ALTRUN_TEST_SETTINGS_DATA", &data)
            .env("ALTRUN_TEST_SETTINGS_WRITE_DELAY_MS", "1800")
            .spawn()
            .unwrap(),
    );
    let pid = process.0.id();
    unsafe {
        let mut launcher = null_mut();
        wait(|| {
            launcher = window("ALTRun.Native.PerformanceProbe", pid);
            !launcher.is_null() && !GetDlgItem(launcher, 1002).is_null()
        });
        let open_settings = || {
            let mut dialog = null_mut();
            wait(|| {
                dialog = window("ALTRun.Native.Settings", pid);
                if dialog.is_null() {
                    PostMessageW(launcher, WM_COMMAND, 18005, 0);
                }
                !dialog.is_null()
                    && IsWindowVisible(dialog) != 0
                    && !GetDlgItem(dialog, 2314).is_null()
            });
            select_page(dialog, 3);
            dialog
        };
        let dialog = open_settings();
        let row = setting(dialog, "RowHeight");
        set(row, "30");
        let start = Instant::now();
        assert_ne!(PostMessageW(row, WM_KEYDOWN, 13, 0), 0);
        let query = GetDlgItem(launcher, 1001);
        let results = GetDlgItem(launcher, 1002);
        wait(|| SendMessageW(results, LB_GETITEMHEIGHT, 0, 0) == 30);
        command(launcher, 18001);
        assert_eq!(IsWindowVisible(launcher), 0);
        command(launcher, 18012);
        assert_ne!(IsWindowVisible(launcher), 0);
        for value in ["al", "be", "alpha"] {
            set(query, value);
            assert_eq!(SendMessageW(results, LB_GETCOUNT, 0, 0), 1);
        }
        for value in ["31", "33", "32"] {
            set(row, value);
            notify_control(dialog, row, EN_KILLFOCUS);
        }
        assert!(
            start.elapsed() < Duration::from_millis(1200),
            "UI stalled during a delayed settings write"
        );
        assert_eq!(std::fs::read_to_string(&ini).unwrap(), original);
        command(dialog, 2314);
        assert_eq!(IsWindowVisible(dialog), 0);
        command(launcher, 18005);
        wait(|| IsWindowVisible(dialog) != 0);
        assert_eq!(text(row), "32");
        command(dialog, 2314);
        wait(|| IsWindow(dialog) == 0);
        wait(|| {
            std::fs::read_to_string(&ini)
                .is_ok_and(|doc| doc.lines().any(|line| line == "RowHeight=32"))
        });
        let dialog = open_settings();
        let row = setting(dialog, "RowHeight");
        assert_eq!(text(row), "32");
        let row_height = SendMessageW(results, LB_GETITEMHEIGHT, 0, 0);
        let mut external = std::fs::read_to_string(&ini).unwrap();
        external.push_str("\n[Synthetic]\nExternalEdit=keep\n");
        std::fs::write(&ini, &external).unwrap();
        set(row, "29");
        notify_control(dialog, row, EN_KILLFOCUS);
        wait(|| text(GetDlgItem(dialog, 2303)).contains("外部修改"));
        assert_eq!(text(row), "32");
        wait(|| SendMessageW(results, LB_GETITEMHEIGHT, 0, 0) == row_height);
        assert_eq!(std::fs::read_to_string(&ini).unwrap(), external);
        command(dialog, 2314);
        wait(|| IsWindow(dialog) == 0);
    }
}
