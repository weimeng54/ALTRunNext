use altrun_probe::{
    management::{self, Validation},
    parse_entries,
};
#[test]
fn validation_does_not_classify_network_system_or_search_targets_as_missing() {
    let rows=parse_entries("a\tA\t\\\\server\\file.exe\t0\nb\tB\tnotepad.exe\t0\nc\tC\thttps://example.com\t0\nd\tD\ttool.exe %p\t0\n").unwrap();
    for row in rows {
        assert!(matches!(
            management::validate(&row, std::path::Path::new(".")),
            Validation::Unconfirmed(_)
        ));
    }
}
#[test]
fn legacy_path_conversion_preserves_parameters_and_id() {
    let root = std::env::current_dir().unwrap();
    let path = root.join("target/folder with spaces/tool.exe");
    let mut entry = parse_entries(&format!("tool\tTool\t\"{}\" --flag\t0\tnone", path.display()))
        .unwrap()
        .remove(0);
    let id = entry.id.clone();
    assert!(entry.command.starts_with('"'));
    assert!(management::convert_path(&mut entry, &root, true).unwrap());
    assert!(entry.command.ends_with(" --flag"));
    assert_eq!(entry.id, id);
    assert!(management::convert_path(&mut entry, &root, false).unwrap());
    assert!(entry.command.contains("folder with spaces"));
}
#[test]
fn sent_documents_round_trip_without_interpreting_the_path_as_a_command() {
    use altrun_probe::{launch, library::{self, OpenTarget}};
    let base = std::env::current_dir().unwrap().join("target");
    let document = base.join("会议 %c 纪要.txt");
    let mut entry = management::from_path(&document).unwrap();
    assert_eq!(entry.command, document.to_str().unwrap());
    assert_eq!(entry.open_target, Some(OpenTarget::Document(None)));
    assert_eq!(entry.name, "会议 %c 纪要.txt");
    let (request, _, _) = launch::resolve_entry(&entry, None, &Default::default()).unwrap();
    assert_eq!(request.file, entry.command);
    assert!(request.parameters.is_empty());
    let program = std::env::current_exe().unwrap().to_str().unwrap().to_owned();
    entry.open_target = Some(OpenTarget::Document(Some(program.clone())));
    let loaded = parse_entries(&library::serialize(&[entry.clone()]).unwrap()).unwrap();
    assert_eq!(loaded[0].open_target, entry.open_target);
    let (request, _, _) = launch::resolve_entry(&loaded[0], None, &Default::default()).unwrap();
    assert_eq!(request.file, program);
    assert_eq!(request.parameters, format!("\"{}\"", document.display()));
    assert!(!management::convert_path(&mut entry, &base, true).unwrap());
}
#[test]
fn open_directory_ignores_parameter_markers_in_the_argument_tail() {
    let root = std::env::current_dir().unwrap();
    let executable = root.join("target/folder with spaces/tool.exe");
    let command = format!(
        "\"{}\" --query {{%p}} --window {{%wt}}",
        executable.display()
    );
    assert_eq!(
        altrun_probe::local_target::directory(&command, &root).unwrap(),
        executable.parent().unwrap()
    );
    assert!(altrun_probe::local_target::directory("https://example.com", &root).is_err());
}
#[cfg(windows)]
#[test]
fn integration_status_and_rollback_use_isolated_namespaces() {
    use altrun_probe::integration::Integration;
    let root = std::env::current_dir()
        .unwrap()
        .join("target/integration-test");
    std::fs::create_dir_all(&root).unwrap();
    let name = format!("fixture-{}", std::process::id());
    let integration = Integration {
        registry: format!(r"Software\ALTRunModern\Tests\{name}"),
        name: name.clone(),
        send_to: root.join(format!("{name}.lnk")),
        executable: std::env::current_exe().unwrap(),
        data: root.join("shortcuts.tsv"),
    };
    integration.apply(false, false).unwrap();
    assert_eq!(integration.status().unwrap(), (false, false));
    integration.apply(true, true).unwrap();
    assert_eq!(integration.status().unwrap(), (true, true));
    integration.apply(false, false).unwrap();
    assert_eq!(integration.status().unwrap(), (false, false));
    integration.apply(false, true).unwrap();
    integration.apply(false, true).unwrap();
    assert_eq!(integration.status().unwrap(), (false, true));
    integration.apply(false, false).unwrap();
    integration.apply(false, false).unwrap();
    assert_eq!(integration.status().unwrap(), (false, false));
}
