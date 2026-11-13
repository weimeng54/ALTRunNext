//! Resident native launcher. Show/search only use cached data and controls.
use crate::theme;
use altrun_probe::{Entry, SearchIndex, advance, parse_entries};
use std::{cell::RefCell, mem::zeroed, ptr::null_mut};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::{Dwm::*, Gdi::*},
    System::LibraryLoader::GetModuleHandleW,
    UI::{Controls::*, HiDpi::*, Input::KeyboardAndMouse::*, Shell::*, WindowsAndMessaging::*},
};

const ID_EDIT: usize = 1001;
const ID_LIST: usize = 1002;
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
    usage_writer: altrun_probe::state_store::Writer,
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
    selected: usize,
    hwnd: HWND,
    edit: HWND,
    list: HWND,
    title: HWND,
    command: HWND,
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

pub(crate) fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}
pub(crate) fn ui(text: &str) -> Vec<u16> {
    wide(&crate::localization::tr(text))
}
pub(crate) fn scaled(value: i32, dpi: i32) -> i32 {
    (value * dpi + 48 * value.signum()) / 96
}

pub fn show_error(message: &str) {
    unsafe {
        MessageBoxW(
            null_mut(),
            ui(message).as_ptr(),
            wide("ALTRunNext").as_ptr(),
            MB_OK | MB_ICONERROR,
        );
    }
}

fn selected_text() -> (HWND, HWND, Vec<u16>, Vec<u16>) {
    STATE.with(|cell| {
        let s = cell.borrow();
        let s = s.as_ref().unwrap();
        let e = s.hits.get(s.selected).map(|&i| &s.entries[i]);
        (
            s.title,
            s.command,
            wide(e.map(|e| e.name.as_str()).unwrap_or("ALTRunNext")),
            wide(&e.map(|e| format!("CMD  {}", e.command)).unwrap_or_default()),
        )
    })
}

fn update_selection() {
    STATE.with(|c| c.borrow_mut().as_mut().unwrap().error = false);
    let (title, command, name, line) = selected_text();
    unsafe {
        SetWindowTextW(title, name.as_ptr());
        SetWindowTextW(command, line.as_ptr());
        InvalidateRect(command, std::ptr::null(), 1);
    }
}

fn refresh() {
    if STATE.with(|c| c.borrow().as_ref().unwrap().parameter_entry.is_some()) {
        return;
    }
    let edit = STATE.with(|c| c.borrow().as_ref().unwrap().edit);
    let query = unsafe {
        let mut buffer = vec![0_u16; GetWindowTextLengthW(edit) as usize + 1];
        let len = GetWindowTextW(edit, buffer.as_mut_ptr(), buffer.len() as i32);
        String::from_utf16_lossy(&buffer[..len as usize])
    };
    // Release RefCell before calling native controls, which may synchronously
    // reenter WM_DRAWITEM. Never keep a mutable state borrow across SendMessage.
    STATE.with(|c| {
        let mut s = c.borrow_mut();
        let s = s.as_mut().unwrap();
        s.generation = s.generation.wrapping_add(1);
        s.current_query = query.clone();
        s.default_list_ready = query.is_empty();
        s.hits = s
            .index
            .query_options(&query, s.preferences.flag("MatchAnywhere"));
        if !s.default_list_ready {
            promote_preference(s);
        }
        s.selected = 0;
    });
    render_hits();
}

fn promote_preference(s: &mut State) {
    if s.preferences.flag("RememberFavouratMatch")
        && let Some(id) = s.usage.favourites.get(&s.current_query)
        && let Some(pos) = s.hits.iter().position(|i| &s.entries[*i].id == id)
        && pos != 0
        && s.index.same_match_priority(
            &s.current_query,
            s.preferences.flag("MatchAnywhere"),
            s.hits[0],
            s.hits[pos],
        )
    {
        let hit = s.hits.remove(pos);
        s.hits.insert(0, hit);
    }
}

