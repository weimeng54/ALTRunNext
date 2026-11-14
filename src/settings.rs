// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

//! Native settings pages, created only while the settings window is open.
use crate::{
    localization,
    native::{make_font, scaled, ui, wide},
    theme, theme_ui,
};
use altrun_probe::{
    preferences::{Kind, Preferences, SPECS},
    state_store::TextFile,
};
use std::{
    cell::{Cell, RefCell},
    path::PathBuf,
    ptr::null_mut,
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::LibraryLoader::GetModuleHandleW,
    UI::{
        Controls::{Dialogs::*, *},
        Input::KeyboardAndMouse::*,
        Shell::{DefSubclassProc, SetWindowSubclass, ShellExecuteW},
        WindowsAndMessaging::*,
    },
};

mod pages;
mod window;
use pages::{
    choices, image_control, open_about_link, show_page, shows_in_background_mode, solid_control,
    switch_field,
};
mod form;
use form::{
    field, fields, form_is_dirty, load_form, read_form, report_language_packs, spec_index, status,
    sync_save_button, sync_sound_picker, text, validated_form,
};
mod hotkeys;
use hotkeys::{cancel_capture, hotkey_proc, modifier_bits, record_key, stop_capture};
mod appearance;
use appearance::{
    adjust_number, align_preview, background_image, choose_colour, choose_panel_font,
    chrome_colors, draw, extra_color, paint_settings, preset, refresh_font_family, refresh_style,
    set_colour, show_without_redraw, strength_label, toggle_extra_colors, update_background_mode,
    update_preview,
};
mod apply;
use apply::{apply, choose_file};
pub(crate) mod layout;
mod pickers;
mod preview;
use layout::BoxRect;
pub(crate) use layout::{ABOUT_PAGE, HOTKEYS_PAGE};
use preview::Preview;
pub(crate) use window::open;
pub const UPDATED: u32 = WM_APP + 12;
const RECORDED: u32 = WM_APP + 1;
const TAB: usize = 2301;
const STATUS: usize = 2303;
const PREVIEW: usize = 2305;
const PREVIEW_QUERY: usize = 2306;
const TITLE_CLOSE: usize = 2307;
const SEGMENT: usize = 2330;
const FONT_FAMILY: usize = 2341;
const MORE_COLORS: usize = 2343;
const IMAGE_THUMB: usize = 2344;
const APPEARANCE_NOTE: usize = 2345;
const STRENGTH_VALUE: usize = 2346;
const STRENGTH_LABEL: usize = 2347;
const CROP_LABEL: usize = 2348;
const SOLID_LABEL: usize = 2349;
const SOLID_DEFAULT: usize = 2350;
const SOLID_PICK: usize = 2354;
const STATIC_NOTIFY: u32 = 0x0100;
// Trackbar messages start at WM_USER. windows-sys omits TBM_GETPOS.
const TBM_GETPOS: u32 = WM_USER;
const FIELD: usize = 2400;
const PICK: usize = 2500;
const CLEAR: usize = 2600;
const STRENGTH_UNIT: usize = 2355;
const OWNER_STATIC: u32 = 0x0d;
const OPEN_PAGE: u32 = WM_APP + 3;
const EMAIL_LINK: usize = 2360;
const OPEN_RELEASES: usize = 2361;
const OPEN_SOURCE: usize = 2362;
const OPEN_ISSUES: usize = 2363;
const ABOUT_SECONDARY: usize = 2364;
const CONTACT_EMAIL: &str = "weimeng54@qq.com";
const CONTACT_URI: &str = "mailto:weimeng54@qq.com";
const PROJECT_LINKS: [(usize, &str, &str, &str); 3] = [
    (
        OPEN_RELEASES,
        "about.latest_version",
        "about.open_releases",
        "https://github.com/weimeng54/ALTRunNext/releases",
    ),
    (
        OPEN_SOURCE,
        "about.view_source",
        "about.open_repository",
        "https://github.com/weimeng54/ALTRunNext",
    ),
    (
        OPEN_ISSUES,
        "about.report_issue",
        "about.open_issues",
        "https://github.com/weimeng54/ALTRunNext/issues",
    ),
];

pub enum Change {
    Prepare(u64, Preferences, mpsc::Sender<Result<(), String>>),
    Commit(u64),
    Cancel(u64),
}

struct Page {
    name: &'static str,
    hint: &'static str,
    keys: &'static [&'static str],
}

