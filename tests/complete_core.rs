use altrun_probe::{
    matching,
    preferences::{Preferences, parse_hotkey},
    usage::Usage,
};
#[test]
fn old_configuration_roundtrips_functional_options_and_unknown_values() {
    let p=Preferences::parse("[Config]\nHotKey1=Alt + R\nHotKey2=Pause\nRegex=1\nNumberKey=1\nIndexFrom0to9=0\nRememberFavouratMatch=1\nHideDelay=15\nParamHistoryLimit=50\n[GUI]\nFormWidth=420\nAlpha=240\nRoundBorderRadius=12\nTitleFont=宋体|[]|12|65535|0\n[Extra]\nFutureValue=keep\n").unwrap();
    assert_eq!(p.get("HotKey1"), "Alt + R");
    assert!(p.flag("Regex"));
    assert!(p.flag("RememberFavouratMatch"));
    assert_eq!(p.get("FormWidth"), "420");
    assert_eq!(Preferences::parse(&p.text()).unwrap(), p);
    let mut p = p;
    p.set("FormWidth", "10").unwrap_err();
    p.set("Alpha", "999").unwrap_err();
    p.set("HotKey1", "Ctrl+Bogus").unwrap_err();
}
#[test]
fn key_bindings_support_original_and_windows_modifiers() {
    let h = parse_hotkey(" Ctrl + Alt + R ").unwrap().unwrap();
    assert_eq!(h.key, 82);
    assert_eq!(h.modifiers, 3);
    assert_eq!(parse_hotkey("Pause").unwrap().unwrap().key, 0x13);
    assert_eq!(parse_hotkey("Win+Space").unwrap().unwrap().modifiers, 8);
    assert!(parse_hotkey("").unwrap().is_none());
    assert!(parse_hotkey("Ctrl+Alt").is_err());
}
#[test]
fn usage_and_parameter_history_persist_unicode_delimiters_and_limits() {
    let mut u = Usage::default();
    u.record("one", "np", Some("中文\t\n%value"), 2);
    u.record("one", "np", Some("second"), 2);
    u.record("two", "x", None, 2);
    u.record("one", "np", Some("third"), 2);
    assert_eq!(u.counts["one"], 3);
    assert_eq!(u.recent, vec!["one", "two"]);
    assert_eq!(u.favourites["np"], "one");
    assert_eq!(u.parameters["one"], vec!["third", "second"]);
    assert_eq!(Usage::parse(&u.text()).unwrap(), u);
}
#[test]
fn actual_regular_expressions_support_classes_anchors_groups_and_prefix_mode() {
    let aliases = vec![
        "notepad".into(),
        "note12".into(),
        "xnote".into(),
        "other".into(),
    ];
    assert_eq!(
        matching::positions(r"^note(pad|[0-9]+)$", &aliases, false).unwrap(),
        vec![Some(0), Some(0), None, None]
    );
    assert_eq!(
        matching::positions("note", &aliases, true).unwrap(),
        vec![Some(0), Some(0), None, None]
    );
    assert_eq!(matching::prepare_legacy("n*e?"), "n.*e.");
    assert!(matching::positions("[", &aliases, false).is_err());
}
#[test]
fn advanced_expression_features_are_available_for_isolated_worker() {
    let aliases = vec!["abab".into(), "abac".into(), "foobar".into()];
    assert_eq!(
        matching::positions(r"^(ab)\1$", &aliases, false).unwrap(),
        vec![Some(0), None, None]
    );
    assert_eq!(
        matching::positions(r"(?<=foo)bar", &aliases, false).unwrap(),
        vec![None, None, Some(3)]
    );
}

#[test]
fn stable_ids_and_working_directories_survive_editing_and_roundtrip() {
    let mut rows =
        altrun_probe::parse_entries("np\t记事本\tnotepad.exe\t7\nnp\t另一个\tother.exe\t0\n")
            .unwrap();
    assert!(!rows[0].id.is_empty());
    assert_ne!(rows[0].id, rows[1].id);
    let id = rows[0].id.clone();
    rows[0].alias = "renamed".into();
    rows[0].working_directory = r"D:\Work Folder".into();
    rows[1].enabled = false;
    let text = altrun_probe::library::serialize(&rows).unwrap();
    let restored = altrun_probe::parse_entries(&text).unwrap();
    assert_eq!(restored[0].id, id);
    assert_eq!(restored[0].working_directory, r"D:\Work Folder");
    assert!(!restored[1].enabled);
    assert_eq!(altrun_probe::search(&restored, ""), vec![0]);
}