fn render_hits() {
    let (hwnd, list, command, dpi, count) = STATE.with(|c| {
        let s = c.borrow();
        let s = s.as_ref().unwrap();
        (s.hwnd, s.list, s.command, s.dpi, s.hits.len())
    });
    let (mut width, row, mut max_rows, show_command, edit, title) = STATE.with(|c| {
        let s = c.borrow();
        let s = s.as_ref().unwrap();
        (
            s.preferences.number("FormWidth"),
            s.preferences.number("RowHeight"),
            s.preferences.number("MaxRows"),
            s.preferences.flag("ShowCommandLine"),
            s.edit,
            s.title,
        )
    });
    let row_pixels = scaled(row, dpi).max(1);
    unsafe {
        let mut monitor: MONITORINFO = zeroed();
        monitor.cbSize = std::mem::size_of_val(&monitor) as u32;
        if GetMonitorInfoW(
            MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST),
            &mut monitor,
        ) != 0
        {
            width =
                width.min(((monitor.rcWork.right - monitor.rcWork.left) * 96 / dpi - 2).max(160));
            let available =
                monitor.rcWork.bottom - monitor.rcWork.top - scaled(theme::BASE_HEIGHT, dpi);
            max_rows = max_rows.min((available / row_pixels).max(1));
        }
    }
    let labels = STATE.with(|c| {
        let s = c.borrow();
        let s = s.as_ref().unwrap();
        s.hits
            .iter()
            .take(count)
            .map(|i| format!("{} · {}", s.entries[*i].alias, s.entries[*i].name))
            .collect::<Vec<_>>()
    });
    unsafe {
        SendMessageW(list, WM_SETREDRAW, 0, 0);
        SendMessageW(list, LB_RESETCONTENT, 0, 0);
        for label in labels {
            SendMessageW(list, LB_ADDSTRING, 0, wide(&label).as_ptr() as isize);
        }
        SendMessageW(list, LB_SETCURSEL, 0, 0);
        let height = (count.max(1) as i32).min(max_rows) * row_pixels;
        let empty = STATE.with(|c| c.borrow().as_ref().unwrap().empty);
        SetWindowPos(
            empty,
            null_mut(),
            scaled(12, dpi),
            scaled(theme::LIST_Y + 2, dpi),
            scaled(width - 24, dpi),
            scaled(row, dpi),
            SWP_NOZORDER | SWP_NOACTIVATE,
        );
        SetWindowPos(
            edit,
            null_mut(),
            scaled(18, dpi),
            scaled(35, dpi),
            scaled(width - 36, dpi),
            scaled(22, dpi),
            SWP_NOZORDER | SWP_NOACTIVATE,
        );
        let buttons = STATE.with(|c| c.borrow().as_ref().unwrap().buttons);
        let mut offset = 10;
        for button in buttons.into_iter().rev() {
            offset += 24;
            SetWindowPos(
                button,
                null_mut(),
                scaled(width - offset, dpi),
                scaled(4, dpi),
                scaled(22, dpi),
                scaled(22, dpi),
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
        SetWindowPos(
            title,
            null_mut(),
            scaled(12, dpi),
            scaled(6, dpi),
            scaled(width - offset - 20, dpi),
            scaled(20, dpi),
            SWP_NOZORDER | SWP_NOACTIVATE,
        );
        SetWindowPos(
            list,
            null_mut(),
            scaled(8, dpi),
            scaled(theme::LIST_Y, dpi),
            scaled(width - 16, dpi),
            height,
            SWP_NOZORDER | SWP_NOACTIVATE,
        );
        SetWindowPos(
            command,
            null_mut(),
            scaled(12, dpi),
            scaled(73, dpi) + height,
            scaled(width - 24, dpi),
            scaled(16, dpi),
            SWP_NOZORDER | SWP_NOACTIVATE,
        );
        SetWindowPos(
            hwnd,
            null_mut(),
            0,
            0,
            scaled(width, dpi),
            scaled(if show_command { theme::BASE_HEIGHT } else { 70 }, dpi) + height,
            SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
        );
        let mut rect: RECT = zeroed();
        GetWindowRect(hwnd, &mut rect);
        let diameter = scaled(16, dpi);
        let region = CreateRoundRectRgn(
            0,
            0,
            rect.right - rect.left + 1,
            rect.bottom - rect.top + 1,
            diameter,
            diameter,
        );
        if !region.is_null() && SetWindowRgn(hwnd, region, 1) == 0 {
            DeleteObject(region);
        }
        SendMessageW(list, WM_SETREDRAW, 1, 0);
        InvalidateRect(list, std::ptr::null(), 0);
        let (empty, has_query) = STATE.with(|c| {
            let state = c.borrow();
            let s = state.as_ref().unwrap();
            (s.empty, !s.current_query.is_empty())
        });
        ShowWindow(
            empty,
            if count == 0 && has_query {
                SW_SHOWNOACTIVATE
            } else {
                SW_HIDE
            },
        );
        InvalidateRect(hwnd, std::ptr::null(), 0);
    }
    update_selection();
}

fn move_selection(delta: isize) {
    if STATE.with(|c| c.borrow().as_ref().unwrap().parameter_entry.is_some()) {
        return;
    }
    let (list, selected) = STATE.with(|c| {
        let mut s = c.borrow_mut();
        let s = s.as_mut().unwrap();
        s.selected = advance(s.selected, s.hits.len(), delta);
        (s.list, s.selected)
    });
    unsafe {
        SendMessageW(list, LB_SETCURSEL, selected, 0);
    }
    update_selection();
}

thread_local! { static LAUNCH_SENDER: RefCell<Option<std::sync::mpsc::Sender<Result<(), String>>>> = const { RefCell::new(None) }; }

fn edit_text(hwnd: HWND) -> String {
    unsafe {
        let mut text = vec![0; GetWindowTextLengthW(hwnd) as usize + 1];
        let n = GetWindowTextW(hwnd, text.as_mut_ptr(), text.len() as i32);
        String::from_utf16_lossy(&text[..n as usize])
    }
}
fn show_inline_error(message: &str) {
    let (hwnd, edit, command) = STATE.with(|c| {
        let mut s = c.borrow_mut();
        let s = s.as_mut().unwrap();
        s.error = true;
        (s.hwnd, s.edit, s.command)
    });
    unsafe {
        SetWindowTextW(command, ui(message).as_ptr());
        InvalidateRect(command, std::ptr::null(), 1);
        ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        SetForegroundWindow(hwnd);
        SetFocus(edit);
    }
}
fn search_hint(show_operations: bool) -> Vec<u16> {
    if show_operations {
        ui("ui.type_a_shortcut_enter_to_launch_esc")
    } else {
        ui("ui.type_a_shortcut")
    }
}

fn update_search_ime(allow_direct: bool) {
    let target = STATE.with(|c| {
        let state = c.borrow();
        let s = state.as_ref()?;
        Some((
            s.hwnd,
            s.edit,
            allow_direct && s.parameter_entry.is_none() && s.preferences.flag("EnglishSearch"),
        ))
    });
    let Some((hwnd, edit, direct)) = target else {
        return;
    };
    unsafe {
        if InSendMessageEx(std::ptr::null()) != ISMEX_NOSEND {
            // Defer across cross-thread sends so IMM cannot reenter another IME call.
            PostMessageW(hwnd, SEARCH_IME_UPDATE, 0, 0);
            return;
        }
        crate::search_ime::set_direct(
            edit,
            direct && GetFocus() == edit && GetActiveWindow() == hwnd && IsWindowVisible(hwnd) != 0,
        );
    }
}

fn reset_search() {
    let (edit, list, was_parameter, show_operations, needs_refresh) = STATE.with(|c| {
        let mut state = c.borrow_mut();
        let s = state.as_mut().unwrap();
        let was_parameter = s.parameter_entry.take().is_some();
        s.saved_query.clear();
        s.history_position = usize::MAX;
        s.last_key = 0;
        s.composing = false;
        (
            s.edit,
            s.list,
            was_parameter,
            s.preferences.flag("ShowOperationHint"),
            was_parameter
                || !s.default_list_ready
                || s.selected != 0
                || s.launching.is_some()
                || s.error,
        )
    });
    unsafe {
        if was_parameter {
            EnableWindow(list, 1);
            SendMessageW(
                edit,
                EM_SETCUEBANNER,
                0,
                search_hint(show_operations).as_ptr() as LPARAM,
            );
        }
        if GetWindowTextLengthW(edit) != 0 {
            // The single-line edit sends EN_CHANGE synchronously.
            SetWindowTextW(edit, [0_u16].as_ptr());
        } else if needs_refresh {
            refresh();
        }
    }
    if was_parameter {
        update_search_ime(true);
    }
}

fn cancel_parameter() -> bool {
    let restore = STATE.with(|c| {
        let mut s = c.borrow_mut();
        let s = s.as_mut().unwrap();
        s.parameter_entry.take().map(|_| {
            (
                s.edit,
                s.list,
                std::mem::take(&mut s.saved_query),
                s.preferences.flag("ShowOperationHint"),
            )
        })
    });
    if let Some((edit, list, query, show_operations)) = restore {
        unsafe {
            EnableWindow(list, 1);
            SetWindowTextW(edit, wide(&query).as_ptr());
            SendMessageW(
                edit,
                EM_SETCUEBANNER,
                0,
                search_hint(show_operations).as_ptr() as LPARAM,
            );
        }
        refresh();
        update_search_ime(true);
        true
    } else {
        false
    }
}
fn launch_selected() {
    let (enabled, allow, busy, selected, parameter_mode, hwnd, edit, title) = STATE.with(|c| {
        let s = c.borrow();
        let s = s.as_ref().unwrap();
        (
            s.enabled,
            s.allow_launch,
            s.launching.is_some(),
            s.parameter_entry
                .or_else(|| s.hits.get(s.selected).copied()),
            s.parameter_entry.is_some(),
            s.hwnd,
            s.edit,
            s.title,
        )
    });
    if busy {
        return;
    }
    let Some(index) = selected else {
        let query = STATE.with(|c| c.borrow().as_ref().unwrap().current_query.clone());
        if enabled
            && !query.is_empty()
            && unsafe {
                MessageBoxW(
                    hwnd,
                    ui("ui.no_matches_add_a_new_shortcut").as_ptr(),
                    ui("ui.new_shortcut").as_ptr(),
                    MB_OKCANCEL | MB_ICONQUESTION | MB_DEFBUTTON2,
                )
            } == IDOK
        {
            crate::manager::enqueue(Vec::new(), Some((query, VK_INSERT)));
            open_manager();
        }
        return;
    };
    let entry = STATE.with(|c| c.borrow().as_ref().unwrap().entries[index].clone());
    if !enabled {
        if allow
            && entry.command == "notepad.exe"
            && let Ok(path) = altrun_probe::safe_notepad_path()
        {
            let _ = std::process::Command::new(path).spawn();
        }
        unsafe {
            ShowWindow(hwnd, SW_HIDE);
        }
        return;
    }
    if entry.param_type.needs_input(&entry.command)
        && !parameter_mode
        && (entry.open_target.is_some()
            || altrun_probe::launch::has_parameter(&entry.command)
            || !altrun_probe::launch::has_context(&entry.command))
    {
        let query = edit_text(edit);
        let list = STATE.with(|c| c.borrow().as_ref().unwrap().list);
        STATE.with(|c| {
            let mut s = c.borrow_mut();
            let s = s.as_mut().unwrap();
            s.saved_query = query;
            s.parameter_entry = Some(index);
            s.history_position = usize::MAX;
        });
        update_search_ime(false);
        unsafe {
            EnableWindow(list, 0);
            SetWindowTextW(
                title,
                wide(&crate::localization::message(
                    "ui.name_enter_a_parameter",
                    &[("name", &entry.name)],
                ))
                .as_ptr(),
            );
            SetWindowTextW(edit, wide("").as_ptr());
            SendMessageW(
                edit,
                EM_SETCUEBANNER,
                0,
                ui("ui.enter_a_parameter_enter_to_launch_esc").as_ptr() as LPARAM,
            );
        }
        return;
    }
    let parameter = if parameter_mode {
        Some(edit_text(edit))
    } else {
        None
    };
    let result_sender = LAUNCH_SENDER.with(|s| s.borrow().as_ref().unwrap().clone());
    let query = if parameter_mode {
        STATE.with(|c| c.borrow().as_ref().unwrap().saved_query.clone())
    } else {
        STATE.with(|c| c.borrow().as_ref().unwrap().current_query.clone())
    };
    let mut context = STATE.with(|c| c.borrow().as_ref().unwrap().context.clone());
    STATE.with(|c| {
        let mut state = c.borrow_mut();
        let s = state.as_mut().unwrap();
        s.successful = Some((entry.id.clone(), query, parameter.clone()));
        s.launching = Some(s.generation);
    });
    unsafe {
        ShowWindow(hwnd, SW_HIDE);
    }
    let owner = hwnd as usize;
    let language = crate::localization::current();
    // All path checks, environment expansion, COM initialization and shell work
    // happen off the resident UI thread. No implicit cmd.exe is introduced.
    std::thread::spawn(move || {
        crate::localization::install_snapshot(language);
        if entry.open_target.is_none()
            && altrun_probe::launch::has_context(&entry.command)
            && entry.command.contains("%c")
        {
            context.clipboard = clipboard_text();
        }
        let result = altrun_probe::launch::resolve_entry(&entry, parameter.as_deref(), &context)
            .and_then(|(request, mode, directory)| {
                execute_request_with(&request, mode, directory.as_deref())
            });
        let _ = result_sender.send(result);
        unsafe {
            PostMessageW(owner as HWND, LAUNCH_DONE, 0, 0);
        }
    });
}
fn execute_request(request: &altrun_probe::launch::LaunchRequest) -> Result<(), String> {
    execute_request_with(request, altrun_probe::launch::ShowMode::Normal, None)
}
pub(crate) fn execute_request_with(
    request: &altrun_probe::launch::LaunchRequest,
    mode: altrun_probe::launch::ShowMode,
    directory: Option<&str>,
) -> Result<(), String> {
    use windows_sys::Win32::System::Com::*;
    let file = wide(&request.file);
    let parameters = wide(&request.parameters);
    let directory = match directory {
        Some(directory) => wide(directory),
        None => {
            let profile = std::env::var_os("USERPROFILE")
                .map(std::path::PathBuf::from)
                .filter(|path| path.is_absolute() && path.is_dir());
            let directory = profile.map_or_else(altrun_probe::windows_system_directory, Ok)?;
            wide(&directory.to_string_lossy())
        }
    };
    unsafe {
        let hr = CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED as u32);
        if hr < 0 {
            return Err(crate::localization::message(
                "ui.could_not_initialize_the_launch_service_0x",
                &[("code", &format!("{:08X}", hr as u32))],
            ));
        }
        let mut info: SHELLEXECUTEINFOW = zeroed();
        info.cbSize = std::mem::size_of_val(&info) as u32;
        info.fMask = SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI;
        // lpVerb must outlive ShellExecuteExW, just like file and parameters.
        let verb = wide("open");
        info.lpVerb = verb.as_ptr();
        info.lpFile = file.as_ptr();
        info.lpParameters = parameters.as_ptr();
        info.lpDirectory = directory.as_ptr();
        info.nShow = match mode {
            altrun_probe::launch::ShowMode::Normal => SW_SHOWNORMAL,
            altrun_probe::launch::ShowMode::Maximized => SW_SHOWMAXIMIZED,
            altrun_probe::launch::ShowMode::Minimized => SW_SHOWMINNOACTIVE,
            altrun_probe::launch::ShowMode::Hidden => SW_HIDE,
        };
        let ok = ShellExecuteExW(&mut info);
        let error = GetLastError();
        CoUninitialize();
        if ok == 0 {
            Err(crate::localization::message(
                "ui.could_not_launch_error_error_check_the",
                &[("error", &error.to_string())],
            ))
        } else {
            Ok(())
        }
    }
}
pub(crate) fn make_font(size: i32, weight: i32, family: &str, dpi: i32) -> HFONT {
    unsafe {
        CreateFontW(
            -scaled(size, dpi),
            0,
            0,
            0,
            weight,
            0,
            0,
            0,
            DEFAULT_CHARSET as u32,
            OUT_DEFAULT_PRECIS as u32,
            CLIP_DEFAULT_PRECIS as u32,
            CLEARTYPE_QUALITY as u32,
            0,
            wide(family).as_ptr(),
        )
    }
}
fn paint_title(hwnd: HWND, item: &DRAWITEMSTRUCT) {
    let name = crate::theme_ui::window_text(item.hwndItem);
    STATE.with(|state| {
        let state = state.borrow();
        let Some(s) = state.as_ref() else {
            return;
        };
        let dc = item.hDC;
        unsafe {
            FillRect(dc, &item.rcItem, s.background);
            if let Some(bitmap) = &s.skin {
                let mut origin = POINT { x: 0, y: 0 };
                let mut rect: RECT = zeroed();
                GetClientRect(hwnd, &mut rect);
                MapWindowPoints(item.hwndItem, hwnd, &mut origin, 1);
                OffsetRect(&mut rect, -origin.x, -origin.y);
                let old = SaveDC(dc);
                if old != 0 {
                    IntersectClipRect(
                        dc,
                        item.rcItem.left,
                        item.rcItem.top,
                        item.rcItem.right,
                        item.rcItem.bottom,
                    );
                    bitmap.draw(dc, rect, &s.preferences.get("BGCrop"));
                    RestoreDC(dc, old);
                }
            }
            let font = SelectObject(dc, s.title_font);
            crate::theme_ui::line_text(dc, item.rcItem, &name, s.font_colours[0], 0);
            SelectObject(dc, font);
        }
    });
}
fn paint_panel(hwnd: HWND) {
    unsafe {
        let mut ps: PAINTSTRUCT = zeroed();
        let dc = BeginPaint(hwnd, &mut ps);
        STATE.with(|c| {
            let s = c.borrow();
            let Some(s) = s.as_ref() else {
                return;
            };
            if let Some(bitmap) = &s.skin {
                let mut rect: RECT = zeroed();
                GetClientRect(hwnd, &mut rect);
                bitmap.draw(dc, rect, &s.preferences.get("BGCrop"));
            }
            let old_brush = SelectObject(dc, s.input_paper);
            let old_pen = SelectObject(dc, s.border);
            let mut panel: RECT = zeroed();
            GetClientRect(hwnd, &mut panel);
            RoundRect(
                dc,
                scaled(10, s.dpi),
                scaled(29, s.dpi),
                panel.right - scaled(10, s.dpi),
                scaled(60, s.dpi),
                scaled(8, s.dpi),
                scaled(8, s.dpi),
            );
            let mut rect: RECT = zeroed();
            GetClientRect(hwnd, &mut rect);
            if s.preferences.flag("ShowCommandLine") {
                MoveToEx(
                    dc,
                    scaled(12, s.dpi),
                    rect.bottom - scaled(25, s.dpi),
                    std::ptr::null_mut(),
                );
                LineTo(
                    dc,
                    rect.right - scaled(12, s.dpi),
                    rect.bottom - scaled(25, s.dpi),
                );
            }
            SelectObject(dc, old_brush);
            SelectObject(dc, old_pen);
        });
        EndPaint(hwnd, &ps);
    }
}
fn show_menu(hwnd: HWND, tray: bool) {
    unsafe {
        let menu = CreatePopupMenu();
        if menu.is_null() {
            return;
        }
        let items: &[(usize, &str)] = if tray {
            &[
                (ID_SHOW, "ui.show_launcher"),
                (ID_MANAGE, "ui.manage_shortcuts"),
                (ID_SETTINGS, "ui.settings"),
                (0, ""),
                (ID_EXIT, "ui.exit_altrunnext"),
            ]
        } else {
            &[
                (ID_COPY, "ui.copy_command_ctrl_c"),
                (ID_OPEN_FOLDER, "ui.open_containing_folder_ctrl_d"),
                (ID_RECENT, "ui.recent_launches_ctrl_l"),
                (0, ""),
                (ID_ABOUT, "ui.about_f1"),
                (ID_HIDE, "ui.hide"),
            ]
        };
        let has_selection = STATE.with(|c| {
            let state = c.borrow();
            let s = state.as_ref().unwrap();
            s.hits.get(s.selected).is_some() && s.parameter_entry.is_none()
        });
        for &(id, label) in items {
            if id == 0 {
                AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
            } else {
                let unavailable = matches!(id, ID_COPY | ID_OPEN_FOLDER) && !has_selection;
                let flags = MF_STRING | if unavailable { MF_GRAYED } else { 0 };
                AppendMenuW(menu, flags, id, ui(label).as_ptr());
            }
        }
        let mut point: POINT = zeroed();
        GetCursorPos(&mut point);
        SetForegroundWindow(hwnd);
        TrackPopupMenu(
            menu,
            TPM_RIGHTBUTTON,
            point.x,
            point.y,
            0,
            hwnd,
            std::ptr::null(),
        );
        DestroyMenu(menu);
        PostMessageW(hwnd, WM_NULL, 0, 0);
    }
}
fn tray_icon(hwnd: HWND, operation: u32) {
    unsafe {
        let mut icon: NOTIFYICONDATAW = zeroed();
        icon.cbSize = std::mem::size_of_val(&icon) as u32;
        icon.hWnd = hwnd;
        icon.uID = 1;
        icon.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
        icon.uCallbackMessage = TRAY;
        icon.hIcon =
            crate::theme_ui::app_icon(GetSystemMetricsForDpi(SM_CXSMICON, GetDpiForSystem()));
        let binding = STATE.with(|c| {
            let state = c.borrow();
            let state = state.as_ref()?;
            ["HotKey1", "HotKey2"]
                .into_iter()
                .zip(&state.hotkeys)
                .find(|(_, (_, key))| key.is_some())
                .map(|(name, _)| state.preferences.get(name))
        });
        let tip =
            wide(&binding.map_or_else(|| "ALTRunNext".into(), |key| format!("ALTRunNext · {key}")));
        let len = tip.len().min(icon.szTip.len() - 1);
        icon.szTip[..len].copy_from_slice(&tip[..len]);
        if Shell_NotifyIconW(operation, &icon) == 0 && operation == NIM_ADD {
            Shell_NotifyIconW(NIM_MODIFY, &icon);
        }
    }
}

