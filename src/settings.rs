//! Native settings pages, created only while the settings window is open.
use crate::{
    native::{make_font, scaled, ui, wide},
    settings_preview::Preview,
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
        WindowsAndMessaging::*,
    },
};

pub const UPDATED: u32 = WM_APP + 12;
const RECORDED: u32 = WM_APP + 1;
const TAB: usize = 2301;
const STATUS: usize = 2303;
const PREVIEW: usize = 2305;
const PREVIEW_QUERY: usize = 2306;
// Trackbar messages start at WM_USER. windows-sys omits TBM_GETPOS.
const TBM_GETPOS: u32 = WM_USER;
const FIELD: usize = 2400;
const PICK: usize = 2500;
const CLEAR: usize = 2600;

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
        name: "常规",
        hint: "管理启动方式、系统托盘和资源管理器菜单。",
        keys: &[
            "ShowMeWhenStart",
            "ShowTrayIcon",
            "AutoRun",
            "AddToSendTo",
            "ShowStartNotification",
            "PlayPopupNotify",
            "PopupFileName",
            "Lang",
        ],
    },
    Page {
        name: "快捷键",
        hint: "点击“录制”，直接按下想用的快捷键。Esc 取消，“清除”停用该键。",
        keys: &["HotKey1", "HotKey2", "LastItemHotKey"],
    },
    Page {
        name: "搜索与执行",
        hint: "调整匹配方式、结果编号和执行后的行为。",
        keys: &[
            "Regex",
            "MatchAnywhere",
            "EnglishSearch",
            "RememberFavouratMatch",
            "NumberKey",
            "IndexFrom0to9",
            "ExecuteIfOnlyOne",
            "ExitWhenExecute",
            "ParamHistoryLimit",
        ],
    },
    Page {
        name: "外观",
        hint: "调整面板和结果列表。更改会在保存后生效。",
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
            "ShowTopTen",
            "ShowOperationHint",
            "ShowShortCutButton",
            "ShowConfigButton",
            "ShowCloseButton",
        ],
    },
    Page {
        name: "字体与颜色",
        hint: "选择字体或颜色，也可以直接编辑已有值。",
        keys: &[
            "TitleFont",
            "KeywordFont",
            "ListFont",
            "TitleColor",
            "KeywordColor",
            "ListColor",
            "TextColor",
            "SelectionColor",
            "Alpha",
            "RoundBorderRadius",
        ],
    },
    Page {
        name: "窗口位置",
        hint: "设置呼出位置、自动隐藏时间和管理窗口大小。",
        keys: &[
            "PositionMode",
            "HideDelay",
            "WinLeft",
            "WinTop",
            "ManWinLeft",
            "ManWinTop",
            "ManWinWidth",
            "ManWinHeight",
        ],
    },
    Page {
        name: "高级",
        hint: "设置正则语法、工作目录和诊断选项。",
        keys: &[
            "StandardRegex",
            "ListFormat",
            "WorkingDirectory",
            "AllowDynamicTarget",
            "TraceEnable",
        ],
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
    font: HFONT,
    heading_font: HFONT,
    section_font: HFONT,
    sections: Vec<(usize, HWND)>,
    dpi: i32,
    page: usize,
    preview: Preview,
    preview_controls: [HWND; 3],
    preview_font: HFONT,
    saved: Preferences,
    draft: Preferences,
    fields: Vec<Field>,
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

fn text(hwnd: HWND) -> String {
    unsafe {
        let mut buffer = vec![0; GetWindowTextLengthW(hwnd) as usize + 1];
        let len = GetWindowTextW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32);
        String::from_utf16_lossy(&buffer[..len as usize])
    }
}

fn status(message: &str) {
    let hwnd = EDITOR.with(|c| c.borrow().as_ref().unwrap().status);
    unsafe {
        SetWindowTextW(hwnd, ui(message).as_ptr());
    }
}

fn choices(key: &str) -> &'static [(&'static str, &'static str)] {
    match key {
        "Theme" => &[("light", "浅色"), ("dark", "深色"), ("system", "跟随系统")],
        "Lang" => &[("简体中文", "简体中文"), ("EN", "English")],
        "BGCrop" => &[("top", "顶部"), ("center", "居中"), ("bottom", "底部")],
        "PositionMode" => &[
            ("remember", "记住位置"),
            ("mouse", "鼠标附近"),
            ("center", "屏幕居中"),
        ],
        _ => &[],
    }
}

fn fields() -> Vec<Field> {
    EDITOR.with(|c| c.borrow().as_ref().unwrap().fields.clone())
}

fn show_page(page: usize) {
    let Some(design) = PAGES.get(page) else {
        return;
    };
    cancel_capture();
    let (tabs, title, hint, sections, preview) = EDITOR.with(|c| {
        let mut editor = c.borrow_mut();
        let editor = editor.as_mut().unwrap();
        editor.page = page;
        (
            editor.tabs,
            editor.title,
            editor.hint,
            editor.sections.clone(),
            editor.preview_controls,
        )
    });
    unsafe {
        SendMessageW(tabs, LB_SETCURSEL, page, 0);
        SetWindowTextW(title, ui(design.name).as_ptr());
        SetWindowTextW(hint, ui(design.hint).as_ptr());
        for field in fields() {
            let visibility = if field.page == page {
                SW_SHOWNA
            } else {
                SW_HIDE
            };
            for hwnd in [field.label, field.value, field.pick, field.clear] {
                if !hwnd.is_null() {
                    ShowWindow(hwnd, visibility);
                }
            }
        }
        for (group, hwnd) in sections {
            ShowWindow(hwnd, if group == page { SW_SHOWNA } else { SW_HIDE });
        }
        for hwnd in preview {
            ShowWindow(
                hwnd,
                if matches!(page, 3 | 4) {
                    SW_SHOWNA
                } else {
                    SW_HIDE
                },
            );
        }
    }
    update_preview();
}

