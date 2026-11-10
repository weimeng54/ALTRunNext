use altrun_probe::{library::{self, ParamType, Snapshot}, launch, parse_entries, search};

#[test]
fn legacy_import_preserves_types_frequency_duplicates_and_command_delimiters() {
    let imported = library::import_legacy(concat!(
        "备注：常用程序\n",
        "F12 | utf8_query | web | 中文搜索 | https://example.com/?q={%p}&flags=a|b,c\n",
        "F3 | no_encoding | cmd | 命令 | cmd.exe /k %p\n",
        "np,记事本,notepad.exe\n",
        "np,另一个,other.exe /a,b\n"
    )).unwrap();
    assert_eq!(imported.entries.len(), 4);
    assert_eq!(imported.ignored_lines, 1);
    assert_eq!(imported.entries[0].frequency, 12);
    assert_eq!(imported.entries[0].param_type, ParamType::Utf8Url);
    assert_eq!(imported.entries[0].command, "https://example.com/?q={%p}&flags=a|b,c");
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
    assert_eq!(r.file, "https://example.com/?q=%E4%B8%AD%E6%96%87+%31%32%2B%26");
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
    assert_eq!(search(&rows, "n?"), vec![0,1,2]);
    assert_eq!(search(&rows, "n*e"), vec![1,2]);
    assert!(search(&rows, " ").is_empty());
}

#[test]
fn bom_decoding_preserves_unicode() {
    assert_eq!(library::decode(b"\xef\xbb\xbfnote").unwrap(), "note");
    assert_eq!(library::decode(&[0xff,0xfe,0x2d,0x4e]).unwrap(), "中");
}

#[test]
fn save_creates_backup_and_rejects_external_changes() {
    let dir = std::env::current_dir().unwrap().join("target/library-save-test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("shortcuts.tsv");
    let initial = "np\t记事本\tnotepad.exe\t7\n";
    std::fs::write(&path, initial).unwrap();
    let mut snapshot = Snapshot::open(&path).unwrap();
    let mut edited = snapshot.entries.clone(); edited[0].name = "新名称".into();
    snapshot.save(&edited).unwrap();
    assert_eq!(std::fs::read(path.with_extension("tsv.bak")).unwrap(), initial.as_bytes());
    assert_eq!(Snapshot::open(&path).unwrap().entries[0].name, "新名称");
    std::fs::write(&path, "external edit").unwrap();
    assert!(snapshot.save(&edited).is_err());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "external edit");
}
