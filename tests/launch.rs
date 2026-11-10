use altrun_probe::launch::{LaunchRequest, resolve};

fn request(file: &str, parameters: &str) -> LaunchRequest {
    LaunchRequest { file: file.into(), parameters: parameters.into() }
}

#[test]
fn quoted_executable_preserves_the_windows_argument_string() {
    assert_eq!(resolve(r#""D:\Program Files\App\app.exe" --open "中文 文档.txt""#, None).unwrap(), request(r"D:\Program Files\App\app.exe", r#"--open "中文 文档.txt""#));
}

#[test]
fn shell_metacharacters_remain_arguments_not_an_implicit_shell() {
    assert_eq!(resolve("notepad.exe a.txt & other.exe", None).unwrap(), request("notepad.exe", "a.txt & other.exe"));
    assert_eq!(resolve("cmd.exe /k echo hello", None).unwrap(), request("cmd.exe", "/k echo hello"));
}

#[test]
fn urls_are_opened_as_one_target() {
    assert_eq!(resolve("https://example.com/?q=hello&x=1", None).unwrap(), request("https://example.com/?q=hello&x=1", ""));
}

#[test]
fn parameters_cannot_change_the_selected_executable() {
    assert_eq!(resolve("notepad.exe %p", Some(r#""a.txt" & calc.exe"#)).unwrap(), request("notepad.exe", r#""a.txt" & calc.exe"#));
    assert!(resolve("%p", Some("calc.exe")).is_err());
    assert!(resolve("notepad.exe %p", None).is_err());
}

#[test]
fn unsupported_legacy_templates_are_rejected_instead_of_changed() {
    for command in ["notepad.exe %c", "notepad.exe %x", "notepad.exe %p%"] {
        assert!(resolve(command, Some("text")).is_err(), "{command}");
    }
}

#[test]
fn malformed_commands_cannot_be_truncated_by_native_apis() {
    for command in ["", "  ", "\"unfinished.exe", "notepad.exe\0calc.exe", "notepad.exe\ncalc.exe"] {
        assert!(resolve(command, None).is_err());
    }
    assert!(resolve("notepad.exe %p", Some("text\0more")).is_err());
}
