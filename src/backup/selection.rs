// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::{Backup, entries, invalid};
use crate::{Entry, library, usage::Usage};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
pub struct Contents {
    pub items: bool,
    pub settings: bool,
    pub history: bool,
}

impl Contents {
    pub const ALL: Self = Self {
        items: true,
        settings: true,
        history: true,
    };

    pub fn any(self) -> bool {
        self.items || self.settings || self.history
    }

    pub fn contains(self, selection: Self) -> bool {
        (!selection.items || self.items)
            && (!selection.settings || self.settings)
            && (!selection.history || self.history)
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum RestoreItems {
    #[default]
    Append,
    Replace,
}

#[derive(Clone, Copy)]
pub struct RestoreSelection {
    pub contents: Contents,
    pub items: RestoreItems,
}

impl RestoreSelection {
    pub(super) fn prepare(
        self,
        backup: &Backup,
        expected: &[Entry],
        current_items: &[u8],
        current_history: &[u8],
    ) -> Result<[Option<Vec<u8>>; 2], String> {
        if !self.contents.any() || !backup.contents().contains(self.contents) {
            return Err(invalid());
        }
        let mut final_items = if self.contents.items || self.contents.history {
            let live = entries(current_items)?;
            if live != expected {
                return Err("backup.changed_since_prepare".into());
            }
            live
        } else {
            Vec::new()
        };
        let mut items = None;
        let replace = self.contents.items && self.items == RestoreItems::Replace;
        if self.contents.items {
            let incoming = backup.launch_items()?;
            if incoming.is_empty() {
                return Err(invalid());
            }
            if replace {
                items = Some(backup.shortcuts.clone());
                final_items = incoming;
            } else {
                let conflicts = library::append_conflicts(&final_items, &incoming);
                final_items.extend(
                    incoming
                        .into_iter()
                        .zip(conflicts)
                        .filter_map(|(entry, reason)| reason.is_empty().then_some(entry)),
                );
                if final_items.len() != expected.len() {
                    items = Some(library::serialize(&final_items)?.into_bytes());
                }
            }
        }
        let history = if self.contents.history {
            let previous = Usage::parse(&library::decode(current_history)?)?;
            let saved = Usage::parse(&library::decode(&backup.history)?)?;
            Some(
                merge_history(previous, saved, &final_items, replace)
                    .text()
                    .into_bytes(),
            )
        } else {
            None
        };
        if items.is_none() && history.is_none() && !self.contents.settings {
            return Err(invalid());
        }
        Ok([items, history])
    }
}

fn merge_history(mut current: Usage, mut saved: Usage, entries: &[Entry], replace: bool) -> Usage {
    let ids: HashSet<_> = entries.iter().map(|entry| entry.id.as_str()).collect();
    let mut global = saved.parameters.remove("*").unwrap_or_default();
    saved.parameters.retain(|id, _| ids.contains(id.as_str()));
    if replace {
        current.counts.clear();
        current.parameters.clear();
        current.favourites.clear();
        current.recent.clear();
        let matching: HashSet<_> = saved.parameters.values().flatten().collect();
        global.retain(|value| matching.contains(value));
        if !global.is_empty() {
            current.parameters.insert("*".into(), global);
        }
    }
    current.counts.extend(
        saved
            .counts
            .into_iter()
            .filter(|(id, _)| ids.contains(id.as_str())),
    );
    current.parameters.extend(saved.parameters);
    current.favourites.extend(
        saved
            .favourites
            .into_iter()
            .filter(|(_, id)| ids.contains(id.as_str())),
    );
    let mut seen = HashSet::new();
    current.recent = saved
        .recent
        .into_iter()
        .filter(|id| ids.contains(id.as_str()))
        .chain(current.recent)
        .filter(|id| seen.insert(id.clone()))
        .take(10)
        .collect();
    current
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_uses_stable_ids_and_keeps_local_window_state() {
        let entries = crate::parse_entries(
            "calc\tCalculator\tcalc.exe\t0\tnone\tcalc-id\nnp\tNotepad\tnotepad.exe\t0\tnone\tnp-id",
        ).unwrap();
        let mut local = Usage::default();
        local.record("calc-id", "cl", None, 50);
        local.positions.insert("launcher".into(), (10, 20, 30, 40));
        let mut archived = Usage::default();
        archived.record("other-id", "cl", Some("skipped"), 50);
        archived.record("np-id", "np", Some("Chinese document.txt"), 50);
        archived
            .positions
            .insert("launcher".into(), (400, 800, 600, 300));

        let merged = merge_history(local.clone(), archived.clone(), &entries, false);
        assert_eq!(merged.counts.len(), 2);
        assert_eq!(merged.recent, ["np-id", "calc-id"]);
        assert_eq!(merged.favourites["cl"], "calc-id");
        assert_eq!(merged.parameters["np-id"], ["Chinese document.txt"]);
        assert!(!merged.parameters.contains_key("other-id"));
        assert!(!merged.parameters.contains_key("*"));
        assert_eq!(merged.positions, local.positions);

        let replaced = merge_history(local.clone(), archived, &entries[1..], true);
        assert_eq!(replaced.recent, ["np-id"]);
        assert_eq!(replaced.counts.len(), 1);
        assert_eq!(replaced.parameters["*"], ["Chinese document.txt"]);
        assert_eq!(replaced.positions, local.positions);
    }
}
