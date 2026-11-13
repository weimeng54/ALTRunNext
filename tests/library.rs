use altrun_probe::{
    launch,
    library::{self, ParamType, Snapshot},
    parse_entries, search,
};

#[test]
fn malformed_data_does_not_silently_drop_shortcuts() {
    assert!(parse_entries("F1|utf8_query|only_alias").is_err());
    assert!(parse_entries("a\tname\tapp.exe\t2\tunknown_type\n").is_err());
    let old = parse_entries("np\t记事本\tnotepad.exe\t7\n").unwrap();
    assert_eq!(old[0].param_type, ParamType::Auto);
}

#[test]
fn typed_parameters_support_url_encoding_and_brace_marker() {
    let url = "https://example.com/?q={%p}";
    let r = launch::resolve_typed(url, Some("中文 12+&"), ParamType::Utf8Url).unwrap();
    assert_eq!(
        r.file,
        "https://example.com/?q=%E4%B8%AD%E6%96%87+%31%32%2B%26"
    );
    let raw = launch::resolve_typed("app.exe --open {%p}", Some("a b"), ParamType::Raw).unwrap();
    assert_eq!(raw.parameters, "--open a b");
    let appended = launch::resolve_typed("app.exe --open", Some("a.txt"), ParamType::Raw).unwrap();
    assert_eq!(appended.parameters, "--open a.txt");
    assert!(!launch::has_parameter("%path%\\app.exe"));
    assert!(!launch::has_parameter("%programfiles%\\app.exe"));
    assert!(launch::has_parameter("app.exe {%p}"));
    assert!(!ParamType::None.needs_input("app.exe %p"));
    assert!(ParamType::Raw.needs_input("app.exe"));
}

#[test]
fn search_treats_metacharacters_as_literal_text() {
    let rows = parse_entries("np\tN\tn.exe\t0\nnote\tN\tn.exe\t0\nn?\tN\tn.exe\t0\n").unwrap();
    assert_eq!(search(&rows, "n?"), vec![2]);
    assert!(search(&rows, "n*e").is_empty());
    assert!(search(&rows, " ").is_empty());
}

#[test]
fn bom_decoding_preserves_unicode() {
    assert_eq!(library::decode(b"\xef\xbb\xbfnote").unwrap(), "note");
    assert_eq!(library::decode(&[0xff, 0xfe, 0x2d, 0x4e]).unwrap(), "中");
}

#[test]
fn shortcut_import_detects_formats_and_keeps_command_tails() {
    let text = "原来的备注\r\n\r\nF12 | UTF8_Query | b | 百度 | https://example.test/?q={%p}&pair=a|b,c\r\nnp,记事本,notepad.exe\r\n";
    let rows = library::decode_shortcut_import(text.as_bytes()).unwrap();
    assert_eq!(rows.ignored_lines, 2);
    assert_eq!(rows.entries.len(), 2);
    assert_eq!(rows.entries[0].frequency, 12);
    assert_eq!(rows.entries[0].param_type, ParamType::Utf8Url);
    assert_eq!(
        rows.entries[0].command,
        "https://example.test/?q={%p}&pair=a|b,c"
    );
    let serialized = library::serialize(&rows.entries).unwrap();
    assert_eq!(
        library::decode_shortcut_import(serialized.as_bytes())
            .unwrap()
            .entries,
        rows.entries
    );
    let wide: Vec<_> = [0xff, 0xfe]
        .into_iter()
        .chain(text.encode_utf16().flat_map(u16::to_le_bytes))
        .collect();
    assert_eq!(
        library::decode_shortcut_import(&wide).unwrap().entries,
        rows.entries
    );
    assert!(library::decode_shortcut_import(b"bad\tlabel\tnp.exe\tinvalid|a|b|c\n").is_err());
    assert!(library::decode_shortcut_import(b"F12|no_encoding|key|name\n").is_err());
    assert!(library::decode_shortcut_import(b"Only notes\n").is_err());
    assert!(library::decode_shortcut_import(b"[Config]\nTitleFont=A|B|C\n").is_err());
}

#[test]
#[cfg(windows)]
fn shortcut_import_accepts_original_chinese_encoding_but_not_ansi_tsv() {
    let rows = library::decode_shortcut_import(b"F7||cn|\xd6\xd0\xce\xc4|notepad.exe\r\n").unwrap();
    assert_eq!(rows.entries[0].name, "中文");
    assert!(library::decode_shortcut_import(b"cn\t\xd6\xd0\xce\xc4\tnotepad.exe\t7\n").is_err());
}