#[test]
fn templates_preserve_context_literals_and_window_display_flags() {
    use altrun_probe::{
        launch::{self, LaunchContext, ShowMode},
        library::ParamType,
    };
    let context = LaunchContext {
        clipboard: Some("literal %TEMP% & 中文".into()),
        window_id: 123,
        window_title: "编辑器".into(),
        window_class: "EditorClass".into(),
        base_directory: r"D:\Work".into(),
        allow_dynamic_target: false,
    };
    let (request, show, directory) = launch::resolve_context(
        "@+ tool.exe --paste {%c} --window {%wd} --title {%wt} --class {%wc}",
        None,
        ParamType::Raw,
        "",
        &context,
    )
    .unwrap();
    assert_eq!(request.file, "tool.exe");
    assert!(request.parameters.contains("literal %TEMP% & 中文"));
    assert!(request.parameters.contains("--window 123"));
    assert!(request.parameters.contains("编辑器"));
    assert_eq!(show, ShowMode::Maximized);
    assert_eq!(directory, Some(r"D:\Work".into()));
    let (request, show, _) = launch::resolve_context(
        "@- https://example.com/?q={%c}",
        None,
        ParamType::Utf8Url,
        "",
        &context,
    )
    .unwrap();
    assert!(
        request
            .file
            .contains("literal+%25TEMP%25+%26+%E4%B8%AD%E6%96%87")
    );
    assert_eq!(show, ShowMode::Minimized);
}

#[test]
fn advanced_matching_worker_returns_results_and_is_time_bounded() {
    let aliases = vec!["abab".into(), "abac".into()];
    let program = std::path::Path::new(env!("CARGO_BIN_EXE_altrun-probe"));
    assert_eq!(
        altrun_probe::isolated_matching::run(
            program,
            r"^(ab)\1$",
            &aliases,
            false,
            std::time::Duration::from_secs(3)
        )
        .unwrap(),
        vec![Some(0), None]
    );
    assert!(
        altrun_probe::isolated_matching::run(
            program,
            r"^(ab)\1$",
            &aliases,
            false,
            std::time::Duration::ZERO
        )
        .is_err()
    );
}

#[test]
fn background_state_writer_flushes_and_refuses_external_overwrite() {
    use altrun_probe::state_store::{TextFile, Writer};
    let dir = std::env::current_dir()
        .unwrap()
        .join("target/complete-state");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("state.tsv");
    let _ = std::fs::remove_file(&path);
    let (file, _) = TextFile::open(&path).unwrap();
    {
        let writer = Writer::start(file);
        for n in 0..100 {
            writer.publish(format!("F\titem\t{n}\n"));
        }
    }
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "F\titem\t99\n");
    let (mut file, _) = TextFile::open(&path).unwrap();
    std::fs::write(&path, "external").unwrap();
    assert!(file.save("changed").is_err());
    assert_eq!(std::fs::read_to_string(path).unwrap(), "external");
}
#[test]
fn search_preferences_keep_literal_and_regex_semantics_separate() {
    let rows = altrun_probe::parse_entries("np\tN\tn.exe\t0\nxnp\tN\tn.exe\t0\n").unwrap();
    let index = altrun_probe::SearchIndex::new(&rows);
    assert_eq!(
        index.query_options("^n", true, true, true).unwrap(),
        vec![0]
    );
    assert!(
        index
            .query_options("^n", false, true, true)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        index.query_options("np", false, false, false).unwrap(),
        vec![0]
    );
    assert!(index.query_options(r"(np)\1", true, true, true).is_err());
}

#[test]
fn context_markers_do_not_capture_environment_prefixes_and_dynamic_target_is_explicit() {
    use altrun_probe::{
        launch::{self, LaunchContext},
        library::ParamType,
    };
    let mut context = LaunchContext::default();
    let command = if cfg!(windows) {
        "%comspec% /D /C echo ok"
    } else {
        "%PATH%"
    };
    assert!(!launch::has_context(command));
    if cfg!(windows) {
        assert!(launch::resolve_context(command, None, ParamType::Auto, "", &context).is_ok());
    }
    assert!(
        launch::resolve_context("{%p}", Some("tool.exe"), ParamType::Raw, "", &context).is_err()
    );
    context.allow_dynamic_target = true;
    assert_eq!(
        launch::resolve_context("{%p}", Some("tool.exe"), ParamType::Raw, "", &context)
            .unwrap()
            .0
            .file,
        "tool.exe"
    );
}
#[test]
fn list_format_is_bounded_and_does_not_treat_user_content_as_format() {
    assert_eq!(
        altrun_probe::list_format::render("%-4s | %s", "中", "%TEMP%").unwrap(),
        "中    | %TEMP%"
    );
    assert!(altrun_probe::list_format::render("%999999s", "a", "b").is_err());
    assert!(altrun_probe::list_format::render("%n", "a", "b").is_err());
}
