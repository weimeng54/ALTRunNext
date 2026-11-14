// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

//! On-demand shortcut editor, isolated from the resident launcher's UI thread.
use crate::{
    theme,
    ui::{make_font, scaled, ui, wide},
    windows::file_picker::{drop_paths, pick_path},
};
use altrun_probe::{
    Entry,
    library::{self, OpenTarget, ParamType, Snapshot},
};
use std::{
    cell::RefCell,
    mem::zeroed,
    path::PathBuf,
    ptr::null_mut,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::LibraryLoader::GetModuleHandleW,
    UI::{Controls::*, Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
};

mod view;
mod window;
use view::{index_of, view_order};
use window::run;
mod controls;
use controls::sync_kind_layout;
mod editor;
use editor::{
    can_test, confirm, pending_status, publish_selection, refresh_list, refresh_list_rows, save,
    select_view_row, show_selected_in_list, status, supports_parameter, sync_fields, text,
    update_draft,
};
mod actions;
use actions::tool;
mod imports;
mod intake;
use intake::{add_paths, apply_intake};
mod menu;
use menu::popup;
mod menu_style;
mod paint;
mod repair;
mod table;
const CLASS: &str = "ALTRun.Native.ShortcutManager";
const WIDTH: i32 = crate::ui::geometry::WIDTH;
const HEIGHT: i32 = crate::ui::geometry::HEIGHT;
const RAIL: i32 = 376;
const CAPTION: i32 = crate::ui::geometry::CAPTION;
const FOOTER_TOP: i32 = HEIGHT - crate::ui::geometry::FOOTER;
const FIELD_LEFT: i32 = RAIL + 41;
const FIELD_RIGHT: i32 = WIDTH - 45;
const FIELD_WIDTH: i32 = FIELD_RIGHT - FIELD_LEFT;
const HALF_FIELD: i32 = (FIELD_WIDTH - 14) / 2;
const FORM_LEFT: i32 = RAIL + 25;
const FORM_TOP: i32 = 112;
const FORM_WIDTH: i32 = WIDTH - RAIL - 51;
const COMMAND_FORM_HEIGHT: i32 = 355;
const TABLE_TOP: i32 = 148;
const LIST_TOP: i32 = 179;
const ROW_HEIGHT: i32 = 31;
const TABLE_ROW_HEIGHT_PROP: &str = "ALTRun.TableRowHeight";

const FOCUS: u32 = WM_APP + 1;
const RESTORE_TABLE_SELECTION: u32 = WM_APP + 2;
const PREPARE_BACKUP_RESTORE: u32 = WM_APP + 17;
pub const UPDATED: u32 = WM_APP + 6;
pub const GEOMETRY_CHANGED: u32 = WM_APP + 16;
#[derive(Clone, Default)]
pub struct WindowState {
    pub selected: String,
}
static WINDOW_STATE: std::sync::Mutex<WindowState> = std::sync::Mutex::new(WindowState {
    selected: String::new(),
});
pub fn configure(state: WindowState) {
    if !is_open()
        && let Ok(mut current) = WINDOW_STATE.lock()
    {
        *current = state;
    }
}
pub fn window_state() -> WindowState {
    WINDOW_STATE.lock().map(|s| s.clone()).unwrap_or_default()
}
const LIST: usize = 2101;
const ALIAS: usize = 2102;
const NAME: usize = 2103;
const COMMAND: usize = 2104;
const KIND: usize = 2105;
const FILTER: usize = 2106;
const FILTER_HINT: usize = 2107;
const PARAMETER_CHOICES: [(ParamType, &str); 3] = [
    (ParamType::Raw, "ui.pass_as_entered"),
    (ParamType::Utf8Url, "ui.utf_8_url_encoding"),
    (ParamType::AnsiUrl, "ui.legacy_encoding_windows_ansi"),
];
const ADD: usize = 2110;
const DELETE: usize = 2111;
const SAVE: usize = 2113;
const CLOSE: usize = 2114;
const OPEN_WITH: usize = 2115;
const OPEN_DEFAULT: usize = 2118;
const OPEN_LABEL: usize = 2174;
const CHOOSE_TARGET: usize = 2175;
const ASK_PARAMETER: usize = 2139;
const ADD_FILE: usize = 2120;
const ADD_FOLDER: usize = 2121;
const VALIDATE: usize = 2124;
const EXPORT: usize = 2128;
const RESTORE: usize = 2129;
const SORT_ORIGINAL: usize = 2130;
const SORT_ALIAS: usize = 2132;
const SORT_NAME: usize = 2133;
const MOVE_UP: usize = 2135;
const MOVE_DOWN: usize = 2136;
const TEST: usize = 2137;
const IMPORT_BOOKMARKS: usize = 2138;
const NOTE_LABEL: usize = 2161;
const EMPTY_HELP: usize = 2163;
const CAPTION_CLOSE: usize = 2142;
const NEW_MENU: usize = 2140;
const TOOLS_MENU: usize = 2141;
static TEST_CONTEXT: std::sync::Mutex<altrun_probe::launch::LaunchContext> =
    std::sync::Mutex::new(altrun_probe::launch::LaunchContext {
        clipboard: None,
        window_id: 0,
        window_title: String::new(),
        window_class: String::new(),
        base_directory: String::new(),
        base_is_working_directory: false,
        allow_dynamic_target: false,
    });
pub fn configure_context(context: altrun_probe::launch::LaunchContext) {
    if let Ok(mut current) = TEST_CONTEXT.lock() {
        *current = context;
    }
}
static OPEN: AtomicBool = AtomicBool::new(false);
static WINDOW: AtomicUsize = AtomicUsize::new(0);

struct Editor {
    launch_enabled: bool,
    sort: usize,
    view: Vec<usize>,
    file: Snapshot,
    draft: Vec<Entry>,
    selected: usize,
    loading: bool,
    refreshing_list: bool,
    laid_out_kind: Option<(Option<library::EntryKind>, bool, bool, bool)>,
    list: HWND,
    filter: HWND,
    filter_hint: HWND,
    query: String,
    alias: HWND,
    name: HWND,
    command: HWND,
    kind: HWND,
    ask_parameter: HWND,
    status: HWND,
    save: HWND,
    delete: HWND,
    font: HFONT,
    heading: HFONT,
    caption: HFONT,
    dpi: i32,
    error: bool,
    parent: usize,
    sender: altrun_probe::library_updates::LibraryWriter,
}
impl Editor {
    fn is_dirty(&self) -> bool {
        self.draft != self.file.entries
    }
}
thread_local! { static EDITOR: RefCell<Option<Editor>> = const { RefCell::new(None) }; }

pub fn is_open() -> bool {
    OPEN.load(Ordering::Acquire)
}

type RestoreReply = std::sync::mpsc::Sender<Result<bool, &'static str>>;
static RESTORE_REPLY: std::sync::Mutex<Option<RestoreReply>> = std::sync::Mutex::new(None);

pub(crate) fn prepare_for_restore() -> Result<bool, &'static str> {
    use std::{sync::mpsc, time::Duration};

    if !is_open() {
        return Ok(true);
    }
    let hwnd = WINDOW.load(Ordering::Acquire) as HWND;
    if hwnd.is_null() {
        return Err("backup.busy");
    }
    let (reply, result) = mpsc::channel();
    *RESTORE_REPLY.lock().map_err(|_| "backup.busy")? = Some(reply);
    if unsafe { PostMessageW(hwnd, PREPARE_BACKUP_RESTORE, 0, 0) } == 0 {
        if let Ok(mut slot) = RESTORE_REPLY.lock() {
            slot.take();
        }
        return Err("backup.busy");
    }
    loop {
        match result.recv_timeout(Duration::from_millis(150)) {
            Ok(result) => return result,
            Err(_) if !is_open() => {
                if let Ok(mut slot) = RESTORE_REPLY.lock() {
                    slot.take();
                }
                return Ok(true);
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => return Err("backup.busy"),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
    }
}

pub fn focus() {
    notify(true);
}
fn notify(show: bool) {
    let hwnd = WINDOW.load(Ordering::Acquire);
    if hwnd != 0 {
        unsafe {
            PostMessageW(hwnd as HWND, FOCUS, usize::from(show), 0);
        }
    }
}
pub fn open(
    path: PathBuf,
    initial: Vec<Entry>,
    parent: HWND,
    sender: altrun_probe::library_updates::LibraryWriter,
    launch_enabled: bool,
    show: bool,
) {
    if OPEN.swap(true, Ordering::AcqRel) {
        notify(show);
        return;
    }
    let owner = parent as usize;
    std::thread::spawn(move || {
        if let Err(error) = run(path, initial, owner, sender, launch_enabled, show) {
            unsafe {
                MessageBoxW(
                    null_mut(),
                    ui(&error).as_ptr(),
                    ui("ui.could_not_open_the_shortcut_manager").as_ptr(),
                    MB_OK | MB_ICONERROR,
                );
            }
        }
        WINDOW.store(0, Ordering::Release);
        OPEN.store(false, Ordering::Release);
    });
}

static INTAKE: std::sync::Mutex<Vec<PathBuf>> = std::sync::Mutex::new(Vec::new());
static ACTION: std::sync::Mutex<Option<(String, u16)>> = std::sync::Mutex::new(None);
pub fn enqueue(paths: Vec<PathBuf>, action: Option<(String, u16)>) {
    if let Ok(mut list) = INTAKE.lock() {
        list.extend(paths);
    }
    let show = action.is_some();
    if show && let Ok(mut pending) = ACTION.lock() {
        *pending = action;
    }
    notify(show);
}

// Build transient native cascading menus from the current draft, never cached state.
