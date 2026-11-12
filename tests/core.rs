use altrun_probe::{Entry, advance, parse_entries, search};

fn entry(alias: &str, frequency: i64) -> Entry {
    Entry {
        alias: alias.into(),
        name: "test".into(),
        command: "notepad.exe".into(),
        frequency,
        param_type: altrun_probe::library::ParamType::Auto,
        id: String::new(),
        working_directory: String::new(),
        open_target: None,
        enabled: true,
        kind: altrun_probe::library::EntryKind::Command,
    }
}

#[test]
fn private_dataset_preserves_duplicate_alias_and_command() {
    let rows = parse_entries("np\t记事本\tnotepad.exe\t7\nnp\t另一个\tother.exe\t2\n").unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].name, "记事本");
    assert_eq!(rows[1].command, "other.exe");
    assert_eq!(rows[0].frequency, 7);
}

#[test]
fn invalid_dataset_is_rejected_instead_of_silently_losing_commands() {
    assert!(parse_entries("np\tbad").is_err());
}

#[test]
fn search_uses_alias_and_original_frequency_position_length_score() {
    let entries = vec![
        entry("xnote", 0),
        entry("notepad", 7),
        entry("note", 0),
        entry("note", 0),
    ];
    assert_eq!(search(&entries, "NOTE"), vec![2, 3, 1, 0]);
    assert_eq!(search(&entries, ""), vec![1, 0, 2, 3]);
    assert_eq!(search(&entries, "notepad-is-longer"), Vec::<usize>::new());
}

#[test]
fn selection_wraps_without_underflow_and_handles_empty_results() {
    assert_eq!(advance(0, 10, -1), 9);
    assert_eq!(advance(9, 10, 1), 0);
    assert_eq!(advance(0, 0, -1), 0);
}
