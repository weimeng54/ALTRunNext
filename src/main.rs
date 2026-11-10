#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(windows)]
mod manager;
#[cfg(windows)]
mod native;
#[cfg(windows)]
mod theme;
#[cfg(windows)]
mod settings;
#[cfg(windows)]
mod instance;
#[cfg(windows)]
mod skin;

fn main() {
    if std::env::args().any(|arg|arg=="--regex-worker") {
        std::process::exit(i32::from(altrun_probe::isolated_matching::worker_main().is_err()));
    }
    if std::env::args().any(|arg| arg == "--convert-legacy") {
        let result = convert_legacy();
        if let Err(error) = &result {
            eprintln!("{error}");
        }
        std::process::exit(i32::from(result.is_err()));
    }
    if std::env::args().any(|arg| arg == "--check") {
        return;
    }
    #[cfg(windows)]
    if let Err(message) = native::run() {
        // A prototype must fail visibly rather than silently running without a hotkey.
        native::show_error(&message);
        std::process::exit(1);
    }
}

fn convert_legacy() -> Result<(), String> {
    let args: Vec<_> = std::env::args_os().collect();
    let value = |flag: &str| {
        args.iter()
            .position(|a| a == flag)
            .and_then(|i| args.get(i + 1))
            .ok_or_else(|| format!("Missing {flag} value"))
    };
    let source = std::path::PathBuf::from(value("--convert-legacy")?);
    let destination = std::path::PathBuf::from(value("--output")?);
    let original = altrun_probe::library::Snapshot::open(&source)?;
    let mut output = altrun_probe::library::Snapshot::new(destination, original.entries.clone())?;
    output.save(&original.entries)
}
