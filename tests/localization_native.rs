#![cfg(windows)]

mod native_snapshot;
mod review_native_support;

use review_native_support::*;
use std::{fs, process::Command, ptr::null_mut};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

#[test]
fn portable_language_pack_is_used_by_settings_and_presets() {
    let root = std::env::current_dir()
        .unwrap()
        .join("target/localized-portable");
    let languages = root.join("languages");
    fs::create_dir_all(&languages).unwrap();
    for code in ["de", "es", "fr", "ja", "ko"] {
        let file = format!("{code}.json");
        fs::copy(
            std::path::Path::new("languages").join(&file),
            languages.join(&file),
        )
        .unwrap();
    }
    let source = std::env::var_os("ALTRUN_RELEASE_SMOKE_EXE")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_ALTRunNext").into());
    let exe = root.join("ALTRunNext.exe");
    fs::copy(source, &exe).unwrap();

    for (code, title, theme, scale) in [
        ("ja", "ALTRunNext · 設定", "light", "100"),
        ("de", "ALTRunNext · Einstellungen", "dark", "125"),
        ("fr", "ALTRunNext · Paramètres", "light", "150"),
        ("ko", "ALTRunNext · 설정", "dark", "200"),
        ("es", "ALTRunNext · Ajustes", "light", "100"),
    ] {
        let data = root.join("shortcuts.tsv");
        if data.exists() {
            fs::remove_file(&data).unwrap();
        }
        fs::write(
            root.join("shortcuts.settings.ini"),
            format!("[GUI]\nLang={code}\nTheme={theme}\n"),
        )
        .unwrap();
        let child = Running(
            Command::new(&exe)
                .args(["--show", "--safe-mode", "--ui-scale", scale])
                .spawn()
                .unwrap(),
        );
        let mut launcher = null_mut();
        wait(|| {
            launcher = window("ALTRun.Native.PerformanceProbe", child.0.id());
            !launcher.is_null() && unsafe { IsWindowVisible(launcher) != 0 }
        });
        let saved = altrun_probe::library::Snapshot::open(&data).unwrap();
        let expected = altrun_probe::localization::Catalog::load(&languages).resolve(code, []);
        assert_eq!(saved.entries[0].name, expected.text("ui.notepad"));
        command(launcher, 18005);
        let mut settings = null_mut();
        wait(|| {
            settings = window("ALTRun.Native.Settings", child.0.id());
            !settings.is_null() && unsafe { IsWindowVisible(settings) != 0 }
        });
        assert_eq!(text(settings), title);
        let language = altrun_probe::preferences::SPECS
            .iter()
            .position(|spec| spec.key == "Lang")
            .unwrap();
        unsafe {
            let combo = GetDlgItem(settings, (2400 + language) as i32);
            assert_eq!(SendMessageW(combo, CB_GETCOUNT, 0, 0), 8);
            let selected = SendMessageW(combo, CB_GETCURSEL, 0, 0);
            let mut label = [0u16; 64];
            let length = SendMessageW(
                combo,
                CB_GETLBTEXT,
                selected as usize,
                label.as_mut_ptr() as isize,
            );
            assert!(length > 0);
            assert_eq!(
                String::from_utf16_lossy(&label[..length as usize]),
                match code {
                    "ja" => "日本語",
                    "de" => "Deutsch",
                    "fr" => "Français",
                    "ko" => "한국어",
                    _ => "Español",
                },
            );
        }
        native_snapshot::save(settings, &format!("language-{code}-{scale}-settings"));
        command(launcher, 18007);
        let tabs = unsafe { GetDlgItem(settings, 2301) };
        wait(|| unsafe { SendMessageW(tabs, LB_GETCURSEL, 0, 0) == 6 });
        assert!(has_text(
            settings,
            "https://github.com/weimeng54/ALTRunNext/releases"
        ));
        native_snapshot::save(settings, &format!("language-{code}-{scale}-about"));
        command(settings, 2314);
        wait(|| unsafe { IsWindow(settings) == 0 });
    }
}
