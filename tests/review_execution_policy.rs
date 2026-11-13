#![cfg(windows)]
mod review_native_support;
use review_native_support::*;
use windows_sys::Win32::UI::{
    Input::KeyboardAndMouse::IsWindowEnabled,
    WindowsAndMessaging::{BM_CLICK, GetDlgItem, IsWindowVisible, SendMessageW},
};
#[test]
fn manager_test_respects_debug_and_explicit_safe_modes() {
    let root = std::env::current_dir()
        .unwrap()
        .join("target/review-execution-policy");
    std::fs::create_dir_all(&root).unwrap();
    for (name, mode, allowed) in [
        ("debug", None, false),
        ("safe", Some("--safe-mode"), false),
        ("enabled", Some("--enable-launch"), true),
    ] {
        let marker = root.join(format!("{name}.txt"));
        let data = root.join(format!("{name}.tsv"));
        let _ = std::fs::remove_file(&marker);
        let original = format!(
            "test\tSynthetic test\t\"{}\" /D /C exit 0\t0\tnone\n",
            shell().display()
        );
        std::fs::write(&data, &original).unwrap();
        let (process, launcher) = start(&data, mode);
        let dialog = manager(launcher, process.0.id());
        unsafe {
            wait(|| !GetDlgItem(dialog, 2137).is_null() && IsWindowVisible(dialog) != 0);
            let test = GetDlgItem(dialog, 2137);
            let command_field = GetDlgItem(dialog, 2104);
            assert_ne!(IsWindowVisible(test), 0);
            set(command_field, "");
            assert_eq!(IsWindowEnabled(test), 0);
            set(
                command_field,
                &format!(
                    "\"{}\" /D /C echo ok>\"{}\"",
                    shell().display(),
                    marker.display()
                ),
            );
            assert_ne!(IsWindowEnabled(test), 0);
            SendMessageW(test, BM_CLICK, 0, 0);
        }
        if allowed {
            wait(|| marker.exists());
            assert_eq!(std::fs::read_to_string(&marker).unwrap().trim(), "ok");
        } else {
            assert!(
                has_text(dialog, "预览"),
                "blocked test launch needs visible preview feedback"
            );
            std::thread::sleep(std::time::Duration::from_millis(350));
            assert!(
                !marker.exists(),
                "safe/default Debug manager test executed a shortcut"
            );
        }
        assert_eq!(std::fs::read_to_string(&data).unwrap(), original);
    }
}
