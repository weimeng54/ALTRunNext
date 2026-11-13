//! Settings geometry in device-independent pixels, measured from the design.
use crate::native::scaled;
use windows_sys::Win32::Foundation::RECT;

pub const WIDTH: i32 = 1102;
pub const HEIGHT: i32 = 721;
pub const RAIL: i32 = 186;
pub const CONTENT: i32 = 214;
pub const RIGHT: i32 = 524;
pub const CAPTION: i32 = 42;
pub const FOOTER: i32 = 62;
pub const ADVANCED_RIGHT: i32 = 652;
pub const ADVANCED_EXPANSION: i32 = 125;
pub const PREVIEW: BoxRect = BoxRect::new(RIGHT, 180, 549, 246);
pub const EXTRA_COLORS: BoxRect = BoxRect::new(RIGHT, 478, 549, 161);

#[derive(Clone, Copy)]
pub struct BoxRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl BoxRect {
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Self { x, y, w, h }
    }

    pub fn pixels(self, dpi: i32) -> RECT {
        let px = |v| scaled(v, dpi);
        RECT {
            left: px(self.x),
            top: px(self.y),
            right: px(self.x + self.w),
            bottom: px(self.y + self.h),
        }
    }
}

pub struct Card {
    pub page: usize,
    pub title: &'static str,
    pub bounds: BoxRect,
    pub rules: &'static [i32],
}

const fn card(page: usize, title: &'static str, bounds: BoxRect, rules: &'static [i32]) -> Card {
    Card {
        page,
        title,
        bounds,
        rules,
    }
}

pub const CARDS: &[Card] = &[
    card(
        0,
        "启动与托盘",
        BoxRect::new(CONTENT, 180, 859, 131),
        &[44, 87],
    ),
    card(0, "资源管理器", BoxRect::new(CONTENT, 357, 859, 55), &[]),
    card(0, "提示", BoxRect::new(CONTENT, 458, 422, 136), &[44, 87]),
    card(0, "语言", BoxRect::new(652, 458, 421, 89), &[45]),
    card(
        1,
        "全局快捷键",
        BoxRect::new(CONTENT, 180, 859, 144),
        &[48, 96],
    ),
    card(
        2,
        "匹配与输入",
        BoxRect::new(CONTENT, 180, 859, 185),
        &[44, 87, 141],
    ),
    card(2, "结果与执行", BoxRect::new(CONTENT, 411, 859, 90), &[44]),
    card(3, "主题", BoxRect::new(CONTENT, 180, 290, 60), &[]),
    card(3, "启动面板背景", BoxRect::new(CONTENT, 284, 290, 226), &[]),
    card(
        3,
        "面板与列表",
        BoxRect::new(RIGHT, 486, 549, 161),
        &[41, 81, 117],
    ),
    card(
        4,
        "文字",
        BoxRect::new(CONTENT, 180, 290, 192),
        &[56, 101, 146],
    ),
    card(4, "颜色", BoxRect::new(CONTENT, 416, 290, 88), &[44]),
    card(
        5,
        "启动面板",
        BoxRect::new(CONTENT, 180, 859, 181),
        &[45, 90, 135],
    ),
    card(5, "管理窗口", BoxRect::new(CONTENT, 407, 859, 90), &[45]),
    card(6, "搜索与结果", BoxRect::new(CONTENT, 180, 422, 246), &[80]),
    card(
        6,
        "命令执行",
        BoxRect::new(ADVANCED_RIGHT, 180, 421, 87),
        &[],
    ),
    card(6, "诊断", BoxRect::new(ADVANCED_RIGHT, 314, 421, 68), &[]),
    card(
        6,
        "启动面板外观",
        BoxRect::new(ADVANCED_RIGHT, 429, 421, 124),
        &[61],
    ),
];

pub struct Placement {
    pub bounds: BoxRect,
    pub value_width: i32,
    pub caption: &'static str,
    pub description: &'static str,
}