fn update_preview() {
    let (page, controls, dpi) = EDITOR.with(|c| {
        let editor = c.borrow();
        let editor = editor.as_ref().unwrap();
        (editor.page, editor.preview_controls, editor.dpi)
    });
    if !matches!(page, 3 | 4) {
        return;
    }
    let Ok(prefs) = read_form() else {
        return;
    };
    EDITOR.with(|c| {
        let mut editor = c.borrow_mut();
        let editor = editor.as_mut().unwrap();
        editor
            .preview
            .update(prefs.clone(), &editor.data, editor.hwnd);
    });
    refresh_style(&prefs);
    unsafe {
        if let Ok(font) = altrun_probe::preferences::Font::parse(&prefs.get("KeywordFont")) {
            let font = make_font(font.points * 4 / 3, font.weight, &font.name, dpi);
            if !font.is_null() {
                SendMessageW(controls[2], WM_SETFONT, font as usize, 1);
                let old = EDITOR.with(|c| {
                    std::mem::replace(&mut c.borrow_mut().as_mut().unwrap().preview_font, font)
                });
                if !old.is_null() {
                    DeleteObject(old);
                }
            }
        }
        let skin = prefs.flag("ShowSkin");
        for (spec, field) in SPECS.iter().zip(fields()) {
            if matches!(spec.key, "BGStrength" | "BGCrop") {
                EnableWindow(field.value, i32::from(skin));
            }
        }
        InvalidateRect(controls[1], std::ptr::null(), 0);
    }
}

fn load_form(prefs: &Preferences) {
    EDITOR.with(|c| c.borrow_mut().as_mut().unwrap().draft = prefs.clone());
    for (spec, field) in SPECS.iter().zip(fields()) {
        let value = prefs.get(spec.key);
        unsafe {
            if spec.key == "BGStrength" {
                SendMessageW(
                    field.value,
                    TBM_SETPOS,
                    1,
                    prefs.number("BGStrength") as isize,
                );
                strength_label(field, prefs.number("BGStrength"));
            } else if matches!(spec.kind, Kind::Font) {
                show_font(field.value, &value);
            } else if matches!(spec.kind, Kind::Bool) {
                SendMessageW(field.value, BM_SETCHECK, usize::from(value == "1"), 0);
            } else if let Some(index) = choices(spec.key).iter().position(|(key, _)| *key == value)
            {
                SendMessageW(field.value, CB_SETCURSEL, index, 0);
            } else {
                SetWindowTextW(field.value, wide(&value).as_ptr());
            }
        }
    }
}

fn read_form() -> Result<Preferences, (usize, String)> {
    let mut prefs = EDITOR.with(|c| c.borrow().as_ref().unwrap().draft.clone());
    for (index, (spec, field)) in SPECS.iter().zip(fields()).enumerate() {
        let value = if matches!(spec.kind, Kind::Font) {
            prefs.get(spec.key)
        } else if spec.key == "BGStrength" {
            unsafe { SendMessageW(field.value, TBM_GETPOS, 0, 0) }.to_string()
        } else if matches!(spec.kind, Kind::Bool) {
            let checked = unsafe { SendMessageW(field.value, BM_GETCHECK, 0, 0) != 0 };
            if checked { "1" } else { "0" }.to_owned()
        } else if !choices(spec.key).is_empty() {
            let selected = unsafe { SendMessageW(field.value, CB_GETCURSEL, 0, 0) };
            choices(spec.key)
                .get(selected as usize)
                .map(|(value, _)| (*value).to_owned())
                .ok_or_else(|| (index, "请选择一个值".into()))?
        } else {
            text(field.value)
        };
        prefs
            .set(spec.key, &value)
            .map_err(|error| (index, error))?;
    }
    Ok(prefs)
}

fn validated_form() -> Option<Preferences> {
    match read_form() {
        Ok(prefs) => Some(prefs),
        Err((index, error)) => {
            let field = fields()[index];
            show_page(field.page);
            unsafe {
                SetFocus(field.value);
            }
            status(&error);
            None
        }
    }
}

fn strength_label(field: Field, strength: i32) {
    let name = crate::localization::tr("底纹强度");
    unsafe {
        SetWindowTextW(field.label, wide(&format!("{name}  {strength}%")).as_ptr());
    }
}

fn show_font(hwnd: HWND, value: &str) {
    if let Ok(font) = altrun_probe::preferences::Font::parse(value) {
        let description = format!("{} · {} pt", font.name, font.points);
        unsafe {
            SetWindowTextW(hwnd, wide(&description).as_ptr());
        }
    }
}

fn stop_capture() {
    if let Some(recording) = CAPTURE.take()
        && !recording.hook.is_null()
    {
        unsafe {
            UnhookWindowsHookEx(recording.hook);
        }
    }
}

