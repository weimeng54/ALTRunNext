// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

mod ini;
mod schema;

pub use ini::SETTINGS_IMPORT_FORMAT_ERROR;
pub use schema::{DEFAULT_POPUP_SOUND, Kind, SPECS, Spec};

use std::collections::BTreeMap;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Preferences {
    pub values: BTreeMap<String, String>,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            values: SPECS
                .iter()
                .map(|s| (s.key.to_owned(), s.default.to_owned()))
                .collect(),
        }
    }
}
impl Preferences {
    pub fn get(&self, key: &str) -> String {
        self.values
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v.clone())
            .unwrap_or_default()
    }
    pub fn flag(&self, key: &str) -> bool {
        self.get(key) == "1"
    }
    pub fn number(&self, key: &str) -> i32 {
        self.get(key).parse().unwrap_or_default()
    }
    pub fn set(&mut self, key: &str, value: &str) -> Result<(), String> {
        if value.contains(['\0', '\r', '\n']) {
            return Err("ui.a_setting_cannot_contain_line_breaks_or".into());
        }
        let spec = SPECS
            .iter()
            .find(|s| s.key.eq_ignore_ascii_case(key))
            .ok_or_else(|| {
                crate::localization::message("ui.unknown_setting_key", &[("key", key)])
            })?;
        let value = match value.trim() {
            "" if spec.key == "PopupFileName" => DEFAULT_POPUP_SOUND,
            "mouse" if spec.key == "PositionMode" => "center",
            value => value,
        };
        let validated = match spec.kind {
            Kind::Bool => match value.to_ascii_lowercase().as_str() {
                "1" | "true" | "yes" => "1".to_owned(),
                "0" | "false" | "no" => "0".to_owned(),
                _ => {
                    return Err(crate::localization::message(
                        "ui.setting_must_be_0_or_1",
                        &[("setting", &crate::localization::tr(spec.label))],
                    ));
                }
            },
            Kind::Number(low, high) => {
                let n: i64 = value.parse().map_err(|_| {
                    crate::localization::message(
                        "ui.setting_must_be_an_integer",
                        &[("setting", &crate::localization::tr(spec.label))],
                    )
                })?;
                if !(low..=high).contains(&n) {
                    return Err(crate::localization::message(
                        "ui.setting_must_be_between_low_and_high",
                        &[
                            ("setting", &crate::localization::tr(spec.label)),
                            ("low", &low.to_string()),
                            ("high", &high.to_string()),
                        ],
                    ));
                }
                n.to_string()
            }
            Kind::Hotkey => {
                parse_hotkey(value)?;
                value.to_owned()
            }
            Kind::Font => {
                Font::parse(value)?;
                value.to_owned()
            }
            Kind::Text => {
                if spec.key == "Lang" {
                    let language = match value {
                        "简体中文" => "zh-Hans".into(),
                        value if value.eq_ignore_ascii_case("English") => "en".into(),
                        "" => "system".into(),
                        value => crate::language_pack::language_tag(value)
                            .ok_or("ui.enter_a_valid_language_code")?,
                    };
                    self.values.insert(spec.key.into(), language);
                    return Ok(());
                }
                if spec.key == "Theme" && !matches!(value, "light" | "dark" | "system") {
                    return Err("ui.choose_a_light_dark_or_system_theme".into());
                }
                if spec.key == "PositionMode" && !matches!(value, "remember" | "center") {
                    return Err("ui.select_a_valid_window_position".into());
                }
                if spec.key == "BGCrop" && !matches!(value, "top" | "center" | "bottom") {
                    return Err("ui.select_a_valid_crop_alignment".into());
                }
                if spec.key.ends_with("Color") {
                    parse_colour(value)?;
                }
                value.to_owned()
            }
        };
        self.values.insert(spec.key.into(), validated);
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hotkey {
    pub modifiers: u32,
    pub key: u32,
}
pub fn parse_hotkey(value: &str) -> Result<Option<Hotkey>, String> {
    let value = value.trim();
    if value.is_empty() || value.eq_ignore_ascii_case("none") {
        return Ok(None);
    }
    let mut modifiers = 0;
    let mut key = None;
    for part in value.split('+').map(str::trim) {
        let upper = part.to_ascii_uppercase();
        match upper.as_str() {
            "CTRL" | "CONTROL" => modifiers |= 2,
            "ALT" => modifiers |= 1,
            "SHIFT" => modifiers |= 4,
            "WIN" | "WINDOWS" => modifiers |= 8,
            other => {
                let code = match other {
                    "PAUSE" => 0x13,
                    "SPACE" => 0x20,
                    "TAB" => 9,
                    "ENTER" => 13,
                    "ESC" | "ESCAPE" => 27,
                    "INSERT" => 0x2d,
                    "DELETE" => 0x2e,
                    "HOME" => 0x24,
                    "END" => 0x23,
                    "PAGEUP" => 0x21,
                    "PAGEDOWN" => 0x22,
                    _ if other.starts_with('F') && other.len() <= 3 => {
                        let n: u32 = other[1..].parse().map_err(|_| "ui.invalid_function_key")?;
                        if !(1..=24).contains(&n) {
                            return Err("ui.choose_a_function_key_from_f1_to".into());
                        }
                        0x6f + n
                    }
                    _ if other.len() == 1 && other.as_bytes()[0].is_ascii_alphanumeric() => {
                        other.as_bytes()[0] as u32
                    }
                    _ => {
                        return Err(crate::localization::message(
                            "ui.unknown_shortcut_key_part",
                            &[("part", part)],
                        ));
                    }
                };
                if key.replace(code).is_some() {
                    return Err("ui.a_shortcut_can_have_only_one_main".into());
                }
            }
        }
    }
    let key = key.ok_or("ui.the_shortcut_is_missing_a_main_key")?;
    Ok(Some(Hotkey { modifiers, key }))
}
#[derive(Clone, Debug)]
pub struct Font {
    pub strike_out: bool,
    pub charset: u8,
    pub name: String,
    pub points: i32,
    pub weight: i32,
    pub italic: bool,
    pub underline: bool,
    pub colour: u32,
    dip: Option<i32>,
}
impl Font {
    pub fn size_dip(&self) -> i32 {
        self.dip.unwrap_or(self.points * 4 / 3)
    }

    pub fn resized(value: &str, size: &str) -> Result<String, String> {
        let font = Self::parse(value)?;
        let size = size
            .trim()
            .parse::<i32>()
            .map_err(|_| "ui.invalid_font_size")?;
        if size == font.size_dip() {
            return Ok(value.to_owned());
        }
        if !(6..=128).contains(&size) {
            return Err("ui.enter_a_text_size_between_6_and".into());
        }
        let mut parts = value.split('|').map(str::to_owned).collect::<Vec<_>>();
        while parts.len() < 5 {
            parts.push(if parts.len() == 3 {
                font.colour.to_string()
            } else {
                font.charset.to_string()
            });
        }
        parts[2] = ((size * 3 + 2) / 4).clamp(5, 96).to_string();
        let logical_size = format!("dip={size}");
        if let Some(value) = parts
            .iter_mut()
            .skip(5)
            .find(|part| part.starts_with("dip="))
        {
            *value = logical_size;
        } else {
            parts.push(logical_size);
        }
        Ok(parts.join("|"))
    }

    pub fn parse(value: &str) -> Result<Self, String> {
        let p: Vec<_> = value.split('|').collect();
        if p.len() < 3 || p[0].trim().is_empty() {
            return Err("ui.font_format_name_style_size_color_character".into());
        }
        let points: i32 = p[2].trim().parse().map_err(|_| "ui.invalid_font_size")?;
        if !(5..=96).contains(&points) {
            return Err("ui.choose_a_font_size_from_5_to".into());
        }
        let colour = parse_colour(p.get(3).copied().unwrap_or("3812384"))?;
        let dip = p
            .iter()
            .skip(5)
            .find_map(|part| part.strip_prefix("dip="))
            .map(|value| {
                value
                    .parse::<i32>()
                    .ok()
                    .filter(|size| (6..=128).contains(size))
                    .ok_or("ui.invalid_font_size")
            })
            .transpose()?;
        Ok(Self {
            strike_out: p[1].contains("fsStrikeOut"),
            charset: p
                .get(4)
                .copied()
                .unwrap_or("1")
                .parse()
                .map_err(|_| "ui.invalid_font_character_set")?,
            name: p[0].trim().into(),
            points,
            weight: if p[1].contains("fsBold") { 600 } else { 400 },
            italic: p[1].contains("fsItalic"),
            underline: p[1].contains("fsUnderline"),
            colour,
            dip,
        })
    }
}
pub fn parse_colour(value: &str) -> Result<u32, String> {
    if let Some(hex) = value.strip_prefix('#') {
        if hex.len() != 6 {
            return Err("ui.enter_a_color_in_rrggbb_format".into());
        }
        let n = u32::from_str_radix(hex, 16).map_err(|_| "ui.invalid_color_format")?;
        Ok(((n >> 16) & 255) | (n & 0xff00) | ((n & 255) << 16))
    } else {
        value
            .parse::<i64>()
            .map(|n| n as u32)
            .map_err(|_| "ui.invalid_color_format".into())
    }
}
