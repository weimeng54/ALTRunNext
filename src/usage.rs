// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use std::collections::BTreeMap;
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Usage {
    pub selection: BTreeMap<String, String>,
    pub positions: BTreeMap<String, (i32, i32, i32, i32)>,
    pub counts: BTreeMap<String, u64>,
    pub recent: Vec<String>,
    pub favourites: BTreeMap<String, String>,
    pub parameters: BTreeMap<String, Vec<String>>,
}
impl Usage {
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut out = Self::default();
        for (number, line) in text.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let fields: Vec<_> = line.split('\t').collect();
            let bad = || {
                crate::localization::message(
                    "ui.launch_history_line_row_is_invalid",
                    &[("row", &(number + 1).to_string())],
                )
            };
            match fields.as_slice() {
                ["S", name, id] => {
                    out.selection.insert(unescape(name)?, entry_id(id)?);
                }
                ["W", name, x, y, width, height] => {
                    out.positions.insert(
                        unescape(name)?,
                        (
                            x.parse().map_err(|_| bad())?,
                            y.parse().map_err(|_| bad())?,
                            width.parse().map_err(|_| bad())?,
                            height.parse().map_err(|_| bad())?,
                        ),
                    );
                }
                ["F", id, count] => {
                    let count: u64 = count.parse().map_err(|_| bad())?;
                    let total = out.counts.entry(entry_id(id)?).or_default();
                    *total = (*total).max(count);
                }
                ["R", id] => {
                    let id = entry_id(id)?;
                    if !out.recent.contains(&id) {
                        out.recent.push(id);
                    }
                }
                ["P", query, id] => {
                    out.favourites.insert(unescape(query)?, entry_id(id)?);
                }
                ["H", id, value] => {
                    let values = out.parameters.entry(entry_id(id)?).or_default();
                    let value = unescape(value)?;
                    if !values.contains(&value) {
                        values.push(value);
                    }
                }
                _ => return Err(bad()),
            }
        }
        Ok(out)
    }
    pub fn text(&self) -> String {
        let mut out = String::new();
        for (name, id) in &self.selection {
            out.push_str(&format!("S\t{}\t{}\n", escape(name), escape(id)));
        }
        for (name, (x, y, w, h)) in &self.positions {
            out.push_str(&format!("W\t{}\t{x}\t{y}\t{w}\t{h}\n", escape(name)));
        }
        for (id, count) in &self.counts {
            out.push_str(&format!("F\t{}\t{count}\n", escape(id)));
        }
        for id in &self.recent {
            out.push_str(&format!("R\t{}\n", escape(id)));
        }
        for (query, id) in &self.favourites {
            out.push_str(&format!("P\t{}\t{}\n", escape(query), escape(id)));
        }
        for (id, values) in &self.parameters {
            for value in values {
                out.push_str(&format!("H\t{}\t{}\n", escape(id), escape(value)));
            }
        }
        out
    }
    pub fn record(&mut self, id: &str, query: &str, parameter: Option<&str>, limit: usize) {
        if id.is_empty() {
            return;
        }
        let count = self.counts.entry(id.into()).or_default();
        *count = count.saturating_add(1);
        self.recent.retain(|old| old != id);
        self.recent.insert(0, id.into());
        self.recent.truncate(10);
        if !query.is_empty() {
            self.favourites.insert(query.into(), id.into());
        }
        if let Some(value) = parameter {
            let values = self.parameters.entry(id.into()).or_default();
            values.retain(|old| old != value);
            values.insert(0, value.into());
            values.truncate(limit);
            let global = self.parameters.entry(String::from("*")).or_default();
            global.retain(|old| old != value);
            global.insert(0, value.into());
            global.truncate(limit);
        }
    }
}
fn entry_id(value: &str) -> Result<String, String> {
    Ok(crate::library::normalize_id(&unescape(value)?))
}
pub fn escape(value: &str) -> String {
    let mut out = String::new();
    for c in value.chars() {
        if c == '%' || (c as u32) < 32 {
            out.push_str(&format!("%{:02X}", c as u32));
        } else {
            out.push(c);
        }
    }
    out
}
pub fn unescape(value: &str) -> Result<String, String> {
    let input = value.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < input.len() {
        if input[i] == b'%' {
            if i + 2 >= input.len() {
                return Err("ui.incomplete_escape_sequence".into());
            }
            let hex = std::str::from_utf8(&input[i + 1..i + 3])
                .map_err(|_| "ui.invalid_escape_sequence")?;
            out.push(u8::from_str_radix(hex, 16).map_err(|_| "ui.invalid_escape_sequence")?);
            i += 3;
        } else {
            out.push(input[i]);
            i += 1;
        }
    }
    String::from_utf8(out).map_err(|_| "ui.the_launch_history_is_not_valid_utf".into())
}
