// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

mod dialog;

use super::{EDITOR, show_selected_in_list, status};
use crate::{localization, ui, windows::file_picker::pick_path_filtered};
use altrun_probe::{Entry, browser_bookmarks, library};
use std::collections::HashSet;
use windows_sys::Win32::Foundation::HWND;

enum Preview {
    LaunchItems {
        existing: usize,
        rows: Vec<Incoming>,
        replace: bool,
        ignored: usize,
    },
    Bookmarks {
        current: Vec<Entry>,
        rows: Vec<browser_bookmarks::PreviewRow>,
    },
}

struct Incoming {
    entry: Entry,
    conflict: &'static str,
}

struct Accepted {
    entries: Vec<Entry>,
    replace: bool,
}

impl Preview {
    fn launch(current: &[Entry], import: library::ShortcutImport) -> Self {
        let mut ids: HashSet<_> = current.iter().map(|entry| entry.id.clone()).collect();
        let mut aliases: HashSet<_> = current.iter().map(alias_key).collect();
        let rows = import
            .entries
            .into_iter()
            .map(|entry| {
                let alias = alias_key(&entry);
                let conflict = if ids.contains(&entry.id) {
                    "ui.import_conflict_id"
                } else if aliases.contains(&alias) {
                    "ui.import_conflict_alias"
                } else {
                    ids.insert(entry.id.clone());
                    aliases.insert(alias);
                    ""
                };
                Incoming { entry, conflict }
            })
            .collect();
        Self::LaunchItems {
            existing: current.len(),
            rows,
            replace: false,
            ignored: import.ignored_lines,
        }
    }
}

fn alias_key(entry: &Entry) -> String {
    entry.alias.trim().to_lowercase()
}

fn draft() -> Vec<Entry> {
    EDITOR.with(|state| state.borrow().as_ref().unwrap().draft.clone())
}

fn apply(result: Accepted) {
    let count = result.entries.len();
    if count == 0 {
        return;
    }
    EDITOR.with(|state| {
        let mut state = state.borrow_mut();
        let state = state.as_mut().unwrap();
        if result.replace {
            state.draft = result.entries;
            state.selected = 0;
        } else {
            state.selected = state.draft.len();
            state.draft.extend(result.entries);
        }
    });
    show_selected_in_list();
    let key = if result.replace {
        "ui.import_replaced_draft"
    } else {
        "ui.import_appended_draft"
    };
    status(
        &localization::message(key, &[("count", &count.to_string())]),
        false,
    );
}

pub(super) fn launch_items(owner: HWND) {
    import(owner, false);
}

pub(super) fn bookmarks(owner: HWND) {
    import(owner, true);
}

fn import(owner: HWND, bookmarks: bool) {
    let (title, picker_label, extensions) = if bookmarks {
        (
            "ui.browser_bookmarks_title",
            "ui.browser_bookmarks_html",
            "*.html;*.htm",
        )
    } else {
        (
            "ui.import_launch_items_title",
            "ui.import_launch_items_files",
            "*.tsv;*.txt",
        )
    };
    let filters = ui::wide(&format!(
        "{} ({extensions})\0{extensions}\0",
        localization::tr(picker_label)
    ));
    while let Some(path) = pick_path_filtered(owner, false, false, Some(&filters)) {
        let current = draft();
        let preview = if bookmarks {
            browser_bookmarks::read(&path).map(|items| Preview::Bookmarks {
                rows: browser_bookmarks::preview(items, &current),
                current,
            })
        } else {
            library::import_shortcuts(&path).map(|items| Preview::launch(&current, items))
        };
        let filename = path
            .file_name()
            .unwrap_or(path.as_os_str())
            .to_string_lossy();
        match preview {
            Ok(preview) => {
                match dialog::open(owner, title, &filename, Some(preview), "") {
                    dialog::Outcome::Accepted(result) => apply(result),
                    dialog::Outcome::Retry => continue,
                    dialog::Outcome::Cancelled => {}
                }
                break;
            }
            Err(error) => {
                if !matches!(
                    dialog::open(owner, title, &filename, None, &localization::tr(&error)),
                    dialog::Outcome::Retry
                ) {
                    break;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_reserves_only_importable_aliases_and_preserves_order() {
        let rows =
            altrun_probe::parse_entries("calc\tCalculator\tcalc.exe\t0\na\ta\ta.exe\t0\n").unwrap();
        let mut incoming = altrun_probe::parse_entries(
            " CALC \tReplacement\tx.exe\t0\npaint\tPaint\tmspaint.exe\t0\npaint\tPaint clone\tx.exe\t0\nterm\tTerminal\twt.exe\t0\n",
        ).unwrap();
        incoming[3].id = rows[1].id.clone();
        let Preview::LaunchItems { rows, replace, .. } = Preview::launch(
            &rows,
            library::ShortcutImport {
                entries: incoming,
                ignored_lines: 0,
            },
        ) else {
            unreachable!()
        };
        assert!(!replace);
        assert_eq!(
            rows.iter().map(|row| row.conflict).collect::<Vec<_>>(),
            [
                "ui.import_conflict_alias",
                "",
                "ui.import_conflict_alias",
                "ui.import_conflict_id"
            ]
        );
        assert_eq!(rows[1].entry.alias, "paint");
    }
}