unsafe extern "system" fn edit_proc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    if msg == WM_IME_STARTCOMPOSITION {
        STATE.with(|c| c.borrow_mut().as_mut().unwrap().composing = true);
    }
    if msg == WM_IME_ENDCOMPOSITION {
        STATE.with(|c| c.borrow_mut().as_mut().unwrap().composing = false);
    }
    let (composing, parameter_mode) = STATE.with(|c| {
        let s = c.borrow();
        let s = s.as_ref().unwrap();
        (s.composing, s.parameter_entry.is_some())
    });
    if matches!(msg, WM_KEYDOWN | WM_SYSKEYDOWN) && !composing {
        let control = unsafe { GetKeyState(VK_CONTROL as i32) } < 0;
        let alt = unsafe { GetKeyState(VK_MENU as i32) } < 0;
        STATE.with(|c| {
            c.borrow_mut().as_mut().unwrap().last_key =
                if control && matches!(w as u16, 0x56 | 0x58 | 0x5a) {
                    u16::MAX
                } else {
                    0
                }
        });
        if (control || alt) && !parameter_mode {
            let digit = match w as u16 {
                0x30..=0x39 => Some(w - 0x30),
                0x60..=0x69 => Some(w - 0x60),
                _ => None,
            };
            if let Some(digit) = digit {
                choose_number(digit);
                return 0;
            }
        }
        let (delay, parent) = STATE.with(|c| {
            let s = c.borrow();
            let s = s.as_ref().unwrap();
            (s.preferences.number("HideDelay"), s.hwnd)
        });
        if delay > 0 {
            unsafe {
                SetTimer(parent, AUTO_HIDE, delay as u32 * 1000, None);
            }
        }
        if control {
            match w as u16 {
                0x43 if !parameter_mode => {
                    selected_action(true);
                    return 0;
                }
                0x44 if !parameter_mode => {
                    selected_action(false);
                    return 0;
                }
                0x4c if !parameter_mode => {
                    recent();
                    return 0;
                }
                VK_UP | VK_DOWN
                    if parameter_mode
                        && parameter_history(if w as u16 == VK_UP { -1 } else { 1 }) =>
                {
                    return 0;
                }
                _ => {}
            }
        }
        match w as u16 {
            VK_F1 if !parameter_mode => {
                about();
                return 0;
            }
            VK_F2 | VK_INSERT | VK_DELETE if !parameter_mode => {
                let id = STATE.with(|c| {
                    let s = c.borrow();
                    let s = s.as_ref().unwrap();
                    if w as u16 == VK_INSERT {
                        return s.current_query.clone();
                    }
                    s.hits
                        .get(s.selected)
                        .map(|i| s.entries[*i].id.clone())
                        .unwrap_or_default()
                });
                if id.is_empty() && w as u16 != VK_INSERT {
                    return 0;
                }
                crate::manager::enqueue(Vec::new(), Some((id, w as u16)));
                open_manager();
                return 0;
            }
            VK_PRIOR if !parameter_mode => {
                move_selection(-10);
                return 0;
            }
            VK_NEXT if !parameter_mode => {
                move_selection(10);
                return 0;
            }
            0xba | 0xde if !parameter_mode => {
                choose_number(if w == 0xba { 2 } else { 3 });
                return 0;
            }
            VK_DOWN if alt => {
                parameter_menu();
                return 0;
            }
            VK_DOWN if !parameter_mode => {
                move_selection(1);
                return 0;
            }
            VK_UP if !parameter_mode => {
                move_selection(-1);
                return 0;
            }
            VK_TAB if !parameter_mode => {
                move_selection(if unsafe { GetKeyState(VK_SHIFT as i32) } < 0 {
                    -1
                } else {
                    1
                });
                return 0;
            }
            VK_RETURN => {
                launch_selected();
                return 0;
            }
            VK_ESCAPE => {
                if cancel_parameter() {
                    return 0;
                }
                if unsafe { GetWindowTextLengthW(hwnd) } > 0 {
                    unsafe {
                        SetWindowTextW(hwnd, wide("").as_ptr());
                    }
                } else {
                    let parent = unsafe { GetParent(hwnd) };
                    unsafe {
                        ShowWindow(parent, SW_HIDE);
                    }
                }
                return 0;
            }
            _ => {}
        }
    }
    if msg == WM_CHAR
        && !parameter_mode
        && !composing
        && !matches!(w, 3 | 4 | 12 | 13 | 27 | 9 | 59 | 39)
    {
        let control = unsafe { GetKeyState(VK_CONTROL as i32) } < 0;
        if control && (0x30..=0x39).contains(&w) {
            return 0;
        }
        STATE.with(|c| {
            c.borrow_mut().as_mut().unwrap().last_key = if w == 32 {
                VK_SPACE
            } else if (0x30..=0x39).contains(&w) {
                w as u16
            } else {
                u16::MAX
            }
        });
    }
    if msg == WM_SYSCHAR && !parameter_mode && (0x30..=0x39).contains(&w) {
        return 0;
    }
    if msg == WM_CHAR
        && !parameter_mode
        && unsafe { GetKeyState(VK_CONTROL as i32) } < 0
        && matches!(w, 3 | 4 | 12)
    {
        return 0;
    }
    if msg == WM_CHAR && !parameter_mode && matches!(w, 59 | 39) && !composing {
        return 0;
    }
    if msg == WM_CHAR && matches!(w as u16, VK_RETURN | VK_ESCAPE | VK_TAB) && !composing {
        return 0;
    }
    let old = STATE.with(|c| c.borrow().as_ref().unwrap().old_edit);
    let result = unsafe {
        CallWindowProcW(
            Some(std::mem::transmute::<
                isize,
                unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT,
            >(old)),
            hwnd,
            msg,
            w,
            l,
        )
    };
    if matches!(msg, WM_SETFOCUS | WM_KILLFOCUS) {
        update_search_ime(msg == WM_SETFOCUS);
    }
    result
}

