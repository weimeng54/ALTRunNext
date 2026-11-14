// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

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
