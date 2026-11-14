// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::{Preferences, SPECS};
use std::collections::BTreeMap;

const SETTINGS_HEADER_PREFIX: &str = "; ALTRunNext settings format ";
const SETTINGS_HEADER: &str = "; ALTRunNext settings format 1";
pub const SETTINGS_IMPORT_FORMAT_ERROR: &str = "ui.choose_an_ini_settings_file_exported_by";

impl Preferences {
    pub fn import(text: &str) -> Result<Self, String> {
        if text.len() > 1024 * 1024 {
            return Err("ui.the_settings_file_is_too_large".into());
        }
        let text = text.trim_start_matches('\u{feff}');
        let mut section = "";
        let mut current_format = false;
        let mut previous_format = false;
        for line in text.lines().map(str::trim) {
            if let Some(version) = line
                .strip_prefix(SETTINGS_HEADER_PREFIX)
                .or_else(|| line.strip_prefix("; ALTRun settings format "))
            {
                if version != "1" {
                    return Err("ui.this_settings_file_version_is_unsupported".into());
                }
                current_format = true;
            }
            if let Some(name) = line
                .strip_prefix('[')
                .and_then(|line| line.strip_suffix(']'))
            {
                section = name.trim();
                continue;
            }
            let Some((key, _)) = line.split_once('=') else {
                continue;
            };
            // Exports made before the format marker always included these ALTRunNext
            // settings, which were absent from the original application's INI.
            let keys: &[&str] = if section.eq_ignore_ascii_case("GUI") {
                &[
                    "Theme",
                    "PositionMode",
                    "BGCrop",
                    "BGStrength",
                    "RowHeight",
                    "MaxRows",
                ]
            } else if section.eq_ignore_ascii_case("Config") {
                &["EnglishSearch", "AllowDynamicTarget", "UseCustomTextColor"]
            } else {
                &[]
            };
            previous_format |= keys
                .iter()
                .any(|name| name.eq_ignore_ascii_case(key.trim()));
        }
        if !current_format && !previous_format {
            return Err(SETTINGS_IMPORT_FORMAT_ERROR.into());
        }
        Self::parse(text)
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        if text.len() > 1024 * 1024 {
            return Err("ui.the_settings_file_is_too_large".into());
        }
        let mut result = Self::default();
        let mut section = String::from("Config");
        let mut custom_text_color = false;
        for (i, line) in text.trim_start_matches('\u{feff}').lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with([';', '#']) {
                continue;
            }
            if line.starts_with('[') && line.ends_with(']') {
                section = line[1..line.len() - 1].trim().to_owned();
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                return Err(crate::localization::message(
                    "ui.settings_line_row_is_missing_an_equals",
                    &[("row", &(i + 1).to_string())],
                ));
            };
            if [
                "AlphaColor",
                "IndexFrom0to9",
                "ExitWhenExecute",
                "ExecuteIfOnlyOne",
                "Regex",
                "StandardRegex",
                "ShowStartNotification",
                "ShowTopTen",
                "ShowShortCutButton",
                "ShowConfigButton",
                "ShowCloseButton",
                "Alpha",
                "RoundBorderRadius",
                "WinTop",
                "WinLeft",
                "ManWinTop",
                "ManWinLeft",
                "ManWinWidth",
                "ManWinHeight",
                "ListFormat",
            ]
            .iter()
            .any(|retired| key.trim().eq_ignore_ascii_case(retired))
            {
                continue;
            }
            if SPECS.iter().any(|s| s.key.eq_ignore_ascii_case(key.trim())) {
                custom_text_color |= key.trim().eq_ignore_ascii_case("UseCustomTextColor");
                result.set(key.trim(), value.trim())?;
            } else {
                result
                    .values
                    .insert(format!("@{}.{}", section, key.trim()), value.trim().into());
            }
        }
        if !custom_text_color && result.get("TextColor") != Self::default().get("TextColor") {
            result.set("UseCustomTextColor", "1")?;
        }
        Ok(result)
    }
    pub fn text(&self) -> String {
        let mut groups: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
        for (key, value) in &self.values {
            if let Some(extra) = key.strip_prefix('@') {
                let (section, name) = extra.split_once('.').unwrap_or(("Extra", extra));
                groups
                    .entry(section.into())
                    .or_default()
                    .push((name.into(), value.clone()));
            } else {
                let section = if [
                    "FormWidth",
                    "BGFileName",
                    "BGCrop",
                    "BGStrength",
                    "Lang",
                    "Theme",
                    "RowHeight",
                    "MaxRows",
                    "PositionMode",
                ]
                .contains(&key.as_str())
                    || key.ends_with("Font")
                    || key.ends_with("Color")
                {
                    "GUI"
                } else if key.starts_with("Win") || key.starts_with("ManWin") {
                    "Window"
                } else {
                    "Config"
                };
                groups
                    .entry(section.into())
                    .or_default()
                    .push((key.clone(), value.clone()));
            }
        }
        let mut out = format!("{SETTINGS_HEADER}\n");
        for (section, values) in groups {
            out.push_str(&format!("[{section}]\n"));
            for (key, value) in values {
                out.push_str(&format!("{key}={value}\n"));
            }
            out.push('\n');
        }
        out
    }
}
