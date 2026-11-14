// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::*;

pub(super) fn apply_intake(hwnd: HWND) {
    if crate::shortcut_intake::is_open() {
        return;
    }
    loop {
        let paths = INTAKE
            .lock()
            .map(|mut p| std::mem::take(&mut *p))
            .unwrap_or_default();
        if paths.is_empty() {
            break;
        }
        let settings_path = EDITOR.with(|c| {
            c.borrow()
                .as_ref()
                .unwrap()
                .file
                .path
                .with_extension("settings.ini")
        });
        if let Ok((_, text)) = altrun_probe::state_store::TextFile::open(&settings_path)
            && let Ok(prefs) = altrun_probe::preferences::Preferences::parse(&text)
        {
            theme::install_chrome(&prefs);
            theme::frame(hwnd);
            let list = EDITOR.with(|c| c.borrow().as_ref().unwrap().list);
            manager_table::set_background(list);
            unsafe {
                RedrawWindow(
                    hwnd,
                    std::ptr::null(),
                    null_mut(),
                    RDW_INVALIDATE | RDW_ALLCHILDREN | RDW_FRAME,
                );
            }
        }
        match paths
            .iter()
            .map(|path| altrun_probe::management::from_path(path))
            .collect::<Result<Vec<_>, _>>()
        {
            Ok(entries) => {
                let launch_enabled = EDITOR.with(|c| c.borrow().as_ref().unwrap().launch_enabled);
                crate::shortcut_intake::open(hwnd, entries, launch_enabled, save_intake);
            }
            Err(error) => crate::native::show_error(&error),
        }
    }
    let action = ACTION.lock().ok().and_then(|mut p| p.take());
    if let Some((id, action)) = action {
        let found = EDITOR.with(|c| {
            let mut s = c.borrow_mut();
            let s = s.as_mut().unwrap();
            if let Some(index) = s.draft.iter().position(|e| e.id == id) {
                s.selected = index;
                true
            } else {
                false
            }
        });
        if action != VK_INSERT && !found {
            status("ui.launch_item_no_longer_in_draft", true);
            return;
        }
        refresh_list();
        unsafe {
            if action == VK_INSERT {
                SendMessageW(hwnd, WM_COMMAND, ADD, 0);
                if !id.is_empty() {
                    let alias = EDITOR.with(|c| c.borrow().as_ref().unwrap().alias);
                    SetWindowTextW(alias, wide(&id).as_ptr());
                }
            } else if action == VK_DELETE {
                SendMessageW(hwnd, WM_COMMAND, DELETE, 0);
            } else {
                let field = EDITOR.with(|c| c.borrow().as_ref().unwrap().command);
                SetFocus(field);
            }
        }
    }
}

pub(super) fn save_intake(additions: &[Entry]) -> Result<(), (usize, String)> {
    let (sender, committed, parent) = EDITOR.with(|c| {
        let mut editor = c.borrow_mut();
        let s = editor.as_mut().unwrap();
        let mut aliases = s
            .file
            .entries
            .iter()
            .chain(&s.draft)
            .filter(|e| e.kind == library::EntryKind::Command)
            .map(|e| e.alias.trim().to_lowercase())
            .collect::<std::collections::HashSet<_>>();
        for (index, entry) in additions.iter().enumerate() {
            if entry.alias.trim().is_empty() {
                return Err((index, "ui.enter_a_shortcut_keyword".into()));
            }
            if !aliases.insert(entry.alias.trim().to_lowercase()) {
                return Err((
                    index,
                    "ui.this_shortcut_keyword_is_taken_choose_another".into(),
                ));
            }
        }
        let mut committed = s.file.entries.clone();
        committed.extend_from_slice(additions);
        s.file.save(&committed).map_err(|error| (0, error))?;
        s.selected = s.draft.len();
        s.draft.extend_from_slice(additions);
        Ok((s.sender.clone(), committed, s.parent))
    })?;
    sender.saved(committed);
    unsafe {
        PostMessageW(parent as HWND, UPDATED, 0, 0);
    }
    refresh_list();
    let dirty = EDITOR.with(|c| c.borrow().as_ref().unwrap().is_dirty());
    status(
        if dirty {
            "ui.shortcut_added_the_manager_still_has_unsaved"
        } else {
            "ui.shortcut_added"
        },
        false,
    );
    Ok(())
}

pub(super) fn add_paths(paths: Vec<PathBuf>) {
    let mut entries = Vec::new();
    for path in paths {
        match altrun_probe::management::from_path(&path) {
            Ok(e) => entries.push(e),
            Err(e) => {
                status(&e, true);
                return;
            }
        }
    }
    if entries.is_empty() {
        return;
    }
    EDITOR.with(|c| {
        let mut s = c.borrow_mut();
        let s = s.as_mut().unwrap();
        s.selected = s.draft.len();
        s.draft.extend(entries);
    });
    refresh_list();
    status("ui.added_to_the_draft_edit_the_alias", false);
}