const PAGES: &[Page] = &[
    Page {
        name: "ui.general",
        hint: "ui.manage_startup_the_system_tray_file_explorer",
        keys: &[
            "ShowMeWhenStart",
            "ShowTrayIcon",
            "AutoRun",
            "AddToSendTo",
            "PlayPopupNotify",
            "PopupFileName",
            "Lang",
        ],
    },
    Page {
        name: "ui.hotkeys",
        hint: "ui.hover_over_a_shortcut_and_press_the",
        keys: &["HotKey1", "HotKey2", "LastItemHotKey"],
    },
    Page {
        name: "ui.search_and_launch",
        hint: "ui.configure_keyword_matching_search_input_and_number",
        keys: &[
            "MatchAnywhere",
            "EnglishSearch",
            "RememberFavouratMatch",
            "NumberKey",
        ],
    },
    Page {
        name: "ui.appearance",
        hint: "ui.customize_the_theme_launcher_and_background",
        keys: &[
            "Theme",
            "ShowSkin",
            "BGFileName",
            "BGStrength",
            "BGCrop",
            "FormWidth",
            "RowHeight",
            "MaxRows",
            "ShowCommandLine",
            "ShowOperationHint",
        ],
    },
    Page {
        name: "ui.fonts_and_colors",
        hint: "ui.set_the_title_search_and_result_text",
        keys: &[
            "TitleFont",
            "KeywordFont",
            "ListFont",
            "TitleColor",
            "KeywordColor",
            "ListColor",
            "UseCustomTextColor",
            "TextColor",
            "SelectionColor",
        ],
    },
    Page {
        name: "ui.window_position",
        hint: "ui.windows_open_in_the_center_of_the",
        keys: &["PositionMode", "HideDelay"],
    },
    Page {
        name: "about.title",
        hint: "",
        keys: &[],
    },
];

static WINDOW: AtomicUsize = AtomicUsize::new(0);
#[derive(Clone, Copy)]
struct Field {
    page: usize,
    label: HWND,
    value: HWND,
    pick: HWND,
    clear: HWND,
}

struct Editor {
    hwnd: HWND,
    tabs: HWND,
    title: HWND,
    hint: HWND,
    status: HWND,
    background: HBRUSH,
    paper: HBRUSH,
    rail: HBRUSH,
    font: HFONT,
    heading_font: HFONT,
    about_font: HFONT,
    section_font: HFONT,
    small_font: HFONT,
    caption_font: HFONT,
    filename_font: HFONT,
    hotkey_font: HFONT,
    sections: Vec<(usize, HWND)>,
    extra_controls: Vec<(usize, HWND)>,
    appearance_children: Vec<HWND>,
    dpi: i32,
    page: usize,
    height: i32,
    more_colors: bool,
    preview: Preview,
    preview_controls: [HWND; 3],
    preview_font: HFONT,
    preview_paper: HBRUSH,
    preview_font_key: String,
    preview_position: Option<(i32, i32, i32, i32, i32, i32)>,
    loading_form: bool,
    chrome_colors: [u32; 5],
    saved: Preferences,
    draft: Preferences,
    fields: Vec<Field>,
    languages: Vec<(String, String)>,
    file: TextFile,
    data: PathBuf,
    sender: mpsc::Sender<Change>,
    owner: usize,
}

#[derive(Clone, Copy)]
struct Capture {
    window: HWND,
    index: usize,
    hook: HHOOK,
}

thread_local! {
    static EDITOR: RefCell<Option<Editor>> = const { RefCell::new(None) };
    static CAPTURE: Cell<Option<Capture>> = const { Cell::new(None) };
}

pub(crate) fn switch_description(hwnd: HWND) -> Option<(&'static str, HFONT)> {
    let id = unsafe { GetDlgCtrlID(hwnd) } as usize;
    let spec = SPECS.get(id.checked_sub(FIELD)?)?;
    let description = layout::field(spec.key).description;
    let font = EDITOR.with(|c| c.borrow().as_ref().map(|e| e.small_font))?;
    Some((description, font))
}

pub(crate) fn preview_edit_colours() -> (u32, u32) {
    EDITOR.with(|c| {
        c.borrow().as_ref().map_or_else(
            || (theme::surface(), theme::paper()),
            |s| (s.preview.input_colours.0, s.preview.input_colours.1),
        )
    })
}

const PRESETS: [(&str, &str, &str); 3] = [
    ("ui.mist_blue", "#DCE7F4", "#263B53"),
    ("ui.sage_gray", "#E0EBE7", "#253B37"),
    ("ui.lavender_gray", "#E9E4F1", "#383149"),
];

pub fn is_open() -> bool {
    WINDOW.load(Ordering::Acquire) != 0
}
