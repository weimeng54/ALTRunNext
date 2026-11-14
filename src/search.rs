// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use crate::{Entry, library::EntryKind};
use std::cmp::Reverse;

pub struct SearchIndex {
    aliases: Vec<String>,
    frequencies: Vec<i64>,
    characters: Vec<Vec<char>>,
    active: Vec<bool>,
    default_order: Vec<usize>,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum MatchPriority {
    Fuzzy,
    Contains,
    Prefix,
    Exact,
}

fn ordered_span(alias: &[char], needle: &str) -> Option<(usize, usize)> {
    let mut remaining = needle.chars();
    let mut target = remaining.next()?;
    let mut first = None;
    for (index, &character) in alias.iter().enumerate() {
        if character != target {
            continue;
        }
        let start = *first.get_or_insert(index);
        let Some(next) = remaining.next() else {
            return Some((start, index + 1 - start));
        };
        target = next;
    }
    None
}

impl SearchIndex {
    pub fn query_options(&self, input: &str, anywhere: bool) -> Vec<usize> {
        if input.chars().count() > 256 {
            return Vec::new();
        }
        if input.is_empty() {
            return self.default_order.clone();
        }
        let needle = input.to_lowercase();
        let length = needle.chars().count();
        let mut hits: Vec<_> = self
            .active
            .iter()
            .enumerate()
            .filter_map(|(i, &active)| {
                if !active {
                    return None;
                }
                let (priority, start, skipped) =
                    self.literal_match(i, &needle, length, anywhere)?;
                let score = self
                    .position_score(i, start, length as i64)
                    .saturating_sub((skipped as i64).saturating_mul(64));
                Some((i, priority, score))
            })
            .collect();
        hits.sort_unstable_by(|a, b| b.1.cmp(&a.1).then(b.2.cmp(&a.2)).then(a.0.cmp(&b.0)));
        hits.into_iter().map(|hit| hit.0).collect()
    }

    pub fn same_match_priority(
        &self,
        input: &str,
        anywhere: bool,
        first: usize,
        other: usize,
    ) -> bool {
        let needle = input.to_lowercase();
        let length = needle.chars().count();
        let priority = |index| {
            self.literal_match(index, &needle, length, anywhere)
                .map(|hit| hit.0)
        };
        priority(first).is_some_and(|best| priority(other) == Some(best))
    }

    fn literal_match(
        &self,
        index: usize,
        needle: &str,
        length: usize,
        anywhere: bool,
    ) -> Option<(MatchPriority, usize, usize)> {
        let alias = self.aliases.get(index)?;
        if let Some(start) = alias.find(needle) {
            let priority = if start == 0 && alias.len() == needle.len() {
                MatchPriority::Exact
            } else if start == 0 {
                MatchPriority::Prefix
            } else if anywhere {
                MatchPriority::Contains
            } else {
                return None;
            };
            return Some((priority, alias[..start].chars().count(), 0));
        }
        let characters = &self.characters[index];
        if !anywhere || length < 2 || length > characters.len() {
            return None;
        }
        let (start, span) = ordered_span(characters, needle)?;
        Some((MatchPriority::Fuzzy, start, span - length))
    }

    fn position_score(&self, index: usize, start: usize, length: i64) -> i64 {
        1024_i64
            .saturating_add(self.frequencies[index].saturating_mul(4))
            .saturating_sub((start as i64 + 1).saturating_mul(128))
            .saturating_sub((self.characters[index].len() as i64 - length).saturating_mul(16))
    }
    pub fn new(entries: &[Entry]) -> Self {
        Self::with_frequencies(entries, |e| e.frequency)
    }

    pub fn with_frequencies(entries: &[Entry], frequency: impl Fn(&Entry) -> i64) -> Self {
        let active: Vec<_> = entries
            .iter()
            .map(|e| e.kind == EntryKind::Command)
            .collect();
        let aliases: Vec<_> = entries.iter().map(|e| e.alias.to_lowercase()).collect();
        let characters = aliases
            .iter()
            .map(|alias| alias.chars().collect())
            .collect();
        let frequencies: Vec<_> = entries.iter().map(frequency).collect();
        let mut default_order: Vec<_> = (0..entries.len()).filter(|&i| active[i]).collect();
        default_order.sort_unstable_by_key(|&i| (Reverse(frequencies[i]), i));
        Self {
            aliases,
            frequencies,
            characters,
            active,
            default_order,
        }
    }

    pub fn set_frequency(&mut self, index: usize, frequency: i64) {
        if self.frequencies[index] == frequency {
            return;
        }
        if !self.active[index] {
            self.frequencies[index] = frequency;
            return;
        }
        let key = |&i: &usize| (Reverse(self.frequencies[i]), i);
        let old = self
            .default_order
            .binary_search_by_key(&key(&index), key)
            .expect("active entry is present in default order");
        self.default_order.remove(old);
        self.frequencies[index] = frequency;
        let key = |&i: &usize| (Reverse(self.frequencies[i]), i);
        let insert = self.default_order.partition_point(|i| key(i) < key(&index));
        self.default_order.insert(insert, index);
    }

    pub fn query(&self, input: &str) -> Vec<usize> {
        self.query_options(input, true)
    }
}

pub fn search(entries: &[Entry], query: &str) -> Vec<usize> {
    SearchIndex::new(entries).query(query)
}

pub fn advance(selected: usize, length: usize, delta: isize) -> usize {
    if length == 0 {
        0
    } else {
        (selected as isize + delta).rem_euclid(length as isize) as usize
    }
}
