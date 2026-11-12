//! On-demand shortcut editor, isolated from the resident launcher's UI thread.
use crate::{
    native::{make_font, scaled, ui, wide},
    theme,
};
use altrun_probe::{
    Entry,
    library::{self, ParamType, Snapshot},
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
    UI::{
        Controls::Dialogs::*,
        Controls::*,
        HiDpi::GetDpiForWindow,
        Input::KeyboardAndMouse::*,
        Shell::{DefSubclassProc, SetWindowSubclass},
        WindowsAndMessaging::*,
    },
};

const CLASS: &str = "ALTRun.Native.ShortcutManager";
const WIDTH: i32 = crate::settings_layout::WIDTH;
const HEIGHT: i32 = crate::settings_layout::page_height(0);
const RAIL: i32 = 376;
const CAPTION: i32 = crate::settings_layout::CAPTION;
const FOOTER_TOP: i32 = HEIGHT - crate::settings_layout::FOOTER;
const FIELD_LEFT: i32 = RAIL + 41;
const FIELD_RIGHT: i32 = WIDTH - 45;
const FIELD_WIDTH: i32 = FIELD_RIGHT - FIELD_LEFT;
const HALF_FIELD: i32 = (FIELD_WIDTH - 14) / 2;
const FORM_LEFT: i32 = RAIL + 25;
const FORM_TOP: i32 = 112;
const FORM_WIDTH: i32 = WIDTH - RAIL - 51;
const COMMAND_FORM_HEIGHT: i32 = 432;
const LIST_TOP: i32 = 137;
const ROW_HEIGHT: i32 = 31;

fn list_height(dpi: i32) -> i32 {
    let row = scaled(ROW_HEIGHT, dpi);
    scaled(FOOTER_TOP - LIST_TOP - 33, dpi) / row * row
}