unsafe extern "system" fn window_proc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    match msg {
        WM_COPYDATA => match crate::instance::decode(l) {
            Ok(paths) => {
                if !paths.is_empty() {
                    crate::manager::enqueue(paths, None);
                    open_manager_window(false);
                } else {
                    unsafe {
                        PostMessageW(hwnd, SHOW, 0, 0);
                    }
                }
                1
            }
            Err(_) => 0,
        },
        WM_DROPFILES => {
            let paths = crate::manager::drop_paths(w as _);
            crate::manager::enqueue(paths, None);
            open_manager_window(false);
            0
        }
        WM_HOTKEY | SHOW => {
            let is_recent = msg == WM_HOTKEY
                && STATE.with(|c| {
                    c.borrow()
                        .as_ref()
                        .unwrap()
                        .hotkeys
                        .get(2)
                        .is_some_and(|h| h.0 == w as i32)
                });
            if msg == WM_HOTKEY && !is_recent && unsafe { IsWindowVisible(hwnd) != 0 } {
                unsafe {
                    ShowWindow(hwnd, SW_HIDE);
                }
                return 0;
            }
            capture_context();
            if is_recent {
                recent();
            } else {
                reset_search();
            }
            position_panel();
            let edit = STATE.with(|c| c.borrow().as_ref().unwrap().edit);
            unsafe {
                ShowWindow(hwnd, SW_SHOWNOACTIVATE);
                SetForegroundWindow(hwnd);
                SetFocus(edit);
                popup_sound();
                let delay =
                    STATE.with(|c| c.borrow().as_ref().unwrap().preferences.number("HideDelay"));
                if delay > 0 {
                    SetTimer(hwnd, AUTO_HIDE, delay as u32 * 1000, None);
                }
            }
            update_search_ime(true);
            0
        }
        SEARCH_IME_UPDATE => {
            update_search_ime(true);
            0
        }
        WM_SHOWWINDOW if w == 0 => {
            update_search_ime(false);
            unsafe { DefWindowProcW(hwnd, msg, w, l) }
        }
        WM_CONTEXTMENU => {
            show_menu(hwnd, false);
            0
        }
        TRAY => {
            if l as u32 == WM_RBUTTONUP {
                show_menu(hwnd, true);
            } else if l as u32 == WM_LBUTTONUP {
                unsafe {
                    SendMessageW(hwnd, SHOW, 0, 0);
                }
            }
            0
        }
        LAUNCH_DONE => {
            let completed = STATE.with(|c| {
                let mut state = c.borrow_mut();
                let s = state.as_mut().unwrap();
                let result = s.launch_results.try_recv().ok()?;
                let current = s.launching.take() == Some(s.generation);
                Some((result, current, s.successful.take()))
            });
            let Some((result, current, launched)) = completed else {
                return 0;
            };
            match result {
                Err(message) => {
                    trace_event("launch_failed");
                    if current {
                        show_inline_error(&message);
                    }
                }
                Ok(()) => {
                    trace_event("launch_succeeded");
                    STATE.with(|c| {
                        let mut s = c.borrow_mut();
                        let s = s.as_mut().unwrap();
                        if let Some((id, query, param)) = launched {
                            s.usage.record(
                                &id,
                                &query,
                                param.as_deref(),
                                s.preferences.number("ParamHistoryLimit") as usize,
                            );
                            s.usage_writer.publish(s.usage.text());
                            rebuild_index(s);
                        }
                    });
                    if current {
                        cancel_parameter();
                    }
                }
            }
            0
        }
        crate::settings::UPDATED => {
            apply_settings_messages();
            0
        }
        STATE_ERROR => {
            let errors = STATE.with(|c| {
                c.borrow()
                    .as_ref()
                    .unwrap()
                    .usage_writer
                    .errors
                    .try_iter()
                    .collect::<Vec<_>>()
            });
            if let Some(error) = errors.last() {
                show_inline_error(&crate::localization::message(
                    "ui.could_not_save_launch_history_error",
                    &[("error", &crate::localization::tr(error))],
                ));
            }
            0
        }
        SKIN_DONE => {
            let mut errors = Vec::new();
            STATE.with(|c| {
                let mut s = c.borrow_mut();
                let s = s.as_mut().unwrap();
                while let Ok((request, result)) = s.skin_results.try_recv() {
                    if s.skin_request.as_ref() == Some(&request) {
                        match result {
                            Ok(bitmap) => s.skin = Some(bitmap),
                            Err(error) => errors.push(error),
                        }
                    }
                }
            });
            unsafe {
                RedrawWindow(
                    hwnd,
                    std::ptr::null(),
                    null_mut(),
                    RDW_INVALIDATE | RDW_ALLCHILDREN,
                );
            }
            if let Some(e) = errors.pop() {
                show_inline_error(&e);
            }
            0
        }
        WM_SETTINGCHANGE | WM_THEMECHANGED => {
            apply_appearance();
            render_hits();
            0
        }
        WM_DPICHANGED => {
            STATE.with(|c| c.borrow_mut().as_mut().unwrap().dpi = (w & 65535) as i32);
            if l != 0 {
                let suggested = unsafe { &*(l as *const RECT) };
                unsafe {
                    SetWindowPos(
                        hwnd,
                        null_mut(),
                        suggested.left,
                        suggested.top,
                        0,
                        0,
                        SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
                    );
                }
            }
            apply_appearance();
            render_hits();
            0
        }
        WM_NCHITTEST => {
            let result = unsafe { DefWindowProcW(hwnd, msg, w, l) };
            if result != HTCLIENT as isize {
                return result;
            }
            let dpi = STATE.with(|c| c.borrow().as_ref().map_or(96, |s| s.dpi));
            let mut point = POINT {
                x: l as i16 as i32,
                y: (l >> 16) as i16 as i32,
            };
            if unsafe { ScreenToClient(hwnd, &mut point) } != 0
                && (0..scaled(TITLE_STRIP, dpi)).contains(&point.y)
            {
                HTCAPTION as isize
            } else {
                result
            }
        }
        WM_ENTERSIZEMOVE => {
            STATE.with(|c| c.borrow_mut().as_mut().unwrap().moving = true);
            unsafe {
                KillTimer(hwnd, AUTO_HIDE);
            }
            0
        }
        WM_EXITSIZEMOVE => {
            let mut rect: RECT = unsafe { zeroed() };
            unsafe {
                GetWindowRect(hwnd, &mut rect);
            }
            let delay = STATE.with(|c| {
                let mut s = c.borrow_mut();
                let s = s.as_mut().unwrap();
                s.moving = false;
                if s.preferences.get("PositionMode") == "remember" {
                    s.usage.positions.insert(
                        "launcher".into(),
                        (
                            rect.left,
                            rect.top,
                            rect.right - rect.left,
                            rect.bottom - rect.top,
                        ),
                    );
                    s.usage_writer.publish(s.usage.text());
                }
                s.preferences.number("HideDelay")
            });
            if delay > 0 && unsafe { IsWindowVisible(hwnd) != 0 } {
                unsafe {
                    SetTimer(hwnd, AUTO_HIDE, delay as u32 * 1000, None);
                }
            }
            0
        }
        WM_TIMER if w == AUTO_HIDE => {
            unsafe {
                KillTimer(hwnd, AUTO_HIDE);
                if !STATE.with(|c| c.borrow().as_ref().unwrap().moving) {
                    ShowWindow(hwnd, SW_HIDE);
                }
            }
            0
        }
        WM_ACTIVATE if w & 65535 == WA_INACTIVE as usize => {
            update_search_ime(false);
            if STATE.with(|c| {
                c.borrow()
                    .as_ref()
                    .is_some_and(|s| !s.moving && s.preferences.number("HideDelay") > 0)
            }) {
                unsafe {
                    ShowWindow(hwnd, SW_HIDE);
                }
            }
            0
        }
        WM_MOUSEWHEEL => {
            move_selection(if ((w >> 16) as i16) > 0 { -1 } else { 1 });
            0
        }
        WM_MBUTTONUP => {
            launch_selected();
            0
        }
        WM_ERASEBKGND => {
            let brush = STATE
                .with(|c| c.borrow().as_ref().map(|s| s.background))
                .unwrap_or_else(|| unsafe { GetSysColorBrush(COLOR_WINDOW) });
            let mut rect: RECT = unsafe { zeroed() };
            unsafe {
                GetClientRect(hwnd, &mut rect);
                FillRect(w as HDC, &rect, brush);
            }
            1
        }
        WM_PAINT => {
            paint_panel(hwnd);
            0
        }
        crate::manager::UPDATED => {
            let entries = STATE.with(|c| c.borrow_mut().as_mut().unwrap().library_updates.take());
            if let Some(entries) = entries {
                cancel_parameter();
                STATE.with(|c| {
                    let mut state = c.borrow_mut();
                    let s = state.as_mut().unwrap();
                    s.entries = entries;
                    rebuild_index(s);
                    s.selected = 0;
                });
                refresh();
            }
            0
        }
        crate::manager::GEOMETRY_CHANGED => {
            let geometry = crate::manager::window_state();
            STATE.with(|c| {
                let mut s = c.borrow_mut();
                let s = s.as_mut().unwrap();
                s.usage
                    .selection
                    .insert("manager".into(), geometry.selected);
                s.usage_writer.publish(s.usage.text());
            });
            0
        }
        WM_COMMAND => {
            if w & 65535 == ID_COPY {
                selected_action(true);
                return 0;
            }
            if w & 65535 == ID_OPEN_FOLDER {
                selected_action(false);
                return 0;
            }
            if w & 0xffff == ID_MANAGE {
                open_manager();
                return 0;
            }
            if w & 0xffff == ID_SETTINGS {
                open_settings();
                return 0;
            }
            if w & 0xffff == ID_RECENT {
                recent();
                return 0;
            }
            if w & 0xffff == ID_ABOUT {
                about();
                return 0;
            }
            if w & 0xffff == ID_RESTART || w & 0xffff == ID_CLEAN {
                lifecycle(w & 0xffff == ID_CLEAN);
                return 0;
            }
            if w & 0xffff == ID_SHOW {
                unsafe {
                    SendMessageW(hwnd, SHOW, 0, 0);
                }
                return 0;
            }
            if w & 0xffff == ID_EXIT {
                unsafe {
                    PostMessageW(hwnd, QUIT, 0, 0);
                }
                return 0;
            }
            if w & 0xffff == ID_HIDE {
                unsafe {
                    ShowWindow(hwnd, SW_HIDE);
                }
                return 0;
            }
            let id = w & 0xffff;
            let code = (w >> 16) & 0xffff;
            if id == ID_LIST
                && STATE.with(|c| c.borrow().as_ref().unwrap().parameter_entry.is_some())
            {
                return 0;
            }
            if id == ID_EDIT && code == EN_CHANGE as usize {
                let (previous, previous_selected, key, edit, number, composing) = STATE.with(|c| {
                    let mut s = c.borrow_mut();
                    let s = s.as_mut().unwrap();
                    let key = std::mem::take(&mut s.last_key);
                    (
                        s.hits.clone(),
                        s.selected,
                        key,
                        s.edit,
                        s.preferences.flag("NumberKey"),
                        s.composing,
                    )
                });
                let query = edit_text(edit);
                refresh();
                let none = STATE.with(|c| {
                    let s = c.borrow();
                    let s = s.as_ref().unwrap();
                    s.hits.is_empty() && s.parameter_entry.is_none()
                });
                if none && !composing && !previous.is_empty() {
                    let digit = match key {
                        0x30..=0x39 => Some((key - 0x30) as usize),
                        0x60..=0x69 => Some((key - 0x60) as usize),
                        _ => None,
                    };
                    if (number && digit.is_some()) || key == VK_SPACE {
                        let trimmed = query
                            .char_indices()
                            .next_back()
                            .map(|(i, _)| &query[..i])
                            .unwrap_or("");
                        unsafe {
                            SetWindowTextW(edit, wide(trimmed).as_ptr());
                        }
                        STATE.with(|c| {
                            let mut s = c.borrow_mut();
                            let s = s.as_mut().unwrap();
                            s.hits = previous;
                            s.selected = previous_selected.min(s.hits.len().saturating_sub(1));
                        });
                        if let Some(digit) = digit {
                            choose_number(digit);
                        } else {
                            launch_selected();
                        }
                    }
                }
            }
            if id == ID_LIST && code == LBN_SELCHANGE as usize {
                let list = STATE.with(|c| c.borrow().as_ref().unwrap().list);
                let selected = unsafe { SendMessageW(list, LB_GETCURSEL, 0, 0) };
                if selected >= 0 {
                    STATE.with(|c| c.borrow_mut().as_mut().unwrap().selected = selected as usize);
                    update_selection();
                    let edit = STATE.with(|c| c.borrow().as_ref().unwrap().edit);
                    unsafe {
                        SetFocus(edit);
                    }
                }
            }
            if id == ID_LIST && code == LBN_DBLCLK as usize {
                launch_selected();
            }
            0
        }
        WM_DRAWITEM => {
            let d = unsafe { &*(l as *const DRAWITEMSTRUCT) };
            if d.CtlID as usize == TITLE {
                paint_title(hwnd, d);
                return 1;
            }
            if d.CtlType == ODT_BUTTON {
                let dpi = STATE.with(|c| c.borrow().as_ref().map_or(96, |s| s.dpi));
                crate::theme_ui::button_on(d, false, theme::surface(), dpi);
                return 1;
            }
            if d.itemID == u32::MAX {
                return 1;
            }
            STATE.with(|c| {
                let s = c.borrow();
                let Some(s) = s.as_ref() else {
                    return;
                };
                let Some(&i) = s.hits.get(d.itemID as usize) else {
                    return;
                };
                let e = &s.entries[i];
                unsafe {
                    FillRect(
                        d.hDC,
                        &d.rcItem,
                        if d.itemState & ODS_SELECTED != 0 {
                            s.selected_brush
                        } else {
                            s.paper
                        },
                    );
                    if d.itemState & ODS_SELECTED != 0 {
                        let rail = RECT {
                            left: 0,
                            top: d.rcItem.top + scaled(3, s.dpi),
                            right: scaled(2, s.dpi),
                            bottom: d.rcItem.bottom - scaled(3, s.dpi),
                        };
                        FillRect(d.hDC, &rail, s.accent);
                    }
                    SetBkMode(d.hDC, TRANSPARENT as i32);
                    SetTextColor(d.hDC, theme::muted());
                    SelectObject(d.hDC, s.font);
                    let number = format!(
                        "{}{}",
                        if e.param_type.needs_input(&e.command) {
                            '*'
                        } else {
                            ' '
                        },
                        if d.itemID >= 10 {
                            ' '
                        } else {
                            char::from(b'0' + ((d.itemID + 1) % 10) as u8)
                        }
                    );
                    let a = wide(&number);
                    let b = wide(&e.alias);
                    let name = wide(&e.name);
                    TextOutW(
                        d.hDC,
                        scaled(8, s.dpi),
                        d.rcItem.top + scaled(3, s.dpi),
                        a.as_ptr(),
                        (a.len() - 1) as i32,
                    );
                    SetTextColor(d.hDC, s.font_colours[2]);
                    SelectObject(d.hDC, s.alias_font);
                    let mut rect = RECT {
                        left: scaled(34, s.dpi),
                        top: d.rcItem.top,
                        right: scaled(143, s.dpi),
                        bottom: d.rcItem.bottom,
                    };
                    DrawTextW(
                        d.hDC,
                        b.as_ptr(),
                        -1,
                        &mut rect,
                        DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS | DT_NOPREFIX,
                    );
                    SelectObject(d.hDC, s.font);
                    rect.left = scaled(150, s.dpi);
                    rect.right = d.rcItem.right - scaled(3, s.dpi);
                    DrawTextW(
                        d.hDC,
                        name.as_ptr(),
                        -1,
                        &mut rect,
                        DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS | DT_NOPREFIX,
                    );
                }
            });
            1
        }
        WM_CTLCOLORSTATIC | WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX => STATE.with(|c| {
            let state = c.borrow();
            let Some(s) = state.as_ref() else {
                return 0;
            };
            let dc = w as HDC;
            let paper = l as HWND == s.edit || l as HWND == s.list || l as HWND == s.command;
            unsafe {
                SetTextColor(
                    dc,
                    if l as HWND == s.command && s.error {
                        theme::error()
                    } else if l as HWND == s.command || l as HWND == s.empty {
                        theme::muted()
                    } else if l as HWND == s.edit {
                        s.font_colours[1]
                    } else if l as HWND == s.title {
                        s.font_colours[0]
                    } else {
                        s.font_colours[2]
                    },
                );
                SetBkColor(
                    dc,
                    if l as HWND == s.edit {
                        theme::keyword()
                    } else if paper {
                        theme::paper()
                    } else {
                        theme::surface()
                    },
                );
            }
            if l as HWND == s.edit {
                s.input_paper as LRESULT
            } else if paper {
                s.paper as LRESULT
            } else {
                s.background as LRESULT
            }
        }),
        WM_CLOSE => {
            unsafe {
                ShowWindow(hwnd, SW_HIDE);
            }
            0
        }
        QUIT => {
            if crate::manager::is_open() || crate::settings::is_open() {
                crate::manager::focus();
                unsafe {
                    MessageBoxW(
                        hwnd,
                        ui("ui.save_or_close_the_shortcut_manager_before").as_ptr(),
                        wide("ALTRunNext").as_ptr(),
                        MB_OK | MB_ICONINFORMATION,
                    );
                }
                return 0;
            }
            unsafe {
                DestroyWindow(hwnd);
            }
            0
        }
        WM_DESTROY => {
            update_search_ime(false);
            tray_icon(hwnd, NIM_DELETE);
            unsafe {
                STATE.with(|c| {
                    if let Some(s) = c.borrow().as_ref() {
                        for (id, _) in &s.hotkeys {
                            UnregisterHotKey(hwnd, *id);
                        }
                    }
                });
                PostQuitMessage(0);
            }
            0
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, w, l) },
    }
}

