// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::{EntryKind, OpenTarget, ParamType, derived_id, normalize_id, validate};
use crate::Entry;

pub fn parse_entries(input: &str) -> Result<Vec<Entry>, String> {
    input
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(i, line)| {
            let fields: Vec<_> = line.split('\t').collect();
            if !(4..=11).contains(&fields.len()) || fields[0].is_empty() {
                return Err(format!("Invalid dataset row {}", i + 1));
            }
            if !matches!(fields.get(8).copied().unwrap_or("1"), "0" | "1") {
                return Err(format!("Invalid enabled flag at row {}", i + 1));
            }
            Ok(Entry {
                alias: fields[0].into(),
                name: fields[1].into(),
                command: fields[2].into(),
                frequency: fields[3]
                    .parse()
                    .map_err(|_| format!("Invalid frequency at row {}", i + 1))?,
                id: fields
                    .get(5)
                    .copied()
                    .filter(|s| !s.is_empty())
                    .map(normalize_id)
                    .unwrap_or_else(|| derived_id(fields[0], fields[1], fields[2], i)),
                working_directory: fields.get(6).unwrap_or(&"").to_string(),
                open_target: OpenTarget::parse(
                    fields.get(9).copied().unwrap_or(""),
                    fields.get(10).copied().unwrap_or(""),
                )?,
                kind: EntryKind::parse(fields.get(7).unwrap_or(&"command"))?,
                param_type: if fields.len() >= 5 {
                    ParamType::parse(fields[4])?
                } else {
                    ParamType::Auto
                },
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .and_then(|mut rows| {
            rows.retain_mut(|entry| {
                if entry.kind != EntryKind::Separator {
                    return true;
                }
                // Older releases let a separator retain the original command.
                if !entry.command.trim().is_empty() {
                    entry.kind = EntryKind::Command;
                    true
                } else if !entry.name.trim().is_empty() {
                    entry.kind = EntryKind::Remark;
                    true
                } else {
                    false
                }
            });
            for entry in &rows {
                validate(entry)?;
            }
            let mut ids = std::collections::HashSet::new();
            if rows.iter().any(|e| !ids.insert(&e.id)) {
                Err("Duplicate shortcut identity".into())
            } else {
                Ok(rows)
            }
        })
}

pub fn serialize(entries: &[Entry]) -> Result<String, String> {
    let mut output = String::new();
    for (i, e) in entries.iter().enumerate() {
        validate(e).map_err(|s| {
            crate::localization::message(
                "ui.shortcut_row_detail",
                &[
                    ("row", &(i + 1).to_string()),
                    ("detail", &crate::localization::tr(&s)),
                ],
            )
        })?;
        output.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t1",
            e.alias,
            e.name,
            e.command,
            e.frequency,
            e.param_type.name(),
            if e.id.is_empty() {
                derived_id(&e.alias, &e.name, &e.command, i)
            } else {
                normalize_id(&e.id)
            },
            e.working_directory,
            e.kind.name()
        ));
        if let Some(target) = &e.open_target {
            let (kind, program) = target.fields();
            output.push_str(&format!("\t{kind}\t{program}"));
        }
        output.push('\n');
    }
    Ok(output)
}
pub fn decode(bytes: &[u8]) -> Result<String, String> {
    if let Some(raw) = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]) {
        return String::from_utf8(raw.to_vec())
            .map_err(|_| "ui.the_utf_8_file_contains_invalid_characters".into());
    }
    if bytes.starts_with(&[0xff, 0xfe]) || bytes.starts_with(&[0xfe, 0xff]) {
        if !bytes.len().is_multiple_of(2) {
            return Err("ui.the_utf_16_file_has_an_invalid".into());
        }
        let be = bytes[0] == 0xfe;
        let text: Vec<_> = bytes[2..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|p| {
                if be {
                    u16::from_be_bytes([p[0], p[1]])
                } else {
                    u16::from_le_bytes([p[0], p[1]])
                }
            })
            .collect();
        return String::from_utf16(&text)
            .map_err(|_| "ui.the_utf_16_file_contains_invalid_characters".into());
    }
    if let Ok(text) = std::str::from_utf8(bytes) {
        return Ok(text.into());
    }
    Err("ui.use_utf_8_or_utf_16_text".into())
}