const FOCUS: u32 = WM_APP + 1;
pub const UPDATED: u32 = WM_APP + 6;
pub const GEOMETRY_CHANGED: u32 = WM_APP + 16;
#[derive(Clone, Default)]
pub struct WindowState {
    pub rect: Option<(i32, i32, i32, i32)>,
    pub selected: String,
}
static WINDOW_STATE: std::sync::Mutex<WindowState> = std::sync::Mutex::new(WindowState {
    rect: None,
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
const ADD: usize = 2110;
const DELETE: usize = 2111;
const SAVE: usize = 2113;
const CLOSE: usize = 2114;
const WORKDIR: usize = 2115;
const ENABLED: usize = 2116;
const ENTRY_KIND: usize = 2117;
const ADD_FILE: usize = 2120;
const ADD_FOLDER: usize = 2121;
const SEPARATOR: usize = 2122;
const REMARK: usize = 2123;
const VALIDATE: usize = 2124;
const CLEAN_MISSING: usize = 2125;
const RELATIVE: usize = 2126;
const ABSOLUTE: usize = 2127;
const EXPORT: usize = 2128;
const RESTORE: usize = 2129;
const EDIT_TEXT: usize = 2130;
const UNDO: usize = 2131;
const SORT_ALIAS: usize = 2132;
const SORT_NAME: usize = 2133;
const SORT_FREQUENCY: usize = 2134;
const MOVE_UP: usize = 2135;
const MOVE_DOWN: usize = 2136;
const TEST: usize = 2137;
const RELOAD: usize = 2139;
const TYPE_LABEL: usize = 2160;
const NOTE_LABEL: usize = 2161;
const SEPARATOR_HELP: usize = 2162;
const EMPTY_HELP: usize = 2163;
const LIST_SCROLLBAR: usize = 2164;
const TYPE_COMMAND: usize = 2150;
const TYPE_SEPARATOR: usize = 2151;
const TYPE_REMARK: usize = 2152;
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
    preferred_sort: usize,
    workdir: HWND,
    enabled: HWND,
    entry_kind: HWND,
    undo: Vec<Vec<Entry>>,
    file: Snapshot,
    draft: Vec<Entry>,
    selected: usize,
    dirty: bool,
    loading: bool,
    laid_out_kind: Option<Option<library::EntryKind>>,
    list: HWND,
    alias: HWND,
    name: HWND,
    command: HWND,
    kind: HWND,
    status: HWND,
    save: HWND,
    delete: HWND,
    font: HFONT,
    heading: HFONT,
    background: HBRUSH,
    paper: HBRUSH,
    selection: HBRUSH,
    dpi: i32,
    error: bool,
    parent: usize,
    sender: altrun_probe::library_updates::LibraryWriter,
}
thread_local! { static EDITOR: RefCell<Option<Editor>> = const { RefCell::new(None) }; }

pub fn is_open() -> bool {
    OPEN.load(Ordering::Acquire)
}
pub fn focus() {
    let hwnd = WINDOW.load(Ordering::Acquire);
    if hwnd != 0 {
        unsafe {
            PostMessageW(hwnd as HWND, FOCUS, 0, 0);
        }
    }
}
pub fn open(
    path: PathBuf,
    initial: Vec<Entry>,
    parent: HWND,
    sender: altrun_probe::library_updates::LibraryWriter,
    launch_enabled: bool,
) {
    if OPEN.swap(true, Ordering::AcqRel) {
        focus();
        return;
    }
    let owner = parent as usize;
    std::thread::spawn(move || {
        if let Err(error) = run(path, initial, owner, sender, launch_enabled) {
            unsafe {
                MessageBoxW(
                    null_mut(),
                    wide(&error).as_ptr(),
                    ui("无法打开快捷项管理").as_ptr(),
                    MB_OK | MB_ICONERROR,
                );
            }
        }
        WINDOW.store(0, Ordering::Release);
        OPEN.store(false, Ordering::Release);
    });
}
fn text(hwnd: HWND) -> String {
    unsafe {
        let mut buffer = vec![0; GetWindowTextLengthW(hwnd) as usize + 1];
        let n = GetWindowTextW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32);
        String::from_utf16_lossy(&buffer[..n as usize])
    }
}
fn status(message: &str, error: bool) {
    let (hwnd, color_changed) = EDITOR.with(|c| {
        let mut state = c.borrow_mut();
        let s = state.as_mut().unwrap();
        let color_changed = s.error != error;
        s.error = error;
        (s.status, color_changed)
    });
    let message = crate::localization::tr(message);
    unsafe {
        if text(hwnd) != message {
            SetWindowTextW(hwnd, wide(&message).as_ptr());
        } else if color_changed {
            InvalidateRect(hwnd, std::ptr::null(), 1);
        }
    }
}
fn sync_fields() {
    let (alias, name, command, kind, save, delete, entry, dirty) = EDITOR.with(|c| {
        let mut state = c.borrow_mut();
        let s = state.as_mut().unwrap();
        s.loading = true;
        (
            s.alias,
            s.name,
            s.command,
            s.kind,
            s.save,
            s.delete,
            s.draft.get(s.selected).cloned(),
            s.dirty,
        )
    });
    unsafe {
        for hwnd in [alias, name, command, kind, delete] {
            EnableWindow(hwnd, i32::from(entry.is_some()));
        }
        let (workdir, enabled, entry_kind) = EDITOR.with(|c| {
            let s = c.borrow();
            let s = s.as_ref().unwrap();
            (s.workdir, s.enabled, s.entry_kind)
        });
        for (field, value) in [
            (
                workdir,
                entry.as_ref().map_or("", |e| e.working_directory.as_str()),
            ),
            (alias, entry.as_ref().map_or("", |e| e.alias.as_str())),
            (name, entry.as_ref().map_or("", |e| e.name.as_str())),
            (command, entry.as_ref().map_or("", |e| e.command.as_str())),
        ] {
            if text(field) != value {
                SetWindowTextW(field, wide(value).as_ptr());
            }
        }
        SendMessageW(
            enabled,
            BM_SETCHECK,
            usize::from(entry.as_ref().is_some_and(|e| e.enabled)),
            0,
        );
        SendMessageW(
            entry_kind,
            CB_SETCURSEL,
            entry.as_ref().map_or(0, |e| e.kind as usize),
            0,
        );
        let index = entry.map_or(0, |e| e.param_type as usize);
        SendMessageW(kind, CB_SETCURSEL, index, 0);
        EnableWindow(save, i32::from(dirty));
    }
    EDITOR.with(|c| c.borrow_mut().as_mut().unwrap().loading = false);
    sync_kind_layout();
}
fn refresh_list() {
    let (list, labels, selected) = EDITOR.with(|c| {
        let s = c.borrow();
        let s = s.as_ref().unwrap();
        (
            s.list,
            s.draft
                .iter()
                .map(|e| format!("{} · {}", e.alias, e.name))
                .collect::<Vec<_>>(),
            s.selected,
        )
    });
    unsafe {
        SendMessageW(list, WM_SETREDRAW, 0, 0);
        SendMessageW(list, LB_RESETCONTENT, 0, 0);
        for label in &labels {
            SendMessageW(list, LB_ADDSTRING, 0, wide(label).as_ptr() as isize);
        }
        SendMessageW(list, WM_SETREDRAW, 1, 0);
        if !labels.is_empty() {
            SendMessageW(list, LB_SETCURSEL, selected, 0);
        }
        InvalidateRect(list, std::ptr::null(), 0);
    }
    sync_fields();
}
fn update_draft() {
    let handles = EDITOR.with(|c| {
        let s = c.borrow();
        let s = s.as_ref()?;
        if s.loading {
            None
        } else {
            Some((s.alias, s.name, s.command, s.kind))
        }
    });
    let Some((alias, name, command, kind)) = handles else {
        return;
    };
    let values = (text(alias), text(name), text(command), unsafe {
        SendMessageW(kind, CB_GETCURSEL, 0, 0)
    });
    let (extra, enabled, entry_kind) = EDITOR.with(|c| {
        let s = c.borrow();
        let s = s.as_ref().unwrap();
        (
            text(s.workdir),
            unsafe { SendMessageW(s.enabled, BM_GETCHECK, 0, 0) != 0 },
            unsafe { SendMessageW(s.entry_kind, CB_GETCURSEL, 0, 0) },
        )
    });
    let change = EDITOR.with(|c| {
        let mut state = c.borrow_mut();
        let s = state.as_mut().unwrap();
        let entry = s.draft.get_mut(s.selected)?;
        let row_changed = entry.alias != values.0 || entry.name != values.1;
        entry.working_directory = extra;
        entry.enabled = enabled;
        entry.kind = match entry_kind {
            1 => library::EntryKind::Separator,
            2 => library::EntryKind::Remark,
            _ => library::EntryKind::Command,
        };
        entry.alias = values.0;
        entry.name = values.1;
        entry.command = values.2;
        entry.param_type = match values.3 {
            1 => ParamType::None,
            2 => ParamType::Raw,
            3 => ParamType::AnsiUrl,
            4 => ParamType::Utf8Url,
            _ => ParamType::Auto,
        };
        let first_edit = !s.dirty;
        s.dirty = true;
        Some((s.list, s.selected, s.save, first_edit, row_changed))
    });
    let Some((list, selected, save, first_edit, row_changed)) = change else {
        return;
    };
    unsafe {
        if first_edit {
            EnableWindow(save, 1);
        }
        if row_changed {
            let mut row = RECT::default();
            if SendMessageW(list, LB_GETITEMRECT, selected, &mut row as *mut _ as LPARAM)
                != LB_ERR as isize
            {
                InvalidateRect(list, &row, 0);
            }
        }
    }
    sync_kind_layout();
    status("尚未保存。保存后立即更新启动面板。", false);
}
fn save() {
    let duplicates = EDITOR.with(|c| {
        let s = c.borrow();
        let s = s.as_ref().unwrap();
        let mut seen = std::collections::HashSet::new();
        s.draft
            .iter()
            .filter(|e| {
                e.kind == library::EntryKind::Command
                    && !seen.insert(e.alias.to_lowercase())
                    && !s
                        .file
                        .entries
                        .iter()
                        .any(|old| old.id == e.id && old.alias == e.alias)
            })
            .map(|e| e.alias.clone())
            .collect::<Vec<_>>()
    });
    if !duplicates.is_empty()
        && !confirm(
            WINDOW.load(Ordering::Acquire) as HWND,
            &format!("以下快捷字重复：{}。仍然保存？", duplicates.join("、")),
        )
    {
        return;
    }
    status("正在保存…", false);
    let result = EDITOR.with(|c| {
        let mut state = c.borrow_mut();
        let s = state.as_mut().unwrap();
        match s.file.save(&s.draft) {
            Ok(()) => {
                s.dirty = false;
                Ok((s.sender.clone(), s.draft.clone(), s.parent))
            }
            Err(error) => Err(error),
        }
    });
    match result {
        Ok((sender, entries, parent)) => {
            sender.saved(entries);
            unsafe {
                PostMessageW(parent as HWND, UPDATED, 0, 0);
            }
            status("已保存。启动面板已更新，上一版保存在 .bak 文件。", false);
            sync_fields();
        }
        Err(error) => status(&error, true),
    }
}
fn confirm(hwnd: HWND, message: &str) -> bool {
    crate::input_dialog::confirm(hwnd, "管理快捷项", message)
}
unsafe extern "system" fn procedure(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    if msg == WM_DESTROY {
        unsafe {
            PostQuitMessage(0);
        }
        return 0;
    }
    if msg == FOCUS {
        apply_intake(hwnd);
        unsafe {
            ShowWindow(hwnd, SW_SHOWNORMAL);
            SetForegroundWindow(hwnd);
        }
        return 0;
    }
    if msg == WM_CLOSE {
        let dirty = EDITOR.with(|c| c.borrow().as_ref().is_some_and(|s| s.dirty));
        if !dirty || confirm(hwnd, "有尚未保存的修改。放弃修改并关闭？") {
            publish_geometry(hwnd);
            unsafe {
                DestroyWindow(hwnd);
            }
        }
        return 0;
    }
    let ready = EDITOR.with(|c| c.borrow().is_some());
    if !ready {
        return unsafe { DefWindowProcW(hwnd, msg, w, l) };
    }
    match msg {
        WM_ERASEBKGND => 1,
        WM_ACTIVATE => {
            unsafe {
                InvalidateRect(GetDlgItem(hwnd, CAPTION_CLOSE as i32), std::ptr::null(), 1);
            }
            0
        }
        WM_NCHITTEST => {
            let result = unsafe { DefWindowProcW(hwnd, msg, w, l) };
            if result == HTCLIENT as isize {
                let mut point = POINT {
                    x: l as i16 as i32,
                    y: (l >> 16) as i16 as i32,
                };
                unsafe {
                    ScreenToClient(hwnd, &mut point);
                }
                let dpi = EDITOR.with(|c| c.borrow().as_ref().unwrap().dpi);
                if (0..scaled(CAPTION, dpi)).contains(&point.y) && point.x < scaled(WIDTH - 46, dpi)
                {
                    return HTCAPTION as isize;
                }
            }
            result
        }
        WM_PAINT => {
            paint_manager(hwnd);
            0
        }
        WM_EXITSIZEMOVE => {
            publish_geometry(hwnd);
            0
        }
        WM_DROPFILES => {
            let paths = drop_paths(w as _);
            add_paths(paths);
            0
        }
        WM_CONTEXTMENU if w as HWND == EDITOR.with(|c| c.borrow().as_ref().unwrap().list) => {
            popup(hwnd, LIST, Some(l));
            0
        }
        WM_COMMAND => {
            let id = w & 0xffff;
            let notification = (w >> 16) & 0xffff;
            if id == LIST && notification == LBN_SELCHANGE as usize {
                let list = EDITOR.with(|c| c.borrow().as_ref().unwrap().list);
                let selected = unsafe { SendMessageW(list, LB_GETCURSEL, 0, 0) };
                if selected >= 0 {
                    EDITOR.with(|c| c.borrow_mut().as_mut().unwrap().selected = selected as usize);
                    sync_fields();
                }
            } else if [ALIAS, NAME, COMMAND, WORKDIR].contains(&id)
                && notification == EN_CHANGE as usize
                || [KIND, ENTRY_KIND].contains(&id) && notification == CBN_SELCHANGE as usize
                || id == ENABLED && notification == BN_CLICKED as usize
            {
                update_draft();
            } else if notification == BN_CLICKED as usize {
                match id {
                    TYPE_COMMAND | TYPE_SEPARATOR | TYPE_REMARK => {
                        let kind = EDITOR.with(|c| c.borrow().as_ref().unwrap().entry_kind);
                        unsafe {
                            SendMessageW(kind, CB_SETCURSEL, id - TYPE_COMMAND, 0);
                        }
                        update_draft();
                    }
                    CAPTION_CLOSE => unsafe {
                        PostMessageW(hwnd, WM_CLOSE, 0, 0);
                    },
                    NEW_MENU | TOOLS_MENU => popup(hwnd, id, None),
                    ADD => {
                        EDITOR.with(|c| {
                            let mut state = c.borrow_mut();
                            let s = state.as_mut().unwrap();
                            s.draft.push(Entry {
                                alias: String::new(),
                                name: String::new(),
                                command: String::new(),
                                frequency: 0,
                                param_type: ParamType::None,
                                id: library::new_id(),
                                working_directory: String::new(),
                                enabled: true,
                                kind: library::EntryKind::Command,
                            });
                            s.selected = s.draft.len() - 1;
                            s.dirty = true;
                        });
                        refresh_list();
                        status("新增项尚未保存，请填写快捷字和命令。", false);
                        let field = EDITOR.with(|c| c.borrow().as_ref().unwrap().alias);
                        unsafe {
                            SetFocus(field);
                        }
                    }
                    DELETE => {
                        let name = EDITOR.with(|c| {
                            let s = c.borrow();
                            let s = s.as_ref().unwrap();
                            s.draft
                                .get(s.selected)
                                .map(|e| format!("{} · {}", e.alias, e.name))
                        });
                        if let Some(name) = name
                            && confirm(hwnd, &format!("从草稿中删除“{name}”？保存后生效。"))
                        {
                            EDITOR.with(|c| {
                                let mut state = c.borrow_mut();
                                let s = state.as_mut().unwrap();
                                s.undo.push(s.draft.clone());
                                if s.undo.len() > 20 {
                                    s.undo.remove(0);
                                }
                                s.draft.remove(s.selected);
                                s.selected = s.selected.min(s.draft.len().saturating_sub(1));
                                s.dirty = true;
                            });
                            refresh_list();
                            status("已从草稿删除，尚未保存。", false);
                        }
                    }
                    SAVE => save(),
                    CLOSE => unsafe {
                        PostMessageW(hwnd, WM_CLOSE, 0, 0);
                    },
                    ADD_FILE | ADD_FOLDER | SEPARATOR | REMARK | VALIDATE | CLEAN_MISSING
                    | RELATIVE | ABSOLUTE | EXPORT | RESTORE | EDIT_TEXT | UNDO | SORT_ALIAS
                    | SORT_NAME | SORT_FREQUENCY | MOVE_UP | MOVE_DOWN | TEST | RELOAD => {
                        tool(hwnd, id)
                    }
                    _ => {}
                }
            }
            0
        }
        WM_MEASUREITEM if l != 0 => {
            let item = unsafe { &mut *(l as *mut MEASUREITEMSTRUCT) };
            if crate::manager_menu::measure(item) {
                1
            } else {
                unsafe { DefWindowProcW(hwnd, msg, w, l) }
            }
        }
        WM_DRAWITEM if l != 0 => {
            let d = unsafe { &*(l as *const DRAWITEMSTRUCT) };
            if crate::manager_menu::draw(d) {
                return 1;
            }
            if d.CtlType == ODT_BUTTON {
                if d.CtlID as usize == CAPTION_CLOSE {
                    crate::theme_ui::caption_close(
                        d,
                        EDITOR.with(|c| c.borrow().as_ref().unwrap().dpi),
                    );
                } else {
                    let selected_type = EDITOR.with(|c| {
                        let state = c.borrow();
                        let s = state.as_ref().unwrap();
                        s.draft.get(s.selected).map(|e| match e.kind {
                            library::EntryKind::Command => TYPE_COMMAND,
                            library::EntryKind::Separator => TYPE_SEPARATOR,
                            library::EntryKind::Remark => TYPE_REMARK,
                        })
                    });
                    if [TYPE_COMMAND, TYPE_SEPARATOR, TYPE_REMARK].contains(&(d.CtlID as usize)) {
                        let selected = selected_type == Some(d.CtlID as usize);
                        let color = if selected {
                            theme::selection()
                        } else {
                            theme::paper()
                        };
                        let dpi = EDITOR.with(|c| c.borrow().as_ref().unwrap().dpi);
                        crate::theme_ui::fill(d.hDC, &d.rcItem, theme::paper());
                        crate::theme_ui::rounded(
                            d.hDC,
                            d.rcItem,
                            scaled(8, dpi),
                            color,
                            if d.itemState & ODS_FOCUS != 0 {
                                theme::accent()
                            } else if selected {
                                theme::border()
                            } else {
                                color
                            },
                        );
                        unsafe {
                            let font = SendMessageW(d.hwndItem, WM_GETFONT, 0, 0) as HFONT;
                            let old = SelectObject(d.hDC, font);
                            crate::theme_ui::line_text(
                                d.hDC,
                                d.rcItem,
                                &text(d.hwndItem),
                                theme::text(),
                                DT_CENTER,
                            );
                            SelectObject(d.hDC, old);
                        }
                    } else {
                        let background = if [DELETE, CLOSE, SAVE, NEW_MENU, TOOLS_MENU]
                            .contains(&(d.CtlID as usize))
                        {
                            theme::chrome()
                        } else {
                            theme::surface()
                        };
                        let dpi = EDITOR.with(|c| c.borrow().as_ref().unwrap().dpi);
                        crate::theme_ui::button_on(d, d.CtlID as usize == SAVE, background, dpi);
                    }
                }
                return 1;
            }
            if d.CtlType == ODT_COMBOBOX {
                let dpi = EDITOR.with(|c| c.borrow().as_ref().unwrap().dpi);
                crate::theme_ui::combo(d, dpi);
                return 1;
            }
            EDITOR.with(|c| {
                let state = c.borrow();
                let s = state.as_ref().unwrap();
                let Some(entry) = s.draft.get(d.itemID as usize) else {
                    return;
                };
                unsafe {
                    FillRect(
                        d.hDC,
                        &d.rcItem,
                        if d.itemState & ODS_SELECTED != 0 {
                            s.selection
                        } else {
                            s.paper
                        },
                    );
                    if d.itemState & ODS_SELECTED != 0 {
                        crate::theme_ui::fill(
                            d.hDC,
                            &RECT {
                                left: d.rcItem.left + scaled(3, s.dpi),
                                top: d.rcItem.top + scaled(5, s.dpi),
                                right: d.rcItem.left + scaled(6, s.dpi),
                                bottom: d.rcItem.bottom - scaled(5, s.dpi),
                            },
                            theme::accent(),
                        );
                    }
                    SetBkMode(d.hDC, TRANSPARENT as i32);
                    SetTextColor(d.hDC, theme::text());
                    SelectObject(d.hDC, s.font);
                    let mut rect = d.rcItem;
                    rect.left += scaled(8, s.dpi);
                    rect.right = d.rcItem.right * 42 / 100;
                    DrawTextW(
                        d.hDC,
                        wide(if entry.alias.is_empty() {
                            "新快捷项"
                        } else {
                            &entry.alias
                        })
                        .as_ptr(),
                        -1,
                        &mut rect,
                        DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS | DT_NOPREFIX,
                    );
                    rect.left = d.rcItem.right * 42 / 100 + scaled(6, s.dpi);
                    rect.right = d.rcItem.right - scaled(6, s.dpi);
                    SetTextColor(d.hDC, theme::muted());
                    DrawTextW(
                        d.hDC,
                        wide(&entry.name).as_ptr(),
                        -1,
                        &mut rect,
                        DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS | DT_NOPREFIX,
                    );
                }
            });
            1
        }
        WM_CTLCOLORSTATIC | WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX => EDITOR.with(|c| {
            let state = c.borrow();
            let s = state.as_ref().unwrap();
            unsafe {
                let id = GetDlgCtrlID(l as HWND) as usize;
                let form = (2170..=2174).contains(&id)
                    || [TYPE_LABEL, NOTE_LABEL, SEPARATOR_HELP].contains(&id);
                let color = if msg != WM_CTLCOLORSTATIC {
                    theme::paper()
                } else if form {
                    theme::control_surface()
                } else if l as HWND == s.status {
                    theme::chrome()
                } else {
                    theme::surface()
                };
                SetTextColor(
                    w as HDC,
                    if l as HWND == s.status && s.error {
                        theme::error()
                    } else {
                        theme::text()
                    },
                );
                SetBkColor(w as HDC, color);
                SetDCBrushColor(w as HDC, color);
                GetStockObject(DC_BRUSH) as LRESULT
            }
        }),
        _ => unsafe { DefWindowProcW(hwnd, msg, w, l) },
    }
}
fn run(
    path: PathBuf,
    initial: Vec<Entry>,
    parent: usize,
    sender: altrun_probe::library_updates::LibraryWriter,
    launch_enabled: bool,
) -> Result<(), String> {
    if !path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("tsv"))
    {
        return Err("请选择 ALTRun 的 TSV 快捷项数据文件。".into());
    }
    let file = if path.exists() {
        Snapshot::open(&path)?
    } else {
        Snapshot::new(path, initial)?
    };
    let (_, text) =
        altrun_probe::state_store::TextFile::open(&file.path.with_extension("settings.ini"))?;
    let prefs = altrun_probe::preferences::Preferences::parse(&text)?;
    theme::install_chrome(&prefs);
    crate::localization::install(&prefs.get("Lang"));
    unsafe {
        let instance = GetModuleHandleW(std::ptr::null());
        let background = CreateSolidBrush(theme::surface());
        let mut class: WNDCLASSW = zeroed();
        class.lpfnWndProc = Some(procedure);
        class.hInstance = instance;
        let class_name = wide(CLASS);
        class.lpszClassName = class_name.as_ptr();
        class.hbrBackground = null_mut();
        class.hCursor = LoadCursorW(null_mut(), IDC_ARROW);
        RegisterClassW(&class);
        let hwnd = CreateWindowExW(
            WS_EX_CONTROLPARENT,
            class_name.as_ptr(),
            ui("ALTRun · 管理快捷项").as_ptr(),
            WS_POPUP | WS_SYSMENU | WS_CLIPCHILDREN,
            450,
            180,
            760,
            440,
            null_mut(),
            null_mut(),
            instance,
            std::ptr::null(),
        );
        if hwnd.is_null() {
            DeleteObject(background);
            return Err("无法创建管理窗口".into());
        }
        theme::frame(hwnd);
        let remembered = window_state();
        let position = remembered.rect.map(|(x, y, _, _)| (x, y)).or_else(|| {
            let defaults = altrun_probe::preferences::Preferences::default();
            let x = prefs.number("ManWinLeft");
            let y = prefs.number("ManWinTop");
            (x != defaults.number("ManWinLeft") || y != defaults.number("ManWinTop"))
                .then_some((x, y))
        });
        let dpi = crate::window_geometry::fit(hwnd, WIDTH, HEIGHT, position);
        let font = make_font(14, 400, "Microsoft YaHei UI", dpi);
        let heading = make_font(27, 600, "Microsoft YaHei UI", dpi);
        let control = |class: &str, label: &str, style: u32, x, y, width, height, id: usize| {
            let id = if class == "STATIC" && id == 0 {
                match label {
                    "快捷字" => 2170,
                    "名称" => 2171,
                    "命令或路径" => 2172,
                    "参数类型" => 2173,
                    "工作目录（留空使用默认）" => 2174,
                    _ => id,
                }
            } else {
                id
            };
            let style = if class == "BUTTON" && style & 15 != BS_AUTOCHECKBOX as u32 {
                (style & !15) | BS_OWNERDRAW as u32
            } else if class == "COMBOBOX" {
                style | (CBS_OWNERDRAWFIXED | CBS_HASSTRINGS) as u32
            } else if class == "EDIT" {
                style & !WS_BORDER
            } else {
                style
            };
            let (x, y, width, height) = match id {
                CAPTION_CLOSE => (WIDTH - 46, 0, 46, CAPTION),
                NEW_MENU => (RAIL - 180, 62, 78, 32),
                TOOLS_MENU => (RAIL - 94, 62, 78, 32),
                LIST => (17, LIST_TOP, RAIL - 34, 0),
                DELETE => (16, FOOTER_TOP + 14, 72, 34),
                CLOSE => (WIDTH - 176, FOOTER_TOP + 14, 72, 34),
                SAVE => (WIDTH - 96, FOOTER_TOP + 14, 80, 34),
                TYPE_COMMAND | TYPE_SEPARATOR | TYPE_REMARK => {
                    let index = match id {
                        TYPE_COMMAND => 0,
                        TYPE_SEPARATOR => 1,
                        _ => 2,
                    };
                    let width = (FIELD_WIDTH - 14) / 3;
                    (FIELD_LEFT + 4 + index * (width + 3), 157, width, 29)
                }
                TYPE_LABEL => (FIELD_LEFT, 126, FIELD_WIDTH, 21),
                NOTE_LABEL => (FIELD_LEFT, 207, FIELD_WIDTH, 21),
                SEPARATOR_HELP | EMPTY_HELP => (FIELD_LEFT, 207, FIELD_WIDTH, 40),
                ALIAS => (FIELD_LEFT, 233, HALF_FIELD, 36),
                NAME => (FIELD_RIGHT - HALF_FIELD, 233, HALF_FIELD, 36),
                COMMAND => (FIELD_LEFT, 317, FIELD_WIDTH, 36),
                KIND => (FIELD_LEFT, 401, FIELD_WIDTH - 144, 220),
                ENABLED => (FIELD_RIGHT - 128, 401, 128, 36),
                WORKDIR => (FIELD_LEFT, 485, FIELD_WIDTH, 36),
                ENTRY_KIND => (FIELD_LEFT, 153, FIELD_WIDTH, 120),
                _ if label == "修改后点击保存。" => (100, FOOTER_TOP + 19, 600, 24),
                _ if label == "管理快捷项" => (RAIL + 25, 60, FIELD_WIDTH, 41),
                _ if label == "快捷字" => (FIELD_LEFT, 207, HALF_FIELD, 21),
                _ if label == "名称" => (FIELD_RIGHT - HALF_FIELD, 207, HALF_FIELD, 21),
                _ if label == "命令或路径" => (FIELD_LEFT, 291, FIELD_WIDTH, 21),
                _ if label == "参数类型" => (FIELD_LEFT, 375, FIELD_WIDTH - 144, 21),
                _ if label == "工作目录（留空使用默认）" => {
                    (FIELD_LEFT, 459, FIELD_WIDTH, 21)
                }
                _ => (x, y, width, height),
            };
            let child = CreateWindowExW(
                0,
                wide(class).as_ptr(),
                ui(if label == "工作目录（留空使用默认）" {
                    "工作目录"
                } else {
                    label
                })
                .as_ptr(),
                WS_CHILD | WS_VISIBLE | style,
                scaled(x, dpi),
                scaled(y, dpi),
                (scaled(x + width, dpi) - scaled(x, dpi)).max(1),
                if id == LIST {
                    list_height(dpi)
                } else {
                    (scaled(y + height, dpi) - scaled(y, dpi)).max(1)
                },
                hwnd,
                id as HMENU,
                instance,
                std::ptr::null(),
            );
            SendMessageW(child, WM_SETFONT, font as WPARAM, 0);
            if class == "COMBOBOX" {
                SendMessageW(
                    child,
                    CB_SETITEMHEIGHT,
                    usize::MAX,
                    scaled(28, dpi) as isize,
                );
                SendMessageW(child, CB_SETITEMHEIGHT, 0, scaled(34, dpi) as isize);
                crate::theme_ui::style_combo(child, dpi);
            } else if class == "EDIT" {
                crate::theme_ui::style_edit(child, dpi, true);
                if id == WORKDIR {
                    SendMessageW(
                        child,
                        EM_SETCUEBANNER,
                        0,
                        ui("可留空，自动处理").as_ptr() as isize,
                    );
                }
            }
            child
        };
        let close_caption = control("BUTTON", "关闭", WS_TABSTOP, 0, 0, 0, 0, CAPTION_CLOSE);
        crate::theme_ui::style_caption_close(close_caption);
        let title = control("STATIC", "管理快捷项", 0, 280, 12, 300, 25, 0);
        control("BUTTON", "新增 ▾", WS_TABSTOP, 12, 12, 116, 30, NEW_MENU);
        control("BUTTON", "工具 ▾", WS_TABSTOP, 138, 12, 124, 30, TOOLS_MENU);
        SetWindowTextW(title, ui("编辑快捷项").as_ptr());
        SendMessageW(title, WM_SETFONT, heading as WPARAM, 0);
        let list = control(
            "LISTBOX",
            "",
            WS_VSCROLL
                | WS_CLIPSIBLINGS
                | WS_TABSTOP
                | LBS_NOTIFY as u32
                | LBS_OWNERDRAWFIXED as u32
                | LBS_HASSTRINGS as u32
                | LBS_NOINTEGRALHEIGHT as u32,
            12,
            52,
            250,
            304,
            LIST,
        );
        SendMessageW(list, LB_SETITEMHEIGHT, 0, scaled(ROW_HEIGHT, dpi) as LPARAM);
        let scrollbar = CreateWindowExW(
            0,
            wide("STATIC").as_ptr(),
            std::ptr::null(),
            WS_CHILD | WS_CLIPSIBLINGS,
            0,
            0,
            0,
            0,
            hwnd,
            LIST_SCROLLBAR as HMENU,
            instance,
            std::ptr::null(),
        );
        if !scrollbar.is_null()
            && SetWindowSubclass(scrollbar, Some(scrollbar_proc), 1, list as usize) != 0
        {
            SetWindowSubclass(list, Some(list_scrollbar_proc), 1, scrollbar as usize);
        }
        control("STATIC", "快捷字", 0, 280, 52, 210, 20, 0);
        control("STATIC", "名称", 0, 512, 52, 232, 20, 0);
        let alias = control(
            "EDIT",
            "",
            WS_BORDER | WS_TABSTOP | ES_AUTOHSCROLL as u32,
            280,
            76,
            216,
            28,
            ALIAS,
        );
        let name = control(
            "EDIT",
            "",
            WS_BORDER | WS_TABSTOP | ES_AUTOHSCROLL as u32,
            512,
            76,
            232,
            28,
            NAME,
        );
        control("STATIC", "命令或路径", 0, 280, 118, 464, 20, 0);
        let command = control(
            "EDIT",
            "",
            WS_BORDER | WS_TABSTOP | ES_AUTOHSCROLL as u32,
            280,
            142,
            464,
            28,
            COMMAND,
        );
        control("STATIC", "参数类型", 0, 280, 184, 464, 20, 0);
        let kind = control(
            "COMBOBOX",
            "",
            WS_TABSTOP | CBS_DROPDOWNLIST as u32 | WS_VSCROLL,
            280,
            208,
            300,
            220,
            KIND,
        );
        for label in [
            "自动检测参数",
            "无参数",
            "原样参数",
            "Windows ANSI URL 编码",
            "UTF-8 URL 编码",
        ] {
            SendMessageW(
                kind,
                CB_ADDSTRING,
                0,
                wide(&crate::localization::tr(label)).as_ptr() as LPARAM,
            );
        }
        control(
            "STATIC",
            "工作目录（留空使用默认）",
            0,
            280,
            245,
            464,
            18,
            0,
        );
        let workdir = control(
            "EDIT",
            "",
            WS_BORDER | WS_TABSTOP | ES_AUTOHSCROLL as u32,
            280,
            266,
            464,
            24,
            WORKDIR,
        );
        let enabled = control(
            "BUTTON",
            "启用",
            BS_AUTOCHECKBOX as u32 | WS_TABSTOP,
            596,
            208,
            145,
            24,
            ENABLED,
        );
        crate::theme_ui::switch(enabled, dpi, false);
        let entry_kind = control(
            "COMBOBOX",
            "",
            CBS_DROPDOWNLIST as u32 | WS_TABSTOP,
            280,
            294,
            200,
            120,
            ENTRY_KIND,
        );
        for label in ["启动命令", "分隔行", "备注"] {
            SendMessageW(
                entry_kind,
                CB_ADDSTRING,
                0,
                wide(&crate::localization::tr(label)).as_ptr() as isize,
            );
        }
        ShowWindow(entry_kind, SW_HIDE);
        control("STATIC", "类型", 0, 0, 0, 0, 0, TYPE_LABEL);
        control("STATIC", "备注内容", 0, 0, 0, 0, 0, NOTE_LABEL);
        control(
            "STATIC",
            "在启动面板中分隔相邻的快捷项。",
            0,
            0,
            0,
            0,
            0,
            SEPARATOR_HELP,
        );
        control(
            "STATIC",
            "用「新增」创建第一个快捷项。",
            0,
            0,
            0,
            0,
            0,
            EMPTY_HELP,
        );
        for (id, label) in [
            (TYPE_COMMAND, "启动命令"),
            (TYPE_SEPARATOR, "分隔行"),
            (TYPE_REMARK, "备注"),
        ] {
            control("BUTTON", label, WS_TABSTOP, 0, 0, 0, 0, id);
        }
        let status = control("STATIC", "修改后点击保存。", 0, 280, 318, 464, 40, 0);
        SetWindowTextW(status, wide("").as_ptr());

        let delete = control("BUTTON", "删除", WS_TABSTOP, 92, 374, 72, 30, DELETE);
        let save = control(
            "BUTTON",
            "保存",
            WS_TABSTOP | BS_DEFPUSHBUTTON as u32,
            560,
            374,
            88,
            30,
            SAVE,
        );
        control("BUTTON", "关闭", WS_TABSTOP, 656, 374, 88, 30, CLOSE);
        let draft = file.entries.clone();
        let paper = CreateSolidBrush(theme::paper());
        let selection = CreateSolidBrush(theme::selection());
        EDITOR.with(|c| {
            *c.borrow_mut() = Some(Editor {
                launch_enabled,
                preferred_sort: SORT_ALIAS,
                workdir,
                enabled,
                entry_kind,
                undo: Vec::new(),
                file,
                draft,
                selected: 0,
                dirty: false,
                loading: false,
                laid_out_kind: None,
                list,
                alias,
                name,
                command,
                kind,
                status,
                save,
                delete,
                font,
                heading,
                background,
                paper,
                selection,
                dpi,
                error: false,
                parent,
                sender,
            })
        });
        EDITOR.with(|c| {
            let mut s = c.borrow_mut();
            let s = s.as_mut().unwrap();
            if let Some(index) = s.draft.iter().position(|e| e.id == remembered.selected) {
                s.selected = index;
            }
        });
        refresh_list();
        windows_sys::Win32::UI::Shell::DragAcceptFiles(hwnd, 1);
        WINDOW.store(hwnd as usize, Ordering::Release);
        ShowWindow(hwnd, SW_SHOWNORMAL);
        SetForegroundWindow(hwnd);
        SetFocus(list);
        apply_intake(hwnd);
        let mut message: MSG = zeroed();
        while GetMessageW(&mut message, null_mut(), 0, 0) > 0 {
            if message.message == WM_KEYDOWN {
                let control = GetKeyState(VK_CONTROL as i32) < 0;
                let focus = GetFocus();
                if control && message.wParam == b'S' as usize {
                    SendMessageW(hwnd, WM_COMMAND, SAVE, 0);
                    continue;
                }
                if focus == list {
                    match message.wParam as u16 {
                        VK_INSERT => {
                            SendMessageW(hwnd, WM_COMMAND, ADD, 0);
                            continue;
                        }
                        VK_DELETE => {
                            SendMessageW(hwnd, WM_COMMAND, DELETE, 0);
                            continue;
                        }
                        VK_F1 => {
                            crate::native::show_error(
                                "选择快捷项后在右侧修改。工具菜单支持验证、路径转换、备份、恢复和测试。保存后更新启动面板。",
                            );
                            continue;
                        }
                        VK_UP | VK_DOWN if control => {
                            tool(
                                hwnd,
                                if message.wParam as u16 == VK_UP {
                                    MOVE_UP
                                } else {
                                    MOVE_DOWN
                                },
                            );
                            continue;
                        }
                        _ => {}
                    }
                }
            }
            if message.message == WM_KEYDOWN
                && message.wParam == VK_TAB as usize
                && IsDialogMessageW(hwnd, &message) != 0
            {
                continue;
            }
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
        EDITOR.with(|c| {
            if let Some(s) = c.borrow_mut().take() {
                for object in [s.font, s.heading, s.background, s.paper, s.selection] {
                    DeleteObject(object);
                }
            }
        });
        UnregisterClassW(class_name.as_ptr(), instance);
    }
    Ok(())
}