pub fn run() -> Result<(), String> {
    crate::localization::refresh_packs();
    crate::localization::set_active("system");
    let args: Vec<_> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--ui-scale") {
        let dpi = match args.get(i + 1).map(String::as_str) {
            Some("100") => 96,
            Some("125") => 120,
            Some("150") => 144,
            Some("200") => 192,
            _ => return Err("--ui-scale accepts 100, 125, 150 or 200".into()),
        };
        crate::window_geometry::set_test_dpi(dpi);
    }
    let data_path = if let Some(i) = args.iter().position(|a| a == "--data") {
        std::path::absolute(args.get(i + 1).ok_or("--data needs a path")?)
            .map_err(|e| e.to_string())?
    } else {
        std::env::current_exe()
            .map_err(|e| e.to_string())?
            .with_file_name("shortcuts.tsv")
    };
    let intake = args
        .iter()
        .position(|a| a == "--add")
        .map(|i| {
            args[i + 1..]
                .iter()
                .map(std::path::absolute)
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()
        .map_err(|error| error.to_string())?
        .unwrap_or_default();
    let Some(instance_lock) = crate::instance::acquire(&data_path, &intake)? else {
        return Ok(());
    };
    let (settings_file, settings_text) =
        altrun_probe::state_store::TextFile::open(&data_path.with_extension("settings.ini"))?;
    drop(settings_file);
    let preferences = altrun_probe::preferences::Preferences::parse(&settings_text)?;
    crate::localization::set_active(&preferences.get("Lang"));
    let entries = if args.iter().any(|a| a == "--data") || data_path.exists() {
        altrun_probe::library::Snapshot::open(&data_path)?.entries
    } else {
        let mut entries = parse_entries(include_str!("../assets/shortcuts.tsv"))?;
        for entry in &mut entries {
            entry.name = crate::localization::tr(&entry.name);
        }
        altrun_probe::library::Snapshot::new(data_path.clone(), entries.clone())?.save(&entries)?;
        entries
    };
    let index = SearchIndex::new(&entries);
    let (base_directory, base_is_working_directory) = working_directory(&preferences, &data_path);
    let allow_dynamic_target = preferences.flag("AllowDynamicTarget");
    let (state_file, state_text) =
        altrun_probe::state_store::TextFile::open(&data_path.with_extension("state.tsv"))?;
    let usage = altrun_probe::usage::Usage::parse(&state_text)?;
    let (settings_sender, settings_updates) = std::sync::mpsc::channel();
    let (skin_sender, skin_results) = std::sync::mpsc::channel();
    let (sound_sender, sound_tasks) = std::sync::mpsc::sync_channel::<PopupSound>(1);
    std::thread::spawn(move || {
        use windows_sys::Win32::Media::Audio::*;
        while let Ok(mut sound) = sound_tasks.recv() {
            while let Ok(newer) = sound_tasks.try_recv() {
                sound = newer;
            }
            let source = wide(&sound.path.to_string_lossy());
            unsafe {
                if PlaySoundW(
                    source.as_ptr(),
                    null_mut(),
                    SND_FILENAME | SND_NODEFAULT | SND_ASYNC,
                ) == 0
                    && sound.use_bundled_fallback
                {
                    // The asynchronous WinMM player needs the buffer to outlive playback.
                    let wav = include_bytes!("../assets/Popup.wav");
                    PlaySoundW(
                        wav.as_ptr().cast(),
                        null_mut(),
                        SND_MEMORY | SND_NODEFAULT | SND_ASYNC,
                    );
                }
            }
        }
    });
    theme::install(&preferences);
    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let instance = GetModuleHandleW(std::ptr::null());
        let class = wide("ALTRun.Native.PerformanceProbe");
        let background = CreateSolidBrush(theme::surface());
        let selected_brush = CreateSolidBrush(theme::selection());
        let paper = CreateSolidBrush(theme::paper());
        let input_paper = CreateSolidBrush(theme::keyword());
        let alias_font = make_font(13, 600, "Microsoft YaHei UI", 96);
        let accent = CreateSolidBrush(theme::accent());
        let border = CreatePen(PS_SOLID, 1, theme::border());
        let mut wc: WNDCLASSW = zeroed();
        wc.lpfnWndProc = Some(window_proc);
        wc.hInstance = instance;
        wc.lpszClassName = class.as_ptr();
        wc.hbrBackground = null_mut();
        wc.hCursor = LoadCursorW(null_mut(), IDC_ARROW);
        wc.hIcon = crate::theme_ui::app_icon(GetSystemMetrics(SM_CXICON));
        if RegisterClassW(&wc) == 0 {
            return Err("Cannot register native window".into());
        }
        let hwnd = CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
            class.as_ptr(),
            wide("ALTRunNext").as_ptr(),
            WS_POPUP | WS_BORDER | WS_CLIPCHILDREN,
            400,
            220,
            420,
            280,
            null_mut(),
            null_mut(),
            instance,
            std::ptr::null(),
        );
        if hwnd.is_null() {
            return Err("Cannot create native window".into());
        }
        crate::instance::publish(hwnd, instance_lock.identity);
        DragAcceptFiles(hwnd, 1);
        let dpi = crate::window_geometry::window_dpi(hwnd);
        let rounded = 2_i32;
        DwmSetWindowAttribute(hwnd, 33, &rounded as *const _ as *const _, 4);
        let no_transitions = 1i32;
        DwmSetWindowAttribute(hwnd, 3, &no_transitions as *const _ as _, 4);
        DwmSetWindowAttribute(hwnd, 34, &theme::border() as *const _ as *const _, 4);
        let font = CreateFontW(
            -scaled(13, dpi),
            0,
            0,
            0,
            400,
            0,
            0,
            0,
            DEFAULT_CHARSET as u32,
            OUT_DEFAULT_PRECIS as u32,
            CLIP_DEFAULT_PRECIS as u32,
            CLEARTYPE_QUALITY as u32,
            0,
            wide("Microsoft YaHei UI").as_ptr(),
        );
        let title_font = make_font(13, 600, "Microsoft YaHei UI", dpi);
        let input_font = make_font(16, 400, "Microsoft YaHei UI", dpi);
        let command_font = make_font(12, 400, "Consolas", dpi);
        let control = |class_name: &str, style: u32, x, y, width, height, id: usize| {
            CreateWindowExW(
                0,
                wide(class_name).as_ptr(),
                wide("").as_ptr(),
                WS_CHILD | WS_VISIBLE | style,
                scaled(x, dpi),
                scaled(y, dpi),
                scaled(width, dpi),
                scaled(height, dpi),
                hwnd,
                id as HMENU,
                instance,
                std::ptr::null(),
            )
        };
        let title = control("STATIC", STATIC_OWNER_DRAW, 12, 6, 396, 20, TITLE);
        let edit = control("EDIT", ES_AUTOHSCROLL as u32, 18, 35, 384, 22, ID_EDIT);
        let list = control(
            "LISTBOX",
            LBS_NOTIFY as u32
                | LBS_OWNERDRAWFIXED as u32
                | LBS_HASSTRINGS as u32
                | LBS_NOINTEGRALHEIGHT as u32
                | WS_VSCROLL,
            8,
            theme::LIST_Y,
            404,
            120,
            ID_LIST,
        );
        let command = control(
            "EDIT",
            ES_AUTOHSCROLL as u32 | ES_READONLY as u32,
            12,
            178,
            396,
            16,
            1003,
        );
        let buttons = [
            control("BUTTON", BS_OWNERDRAW as u32, 0, 0, 22, 22, ID_MANAGE),
            control("BUTTON", BS_OWNERDRAW as u32, 0, 0, 22, 22, ID_SETTINGS),
            control("BUTTON", BS_OWNERDRAW as u32, 0, 0, 22, 22, ID_HIDE),
        ];
        for (button, text) in buttons.iter().zip(["≡", "⚙", "×"]) {
            SetWindowTextW(*button, wide(text).as_ptr());
            SendMessageW(*button, WM_SETFONT, font as usize, 0);
        }
        let empty = control("STATIC", 1, 12, theme::LIST_Y + 2, 396, 20, 1004);
        SetWindowTextW(empty, ui("ui.no_matches_try_another_shortcut").as_ptr());
        if [title, edit, list, command, empty]
            .iter()
            .any(|h| h.is_null())
        {
            return Err("Cannot create native controls".into());
        }
        for child in [title, edit, list, command, empty] {
            SendMessageW(child, WM_SETFONT, font as WPARAM, 0);
        }
        SendMessageW(title, WM_SETFONT, title_font as WPARAM, 0);
        SendMessageW(edit, WM_SETFONT, input_font as WPARAM, 0);
        SendMessageW(
            edit,
            EM_SETCUEBANNER,
            0,
            ui("ui.type_a_shortcut").as_ptr() as LPARAM,
        );
        SendMessageW(command, WM_SETFONT, command_font as WPARAM, 0);
        SendMessageW(list, LB_SETITEMHEIGHT, 0, scaled(theme::ROW, dpi) as LPARAM);
        let (launch_sender, launch_results) = std::sync::mpsc::channel();
        let (library_sender, library_updates) = altrun_probe::library_updates::channel();
        let owner = hwnd as usize;
        let usage_writer =
            altrun_probe::state_store::Writer::start_notified(state_file, move || {
                PostMessageW(owner as HWND, STATE_ERROR, 0, 0);
            });
        LAUNCH_SENDER.with(|s| *s.borrow_mut() = Some(launch_sender));
        let old_edit = SetWindowLongPtrW(edit, GWLP_WNDPROC, edit_proc as *const () as isize);
        let old_list = SetWindowLongPtrW(list, GWLP_WNDPROC, list_proc as *const () as isize);
        STATE.with(|c| {
            *c.borrow_mut() = Some(State {
                sound_sender,
                input_paper,
                alias_font,
                font_colours: [theme::text(); 3],
                buttons,
                skin: None,
                skin_request: None,
                skin_sender,
                skin_results,
                preferences,
                usage,
                usage_writer,
                settings_sender,
                settings_updates,
                hotkeys: Vec::new(),
                pending_settings: None,
                generation: 0,
                current_query: String::new(),
                default_list_ready: false,
                last_key: 0,
                context: altrun_probe::launch::LaunchContext {
                    base_directory,
                    base_is_working_directory,
                    allow_dynamic_target,
                    ..Default::default()
                },
                successful: None,
                history_position: usize::MAX,
                entries,
                index,
                hits: Vec::new(),
                selected: 0,
                hwnd,
                edit,
                list,
                title,
                command,
                font,
                title_font,
                input_font,
                command_font,
                paper,
                accent,
                border,
                empty,
                enabled: !args.iter().any(|a| a == "--safe-mode")
                    && (args.iter().any(|a| a == "--enable-launch")
                        || (!cfg!(debug_assertions)
                            && !args.iter().any(|a| a == "--allow-safe-notepad"))),
                parameter_entry: None,
                saved_query: String::new(),
                error: false,
                launching: None,
                launch_results,
                background,
                selected_brush,
                old_edit,
                old_list,
                dpi,
                allow_launch: args.iter().any(|a| a == "--allow-safe-notepad"),
                composing: false,
                moving: false,
                data_path,
                library_sender,
                library_updates,
            })
        });
        STATE.with(|c| rebuild_index(c.borrow_mut().as_mut().unwrap()));
        refresh();
        let prefs = STATE.with(|c| c.borrow().as_ref().unwrap().preferences.clone());
        let mut startup_warnings = Vec::new();
        match register_bindings(
            hwnd,
            &prefs,
            &[],
            BindingUpdate::Startup(&mut startup_warnings),
        ) {
            Ok(keys) => STATE.with(|c| c.borrow_mut().as_mut().unwrap().hotkeys = keys),
            Err(error) => startup_warnings.push(error),
        }
        apply_appearance();
        if !intake.is_empty() {
            crate::manager::enqueue(intake, None);
            open_manager_window(false);
        }
        if args
            .iter()
            .any(|a| matches!(a.as_str(), "--clean" | "Clean"))
        {
            PostMessageW(hwnd, WM_COMMAND, ID_CLEAN, 0);
        }
        if args.iter().any(|a| a == "--show")
            || !startup_warnings.is_empty()
            || STATE.with(|c| {
                c.borrow()
                    .as_ref()
                    .unwrap()
                    .preferences
                    .flag("ShowMeWhenStart")
            })
        {
            SendMessageW(hwnd, SHOW, 0, 0);
        }
        if !startup_warnings.is_empty() {
            show_inline_error(&startup_warnings.join("，"));
        }
        let accelerator = ACCEL {
            fVirt: FVIRTKEY | FALT,
            key: b'S' as u16,
            cmd: ID_MANAGE as u16,
        };
        let table = CreateAcceleratorTableW(&accelerator, 1);
        let mut message: MSG = zeroed();
        loop {
            let result = GetMessageW(&mut message, null_mut(), 0, 0);
            if result <= 0 {
                break;
            }
            if TranslateAcceleratorW(hwnd, table, &message) != 0 {
                continue;
            }
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
        if !table.is_null() {
            DestroyAcceleratorTable(table);
        }
        STATE.with(|c| {
            if let Some(s) = c.borrow_mut().take() {
                for font in [
                    s.font,
                    s.title_font,
                    s.input_font,
                    s.command_font,
                    s.alias_font,
                ] {
                    DeleteObject(font);
                }
                DeleteObject(s.paper);
                DeleteObject(s.input_paper);
                DeleteObject(s.accent);
                DeleteObject(s.border);
                DeleteObject(s.selected_brush);
                DeleteObject(s.background);
            }
        });
        UnregisterClassW(class.as_ptr(), instance);
    }
    Ok(())
}

