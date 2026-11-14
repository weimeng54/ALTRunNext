// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::{SORT_ALIAS, SORT_NAME};
use altrun_probe::Entry;

pub(super) fn view_order(entries: &[Entry], sort: usize) -> Vec<usize> {
    let mut order: Vec<_> = (0..entries.len()).collect();
    match sort {
        SORT_ALIAS => order.sort_by_cached_key(|&i| entries[i].alias.to_lowercase()),
        SORT_NAME => order.sort_by_cached_key(|&i| entries[i].name.to_lowercase()),
        _ => {}
    }
    order
}

pub(super) fn index_of(entries: &[Entry], id: &str) -> Option<usize> {
    entries.iter().position(|entry| entry.id == id)
}