static INTAKE: std::sync::Mutex<Vec<PathBuf>> = std::sync::Mutex::new(Vec::new());
static ACTION: std::sync::Mutex<Option<(String, u16)>> = std::sync::Mutex::new(None);
pub fn enqueue(paths: Vec<PathBuf>, action: Option<(String, u16)>) {
    if let Ok(mut list) = INTAKE.lock() {
        list.extend(paths);
    }
    if action.is_some()
        && let Ok(mut pending) = ACTION.lock()
    {
        *pending = action;
    }
    focus();
}
fn apply_intake(hwnd: HWND) {
    let paths = INTAKE
        .lock()
        .map(|mut p| std::mem::take(&mut *p))
        .unwrap_or_default();
    if !paths.is_empty() {
        add_paths(paths);
    }
    let action = ACTION.lock().ok().and_then(|mut p| p.take());
    if let Some((id, action)) = action {
        EDITOR.with(|c| {
            let mut s = c.borrow_mut();
            let s = s.as_mut().unwrap();
            if let Some(index) = s.draft.iter().position(|e| e.id == id) {
                s.selected = index;
            }
        });
        refresh_list();
        unsafe {
            if action == VK_INSERT {
                SendMessageW(hwnd, WM_COMMAND, ADD, 0);
                if !id.is_empty() {
                    let alias = EDITOR.with(|c| c.borrow().as_ref().unwrap().alias);
                    SetWindowTextW(alias, wide(&id).as_ptr());
                }
            } else if action == VK_DELETE {
                SendMessageW(hwnd, WM_COMMAND, DELETE, 0);
            } else {
                let field = EDITOR.with(|c| c.borrow().as_ref().unwrap().alias);
                SetFocus(field);
            }
        }
    }
}
pub fn drop_paths(handle: windows_sys::Win32::UI::Shell::HDROP) -> Vec<PathBuf> {
    unsafe {
        use windows_sys::Win32::UI::Shell::*;
        let count = DragQueryFileW(handle, u32::MAX, std::ptr::null_mut(), 0).min(1000);
        let mut paths = Vec::new();
        for i in 0..count {
            let n = DragQueryFileW(handle, i, std::ptr::null_mut(), 0);
            if n > 32767 {
                continue;
            }
            let mut b = vec![0u16; n as usize + 1];
            DragQueryFileW(handle, i, b.as_mut_ptr(), b.len() as u32);
            paths.push(PathBuf::from(String::from_utf16_lossy(&b[..n as usize])));
        }
        DragFinish(handle);
        paths
    }
}
fn remember() {
    EDITOR.with(|c| {
        let mut s = c.borrow_mut();
        let s = s.as_mut().unwrap();
        s.undo.push(s.draft.clone());
        if s.undo.len() > 20 {
            s.undo.remove(0);
        }
    });
}
fn add_paths(paths: Vec<PathBuf>) {
    let mut entries = Vec::new();
    for path in paths {
        match altrun_probe::management::from_path(&path) {
            Ok(e) => entries.push(e),
            Err(e) => {
                status(&e, true);
                return;
            }
        }
    }
    if entries.is_empty() {
        return;
    }
    remember();
    EDITOR.with(|c| {
        let mut s = c.borrow_mut();
        let s = s.as_mut().unwrap();
        s.selected = s.draft.len();
        s.draft.extend(entries);
        s.dirty = true;
    });
    refresh_list();
    status("已添加到草稿。请调整快捷字后保存。", false);
}
pub(crate) fn pick_path(hwnd: HWND, save: bool, folder: bool) -> Option<PathBuf> {
    unsafe {
        if folder {
            use windows_sys::Win32::{System::Com::*, UI::Shell::*};
            let hr = CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED as u32);
            let mut info: BROWSEINFOW = zeroed();
            info.hwndOwner = hwnd;
            info.ulFlags = BIF_RETURNONLYFSDIRS | BIF_NEWDIALOGSTYLE;
            let pidl = SHBrowseForFolderW(&info);
            let mut b = vec![0u16; 32768];
            let ok = !pidl.is_null() && SHGetPathFromIDListW(pidl, b.as_mut_ptr()) != 0;
            if !pidl.is_null() {
                CoTaskMemFree(pidl as _);
            }
            if hr >= 0 {
                CoUninitialize();
            }
            if !ok {
                return None;
            }
            return Some(PathBuf::from(String::from_utf16_lossy(
                &b[..b.iter().position(|v| *v == 0).unwrap_or(0)],
            )));
        }
        let mut b = vec![0u16; 32768];
        let mut dialog: OPENFILENAMEW = zeroed();
        dialog.lStructSize = std::mem::size_of_val(&dialog) as u32;
        dialog.hwndOwner = hwnd;
        dialog.lpstrFile = b.as_mut_ptr();
        dialog.nMaxFile = b.len() as u32;
        dialog.Flags = OFN_NOCHANGEDIR
            | OFN_EXPLORER
            | if save {
                OFN_OVERWRITEPROMPT
            } else {
                OFN_FILEMUSTEXIST
            };
        let ok = if save {
            GetSaveFileNameW(&mut dialog)
        } else {
            GetOpenFileNameW(&mut dialog)
        };
        if ok == 0 {
            None
        } else {
            Some(PathBuf::from(String::from_utf16_lossy(
                &b[..b.iter().position(|v| *v == 0).unwrap_or(0)],
            )))
        }
    }
}
fn tool(hwnd: HWND, id: usize) {
    if id == TEST {
        test_selected(hwnd);
        return;
    }
    if matches!(
        id,
        SORT_ALIAS | SORT_NAME | SORT_FREQUENCY | MOVE_UP | MOVE_DOWN
    ) {
        remember();
        EDITOR.with(|c| {
            let mut s = c.borrow_mut();
            let s = s.as_mut().unwrap();
            if matches!(id, SORT_ALIAS | SORT_NAME | SORT_FREQUENCY) {
                s.preferred_sort = id;
            }
            let selected = s.draft.get(s.selected).map(|e| e.id.clone());
            match id {
                SORT_ALIAS => s.draft.sort_by_key(|e| e.alias.to_lowercase()),
                SORT_NAME => s.draft.sort_by_key(|e| e.name.to_lowercase()),
                SORT_FREQUENCY => s.draft.sort_by_key(|e| std::cmp::Reverse(e.frequency)),
                MOVE_UP if s.selected > 0 => {
                    let i = s.selected;
                    s.draft.swap(i, i - 1);
                }
                MOVE_DOWN if s.selected + 1 < s.draft.len() => {
                    let i = s.selected;
                    s.draft.swap(i, i + 1);
                }
                _ => {}
            }
            if let Some(id) = selected
                && let Some(index) = s.draft.iter().position(|e| e.id == id)
            {
                s.selected = index;
            }
            s.dirty = true;
        });
        refresh_list();
        status("顺序已更新，保存后生效。", false);
        return;
    }
    if id == ADD_FILE || id == ADD_FOLDER {
        if let Some(path) = pick_path(hwnd, false, id == ADD_FOLDER) {
            add_paths(vec![path]);
        }
        return;
    }
    if id == SEPARATOR || id == REMARK {
        remember();
        EDITOR.with(|c| {
            let mut s = c.borrow_mut();
            let s = s.as_mut().unwrap();
            let identity = library::new_id();
            s.draft.push(Entry {
                alias: if id == SEPARATOR {
                    "---".into()
                } else {
                    "备注".into()
                },
                name: String::new(),
                command: String::new(),
                frequency: 0,
                param_type: ParamType::None,
                id: identity,
                working_directory: String::new(),
                enabled: false,
                kind: if id == SEPARATOR {
                    library::EntryKind::Separator
                } else {
                    library::EntryKind::Remark
                },
            });
            s.selected = s.draft.len() - 1;
            s.dirty = true;
        });
        refresh_list();
        return;
    }
    let (path, rows) = EDITOR.with(|c| {
        let s = c.borrow();
        let s = s.as_ref().unwrap();
        (s.file.path.clone(), s.draft.clone())
    });
    let context = TEST_CONTEXT.lock().map(|c| c.clone()).unwrap_or_default();
    let base = if context.base_directory.is_empty() {
        path.parent().unwrap_or(std::path::Path::new("."))
    } else {
        std::path::Path::new(&context.base_directory)
    };
    match id {
        UNDO => {
            EDITOR.with(|c| {
                let mut s = c.borrow_mut();
                let s = s.as_mut().unwrap();
                if let Some(rows) = s.undo.pop() {
                    s.draft = rows;
                    s.selected = s.selected.min(s.draft.len().saturating_sub(1));
                    s.dirty = true;
                }
            });
            refresh_list();
            status("已撤销上一次批量操作，保存后生效。", false);
        }
        VALIDATE | CLEAN_MISSING => {
            let mut missing = Vec::new();
            let mut unconfirmed = Vec::new();
            for (i, e) in rows.iter().enumerate() {
                match altrun_probe::management::validate(e, base) {
                    altrun_probe::management::Validation::Missing => missing.push(i),
                    altrun_probe::management::Validation::Unconfirmed(reason) => {
                        unconfirmed.push(format!("{}：{}", e.alias, reason))
                    }
                    _ => {}
                }
            }
            let names = missing
                .iter()
                .map(|i| rows[*i].alias.as_str())
                .collect::<Vec<_>>()
                .join("、");
            let message = format!(
                "确认缺失 {} 项：{}\n\n未确认 {} 项（网络 / 系统 / 搜索路径 / 模板不会自动删除）：\n{}",
                missing.len(),
                names,
                unconfirmed.len(),
                unconfirmed
                    .iter()
                    .take(15)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join("\n")
            );
            if missing.is_empty() {
                unsafe {
                    MessageBoxW(
                        hwnd,
                        wide(&message).as_ptr(),
                        ui("验证结果").as_ptr(),
                        MB_OK | MB_ICONINFORMATION,
                    );
                }
            } else if !missing.is_empty()
                && confirm(
                    hwnd,
                    &format!("{message}\n\n从草稿中删除确认缺失项？可通过工具菜单撤销。"),
                )
            {
                remember();
                EDITOR.with(|c| {
                    let mut s = c.borrow_mut();
                    let s = s.as_mut().unwrap();
                    for i in missing.into_iter().rev() {
                        s.draft.remove(i);
                    }
                    s.selected = 0;
                    s.dirty = true;
                });
                refresh_list();
            }
        }
        RELATIVE | ABSOLUTE => {
            remember();
            let mut updated = rows;
            let mut changed = 0;
            let mut skipped = 0;
            for e in &mut updated {
                match altrun_probe::management::convert_path(e, base, id == RELATIVE) {
                    Ok(true) => changed += 1,
                    Ok(false) => {}
                    Err(_) => skipped += 1,
                }
            }
            EDITOR.with(|c| {
                let mut s = c.borrow_mut();
                let s = s.as_mut().unwrap();
                s.draft = updated;
                s.dirty |= changed > 0;
            });
            refresh_list();
            status(
                &format!("已转换 {changed} 项，跳过 {skipped} 项。尚未保存。"),
                false,
            );
        }
        EXPORT => {
            if let Some(destination) = pick_path(hwnd, true, false) {
                if destination == path
                    || std::fs::canonicalize(&destination)
                        .ok()
                        .is_some_and(|p| Some(p) == std::fs::canonicalize(&path).ok())
                {
                    status("请选择不同的备份文件；保存数据请使用“保存”。", true);
                    return;
                }
                let result = library::serialize(&rows).and_then(|text| {
                    altrun_probe::state_store::TextFile::open(&destination)
                        .and_then(|(mut file, _)| file.save(&text))
                });
                match result {
                    Ok(()) => status("草稿已导出 / 备份。", false),
                    Err(e) => status(&e, true),
                }
            }
        }
        RESTORE => {
            if let Some(source) = pick_path(hwnd, false, false) {
                match Snapshot::open(&source) {
                    Ok(snapshot) => {
                        if confirm(
                            hwnd,
                            &format!(
                                "从备份载入 {} 项并替换草稿？保存前数据文件不变。",
                                snapshot.entries.len()
                            ),
                        ) {
                            remember();
                            EDITOR.with(|c| {
                                let mut s = c.borrow_mut();
                                let s = s.as_mut().unwrap();
                                s.draft = snapshot.entries;
                                s.selected = 0;
                                s.dirty = true;
                            });
                            refresh_list();
                        }
                    }
                    Err(e) => status(&e, true),
                }
            }
        }
        EDIT_TEXT => {
            if EDITOR.with(|c| c.borrow().as_ref().unwrap().dirty) {
                status("请先保存或关闭草稿，再用文本编辑器修改文件。", true);
            } else if let Ok(notepad) = altrun_probe::safe_notepad_path() {
                match std::process::Command::new(notepad).arg(&path).spawn() {
                    Ok(_) => status("保存文本后，在工具菜单中选择“重新读取快捷项文件”。", false),
                    Err(e) => status(&e.to_string(), true),
                }
            }
        }
        RELOAD => {
            if EDITOR.with(|c| c.borrow().as_ref().unwrap().dirty) {
                status("请先保存或关闭草稿，再重新读取文件。", true);
                return;
            }
            match Snapshot::open(&path) {
                Ok(file) => {
                    let parent = EDITOR.with(|c| {
                        let mut state = c.borrow_mut();
                        let s = state.as_mut().unwrap();
                        s.draft = file.entries.clone();
                        s.file = file;
                        s.undo.clear();
                        s.selected = s.selected.min(s.draft.len().saturating_sub(1));
                        s.sender.saved(s.draft.clone());
                        s.parent
                    });
                    refresh_list();
                    unsafe {
                        PostMessageW(parent as HWND, UPDATED, 0, 0);
                    }
                    status("已重新读取快捷项。", false);
                }
                Err(error) => status(&error, true),
            }
        }
        _ => {}
    }
}