fn cancel_capture() {
    if CAPTURE.get().is_some() {
        stop_capture();
        status("已取消录制。");
    }
}

fn key_name(vk: u32) -> Option<String> {
    Some(match vk {
        0x30..=0x39 | 0x41..=0x5a => char::from(vk as u8).to_string(),
        0x70..=0x87 => format!("F{}", vk - 0x6f),
        0x13 => "Pause".into(),
        0x20 => "Space".into(),
        0x09 => "Tab".into(),
        0x0d => "Enter".into(),
        0x1b => "Esc".into(),
        0x2d => "Insert".into(),
        0x2e => "Delete".into(),
        0x24 => "Home".into(),
        0x23 => "End".into(),
        0x21 => "PageUp".into(),
        0x22 => "PageDown".into(),
        _ => return None,
    })
}

fn modifier_bits(state: impl Fn(i32) -> i16) -> usize {
    usize::from(state(VK_MENU as i32) < 0)
        | (usize::from(state(VK_CONTROL as i32) < 0) << 1)
        | (usize::from(state(VK_SHIFT as i32) < 0) << 2)
        | (usize::from(state(VK_LWIN as i32) < 0 || state(VK_RWIN as i32) < 0) << 3)
}

fn is_modifier(vk: u32) -> bool {
    matches!(vk, 0x10..=0x12 | 0x5b | 0x5c | 0xa0..=0xa5)
}

fn record_key(vk: u32, modifiers: usize) -> bool {
    let Some(recording) = CAPTURE.get() else {
        return false;
    };
    if is_modifier(vk) {
        return false;
    }
    if vk == VK_ESCAPE as u32 && modifiers == 0 {
        stop_capture();
        status("已取消录制。");
        return true;
    }
    let Some(key) = key_name(vk) else {
        status("请按字母、数字、功能键或 Pause 等受支持的按键。");
        return true;
    };
    let mut value = String::new();
    for (bit, label) in [(2, "Ctrl+"), (1, "Alt+"), (4, "Shift+"), (8, "Win+")] {
        if modifiers & bit != 0 {
            value.push_str(label);
        }
    }
    value.push_str(&key);
    stop_capture();
    let field = fields()[recording.index];
    unsafe {
        SetWindowTextW(field.value, wide(&value).as_ptr());
    }
    status("已录制。点击“保存并应用”生效。");
    true
}

unsafe extern "system" fn capture_hook(code: i32, w: WPARAM, l: LPARAM) -> LRESULT {
    if code >= 0
        && matches!(w as u32, WM_KEYDOWN | WM_SYSKEYDOWN)
        && let Some(recording) = CAPTURE.get()
        && unsafe { GetForegroundWindow() == recording.window }
    {
        let vk = unsafe { (*(l as *const KBDLLHOOKSTRUCT)).vkCode };
        if !is_modifier(vk) {
            let modifiers = modifier_bits(|key| unsafe { GetAsyncKeyState(key) });
            unsafe {
                PostMessageW(recording.window, RECORDED, vk as usize, modifiers as isize);
            }
            return 1;
        }
    }
    unsafe { CallNextHookEx(null_mut(), code, w, l) }
}

fn start_capture(hwnd: HWND, index: usize) {
    stop_capture();
    let hook = unsafe {
        SetWindowsHookExW(
            WH_KEYBOARD_LL,
            Some(capture_hook),
            GetModuleHandleW(std::ptr::null()),
            0,
        )
    };
    CAPTURE.set(Some(Capture {
        window: hwnd,
        index,
        hook,
    }));
    status("正在录制，按下快捷键。Esc 取消。");
}
fn apply() {
    let Some(prefs) = validated_form() else {
        return;
    };
    let (sender, owner) = EDITOR.with(|c| {
        let s = c.borrow();
        let s = s.as_ref().unwrap();
        (s.sender.clone(), s.owner)
    });
    static NEXT: AtomicUsize = AtomicUsize::new(1);
    let token = NEXT.fetch_add(1, Ordering::Relaxed) as u64;
    let (tx, rx) = mpsc::channel();
    let _ = sender.send(Change::Prepare(token, prefs.clone(), tx));
    unsafe {
        PostMessageW(owner as HWND, UPDATED, 0, 0);
    }
    let prepared = rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .map_err(|_| "启动面板暂时未响应".to_owned())
        .and_then(|v| v);
    let result = prepared.and_then(|_| {
        EDITOR.with(|c| {
            let mut s = c.borrow_mut();
            let s = s.as_mut().unwrap();
            let integration = altrun_probe::integration::Integration::for_data(&s.data)?;
            let previous = integration.status()?;
            let requested = (prefs.flag("AutoRun"), prefs.flag("AddToSendTo"));
            if previous != requested {
                integration.apply(requested.0, requested.1)?;
            }
            if let Err(error) = s.file.save(&prefs.text()) {
                if previous != requested {
                    let _ = integration.apply(previous.0, previous.1);
                }
                return Err(error);
            }
            Ok(())
        })
    });
    match result {
        Ok(()) => {
            let _ = sender.send(Change::Commit(token));
            let language_changed = EDITOR
                .with(|c| c.borrow().as_ref().unwrap().saved.get("Lang") != prefs.get("Lang"));
            EDITOR.with(|c| c.borrow_mut().as_mut().unwrap().saved = prefs.clone());
            load_form(&prefs);
            refresh_style(&prefs);
            update_preview();
            status(if language_changed {
                "语言已保存。重新打开设置即可查看。"
            } else {
                "已保存并应用。"
            });
        }
        Err(e) => {
            let _ = sender.send(Change::Cancel(token));
            status(&format!("未保存。{e}"));
        }
    }
    unsafe {
        PostMessageW(owner as HWND, UPDATED, 0, 0);
    }
}