fn working_directory(
    preferences: &altrun_probe::preferences::Preferences,
    data_path: &std::path::Path,
) -> (String, bool) {
    let parent = data_path.parent().unwrap_or(std::path::Path::new("."));
    let configured = preferences.get("WorkingDirectory");
    if configured.is_empty() {
        (parent.to_string_lossy().into_owned(), false)
    } else {
        (parent.join(configured).to_string_lossy().into_owned(), true)
    }
}

fn open_manager() {
    open_manager_window(true);
}

fn open_manager_window(show: bool) {
    let context = STATE.with(|c| c.borrow().as_ref().unwrap().context.clone());
    crate::manager::configure_context(context);
    let geometry = STATE.with(|c| {
        let s = c.borrow();
        let s = s.as_ref().unwrap();
        crate::manager::WindowState {
            selected: s
                .usage
                .selection
                .get("manager")
                .cloned()
                .unwrap_or_default(),
        }
    });
    crate::manager::configure(geometry);
    let (path, entries, hwnd, sender, enabled) = STATE.with(|c| {
        let state = c.borrow();
        let s = state.as_ref().unwrap();
        (
            s.data_path.clone(),
            s.entries.clone(),
            s.hwnd,
            s.library_sender.clone(),
            s.enabled,
        )
    });
    crate::manager::open(path, entries, hwnd, sender, enabled, show);
    unsafe {
        ShowWindow(hwnd, SW_HIDE);
    }
}
fn rebuild_index(s: &mut State) {
    s.default_list_ready = false;
    let mut rows = s.entries.clone();
    for entry in &mut rows {
        entry.frequency = entry.frequency.saturating_add(
            s.usage
                .counts
                .get(&entry.id)
                .copied()
                .unwrap_or(0)
                .min(i64::MAX as u64) as i64,
        );
    }
    s.index = SearchIndex::new(&rows);
}
pub(crate) fn clipboard_text() -> Option<String> {
    use windows_sys::Win32::System::{DataExchange::*, Memory::*};
    unsafe {
        if OpenClipboard(null_mut()) == 0 {
            return None;
        }
        let handle = GetClipboardData(13);
        let result = if handle.is_null() {
            None
        } else {
            let pointer = GlobalLock(handle) as *const u16;
            if pointer.is_null() {
                None
            } else {
                let length = GlobalSize(handle) / 2;
                let result = if length > 1024 * 1024 {
                    None
                } else {
                    let slice = std::slice::from_raw_parts(pointer, length);
                    String::from_utf16(slice.split(|v| *v == 0).next().unwrap_or_default()).ok()
                };
                GlobalUnlock(handle);
                result
            }
        };
        CloseClipboard();
        result
    }
}
fn copy_text(value: &str) -> Result<(), String> {
    use windows_sys::Win32::System::{DataExchange::*, Memory::*};
    let bytes = wide(value);
    let owner = STATE.with(|c| c.borrow().as_ref().unwrap().hwnd);
    unsafe {
        let handle = GlobalAlloc(GMEM_MOVEABLE, bytes.len() * 2);
        if handle.is_null() {
            return Err("ui.could_not_allocate_clipboard_memory".into());
        }
        let pointer = GlobalLock(handle) as *mut u16;
        if pointer.is_null() {
            GlobalFree(handle);
            return Err("ui.could_not_access_clipboard_memory".into());
        }
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), pointer, bytes.len());
        GlobalUnlock(handle);
        if OpenClipboard(owner) == 0 {
            GlobalFree(handle);
            return Err("ui.another_app_is_using_the_clipboard".into());
        }
        EmptyClipboard();
        let success = !SetClipboardData(13, handle).is_null();
        CloseClipboard();
        if !success {
            GlobalFree(handle);
            return Err("ui.could_not_write_to_the_clipboard".into());
        }
    }
    Ok(())
}
fn capture_context() {
    unsafe {
        let foreground = GetForegroundWindow();
        if foreground.is_null() {
            return;
        }
        let mut process = 0;
        GetWindowThreadProcessId(foreground, &mut process);
        if process == std::process::id() {
            return;
        }
        let title = edit_text(foreground);
        let mut class = [0u16; 256];
        let n = GetClassNameW(foreground, class.as_mut_ptr(), class.len() as i32);
        STATE.with(|c| {
            let mut s = c.borrow_mut();
            let s = s.as_mut().unwrap();
            s.context.window_id = foreground as usize;
            s.context.window_title = title;
            s.context.window_class = String::from_utf16_lossy(&class[..n.max(0) as usize]);
        });
    }
}
fn recent() {
    reset_search();
    STATE.with(|c| {
        let mut s = c.borrow_mut();
        let s = s.as_mut().unwrap();
        s.default_list_ready = false;
        s.hits = s
            .usage
            .recent
            .iter()
            .filter_map(|id| {
                s.entries.iter().position(|e| {
                    &e.id == id && e.kind == altrun_probe::library::EntryKind::Command
                })
            })
            .collect();
        s.selected = 0;
    });
    render_hits();
}
fn about() {
    open_settings_page(true);
}
fn open_settings() {
    open_settings_page(false);
}
fn open_settings_page(about: bool) {
    let (path, prefs, hwnd, sender) = STATE.with(|c| {
        let s = c.borrow();
        let s = s.as_ref().unwrap();
        (
            s.data_path.clone(),
            s.preferences.clone(),
            s.hwnd,
            s.settings_sender.clone(),
        )
    });
    crate::settings::open(path, prefs, hwnd, sender, about);
    unsafe {
        ShowWindow(hwnd, SW_HIDE);
    }
}
type Bindings = Vec<(i32, Option<altrun_probe::preferences::Hotkey>)>;
enum BindingUpdate<'a> {
    Startup(&'a mut Vec<String>),
    Settings(&'a altrun_probe::preferences::Preferences),
}

fn register_bindings(
    hwnd: HWND,
    prefs: &altrun_probe::preferences::Preferences,
    old: &[(i32, Option<altrun_probe::preferences::Hotkey>)],
    mut update: BindingUpdate<'_>,
) -> Result<Bindings, String> {
    let requested = ["HotKey1", "HotKey2", "LastItemHotKey"]
        .iter()
        .map(|key| altrun_probe::preferences::parse_hotkey(&prefs.get(key)))
        .collect::<Result<Vec<_>, _>>()?;
    if requested.iter().all(Option::is_none) && !prefs.flag("ShowTrayIcon") {
        return Err("ui.keep_a_launcher_hotkey_or_the_tray".into());
    }
    for (i, value) in requested.iter().enumerate() {
        if value.is_some() && requested[..i].contains(value) {
            return Err("ui.choose_different_hotkeys_for_opening_the_launcher".into());
        }
    }
    let mut next = Vec::new();
    let mut added = Vec::new();
    for (i, key) in ["HotKey1", "HotKey2", "LastItemHotKey"].iter().enumerate() {
        let value = requested[i];
        if let Some(existing) = old
            .iter()
            .find(|(_, old_value)| value.is_some() && *old_value == value)
        {
            next.push(*existing);
            continue;
        }
        let id = (HOTKEY..HOTKEY + 20)
            .find(|id| !old.iter().any(|v| v.0 == *id) && !next.iter().any(|v| v.0 == *id))
            .ok_or("ui.no_hotkey_slot_is_available")?;
        let previously_unavailable = if let BindingUpdate::Settings(previous) = &update {
            old.get(i).is_none_or(|entry| entry.1.is_none())
                && altrun_probe::preferences::parse_hotkey(&previous.get(key))? == value
        } else {
            false
        };
        if let Some(value) = value {
            if unsafe { RegisterHotKey(hwnd, id, value.modifiers | MOD_NOREPEAT, value.key) } == 0 {
                let error = unsafe { GetLastError() };
                let binding = prefs.get(key);
                if let BindingUpdate::Startup(warnings) = &mut update {
                    eprintln!("RegisterHotKey({binding}) failed: Win32 {error}");
                    warnings.push(
                        crate::localization::tr("ui.hotkey_binding_is_unavailable")
                            .replace("{binding}", &binding),
                    );
                    next.push((id, None));
                    continue;
                }
                // Retry unchanged bindings in case the conflict has gone away,
                // but keep unrelated edits saveable while it persists.
                if previously_unavailable {
                    next.push((id, None));
                    continue;
                }
                for id in added {
                    unsafe {
                        UnregisterHotKey(hwnd, id);
                    }
                }
                if error == ERROR_HOTKEY_ALREADY_REGISTERED {
                    return Err(crate::localization::tr(
                        "ui.hotkey_binding_is_already_in_use_your",
                    )
                    .replace("{binding}", &binding));
                }
                return Err(crate::localization::message(
                    "ui.could_not_register_binding_windows_error_error",
                    &[("binding", &binding), ("error", &error.to_string())],
                ));
            }
            added.push(id);
        }
        next.push((id, value));
    }
    Ok(next)
}
fn apply_settings_messages() {
    loop {
        let change = STATE.with(|c| {
            c.borrow()
                .as_ref()
                .unwrap()
                .settings_updates
                .try_recv()
                .ok()
        });
        let Some(change) = change else {
            break;
        };
        match change {
            crate::settings::Change::Prepare(token, prefs, reply) => {
                let (hwnd, old, previous, pending) = STATE.with(|c| {
                    let s = c.borrow();
                    let s = s.as_ref().unwrap();
                    (
                        s.hwnd,
                        s.hotkeys.clone(),
                        s.preferences.clone(),
                        s.pending_settings.is_some(),
                    )
                });
                if pending {
                    let _ = reply.send(Err("ui.another_settings_change_is_in_progress".into()));
                    continue;
                }
                match register_bindings(hwnd, &prefs, &old, BindingUpdate::Settings(&previous)) {
                    Ok(keys) => {
                        STATE.with(|c| {
                            c.borrow_mut().as_mut().unwrap().pending_settings =
                                Some((token, prefs, keys))
                        });
                        let _ = reply.send(Ok(()));
                    }
                    Err(e) => {
                        let _ = reply.send(Err(e));
                    }
                }
            }
            crate::settings::Change::Commit(token) => {
                let accepted = STATE.with(|c| {
                    let mut s = c.borrow_mut();
                    let s = s.as_mut().unwrap();
                    if !s.pending_settings.as_ref().is_some_and(|p| p.0 == token) {
                        return false;
                    }
                    let (_, prefs, keys) = s.pending_settings.take().unwrap();
                    for (id, value) in &s.hotkeys {
                        if value.is_some() && !keys.iter().any(|v| v.0 == *id) {
                            unsafe {
                                UnregisterHotKey(s.hwnd, *id);
                            }
                        }
                    }
                    s.preferences = prefs;
                    (
                        s.context.base_directory,
                        s.context.base_is_working_directory,
                    ) = working_directory(&s.preferences, &s.data_path);
                    s.context.allow_dynamic_target = s.preferences.flag("AllowDynamicTarget");
                    crate::manager::configure_context(s.context.clone());
                    s.hotkeys = keys;
                    true
                });
                if accepted {
                    let language =
                        STATE.with(|c| c.borrow().as_ref().unwrap().preferences.get("Lang"));
                    crate::localization::set_active(&language);
                    let empty = STATE.with(|c| c.borrow().as_ref().unwrap().empty);
                    unsafe {
                        SetWindowTextW(empty, ui("ui.no_matches_try_another_shortcut").as_ptr());
                    }
                    apply_appearance();
                    refresh();
                    update_search_ime(true);
                }
            }
            crate::settings::Change::Cancel(token) => {
                STATE.with(|c| {
                    let mut s = c.borrow_mut();
                    let s = s.as_mut().unwrap();
                    if s.pending_settings.as_ref().is_some_and(|p| p.0 == token) {
                        let (_, _, keys) = s.pending_settings.take().unwrap();
                        for (id, value) in keys {
                            if value.is_some() && !s.hotkeys.iter().any(|v| v.0 == id) {
                                unsafe {
                                    UnregisterHotKey(s.hwnd, id);
                                }
                            }
                        }
                    }
                });
            }
        }
    }
}
fn apply_appearance() {
    let (hwnd, edit, list, title, command, prefs, dpi) = STATE.with(|c| {
        let s = c.borrow();
        let s = s.as_ref().unwrap();
        (
            s.hwnd,
            s.edit,
            s.list,
            s.title,
            s.command,
            s.preferences.clone(),
            s.dpi,
        )
    });
    theme::install(&prefs);
    unsafe {
        STATE.with(|c| {
            let mut s = c.borrow_mut();
            let s = s.as_mut().unwrap();
            let background = CreateSolidBrush(theme::surface());
            for (slot, new) in [
                (&mut s.background, background),
                (&mut s.paper, CreateSolidBrush(theme::paper())),
                (&mut s.selected_brush, CreateSolidBrush(theme::selection())),
                (&mut s.accent, CreateSolidBrush(theme::accent())),
                (&mut s.input_paper, CreateSolidBrush(theme::keyword())),
            ] {
                let old = std::mem::replace(slot, new);
                DeleteObject(old);
            }
            let old = std::mem::replace(&mut s.border, CreatePen(PS_SOLID, 1, theme::border()));
            DeleteObject(old);
        });
        STATE.with(|c| {
            let mut s = c.borrow_mut();
            let s = s.as_mut().unwrap();
            s.font_colours = [
                theme::font_colour(&prefs.get("TitleFont")),
                theme::font_colour(&prefs.get("KeywordFont")),
                theme::font_colour(&prefs.get("ListFont")),
            ];
        });
        let definitions = [
            ("ListFont", list),
            ("TitleFont", title),
            ("KeywordFont", edit),
        ];
        let mut fonts = Vec::new();
        for (key, control) in definitions {
            if let Ok(f) = altrun_probe::preferences::Font::parse(&prefs.get(key)) {
                let font = CreateFontW(
                    -scaled(f.size_dip(), dpi),
                    0,
                    0,
                    0,
                    f.weight,
                    u32::from(f.italic),
                    u32::from(f.underline),
                    u32::from(f.strike_out),
                    u32::from(f.charset),
                    0,
                    0,
                    CLEARTYPE_QUALITY as u32,
                    0,
                    wide(&f.name).as_ptr(),
                );
                if !font.is_null() {
                    SendMessageW(control, WM_SETFONT, font as usize, 1);
                    if key == "ListFont" {
                        let empty = STATE.with(|c| c.borrow().as_ref().unwrap().empty);
                        SendMessageW(empty, WM_SETFONT, font as usize, 1);
                    }
                    fonts.push((key, font));
                }
            }
        }
        STATE.with(|c| {
            let mut s = c.borrow_mut();
            let s = s.as_mut().unwrap();
            for (key, font) in fonts {
                let slot = match key {
                    "ListFont" => &mut s.font,
                    "TitleFont" => &mut s.title_font,
                    _ => &mut s.input_font,
                };
                let old = std::mem::replace(slot, font);
                DeleteObject(old);
            }
        });
        if let Ok(font) = altrun_probe::preferences::Font::parse(&prefs.get("ListFont")) {
            let alias = make_font(font.size_dip(), 600, &font.name, dpi);
            let preview = make_font(12, 400, "Consolas", dpi);
            SendMessageW(command, WM_SETFONT, preview as usize, 1);
            STATE.with(|c| {
                let mut s = c.borrow_mut();
                let s = s.as_mut().unwrap();
                DeleteObject(std::mem::replace(&mut s.alias_font, alias));
                DeleteObject(std::mem::replace(&mut s.command_font, preview));
            });
        }
        let hint = search_hint(prefs.flag("ShowOperationHint"));
        SendMessageW(edit, EM_SETCUEBANNER, 0, hint.as_ptr() as isize);
        ShowWindow(
            command,
            if prefs.flag("ShowCommandLine") {
                SW_SHOWNOACTIVATE
            } else {
                SW_HIDE
            },
        );
        let has_hotkey = STATE.with(|c| {
            let state = c.borrow();
            state
                .as_ref()
                .unwrap()
                .hotkeys
                .iter()
                .any(|h| h.1.is_some())
        });
        tray_icon(
            hwnd,
            if prefs.flag("ShowTrayIcon") || !has_hotkey {
                NIM_ADD
            } else {
                NIM_DELETE
            },
        );
        SendMessageW(
            list,
            LB_SETITEMHEIGHT,
            0,
            scaled(prefs.number("RowHeight"), dpi) as isize,
        );
    }
    load_skin();
    unsafe {
        RedrawWindow(
            hwnd,
            std::ptr::null(),
            null_mut(),
            RDW_INVALIDATE | RDW_ALLCHILDREN,
        );
    }
}
fn choose_number(digit: usize) {
    let chosen = STATE.with(|c| {
        let mut s = c.borrow_mut();
        let s = s.as_mut().unwrap();
        if s.parameter_entry.is_some() {
            return false;
        }
        let index = (digit + 9) % 10;
        if index < s.hits.len() {
            s.selected = index;
            true
        } else {
            false
        }
    });
    if chosen {
        launch_selected();
    }
}
fn parameter_history(delta: isize) -> bool {
    let value = STATE.with(|c| {
        let mut s = c.borrow_mut();
        let s = s.as_mut().unwrap();
        let index = s.parameter_entry?;
        let history = s
            .usage
            .parameters
            .get(&s.entries[index].id)
            .or_else(|| s.usage.parameters.get("*"))?;
        if history.is_empty() {
            return None;
        }
        s.history_position = if s.history_position == usize::MAX {
            0
        } else {
            advance(s.history_position, history.len(), delta)
        };
        Some((s.edit, history[s.history_position].clone()))
    });
    if let Some((edit, value)) = value {
        unsafe {
            SetWindowTextW(edit, wide(&value).as_ptr());
            SendMessageW(edit, EM_SETSEL, 0, -1);
        }
        true
    } else {
        false
    }
}
fn selected_action(copy: bool) {
    let entry = STATE.with(|c| {
        let s = c.borrow();
        let s = s.as_ref().unwrap();
        s.hits
            .get(s.selected)
            .map(|i| (s.entries[*i].clone(), s.context.clone()))
    });
    if let Some((entry, context)) = entry {
        if copy {
            if let Err(e) = copy_text(&entry.command) {
                show_inline_error(&e);
            }
        } else {
            std::thread::spawn(move || {
                let base = if entry.working_directory.is_empty() {
                    context.base_directory
                } else {
                    entry.working_directory
                };
                let directory = if entry.open_target.is_some() {
                    std::path::Path::new(&entry.command)
                        .parent()
                        .map(std::path::Path::to_path_buf)
                        .ok_or_else(|| "ui.could_not_find_the_target_s_folder".to_owned())
                } else {
                    altrun_probe::local_target::directory(
                        &entry.command,
                        std::path::Path::new(&base),
                    )
                };
                match directory {
                    Ok(parent) => {
                        let _ = execute_request(&altrun_probe::launch::LaunchRequest {
                            file: parent.to_string_lossy().into_owned(),
                            parameters: String::new(),
                        });
                    }
                    Err(e) => crate::native::show_error(&e),
                }
            });
        }
    }
}

fn load_skin() {
    let task = STATE.with(|c| {
        let mut state = c.borrow_mut();
        let s = state.as_mut().unwrap();
        let request =
            crate::skin::Request::for_settings(&s.preferences, &s.data_path, theme::surface());
        if request == s.skin_request {
            return None;
        }
        s.skin = None;
        s.skin_request = request.clone();
        request.map(|request| (request, s.skin_sender.clone(), s.hwnd as usize))
    });
    if let Some((request, sender, owner)) = task {
        std::thread::spawn(move || {
            let result = crate::skin::load(&request);
            if sender.send((request, result)).is_ok() {
                unsafe {
                    PostMessageW(owner as HWND, SKIN_DONE, 0, 0);
                }
            }
        });
    }
}
fn position_panel() {
    use crate::window_geometry::{cursor_monitor, work_area};

    let (hwnd, remembered) = STATE.with(|c| {
        let s = c.borrow();
        let s = s.as_ref().unwrap();
        let remembered = if s.preferences.get("PositionMode") == "remember" {
            s.usage
                .positions
                .get("launcher")
                .map(|&(x, y, _, _)| POINT { x, y })
        } else {
            None
        };
        (s.hwnd, remembered)
    });
    unsafe {
        let saved = remembered.and_then(|point| {
            let monitor = MonitorFromPoint(point, MONITOR_DEFAULTTONULL);
            let work = work_area(monitor)?;
            (PtInRect(&work, point) != 0).then_some((monitor, work, point))
        });
        let (monitor, work, anchor) = match saved {
            Some((monitor, work, point)) => (monitor, work, Some(point)),
            None => {
                let monitor = cursor_monitor(hwnd);
                let Some(work) = work_area(monitor) else {
                    return;
                };
                (monitor, work, None)
            }
        };
        let flags = SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE;
        if monitor != MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) {
            // Move the hidden panel first so layout uses the destination monitor.
            SetWindowPos(hwnd, null_mut(), work.left, work.top, 0, 0, flags);
            render_hits();
        }
        let mut rect = RECT::default();
        GetWindowRect(hwnd, &mut rect);
        let right = (work.right - (rect.right - rect.left)).max(work.left);
        let bottom = (work.bottom - (rect.bottom - rect.top)).max(work.top);
        let (x, y) = anchor.map_or(
            (
                work.left + (right - work.left) / 2,
                work.top + (bottom - work.top) / 2,
            ),
            |p| (p.x.clamp(work.left, right), p.y.clamp(work.top, bottom)),
        );
        if x != rect.left || y != rect.top {
            SetWindowPos(hwnd, null_mut(), x, y, 0, 0, flags);
        }
    }
}

