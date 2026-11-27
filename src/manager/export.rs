// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::{confirm, status};
use crate::{
    localization,
    windows::file_picker::{SaveExtension, file_types, save_file},
};
use altrun_probe::{Entry, library, state_store::TextFile};
use std::path::Path;
use windows_sys::Win32::Foundation::HWND;

pub(super) fn launch_items(owner: HWND, active: &Path, rows: &[Entry]) {
    let filter = file_types(&[("ui.import_launch_items_files", "*.tsv")]);
    let Some(path) = save_file(
        owner,
        &filter,
        "ALTRunNext-shortcuts.tsv",
        SaveExtension::Required("tsv"),
    ) else {
        return;
    };
    if path == active
        || path
            .canonicalize()
            .ok()
            .is_some_and(|p| Some(p) == active.canonicalize().ok())
    {
        status("ui.choose_a_different_backup_file_select_save", true);
        return;
    }
    let mut file = match TextFile::snapshot(&path) {
        Ok(file) => file,
        Err(error) => {
            status(&error, true);
            return;
        }
    };
    if file.existed() {
        let name = path
            .file_name()
            .unwrap_or(path.as_os_str())
            .to_string_lossy();
        let question = localization::message("ui.confirm_replace_export", &[("file", &name)]);
        if !confirm(owner, &question) {
            return;
        }
    }
    match library::serialize(rows).and_then(|text| file.save(&text)) {
        Ok(()) => status("ui.shortcut_draft_exported", false),
        Err(error) => status(&error, true),
    }
}
