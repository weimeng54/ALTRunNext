// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

//! Resident native launcher. Show/search only use cached data and controls.
use crate::{
    theme,
    ui::{make_font, scaled, ui, wide},
    windows::{
        clipboard::{clipboard_text, copy_text},
        shell_execute::{execute_request, execute_request_with},
    },
};
use altrun_probe::usage::UsageChange;
use altrun_probe::{Entry, SearchIndex, advance, parse_entries};
use std::{
    cell::{Cell, RefCell},
    mem::zeroed,
    ptr::null_mut,
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::{Dwm::*, Gdi::*},
    System::LibraryLoader::GetModuleHandleW,
    UI::{Controls::*, HiDpi::*, Input::KeyboardAndMouse::*, Shell::*, WindowsAndMessaging::*},
};

mod startup;
pub(crate) use startup::run;
mod window;
use window::{open_manager, open_manager_window, window_proc};
mod search;
use search::{
    choose_number, move_selection, rebuild_index, recent, refresh, reset_search, update_selection,
};
mod input;
use input::{cancel_parameter, edit_proc, edit_text, search_hint, update_search_ime};
mod panel;
use panel::{
    apply_appearance, layout_inline_error, list_proc, paint_panel, paint_title, render_hits,
    show_inline_error,
};
mod hotkeys;
use hotkeys::{BindingUpdate, apply_settings_messages, register_bindings};
mod tray;
use tray::{
    about, edit_selected_command, lifecycle, open_settings, open_settings_page, popup_sound,
    show_menu, tray_icon,
};
mod execution;
use execution::{
    capture_context, launch_selected, selected_action, trace_event, working_directory,
};

const ID_EDIT: usize = 1001;
const ID_LIST: usize = 1002;
const ID_SCROLLBAR: usize = 1006;
const HOTKEY: i32 = 17001;
const SHOW: u32 = WM_APP + 1;
const QUIT: u32 = WM_APP + 2;
const LAUNCH_DONE: u32 = WM_APP + 3;
const TRAY: u32 = WM_APP + 4;
const ID_HIDE: usize = 18001;
const ID_EXIT: usize = 18002;
const ID_MANAGE: usize = 18003;
const ID_SETTINGS: usize = 18005;
const ID_RECENT: usize = 18006;
const ID_ABOUT: usize = 18007;
const ID_RESTART: usize = 18008;
const ID_CLEAN: usize = 18009;
const ID_COPY: usize = 18010;
const ID_OPEN_FOLDER: usize = 18011;
const ID_SHOW: usize = 18012;
const ID_EDIT_COMMAND: usize = 18013;
const AUTO_HIDE: usize = 17030;
const SKIN_DONE: u32 = WM_APP + 15;
const STATE_ERROR: u32 = WM_APP + 17;
const SEARCH_IME_UPDATE: u32 = WM_APP + 18;
const TITLE_STRIP: i32 = 29;
// SS_CENTER from WinUser.h; not projected by the selected windows-sys features.
const TITLE: usize = 1000;
const STATIC_OWNER_DRAW: u32 = 0x0000000d;
type BackdropResult = (crate::skin::Request, Result<crate::skin::Bitmap, String>);
type PendingSettings = (u64, altrun_probe::preferences::Preferences, Bindings);

struct PopupSound {
    path: std::path::PathBuf,
    use_bundled_fallback: bool,
}

struct State {
    sound_sender: std::sync::mpsc::SyncSender<PopupSound>,
    input_paper: HBRUSH,
    font_colours: [u32; 3],
    alias_font: HFONT,
    buttons: [HWND; 3],
    skin: Option<crate::skin::Bitmap>,
    skin_request: Option<crate::skin::Request>,
    skin_sender: std::sync::mpsc::Sender<BackdropResult>,
    skin_results: std::sync::mpsc::Receiver<BackdropResult>,
    preferences: altrun_probe::preferences::Preferences,
    usage: altrun_probe::usage::Usage,
    usage_writer: altrun_probe::state_store::Writer<UsageChange>,
    settings_sender: std::sync::mpsc::Sender<crate::settings::Change>,
    settings_updates: std::sync::mpsc::Receiver<crate::settings::Change>,
    hotkeys: Vec<(i32, Option<altrun_probe::preferences::Hotkey>)>,
    pending_settings: Option<PendingSettings>,
    generation: u64,
    current_query: String,
    default_list_ready: bool,
    last_key: u16,
    context: altrun_probe::launch::LaunchContext,
    successful: Option<(String, String, Option<String>)>,
    history_position: usize,
    entries: Vec<Entry>,
    index: SearchIndex,
    hits: Vec<usize>,
    list_dirty: bool,
    layout: Option<panel::PanelLayout>,
    selected: usize,
    hwnd: HWND,
    edit: HWND,
    list: HWND,
    title: HWND,
    command: HWND,
    inline_error: HWND,
    error_font: HFONT,
    font: HFONT,
    title_font: HFONT,
    input_font: HFONT,
    command_font: HFONT,
    paper: HBRUSH,
    accent: HBRUSH,
    border: HPEN,
    empty: HWND,
    enabled: bool,
    parameter_entry: Option<usize>,
    saved_query: String,
    error: bool,
    launching: Option<u64>,
    launch_results: std::sync::mpsc::Receiver<Result<(), String>>,
    background: HBRUSH,
    selected_brush: HBRUSH,
    old_edit: isize,
    old_list: isize,
    dpi: i32,
    allow_launch: bool,
    composing: bool,
    moving: bool,
    data_path: std::path::PathBuf,
    library_sender: altrun_probe::library_updates::LibraryWriter,
    library_updates: altrun_probe::library_updates::LibraryMailbox,
}

thread_local! { static STATE: RefCell<Option<State>> = const { RefCell::new(None) }; }
thread_local! { static RESTORING_BACKUP: Cell<bool> = const { Cell::new(false) }; }

thread_local! { static LAUNCH_SENDER: RefCell<Option<std::sync::mpsc::Sender<Result<(), String>>>> = const { RefCell::new(None) }; }

type Bindings = Vec<(i32, Option<altrun_probe::preferences::Hotkey>)>;
