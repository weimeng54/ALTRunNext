#[path = "src/language_pack.rs"]
mod language_pack;

use std::{fs, path::PathBuf, process::Command};

fn build_messages() {
    let read_pack = |path: &str| {
        let input = fs::read_to_string(path).unwrap_or_else(|error| panic!("{path}: {error}"));
        language_pack::LanguagePack::parse(&input).unwrap_or_else(|error| panic!("{path}: {error}"))
    };
    let english = read_pack("assets/i18n/en.json");
    let chinese = read_pack("assets/i18n/zh-Hans.json");
    assert_eq!(english.locale, "en");
    assert_eq!(chinese.locale, "zh-Hans");
    let base = || {
        english
            .strings
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
    };
    chinese.check(base(), true).expect("Chinese resources");
    println!("cargo:rerun-if-changed=languages");
    println!("cargo:rerun-if-changed=src/language_pack.rs");
    for locale in ["ja", "ko", "fr", "de", "es"] {
        let path = format!("languages/{locale}.json");
        let pack = read_pack(&path);
        assert_eq!(locale, pack.locale);
        pack.check(base(), true)
            .unwrap_or_else(|error| panic!("{path}: {error}"));
    }
    let mut table = String::from("static MESSAGES: &[(&str, &str, &str)] = &[\n");
    for (key, value) in &english.strings {
        table.push_str(&format!(
            "    ({key:?}, {value:?}, {:?}),\n",
            chinese.strings[key]
        ));
    }
    table.push_str("];\n");
    fs::write(
        PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("messages.rs"),
        table,
    )
    .expect("write built-in messages");
}

fn main() {
    println!("cargo:rerun-if-changed=assets");
    build_messages();
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let kits = PathBuf::from(
        std::env::var_os("ProgramFiles(x86)").expect("Windows SDK location unavailable"),
    )
    .join("Windows Kits/10/bin");
    let mut compilers = std::fs::read_dir(kits)
        .expect("Windows SDK required on the cloud builder")
        .filter_map(Result::ok)
        .map(|entry| entry.path().join("x64/rc.exe"))
        .filter(|path| path.exists())
        .collect::<Vec<_>>();
    compilers.sort();
    let compiler = compilers
        .last()
        .expect("Windows SDK resource compiler unavailable");
    let output = PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("altrun.res");
    let assets = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("assets");
    let status = Command::new(compiler)
        .current_dir(assets)
        .arg("/nologo")
        .arg("/fo")
        .arg(&output)
        .arg("app.rc")
        .status()
        .expect("Could not run cloud resource compiler");
    assert!(status.success(), "Windows resources failed to compile");
    println!("cargo:rustc-link-arg-bin=ALTRunNext={}", output.display());
}
