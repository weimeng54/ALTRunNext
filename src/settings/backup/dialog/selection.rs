// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::{
    APPEND, DETAILS, HISTORY, ITEM_INFO, ITEMS, NOTE, Options, REPLACE, SETTINGS, SUBMIT, TOTAL,
};
use crate::{
    localization::{message, tr},
    ui::{
        dialogs::input::{PreviewLabels, Report, confirm_preview},
        wide,
    },
};
use altrun_probe::backup::{Contents, RestoreItems, RestoreSelection};
use windows_sys::Win32::{
    Foundation::HWND,
    UI::{
        Controls::{BST_CHECKED, IsDlgButtonChecked},
        Input::KeyboardAndMouse::EnableWindow,
        WindowsAndMessaging::{GetDlgItem, SetWindowTextW},
    },
};

pub(super) fn read(hwnd: HWND) -> RestoreSelection {
    let checked = |id| unsafe { IsDlgButtonChecked(hwnd, id as i32) == BST_CHECKED };
    RestoreSelection {
        contents: Contents {
            items: checked(ITEMS),
            settings: checked(SETTINGS),
            history: checked(HISTORY),
        },
        items: if checked(REPLACE) {
            RestoreItems::Replace
        } else {
            RestoreItems::Append
        },
    }
}

pub(super) fn refresh(hwnd: HWND, options: Option<&Options>) {
    let selection = read(hwnd);
    let Contents {
        items,
        settings,
        history,
    } = selection.contents;
    let replace = selection.items == RestoreItems::Replace;
    let mut effective_items = items;
    let mut note = tr("backup.zip_note");
    let set = |id, text: &str| unsafe {
        SetWindowTextW(GetDlgItem(hwnd, id as i32), wide(text).as_ptr());
    };
    let enable = |id, active| unsafe {
        EnableWindow(GetDlgItem(hwnd, id as i32), i32::from(active));
    };
    if let Some(options) = options {
        let skipped = options.conflicts.len();
        let added = options.incoming - skipped;
        effective_items = items && (replace || added != 0);
        if options.incoming != 0 {
            let summary = if !items {
                message(
                    "backup.items_available",
                    &[
                        ("count", &options.incoming.to_string()),
                        ("existing", &options.current.to_string()),
                    ],
                )
            } else if replace {
                message(
                    "backup.items_replace",
                    &[
                        ("count", &options.incoming.to_string()),
                        ("existing", &options.current.to_string()),
                    ],
                )
            } else {
                message(
                    "backup.items_append",
                    &[
                        ("count", &added.to_string()),
                        ("skipped", &skipped.to_string()),
                    ],
                )
            };
            set(ITEM_INFO, &summary);
            enable(APPEND, items);
            enable(REPLACE, items);
        }
        enable(DETAILS, settings || (items && !replace && skipped != 0));
        note = if settings {
            format!(
                "{} {}",
                tr("backup.replace_settings"),
                tr("backup.restore_note")
            )
        } else {
            tr("backup.restore_note")
        };
    }
    enable(SUBMIT, effective_items || settings || history);
    set(NOTE, &note);
    let count = usize::from(items) + usize::from(settings) + usize::from(history);
    set(
        TOTAL,
        &message("backup.selected", &[("count", &count.to_string())]),
    );
}

pub(super) fn details(hwnd: HWND, opts: &Options) {
    let selection = read(hwnd);
    let mut rows = Vec::new();
    if selection.contents.settings {
        rows.extend(
            opts.media
                .iter()
                .map(|(label, value)| (tr(label), value.clone())),
        );
    }
    if selection.contents.items && selection.items == RestoreItems::Append {
        rows.extend(
            opts.conflicts
                .iter()
                .map(|(alias, reason)| (alias.clone(), tr(reason))),
        );
    }
    if !rows.is_empty() {
        confirm_preview(
            hwnd,
            "backup.details",
            Report {
                summary: tr("backup.preview_details"),
                rows,
            },
            PreviewLabels {
                columns: ["backup.content", "backup.restore_result"],
                note: if selection.contents.settings {
                    "backup.keep_integrations"
                } else {
                    ""
                },
            },
            "ui.close",
        );
    }
}
