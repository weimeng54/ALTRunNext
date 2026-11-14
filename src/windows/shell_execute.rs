// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

//! Windows Shell execution shared by launch, test and Send To.
use crate::ui::wide;
use std::mem::zeroed;
use windows_sys::Win32::{
    Foundation::GetLastError,
    UI::{Shell::*, WindowsAndMessaging::*},
};

pub(crate) fn execute_request(request: &altrun_probe::launch::LaunchRequest) -> Result<(), String> {
    execute_request_with(request, altrun_probe::launch::ShowMode::Normal, None)
}
pub(crate) fn execute_request_with(
    request: &altrun_probe::launch::LaunchRequest,
    mode: altrun_probe::launch::ShowMode,
    directory: Option<&str>,
) -> Result<(), String> {
    use windows_sys::Win32::System::Com::*;
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
        let verb = wide("open");
        info.lpVerb = verb.as_ptr();
        info.lpFile = file.as_ptr();
        info.lpParameters = parameters.as_ptr();
        info.lpDirectory = directory.as_ptr();
        info.nShow = match mode {
            altrun_probe::launch::ShowMode::Normal => SW_SHOWNORMAL,
            altrun_probe::launch::ShowMode::Maximized => SW_SHOWMAXIMIZED,
            altrun_probe::launch::ShowMode::Minimized => SW_SHOWMINNOACTIVE,
            altrun_probe::launch::ShowMode::Hidden => SW_HIDE,
        };
        let ok = ShellExecuteExW(&mut info);
        let error = GetLastError();
        CoUninitialize();
        if ok == 0 {
            Err(crate::localization::message(
                "ui.could_not_launch_error_error_check_the",
                &[("error", &error.to_string())],
            ))
        } else {
            Ok(())
        }
    }
}