#[test]
fn migrated_ids_match_across_shortcuts_and_saved_history() {
    let old = "legacy-0123456789abcdef";
    let rows = parse_entries(&format!("a\tA\ta.exe\t0\tnone\t{old}\t\tcommand\t0\n")).unwrap();
    let state = altrun_probe::usage::Usage::parse(&format!(
        "F\t{old}\t5\nR\t{old}\nP\talias\t{old}\nH\t{old}\tvalue\nS\tmanager\t{old}\n",
    ))
    .unwrap();
    let id = &rows[0].id;
    assert!(!id.starts_with("legacy-"));
    assert_eq!(state.counts[id], 5);
    assert_eq!(state.recent.as_slice(), std::slice::from_ref(id));
    assert_eq!(&state.favourites["alias"], id);
    assert_eq!(state.parameters[id], ["value"]);
    assert_eq!(&state.selection["manager"], id);
    assert_eq!(search(&rows, ""), vec![0]);
}

#[test]
fn save_creates_backup_and_rejects_external_changes() {
    let dir = std::env::current_dir()
        .unwrap()
        .join("target/library-save-test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("shortcuts.tsv");
    let initial = "np\t记事本\tnotepad.exe\t7\n";
    std::fs::write(&path, initial).unwrap();
    let mut snapshot = Snapshot::open(&path).unwrap();
    let mut edited = snapshot.entries.clone();
    edited[0].name = "新名称".into();
    snapshot.save(&edited).unwrap();
    assert_eq!(
        std::fs::read(path.with_extension("tsv.bak")).unwrap(),
        initial.as_bytes()
    );
    assert_eq!(Snapshot::open(&path).unwrap().entries[0].name, "新名称");
    std::fs::write(&path, "external edit").unwrap();
    assert!(snapshot.save(&edited).is_err());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "external edit");
}

#[test]
fn ansi_url_parameters_use_the_system_encoding() {
    assert!(library::decode(&[0xd6, 0xd0, 0xce, 0xc4]).is_err());
    assert!(library::decode(&[0xff]).is_err());
    let encoded = launch::resolve_typed(
        "https://example.com/?q=%p",
        Some("A1 +@"),
        ParamType::AnsiUrl,
    )
    .unwrap();
    assert_eq!(encoded.file, "https://example.com/?q=A%31+%2B@");
}

#[test]
#[cfg(windows)]
fn p_underscore_environment_variables_are_not_parameter_markers() {
    // Windows environment operations are synchronized by the operating system.
    unsafe {
        std::env::set_var("p_altrun_compat_test", "D:\\Tools");
    }
    let resolved = launch::resolve(
        r#""%p_altrun_compat_test%\app.exe" %p_altrun_compat_test%"#,
        None,
    )
    .unwrap();
    assert_eq!(resolved.file, r"D:\Tools\app.exe");
    assert_eq!(resolved.parameters, r"D:\Tools");
}

#[test]
fn delayed_reload_cannot_replace_a_newer_saved_snapshot() {
    let (writer, mut mailbox) = altrun_probe::library_updates::channel();
    let old = parse_entries("old\t旧\told.exe\t0\n").unwrap();
    let new = parse_entries("new\t新\tnew.exe\t0\n").unwrap();
    let delayed = writer.begin_read();
    writer.saved(new.clone());
    writer.read_complete(delayed, old.clone());
    assert_eq!(mailbox.take().unwrap()[0].alias, "new");
    let delayed = writer.begin_read();
    writer.saved(new);
    assert_eq!(mailbox.take().unwrap()[0].alias, "new");
    writer.read_complete(delayed, old);
    assert!(mailbox.take().is_none());
}

#[test]
#[cfg(windows)]
fn failed_replacement_keeps_the_original_file_and_snapshot() {
    let dir = std::env::current_dir()
        .unwrap()
        .join("target/library-readonly-test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("shortcuts.tsv");
    let original = "np\t记事本\tnotepad.exe\t7\n";
    std::fs::write(&path, original).unwrap();
    let mut snapshot = Snapshot::open(&path).unwrap();
    let mut edited = snapshot.entries.clone();
    edited[0].name = "未保存的修改".into();
    let permissions = std::fs::metadata(&path).unwrap().permissions();
    let mut readonly = permissions.clone();
    readonly.set_readonly(true);
    std::fs::set_permissions(&path, readonly).unwrap();
    let result = snapshot.save(&edited);
    std::fs::set_permissions(&path, permissions).unwrap();
    assert!(result.is_err());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
    assert_eq!(snapshot.entries[0].name, "记事本");
    assert!(!std::fs::read_dir(dir).unwrap().any(|entry| {
        entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .contains("altrun-tmp")
    }));
}
