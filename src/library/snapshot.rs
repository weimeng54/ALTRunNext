// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::{decode, serialize};
use crate::Entry;
use std::{
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

pub struct Snapshot {
    pub path: PathBuf,
    pub entries: Vec<Entry>,
    original: Option<Vec<u8>>,
}
impl Snapshot {
    pub fn new(path: PathBuf, entries: Vec<Entry>) -> Result<Self, String> {
        serialize(&entries)?;
        Ok(Self {
            path,
            entries,
            original: None,
        })
    }
    pub fn open(path: &Path) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|e| {
            crate::localization::message(
                "ui.could_not_read_shortcuts_error",
                &[("error", &e.to_string())],
            )
        })?;
        if bytes.len() > 16 * 1024 * 1024 {
            return Err("ui.the_shortcuts_file_exceeds_16_mib".into());
        }
        let text = decode(&bytes)?;
        let entries = crate::parse_entries(&text)?;
        Ok(Self {
            path: path.into(),
            entries,
            original: Some(bytes),
        })
    }
    pub fn save(&mut self, entries: &[Entry]) -> Result<(), String> {
        let content = serialize(entries)?;
        let current = match std::fs::read(&self.path) {
            Ok(b) => Some(b),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e.to_string()),
        };
        if current != self.original {
            return Err("ui.the_data_file_changed_outside_altrunnext_keep".into());
        }
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let stage = self.path.with_extension(format!(
            "altrun-tmp-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        struct Cleanup(PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(&self.0);
            }
        }
        let mut file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&stage)
            .map_err(|e| {
                crate::localization::message(
                    "ui.could_not_create_a_temporary_file_e",
                    &[("e", &e.to_string())],
                )
            })?;
        let _cleanup = Cleanup(stage.clone());
        file.write_all(content.as_bytes())
            .and_then(|_| file.sync_all())
            .map_err(|e| {
                crate::localization::message(
                    "ui.could_not_write_the_changes_the_original",
                    &[("e", &e.to_string())],
                )
            })?;
        drop(file);
        replace(&stage, &self.path, self.original.is_some())?;
        self.original = Some(content.into_bytes());
        self.entries = entries.to_vec();
        Ok(())
    }
}
pub(crate) fn replace(stage: &Path, destination: &Path, existed: bool) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::{Foundation::GetLastError, Storage::FileSystem::*};
        let wide = |p: &Path| {
            p.as_os_str()
                .encode_wide()
                .chain(Some(0))
                .collect::<Vec<_>>()
        };
        let stage = wide(stage);
        let target = wide(destination);
        let extension = format!(
            "{}.bak",
            destination
                .extension()
                .and_then(|p| p.to_str())
                .unwrap_or("data")
        );
        let backup = wide(&destination.with_extension(extension));
        let ok = unsafe {
            if existed {
                ReplaceFileW(
                    target.as_ptr(),
                    stage.as_ptr(),
                    backup.as_ptr(),
                    0,
                    std::ptr::null(),
                    std::ptr::null(),
                )
            } else {
                MoveFileExW(stage.as_ptr(), target.as_ptr(), MOVEFILE_WRITE_THROUGH)
            }
        };
        if ok == 0 {
            return Err(crate::localization::message(
                "ui.could_not_save_error_code_keep_a",
                &[("code", &unsafe { GetLastError() }.to_string())],
            ));
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = (stage, destination, existed);
        Err("Windows only".into())
    }
}
