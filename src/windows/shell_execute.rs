// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

//! Windows Shell execution shared by launch, test and Send To.
use crate::ui::wide;
use altrun_probe::{
    Entry,
    launch::{LaunchContext, LaunchRequest, ShowMode, resolve_entry},
};
use std::mem::zeroed;
use windows_sys::Win32::{
    Foundation::{ERROR_CANCELLED, GetLastError},
    UI::{Shell::*, WindowsAndMessaging::*},
};

pub(crate) fn execute_request(request: &LaunchRequest) -> Result<(), String> {
    execute_request_with(request, ShowMode::Normal, None, false)
}

pub(crate) fn execute_entry(
    entry: &Entry,
    parameter: Option<&str>,
    context: &LaunchContext,
) -> Result<(), String> {
    let (request, mode, directory) = resolve_entry(entry, parameter, context)?;
    execute_request_with(&request, mode, directory.as_deref(), entry.run_as_admin)
}

fn supports_elevation(file: &str) -> bool {
    let path = std::path::Path::new(file);
    if file.contains("://")
        || file
            .split_once(':')
            .is_some_and(|(prefix, _)| prefix.len() != 1 && !path.is_absolute())
    {
        return false;
    }
    match path.extension() {
        Some(extension) => {
            extension.eq_ignore_ascii_case("exe") || extension.eq_ignore_ascii_case("com")
        }
        None => !file.contains([':', '\\', '/']),
    }
}

fn execute_request_with(
    request: &LaunchRequest,
    mode: ShowMode,
    directory: Option<&str>,
    run_as_admin: bool,
) -> Result<(), String> {
    use windows_sys::Win32::System::Com::*;
    if run_as_admin && !supports_elevation(&request.file) {
        return Err("ui.administrator_requires_program".into());
    }
    let file = wide(&request.file);
    let parameters = wide(&request.parameters);
    let directory = match directory {
        Some(directory) => wide(directory),
        None => {
            let profile = std::env::var_os("USERPROFILE")
                .map(std::path::PathBuf::from)
                .filter(|path| path.is_absolute() && path.is_dir());
            let directory = profile.map_or_else(altrun_probe::windows_system_directory, Ok)?;
            wide(&directory.to_string_lossy())
        }
    };
    unsafe {
        let hr = CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED as u32);
        if hr < 0 {
            return Err(crate::localization::message(
                "ui.could_not_initialize_the_launch_service_0x",
                &[("code", &format!("{:08X}", hr as u32))],
            ));
        }
        let mut info: SHELLEXECUTEINFOW = zeroed();
        info.cbSize = std::mem::size_of_val(&info) as u32;
        info.fMask = SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI;
        // lpVerb must outlive ShellExecuteExW, just like file and parameters.
        let verb = wide(if run_as_admin { "runas" } else { "open" });
        info.lpVerb = verb.as_ptr();
        info.lpFile = file.as_ptr();
        info.lpParameters = parameters.as_ptr();
        info.lpDirectory = directory.as_ptr();
        info.nShow = match mode {
            ShowMode::Normal => SW_SHOWNORMAL,
            ShowMode::Maximized => SW_SHOWMAXIMIZED,
            ShowMode::Minimized => SW_SHOWMINNOACTIVE,
            ShowMode::Hidden => SW_HIDE,
        };
        let ok = ShellExecuteExW(&mut info);
        let error = GetLastError();
        CoUninitialize();
        if ok != 0 {
            Ok(())
        } else if run_as_admin && error == ERROR_CANCELLED {
            Err("ui.administrator_launch_cancelled".into())
        } else {
            Err(crate::localization::message(
                "ui.could_not_launch_error_error_check_the",
                &[("error", &error.to_string())],
            ))
        }
    }
}
