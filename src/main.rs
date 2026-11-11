#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(windows)]
mod input_dialog;
#[cfg(windows)]
mod instance;
#[cfg(windows)]
mod localization;
#[cfg(windows)]
mod manager;
#[cfg(windows)]
mod native;
#[cfg(windows)]
mod search_ime;
#[cfg(windows)]
mod settings;
#[cfg(windows)]
mod settings_pickers;
#[cfg(windows)]
mod skin;
#[cfg(windows)]
mod theme;
#[cfg(windows)]
mod window_actions;
#[cfg(windows)]
mod window_geometry;

fn main() {
    #[cfg(windows)]
    if let Some(pid) = std::env::args()
        .collect::<Vec<_>>()
        .windows(2)
        .find(|p| p[0] == "--wait-pid")
        .and_then(|p| p[1].parse::<u32>().ok())
    {
        unsafe {
            use windows_sys::Win32::{Foundation::*, System::Threading::*};
            let process = OpenProcess(0x00100000, 0, pid);
            if !process.is_null() {
                WaitForSingleObject(process, 10000);
                CloseHandle(process);
            }
        }
    }
    if std::env::args().any(|arg| arg == "--migrate") {
        let args = std::env::args_os().collect::<Vec<_>>();
        let value = |key: &str| {
            args.iter()
                .position(|a| a == key)
                .and_then(|i| args.get(i + 1))
                .map(std::path::PathBuf::from)
                .ok_or_else(|| format!("Missing {key}"))
        };
        let result = value("--migrate").and_then(|source| {
            value("--output-dir").and_then(|destination| {
                altrun_probe::migration::Migration::read(
                    &source,
                    args.iter().any(|a| a == "--legacy-appearance"),
                )
                .and_then(|m| m.write_new(&destination))
            })
        });
        if let Err(error) = &result {
            eprintln!("{error}");
        }
        std::process::exit(i32::from(result.is_err()));
    }
    if std::env::args().any(|arg| arg == "--regex-worker") {
        std::process::exit(i32::from(
            altrun_probe::isolated_matching::worker_main().is_err(),
        ));
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
