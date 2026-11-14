#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(windows)]
mod input_dialog;
#[cfg(windows)]
mod instance;
#[cfg(windows)]
use altrun_probe::localization;
#[cfg(windows)]
mod manager;
#[cfg(windows)]
mod manager_menu;
#[cfg(windows)]
mod manager_table;
#[cfg(windows)]
mod native;
#[cfg(windows)]
mod search_ime;
#[cfg(windows)]
mod settings;
#[cfg(windows)]
mod settings_layout;
#[cfg(windows)]
mod settings_pickers;
#[cfg(windows)]
mod settings_preview;
#[cfg(windows)]
mod shortcut_intake;
#[cfg(windows)]
mod skin;
#[cfg(windows)]
mod startup_warning;
#[cfg(windows)]
mod theme;
#[cfg(windows)]
mod theme_ui;
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
    if std::env::args().any(|arg| arg == "--check") {
        let args = std::env::args_os().collect::<Vec<_>>();
        let result = if let Some(i) = args.iter().position(|arg| arg == "--data") {
            args.get(i + 1)
                .ok_or_else(|| "--data needs a path".to_owned())
                .and_then(|path| altrun_probe::library::Snapshot::open(path.as_ref()))
                .map(|_| ())
        } else {
            Ok(())
        };
        if let Err(error) = &result {
            eprintln!("{}", altrun_probe::localization::tr(error));
        }
        std::process::exit(i32::from(result.is_err()));
    }
    #[cfg(windows)]
    if let Err(message) = native::run() {
        native::show_error(&localization::message(
            "launcher.startup_error",
            &[("detail", &localization::tr(&message))],
        ));
        std::process::exit(1);
    }
}