unsafe extern "system" fn list_proc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    if msg == WM_MBUTTONUP {
        launch_selected();
        return 0;
    }
    if msg == WM_MOUSEWHEEL {
        move_selection(if ((w >> 16) as i16) > 0 { -1 } else { 1 });
        return 0;
    }
    let old = STATE.with(|c| c.borrow().as_ref().unwrap().old_list);
    let result = unsafe {
        CallWindowProcW(
            Some(std::mem::transmute::<
                isize,
                unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT,
            >(old)),
            hwnd,
            msg,
            w,
            l,
        )
    };
    if matches!(
        msg,
        WM_NCPAINT | WM_PAINT | WM_VSCROLL | WM_NCMOUSEMOVE | WM_NCLBUTTONDOWN
    ) {
        paint_scrollbar(hwnd);
    }
    result
}

fn paint_scrollbar(hwnd: HWND) {
    let Some(bar) = crate::theme_ui::vertical_scrollbar(hwnd) else {
        return;
    };
    unsafe {
        let mut window = RECT::default();
        GetWindowRect(hwnd, &mut window);
        let mut track = bar.rcScrollBar;
        OffsetRect(&mut track, -window.left, -window.top);
        let dc = GetWindowDC(hwnd);
        if dc.is_null() {
            return;
        }
        let dpi = GetDpiForWindow(hwnd).max(96) as i32;
        crate::theme_ui::draw_scrollbar(dc, track, &bar, dpi);
        ReleaseDC(hwnd, dc);
    }
}
fn parameter_menu() {
    let (hwnd, values) = STATE.with(|c| {
        let s = c.borrow();
        let s = s.as_ref().unwrap();
        let values = s
            .parameter_entry
            .and_then(|i| s.usage.parameters.get(&s.entries[i].id))
            .or_else(|| s.usage.parameters.get("*"))
            .cloned()
            .unwrap_or_default();
        (s.hwnd, values)
    });
    if values.is_empty() {
        return;
    }
    unsafe {
        let menu = CreatePopupMenu();
        for (i, value) in values.iter().take(1000).enumerate() {
            let display = value.chars().take(80).collect::<String>();
            AppendMenuW(menu, MF_STRING, 27000 + i, wide(&display).as_ptr());
        }
        let mut point: POINT = zeroed();
        GetCursorPos(&mut point);
        let selected = TrackPopupMenu(
            menu,
            TPM_RETURNCMD,
            point.x,
            point.y,
            0,
            hwnd,
            std::ptr::null(),
        );
        DestroyMenu(menu);
        if selected >= 27000
            && let Some(value) = values.get(selected as usize - 27000)
        {
            let edit = STATE.with(|c| c.borrow().as_ref().unwrap().edit);
            SetWindowTextW(edit, wide(value).as_ptr());
            SetFocus(edit);
        }
    }
}
fn popup_sound() {
    STATE.with(|c| {
        let s = c.borrow();
        let s = s.as_ref().unwrap();
        if !s.preferences.flag("PlayPopupNotify") {
            return;
        }
        let file = s.preferences.get("PopupFileName");
        let data_directory = s.data_path.parent().unwrap_or(std::path::Path::new("."));
        let sound = PopupSound {
            path: data_directory.join(&file),
            use_bundled_fallback: file
                .eq_ignore_ascii_case(altrun_probe::preferences::DEFAULT_POPUP_SOUND),
        };
        let _ = s.sound_sender.try_send(sound);
    });
}

