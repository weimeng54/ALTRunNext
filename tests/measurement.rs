#![cfg(windows)]

use std::{process::Command, time::SystemTime};

#[test]
fn rejected_measurement_keeps_a_machine_readable_pending_report() {
    let unique = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let output = std::env::current_dir()
        .unwrap()
        .join("target")
        .join(format!("rejected-measurement-{unique}.json"));
    let status = Command::new(env!("CARGO_BIN_EXE_altrun-measure"))
        .args([
            "--original-pid",
            "4294967295",
            "--probe-pid",
            "4294967294",
            "--confirm-original-auto-execute-off",
            "--output",
        ])
        .arg(&output)
        .status()
        .unwrap();
    assert!(!status.success());
    assert!(
        output.exists(),
        "rejected measurements must preserve a report instead of discarding evidence"
    );
    let validated = Command::new("python")
        .arg("-c")
        .arg(concat!(
            "import json,sys; r=json.load(open(sys.argv[1],encoding='utf-8')); ",
            "assert r['decision']=='pending'; assert r['session_status']=='rejected'; ",
            "assert r['stage']=='original_identity'; assert r['samples']==[]"
        ))
        .arg(&output)
        .status()
        .unwrap();
    assert!(validated.success(), "rejection report must be valid and conservative");
}
