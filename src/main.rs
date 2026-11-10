#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(windows)]
mod native;
#[cfg(windows)]
mod theme;
#[cfg(windows)]
mod manager;

fn main() {
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
