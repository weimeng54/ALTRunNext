// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

//! Per-user startup and File Explorer integration for each data file.
use std::{
    path::{Path, PathBuf},
    ptr::null_mut,
};
use windows_sys::Win32::{
    Foundation::*,
    Globalization::{CSTR_EQUAL, CompareStringOrdinal},
    System::Registry::*,
    UI::Shell::*,
};
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
pub struct Integration {
    pub registry: String,
    pub name: String,
    pub send_to: PathBuf,
    pub executable: PathBuf,
    pub data: PathBuf,
}
impl Integration {
    pub fn for_data(data: &Path) -> Result<Self, String> {
        let data = std::path::absolute(data).map_err(|e| e.to_string())?;
        unsafe {
            let mut b = vec![0u16; 32768];
            if SHGetFolderPathW(null_mut(), 9, null_mut(), 0, b.as_mut_ptr()) < 0 {
                return Err("ui.could_not_find_the_send_to_folder".into());
            }
            let folder = PathBuf::from(String::from_utf16_lossy(
                &b[..b.iter().position(|c| *c == 0).unwrap_or(0)],
            ));
            let mut hash = 0u64;
            for byte in data.to_string_lossy().to_lowercase().bytes() {
                hash = hash.wrapping_mul(131).wrapping_add(byte as u64);
            }
            let name = format!("ALTRunNext-{hash:016x}");
            Ok(Self {
                registry: r"Software\Microsoft\Windows\CurrentVersion\Run".into(),
                send_to: folder.join("ALTRunNext.lnk"),
                name,
                executable: std::env::current_exe().map_err(|e| e.to_string())?,
                data,
            })
        }
    }
    pub fn command(&self) -> String {
        format!(
            "\"{}\" --enable-launch --data \"{}\"",
            self.executable.display(),
            self.data.display()
        )
    }
    pub fn status(&self) -> Result<(bool, bool), String> {
        let current = self.startup_value(&self.name)?;
        let previous = self
            .previous_name()
            .map(|name| self.startup_value(&name))
            .transpose()?
            .flatten();
        let startup = current.is_some() || previous.is_some();
        let previous_link = self.previous_link().is_some_and(|path| path.is_file());
        let send_to = previous_link || self.link_state()? != LinkState::Other;
        Ok((startup, send_to))
    }

    fn previous_name(&self) -> Option<String> {
        self.name
            .strip_prefix("ALTRunNext-")
            .map(|hash| format!("ALTRunModern-{hash}"))
    }

    fn previous_link(&self) -> Option<PathBuf> {
        self.previous_name()
            .map(|name| self.send_to.with_file_name(format!("{name}.lnk")))
    }

    fn startup_value(&self, name: &str) -> Result<Option<String>, String> {
        unsafe {
            let mut key = null_mut();
            let status = RegOpenKeyExW(
                HKEY_CURRENT_USER,
                wide(&self.registry).as_ptr(),
                0,
                KEY_QUERY_VALUE,
                &mut key,
            );
            if status == ERROR_FILE_NOT_FOUND {
                Ok(None)
            } else if status != 0 {
                Err(crate::localization::message(
                    "ui.could_not_read_the_startup_entry_status",
                    &[("status", &status.to_string())],
                ))
            } else {
                let mut b = vec![0u16; 32768];
                let mut size = (b.len() * 2) as u32;
                let mut kind = 0;
                let status = RegQueryValueExW(
                    key,
                    wide(name).as_ptr(),
                    std::ptr::null(),
                    &mut kind,
                    b.as_mut_ptr() as _,
                    &mut size,
                );
                RegCloseKey(key);
                if status == ERROR_FILE_NOT_FOUND {
                    Ok(None)
                } else if status != 0 {
                    Err(crate::localization::message(
                        "ui.could_not_read_the_startup_entry_status",
                        &[("status", &status.to_string())],
                    ))
                } else {
                    Ok((kind == REG_SZ).then(|| {
                        String::from_utf16_lossy(&b[..(size as usize / 2).min(b.len())])
                            .trim_end_matches('\0')
                            .to_owned()
                    }))
                }
            }
        }
    }
    fn startup(&self, name: &str, value: Option<&str>) -> Result<(), String> {
        unsafe {
            let mut key = null_mut();
            let status = RegCreateKeyExW(
                HKEY_CURRENT_USER,
                wide(&self.registry).as_ptr(),
                0,
                std::ptr::null(),
                0,
                KEY_SET_VALUE,
                std::ptr::null(),
                &mut key,
                std::ptr::null_mut(),
            );
            if status != 0 {
                return Err(crate::localization::message(
                    "ui.could_not_change_the_startup_entry_status",
                    &[("status", &status.to_string())],
                ));
            }
            let status = if let Some(value) = value {
                let command = wide(value);
                RegSetValueExW(
                    key,
                    wide(name).as_ptr(),
                    0,
                    REG_SZ,
                    command.as_ptr() as _,
                    (command.len() * 2) as u32,
                )
            } else {
                RegDeleteValueW(key, wide(name).as_ptr())
            };
            RegCloseKey(key);
            if status != 0 && status != ERROR_FILE_NOT_FOUND {
                return Err(crate::localization::message(
                    "ui.could_not_update_the_startup_entry_status",
                    &[("status", &status.to_string())],
                ));
            }
            Ok(())
        }
    }
    fn link_arguments(&self) -> String {
        format!("--enable-launch --data \"{}\" --add", self.data.display())
    }

