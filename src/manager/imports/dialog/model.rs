// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::{ALIAS_COLUMN, Accepted, Outcome, Preview};
use crate::localization as l10n;
use crate::manager::imports::Incoming;
use altrun_probe::browser_bookmarks::{BookmarkStatus, PreviewRow, selected_entries};

pub(super) fn check_mark(status: BookmarkStatus, checked: bool) -> &'static str {
    match (status, checked) {
        (BookmarkStatus::Ready, true) => "☑",
        (BookmarkStatus::Ready, false) => "☐",
        _ => "—",
    }
}

fn launch_cells(row: &Incoming, replace: bool) -> Vec<String> {
    vec![
        row.entry.alias.clone(),
        row.entry.name.clone(),
        row.entry.command.clone(),
        l10n::tr(if !replace && !row.conflict.is_empty() {
            row.conflict
        } else {
            "ui.import_ready"
        }),
    ]
}

fn bookmark_cells(row: &PreviewRow, folder: String) -> Vec<String> {
    vec![
        check_mark(row.status, row.checked).into(),
        row.bookmark.title.clone(),
        row.alias.clone(),
        folder,
        row.bookmark.url.clone(),
        l10n::tr(row.status.label()),
    ]
}

pub(super) struct DisplayRow {
    pub(super) cells: Vec<String>,
    pub(super) bookmark: Option<(BookmarkStatus, bool, String)>,
}

pub(super) struct Model {
    pub(super) preview: Option<Preview>,
    pub(super) shown: Vec<usize>,
    pub(super) selected: Option<usize>,
    pub(super) editing_alias: Option<usize>,
    pub(super) query: String,
    pub(super) updating: bool,
    pub(super) outcome: Outcome,
}

impl Model {
    pub(super) fn visible_rows(&mut self) -> Vec<Vec<String>> {
        let mut shown = Vec::new();
        let table = match self.preview.as_ref() {
            Some(Preview::LaunchItems { rows, replace, .. }) => {
                shown.extend(0..rows.len());
                rows.iter().map(|row| launch_cells(row, *replace)).collect()
            }
            Some(Preview::Bookmarks { rows, .. }) => rows
                .iter()
                .enumerate()
                .filter_map(|(index, row)| {
                    let folder = row.bookmark.folders.join(" / ");
                    let matches = [&row.bookmark.title, &row.bookmark.url, &folder]
                        .iter()
                        .any(|text| text.to_lowercase().contains(&self.query));
                    if !matches {
                        return None;
                    }
                    shown.push(index);
                    Some(bookmark_cells(row, folder))
                })
                .collect(),
            None => Vec::new(),
        };
        self.shown = shown;
        if !self.selected.is_some_and(|row| self.shown.contains(&row)) {
            self.selected = self.shown.first().copied();
        }
        table
    }

    pub(super) fn display_row(&self, position: usize) -> Option<DisplayRow> {
        let &index = self.shown.get(position)?;
        match self.preview.as_ref()? {
            Preview::LaunchItems { rows, replace, .. } => Some(DisplayRow {
                cells: launch_cells(&rows[index], *replace),
                bookmark: None,
            }),
            Preview::Bookmarks { rows, .. } => {
                let row = &rows[index];
                let mut cells = bookmark_cells(row, row.bookmark.folders.join(" / "));
                cells[0].clear();
                let alias = std::mem::take(&mut cells[ALIAS_COLUMN]);
                Some(DisplayRow {
                    cells,
                    bookmark: Some((row.status, row.checked, alias)),
                })
            }
        }
    }

    pub(super) fn status(&self) -> (String, String, bool) {
        let label = l10n::tr("ui.choose_again");
        match self.preview.as_ref() {
            Some(Preview::LaunchItems {
                existing,
                rows,
                replace,
                ignored,
            }) => {
                let included = rows
                    .iter()
                    .filter(|row| *replace || row.conflict.is_empty())
                    .count();
                let added = included.to_string();
                let skipped = (rows.len() - included + ignored).to_string();
                let (key, args, label) = if *replace {
                    (
                        "ui.import_replace_summary",
                        vec![
                            ("existing", existing.to_string()),
                            ("count", added),
                            ("skipped", skipped),
                        ],
                        "ui.import_replace_draft",
                    )
                } else {
                    (
                        "ui.import_append_summary",
                        vec![("count", added), ("skipped", skipped)],
                        "ui.import_append_draft",
                    )
                };
                let args = args
                    .iter()
                    .map(|(label, value)| (*label, value.as_str()))
                    .collect::<Vec<_>>();
                (l10n::message(key, &args), l10n::tr(label), included > 0)
            }
            Some(Preview::Bookmarks { rows, .. }) => {
                let total = rows
                    .iter()
                    .filter(|row| row.status == BookmarkStatus::Ready)
                    .count();
                let selected = rows
                    .iter()
                    .filter(|row| row.checked && row.status == BookmarkStatus::Ready)
                    .count();
                let selected_text = selected.to_string();
                (
                    l10n::message(
                        "ui.browser_bookmarks_selected",
                        &[
                            ("selected", &selected_text),
                            ("total", &total.to_string()),
                            ("skipped", &(rows.len() - total).to_string()),
                        ],
                    ),
                    l10n::message("ui.browser_bookmarks_confirm", &[("count", &selected_text)]),
                    selected > 0,
                )
            }
            None => (String::new(), label, true),
        }
    }

    pub(super) fn accept(&self) -> Result<Option<Accepted>, String> {
        match self.preview.as_ref() {
            Some(Preview::LaunchItems { rows, replace, .. }) => {
                let entries: Vec<_> = rows
                    .iter()
                    .filter(|row| *replace || row.conflict.is_empty())
                    .map(|row| row.entry.clone())
                    .collect();
                Ok((!entries.is_empty()).then_some(Accepted {
                    entries,
                    replace: *replace,
                }))
            }
            Some(Preview::Bookmarks { current, rows }) => {
                let entries = selected_entries(rows, current)?;
                Ok((!entries.is_empty()).then_some(Accepted {
                    entries,
                    replace: false,
                }))
            }
            None => Ok(None),
        }
    }
}
