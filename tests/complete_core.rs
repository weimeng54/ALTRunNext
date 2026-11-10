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
