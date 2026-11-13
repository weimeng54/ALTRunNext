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
pub const PREVIEW: BoxRect = BoxRect::new(RIGHT, 180, 549, 246);
pub const EXTRA_COLORS: BoxRect = BoxRect::new(RIGHT, 478, 549, 161);
pub const ABOUT_PAGE: usize = 6;
pub const ABOUT_PROFILE: BoxRect = BoxRect::new(CONTENT, 132, 859, 190);
pub const ABOUT_LINKS: BoxRect = BoxRect::new(CONTENT, 382, 859, 204);

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
        "ui.startup_and_tray",
        BoxRect::new(CONTENT, 180, 859, 131),
        &[44, 87],
    ),
    card(
        0,
        "ui.file_explorer",
        BoxRect::new(CONTENT, 357, 859, 55),
        &[],
    ),
    card(0, "ui.sound", BoxRect::new(CONTENT, 458, 422, 93), &[44]),
    card(0, "ui.language", BoxRect::new(652, 458, 421, 45), &[]),
    card(
        1,
        "ui.global_hotkeys",
        BoxRect::new(CONTENT, 180, 859, 144),
        &[48, 96],
    ),
    card(
        2,
        "ui.matching_and_input",
        BoxRect::new(CONTENT, 180, 859, 153),
        &[44, 98],
    ),
    card(
        2,
        "ui.results_and_launch",
        BoxRect::new(CONTENT, 379, 859, 45),
        &[],
    ),
    card(3, "ui.theme", BoxRect::new(CONTENT, 180, 290, 60), &[]),
    card(
        3,
        "ui.launcher_background",
        BoxRect::new(CONTENT, 284, 290, 226),
        &[],
    ),
    card(
        3,
        "ui.panel_and_results",
        BoxRect::new(RIGHT, 486, 549, 117),
        &[41, 81],
    ),
    card(
        4,
        "ui.text",
        BoxRect::new(CONTENT, 180, 290, 192),
        &[56, 101, 146],
    ),
    card(4, "ui.colors", BoxRect::new(CONTENT, 416, 290, 88), &[44]),
    card(
        5,
        "ui.launcher",
        BoxRect::new(CONTENT, 180, 859, 100),
        &[55],
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
            "ui.add_altrunnext_to_the_send_to_menu",
            "ui.right_click_a_file_or_folder_to",
        ),
        "PlayPopupNotify" => (215, 459, 420, 43, 0, "", ""),
        "PopupFileName" => (215, 502, 420, 48, 230, "ui.sound_file", ""),
        "Lang" => (653, 459, 419, 44, 192, "", ""),
        "HotKey1" => (215, 181, 857, 47, 233, "", ""),
        "HotKey2" => (215, 228, 857, 48, 233, "", ""),
        "LastItemHotKey" => (215, 276, 857, 47, 243, "ui.recent_launches", ""),
        "MatchAnywhere" => (215, 181, 857, 43, 0, "ui.enable_fuzzy_matching", ""),
        "EnglishSearch" => (
            215,
            224,
            857,
            54,
            0,
            "",
            "ui.restore_the_previous_input_method_state_when",
        ),
        "RememberFavouratMatch" => (
            215,
            278,
            857,
            54,
            0,
            "ui.show_the_last_opened_result_first",
            "ui.applies_to_the_same_search_term_when",
        ),
        "NumberKey" => (
            215,
            380,
            857,
            43,
            0,
            "ui.use_number_keys_to_select_results",
            "",
        ),
        "Theme" | "ShowSkin" => (0, 0, 1, 1, 1, "", ""),
        "BGFileName" => (227, 337, 264, 40, 222, "", ""),
        "BGStrength" => (226, 386, 266, 46, 266, "ui.image_strength_2", ""),
        "BGCrop" => (226, 438, 266, 37, 66, "", ""),
        "FormWidth" => (525, 487, 273, 40, 78, "", ""),
        "MaxRows" => (799, 487, 273, 40, 78, "ui.visible_rows", ""),
        "RowHeight" => (525, 527, 273, 40, 78, "ui.result_row_height_2", ""),
        "ShowCommandLine" => (799, 527, 273, 40, 0, "ui.show_command_line", ""),
        "ShowOperationHint" => (525, 567, 547, 35, 0, "", ""),
        "TitleFont" => (215, 236, 288, 45, 78, "ui.title_size", ""),
        "KeywordFont" => (215, 281, 288, 45, 78, "ui.search_size", ""),
        "ListFont" => (215, 326, 288, 45, 78, "ui.result_size", ""),
        "UseCustomTextColor" => (215, 417, 288, 43, 0, "", ""),
        "TextColor" => (215, 460, 288, 43, 130, "ui.custom_text_color", ""),
        "TitleColor" => (525, 479, 547, 39, 207, "", ""),
        "KeywordColor" => (525, 518, 547, 40, 207, "", ""),
        "ListColor" => (525, 558, 547, 40, 207, "", ""),
        "SelectionColor" => (525, 598, 547, 40, 207, "", ""),
        "PositionMode" => (
            215,
            181,
            857,
            54,
            0,
            "",
            "ui.remember_where_you_drag_the_launcher_turn",
        ),
        "HideDelay" => (215, 235, 857, 44, 78, "", ""),
        _ => unreachable!("No layout for {key}"),
    };
    Placement {
        bounds: BoxRect::new(x, y, w, h),
        value_width,
        caption,
        description,
    }
}