fn lifecycle(clean: bool) {
    if crate::manager::is_open() || crate::settings::is_open() {
        show_inline_error("ui.save_and_close_the_shortcut_manager_or");
        return;
    }
    let (path, enabled, hwnd) = STATE.with(|c| {
        let s = c.borrow();
        let s = s.as_ref().unwrap();
        (s.data_path.clone(), s.enabled, s.hwnd)
    });
    if clean {
        if unsafe {
            MessageBoxW(
                hwnd,
                ui("ui.turn_off_startup_remove_the_send_to").as_ptr(),
                ui("ui.remove_windows_integrations").as_ptr(),
                MB_OKCANCEL | MB_ICONQUESTION | MB_DEFBUTTON2,
            )
        } != IDOK
        {
            return;
        }
        let owner = hwnd as usize;
        std::thread::spawn(move || {
            match altrun_probe::integration::Integration::for_data(&path)
                .and_then(|i| i.apply(false, false))
            {
                Ok(()) => unsafe {
                    PostMessageW(owner as HWND, QUIT, 0, 0);
                },
                Err(e) => show_error(&e),
            }
        });
        return;
    }
    match std::env::current_exe()
        .map_err(|e| e.to_string())
        .and_then(|exe| {
            use std::os::windows::process::CommandExt;
            let mut command = std::process::Command::new(exe);
            command
                .arg("--data")
                .arg(path)
                .arg("--wait-pid")
                .arg(std::process::id().to_string());
            if enabled {
                command.arg("--enable-launch");
            }
            command
                .creation_flags(0x08000000)
                .spawn()
                .map(|_| ())
                .map_err(|e| e.to_string())
        }) {
        Ok(()) => unsafe {
            PostMessageW(hwnd, QUIT, 0, 0);
        },
        Err(e) => show_inline_error(&e),
    }
}

fn trace_event(event: &'static str) {
    let path = STATE.with(|c| {
        let s = c.borrow();
        let s = s.as_ref().unwrap();
        s.preferences
            .flag("TraceEnable")
            .then(|| s.data_path.with_extension("diagnostic.log"))
    });
    if let Some(path) = path {
        std::thread::spawn(move || {
            use std::io::Write;
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis();
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
            {
                let _ = writeln!(f, "{stamp}\t{event}");
            }
        });
    }
}