fn field_position(page: usize, row: usize) -> (i32, i32, i32) {
    match page {
        0 => (190, [115, 150, 185, 260, 333, 368, 403, 480][row], 790),
        3 if row < 9 => (190, [115, 181, 215, 249, 283, 355, 389, 423, 457][row], 372),
        3 => (578, 365 + (row as i32 - 9) * 30, 410),
        4 => (190, 113 + row as i32 * 35, 372),
        _ => (190, 115 + row as i32 * 36, 790),
    }
}

fn draw(item: &DRAWITEMSTRUCT) {
    if item.CtlType == ODT_BUTTON {
        theme_ui::button(item, item.CtlID == 2310);
        return;
    }
    if item.CtlType == ODT_COMBOBOX {
        theme_ui::combo(item);
        return;
    }
    let query = if item.CtlID as usize == PREVIEW {
        let input = EDITOR.with(|c| c.borrow().as_ref().map(|e| e.preview_controls[2]));
        input.map(text).unwrap_or_default()
    } else {
        String::new()
    };
    EDITOR.with(|c| {
        let state = c.borrow();
        let Some(s) = state.as_ref() else {
            return;
        };
        if item.CtlID as usize == PREVIEW {
            s.preview.paint(item, s.dpi, &query);
        } else if item.CtlID as usize == TAB
            && let Some(page) = PAGES.get(item.itemID as usize)
        {
            let selected = item.itemState & ODS_SELECTED != 0;
            let mut bounds = item.rcItem;
            theme_ui::fill(item.hDC, &bounds, theme::chrome());
            bounds.left += scaled(8, s.dpi);
            bounds.right -= scaled(8, s.dpi);
            bounds.top += scaled(2, s.dpi);
            bounds.bottom -= scaled(2, s.dpi);
            if selected {
                theme_ui::fill(item.hDC, &bounds, theme::selection());
                let rail = RECT {
                    right: bounds.left + scaled(3, s.dpi),
                    top: bounds.top + scaled(9, s.dpi),
                    bottom: bounds.bottom - scaled(9, s.dpi),
                    ..bounds
                };
                theme_ui::fill(item.hDC, &rail, theme::accent());
            }
            bounds.left += scaled(14, s.dpi);
            unsafe {
                let old = SelectObject(item.hDC, if selected { s.section_font } else { s.font });
                theme_ui::line_text(
                    item.hDC,
                    bounds,
                    &crate::localization::tr(page.name),
                    theme::text(),
                    0,
                );
                SelectObject(item.hDC, old);
                if item.itemState & ODS_FOCUS != 0 {
                    DrawFocusRect(item.hDC, &bounds);
                }
            }
        }
    });
}
fn choose_file(save: bool) -> Option<PathBuf> {
    unsafe {
        let mut b = vec![0u16; 32768];
        let mut d: OPENFILENAMEW = std::mem::zeroed();
        d.lStructSize = std::mem::size_of_val(&d) as u32;
        d.hwndOwner = WINDOW.load(Ordering::Acquire) as HWND;
        d.lpstrFile = b.as_mut_ptr();
        d.nMaxFile = b.len() as u32;
        let filter = ui("设置文件\0*.ini\0所有文件\0*.*\0");
        d.lpstrFilter = filter.as_ptr();
        d.Flags = OFN_EXPLORER
            | OFN_NOCHANGEDIR
            | if save {
                OFN_OVERWRITEPROMPT
            } else {
                OFN_FILEMUSTEXIST
            };
        let ok = if save {
            GetSaveFileNameW(&mut d)
        } else {
            GetOpenFileNameW(&mut d)
        };
        if ok == 0 {
            None
        } else {
            Some(PathBuf::from(String::from_utf16_lossy(
                &b[..b.iter().position(|c| *c == 0).unwrap_or(b.len())],
            )))
        }
    }
}
unsafe extern "system" fn procedure(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    match msg {
        WM_ERASEBKGND => {
            let style = EDITOR.with(|c| c.borrow().as_ref().map(|s| (s.background, s.dpi)));
            let mut rect: RECT = unsafe { std::mem::zeroed() };
            unsafe {
                GetClientRect(hwnd, &mut rect);
                let (brush, dpi) = style.unwrap_or_else(|| (GetSysColorBrush(COLOR_WINDOW), 96));
                FillRect(w as HDC, &rect, brush);
                let sidebar = RECT {
                    right: scaled(164, dpi),
                    bottom: scaled(546, dpi),
                    ..rect
                };
                let footer = RECT {
                    top: scaled(546, dpi),
                    ..rect
                };
                theme_ui::fill(w as HDC, &sidebar, theme::chrome());
                theme_ui::fill(w as HDC, &footer, theme::chrome());
            }
            1
        }
        WM_DRAWITEM if l != 0 => {
            draw(unsafe { &*(l as *const DRAWITEMSTRUCT) });
            1
        }
        WM_CTLCOLORSTATIC | WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX | WM_CTLCOLORBTN => {
            EDITOR.with(|c| {
                let s = c.borrow();
                let Some(s) = s.as_ref() else {
                    return unsafe { GetSysColorBrush(COLOR_WINDOW) } as isize;
                };
                unsafe {
                    SetTextColor(
                        w as HDC,
                        if l as HWND == s.hint || l as HWND == s.status {
                            theme::muted()
                        } else {
                            theme::text()
                        },
                    );
                    SetBkColor(
                        w as HDC,
                        if msg == WM_CTLCOLOREDIT || msg == WM_CTLCOLORLISTBOX {
                            crate::theme::paper()
                        } else {
                            crate::theme::surface()
                        },
                    );
                }
                if msg == WM_CTLCOLOREDIT || msg == WM_CTLCOLORLISTBOX {
                    s.paper as isize
                } else {
                    s.background as isize
                }
            })
        }
        RECORDED => {
            record_key(w as u32, l as usize);
            0
        }
        crate::settings_preview::READY => {
            let (error, preview) = EDITOR.with(|c| {
                let mut state = c.borrow_mut();
                let s = state.as_mut().unwrap();
                (s.preview.complete(), s.preview_controls[1])
            });
            if let Some(error) = error {
                status(&error);
            }
            unsafe {
                InvalidateRect(preview, std::ptr::null(), 0);
            }
            0
        }
        WM_HSCROLL if !((l as HWND).is_null()) && EDITOR.with(|c| c.borrow().is_some()) => {
            if let Some(field) = fields().into_iter().find(|field| field.value == l as HWND) {
                let strength = unsafe { SendMessageW(field.value, TBM_GETPOS, 0, 0) } as i32;
                strength_label(field, strength);
                if w & 0xffff != TB_THUMBTRACK as usize {
                    update_preview();
                }
            }
            0
        }
        WM_KEYDOWN | WM_SYSKEYDOWN
            if record_key(w as u32, modifier_bits(|key| unsafe { GetKeyState(key) })) =>
        {
            0
        }
        WM_ACTIVATE if w & 65535 == WA_INACTIVE as usize => {
            cancel_capture();
            0
        }
        WM_COMMAND => {
            let id = w & 65535;
            let code = (w >> 16) as u32;
            if id == TAB && code == LBN_SELCHANGE {
                let page = unsafe { SendMessageW(l as HWND, LB_GETCURSEL, 0, 0) };
                show_page(page as usize);
                return 0;
            }
            if id == PREVIEW_QUERY && code == EN_CHANGE {
                let preview = EDITOR.with(|c| c.borrow().as_ref().map(|e| e.preview_controls[1]));
                if let Some(preview) = preview {
                    unsafe {
                        InvalidateRect(preview, std::ptr::null(), 0);
                    }
                }
                return 0;
            }
            if let Some(index) = id.checked_sub(FIELD).filter(|i| *i < SPECS.len()) {
                let spec = &SPECS[index];
                let changed = if matches!(spec.kind, Kind::Bool) {
                    code == BN_CLICKED
                } else if !choices(spec.key).is_empty() {
                    code == CBN_SELCHANGE
                } else {
                    code == EN_KILLFOCUS
                };
                if changed && EDITOR.with(|c| c.borrow().is_some()) {
                    update_preview();
                }
                return 0;
            }
            if let Some(index) = id.checked_sub(PICK).filter(|index| *index < SPECS.len()) {
                let spec = &SPECS[index];
                if matches!(spec.kind, Kind::Hotkey) {
                    start_capture(hwnd, index);
                } else {
                    stop_capture();
                    let value = fields()[index].value;
                    let font = matches!(spec.kind, Kind::Font);
                    let current = if font {
                        EDITOR.with(|c| c.borrow().as_ref().unwrap().draft.get(spec.key))
                    } else {
                        text(value)
                    };
                    if let Some(chosen) = crate::settings_pickers::choose(hwnd, spec.key, &current)
                    {
                        if font {
                            let result = EDITOR.with(|c| {
                                c.borrow_mut()
                                    .as_mut()
                                    .unwrap()
                                    .draft
                                    .set(spec.key, &chosen)
                            });
                            if let Err(error) = result {
                                status(&error);
                                return 0;
                            }
                            show_font(value, &chosen);
                        } else {
                            unsafe {
                                SetWindowTextW(value, wide(&chosen).as_ptr());
                            }
                        }
                        if spec.key == "BGFileName" {
                            let skin = SPECS
                                .iter()
                                .position(|spec| spec.key == "ShowSkin")
                                .unwrap();
                            unsafe {
                                SendMessageW(
                                    fields()[skin].value,
                                    BM_SETCHECK,
                                    BST_CHECKED as usize,
                                    0,
                                );
                            }
                        }
                        update_preview();
                    }
                }
                return 0;
            }
            if let Some(index) = id.checked_sub(CLEAR).filter(|index| *index < SPECS.len()) {
                stop_capture();
                unsafe {
                    SetWindowTextW(fields()[index].value, wide("").as_ptr());
                }
                status("已清除。点击“保存并应用”生效。");
                return 0;
            }
            match id {
                2310 => {
                    stop_capture();
                    apply();
                }
                2311 => {
                    stop_capture();
                    if crate::input_dialog::confirm(
                        hwnd,
                        "恢复默认",
                        "将恢复默认设置，保存后生效。",
                    ) {
                        load_form(&Preferences::default());
                        update_preview();
                        status("已载入默认设置，保存后生效。");
                    }
                }
                2312 => {
                    stop_capture();
                    if let Some(path) = choose_file(false) {
                        let result = std::fs::read(&path)
                            .map_err(|e| e.to_string())
                            .and_then(|b| altrun_probe::library::decode(&b))
                            .and_then(|t| Preferences::parse(&t));
                        match result {
                            Ok(mut prefs) => {
                                prefs.set("AutoRun", "0").unwrap();
                                prefs.set("AddToSendTo", "0").unwrap();
                                load_form(&prefs);
                                update_preview();
                                status("已导入，保存后生效。请按需重新启用系统集成。");
                            }
                            Err(e) => status(&e),
                        }
                    }
                }
                2313 => {
                    stop_capture();
                    if let Some(prefs) = validated_form()
                        && let Some(path) = choose_file(true)
                    {
                        let source =
                            EDITOR.with(|c| c.borrow().as_ref().unwrap().file.path.clone());
                        if path == source
                            || std::fs::canonicalize(&path)
                                .ok()
                                .is_some_and(|p| Some(p) == std::fs::canonicalize(&source).ok())
                        {
                            status("请选择另一个文件用于导出。");
                            return 0;
                        }
                        match TextFile::open(&path)
                            .and_then(|(mut file, _)| file.save(&prefs.text()))
                        {
                            Ok(()) => status("设置已导出。"),
                            Err(e) => status(&e),
                        }
                    }
                }
                2314 => unsafe {
                    SendMessageW(hwnd, WM_CLOSE, 0, 0);
                },
                _ => {}
            }
            0
        }
        WM_CLOSE => {
            stop_capture();
            let dirty = read_form().map_or(true, |prefs| {
                EDITOR.with(|c| c.borrow().as_ref().unwrap().saved != prefs)
            });
            if dirty && !crate::input_dialog::confirm(hwnd, "关闭设置", "放弃尚未保存的设置？")
            {
                return 0;
            }
            unsafe {
                DestroyWindow(hwnd);
            }
            0
        }
        WM_DESTROY => {
            stop_capture();
            unsafe {
                PostQuitMessage(0);
            }
            0
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, w, l) },
    }
}
pub fn open(path: PathBuf, prefs: Preferences, owner: HWND, sender: mpsc::Sender<Change>) {
    if WINDOW
        .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        let h = WINDOW.load(Ordering::Acquire);
        if h > 1 {
            unsafe {
                ShowWindow(h as HWND, SW_SHOW);
                SetForegroundWindow(h as HWND);
            }
        }
        return;
    }
    let owner = owner as usize;
    std::thread::spawn(move || {
        let result = (|| -> Result<(), String> {
            let (file, _) = TextFile::open(&path.with_extension("settings.ini"))?;
            let mut prefs = prefs;
            crate::localization::install(&prefs.get("Lang"));
            if let Ok(integration) = altrun_probe::integration::Integration::for_data(&path)
                && let Ok((startup, send_to)) = integration.status()
            {
                let _ = prefs.set("AutoRun", if startup { "1" } else { "0" });
                let _ = prefs.set("AddToSendTo", if send_to { "1" } else { "0" });
            }
            theme::install_chrome(&prefs);
            let background = unsafe { CreateSolidBrush(crate::theme::surface()) };
            let paper = unsafe { CreateSolidBrush(crate::theme::paper()) };
            unsafe {
                let instance = GetModuleHandleW(std::ptr::null());
                let class = wide("ALTRun.Native.Settings");
                let mut wc: WNDCLASSW = std::mem::zeroed();
                wc.lpfnWndProc = Some(procedure);
                wc.hInstance = instance;
                wc.lpszClassName = class.as_ptr();
                wc.hCursor = LoadCursorW(null_mut(), IDC_ARROW);
                wc.hbrBackground = null_mut();
                RegisterClassW(&wc);
                let hwnd = CreateWindowExW(
                    0,
                    class.as_ptr(),
                    ui("ALTRun · 设置").as_ptr(),
                    WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX,
                    CW_USEDEFAULT,
                    CW_USEDEFAULT,
                    700,
                    490,
                    null_mut(),
                    null_mut(),
                    instance,
                    std::ptr::null(),
                );
                if hwnd.is_null() {
                    return Err("无法创建设置窗口".into());
                }
                WINDOW.store(hwnd as usize, Ordering::Release);
                theme::frame(hwnd);
                let dpi = crate::window_geometry::fit(hwnd, 1010, 595, None);
                let font = make_font(15, 400, "Microsoft YaHei UI", dpi);
                let heading_font = make_font(25, 700, "Microsoft YaHei UI", dpi);
                let section_font = make_font(15, 600, "Microsoft YaHei UI", dpi);
                let common = INITCOMMONCONTROLSEX {
                    dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
                    dwICC: ICC_BAR_CLASSES,
                };
                InitCommonControlsEx(&common);
                let control = |class: &str, label: &str, style: u32, x, y, width, height, id| {
                    let style = if class == "BUTTON" && style & 15 != BS_AUTOCHECKBOX as u32 {
                        (style & !15) | BS_OWNERDRAW as u32
                    } else {
                        style
                    };
                    let h = CreateWindowExW(
                        0,
                        wide(class).as_ptr(),
                        wide(&crate::localization::tr(label)).as_ptr(),
                        WS_CHILD | WS_VISIBLE | style,
                        scaled(x, dpi),
                        scaled(y, dpi),
                        scaled(width, dpi),
                        scaled(height, dpi),
                        hwnd,
                        id as HMENU,
                        instance,
                        std::ptr::null(),
                    );
                    SendMessageW(h, WM_SETFONT, font as usize, 0);
                    h
                };
                let tabs = control(
                    "LISTBOX",
                    "",
                    WS_TABSTOP
                        | (LBS_OWNERDRAWFIXED | LBS_HASSTRINGS | LBS_NOTIFY | LBS_NOINTEGRALHEIGHT)
                            as u32,
                    0,
                    14,
                    163,
                    280,
                    TAB,
                );
                SendMessageW(tabs, LB_SETITEMHEIGHT, 0, scaled(38, dpi) as isize);
                for page in PAGES {
                    SendMessageW(tabs, LB_ADDSTRING, 0, ui(page.name).as_ptr() as isize);
                }
                let title = control("STATIC", "", 0, 190, 16, 790, 34, 0);
                SendMessageW(title, WM_SETFONT, heading_font as usize, 0);
                let hint = control("STATIC", "", 0, 190, 54, 790, 28, 0);
                let status = control("STATIC", "", 0, 190, 519, 790, 25, STATUS);
                let preview_controls = [
                    control("STATIC", "启动面板预览", 0, 578, 88, 410, 24, 0),
                    control("STATIC", "", 0x0d, 578, 113, 410, 227, PREVIEW),
                    control(
                        "EDIT",
                        "",
                        WS_BORDER | WS_TABSTOP | ES_AUTOHSCROLL as u32,
                        588,
                        149,
                        390,
                        30,
                        PREVIEW_QUERY,
                    ),
                ];
                SendMessageW(preview_controls[0], WM_SETFONT, section_font as usize, 0);
                SendMessageW(
                    preview_controls[2],
                    EM_SETCUEBANNER,
                    0,
                    ui("试输快捷字，例如 db").as_ptr() as isize,
                );
                SetWindowTextW(preview_controls[2], wide("d").as_ptr());
                let mut sections = Vec::new();
                for (page, label, x, y) in [
                    (0, "启动与托盘", 190, 89),
                    (0, "资源管理器", 190, 234),
                    (0, "提示", 190, 306),
                    (0, "语言", 190, 454),
                    (1, "全局快捷键", 190, 89),
                    (2, "搜索与执行", 190, 89),
                    (3, "主题", 190, 89),
                    (3, "启动面板背景", 190, 155),
                    (3, "面板与列表", 190, 329),
                    (3, "显示选项", 578, 341),
                    (4, "字体与颜色", 190, 88),
                    (5, "窗口位置", 190, 89),
                    (6, "工作目录与诊断", 190, 89),
                ] {
                    let group = control("STATIC", label, 0, x, y, 372, 23, 0);
                    SendMessageW(group, WM_SETFONT, section_font as usize, 0);
                    sections.push((page, group));
                }
                let mut fields = vec![None; SPECS.len()];
                for (page, layout) in PAGES.iter().enumerate() {
                    for (row, key) in layout.keys.iter().enumerate() {
                        let index = SPECS
                            .iter()
                            .position(|spec| spec.key == *key)
                            .expect("every settings field must have a specification");
                        let spec = &SPECS[index];
                        let (x, y, width) = field_position(page, row);
                        let mut field = Field {
                            page,
                            label: null_mut(),
                            value: null_mut(),
                            pick: null_mut(),
                            clear: null_mut(),
                        };
                        if matches!(spec.kind, Kind::Bool) {
                            field.value = control(
                                "BUTTON",
                                spec.label,
                                BS_AUTOCHECKBOX as u32 | WS_TABSTOP,
                                x,
                                y,
                                width,
                                28,
                                FIELD + index,
                            );
                            theme_ui::switch(field.value, dpi);
                        } else {
                            let label = match *key {
                                "Theme" => "主题",
                                "PositionMode" => "呼出位置",
                                _ => spec.label,
                            };
                            let label_width = if width > 500 { 328 } else { 162 };
                            let value_x = x + label_width;
                            let value_width = width - label_width;
                            field.label =
                                control("STATIC", label, 0, x, y + 4, label_width - 6, 25, 0);
                            let options = choices(key);
                            let hotkey = matches!(spec.kind, Kind::Hotkey);
                            let picker = matches!(spec.kind, Kind::Font)
                                || key.ends_with("Color")
                                || ["BGFileName", "PopupFileName", "WorkingDirectory"]
                                    .contains(key);
                            field.value = if *key == "BGStrength" {
                                let slider = control(
                                    "msctls_trackbar32",
                                    "",
                                    WS_TABSTOP | TBS_NOTICKS,
                                    value_x,
                                    y,
                                    value_width,
                                    28,
                                    FIELD + index,
                                );
                                SendMessageW(slider, TBM_SETRANGEMAX, 1, 100);
                                slider
                            } else if options.is_empty() {
                                control(
                                    "EDIT",
                                    "",
                                    ES_AUTOHSCROLL as u32
                                        | WS_BORDER
                                        | WS_TABSTOP
                                        | if hotkey || matches!(spec.kind, Kind::Font) {
                                            ES_READONLY as u32
                                        } else {
                                            0
                                        },
                                    value_x,
                                    y,
                                    if hotkey {
                                        value_width - 174
                                    } else if picker {
                                        value_width - 90
                                    } else {
                                        value_width
                                    },
                                    28,
                                    FIELD + index,
                                )
                            } else {
                                let combo = control(
                                    "COMBOBOX",
                                    "",
                                    (CBS_DROPDOWNLIST | CBS_OWNERDRAWFIXED | CBS_HASSTRINGS) as u32
                                        | WS_VSCROLL
                                        | WS_TABSTOP,
                                    value_x,
                                    y,
                                    value_width,
                                    140,
                                    FIELD + index,
                                );
                                SendMessageW(
                                    combo,
                                    CB_SETITEMHEIGHT,
                                    usize::MAX,
                                    scaled(26, dpi) as isize,
                                );
                                SendMessageW(combo, CB_SETITEMHEIGHT, 0, scaled(28, dpi) as isize);
                                for (_, name) in options {
                                    SendMessageW(
                                        combo,
                                        CB_ADDSTRING,
                                        0,
                                        ui(name).as_ptr() as isize,
                                    );
                                }
                                combo
                            };
                            if hotkey {
                                field.pick = control(
                                    "BUTTON",
                                    "录制",
                                    WS_TABSTOP,
                                    x + width - 166,
                                    y,
                                    78,
                                    28,
                                    PICK + index,
                                );
                                field.clear = control(
                                    "BUTTON",
                                    "清除",
                                    WS_TABSTOP,
                                    x + width - 80,
                                    y,
                                    80,
                                    28,
                                    CLEAR + index,
                                );
                            } else if picker {
                                field.pick = control(
                                    "BUTTON",
                                    if *key == "BGFileName" {
                                        "换图…"
                                    } else {
                                        "选择…"
                                    },
                                    WS_TABSTOP,
                                    x + width - 82,
                                    y,
                                    82,
                                    28,
                                    PICK + index,
                                );
                            }
                        }
                        assert!(
                            fields[index].replace(field).is_none(),
                            "duplicate settings field"
                        );
                    }
                }
                for (label, id, x, width) in [
                    ("恢复默认", 2311, 16, 112),
                    ("导入…", 2312, 136, 86),
                    ("导出…", 2313, 230, 86),
                    ("关闭", 2314, 769, 84),
                    ("保存并应用", 2310, 861, 128),
                ] {
                    control("BUTTON", label, WS_TABSTOP, x, 554, width, 32, id);
                }
                EDITOR.with(|c| {
                    *c.borrow_mut() = Some(Editor {
                        hwnd,
                        tabs,
                        title,
                        hint,
                        status,
                        background,
                        paper,
                        font,
                        heading_font,
                        section_font,
                        sections,
                        dpi,
                        page: 0,
                        preview: Preview::new(prefs.clone()),
                        preview_controls,
                        preview_font: null_mut(),
                        saved: prefs.clone(),
                        draft: prefs.clone(),
                        fields: fields
                            .into_iter()
                            .map(|field| field.expect("missing settings field"))
                            .collect(),
                        file,
                        data: path,
                        sender,
                        owner,
                    });
                });
                load_form(&prefs);
                show_page(0);
                ShowWindow(hwnd, SW_SHOW);
                SetFocus(tabs);
                let mut msg: MSG = std::mem::zeroed();
                while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
                    if matches!(msg.message, WM_KEYDOWN | WM_SYSKEYDOWN)
                        && record_key(msg.wParam as u32, modifier_bits(|key| GetKeyState(key)))
                    {
                        continue;
                    }
                    if IsDialogMessageW(hwnd, &msg) == 0 {
                        TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    }
                }
                EDITOR.with(|c| {
                    if let Some(e) = c.borrow_mut().take() {
                        DeleteObject(e.font);
                        DeleteObject(e.heading_font);
                        DeleteObject(e.section_font);
                        if !e.preview_font.is_null() {
                            DeleteObject(e.preview_font);
                        }
                        DeleteObject(e.background);
                        DeleteObject(e.paper);
                    }
                });
                UnregisterClassW(class.as_ptr(), instance);
            }
            Ok(())
        })();
        if let Err(e) = result {
            crate::native::show_error(&e);
        }
        WINDOW.store(0, Ordering::Release);
    });
}
pub fn is_open() -> bool {
    WINDOW.load(Ordering::Acquire) != 0
}

fn refresh_style(prefs: &Preferences) {
    let hwnd = EDITOR.with(|c| c.borrow().as_ref().unwrap().hwnd);
    theme::install_chrome(prefs);
    theme::frame(hwnd);
    unsafe {
        let background = CreateSolidBrush(crate::theme::surface());
        let paper = CreateSolidBrush(crate::theme::paper());
        EDITOR.with(|c| {
            let mut s = c.borrow_mut();
            let s = s.as_mut().unwrap();
            DeleteObject(std::mem::replace(&mut s.background, background));
            DeleteObject(std::mem::replace(&mut s.paper, paper));
        });
        RedrawWindow(
            hwnd,
            std::ptr::null(),
            null_mut(),
            0x0001 | 0x0004 | 0x0080 | 0x0100,
        );
    }
}
