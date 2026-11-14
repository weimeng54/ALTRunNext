use altrun_probe::localization::{Catalog, Language};
use std::{fs, path::PathBuf};

fn fixture(name: &str) -> PathBuf {
    let path = PathBuf::from("target").join(format!("languages-{name}-{}", std::process::id()));
    fs::create_dir_all(&path).unwrap();
    path
}

#[test]
fn system_and_manual_selection_with_regional_fallback() {
    let catalog = Catalog::load(&PathBuf::from("languages"));
    assert!(catalog.invalid.is_empty());
    for (language, native_name) in [
        ("ja", "日本語"),
        ("ko", "한국어"),
        ("fr", "Français"),
        ("de", "Deutsch"),
        ("es", "Español"),
    ] {
        assert!(
            catalog
                .options()
                .contains(&(language.into(), native_name.into()))
        );
    }
    assert_eq!(catalog.resolve("system", ["ja-JP", "en-US"]).locale(), "ja");
    assert_eq!(
        catalog.resolve("system", ["xx-XX", "zh-CN"]).locale(),
        "zh-Hans"
    );
    assert_eq!(catalog.resolve("system", ["zh-TW", "fr-CA"]).locale(), "fr");
    assert_eq!(catalog.resolve("system", ["en-GB", "ja-JP"]).locale(), "en");
    assert_eq!(catalog.resolve("system", ["xx-XX"]).locale(), "en");
    assert_eq!(catalog.resolve("en", ["zh-CN"]).locale(), "en");
    assert_eq!(catalog.resolve("pt", ["ja-JP"]).locale(), "en");
}

#[test]
fn external_packs_use_declared_codes_and_fall_back_per_message() {
    let root = fixture("external");
    let japanese = r#"{"locale":"ja","nativeName":"日本語","formatVersion":1,"strings":{"about.title":"バージョン情報"}}"#;
    fs::write(root.join("arbitrary-name.json"), japanese).unwrap();
    let catalog = Catalog::load(&root);
    assert!(catalog.invalid.is_empty());
    let language = catalog.resolve("system", ["ja-JP"]);
    assert_eq!(language.text("about.title"), "バージョン情報");
    assert_eq!(language.text("about.open_repository"), "Open repository");
    fs::write(
        root.join("ja-JP.json"),
        japanese.replace("\"ja\"", "\"ja-JP\""),
    )
    .unwrap();
    let catalog = Catalog::load(&root);
    assert_eq!(catalog.resolve("system", ["ja-JP"]).locale(), "ja-JP");
    assert_eq!(catalog.resolve("system", ["ja"]).locale(), "ja");
    assert_eq!(Language::English.text("about.title"), "About");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn malformed_pack_is_skipped_without_preventing_language_selection() {
    let root = fixture("invalid");
    for (name, strings) in [
        ("duplicate", r#""about.title":"Uno","about.title":"Dos""#),
        ("placeholder", r#""launcher.startup_error":"Erreur""#),
    ] {
        fs::write(
            root.join(format!("{name}.json")),
            format!(r#"{{"locale":"es","nativeName":"Español","formatVersion":1,"strings":{{{strings}}}}}"#),
        ).unwrap();
    }
    let catalog = Catalog::load(&root);
    assert_eq!(catalog.invalid, ["duplicate.json", "placeholder.json"]);
    assert_eq!(
        catalog.resolve("system", ["es-ES", "zh-CN"]).locale(),
        "zh-Hans"
    );
    assert_eq!(catalog.resolve("es", ["zh-CN"]).locale(), "en");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn an_old_external_pack_keeps_active_translations_and_ignores_retired_keys() {
    let root = fixture("old-pack");
    fs::write(
        root.join("es.json"),
        r#"{"locale":"es","nativeName":"Español","formatVersion":1,"strings":{"about.title":"Acerca de","ui.convert_to_relative_paths":"Antiguo"}}"#,
    )
    .unwrap();
    let catalog = Catalog::load(&root);
    assert!(catalog.invalid.is_empty());
    let language = catalog.resolve("es", []);
    assert_eq!(language.text("about.title"), "Acerca de");
    assert_eq!(language.text("about.open_repository"), "Open repository");
    fs::remove_dir_all(root).unwrap();
}