fn publish_geometry(hwnd: HWND) {
    let mut rect: RECT = unsafe { zeroed() };
    unsafe {
        GetWindowRect(hwnd, &mut rect);
    }
    let (parent, selected) = EDITOR.with(|c| {
        let s = c.borrow();
        let s = s.as_ref().unwrap();
        (
            s.parent,
            s.draft
                .get(s.selected)
                .map(|e| e.id.clone())
                .unwrap_or_default(),
        )
    });
    if let Ok(mut state) = WINDOW_STATE.lock() {
        state.rect = Some((
            rect.left,
            rect.top,
            rect.right - rect.left,
            rect.bottom - rect.top,
        ));
        state.selected = selected;
    }
    unsafe {
        PostMessageW(parent as HWND, GEOMETRY_CHANGED, 0, 0);
    }
}

fn test_selected(hwnd: HWND) {
    if !EDITOR.with(|c| c.borrow().as_ref().unwrap().launch_enabled) {
        status("预览模式：未执行测试命令。", false);
        return;
    }
    let (entry, base) = EDITOR.with(|c| {
        let s = c.borrow();
        let s = s.as_ref().unwrap();
        (
            s.draft.get(s.selected).cloned(),
            s.file
                .path
                .parent()
                .unwrap_or(std::path::Path::new("."))
                .to_path_buf(),
        )
    });
    let Some(entry) = entry else {
        return;
    };
    if entry.kind != library::EntryKind::Command || entry.command.trim().is_empty() {
        status("所选项没有可测试的启动命令。", true);
        return;
    }
    let mut context = TEST_CONTEXT.lock().map(|c| c.clone()).unwrap_or_default();
    if context.base_directory.is_empty() {
        context.base_directory = base.to_string_lossy().into_owned();
    }
    let needs_input = entry.param_type.needs_input(&entry.command)
        && (altrun_probe::launch::has_parameter(&entry.command)
            || !altrun_probe::launch::has_context(&entry.command));
    let parameter = if needs_input {
        let Some(value) = crate::input_dialog::ask(hwnd, &format!("{} · 输入测试参数", entry.name))
        else {
            return;
        };
        Some(value)
    } else {
        None
    };
    status("已发出测试启动请求。草稿尚未保存。", false);
    std::thread::spawn(move || {
        if altrun_probe::launch::has_context(&entry.command) && entry.command.contains("%c") {
            context.clipboard = crate::native::clipboard_text();
        }
        let result = altrun_probe::launch::resolve_context(
            &entry.command,
            parameter.as_deref(),
            entry.param_type,
            &entry.working_directory,
            &context,
        )
        .and_then(|(request, mode, directory)| {
            crate::native::execute_request_with(&request, mode, directory.as_deref())
        });
        if let Err(error) = result {
            crate::native::show_error(&error);
        }
    });
}

