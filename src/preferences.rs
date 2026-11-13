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
pub const SETTINGS_IMPORT_FORMAT_ERROR: &str = "请选择 ALTRunNext 导出的 INI 设置文件";

pub const SPECS: &[Spec] = &[
    spec!("AutoRun", "开机启动", Bool, "0"),
    spec!("AddToSendTo", "在「发送到」菜单显示 ALTRunNext", Bool, "0"),
    spec!("MatchAnywhere", "允许中间和模糊匹配", Bool, "1"),
    spec!("EnglishSearch", "搜索框默认使用英文输入", Bool, "1"),
    spec!("NumberKey", "数字快捷执行", Bool, "1"),
    spec!("RememberFavouratMatch", "记住查询的上次选择", Bool, "0"),
    spec!("ShowOperationHint", "显示操作提示", Bool, "1"),
    spec!("ShowCommandLine", "显示命令预览", Bool, "1"),
    spec!("PlayPopupNotify", "呼出声音", Bool, "1"),
    spec!("ShowSkin", "启用背景图片", Bool, "0"),
    spec!("ShowMeWhenStart", "启动时显示面板", Bool, "0"),
    spec!("ShowTrayIcon", "显示托盘图标", Bool, "1"),
    spec!("TraceEnable", "保存诊断日志", Bool, "0"),
    spec!("AllowDynamicTarget", "允许参数作为启动目标", Bool, "0"),
    Spec {
        key: "HideDelay",
        label: "自动隐藏秒数（0 为关闭）",
        kind: Kind::Number(0, 3600),
        default: "0",
    },
    Spec {
        key: "ParamHistoryLimit",
        label: "参数历史上限",
        kind: Kind::Number(0, 1000),
        default: "50",
    },
    Spec {
        key: "FormWidth",
        label: "面板宽度",
        kind: Kind::Number(240, 1600),
        default: "420",
    },
    Spec {
        key: "RowHeight",
        label: "列表行高",
        kind: Kind::Number(16, 48),
        default: "22",
    },
    Spec {
        key: "MaxRows",
        label: "展开列表可见行数",
        kind: Kind::Number(1, 50),
        default: "10",
    },
    spec!("BGFileName", "背景图片", Text, ""),
    spec!("BGCrop", "裁切位置", Text, "center"),
    Spec {
        key: "BGStrength",
        label: "底纹强度（%）",
        kind: Kind::Number(0, 100),
        default: "68",
    },
    Spec {
        key: "PopupFileName",
        label: "呼出声音文件",
        kind: Kind::Text,
        default: DEFAULT_POPUP_SOUND,
    },
    spec!("Lang", "显示语言", Text, "简体中文"),
    spec!("WorkingDirectory", "默认工作目录", Text, ""),
    spec!("Theme", "主题（light / dark / system）", Text, "light"),
    spec!("PositionMode", "记住拖动位置", Text, "center"),
    spec!("TitleColor", "标题背景色", Text, "#F5F7FA"),
    spec!("KeywordColor", "输入框背景色", Text, "#FFFFFF"),
    spec!("ListColor", "列表背景色", Text, "#FFFFFF"),
    spec!("TextColor", "文字颜色", Text, "#202C3A"),
    spec!("UseCustomTextColor", "使用自定义文字颜色", Bool, "0"),
    spec!("SelectionColor", "选中背景色", Text, "#E7EFF8"),
    spec!(
        "TitleFont",
        "标题字体",
        Font,
        "Microsoft YaHei UI|[fsBold]|10|3812384|0"
    ),
    spec!(
        "KeywordFont",
        "输入字体",
        Font,
        "Microsoft YaHei UI|[]|12|3812384|0"
    ),
    spec!(
        "ListFont",
        "列表字体",
        Font,
        "Microsoft YaHei UI|[]|10|3812384|0"
    ),
    spec!("HotKey1", "主呼出快捷键", Hotkey, "Alt+R"),
    spec!("HotKey2", "第二呼出快捷键", Hotkey, ""),
    spec!("LastItemHotKey", "最近项目快捷键", Hotkey, "Ctrl+Alt+L"),
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
            return Err("设置文件过大".into());
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
                    return Err("不支持此设置文件版本".into());
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
            return Err("设置文件过大".into());
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
                return Err(format!("设置第 {} 行缺少等号", i + 1));
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
            return Err("设置值不能含换行或空字符".into());
        }
        let spec = SPECS
            .iter()
            .find(|s| s.key.eq_ignore_ascii_case(key))
            .ok_or_else(|| format!("未知设置：{key}"))?;
        let value = match value.trim() {
            "" if spec.key == "PopupFileName" => DEFAULT_POPUP_SOUND,
            "mouse" if spec.key == "PositionMode" => "center",
            value => value,
        };
        let validated = match spec.kind {
            Kind::Bool => match value.to_ascii_lowercase().as_str() {
                "1" | "true" | "yes" => "1".to_owned(),
                "0" | "false" | "no" => "0".to_owned(),
                _ => return Err(format!("{}需要是 0 或 1", spec.label)),
            },
            Kind::Number(low, high) => {
                let n: i64 = value
                    .parse()
                    .map_err(|_| format!("{}需要整数", spec.label))?;
                if !(low..=high).contains(&n) {
                    return Err(format!("{}范围为 {low}–{high}", spec.label));
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
                    let language = if value.eq_ignore_ascii_case("EN")
                        || value.eq_ignore_ascii_case("English")
                    {
                        "EN"
                    } else {
                        "简体中文"
                    };
                    self.values.insert(spec.key.into(), language.into());
                    return Ok(());
                }
                if spec.key == "Theme" && !matches!(value, "light" | "dark" | "system") {
                    return Err("主题应为 light、dark 或 system".into());
                }
                if spec.key == "PositionMode" && !matches!(value, "remember" | "center") {
                    return Err("请选择有效的窗口位置".into());
                }
                if spec.key == "BGCrop" && !matches!(value, "top" | "center" | "bottom") {
                    return Err("请选择有效的裁切位置".into());
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
                        let n: u32 = other[1..].parse().map_err(|_| "无效功能键")?;
                        if !(1..=24).contains(&n) {
                            return Err("功能键范围为 F1–F24".into());
                        }
                        0x6f + n
                    }
                    _ if other.len() == 1 && other.as_bytes()[0].is_ascii_alphanumeric() => {
                        other.as_bytes()[0] as u32
                    }
                    _ => return Err(format!("无法识别快捷键：{part}")),
                };
                if key.replace(code).is_some() {
                    return Err("快捷键只能有一个主键".into());
                }
            }
        }
    }
    let key = key.ok_or("快捷键缺少主键")?;
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
        let size = size.trim().parse::<i32>().map_err(|_| "字体字号无效")?;
        if size == font.size_dip() {
            return Ok(value.to_owned());
        }
        if !(6..=128).contains(&size) {
            return Err("字号应在 6–128 之间".into());
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
            return Err("字体格式：名称|样式|字号|颜色|字符集".into());
        }
        let points: i32 = p[2].trim().parse().map_err(|_| "字体字号无效")?;
        if !(5..=96).contains(&points) {
            return Err("字号应在 5–96 之间".into());
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
                    .ok_or("字体字号无效")
            })
            .transpose()?;
        Ok(Self {
            strike_out: p[1].contains("fsStrikeOut"),
            charset: p
                .get(4)
                .copied()
                .unwrap_or("1")
                .parse()
                .map_err(|_| "字体字符集无效")?,
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
            return Err("颜色应为 #RRGGBB".into());
        }
        let n = u32::from_str_radix(hex, 16).map_err(|_| "颜色格式无效")?;
        Ok(((n >> 16) & 255) | (n & 0xff00) | ((n & 255) << 16))
    } else {
        value
            .parse::<i64>()
            .map(|n| n as u32)
            .map_err(|_| "颜色格式无效".into())
    }
}
