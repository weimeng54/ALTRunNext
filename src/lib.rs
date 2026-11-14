// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub alias: String,
    pub name: String,
    pub command: String,
    pub frequency: i64,
    pub param_type: library::ParamType,
    pub id: String,
    pub working_directory: String,
    pub open_target: Option<library::OpenTarget>,
    pub kind: library::EntryKind,
}

#[cfg(windows)]
pub fn safe_notepad_path() -> Result<std::path::PathBuf, String> {
    Ok(windows_system_directory()?.join("notepad.exe"))
}

#[cfg(windows)]
pub fn windows_system_directory() -> Result<std::path::PathBuf, String> {
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Win32::System::SystemInformation::GetSystemDirectoryW;
    let mut buffer = vec![0_u16; 512];
    loop {
        let size = unsafe { GetSystemDirectoryW(buffer.as_mut_ptr(), buffer.len() as u32) };
        if size == 0 {
            return Err("Cannot resolve the Windows system directory".into());
        }
        if size as usize >= buffer.len() {
            buffer.resize(size as usize + 1, 0);
            continue;
        }
        let directory = std::ffi::OsString::from_wide(&buffer[..size as usize]);
        return Ok(std::path::PathBuf::from(directory));
    }
}
#[cfg(windows)]
pub mod integration;
mod language_pack;
pub mod launch;
pub mod library;
pub use library::parse_entries;
mod search;
pub use search::{SearchIndex, advance, search};
pub mod library_updates;
pub mod local_target;
pub mod localization;
pub mod management;
pub mod preferences;
#[cfg(windows)]
mod shell_link;
pub mod state_store;
pub mod textcodec;
pub mod usage;