// Build transient native cascading menus from the current draft, never cached state.
fn popup(hwnd: HWND, group: usize, position: Option<LPARAM>) {
    unsafe {
        let menu = CreatePopupMenu();
        let (dpi, font) = EDITOR.with(|c| {
            let state = c.borrow();
            let s = state.as_ref().unwrap();
            (s.dpi, s.font)
        });
        let style = crate::manager_menu::PopupStyle::new(dpi, font);
        let append = |target, id, label: &str, checked: bool, disabled: bool| {
            let width = if target != menu || position.is_some() || group == NEW_MENU {
                136
            } else {
                183
            };
            style.add(target, id, label, width, checked, disabled);
        };
        let (rows, selected, can_undo, list) = EDITOR.with(|c| {
            let s = c.borrow();
            let s = s.as_ref().unwrap();
            (s.draft.clone(), s.selected, !s.undo.is_empty(), s.list)
        });
        let mut point = POINT { x: 0, y: 0 };
        if let Some(p) = position {
            if p == -1 {
                let mut rect: RECT = zeroed();
                SendMessageW(
                    list,
                    LB_GETITEMRECT,
                    selected,
                    &mut rect as *mut _ as LPARAM,
                );
                point = POINT {
                    x: rect.left + 12,
                    y: rect.bottom,
                };
                ClientToScreen(list, &mut point);
            } else {
                point = POINT {
                    x: (p as u16) as i16 as i32,
                    y: ((p >> 16) as u16) as i16 as i32,
                };
                let mut local = point;
                ScreenToClient(list, &mut local);
                let hit = SendMessageW(
                    list,
                    LB_ITEMFROMPOINT,
                    0,
                    ((local.y as u32) << 16 | (local.x as u32 & 65535)) as LPARAM,
                );
                if hit >> 16 != 0 {
                    DestroyMenu(menu);
                    return;
                }
                let index = (hit & 65535) as usize;
                if index >= rows.len() {
                    DestroyMenu(menu);
                    return;
                }
                EDITOR.with(|c| c.borrow_mut().as_mut().unwrap().selected = index);
                SendMessageW(list, LB_SETCURSEL, index, 0);
                sync_fields();
            }
            let selected = EDITOR.with(|c| c.borrow().as_ref().unwrap().selected);
            append(
                menu,
                MOVE_UP,
                "上移所选项",
                false,
                selected == 0 || rows.is_empty(),
            );
            append(
                menu,
                MOVE_DOWN,
                "下移所选项",
                false,
                selected + 1 >= rows.len(),
            );
        } else {
            let mut rect: RECT = zeroed();
            GetWindowRect(GetDlgItem(hwnd, group as i32), &mut rect);
            point.x = rect.left;
            point.y = rect.bottom;
            if group == NEW_MENU {
                for (id, label) in [
                    (ADD, "空白启动命令"),
                    (ADD_FILE, "从文件添加…"),
                    (ADD_FOLDER, "从文件夹添加…"),
                    (SEPARATOR, "分隔行"),
                    (REMARK, "备注"),
                ] {
                    if id == SEPARATOR {
                        append(menu, 0, "", false, true);
                    }
                    append(menu, id, label, false, false);
                }
            } else {
                append(menu, 0, "检查", false, true);
                append(menu, TEST, "测试当前快捷项…", false, rows.is_empty());
                append(menu, VALIDATE, "检查快捷项…", false, rows.is_empty());
                append(menu, 0, "", false, true);
                append(menu, 0, "整理", false, true);
                let sort = CreatePopupMenu();
                let choices = [
                    (
                        SORT_ALIAS,
                        "按快捷字",
                        rows.windows(2)
                            .all(|p| p[0].alias.to_lowercase() <= p[1].alias.to_lowercase()),
                    ),
                    (
                        SORT_NAME,
                        "按名称",
                        rows.windows(2)
                            .all(|p| p[0].name.to_lowercase() <= p[1].name.to_lowercase()),
                    ),
                    (
                        SORT_FREQUENCY,
                        "按启动次数",
                        rows.windows(2).all(|p| p[0].frequency >= p[1].frequency),
                    ),
                ];
                let preferred = EDITOR.with(|c| c.borrow().as_ref().unwrap().preferred_sort);
                let selected_sort = choices
                    .iter()
                    .find(|(id, _, ok)| *id == preferred && *ok)
                    .or_else(|| choices.iter().find(|(_, _, ok)| *ok))
                    .map(|(id, _, _)| *id);
                for (id, label, _) in choices {
                    append(
                        sort,
                        id,
                        label,
                        selected_sort == Some(id) && !rows.is_empty(),
                        rows.is_empty(),
                    );
                }
                style.submenu(menu, sort, "排序");
                let paths = CreatePopupMenu();
                let paths_in_rows: Vec<_> = rows
                    .iter()
                    .filter(|e| e.kind == library::EntryKind::Command)
                    .map(|e| {
                        e.command
                            .trim()
                            .trim_start_matches('"')
                            .split('"')
                            .next()
                            .unwrap_or("")
                    })
                    .filter(|s| s.contains('\\') || s.starts_with("./"))
                    .collect();
                let absolute = !paths_in_rows.is_empty()
                    && paths_in_rows
                        .iter()
                        .all(|p| std::path::Path::new(p).is_absolute());
                let relative = !paths_in_rows.is_empty()
                    && paths_in_rows
                        .iter()
                        .all(|p| !std::path::Path::new(p).is_absolute() && !p.contains('%'));
                append(paths, RELATIVE, "转为相对路径", relative, rows.is_empty());
                append(paths, ABSOLUTE, "转为绝对路径", absolute, rows.is_empty());
                style.submenu(menu, paths, "路径格式");
                append(menu, 0, "", false, true);
                append(menu, UNDO, "撤销上次批量操作", false, !can_undo);
                append(menu, 0, "", false, true);
                append(menu, 0, "备份", false, true);
                append(menu, EXPORT, "导出备份…", false, false);
                append(menu, RESTORE, "从备份恢复…", false, false);
                append(menu, 0, "", false, true);
            }
        }
        style.background(menu);
        let command = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_RIGHTBUTTON,
            point.x,
            point.y,
            0,
            hwnd,
            std::ptr::null(),
        );
        DestroyMenu(menu);
        if command != 0 {
            SendMessageW(hwnd, WM_COMMAND, command as WPARAM, 0);
        }
    }
}

