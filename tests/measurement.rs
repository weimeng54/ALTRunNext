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
    assert!(
        validated.success(),
        "rejection report must be valid and conservative"
    );
}

#[test]
fn later_failure_preserves_earlier_synthetic_observations_without_accepting_them() {
    let unique = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let output = std::env::current_dir()
        .unwrap()
        .join("target")
        .join(format!("partial-measurement-{unique}.json"));
    let status = Command::new(env!("CARGO_BIN_EXE_altrun-measure"))
        .args([
            "--original-pid",
            "4294967295",
            "--probe-pid",
            "4294967294",
            "--confirm-original-auto-execute-off",
            "--diagnostic-fixture-fail-after-one",
            "--output",
        ])
        .arg(&output)
        .status()
        .unwrap();
    assert!(!status.success());
    assert!(
        output.exists(),
        "partial observations must survive a later failure"
    );
    let validated = Command::new("python")
        .arg("-c")
        .arg(concat!(
            "import json,sys; r=json.load(open(sys.argv[1],encoding='utf-8')); ",
            "assert r['decision']=='pending'; assert r['session_status']=='rejected'; ",
            "assert r['stage']=='fixture_failure'; assert len(r['samples'])==1; ",
            "assert r['measurement_kind']=='synthetic-test'"
        ))
        .arg(&output)
        .status()
        .unwrap();
    assert!(validated.success());
}

#[test]
fn probe_replay_never_requires_or_falls_back_to_an_original_process() {
    let unique = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let output = std::env::current_dir()
        .unwrap()
        .join("target")
        .join(format!("isolated-probe-replay-{unique}.json"));
    let status = Command::new(env!("CARGO_BIN_EXE_altrun-measure"))
        .args([
            "--probe-only-replay",
            "--probe-pid",
            "4294967294",
            "--output",
        ])
        .arg(&output)
        .status()
        .unwrap();
    assert!(!status.success());
    assert!(
        output.exists(),
        "probe-only replay must create its own isolated diagnostic report"
    );
    let validated = Command::new("python")
        .arg("-c")
        .arg(concat!(
            "import json,sys; r=json.load(open(sys.argv[1],encoding='utf-8')); ",
            "assert r['decision']=='pending'; assert r['session_status']=='rejected'; ",
            "assert r['stage']=='probe_identity'; assert r['target']=='probe'; ",
            "assert r['measurement_kind']=='observer-self-replay'; assert r['samples']==[]"
        ))
        .arg(&output)
        .status()
        .unwrap();
    assert!(validated.success());
}

#[test]
fn window_stack_inspection_is_isolated_from_the_original_and_from_replay() {
    let unique = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let output = std::env::current_dir()
        .unwrap()
        .join("target")
        .join(format!("window-stack-inspection-{unique}.json"));
    let status = Command::new(env!("CARGO_BIN_EXE_altrun-measure"))
        .args(["--inspect-window-stack", "--output"])
        .arg(&output)
        .status()
        .unwrap();
    assert!(status.success(), "read-only inspection requires no original or probe PID");
    let validated = Command::new("python")
        .arg("-c")
        .arg(concat!(
            "import json,sys; r=json.load(open(sys.argv[1],encoding='utf-8')); ",
            "assert r['decision']=='pending'; assert r['samples']==[]; ",
            "assert r['stage']=='window_stack_inspected'; assert r['current_trace']==[]; ",
            "assert r['measurement_kind']=='observer-window-stack'; ",
            "assert not r['diagnostic_windows']['root']['visible']; ",
            "assert bytes.fromhex(r['diagnostic_windows']['root']['class_utf8_hex']).decode().lower()=='static'"
        ))
        .arg(&output)
        .status()
        .unwrap();
    assert!(validated.success());
}
