#![cfg(windows)]
mod review_native_support;
use review_native_support::*;
#[test]
fn manager_conversion_honors_the_configured_default_directory() {
    let root = std::env::current_dir()
        .unwrap()
        .join("target/review-manager-directory");
    let folder = root.join("data");
    let working = root.join("working");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::create_dir_all(working.join("bin")).unwrap();
    std::fs::write(working.join("bin/app.exe"), "fixture").unwrap();
    let data = folder.join("shortcuts.tsv");
    std::fs::write(&data, "app\tFixture\t\".\\bin\\app.exe\" --flag\t0\tnone\n").unwrap();
    std::fs::write(
        data.with_extension("settings.ini"),
        format!("[Config]\nWorkingDirectory={}\n", working.display()),
    )
    .unwrap();
    let (process, launcher) = start(&data, Some("--safe-mode"));
    let dialog = manager(launcher, process.0.id());
    command(dialog, 2127);
    command(dialog, 2113);
    let rows = altrun_probe::library::Snapshot::open(&data)
        .unwrap()
        .entries;
    let target = altrun_probe::launch::resolve_typed(&rows[0].command, None, rows[0].param_type)
        .unwrap()
        .file;
    assert_eq!(
        std::fs::canonicalize(target).unwrap(),
        std::fs::canonicalize(working.join("bin/app.exe")).unwrap()
    );
}