fn paint_manager(hwnd: HWND) {
    unsafe {
        let mut ps: PAINTSTRUCT = zeroed();
        let dc = BeginPaint(hwnd, &mut ps);
        let mut bounds: RECT = zeroed();
        GetClientRect(hwnd, &mut bounds);
        let (dpi, font) = EDITOR.with(|c| {
            let s = c.borrow();
            let s = s.as_ref().unwrap();
            (s.dpi, s.font)
        });
        let rect = |x, y, w, h| RECT {
            left: scaled(x, dpi),
            top: scaled(y, dpi),
            right: scaled(x + w, dpi),
            bottom: scaled(y + h, dpi),
        };
        crate::theme_ui::fill(dc, &bounds, theme::surface());
        crate::theme_ui::fill(dc, &rect(0, 0, WIDTH, CAPTION), theme::chrome());
        crate::theme_ui::fill(
            dc,
            &rect(0, CAPTION, RAIL, FOOTER_TOP - CAPTION),
            theme::chrome(),
        );
        crate::theme_ui::fill(
            dc,
            &rect(0, FOOTER_TOP, WIDTH, HEIGHT - FOOTER_TOP),
            theme::chrome(),
        );
        let mut list_card = rect(16, 106, RAIL - 32, 0);
        list_card.bottom = scaled(LIST_TOP, dpi) + list_height(dpi) + scaled(2, dpi);
        crate::theme_ui::rounded(
            dc,
            list_card,
            scaled(16, dpi),
            theme::control_surface(),
            theme::divider(),
        );
        let kind = EDITOR.with(|c| {
            let s = c.borrow();
            let s = s.as_ref().unwrap();
            s.draft.get(s.selected).map(|e| e.kind)
        });
        if let Some(kind) = kind {
            crate::theme_ui::rounded(
                dc,
                rect(
                    FORM_LEFT,
                    FORM_TOP,
                    FORM_WIDTH,
                    if kind == library::EntryKind::Command {
                        COMMAND_FORM_HEIGHT
                    } else {
                        164
                    },
                ),
                scaled(16, dpi),
                theme::control_surface(),
                theme::divider(),
            );
            crate::theme_ui::rounded(
                dc,
                rect(FIELD_LEFT, 153, FIELD_WIDTH, 37),
                scaled(14, dpi),
                theme::paper(),
                theme::border(),
            );
        }
        for line in [
            rect(0, CAPTION, WIDTH, 1),
            rect(RAIL, CAPTION, 1, FOOTER_TOP - CAPTION),
            rect(0, FOOTER_TOP - 1, WIDTH, 1),
            rect(17, 136, RAIL - 34, 1),
        ] {
            crate::theme_ui::fill(dc, &line, theme::divider());
        }
        crate::theme_ui::caption_mark(dc, dpi);
        SetBkMode(dc, TRANSPARENT as i32);
        SetTextColor(dc, theme::text());
        SelectObject(dc, font);
        let mut caption = rect(44, 0, WIDTH - 90, CAPTION);
        DrawTextW(
            dc,
            ui("ALTRun 管理快捷项").as_ptr(),
            -1,
            &mut caption,
            DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX,
        );
        let mut library_title = rect(17, 62, 120, 32);
        DrawTextW(
            dc,
            ui("快捷项").as_ptr(),
            -1,
            &mut library_title,
            DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX,
        );
        crate::theme_ui::line_text(
            dc,
            rect(28, 106, 115, 31),
            &crate::localization::tr("快捷字"),
            theme::muted(),
            0,
        );
        crate::theme_ui::line_text(
            dc,
            rect(17 + (RAIL - 34) * 42 / 100, 106, RAIL / 2, 31),
            &crate::localization::tr("名称"),
            theme::muted(),
            0,
        );
        EndPaint(hwnd, &ps);
    }
}

