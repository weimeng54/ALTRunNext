use std::collections::BTreeMap;
#[derive(Clone, Copy, Debug)]
pub enum Kind {
    Bool,
    Number(i64, i64),
    Text,
    Hotkey,
    Font,
}
#[derive(Clone, Copy, Debug)]
pub struct Spec {
    pub key: &'static str,
    pub label: &'static str,
    pub kind: Kind,
    pub default: &'static str,
}
macro_rules! spec {
    ($key:literal,$label:literal,$kind:ident,$default:literal) => {
        Spec {
            key: $key,
            label: $label,
            kind: Kind::$kind,
            default: $default,
        }
    };
}
pub const DEFAULT_POPUP_SOUND: &str = "Popup.wav";
const SETTINGS_HEADER_PREFIX: &str = "; ALTRunNext settings format ";
const SETTINGS_HEADER: &str = "; ALTRunNext settings format 1";
pub const SETTINGS_IMPORT_FORMAT_ERROR: &str = "ui.choose_an_ini_settings_file_exported_by";

pub const SPECS: &[Spec] = &[
    spec!("AutoRun", "ui.run_at_sign_in", Bool, "0"),
    spec!(
        "AddToSendTo",
        "ui.add_altrunnext_to_the_send_to_menu",
        Bool,
        "0"
    ),
    spec!(
        "MatchAnywhere",
        "ui.allow_substring_and_fuzzy_matches",
        Bool,
        "1"
    ),
    spec!(
        "EnglishSearch",
        "ui.use_direct_english_input_in_the_search",
        Bool,
        "1"
    ),
    spec!("NumberKey", "ui.number_key_launching", Bool, "1"),
    spec!(
        "RememberFavouratMatch",
        "ui.remember_last_choice_for_each_query",
        Bool,
        "0"
    ),
    spec!("ShowOperationHint", "ui.show_operation_hints", Bool, "1"),
    spec!("ShowCommandLine", "ui.show_command_preview", Bool, "1"),
    spec!("PlayPopupNotify", "ui.popup_sound", Bool, "1"),
    spec!("ShowSkin", "ui.use_background_image", Bool, "0"),
    spec!("ShowMeWhenStart", "ui.show_panel_at_startup", Bool, "0"),
    spec!("ShowTrayIcon", "ui.show_tray_icon", Bool, "1"),
    spec!("TraceEnable", "ui.save_diagnostic_log", Bool, "0"),
    spec!(
        "AllowDynamicTarget",
        "ui.allow_dynamic_launch_target",
        Bool,
        "0"
    ),
    Spec {
        key: "HideDelay",
        label: "ui.auto_hide_seconds_0_disables",
        kind: Kind::Number(0, 3600),
        default: "0",
    },
    Spec {
        key: "ParamHistoryLimit",
        label: "ui.parameter_history_limit",
        kind: Kind::Number(0, 1000),
        default: "50",
    },
    Spec {
        key: "FormWidth",
        label: "ui.panel_width",
        kind: Kind::Number(240, 1600),
        default: "420",
    },
    Spec {
        key: "RowHeight",
        label: "ui.result_row_height",
        kind: Kind::Number(16, 48),
        default: "22",
    },
    Spec {
        key: "MaxRows",
        label: "ui.visible_rows_when_expanded",
        kind: Kind::Number(1, 50),
        default: "10",
    },
    spec!("BGFileName", "ui.background_image", Text, ""),
    spec!("BGCrop", "ui.crop_alignment", Text, "center"),
    Spec {
        key: "BGStrength",
        label: "ui.image_strength",
        kind: Kind::Number(0, 100),
        default: "68",
    },
    Spec {
        key: "PopupFileName",
        label: "ui.popup_sound_file",
        kind: Kind::Text,
        default: DEFAULT_POPUP_SOUND,
    },
    spec!("Lang", "settings.display_language", Text, "system"),
    spec!("WorkingDirectory", "ui.default_working_directory", Text, ""),
    spec!("Theme", "ui.theme_light_dark_system", Text, "light"),
    spec!(
        "PositionMode",
        "ui.remember_launcher_position",
        Text,
        "center"
    ),
    spec!("TitleColor", "ui.title_background", Text, "#F5F7FA"),
    spec!("KeywordColor", "ui.input_background", Text, "#FFFFFF"),
    spec!("ListColor", "ui.list_background", Text, "#FFFFFF"),
    spec!("TextColor", "ui.text_color", Text, "#202C3A"),
    spec!("UseCustomTextColor", "ui.use_custom_text_color", Bool, "0"),
    spec!("SelectionColor", "ui.selection_background", Text, "#E7EFF8"),
    spec!(
        "TitleFont",
        "ui.title_font",
        Font,
        "Microsoft YaHei UI|[fsBold]|10|3812384|0"
    ),
    spec!(
        "KeywordFont",
        "ui.input_font",
        Font,
        "Microsoft YaHei UI|[]|12|3812384|0"
    ),
    spec!(
        "ListFont",
        "ui.list_font",
        Font,
        "Microsoft YaHei UI|[]|10|3812384|0"
    ),
    spec!("HotKey1", "ui.main_hotkey", Hotkey, "Alt+R"),
    spec!("HotKey2", "ui.secondary_hotkey", Hotkey, ""),
    spec!(
        "LastItemHotKey",
        "ui.recent_list_hotkey",
        Hotkey,
        "Ctrl+Alt+L"
    ),
];
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
