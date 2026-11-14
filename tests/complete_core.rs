// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use altrun_probe::{
    preferences::{Preferences, parse_hotkey},
    usage::Usage,
};
#[test]
fn old_configuration_roundtrips_functional_options_and_unknown_values() {
    let p=Preferences::parse("[Config]\nHotKey1=Alt + R\nHotKey2=Pause\nRegex=1\nNumberKey=1\nIndexFrom0to9=0\nRememberFavouratMatch=1\nHideDelay=15\nParamHistoryLimit=50\n[GUI]\nFormWidth=420\nAlpha=240\nRoundBorderRadius=12\nTitleFont=宋体|[]|12|65535|0\n[Extra]\nFutureValue=keep\n").unwrap();
    assert_eq!(p.get("HotKey1"), "Alt + R");
    assert_eq!(p.get("Regex"), "");
    assert_eq!(p.get("Alpha"), "");
    assert_eq!(p.get("PositionMode"), "center");
    assert!(p.flag("RememberFavouratMatch"));
    assert_eq!(p.get("FormWidth"), "420");
    assert_eq!(Preferences::parse(&p.text()).unwrap(), p);
    let mut p = p;
    p.set("FormWidth", "10").unwrap_err();
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
fn stable_ids_and_working_directories_survive_editing_and_roundtrip() {
    let mut rows =
        altrun_probe::parse_entries("np\t记事本\tnotepad.exe\t7\nnp\t另一个\tother.exe\t0\n")
            .unwrap();
    assert!(!rows[0].id.is_empty());
    assert_ne!(rows[0].id, rows[1].id);
    let id = rows[0].id.clone();
    rows[0].alias = "renamed".into();
    rows[0].working_directory = r"D:\Work Folder".into();
    let text = altrun_probe::library::serialize(&rows).unwrap();
    let restored = altrun_probe::parse_entries(&text).unwrap();
    assert_eq!(restored[0].id, id);
    assert_eq!(restored[0].working_directory, r"D:\Work Folder");
    assert_eq!(altrun_probe::search(&restored, ""), vec![0, 1]);
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
        base_is_working_directory: true,
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
fn search_treats_punctuation_as_literal_text() {
    let rows = altrun_probe::parse_entries(concat!(
        "note?*\tN\tn.exe\t0\n",
        "notepad\tN\tn.exe\t0\n",
        "a.b\tN\tn.exe\t0\n",
        "axb\tN\tn.exe\t0\n",
    ))
    .unwrap();
    let index = altrun_probe::SearchIndex::new(&rows);
    assert_eq!(index.query("note?*"), vec![0]);
    assert_eq!(index.query("a.b"), vec![2]);
    assert!(index.query("^note").is_empty());
}

#[test]
fn literal_search_ranks_exact_prefix_contiguous_then_fuzzy() {
    let rows = altrun_probe::parse_entries(concat!(
        "calc\t计算器\tcalc.exe\t5000\n",
        "mycl\tMiddle\tn.exe\t4000\n",
        "cl-tools\tPrefix\tn.exe\t1000\n",
        "cl\tExact\tn.exe\t0\n",
        "lc\tWrong order\tn.exe\t9999\n",
    ))
    .unwrap();
    let index = altrun_probe::SearchIndex::new(&rows);
    assert_eq!(index.query_options("CL", true), vec![3, 2, 1, 0]);
    assert_eq!(index.query_options("cl", false), vec![3, 2]);
    assert!(!index.same_match_priority("cl", true, 3, 2));
    assert!(index.query("计算器").is_empty());
    assert!(altrun_probe::preferences::Preferences::default().flag("EnglishSearch"));
    assert!(
        !altrun_probe::preferences::Preferences::parse("[Config]\nEnglishSearch=0")
            .unwrap()
            .flag("EnglishSearch")
    );
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
