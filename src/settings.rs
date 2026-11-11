//! Native settings pages, created only while the settings window is open.
use crate::native::{make_font, scaled, ui, wide};
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
            "FormWidth",
            "RowHeight",
            "MaxRows",
            "ShowTopTen",
            "ShowOperationHint",
            "ShowCommandLine",
            "ShowShortCutButton",
            "ShowConfigButton",
            "ShowCloseButton",
            "ShowSkin",
            "BGFileName",
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
        hint: "兼容旧版格式、正则语法和诊断选项。",
        keys: &[
            "StandardRegex",
            "ListFormat",
            "WorkingDirectory",
            "AllowDynamicTarget",
            "TraceEnable",
            "AlphaColor",
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
    hint: HWND,
    status: HWND,
    background: HBRUSH,
    paper: HBRUSH,
    font: HFONT,
    saved: Preferences,
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
    stop_capture();
    let (tabs, hint) = EDITOR.with(|c| {
        let editor = c.borrow();
        let editor = editor.as_ref().unwrap();
        (editor.tabs, editor.hint)
    });
    unsafe {
        SendMessageW(tabs, TCM_SETCURSEL, page, 0);
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
    }
}

fn load_form(prefs: &Preferences) {
    for (spec, field) in SPECS.iter().zip(fields()) {
        let value = prefs.get(spec.key);
        unsafe {
            if matches!(spec.kind, Kind::Bool) {
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
    let mut prefs = Preferences::default();
    for (index, (spec, field)) in SPECS.iter().zip(fields()).enumerate() {
        let value = if matches!(spec.kind, Kind::Bool) {
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

fn stop_capture() {
    if let Some(recording) = CAPTURE.take()
        && !recording.hook.is_null()
    {
        unsafe {
            UnhookWindowsHookEx(recording.hook);
        }
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
    let (sender, owner, data) = EDITOR.with(|c| {
        let s = c.borrow();
        let s = s.as_ref().unwrap();
        (s.sender.clone(), s.owner, s.data.clone())
    });
    if let Err(error) = crate::localization::load(
        &prefs.get("Lang"),
        data.parent().unwrap_or(std::path::Path::new(".")),
    ) {
        status(&error);
        return;
    }
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
            EDITOR.with(|c| c.borrow_mut().as_mut().unwrap().saved = prefs.clone());
            load_form(&prefs);
            refresh_style();
            status("已保存并应用。");
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
            let brush = EDITOR
                .with(|c| c.borrow().as_ref().map(|s| s.background))
                .unwrap_or_else(|| unsafe { GetSysColorBrush(COLOR_WINDOW) });
            let mut rect: RECT = unsafe { std::mem::zeroed() };
            unsafe {
                GetClientRect(hwnd, &mut rect);
                FillRect(w as HDC, &rect, brush);
            }
            1
        }
        WM_CTLCOLORSTATIC | WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX | WM_CTLCOLORBTN => {
            EDITOR.with(|c| {
                let s = c.borrow();
                let Some(s) = s.as_ref() else {
                    return unsafe { GetSysColorBrush(COLOR_WINDOW) } as isize;
                };
                unsafe {
                    SetTextColor(w as HDC, crate::theme::text());
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
        WM_KEYDOWN | WM_SYSKEYDOWN
            if record_key(w as u32, modifier_bits(|key| unsafe { GetKeyState(key) })) =>
        {
            0
        }
        WM_ACTIVATE if w & 65535 == WA_INACTIVE as usize => {
            stop_capture();
            0
        }
        WM_NOTIFY if w == TAB && l != 0 => {
            let header = unsafe { &*(l as *const NMHDR) };
            if header.code == TCN_SELCHANGE {
                let page = unsafe { SendMessageW(header.hwndFrom, TCM_GETCURSEL, 0, 0) };
                show_page(page as usize);
            }
            0
        }
        WM_COMMAND => {
            let id = w & 65535;
            if let Some(index) = id.checked_sub(PICK).filter(|index| *index < SPECS.len()) {
                let spec = &SPECS[index];
                if matches!(spec.kind, Kind::Hotkey) {
                    start_capture(hwnd, index);
                } else {
                    stop_capture();
                    let value = fields()[index].value;
                    if let Some(chosen) =
                        crate::settings_pickers::choose(hwnd, spec.key, &text(value))
                    {
                        unsafe {
                            SetWindowTextW(value, wide(&chosen).as_ptr());
                        }
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
                    if unsafe {
                        MessageBoxW(
                            hwnd,
                            ui("将恢复默认设置，保存后生效。").as_ptr(),
                            ui("恢复默认").as_ptr(),
                            MB_OKCANCEL | MB_DEFBUTTON2,
                        )
                    } == IDOK
                    {
                        load_form(&Preferences::default());
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
            if dirty
                && unsafe {
                    MessageBoxW(
                        hwnd,
                        ui("放弃尚未保存的设置？").as_ptr(),
                        ui("关闭设置").as_ptr(),
                        MB_OKCANCEL | MB_DEFBUTTON2,
                    )
                } != IDOK
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
            let _ = crate::localization::install(
                &prefs.get("Lang"),
                path.parent().unwrap_or(std::path::Path::new(".")),
            );
            if let Ok(integration) = altrun_probe::integration::Integration::for_data(&path)
                && let Ok((startup, send_to)) = integration.status()
            {
                let _ = prefs.set("AutoRun", if startup { "1" } else { "0" });
                let _ = prefs.set("AddToSendTo", if send_to { "1" } else { "0" });
            }
            crate::theme::install(&prefs);
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
                let dpi = crate::window_geometry::fit(hwnd, 760, 500, None);
                let font = make_font(14, 400, "Microsoft YaHei UI", dpi);
                let control = |class: &str, label: &str, style: u32, x, y, width, height, id| {
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
                let mut common: INITCOMMONCONTROLSEX = std::mem::zeroed();
                common.dwSize = std::mem::size_of_val(&common) as u32;
                common.dwICC = ICC_TAB_CLASSES;
                InitCommonControlsEx(&common);
                let tabs = control("SysTabControl32", "", WS_TABSTOP, 16, 12, 728, 34, TAB);
                for (index, page) in PAGES.iter().enumerate() {
                    let mut title = ui(page.name);
                    let mut item: TCITEMW = std::mem::zeroed();
                    item.mask = TCIF_TEXT;
                    item.pszText = title.as_mut_ptr();
                    SendMessageW(tabs, TCM_INSERTITEMW, index, &item as *const _ as isize);
                }
                let hint = control("STATIC", "", 0, 24, 54, 712, 25, 0);
                let status = control("STATIC", "", 0, 24, 416, 712, 36, STATUS);
                let mut fields = vec![None; SPECS.len()];
                for (page, layout) in PAGES.iter().enumerate() {
                    for (row, key) in layout.keys.iter().enumerate() {
                        let index = SPECS
                            .iter()
                            .position(|spec| spec.key == *key)
                            .expect("every settings field must have a specification");
                        let spec = &SPECS[index];
                        let y = 78 + row as i32 * 28;
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
                                24,
                                y,
                                710,
                                26,
                                FIELD + index,
                            );
                        } else {
                            let label = match *key {
                                "Theme" => "主题",
                                "PositionMode" => "呼出位置",
                                _ => spec.label,
                            };
                            field.label = control("STATIC", label, 0, 24, y + 3, 330, 26, 0);
                            let options = choices(key);
                            let hotkey = matches!(spec.kind, Kind::Hotkey);
                            let picker = matches!(spec.kind, Kind::Font)
                                || key.ends_with("Color")
                                || ["BGFileName", "PopupFileName", "Lang", "WorkingDirectory"]
                                    .contains(key);
                            field.value = if options.is_empty() {
                                control(
                                    "EDIT",
                                    "",
                                    ES_AUTOHSCROLL as u32
                                        | WS_BORDER
                                        | WS_TABSTOP
                                        | if hotkey { ES_READONLY as u32 } else { 0 },
                                    360,
                                    y,
                                    if hotkey {
                                        216
                                    } else if picker {
                                        280
                                    } else {
                                        376
                                    },
                                    26,
                                    FIELD + index,
                                )
                            } else {
                                let combo = control(
                                    "COMBOBOX",
                                    "",
                                    CBS_DROPDOWNLIST as u32 | WS_VSCROLL | WS_TABSTOP,
                                    360,
                                    y,
                                    376,
                                    140,
                                    FIELD + index,
                                );
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
                                    584,
                                    y,
                                    72,
                                    26,
                                    PICK + index,
                                );
                                field.clear = control(
                                    "BUTTON",
                                    "清除",
                                    WS_TABSTOP,
                                    664,
                                    y,
                                    72,
                                    26,
                                    CLEAR + index,
                                );
                            } else if picker {
                                field.pick = control(
                                    "BUTTON",
                                    "选择…",
                                    WS_TABSTOP,
                                    648,
                                    y,
                                    88,
                                    26,
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
                    ("恢复默认", 2311, 24, 100),
                    ("导入…", 2312, 132, 82),
                    ("导出…", 2313, 222, 82),
                    ("保存并应用", 2310, 508, 120),
                    ("关闭", 2314, 636, 100),
                ] {
                    control("BUTTON", label, WS_TABSTOP, x, 456, width, 30, id);
                }
                EDITOR.with(|c| {
                    *c.borrow_mut() = Some(Editor {
                        hwnd,
                        tabs,
                        hint,
                        status,
                        background,
                        paper,
                        font,
                        saved: prefs.clone(),
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

fn refresh_style() {
    let (hwnd, prefs) = EDITOR.with(|c| {
        let s = c.borrow();
        let s = s.as_ref().unwrap();
        (s.hwnd, s.saved.clone())
    });
    crate::theme::install(&prefs);
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
