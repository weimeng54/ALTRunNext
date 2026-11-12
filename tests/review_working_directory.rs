use altrun_probe::{
    management::{self, Validation},
    parse_entries,
};
fn fixture() -> (std::path::PathBuf, std::path::PathBuf, std::path::PathBuf) {
    let root = std::env::current_dir()
        .unwrap()
        .join("target/review-working-directory");
    let data = root.join("data");
    let default = root.join("default-run");
    let explicit = root.join("entry-run");
    for folder in [&data, &default, &explicit] {
        std::fs::create_dir_all(folder.join("bin")).unwrap();
    }
    std::fs::write(default.join("bin/app.exe"), "fixture").unwrap();
    std::fs::write(explicit.join("bin/app.exe"), "fixture").unwrap();
    (data, default, explicit)
}
#[test]
fn validation_uses_entry_directory_before_dataset_or_default() {
    let (data, _, explicit) = fixture();
    let mut entry = parse_entries("app\tFixture\t\".\\bin\\app.exe\" --flag\t0\tnone\n")
        .unwrap()
        .remove(0);
    entry.working_directory = explicit.to_string_lossy().into_owned();
    assert_eq!(management::validate(&entry, &data), Validation::Present);
}
#[test]
fn absolute_conversion_preserves_the_original_target_and_working_directory() {
    let (data, _, explicit) = fixture();
    let mut entry = parse_entries("app\tFixture\t\".\\bin\\app.exe\" --flag\t0\tnone\n")
        .unwrap()
        .remove(0);
    entry.working_directory = explicit.to_string_lossy().into_owned();
    let directory = entry.working_directory.clone();
    assert!(management::convert_path(&mut entry, &data, false).unwrap());
    let target = altrun_probe::launch::resolve_typed(&entry.command, None, entry.param_type)
        .unwrap()
        .file;
    assert_eq!(
        std::fs::canonicalize(target).unwrap(),
        std::fs::canonicalize(explicit.join("bin/app.exe")).unwrap()
    );
    assert_eq!(entry.working_directory, directory);
}
#[test]
fn relative_conversion_keeps_default_directory_semantics() {
    let (_, default, _) = fixture();
    let mut entry = management::from_path(&default.join("bin/app.exe")).unwrap();
    assert!(management::convert_path(&mut entry, &default, true).unwrap());
    assert!(
        entry.working_directory.is_empty(),
        "relative conversion changed the process working directory"
    );
}
#[test]
fn network_working_directories_are_unconfirmed_not_missing() {
    let (data, _, _) = fixture();
    let mut entry = parse_entries("app\tFixture\t\".\\bin\\app.exe\"\t0\tnone\n")
        .unwrap()
        .remove(0);
    entry.working_directory = r"\\server\share".into();
    assert!(matches!(
        management::validate(&entry, &data),
        Validation::Unconfirmed(_)
    ));
}

#[test]
fn portable_target_resolution_does_not_set_the_process_working_directory() {
    use altrun_probe::{launch::LaunchContext, launch::resolve_context, library::ParamType};
    let (_, base, explicit) = fixture();
    let base = base.join("bin");
    let mut context = LaunchContext {
        base_directory: base.to_string_lossy().into_owned(),
        ..LaunchContext::default()
    };
    for command in ["./app.exe", "app.exe", "app"] {
        let (request, _, directory) =
            resolve_context(command, None, ParamType::None, "", &context).unwrap();
        assert_eq!(
            std::fs::canonicalize(request.file).unwrap(),
            std::fs::canonicalize(base.join("app.exe")).unwrap()
        );
        assert_eq!(directory, None);
    }
    for command in ["notepad.exe", "https://example.com", "shell:Downloads"] {
        let (request, _, directory) =
            resolve_context(command, None, ParamType::None, "", &context).unwrap();
        assert_eq!(request.file, command);
        assert_eq!(directory, None);
    }
    context.base_is_working_directory = true;
    let (_, _, directory) =
        resolve_context("./app.exe", None, ParamType::None, "", &context).unwrap();
    assert_eq!(directory.as_deref(), base.to_str());
    let (request, _, directory) = resolve_context(
        "./bin/app.exe",
        None,
        ParamType::None,
        explicit.to_str().unwrap(),
        &context,
    )
    .unwrap();
    assert_eq!(directory.as_deref(), explicit.to_str());
    assert_eq!(
        std::fs::canonicalize(request.file).unwrap(),
        std::fs::canonicalize(explicit.join("bin/app.exe")).unwrap()
    );
}
