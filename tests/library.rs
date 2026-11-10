use altrun_probe::{
    launch,
    library::{self, ParamType, Snapshot},
    parse_entries, search,
};

#[test]
fn legacy_import_preserves_types_frequency_duplicates_and_command_delimiters() {
    let imported = library::import_legacy(concat!(
        "备注：常用程序\n",
        "F12 | utf8_query | web | 中文搜索 | https://example.com/?q={%p}&flags=a|b,c\n",
        "F3 | no_encoding | cmd | 命令 | cmd.exe /k %p\n",
        "np,记事本,notepad.exe\n",
        "np,另一个,other.exe /a,b\n"
    ))
    .unwrap();
    assert_eq!(imported.entries.len(), 4);
    assert_eq!(imported.ignored_lines, 1);
    assert_eq!(imported.entries[0].frequency, 12);
    assert_eq!(imported.entries[0].param_type, ParamType::Utf8Url);
    assert_eq!(
        imported.entries[0].command,
        "https://example.com/?q={%p}&flags=a|b,c"
    );
    assert_eq!(imported.entries[2].param_type, ParamType::None);
    assert_eq!(imported.entries[3].command, "other.exe /a,b");
    let saved = library::serialize(&imported.entries).unwrap();
    let restored = parse_entries(&saved).unwrap();
    assert_eq!(restored[0].param_type, ParamType::Utf8Url);
    assert_eq!(restored[3].alias, "np");
}

#[test]
fn malformed_data_does_not_silently_drop_shortcuts() {
    assert!(library::import_legacy("F1|utf8_query|only_alias").is_err());
    assert!(parse_entries("a\tname\tapp.exe\t2\tunknown_type\n").is_err());
    let old = parse_entries("np\t记事本\tnotepad.exe\t7\n").unwrap();
    assert_eq!(old[0].param_type, ParamType::Auto);
}

#[test]
fn typed_parameters_match_original_url_encoding_and_brace_marker() {
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
fn common_wildcards_match_aliases_without_changing_plain_search() {
    let rows = parse_entries("np\tN\tn.exe\t0\nnote\tN\tn.exe\t0\nxnote\tN\tn.exe\t0\n").unwrap();
    assert_eq!(search(&rows, "n?"), vec![0, 1, 2]);
    assert_eq!(search(&rows, "n*e"), vec![1, 2]);
    assert!(search(&rows, " ").is_empty());
}

#[test]
fn bom_decoding_preserves_unicode() {
    assert_eq!(library::decode(b"\xef\xbb\xbfnote").unwrap(), "note");
    assert_eq!(library::decode(&[0xff, 0xfe, 0x2d, 0x4e]).unwrap(), "中");
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
fn old_chinese_file_decodes_without_replacement_characters() {
    assert_eq!(library::decode(&[0xd6, 0xd0, 0xce, 0xc4]).unwrap(), "中文");
    assert!(library::decode(&[0xff]).is_err());
    let encoded = launch::resolve_typed("https://example.com/?q=%p", Some("A1 +@"), ParamType::LegacyUrl).unwrap();
    assert_eq!(encoded.file,"https://example.com/?q=A%31+%2B@");
}

#[test]
fn headless_conversion_never_overwrites_the_import_source() {
    let dir=std::env::current_dir().unwrap().join("target/library-conversion-test");
    std::fs::create_dir_all(&dir).unwrap();
    let source=dir.join("legacy.txt");let output=dir.join("converted.tsv");
    let original="F8|utf8_query|web|网页搜索|https://example.com/?q={%p}\n";
    std::fs::write(&source,original).unwrap();let _=std::fs::remove_file(&output);
    let program=env!("CARGO_BIN_EXE_altrun-probe");
    assert!(std::process::Command::new(program).arg("--convert-legacy").arg(&source).arg("--output").arg(&output).status().unwrap().success());
    assert_eq!(Snapshot::open(&output).unwrap().entries[0].param_type,ParamType::Utf8Url);
    assert!(!std::process::Command::new(program).arg("--convert-legacy").arg(&source).arg("--output").arg(&source).status().unwrap().success());
    assert_eq!(std::fs::read_to_string(source).unwrap(),original);
}

#[test]
fn comma_headers_starting_with_f_keep_pipes_in_command_tails() {
    let imported=library::import_legacy("Firefox,浏览器,app.exe --filter a|b\nF7,no_encoding,Find,查找,app.exe %p | other\n").unwrap();
    assert_eq!(imported.entries[0].alias,"Firefox");
    assert_eq!(imported.entries[0].command,"app.exe --filter a|b");
    assert_eq!(imported.entries[1].frequency,7);
    assert_eq!(imported.entries[1].command,"app.exe %p | other");
}

#[test]
#[cfg(windows)]
fn p_underscore_environment_variables_are_not_parameter_markers() {
    // Windows environment operations are synchronized by the operating system.
    unsafe { std::env::set_var("p_altrun_compat_test","D:\\Tools"); }
    let resolved=launch::resolve(r#""%p_altrun_compat_test%\app.exe" %p_altrun_compat_test%"#,None).unwrap();
    assert_eq!(resolved.file,r"D:\Tools\app.exe");
    assert_eq!(resolved.parameters,r"D:\Tools");
}

#[test]
fn delayed_reload_cannot_replace_a_newer_saved_snapshot() {
    let(writer,mut mailbox)=altrun_probe::library_updates::channel();
    let old=parse_entries("old\t旧\told.exe\t0\n").unwrap();
    let new=parse_entries("new\t新\tnew.exe\t0\n").unwrap();
    let delayed=writer.begin_read();writer.saved(new.clone());writer.read_complete(delayed,old.clone());
    assert_eq!(mailbox.take().unwrap()[0].alias,"new");
    let delayed=writer.begin_read();writer.saved(new);assert_eq!(mailbox.take().unwrap()[0].alias,"new");
    writer.read_complete(delayed,old);assert!(mailbox.take().is_none());
}

#[test]
#[cfg(windows)]
fn failed_replacement_keeps_the_original_file_and_snapshot() {
    let dir=std::env::current_dir().unwrap().join("target/library-readonly-test");
    std::fs::create_dir_all(&dir).unwrap();let path=dir.join("shortcuts.tsv");
    let original="np\t记事本\tnotepad.exe\t7\n";
    std::fs::write(&path,original).unwrap();let mut snapshot=Snapshot::open(&path).unwrap();
    let mut edited=snapshot.entries.clone();edited[0].name="未保存的修改".into();
    let permissions=std::fs::metadata(&path).unwrap().permissions();let mut readonly=permissions.clone();readonly.set_readonly(true);std::fs::set_permissions(&path,readonly).unwrap();
    let result=snapshot.save(&edited);
    std::fs::set_permissions(&path,permissions).unwrap();
    assert!(result.is_err());
    assert_eq!(std::fs::read_to_string(&path).unwrap(),original);
    assert_eq!(snapshot.entries[0].name,"记事本");
    assert!(!std::fs::read_dir(dir).unwrap().any(|entry|entry.unwrap().file_name().to_string_lossy().contains("altrun-tmp")));
}
