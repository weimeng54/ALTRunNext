// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::{SORT_ALIAS, SORT_NAME};
use altrun_probe::Entry;

pub(super) fn view_order(entries: &[Entry], sort: usize, filter: &str) -> Vec<usize> {
    let query = filter.trim().to_lowercase();
    let mut order: Vec<_> = entries
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| matches_filter(entry, &query).then_some(index))
        .collect();
    match sort {
        SORT_ALIAS => order.sort_by_cached_key(|&i| entries[i].alias.to_lowercase()),
        SORT_NAME => order.sort_by_cached_key(|&i| entries[i].name.to_lowercase()),
        _ => {}
    }
    order
}

/// The filter has already been trimmed and folded to lowercase by the caller.
pub(super) fn matches_filter(entry: &Entry, query: &str) -> bool {
    query.is_empty()
        || [&entry.alias, &entry.name, &entry.command]
            .into_iter()
            .any(|field| field.to_lowercase().contains(query))
}

pub(super) fn index_of(entries: &[Entry], id: &str) -> Option<usize> {
    entries.iter().position(|entry| entry.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filtering_keeps_stable_indices_and_respects_sort_order() {
        let entries = altrun_probe::parse_entries(
            "z\t工具\tD:\\Portable\\editor.exe\t0\n\
             calc\t计算器\tcalc.exe\t0\n\
             a\tPORTABLE notes\tnotepad.exe\t0",
        )
        .unwrap();
        assert_eq!(view_order(&entries, 0, "  PORTABLE "), [0, 2]);
        assert_eq!(view_order(&entries, SORT_ALIAS, "portable"), [2, 0]);
        assert_eq!(view_order(&entries, 0, "计算"), [1]);
        assert!(view_order(&entries, 0, "not-here").is_empty());
        assert_eq!(view_order(&entries, 0, ""), [0, 1, 2]);
    }
}
