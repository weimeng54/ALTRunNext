// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::{Accepted, Outcome, Preview};
use crate::localization as l10n;
use crate::manager::imports::Incoming;
use altrun_probe::browser_bookmarks::{BookmarkStatus, PreviewRow, selected_entries};
use std::borrow::Cow;

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

pub(super) struct DisplayRow {
    pub(super) cells: Vec<String>,
    pub(super) bookmark: Option<(BookmarkStatus, bool, String)>,
}

struct BookmarkFilter {
    folder: String,
    title: String,
    address: String,
    folded_folder: String,
}

impl BookmarkFilter {
    fn new(row: &PreviewRow) -> Self {
        let folder = row.bookmark.folders.join(" / ");
        Self {
            title: row.bookmark.title.to_lowercase(),
            address: row.bookmark.url.to_lowercase(),
            folded_folder: folder.to_lowercase(),
            folder,
        }
    }

    fn matches(&self, query: &str) -> bool {
        self.title.contains(query)
            || self.address.contains(query)
            || self.folded_folder.contains(query)
    }
}

pub(super) struct Model {
    pub(super) preview: Option<Preview>,
    pub(super) shown: Vec<usize>,
    pub(super) selected: Option<usize>,
    pub(super) editing_alias: Option<usize>,
    query: String,
    pub(super) updating: bool,
    pub(super) outcome: Outcome,
    bookmark_filter: Vec<BookmarkFilter>,
    ready_count: usize,
    checked_count: usize,
}

impl Model {
    pub(super) fn new(preview: Option<Preview>) -> Self {
        let (bookmark_filter, ready_count, checked_count) = match &preview {
            Some(Preview::Bookmarks { rows, .. }) => {
                let mut ready = 0;
                let mut checked = 0;
                let filter = rows
                    .iter()
                    .map(|row| {
                        if row.status == BookmarkStatus::Ready {
                            ready += 1;
                            checked += usize::from(row.checked);
                        }
                        BookmarkFilter::new(row)
                    })
                    .collect();
                (filter, ready, checked)
            }
            _ => (Vec::new(), 0, 0),
        };
        let shown = match &preview {
            Some(Preview::Bookmarks { rows, .. }) => (0..rows.len()).collect(),
            Some(Preview::LaunchItems { rows, .. }) => (0..rows.len()).collect(),
            None => Vec::new(),
        };
        let selected = shown.first().copied();
        Self {
            preview,
            bookmark_filter,
            ready_count,
            checked_count,
            shown,
            selected,
            editing_alias: None,
            query: String::new(),
            updating: true,
            outcome: Outcome::Cancelled,
        }
    }

    pub(super) fn filter(&mut self, query: String) -> bool {
        if self.query == query {
            return false;
        }
        let shown: Vec<_> = self
            .bookmark_filter
            .iter()
            .enumerate()
            .filter_map(|(index, filter)| {
                (query.is_empty() || filter.matches(&query)).then_some(index)
            })
            .collect();
        self.query = query;
        if shown == self.shown {
            return false;
        }
        self.shown = shown;
        if !self
            .selected
            .is_some_and(|row| self.shown.binary_search(&row).is_ok())
        {
            self.selected = self.shown.first().copied();
        }
        true
    }

    pub(super) fn launch_rows(&self) -> Vec<Vec<String>> {
        match self.preview.as_ref() {
            Some(Preview::LaunchItems { rows, replace, .. }) => {
                rows.iter().map(|row| launch_cells(row, *replace)).collect()
            }
            _ => Vec::new(),
        }
    }

    pub(super) fn bookmark_text(&self, position: usize, column: i32) -> Option<Cow<'_, str>> {
        let &index = self.shown.get(position)?;
        let Some(Preview::Bookmarks { rows, .. }) = self.preview.as_ref() else {
            return None;
        };
        let row = &rows[index];
        Some(match column {
            0 => check_mark(row.status, row.checked).into(),
            1 => row.bookmark.title.as_str().into(),
            2 if row.status == BookmarkStatus::Ready => row.alias.as_str().into(),
            2 => "—".into(),
            3 => self.bookmark_filter[index].folder.as_str().into(),
            4 => row.bookmark.url.as_str().into(),
            5 => l10n::tr(row.status.label()).into(),
            _ => return None,
        })
    }

    pub(super) fn toggle(&mut self, position: usize) -> Option<()> {
        let &index = self.shown.get(position)?;
        let Some(Preview::Bookmarks { rows, .. }) = self.preview.as_mut() else {
            return None;
        };
        let row = &mut rows[index];
        if row.status != BookmarkStatus::Ready {
            return None;
        }
        row.checked = !row.checked;
        if row.checked {
            self.checked_count += 1;
        } else {
            self.checked_count -= 1;
        }
        Some(())
    }

    pub(super) fn set_checks(&mut self, update: impl Fn(bool) -> bool) {
        let Some(Preview::Bookmarks { rows, .. }) = self.preview.as_mut() else {
            return;
        };
        let mut checked = 0;
        for row in rows
            .iter_mut()
            .filter(|row| row.status == BookmarkStatus::Ready)
        {
            row.checked = update(row.checked);
            checked += usize::from(row.checked);
        }
        self.checked_count = checked;
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
                Some(DisplayRow {
                    cells: vec![
                        String::new(),
                        row.bookmark.title.clone(),
                        String::new(),
                        self.bookmark_filter[index].folder.clone(),
                        row.bookmark.url.clone(),
                        l10n::tr(row.status.label()),
                    ],
                    bookmark: Some((row.status, row.checked, row.alias.clone())),
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
                let total = self.ready_count;
                let selected = self.checked_count;
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
