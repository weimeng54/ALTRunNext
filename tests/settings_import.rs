// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use altrun_probe::preferences::Preferences;

#[test]
fn imports_current_exports_and_previous_app_settings() {
    let old_export = "[Config]\nEnglishSearch=1\nParamHistoryLimit=0\nTraceEnable=1\nAllowDynamicTarget=1\n[GUI]\nTheme=dark\nPositionMode=remember\n";
    let settings = Preferences::import(old_export).unwrap();
    assert_eq!(settings.get("Theme"), "dark");
    assert_eq!(settings.get("PositionMode"), "remember");
    assert_eq!(settings.number("ParamHistoryLimit"), 0);
    assert!(settings.flag("TraceEnable"));
    assert!(settings.flag("AllowDynamicTarget"));
    let output = settings.text();
    assert!(output.starts_with("; ALTRunNext settings format 1\n"));
    assert_eq!(Preferences::import(&output).unwrap(), settings);
    assert_eq!(
        Preferences::import("; ALTRun settings format 1\n[GUI]\nTheme=dark\n")
            .unwrap()
            .get("Theme"),
        "dark"
    );
}

#[test]
fn rejects_original_settings_and_normalizes_removed_options() {
    let original = "[Config]\nRegex=1\nHotKey1=Alt + R\n[Window]\nWinTop=0\nWinLeft=0\n[GUI]\nFormWidth=420\nLang=English.lng\n";
    assert!(Preferences::import(original).is_err());
    let prior = "[Config]\nEnglishSearch=1\nRegex=1\nStandardRegex=1\nShowTopTen=1\nShowStartNotification=1\n[GUI]\nPositionMode=mouse\nAlpha=200\nRoundBorderRadius=4\nListFormat=%s\n[Window]\nWinLeft=220\nManWinTop=160\n";
    let normalized = Preferences::import(prior).unwrap();
    assert_eq!(normalized.get("PositionMode"), "center");
    for key in [
        "Regex",
        "StandardRegex",
        "ShowTopTen",
        "ShowStartNotification",
        "Alpha",
        "RoundBorderRadius",
        "ListFormat",
        "WinLeft",
        "ManWinTop",
    ] {
        assert!(!normalized.text().contains(&format!("{key}=")), "{key}");
    }
    assert_eq!(Preferences::default().get("PositionMode"), "center");
    assert!(Preferences::import("; ALTRun settings format 2\n[GUI]\nTheme=dark\n").is_err());
}
#[test]
fn language_defaults_to_system_and_preserves_explicit_choices() {
    use altrun_probe::preferences::Preferences;

    assert_eq!(Preferences::default().get("Lang"), "system");
    for (previous, expected) in [
        ("EN", "en"),
        ("简体中文", "zh-Hans"),
        ("ja-jp", "ja-JP"),
        ("fr", "fr"),
    ] {
        let config = Preferences::parse(&format!("[GUI]\nLang={previous}\n")).unwrap();
        assert_eq!(config.get("Lang"), expected);
        assert_eq!(
            Preferences::parse(&config.text()).unwrap().get("Lang"),
            expected
        );
    }
}