pub fn field(key: &str) -> Placement {
    let (x, y, w, h, value_width, caption, description) = match key {
        "ShowMeWhenStart" => (215, 181, 857, 43, 0, "", ""),
        "ShowTrayIcon" => (215, 224, 857, 43, 0, "", ""),
        "AutoRun" => (215, 267, 857, 43, 0, "", ""),
        "AddToSendTo" => (
            215,
            358,
            857,
            53,
            0,
            "添加 ALTRun 到「发送到」菜单",
            "右键文件或文件夹，即可添加为启动命令。",
        ),
        "ShowStartNotification" => (215, 459, 420, 43, 0, "", ""),
        "PlayPopupNotify" => (215, 502, 420, 43, 0, "", ""),
        "PopupFileName" => (215, 545, 420, 48, 230, "声音文件", ""),
        "Lang" => (653, 459, 419, 44, 100, "", ""),
        "HotKey1" => (215, 181, 857, 47, 233, "", ""),
        "HotKey2" => (215, 228, 857, 48, 233, "", ""),
        "LastItemHotKey" => (215, 276, 857, 47, 243, "最近启动", ""),
        "Regex" => (215, 181, 857, 43, 0, "支持正则表达式", ""),
        "MatchAnywhere" => (215, 224, 857, 43, 0, "支持模糊匹配", ""),
        "EnglishSearch" => (
            215,
            267,
            857,
            54,
            0,
            "",
            "搜索框失焦后恢复原来的输入法状态。",
        ),
        "RememberFavouratMatch" => (215, 321, 857, 43, 0, "记住常用匹配", ""),
        "NumberKey" => (215, 412, 857, 43, 0, "支持数字键选择结果", ""),
        "ParamHistoryLimit" => (215, 455, 857, 45, 78, "", ""),
        "Theme" | "ShowSkin" => (0, 0, 1, 1, 1, "", ""),
        "BGFileName" => (227, 337, 264, 40, 222, "", ""),
        "BGStrength" => (226, 386, 266, 46, 266, "底纹强度", ""),
        "BGCrop" => (226, 438, 266, 37, 66, "", ""),
        "FormWidth" => (525, 487, 273, 40, 78, "", ""),
        "MaxRows" => (799, 487, 273, 40, 78, "最多显示结果数", ""),
        "RowHeight" => (525, 527, 273, 40, 78, "结果行高", ""),
        "ShowCommandLine" => (799, 527, 273, 40, 0, "显示命令行", ""),
        "ShowTopTen" => (525, 567, 273, 36, 0, "搜索时最多显示十条", ""),
        "ShowOperationHint" => (799, 567, 273, 36, 0, "", ""),
        "ShowShortCutButton" => (525, 603, 182, 43, 0, "", ""),
        "ShowConfigButton" => (707, 603, 182, 43, 0, "", ""),
        "ShowCloseButton" => (889, 603, 183, 43, 0, "", ""),
        "TitleFont" => (215, 236, 288, 45, 78, "标题字号", ""),
        "KeywordFont" => (215, 281, 288, 45, 78, "输入字号", ""),
        "ListFont" => (215, 326, 288, 45, 78, "结果字号", ""),
        "UseCustomTextColor" => (215, 417, 288, 43, 0, "", ""),
        "TextColor" => (215, 460, 288, 43, 130, "自定义文字颜色", ""),
        "TitleColor" => (525, 479, 547, 39, 207, "", ""),
        "KeywordColor" => (525, 518, 547, 40, 207, "", ""),
        "ListColor" => (525, 558, 547, 40, 207, "", ""),
        "SelectionColor" => (525, 598, 547, 40, 207, "", ""),
        "PositionMode" => (215, 181, 857, 44, 170, "呼出位置", ""),
        "HideDelay" => (215, 225, 857, 45, 78, "", ""),
        "WinLeft" => (215, 270, 857, 45, 78, "横坐标", ""),
        "WinTop" => (215, 315, 857, 45, 78, "纵坐标", ""),
        "ManWinLeft" => (215, 408, 857, 44, 78, "横坐标", ""),
        "ManWinTop" => (215, 452, 857, 45, 78, "纵坐标", ""),
        "StandardRegex" => (215, 184, 420, 34, 0, "", ""),
        "ListFormat" => (215, 441, 421, 46, 396, "自定义格式", ""),
        "WorkingDirectory" => (580, 136, 378, 119, 352, "", ""),
        "AllowDynamicTarget" => (653, 185, 419, 34, 0, "允许用参数指定启动目标", ""),
        "TraceEnable" => (653, 318, 419, 34, 0, "保存启动诊断日志", ""),
        "Alpha" => (653, 429, 372, 61, 66, "不透明度", ""),
        "RoundBorderRadius" => (653, 490, 372, 62, 66, "窗口圆角", ""),
        _ => unreachable!("No layout for {key}"),
    };
    Placement {
        bounds: BoxRect::new(x, y, w, h),
        value_width,
        caption,
        description,
    }
}
