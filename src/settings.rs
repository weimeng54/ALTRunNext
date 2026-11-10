//! Lazy native property editor, running independently of the launch window.
use crate::native::{make_font, ui, wide};
use altrun_probe::{
    preferences::{Kind, Preferences, SPECS},
    state_store::TextFile,
};
use std::{
    cell::RefCell,
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
    UI::{Controls::Dialogs::*, WindowsAndMessaging::*},
};
pub const UPDATED: u32 = WM_APP + 12;
pub enum Change {
    Prepare(u64, Preferences, mpsc::Sender<Result<(), String>>),
    Commit(u64),
    Cancel(u64),
}
static WINDOW: AtomicUsize = AtomicUsize::new(0);
struct Editor {
    background: HBRUSH,
    paper: HBRUSH,
    boolean: HWND,
    picker: HWND,
    dpi: i32,
    prefs: Preferences,
    file: TextFile,
    data: PathBuf,
    list: HWND,
    value: HWND,
    status: HWND,
    index: usize,
    dirty: bool,
    loading: bool,
    font: HFONT,
    sender: mpsc::Sender<Change>,
    owner: usize,
}
thread_local! {static EDITOR:RefCell<Option<Editor>>=const{RefCell::new(None)};}
fn text(hwnd: HWND) -> String {
    unsafe {
        let mut b = vec![0; GetWindowTextLengthW(hwnd) as usize + 1];
        let n = GetWindowTextW(hwnd, b.as_mut_ptr(), b.len() as i32);
        String::from_utf16_lossy(&b[..n as usize])
    }
}
fn status(message: &str) {
    let h = EDITOR.with(|c| c.borrow().as_ref().unwrap().status);
    unsafe {
        SetWindowTextW(h, wide(message).as_ptr());
    }
}
fn select() {
    let (value, index, prefs, boolean, picker, dpi) = EDITOR.with(|c| {
        let mut s = c.borrow_mut();
        let s = s.as_mut().unwrap();
        s.loading = true;
        (
            s.value,
            s.index,
            s.prefs.clone(),
            s.boolean,
            s.picker,
            s.dpi,
        )
    });
    unsafe {
        SetWindowTextW(value, wide(&prefs.get(SPECS[index].key)).as_ptr());
        let spec = &SPECS[index];
        let is_bool = matches!(spec.kind, Kind::Bool);
        ShowWindow(boolean, if is_bool { SW_SHOW } else { SW_HIDE });
        ShowWindow(value, if is_bool { SW_HIDE } else { SW_SHOW });
        SendMessageW(boolean, BM_SETCHECK, usize::from(prefs.flag(spec.key)), 0);
        let can_pick = matches!(spec.kind, Kind::Font)
            || spec.key.ends_with("Color")
            || ["BGFileName", "PopupFileName", "Lang", "WorkingDirectory"].contains(&spec.key);
        ShowWindow(picker, if can_pick { SW_SHOW } else { SW_HIDE });
        SetWindowPos(
            value,
            null_mut(),
            crate::native::scaled(310, dpi),
            crate::native::scaled(52, dpi),
            crate::native::scaled(if can_pick { 258 } else { 350 }, dpi),
            crate::native::scaled(30, dpi),
            SWP_NOZORDER | SWP_NOACTIVATE,
        );
    }
    EDITOR.with(|c| c.borrow_mut().as_mut().unwrap().loading = false);
    let spec = &SPECS[index];
    status(&format!(
        "{} · {}",
        spec.key,
        match spec.kind {
            Kind::Bool => "0 = 关闭，1 = 开启",
            Kind::Hotkey => "例如 Ctrl+Alt+R；留空可关闭",
            Kind::Font => "字体|[fsBold,fsItalic]|字号|颜色|字符集",
            Kind::Number(_, _) => "输入整数",
            Kind::Text => "输入文本；颜色格式 #RRGGBB",
        }
    ));
}
fn store_value() -> Result<(), String> {
    let (h, index, loading) = EDITOR.with(|c| {
        let s = c.borrow();
        let s = s.as_ref().unwrap();
        (s.value, s.index, s.loading)
    });
    if loading {
        return Ok(());
    }
    let value = text(h);
    EDITOR.with(|c| {
        let mut s = c.borrow_mut();
        let s = s.as_mut().unwrap();
        let changed = s.prefs.get(SPECS[index].key) != value.trim();
        s.prefs.set(SPECS[index].key, &value)?;
        s.dirty |= changed;
        Ok(())
    })
}
fn apply() {
    if let Err(e) = store_value() {
        status(&e);
        return;
    }
    let (prefs, sender, owner, data) = EDITOR.with(|c| {
        let s = c.borrow();
        let s = s.as_ref().unwrap();
        (s.prefs.clone(), s.sender.clone(), s.owner, s.data.clone())
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
            EDITOR.with(|c| c.borrow_mut().as_mut().unwrap().dirty = false);
            refresh_style();
            status("已保存并应用。热键冲突会保留原有可用设置。");
        }
        Err(e) => {
            let _ = sender.send(Change::Cancel(token));
            status(&e);
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
        WM_COMMAND => {
            let id = w & 65535;
            let code = w >> 16;
            if id == 2301 && code == LBN_SELCHANGE as usize {
                if let Err(e) = store_value() {
                    status(&e);
                    let (list, index) = EDITOR.with(|c| {
                        let s = c.borrow();
                        let s = s.as_ref().unwrap();
                        (s.list, s.index)
                    });
                    unsafe {
                        SendMessageW(list, LB_SETCURSEL, index, 0);
                    }
                    return 0;
                }
                let list = EDITOR.with(|c| c.borrow().as_ref().unwrap().list);
                let n = unsafe { SendMessageW(list, LB_GETCURSEL, 0, 0) };
                if n >= 0 {
                    EDITOR.with(|c| c.borrow_mut().as_mut().unwrap().index = n as usize);
                    select();
                }
            }
            match id {
                2304 if code == BN_CLICKED as usize => {
                    let (boolean, value) = EDITOR.with(|c| {
                        let s = c.borrow();
                        let s = s.as_ref().unwrap();
                        (s.boolean, s.value)
                    });
                    let checked = unsafe { SendMessageW(boolean, BM_GETCHECK, 0, 0) != 0 };
                    unsafe {
                        SetWindowTextW(value, wide(if checked { "1" } else { "0" }).as_ptr());
                    }
                }
                2305 => {
                    let (key, value) = EDITOR.with(|c| {
                        let s = c.borrow();
                        let s = s.as_ref().unwrap();
                        (SPECS[s.index].key, s.value)
                    });
                    if let Some(chosen) = crate::settings_pickers::choose(hwnd, key, &text(value)) {
                        unsafe {
                            SetWindowTextW(value, wide(&chosen).as_ptr());
                        }
                    }
                }
                2310 => apply(),
                2311 => {
                    if unsafe {
                        MessageBoxW(
                            hwnd,
                            ui("将恢复默认设置；点击保存后才生效。").as_ptr(),
                            ui("恢复默认").as_ptr(),
                            MB_OKCANCEL,
                        )
                    } == IDOK
                    {
                        EDITOR.with(|c| {
                            let mut s = c.borrow_mut();
                            let s = s.as_mut().unwrap();
                            s.prefs = Preferences::default();
                            s.dirty = true;
                        });
                        select();
                    }
                }
                2312 => {
                    if let Some(path) = choose_file(false) {
                        let result = std::fs::read(&path)
                            .map_err(|e| e.to_string())
                            .and_then(|b| altrun_probe::library::decode(&b))
                            .and_then(|t| Preferences::parse(&t));
                        match result {
                            Ok(mut prefs) => {
                                prefs.set("AutoRun", "0").unwrap();
                                prefs.set("AddToSendTo", "0").unwrap();
                                EDITOR.with(|c| {
                                    let mut s = c.borrow_mut();
                                    let s = s.as_mut().unwrap();
                                    s.prefs = prefs;
                                    s.dirty = true;
                                });
                                select();
                                status("设置已载入草稿，尚未应用。系统集成默认关闭。");
                            }
                            Err(e) => status(&e),
                        }
                    }
                }
                2313 => {
                    if store_value().is_ok()
                        && let Some(path) = choose_file(true)
                    {
                        let source =
                            EDITOR.with(|c| c.borrow().as_ref().unwrap().file.path.clone());
                        if path == source
                            || std::fs::canonicalize(&path)
                                .ok()
                                .is_some_and(|p| Some(p) == std::fs::canonicalize(&source).ok())
                        {
                            status("请选择不同的导出文件；应用当前设置请使用保存。");
                            return 0;
                        }
                        let t = EDITOR.with(|c| c.borrow().as_ref().unwrap().prefs.text());
                        match TextFile::open(&path).and_then(|(mut f, _)| f.save(&t)) {
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
            let dirty = EDITOR.with(|c| {
                c.borrow()
                    .as_ref()
                    .is_some_and(|s| s.dirty || text(s.value) != s.prefs.get(SPECS[s.index].key))
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
                let dpi = crate::window_geometry::fit(hwnd, 680, 440, None);
                let font = make_font(14, 400, "Microsoft YaHei UI", dpi);
                let control = |class: &str, label: &str, style: u32, x, y, width, height, id| {
                    let h = CreateWindowExW(
                        0,
                        wide(class).as_ptr(),
                        wide(&crate::localization::tr(label)).as_ptr(),
                        WS_CHILD | WS_VISIBLE | style,
                        crate::native::scaled(x, dpi),
                        crate::native::scaled(y, dpi),
                        crate::native::scaled(width, dpi),
                        crate::native::scaled(height, dpi),
                        hwnd,
                        id as HMENU,
                        instance,
                        std::ptr::null(),
                    );
                    SendMessageW(h, WM_SETFONT, font as usize, 0);
                    h
                };
                let list = control(
                    "LISTBOX",
                    "",
                    LBS_NOTIFY as u32 | WS_VSCROLL | WS_BORDER,
                    16,
                    16,
                    276,
                    364,
                    2301,
                );
                for spec in SPECS {
                    SendMessageW(
                        list,
                        LB_ADDSTRING,
                        0,
                        wide(&crate::localization::tr(spec.label)).as_ptr() as isize,
                    );
                }
                control("STATIC", "选中设置的值", 0, 310, 20, 350, 24, 0);
                let value = control(
                    "EDIT",
                    "",
                    ES_AUTOHSCROLL as u32 | WS_BORDER | WS_TABSTOP,
                    310,
                    52,
                    350,
                    30,
                    2302,
                );
                let status = control("STATIC", "", 0, 310, 100, 350, 245, 2303);
                let boolean = control(
                    "BUTTON",
                    "启用",
                    BS_AUTOCHECKBOX as u32 | WS_TABSTOP,
                    310,
                    52,
                    350,
                    30,
                    2304,
                );
                let picker = control("BUTTON", "选择…", WS_TABSTOP, 580, 52, 80, 30, 2305);
                for (label, id, x, width) in [
                    ("保存并应用", 2310, 16, 120),
                    ("恢复默认", 2311, 146, 110),
                    ("导入…", 2312, 266, 90),
                    ("导出…", 2313, 366, 90),
                    ("关闭", 2314, 558, 100),
                ] {
                    control("BUTTON", label, WS_TABSTOP, x, 396, width, 30, id);
                }
                EDITOR.with(|c| {
                    *c.borrow_mut() = Some(Editor {
                        background,
                        paper,
                        boolean,
                        picker,
                        dpi,
                        prefs,
                        file,
                        data: path,
                        list,
                        value,
                        status,
                        index: 0,
                        dirty: false,
                        loading: false,
                        font,
                        sender,
                        owner,
                    })
                });
                SendMessageW(list, LB_SETCURSEL, 0, 0);
                select();
                ShowWindow(hwnd, SW_SHOW);
                let mut msg: MSG = std::mem::zeroed();
                while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
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
        (unsafe { GetParent(s.value) }, s.prefs.clone())
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