unsafe extern "system" fn list_scrollbar_proc(
    hwnd: HWND,
    msg: u32,
    w: WPARAM,
    l: LPARAM,
    _: usize,
    scrollbar: usize,
) -> LRESULT {
    let result = unsafe { DefSubclassProc(hwnd, msg, w, l) };
    if matches!(
        msg,
        WM_NCPAINT
            | WM_PAINT
            | WM_VSCROLL
            | WM_MOUSEWHEEL
            | WM_KEYDOWN
            | WM_NCLBUTTONDOWN
            | WM_SIZE
            | LB_SETTOPINDEX
            | LB_SETCURSEL
    ) {
        update_scrollbar(hwnd, scrollbar as HWND);
    }
    result
}

fn update_scrollbar(list: HWND, scrollbar: HWND) {
    unsafe {
        let visible = GetWindowLongW(scrollbar, GWL_STYLE) as u32 & WS_VISIBLE != 0;
        let Some(bar) = crate::theme_ui::vertical_scrollbar(list) else {
            if visible {
                ShowWindow(scrollbar, SW_HIDE);
            }
            return;
        };
        let mut bounds = RECT::default();
        GetWindowRect(scrollbar, &mut bounds);
        let track = bar.rcScrollBar;
        if !visible || EqualRect(&bounds, &track) == 0 {
            let mut origin = POINT {
                x: track.left,
                y: track.top,
            };
            ScreenToClient(GetParent(scrollbar), &mut origin);
            SetWindowPos(
                scrollbar,
                HWND_TOP,
                origin.x,
                origin.y,
                track.right - track.left,
                track.bottom - track.top,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
        }
        // Native scrollbar tracking can draw directly into its list window.
        // A separate, clipped sibling keeps our colors visible during that loop.
        RedrawWindow(
            scrollbar,
            std::ptr::null(),
            null_mut(),
            RDW_INVALIDATE | RDW_UPDATENOW,
        );
    }
}

unsafe extern "system" fn scrollbar_proc(
    hwnd: HWND,
    msg: u32,
    w: WPARAM,
    l: LPARAM,
    _: usize,
    list: usize,
) -> LRESULT {
    match msg {
        WM_NCHITTEST => HTTRANSPARENT as isize,
        WM_ERASEBKGND => 1,
        WM_PAINT | WM_PRINTCLIENT => unsafe {
            let mut paint = PAINTSTRUCT::default();
            let dc = if msg == WM_PAINT {
                BeginPaint(hwnd, &mut paint)
            } else {
                w as HDC
            };
            let mut track = RECT::default();
            GetClientRect(hwnd, &mut track);
            if let Some(bar) = crate::theme_ui::vertical_scrollbar(list as HWND) {
                crate::theme_ui::draw_scrollbar(
                    dc,
                    track,
                    &bar,
                    EDITOR.with(|c| {
                        c.borrow()
                            .as_ref()
                            .map_or_else(|| GetDpiForWindow(hwnd).max(96) as i32, |s| s.dpi)
                    }),
                );
            } else {
                crate::theme_ui::fill(dc, &track, theme::paper());
            }
            if msg == WM_PAINT {
                EndPaint(hwnd, &paint);
            }
            0
        },
        _ => unsafe { DefSubclassProc(hwnd, msg, w, l) },
    }
}

fn sync_kind_layout() {
    let layout = EDITOR.with(|c| {
        let mut state = c.borrow_mut();
        let s = state.as_mut().unwrap();
        let kind = s.draft.get(s.selected).map(|e| e.kind);
        if s.laid_out_kind == Some(kind) {
            return None;
        }
        s.laid_out_kind = Some(kind);
        Some((unsafe { GetParent(s.list) }, kind, s.name, s.dpi))
    });
    let Some((hwnd, kind, name, dpi)) = layout else {
        return;
    };
    unsafe {
        for id in [
            ALIAS, COMMAND, KIND, ENABLED, WORKDIR, 2170, 2171, 2172, 2173, 2174,
        ] {
            ShowWindow(
                GetDlgItem(hwnd, id as i32),
                if kind == Some(library::EntryKind::Command) {
                    SW_SHOW
                } else {
                    SW_HIDE
                },
            );
        }
        for id in [TYPE_COMMAND, TYPE_SEPARATOR, TYPE_REMARK, TYPE_LABEL] {
            let child = GetDlgItem(hwnd, id as i32);
            ShowWindow(child, if kind.is_some() { SW_SHOW } else { SW_HIDE });
            InvalidateRect(child, std::ptr::null(), 1);
        }
        ShowWindow(
            GetDlgItem(hwnd, NOTE_LABEL as i32),
            if kind == Some(library::EntryKind::Remark) {
                SW_SHOW
            } else {
                SW_HIDE
            },
        );
        ShowWindow(
            GetDlgItem(hwnd, SEPARATOR_HELP as i32),
            if kind == Some(library::EntryKind::Separator) {
                SW_SHOW
            } else {
                SW_HIDE
            },
        );
        ShowWindow(
            GetDlgItem(hwnd, EMPTY_HELP as i32),
            if kind.is_none() { SW_SHOW } else { SW_HIDE },
        );
        ShowWindow(
            name,
            if matches!(
                kind,
                Some(library::EntryKind::Command | library::EntryKind::Remark)
            ) {
                SW_SHOW
            } else {
                SW_HIDE
            },
        );
        if let Some((x, width)) = match kind {
            Some(library::EntryKind::Remark) => Some((FIELD_LEFT, FIELD_WIDTH)),
            Some(library::EntryKind::Command) => Some((FIELD_RIGHT - HALF_FIELD, HALF_FIELD)),
            _ => None,
        } {
            SetWindowPos(
                name,
                null_mut(),
                scaled(x, dpi),
                scaled(233, dpi),
                scaled(x + width, dpi) - scaled(x, dpi),
                scaled(269, dpi) - scaled(233, dpi),
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
        let card = RECT {
            left: scaled(FORM_LEFT, dpi),
            top: scaled(FORM_TOP, dpi),
            right: scaled(FORM_LEFT + FORM_WIDTH, dpi),
            bottom: scaled(FORM_TOP + COMMAND_FORM_HEIGHT, dpi),
        };
        InvalidateRect(hwnd, &card, 0);
    }
}