    fn link_state(&self) -> Result<LinkState, String> {
        if !self
            .send_to
            .try_exists()
            .map_err(|error| error.to_string())?
        {
            return Ok(LinkState::Other);
        }
        let link = crate::shell_link::read(&self.send_to)
            .ok_or("ui.could_not_read_the_send_to_shortcut")?;
        if !same_link_text(&link.arguments, &self.link_arguments()) {
            return Ok(LinkState::Other);
        }
        let executable = self.executable.to_string_lossy();
        let current_icon = link
            .icon
            .is_some_and(|(path, index)| index == 0 && same_link_text(&path, &executable));
        if current_icon && same_link_text(&link.path, &executable) {
            Ok(LinkState::Current)
        } else {
            Ok(LinkState::PreviousExecutable)
        }
    }

    fn remove_link(path: &Path) -> Result<(), String> {
        match std::fs::remove_file(path) {
            Ok(()) => {
                notify_send_to(path, SHCNE_DELETE as i32);
                Ok(())
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }

    fn create_link(&self) -> Result<(), String> {
        if let Some(directory) = self.send_to.parent() {
            std::fs::create_dir_all(directory).map_err(|e| e.to_string())?;
        }
        crate::shell_link::write(
            &self.send_to,
            &self.executable,
            &self.link_arguments(),
            self.data.parent().unwrap_or(Path::new(".")),
        )
        .ok_or("ui.could_not_create_the_send_to_shortcut")?;
        notify_send_to(&self.send_to, SHCNE_CREATE as i32);
        Ok(())
    }

    pub fn apply(&self, startup: bool, send_to: bool) -> Result<(), String> {
        let link = self.link_state()?;
        let previous = self.startup_value(&self.name)?;
        let command = startup.then(|| self.command());
        let old = self.previous_name();
        let old_value = old
            .as_deref()
            .map(|name| self.startup_value(name))
            .transpose()?
            .flatten();
        if previous != command {
            self.startup(&self.name, command.as_deref())?;
        }
        let result = (|| {
            if send_to && link != LinkState::Current {
                self.create_link()?;
            } else if !send_to && link != LinkState::Other {
                Self::remove_link(&self.send_to)?;
            }
            if let Some(path) = self.previous_link() {
                Self::remove_link(&path)?;
            }
            if let Some(name) = &old
                && old_value.is_some()
            {
                self.startup(name, None)?;
            }
            Ok(())
        })();
        if result.is_err() && previous != command {
            let _ = self.startup(&self.name, previous.as_deref());
        }
        result
    }
}

#[derive(PartialEq, Eq)]
enum LinkState {
    Current,
    PreviousExecutable,
    Other,
}

fn notify_send_to(path: &Path, event: i32) {
    let path = wide(&path.to_string_lossy());
    unsafe {
        SHChangeNotify(event, SHCNF_PATHW, path.as_ptr().cast(), std::ptr::null());
    }
}

fn same_link_text(left: &str, right: &str) -> bool {
    if left == right {
        return true;
    }
    let left = wide(left);
    let right = wide(right);
    unsafe { CompareStringOrdinal(left.as_ptr(), -1, right.as_ptr(), -1, 1) == CSTR_EQUAL }
}
