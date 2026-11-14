// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::{EntryKind, ParamType, decode, derived_id, validate};
use crate::Entry;
use std::path::Path;

pub struct ShortcutImport {
    pub entries: Vec<Entry>,
    pub ignored_lines: usize,
}

pub fn import_shortcuts(path: &Path) -> Result<ShortcutImport, String> {
    use std::io::Read;
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .and_then(|file| file.take(16 * 1024 * 1024 + 1).read_to_end(&mut bytes))
        .map_err(|error| {
            crate::localization::message(
                "ui.could_not_read_shortcuts_error",
                &[("error", &error.to_string())],
            )
        })?;
    decode_shortcut_import(&bytes)
}

pub fn decode_shortcut_import(bytes: &[u8]) -> Result<ShortcutImport, String> {
    if bytes.len() > 16 * 1024 * 1024 {
        return Err("ui.the_shortcuts_file_exceeds_16_mib".into());
    }
    let text = match decode(bytes) {
        Ok(text) => text,
        Err(error)
            if bytes.contains(&b'\t')
                || bytes.starts_with(&[0xef, 0xbb, 0xbf])
                || bytes.starts_with(&[0xff, 0xfe])
                || bytes.starts_with(&[0xfe, 0xff]) =>
        {
            return Err(error);
        }
        Err(_) => crate::textcodec::decode_gb18030(bytes)?,
    };
    let import = if text
        .lines()
        .any(|line| !line.trim().is_empty() && line.contains('\t'))
    {
        ShortcutImport {
            entries: crate::parse_entries(&text)?,
            ignored_lines: 0,
        }
    } else {
        parse_original_shortcuts(&text)?
    };
    if !import
        .entries
        .iter()
        .any(|entry| entry.kind == EntryKind::Command)
    {
        return Err("ui.there_are_no_shortcuts_to_import".into());
    }
    Ok(import)
}

fn parse_original_shortcuts(text: &str) -> Result<ShortcutImport, String> {
    let mut entries = Vec::new();
    let mut ignored_lines = 0;
    for (row, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.contains('\0') {
            return Err(crate::localization::message(
                "ui.line_row_contains_invalid_characters",
                &[("row", &(row + 1).to_string())],
            ));
        }
        if line.starts_with('[') && line.ends_with(']') {
            return Err("ui.choose_a_shortcuts_file_import_settings_from".into());
        }
        if line.starts_with([';', '#']) {
            ignored_lines += 1;
            continue;
        }
        let delimiter = if line.starts_with('F') && line.contains('|') {
            Some('|')
        } else {
            line.chars().find(|c| matches!(c, '|' | ',' | '，'))
        };
        let Some(delimiter) = delimiter else {
            ignored_lines += 1;
            continue;
        };
        let invalid = || {
            crate::localization::message(
                "ui.the_shortcut_on_line_row_has_an",
                &[("row", &(row + 1).to_string())],
            )
        };
        let (first, rest) = line.split_once(delimiter).ok_or_else(invalid)?;
        let first = first.trim();
        let header = first.strip_prefix('F').filter(|digits| {
            !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit())
        });
        let mut frequency = 0;
        let (first, rest) = if let Some(digits) = header {
            frequency = if digits.len() > 8 {
                10000
            } else {
                digits.parse().map_err(|_| invalid())?
            };
            rest.split_once(delimiter).ok_or_else(invalid)?
        } else {
            (first, rest)
        };
        let (alias, rest, param_type) = match first.trim().to_ascii_lowercase().as_str() {
            kind @ ("" | "no_encoding" | "url_query" | "utf8_query") => {
                let (alias, tail) = rest.split_once(delimiter).ok_or_else(invalid)?;
                let kind = match kind {
                    "no_encoding" => ParamType::Raw,
                    kind => ParamType::parse(kind)?,
                };
                (alias, tail, kind)
            }
            _ => (first, rest, ParamType::None),
        };
        let (name, command) = rest.split_once(delimiter).ok_or_else(invalid)?;
        let (alias, name, command) = (alias.trim(), name.trim(), command.trim());
        let entry = Entry {
            alias: alias.into(),
            name: name.into(),
            command: crate::launch::normalize_original_command(command),
            frequency,
            param_type,
            id: derived_id(alias, name, command, row),
            working_directory: String::new(),
            open_target: None,
            kind: EntryKind::Command,
        };
        validate(&entry).map_err(|error| {
            crate::localization::message(
                "ui.line_row_detail",
                &[
                    ("row", &(row + 1).to_string()),
                    ("detail", &crate::localization::tr(&error)),
                ],
            )
        })?;
        entries.push(entry);
    }
    Ok(ShortcutImport {
        entries,
        ignored_lines,
    })
}
