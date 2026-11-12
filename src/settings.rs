//! Native settings pages, created only while the settings window is open.
use crate::{
    native::{make_font, scaled, ui, wide},
    settings_layout::{self as layout, BoxRect},
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
        Shell::{DefSubclassProc, SetWindowSubclass},
        WindowsAndMessaging::*,
    },
};

pub const UPDATED: u32 = WM_APP + 12;
const RECORDED: u32 = WM_APP + 1;
const TAB: usize = 2301;
const STATUS: usize = 2303;
const PREVIEW: usize = 2305;
const PREVIEW_QUERY: usize = 2306;
const TITLE_CLOSE: usize = 2307;
const SEGMENT: usize = 2330;
const MORE_PANEL: usize = 2340;
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
const FORMAT_MODE: usize = 2356;
const FORMAT_PREVIEW: usize = 2357;
const FORMAT_DISCLOSE: usize = 2358;
const FORMAT_GUIDE: usize = 2359;
const ADVANCED_NOTE: usize = 2360;
const ALPHA_UNIT: usize = 2361;
const RADIUS_UNIT: usize = 2362;
const ALPHA_HELP: usize = 2363;
const RADIUS_HELP: usize = 2364;
const FORMAT_PRESETS: [&str; 3] = ["%-25s| %s", "%s — %s", "%s"];
const OWNER_STATIC: u32 = 0x0d;

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
        hint: "管理启动方式、系统托盘、资源管理器菜单和提示。",
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
        hint: "将鼠标移到快捷键框上，直接按下新的组合键。",
        keys: &["HotKey1", "HotKey2", "LastItemHotKey"],
    },
    Page {
        name: "搜索与执行",
        hint: "调整快捷字匹配、搜索框输入和执行后的行为。",
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
        hint: "调整主题、启动面板和背景。",
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
        hint: "分别调整标题、输入框与结果的字号，或使用主题颜色。",
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
        name: "窗口位置",
        hint: "设置面板的呼出位置、自动隐藏和管理窗口位置。",
        keys: &[
            "PositionMode",
            "HideDelay",
            "WinLeft",
            "WinTop",
            "ManWinLeft",
            "ManWinTop",
        ],
    },
    Page {
        name: "高级",
        hint: "设置搜索结果、命令执行和诊断选项。",
        keys: &[
            "StandardRegex",
            "ListFormat",
            "WorkingDirectory",
            "AllowDynamicTarget",
            "TraceEnable",
            "Alpha",
            "RoundBorderRadius",
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
    rail: HBRUSH,
    font: HFONT,
    heading_font: HFONT,
    section_font: HFONT,
    small_font: HFONT,
    caption_font: HFONT,
    filename_font: HFONT,
    hotkey_font: HFONT,
    footer_controls: [HWND; 5],
    sections: Vec<(usize, HWND)>,
    extra_controls: Vec<(usize, HWND)>,
    appearance_children: Vec<(HWND, RECT, bool)>,
    dpi: i32,
    page: usize,
    height: i32,
    more_panel: bool,
    more_colors: bool,
    format_expanded: bool,
    scroll: i32,
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
        let message = crate::localization::tr(message);
        if text(hwnd) != message {
            SetWindowTextW(hwnd, wide(&message).as_ptr());
            InvalidateRect(hwnd, std::ptr::null(), 0);
        }
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

fn field(key: &str) -> Field {
    EDITOR.with(|c| c.borrow().as_ref().unwrap().fields[spec_index(key)])
}

fn spec_index(key: &str) -> usize {
    SPECS.iter().position(|spec| spec.key == key).unwrap()
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

fn hidden_field(key: &str) -> bool {
    matches!(key, "Theme" | "ShowSkin" | "WorkingDirectory")
}

fn extra_color(key: &str) -> bool {
    matches!(
        key,
        "TitleColor" | "KeywordColor" | "ListColor" | "SelectionColor"
    )
}

fn image_control(id: usize) -> bool {
    matches!(
        id,
        IMAGE_THUMB | STRENGTH_LABEL | STRENGTH_VALUE | STRENGTH_UNIT | CROP_LABEL
    ) || [FIELD, PICK].iter().any(|start| {
        id.checked_sub(*start)
            .and_then(|i| SPECS.get(i))
            .is_some_and(|spec| matches!(spec.key, "BGFileName" | "BGStrength" | "BGCrop"))
    })
}

fn solid_control(id: usize) -> bool {
    (SOLID_LABEL..=SOLID_PICK).contains(&id)
}

fn background_image() -> bool {
    unsafe { SendMessageW(field("ShowSkin").value, BM_GETCHECK, 0, 0) != 0 }
}

fn shows_in_background_mode(id: usize, image: bool) -> bool {
    (!image_control(id) || image) && (!solid_control(id) || !image)
}

const PRESETS: [(&str, &str, &str); 3] = [
    ("雾蓝", "#DCE7F4", "#263B53"),
    ("青灰", "#E0EBE7", "#253B37"),
    ("紫灰", "#E9E4F1", "#383149"),
];

fn preset(index: usize) -> String {
    if index == 0 {
        return SPECS[spec_index("TitleColor")].default.into();
    }
    let (_, light, dark) = PRESETS[index - 1];
    if theme::is_dark() { dark } else { light }.into()
}

fn swatch_colour(key: &str, value: &str) -> u32 {
    if theme::high_contrast() {
        return theme::paper();
    }
    if value == SPECS[spec_index(key)].default {
        match key {
            "TitleColor" => theme::surface(),
            "SelectionColor" => theme::selection(),
            "KeywordColor" | "ListColor" => theme::paper(),
            _ => altrun_probe::preferences::parse_colour(value).unwrap_or_else(|_| theme::text()),
        }
    } else {
        altrun_probe::preferences::parse_colour(value).unwrap_or_else(|_| theme::text())
    }
}

fn set_colour(key: &str, value: &str) {
    let colour = field(key);
    unsafe {
        SetWindowTextW(colour.value, wide(value).as_ptr());
        InvalidateRect(colour.pick, std::ptr::null(), 0);
        if key == "TextColor" {
            SendMessageW(field("UseCustomTextColor").value, BM_SETCHECK, 1, 0);
        }
        if key == "TitleColor" {
            let window = GetParent(colour.value);
            for id in SOLID_DEFAULT..=SOLID_PICK {
                InvalidateRect(GetDlgItem(window, id as i32), std::ptr::null(), 0);
            }
        }
    }
    update_preview();
}

fn choose_colour(hwnd: HWND, key: &str) {
    let current = text(field(key).value);
    let rgb = swatch_colour(key, &current);
    let initial = format!(
        "#{:02X}{:02X}{:02X}",
        rgb & 255,
        (rgb >> 8) & 255,
        (rgb >> 16) & 255
    );
    if let Some(chosen) = crate::settings_pickers::choose(hwnd, key, &initial) {
        set_colour(key, &chosen);
    }
}

fn update_background_mode() {
    let (window, page, dpi, scroll, height, children) = EDITOR.with(|c| {
        let state = c.borrow();
        let s = state.as_ref().unwrap();
        (
            s.hwnd,
            s.page,
            s.dpi,
            s.scroll,
            s.height,
            s.appearance_children.clone(),
        )
    });
    if page != 3 {
        return;
    }
    let image = background_image();
    let footer = height - scaled(layout::FOOTER, dpi);
    unsafe {
        for (child, base, _) in children {
            let id = GetDlgCtrlID(child) as usize;
            if image_control(id) || solid_control(id) {
                position_appearance_child(
                    child,
                    base,
                    scroll,
                    dpi,
                    footer,
                    shows_in_background_mode(id, image),
                );
            }
        }
        let mut rect = BoxRect::new(214, 284, 290, 227).pixels(dpi);
        OffsetRect(&mut rect, 0, -scroll);
        rect.top = rect.top.max(scaled(150, dpi));
        rect.bottom = rect.bottom.min(footer);
        if rect.top < rect.bottom {
            RedrawWindow(
                window,
                &rect,
                null_mut(),
                RDW_INVALIDATE | RDW_ALLCHILDREN | RDW_NOERASE,
            );
        }
    }
}

fn position_appearance_child(
    child: HWND,
    base: RECT,
    scroll: i32,
    dpi: i32,
    footer: i32,
    enabled: bool,
) {
    let y = base.top - scroll;
    let h = base.bottom - base.top;
    let w = base.right - base.left;
    let top = (scaled(150, dpi) - y).max(0);
    let bottom = h.min(footer - y);
    let visible = enabled && top < bottom;
    unsafe {
        if visible {
            SetWindowPos(
                child,
                null_mut(),
                base.left,
                y,
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
            let clipping = if top != 0 || bottom != h {
                CreateRectRgn(0, top, w, bottom)
            } else {
                null_mut()
            };
            if SetWindowRgn(child, clipping, 0) == 0 && !clipping.is_null() {
                DeleteObject(clipping);
            }
        }
        ShowWindow(child, if visible { SW_SHOWNA } else { SW_HIDE });
    }
}

fn show_page(page: usize) {
    let Some(design) = PAGES.get(page) else {
        return;
    };
    cancel_capture();
    let (window, tabs, title, hint, sections, extras, preview, more_colors, format_expanded) =
        EDITOR.with(|c| {
            let mut editor = c.borrow_mut();
            let editor = editor.as_mut().unwrap();
            editor.page = page;
            (
                editor.hwnd,
                editor.tabs,
                editor.title,
                editor.hint,
                editor.sections.clone(),
                editor.extra_controls.clone(),
                editor.preview_controls,
                editor.more_colors,
                editor.format_expanded,
            )
        });
    let shown = unsafe { IsWindowVisible(window) != 0 };
    unsafe {
        if shown {
            SendMessageW(window, WM_SETREDRAW, 0, 0);
        }
        SendMessageW(tabs, LB_SETCURSEL, page, 0);
        SetWindowTextW(title, ui(design.name).as_ptr());
        SetWindowTextW(hint, ui(design.hint).as_ptr());
        ShowWindow(hint, if page == 6 { SW_HIDE } else { SW_SHOWNA });
        for (spec, field) in SPECS.iter().zip(fields()) {
            let visibility = if field.page == page
                && !hidden_field(spec.key)
                && (spec.key != "ListFormat" || format_expanded)
                && (!extra_color(spec.key) || more_colors)
            {
                SW_SHOWNA
            } else {
                SW_HIDE
            };
            for hwnd in [field.label, field.value, field.pick, field.clear] {
                if !hwnd.is_null() {
                    ShowWindow(
                        hwnd,
                        if matches!(spec.kind, Kind::Text)
                            && spec.key.ends_with("Color")
                            && hwnd == field.value
                        {
                            SW_HIDE
                        } else {
                            visibility
                        },
                    );
                }
            }
        }
        for (group, hwnd) in sections {
            ShowWindow(hwnd, if group == page { SW_SHOWNA } else { SW_HIDE });
        }
        for (group, hwnd) in extras {
            let format_hidden = GetDlgCtrlID(hwnd) == FORMAT_GUIDE as i32 && !format_expanded;
            ShowWindow(
                hwnd,
                if group == page && !format_hidden {
                    SW_SHOWNA
                } else {
                    SW_HIDE
                },
            );
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
    layout_page();
    if page == 6 {
        refresh_format(window);
    }
    update_preview();
    unsafe {
        if shown {
            SendMessageW(window, WM_SETREDRAW, 1, 0);
        }
        RedrawWindow(
            window,
            std::ptr::null(),
            null_mut(),
            RDW_INVALIDATE | RDW_ALLCHILDREN | RDW_NOERASE,
        );
    }
}

fn layout_page() {
    let (hwnd, dpi, page, expanded, more_colors, scroll, footer, status, children) =
        EDITOR.with(|c| {
            let state = c.borrow();
            let e = state.as_ref().unwrap();
            (
                e.hwnd,
                e.dpi,
                e.page,
                e.more_panel,
                e.more_colors,
                e.scroll,
                e.footer_controls,
                e.status,
                e.appearance_children.clone(),
            )
        });
    let px = |value| scaled(value, dpi);
    let image = page == 3 && background_image();
    let advanced = page == 6;
    let width = layout::WIDTH;
    let (tabs, title, format_expanded, sections, extras) = EDITOR.with(|c| {
        let state = c.borrow();
        let s = state.as_ref().unwrap();
        (
            s.tabs,
            s.title,
            s.format_expanded,
            s.sections.clone(),
            s.extra_controls.clone(),
        )
    });
    unsafe {
        let mut rect: RECT = std::mem::zeroed();
        GetWindowRect(hwnd, &mut rect);
        let mut monitor: MONITORINFO = std::mem::zeroed();
        monitor.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        GetMonitorInfoW(
            MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST),
            &mut monitor,
        );
        let work = monitor.rcWork;
        let available = (work.bottom - work.top).max(1);
        let target = if page == 3 && expanded {
            936
        } else if page == 4 && more_colors {
            721
        } else if advanced && format_expanded {
            layout::page_height(page) + layout::ADVANCED_EXPANSION
        } else {
            layout::page_height(page)
        };
        let height = px(layout::HEIGHT).min(available);
        let foot = height - px(layout::FOOTER);
        let scroll_max = if (page == 3 && expanded) || (advanced && format_expanded) {
            (px(target) - height).max(0)
        } else {
            0
        };
        let scroll = scroll.clamp(0, scroll_max);
        EDITOR.with(|c| {
            let mut editor = c.borrow_mut();
            let e = editor.as_mut().unwrap();
            e.height = height;
            e.scroll = scroll;
        });
        SetWindowPos(
            hwnd,
            null_mut(),
            rect.left.min(work.right - px(width)).max(work.left),
            rect.top.min(work.bottom - height).max(work.top),
            px(width),
            height,
            SWP_NOZORDER | SWP_NOACTIVATE,
        );
        let old_width = rect.right - rect.left;
        let old_height = rect.bottom - rect.top;
        if old_width != px(width) || old_height != height {
            theme_ui::round_panel(hwnd, px(width), height, dpi);
        }
        for (i, h) in footer.into_iter().enumerate() {
            ShowWindow(
                h,
                if advanced && i < 3 {
                    SW_HIDE
                } else {
                    SW_SHOWNA
                },
            );
            let (x, w) = [(17, 132), (157, 136), (301, 136), (882, 65), (955, 130)][i];
            MoveWindow(h, px(x), foot + px(13), px(w), px(37), 0);
        }
        MoveWindow(
            status,
            px(if advanced { 17 } else { 449 }),
            foot + px(17),
            px(if advanced { 650 } else { 425 }),
            px(25),
            0,
        );
        let tab_height = px(if advanced { 39 } else { 45 });
        MoveWindow(
            tabs,
            px(if advanced { 11 } else { 13 }),
            px(if advanced { 62 } else { 67 }),
            px(if advanced { 134 } else { 160 }),
            tab_height * PAGES.len() as i32,
            0,
        );
        SendMessageW(tabs, LB_SETITEMHEIGHT, 0, tab_height as isize);
        SendMessageW(tabs, LB_SETTOPINDEX, 0, 0);
        MoveWindow(
            title,
            px(if advanced { 177 } else { 214 }),
            px(if advanced { 61 } else { 68 }),
            px(if advanced { 780 } else { 859 }),
            px(38),
            0,
        );
        MoveWindow(
            GetDlgItem(hwnd, TITLE_CLOSE as i32),
            px(width - 47),
            px(1),
            px(46),
            px(41),
            0,
        );
        if advanced {
            let shift = if format_expanded {
                layout::ADVANCED_EXPANSION
            } else {
                0
            };
            let position = |child, x, y, w, h, shown| {
                position_appearance_child(
                    child,
                    BoxRect::new(x, y, w, h).pixels(dpi),
                    scroll,
                    dpi,
                    foot,
                    shown,
                );
            };
            for (key, x, y) in [("Alpha", 469, 433), ("RoundBorderRadius", 458, 495)] {
                let f = field(key);
                position(f.value, x, y + shift, 66, 34, true);
                position(f.label, 189, y - 7 + shift, 236, 21, true);
            }
            if let Some((_, child)) = sections.iter().filter(|(group, _)| *group == 6).nth(1) {
                position(*child, 177, 393 + shift, 391, 21, true);
            }
            for (id, x, y, w, h) in [
                (ALPHA_UNIT, 544, 438 + shift, 18, 22),
                (RADIUS_UNIT, 533, 501 + shift, 31, 22),
                (ALPHA_HELP, 189, 448 + shift, 242, 24),
                (RADIUS_HELP, 189, 511 + shift, 255, 24),
                (FORMAT_GUIDE, 189, 442, 367, 59),
            ] {
                if let Some((_, child)) = extras.iter().find(|(_, h)| GetDlgCtrlID(*h) == id as i32)
                {
                    position(*child, x, y, w, h, id != FORMAT_GUIDE || format_expanded);
                }
            }
        }
        if page == 3 {
            for (child, base, needs_expansion) in children {
                let id = GetDlgCtrlID(child) as usize;
                position_appearance_child(
                    child,
                    base,
                    scroll,
                    dpi,
                    foot,
                    (!needs_expansion || expanded) && shows_in_background_mode(id, image),
                );
            }
        }
        RedrawWindow(
            hwnd,
            std::ptr::null(),
            null_mut(),
            RDW_INVALIDATE | RDW_ALLCHILDREN | RDW_NOERASE,
        );
    }
    if matches!(page, 3 | 4) {
        align_preview();
    }
}

fn align_preview() {
    let (query, dpi, scroll, footer) = EDITOR.with(|c| {
        let state = c.borrow();
        let s = state.as_ref().unwrap();
        (
            s.preview_controls[2],
            s.dpi,
            if s.page == 3 { s.scroll } else { 0 },
            s.height - scaled(layout::FOOTER, s.dpi),
        )
    });
    let query_text = text(query);
    let (panel, input_height) = EDITOR.with(|c| {
        let state = c.borrow();
        let preview = &state.as_ref().unwrap().preview;
        let (bounds, input, _) = preview.geometry(preview.matches(&query_text).len());
        (bounds, input)
    });
    let px = |value| scaled(value, dpi);
    let x = px(layout::PREVIEW.x + panel.x + 9);
    let y = px(layout::PREVIEW.y + panel.y + 35) - scroll;
    let height = px(input_height);
    let width = px(panel.w - 18);
    let top = (px(150) - y).max(0);
    let bottom = height.min(footer - y);
    let position = (x, y, width, height, top, bottom);
    if !EDITOR.with(|c| {
        let mut state = c.borrow_mut();
        let s = state.as_mut().unwrap();
        s.preview_position.replace(position) != Some(position)
    }) {
        return;
    }
    unsafe {
        let visible = top < bottom;
        if visible {
            SetWindowPos(query, HWND_TOP, x, y, width, height, SWP_NOACTIVATE);
            let clip = if top != 0 || bottom != height {
                CreateRectRgn(0, top, width, bottom)
            } else {
                null_mut()
            };
            if SetWindowRgn(query, clip, 0) == 0 && !clip.is_null() {
                DeleteObject(clip);
            }
        }
        ShowWindow(query, if visible { SW_SHOWNA } else { SW_HIDE });
        if visible {
            RedrawWindow(
                query,
                std::ptr::null(),
                null_mut(),
                RDW_INVALIDATE | RDW_FRAME,
            );
        }
    }
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
    if EDITOR.with(|c| {
        let state = c.borrow();
        let s = state.as_ref().unwrap();
        s.preview.prefs == prefs && !s.preview_font.is_null()
    }) {
        return;
    }
    let input_changed = EDITOR.with(|c| {
        let mut editor = c.borrow_mut();
        let editor = editor.as_mut().unwrap();
        let previous = editor.preview.input_colours;
        editor
            .preview
            .update(prefs.clone(), &editor.data, editor.hwnd);
        if editor.preview_paper.is_null() || previous.1 != editor.preview.input_colours.1 {
            unsafe {
                if !editor.preview_paper.is_null() {
                    DeleteObject(editor.preview_paper);
                }
                editor.preview_paper = CreateSolidBrush(editor.preview.input_colours.1);
            }
        }
        previous != editor.preview.input_colours
    });
    refresh_style(&prefs);
    align_preview();
    unsafe {
        let key = prefs.get("KeywordFont");
        let changed = EDITOR.with(|c| c.borrow().as_ref().unwrap().preview_font_key != key);
        if changed && let Ok(font) = altrun_probe::preferences::Font::parse(&key) {
            let font = make_font(font.size_dip(), font.weight, &font.name, dpi);
            if !font.is_null() {
                SendMessageW(controls[2], WM_SETFONT, font as usize, 1);
                let old = EDITOR.with(|c| {
                    let mut state = c.borrow_mut();
                    let s = state.as_mut().unwrap();
                    s.preview_font_key = key;
                    std::mem::replace(&mut s.preview_font, font)
                });
                if !old.is_null() {
                    DeleteObject(old);
                }
            }
        }
        InvalidateRect(controls[1], std::ptr::null(), 0);
        if input_changed {
            RedrawWindow(
                controls[2],
                std::ptr::null(),
                null_mut(),
                RDW_INVALIDATE | RDW_FRAME,
            );
        }
    }
}

fn load_form(prefs: &Preferences) {
    EDITOR.with(|c| {
        let mut state = c.borrow_mut();
        let s = state.as_mut().unwrap();
        s.draft = prefs.clone();
        s.loading_form = true;
    });
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
                if let Ok(font) = altrun_probe::preferences::Font::parse(&value) {
                    SetWindowTextW(field.value, wide(&font.size_dip().to_string()).as_ptr());
                }
            } else if spec.key == "Alpha" {
                SetWindowTextW(
                    field.value,
                    wide(&((prefs.number("Alpha") * 100 + 127) / 255).to_string()).as_ptr(),
                );
            } else if matches!(spec.kind, Kind::Bool) {
                SendMessageW(field.value, BM_SETCHECK, usize::from(value == "1"), 0);
            } else if let Some(index) = choices(spec.key).iter().position(|(key, _)| *key == value)
            {
                SendMessageW(field.value, CB_SETCURSEL, index, 0);
            } else {
                SetWindowTextW(field.value, wide(&value).as_ptr());
                if spec.key.ends_with("Color") {
                    InvalidateRect(field.pick, std::ptr::null(), 0);
                }
            }
        }
    }
    refresh_font_family(prefs);
    sync_sound_picker(prefs.flag("PlayPopupNotify"));
    let window = EDITOR.with(|c| c.borrow().as_ref().unwrap().hwnd);
    unsafe {
        for id in SEGMENT..SEGMENT + 3 {
            InvalidateRect(GetDlgItem(window, id as i32), std::ptr::null(), 0);
        }
    }
    EDITOR.with(|c| c.borrow_mut().as_mut().unwrap().loading_form = false);
    update_background_mode();
    refresh_format(window);
}

fn refresh_format(window: HWND) {
    let format = text(field("ListFormat").value);
    let mode = FORMAT_PRESETS
        .iter()
        .position(|preset| *preset == format)
        .unwrap_or(3);
    unsafe {
        SendMessageW(
            GetDlgItem(window, FORMAT_MODE as i32),
            CB_SETCURSEL,
            mode,
            0,
        );
        InvalidateRect(
            GetDlgItem(window, FORMAT_PREVIEW as i32),
            std::ptr::null(),
            0,
        );
    }
}

fn expand_format(expand: bool) {
    let changed = EDITOR.with(|c| {
        let mut state = c.borrow_mut();
        let s = state.as_mut().unwrap();
        let changed = s.format_expanded != expand;
        s.format_expanded = expand;
        changed
    });
    if changed {
        show_page(6);
    }
}

fn sync_sound_picker(enabled: bool) {
    if let Some(index) = SPECS.iter().position(|spec| spec.key == "PopupFileName") {
        unsafe {
            EnableWindow(fields()[index].pick, i32::from(enabled));
        }
    }
}

fn read_form() -> Result<Preferences, (usize, String)> {
    let mut prefs = EDITOR.with(|c| c.borrow().as_ref().unwrap().draft.clone());
    for (index, (spec, field)) in SPECS.iter().zip(fields()).enumerate() {
        let value = if matches!(spec.kind, Kind::Font) {
            altrun_probe::preferences::Font::resized(&prefs.get(spec.key), &text(field.value))
                .map_err(|error| (index, error))?
        } else if spec.key == "Alpha" {
            let opacity = text(field.value)
                .trim()
                .parse::<i32>()
                .ok()
                .filter(|v| (0..=100).contains(v))
                .ok_or_else(|| (index, "不透明度应在 0–100 之间".to_owned()))?;
            let saved = prefs.number("Alpha");
            if opacity == (saved * 100 + 127) / 255 {
                saved
            } else {
                (opacity * 255 + 50) / 100
            }
            .to_string()
        } else if spec.key == "BGStrength" {
            text(unsafe { GetDlgItem(GetParent(field.value), STRENGTH_VALUE as i32) })
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
    unsafe {
        let value = GetDlgItem(GetParent(field.value), STRENGTH_VALUE as i32);
        let formatted = strength.to_string();
        if text(value) != formatted {
            SetWindowTextW(value, wide(&formatted).as_ptr());
        }
    }
}

fn refresh_font_family(prefs: &Preferences) {
    if let Ok(font) = altrun_probe::preferences::Font::parse(&prefs.get("TitleFont")) {
        let hwnd = EDITOR.with(|c| c.borrow().as_ref().unwrap().hwnd);
        unsafe {
            SetWindowTextW(
                GetDlgItem(hwnd, FONT_FAMILY as i32),
                wide(&font.name).as_ptr(),
            );
        }
    }
}

fn choose_panel_font(hwnd: HWND) {
    let draft = EDITOR.with(|c| c.borrow().as_ref().unwrap().draft.clone());
    if let Some(choice) =
        crate::settings_pickers::choose(hwnd, "TitleFont", &draft.get("TitleFont"))
        && let Ok(font) = altrun_probe::preferences::Font::parse(&choice)
    {
        let prefs = EDITOR.with(|c| {
            let mut editor = c.borrow_mut();
            let draft = &mut editor.as_mut().unwrap().draft;
            for key in ["TitleFont", "KeywordFont", "ListFont"] {
                let value = draft.get(key);
                if let Some((_, rest)) = value.split_once('|') {
                    let _ = draft.set(key, &format!("{}|{rest}", font.name));
                }
            }
            draft.clone()
        });
        refresh_font_family(&prefs);
        update_preview();
    }
}

fn stop_capture() {
    if let Some(recording) = CAPTURE.take() {
        unsafe {
            if !recording.hook.is_null() {
                UnhookWindowsHookEx(recording.hook);
            }
            let control = GetDlgItem(recording.window, (FIELD + recording.index) as i32);
            InvalidateRect(control, std::ptr::null(), 0);
        }
    }
}

fn cancel_capture() {
    stop_capture();
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
    if vk == VK_TAB as u32 && modifiers & !4 == 0 {
        stop_capture();
        return false;
    }
    if vk == VK_ESCAPE as u32 && modifiers == 0 {
        stop_capture();
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
    if altrun_probe::preferences::parse_hotkey(&value).is_err() {
        status("请按字母、数字、功能键或 Pause 等受支持的按键。");
        return true;
    }
    let field = fields()[recording.index];
    if text(field.value) != value {
        unsafe {
            SetWindowTextW(field.value, wide(&value).as_ptr());
            InvalidateRect(field.value, std::ptr::null(), 0);
        }
        status("快捷键已更改，保存后生效。");
    }
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
            if vk == VK_TAB as u32 && modifiers & !4 == 0 {
                return unsafe { CallNextHookEx(null_mut(), code, w, l) };
            }
            unsafe {
                PostMessageW(
                    recording.window,
                    RECORDED,
                    vk as usize,
                    (recording.index << 8 | modifiers) as isize,
                );
            }
            return 1;
        }
    }
    unsafe { CallNextHookEx(null_mut(), code, w, l) }
}

fn start_capture(hwnd: HWND, index: usize) {
    if CAPTURE.get().is_some_and(|capture| capture.index == index) {
        return;
    }
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
    unsafe {
        InvalidateRect(
            GetDlgItem(hwnd, (FIELD + index) as i32),
            std::ptr::null(),
            0,
        );
    }
}

unsafe extern "system" fn hotkey_proc(
    control: HWND,
    msg: u32,
    w: WPARAM,
    l: LPARAM,
    _: usize,
    index: usize,
) -> LRESULT {
    unsafe {
        match msg {
            WM_MOUSEMOVE => {
                let window = GetParent(control);
                if GetForegroundWindow() == window {
                    let mut track = TRACKMOUSEEVENT {
                        cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                        dwFlags: TME_LEAVE,
                        hwndTrack: control,
                        dwHoverTime: 0,
                    };
                    TrackMouseEvent(&mut track);
                    start_capture(window, index);
                }
            }
            WM_SETFOCUS => start_capture(GetParent(control), index),
            WM_MOUSELEAVE | WM_KILLFOCUS | WM_NCDESTROY
                if CAPTURE.get().is_some_and(|capture| capture.index == index) =>
            {
                stop_capture();
            }
            _ => {}
        }
        DefSubclassProc(control, msg, w, l)
    }
}

fn adjust_number(hwnd: HWND, id: usize, delta: i32) {
    let (control, fallback, min, max) = if id == STRENGTH_VALUE {
        let slider = field("BGStrength").value;
        let value = unsafe { SendMessageW(slider, TBM_GETPOS, 0, 0) } as i32;
        (unsafe { GetDlgItem(hwnd, id as i32) }, value, 0, 100)
    } else {
        let Some(index) = id.checked_sub(FIELD) else {
            return;
        };
        let Some(spec) = SPECS.get(index) else { return };
        let value = EDITOR.with(|state| state.borrow().as_ref().unwrap().draft.get(spec.key));
        let (fallback, min, max) = match spec.kind {
            Kind::Font => (
                altrun_probe::preferences::Font::parse(&value).map_or(13, |font| font.size_dip()),
                6,
                128,
            ),
            Kind::Number(min, max) if spec.key != "Alpha" => {
                (value.parse().unwrap_or(min as i32), min as i32, max as i32)
            }
            Kind::Number(..) => (
                (value.parse::<i32>().unwrap_or(255) * 100 + 127) / 255,
                0,
                100,
            ),
            _ => return,
        };
        (fields()[index].value, fallback, min, max)
    };
    let old = text(control);
    let value = old
        .trim()
        .parse::<i32>()
        .unwrap_or(fallback)
        .saturating_add(delta)
        .clamp(min, max)
        .to_string();
    if old != value {
        unsafe {
            SetWindowTextW(control, wide(&value).as_ptr());
        }
    }
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

fn draw(item: &DRAWITEMSTRUCT) {
    if EDITOR.with(|c| c.borrow().is_none()) {
        return;
    }
    if item.CtlType == ODT_BUTTON {
        let id = item.CtlID as usize;
        if (SEGMENT..SEGMENT + 5).contains(&id) {
            let selected = if id < SEGMENT + 3 {
                let selected = unsafe { SendMessageW(field("Theme").value, CB_GETCURSEL, 0, 0) };
                selected == [2, 0, 1][id - SEGMENT]
            } else {
                let image =
                    unsafe { SendMessageW(field("ShowSkin").value, BM_GETCHECK, 0, 0) != 0 };
                image == (id == SEGMENT + 4)
            };
            let focused = item.itemState & ODS_FOCUS != 0;
            let color = if selected {
                theme::selection()
            } else {
                theme::paper()
            };
            theme_ui::fill(item.hDC, &item.rcItem, theme::paper());
            let dpi = EDITOR.with(|c| c.borrow().as_ref().map_or(96, |e| e.dpi));
            theme_ui::rounded(
                item.hDC,
                item.rcItem,
                scaled(8, dpi),
                color,
                if focused {
                    theme::accent()
                } else if selected {
                    theme::border()
                } else {
                    color
                },
            );
            draw_button_text(
                item,
                item.rcItem,
                if selected {
                    theme::text()
                } else {
                    theme::muted()
                },
                DT_CENTER,
            );
        } else if let Some((key, preset_index)) = if (SOLID_DEFAULT..=SOLID_PICK).contains(&id) {
            Some((
                "TitleColor",
                (id < SOLID_PICK).then_some(id - SOLID_DEFAULT),
            ))
        } else {
            id.checked_sub(PICK)
                .and_then(|i| SPECS.get(i))
                .filter(|spec| spec.key.ends_with("Color"))
                .map(|spec| (spec.key, None))
        } {
            let current = text(field(key).value);
            let value = preset_index.map_or_else(|| current.clone(), preset);
            let color = swatch_colour(key, &value);
            let rect = item.rcItem;
            theme_ui::fill(item.hDC, &rect, theme::control_surface());
            let dpi = EDITOR.with(|c| c.borrow().as_ref().unwrap().dpi);
            theme_ui::rounded(
                item.hDC,
                rect,
                scaled(10, dpi),
                theme::paper(),
                if item.itemState & ODS_FOCUS != 0 || (preset_index.is_some() && current == value) {
                    theme::accent()
                } else {
                    theme::border()
                },
            );
            let mut swatch = rect;
            unsafe {
                InflateRect(&mut swatch, -scaled(5, dpi), -scaled(5, dpi));
            }
            if id == SOLID_PICK {
                swatch.right = swatch.left + scaled(16, dpi);
                draw_button_text(
                    item,
                    RECT {
                        left: swatch.right + scaled(4, dpi),
                        ..rect
                    },
                    theme::text(),
                    DT_CENTER,
                );
            }
            theme_ui::rounded(item.hDC, swatch, scaled(4, dpi), color, color);
        } else if let Some(index) = id.checked_sub(FIELD).filter(|index| {
            SPECS
                .get(*index)
                .is_some_and(|spec| matches!(spec.kind, Kind::Hotkey))
        }) {
            let dpi = EDITOR.with(|c| c.borrow().as_ref().unwrap().dpi);
            let rect = item.rcItem;
            let active = CAPTURE.get().is_some_and(|capture| capture.index == index);
            theme_ui::fill(item.hDC, &rect, theme::control_surface());
            theme_ui::rounded(
                item.hDC,
                rect,
                scaled(12, dpi),
                if active {
                    theme::selection()
                } else {
                    theme::paper()
                },
                if active || item.itemState & ODS_FOCUS != 0 {
                    theme::accent()
                } else {
                    theme::border()
                },
            );
            let value = text(item.hwndItem);
            let label = if value.is_empty() {
                crate::localization::tr("未设置")
            } else {
                value
            };
            unsafe {
                let font = SendMessageW(item.hwndItem, WM_GETFONT, 0, 0) as HFONT;
                let old = SelectObject(item.hDC, font);
                theme_ui::line_text(item.hDC, rect, &label, theme::text(), DT_CENTER);
                SelectObject(item.hDC, old);
            }
        } else if id == FONT_FAMILY {
            let dpi = EDITOR.with(|c| c.borrow().as_ref().map_or(96, |e| e.dpi));
            let rect = item.rcItem;
            theme_ui::fill(item.hDC, &rect, theme::control_surface());
            theme_ui::rounded(
                item.hDC,
                rect,
                scaled(12, dpi),
                if item.itemState & ODS_SELECTED != 0 {
                    theme::selection()
                } else {
                    theme::paper()
                },
                if item.itemState & ODS_FOCUS != 0 {
                    theme::accent()
                } else {
                    theme::border()
                },
            );
            draw_button_text(
                item,
                RECT {
                    left: rect.left + scaled(9, dpi),
                    right: rect.right - scaled(9, dpi),
                    ..rect
                },
                theme::text(),
                0,
            );
        } else if id == TITLE_CLOSE {
            theme_ui::caption_close(item, EDITOR.with(|c| c.borrow().as_ref().unwrap().dpi));
        } else if id == FORMAT_DISCLOSE {
            let (dpi, expanded) = EDITOR.with(|c| {
                let state = c.borrow();
                let s = state.as_ref().unwrap();
                (s.dpi, s.format_expanded)
            });
            theme_ui::fill(item.hDC, &item.rcItem, theme::control_surface());
            let mut rect = item.rcItem;
            rect.left += scaled(14, dpi);
            draw_button_text(item, rect, theme::muted(), 0);
            theme_ui::chevron(
                item.hDC,
                POINT {
                    x: item.rcItem.left + scaled(5, dpi),
                    y: (rect.top + rect.bottom) / 2,
                },
                if expanded {
                    theme_ui::Chevron::Down
                } else {
                    theme_ui::Chevron::Right
                },
                dpi,
                theme::muted(),
            );
            if item.itemState & ODS_FOCUS != 0 && item.itemState & ODS_NOFOCUSRECT == 0 {
                unsafe {
                    DrawFocusRect(item.hDC, &item.rcItem);
                }
            }
        } else if matches!(id, MORE_PANEL | MORE_COLORS) {
            let state = EDITOR.with(|c| {
                c.borrow()
                    .as_ref()
                    .map(|s| (s.dpi, s.more_panel, s.more_colors))
            });
            let Some((dpi, panel, colors)) = state else {
                return;
            };
            theme_ui::fill(
                item.hDC,
                &item.rcItem,
                if item.itemState & ODS_SELECTED != 0 {
                    theme::selection()
                } else {
                    theme::control_surface()
                },
            );
            {
                let mut label = item.rcItem;
                label.left += scaled(31, dpi);
                draw_button_text(item, label, theme::text(), 0);
                theme_ui::chevron(
                    item.hDC,
                    POINT {
                        x: item.rcItem.left + scaled(19, dpi),
                        y: (label.top + label.bottom) / 2,
                    },
                    if (id == MORE_PANEL && panel) || (id == MORE_COLORS && colors) {
                        theme_ui::Chevron::Down
                    } else {
                        theme_ui::Chevron::Right
                    },
                    dpi,
                    theme::muted(),
                );
            }
            if item.itemState & ODS_FOCUS != 0 {
                let mut focus = item.rcItem;
                unsafe {
                    let inset = scaled(2, dpi);
                    InflateRect(&mut focus, -inset, -inset);
                    DrawFocusRect(item.hDC, &focus);
                }
            }
        } else {
            let background = if (2310..=2314).contains(&id) {
                theme::chrome()
            } else {
                theme::control_surface()
            };
            let dpi = EDITOR.with(|c| c.borrow().as_ref().map_or(96, |s| s.dpi));
            theme_ui::button_on(item, id == 2310, background, dpi);
        }
        return;
    }
    if item.CtlType == ODT_COMBOBOX {
        let dpi = EDITOR.with(|c| c.borrow().as_ref().map_or(96, |s| s.dpi));
        theme_ui::combo(item, dpi);
        return;
    }
    let format = (item.CtlID as usize == FORMAT_PREVIEW).then(|| text(field("ListFormat").value));
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
        if let Some(format) = format.as_deref() {
            let dc = item.hDC;
            let px = |value| scaled(value, s.dpi);
            let rect = item.rcItem;
            theme_ui::fill(dc, &rect, theme::control_surface());
            theme_ui::rounded(dc, rect, px(10), theme::paper(), theme::divider());
            let row = |x, y| RECT {
                left: rect.left + px(x),
                top: rect.top + px(y),
                right: rect.right - px(8),
                bottom: rect.top + px(y + 18),
            };
            unsafe {
                let old = SelectObject(dc, s.small_font);
                theme_ui::line_text(
                    dc,
                    row(8, 5),
                    &crate::localization::tr("启动面板预览"),
                    theme::muted(),
                    0,
                );
                SelectObject(dc, s.filename_font);
                if format == FORMAT_PRESETS[0] {
                    theme_ui::line_text(dc, row(8, 26), "doubao", theme::text(), 0);
                    theme_ui::line_text(
                        dc,
                        row(144, 26),
                        &crate::localization::tr("豆包"),
                        theme::text(),
                        0,
                    );
                } else {
                    let name = crate::localization::tr("豆包");
                    match altrun_probe::list_format::render(format, "doubao", &name) {
                        Ok(line) => theme_ui::line_text(dc, row(8, 26), &line, theme::text(), 0),
                        Err(error) => theme_ui::line_text(
                            dc,
                            row(8, 26),
                            &crate::localization::tr(&error),
                            theme::error(),
                            0,
                        ),
                    }
                }
                SelectObject(dc, old);
            }
        } else if item.CtlID as usize == PREVIEW {
            unsafe {
                let input = s.preview_controls[2];
                let mut input_rect: RECT = std::mem::zeroed();
                let clip = SaveDC(item.hDC);
                if clip != 0
                    && IsWindowVisible(input) != 0
                    && GetWindowRect(input, &mut input_rect) != 0
                {
                    MapWindowPoints(
                        null_mut(),
                        item.hwndItem,
                        &mut input_rect as *mut RECT as *mut POINT,
                        2,
                    );
                    ExcludeClipRect(
                        item.hDC,
                        input_rect.left,
                        input_rect.top,
                        input_rect.right,
                        input_rect.bottom,
                    );
                }
                s.preview.paint(item, s.dpi, &query);
                if clip != 0 {
                    RestoreDC(item.hDC, clip);
                }
            }
        } else if item.CtlID as usize == STATUS
            || item.CtlID as usize == FIELD + spec_index("BGFileName")
        {
            let id = item.CtlID as usize;
            let face = if id == STATUS {
                theme::chrome()
            } else {
                theme::control_surface()
            };
            theme_ui::fill(item.hDC, &item.rcItem, face);
            let value = text(item.hwndItem);
            let value = if id == FIELD + spec_index("BGFileName") {
                if value.is_empty() {
                    crate::localization::tr("未选择图片")
                } else {
                    std::path::Path::new(&value)
                        .file_name()
                        .map_or(value.clone(), |name| name.to_string_lossy().into_owned())
                }
            } else {
                value
            };
            unsafe {
                let old = SelectObject(item.hDC, s.small_font);
                theme_ui::line_text(item.hDC, item.rcItem, &value, theme::muted(), 0);
                SelectObject(item.hDC, old);
            }
        } else if item.CtlID as usize == IMAGE_THUMB {
            s.preview.paint_thumb(item, s.dpi);
        } else if item.CtlID as usize == TAB
            && let Some(page) = PAGES.get(item.itemID as usize)
        {
            let selected = item.itemState & ODS_SELECTED != 0;
            let mut bounds = item.rcItem;
            theme_ui::fill(item.hDC, &bounds, theme::chrome());
            bounds.bottom -= scaled(5, s.dpi);
            if selected {
                theme_ui::rounded(
                    item.hDC,
                    bounds,
                    scaled(12, s.dpi),
                    theme::selection(),
                    theme::selection(),
                );
                let rail = RECT {
                    left: bounds.left + scaled(3, s.dpi),
                    right: bounds.left + scaled(6, s.dpi),
                    top: bounds.top + scaled(10, s.dpi),
                    bottom: bounds.bottom - scaled(10, s.dpi),
                };
                theme_ui::rounded(
                    item.hDC,
                    rail,
                    scaled(4, s.dpi),
                    theme::accent(),
                    theme::accent(),
                );
            }
            bounds.left += scaled(if s.page == 6 { 10 } else { 15 }, s.dpi);
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
            }
        }
    });
}

fn draw_button_text(item: &DRAWITEMSTRUCT, rect: RECT, color: u32, flags: u32) {
    unsafe {
        let font = SendMessageW(item.hwndItem, WM_GETFONT, 0, 0) as HFONT;
        let old = SelectObject(item.hDC, font);
        theme_ui::line_text(item.hDC, rect, &text(item.hwndItem), color, flags);
        SelectObject(item.hDC, old);
    }
}

fn paint_settings(hwnd: HWND, dc: HDC) {
    let state = EDITOR.with(|c| {
        c.borrow().as_ref().map(|s| {
            (
                s.background,
                s.dpi,
                s.height,
                s.page,
                s.more_panel,
                s.more_colors,
                s.format_expanded,
                s.scroll,
                s.caption_font,
            )
        })
    });
    let mut rect: RECT = unsafe { std::mem::zeroed() };
    unsafe {
        GetClientRect(hwnd, &mut rect);
    }
    let Some((
        background,
        dpi,
        height,
        page,
        more_panel,
        more_colors,
        format_expanded,
        scroll,
        caption,
    )) = state
    else {
        theme_ui::fill(dc, &rect, theme::surface());
        return;
    };
    let px = |v| scaled(v, dpi);
    let divider = theme::divider();
    let advanced = page == 6;
    let foot = height - px(layout::FOOTER);
    let rail = px(if advanced {
        layout::ADVANCED_RAIL
    } else {
        layout::RAIL
    });
    unsafe {
        FillRect(dc, &rect, background);
    }
    theme_ui::fill(
        dc,
        &RECT {
            bottom: px(layout::CAPTION),
            ..rect
        },
        theme::chrome(),
    );
    theme_ui::fill(
        dc,
        &RECT {
            top: px(layout::CAPTION),
            right: rail,
            bottom: foot,
            ..rect
        },
        theme::chrome(),
    );
    theme_ui::fill(dc, &RECT { top: foot, ..rect }, theme::chrome());
    for line in [
        RECT {
            top: px(layout::CAPTION),
            bottom: px(layout::CAPTION) + 1,
            ..rect
        },
        RECT {
            top: foot,
            bottom: foot + 1,
            ..rect
        },
        RECT {
            left: rail - 1,
            right: rail,
            top: px(layout::CAPTION),
            bottom: foot,
        },
    ] {
        theme_ui::fill(dc, &line, divider);
    }
    theme_ui::panel_caption(dc, dpi, caption, "ALTRun 设置");
    unsafe {
        let clip = SaveDC(dc);
        if clip != 0 {
            IntersectClipRect(
                dc,
                px(if advanced { 176 } else { layout::CONTENT }) - 1,
                px(if advanced { 133 } else { 150 }),
                rect.right,
                foot,
            );
        }
        for card in layout::CARDS.iter().filter(|card| card.page == page) {
            let mut bounds = card.bounds;
            if advanced && format_expanded && bounds.x == 177 {
                if bounds.y == 135 {
                    bounds.h += layout::ADVANCED_EXPANSION;
                } else {
                    bounds.y += layout::ADVANCED_EXPANSION;
                }
            }
            if page == 3 && bounds.y == 284 && !background_image() {
                bounds.h = 120;
            }
            if page == 3 && more_panel && bounds.x == layout::RIGHT {
                bounds.h += 215;
            }
            let mut r = bounds.pixels(dpi);
            let card_scroll = if !advanced || card.bounds.y == 419 {
                scroll
            } else {
                0
            };
            OffsetRect(&mut r, 0, -card_scroll);
            theme_ui::rounded(dc, r, px(16), theme::control_surface(), divider);
            for &y in
                card.rules
                    .iter()
                    .chain(if page == 3 && bounds.x == layout::RIGHT && more_panel {
                        &[134, 177, 220, 263, 306][..]
                    } else {
                        &[]
                    })
            {
                let top = px(bounds.y + y) - card_scroll;
                theme_ui::fill(
                    dc,
                    &RECT {
                        left: r.left + 1,
                        top,
                        right: r.right - 1,
                        bottom: top + 1,
                    },
                    divider,
                );
            }
            if page == 3 && bounds.x == layout::RIGHT {
                let x = px(798);
                theme_ui::fill(
                    dc,
                    &RECT {
                        left: x,
                        right: x + 1,
                        top: r.top + 1,
                        bottom: r.top + px(90),
                    },
                    divider,
                );
            }
        }
        if page == 3 {
            for segment in [
                BoxRect::new(226, 191, 266, 38),
                BoxRect::new(226, 295, 266, 38),
            ] {
                let mut r = segment.pixels(dpi);
                OffsetRect(&mut r, 0, -scroll);
                theme_ui::rounded(dc, r, px(14), theme::paper(), theme::border());
            }
        }
        if page == 4 && more_colors {
            let r = BoxRect::new(524, 478, 549, 161).pixels(dpi);
            theme_ui::rounded(dc, r, px(16), theme::control_surface(), divider);
            for y in [518, 558, 598] {
                theme_ui::fill(dc, &BoxRect::new(525, y, 547, 1).pixels(dpi), divider);
            }
        }
        if clip != 0 {
            RestoreDC(dc, clip);
        }
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
        WM_ERASEBKGND => 1,
        WM_PAINT => unsafe {
            let mut paint = PAINTSTRUCT::default();
            let dc = BeginPaint(hwnd, &mut paint);
            paint_settings(hwnd, dc);
            EndPaint(hwnd, &paint);
            0
        },
        WM_PRINTCLIENT => {
            paint_settings(hwnd, w as HDC);
            1
        }
        WM_NCHITTEST => {
            let Some((dpi, width)) =
                EDITOR.with(|c| c.borrow().as_ref().map(|s| (s.dpi, layout::WIDTH)))
            else {
                return HTCLIENT as isize;
            };
            let mut point = POINT {
                x: l as i16 as i32,
                y: (l >> 16) as i16 as i32,
            };
            unsafe {
                ScreenToClient(hwnd, &mut point);
            }
            if (0..scaled(layout::CAPTION, dpi)).contains(&point.y)
                && point.x < scaled(width - 46, dpi)
            {
                HTCAPTION as isize
            } else {
                HTCLIENT as isize
            }
        }
        WM_MOUSEWHEEL => {
            let delta = (w >> 16) as i16 as i32;
            let scroll = EDITOR.with(|c| {
                let mut editor = c.borrow_mut();
                let Some(s) = editor.as_mut() else {
                    return false;
                };
                if !((s.page == 3 && s.more_panel) || (s.page == 6 && s.format_expanded)) {
                    return false;
                }
                s.scroll -= scaled(55, s.dpi) * delta / 120;
                true
            });
            if scroll {
                layout_page();
            }
            0
        }
        WM_DRAWITEM if l != 0 => {
            draw(unsafe { &*(l as *const DRAWITEMSTRUCT) });
            1
        }
        WM_NOTIFY if l != 0 => {
            let header = unsafe { &*(l as *const NMHDR) };
            if header.code == NM_CUSTOMDRAW
                && let Some(spec) = header
                    .idFrom
                    .checked_sub(FIELD)
                    .and_then(|index| SPECS.get(index))
                && spec.key == "BGStrength"
            {
                return theme_ui::slider(unsafe { &*(l as *const NMCUSTOMDRAW) });
            }
            0
        }
        WM_CTLCOLORSTATIC | WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX | WM_CTLCOLORBTN => {
            EDITOR.with(|c| {
                let s = c.borrow();
                let Some(s) = s.as_ref() else {
                    return unsafe { GetSysColorBrush(COLOR_WINDOW) } as isize;
                };
                let control_id = unsafe { GetDlgCtrlID(l as HWND) } as usize;
                if control_id == PREVIEW_QUERY && !s.preview_paper.is_null() {
                    unsafe {
                        SetBkColor(w as HDC, s.preview.input_colours.1);
                        SetTextColor(w as HDC, s.preview.input_colours.2);
                    }
                    return s.preview_paper as isize;
                }
                let filename = ["PopupFileName", "BGFileName"].iter().any(|key| {
                    control_id
                        .checked_sub(FIELD)
                        .and_then(|i| SPECS.get(i))
                        .is_some_and(|spec| spec.key == *key)
                });
                let paper =
                    msg == WM_CTLCOLOREDIT || (msg == WM_CTLCOLORLISTBOX && l as HWND != s.tabs);
                unsafe {
                    SetTextColor(
                        w as HDC,
                        if l as HWND == s.hint
                            || l as HWND == s.status
                            || filename
                            || matches!(
                                control_id,
                                APPEARANCE_NOTE
                                    | ADVANCED_NOTE
                                    | ALPHA_HELP
                                    | RADIUS_HELP
                                    | FORMAT_GUIDE
                            )
                            || IsWindowEnabled(l as HWND) == 0
                        {
                            theme::muted()
                        } else {
                            theme::text()
                        },
                    );
                    SetBkColor(
                        w as HDC,
                        if paper {
                            theme::paper()
                        } else {
                            theme::chrome()
                        },
                    );
                    if !paper && msg == WM_CTLCOLORSTATIC {
                        SetBkMode(w as HDC, TRANSPARENT as i32);
                        return GetStockObject(NULL_BRUSH) as isize;
                    }
                }
                if paper {
                    s.paper as isize
                } else if l as HWND == s.tabs {
                    s.rail as isize
                } else {
                    s.background as isize
                }
            })
        }
        RECORDED => {
            if CAPTURE
                .get()
                .is_some_and(|capture| capture.index == (l as usize >> 8))
            {
                record_key(w as u32, l as usize & 255);
            }
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
                InvalidateRect(GetDlgItem(hwnd, IMAGE_THUMB as i32), std::ptr::null(), 0);
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
        WM_ACTIVATE => {
            if w & 65535 == WA_INACTIVE as usize {
                cancel_capture();
            }
            unsafe {
                InvalidateRect(GetDlgItem(hwnd, TITLE_CLOSE as i32), std::ptr::null(), 0);
                DefWindowProcW(hwnd, msg, w, l)
            }
        }
        WM_COMMAND => {
            let id = w & 65535;
            let code = (w >> 16) as u32;
            if id == FORMAT_MODE && code == CBN_SELCHANGE {
                let selected = unsafe { SendMessageW(l as HWND, CB_GETCURSEL, 0, 0) } as usize;
                if let Some(value) = FORMAT_PRESETS.get(selected) {
                    unsafe {
                        SetWindowTextW(field("ListFormat").value, wide(value).as_ptr());
                    }
                } else if selected == FORMAT_PRESETS.len() {
                    expand_format(true);
                    unsafe {
                        SetFocus(field("ListFormat").value);
                    }
                }
                return 0;
            }
            if id == FORMAT_DISCLOSE && code == BN_CLICKED {
                let expanded = EDITOR.with(|c| c.borrow().as_ref().unwrap().format_expanded);
                expand_format(!expanded);
                return 0;
            }
            if (SEGMENT..SEGMENT + 5).contains(&id) && code == BN_CLICKED {
                unsafe {
                    if id < SEGMENT + 3 {
                        SendMessageW(
                            field("Theme").value,
                            CB_SETCURSEL,
                            [2, 0, 1][id - SEGMENT],
                            0,
                        );
                        for segment in SEGMENT..SEGMENT + 3 {
                            InvalidateRect(GetDlgItem(hwnd, segment as i32), std::ptr::null(), 0);
                        }
                    } else {
                        if background_image() == (id == SEGMENT + 4) {
                            return 0;
                        }
                        SendMessageW(
                            field("ShowSkin").value,
                            BM_SETCHECK,
                            usize::from(id == SEGMENT + 4),
                            0,
                        );
                        update_background_mode();
                    }
                }
                update_preview();
                return 0;
            }
            if (SOLID_DEFAULT..=SOLID_PICK).contains(&id) && code == BN_CLICKED {
                if id == SOLID_PICK {
                    choose_colour(hwnd, "TitleColor");
                } else {
                    set_colour("TitleColor", &preset(id - SOLID_DEFAULT));
                }
                return 0;
            }
            if code == theme_ui::NUMBER_STEP {
                adjust_number(hwnd, id, l as i32);
                return 0;
            }
            if id == STRENGTH_VALUE && code == EN_CHANGE {
                if EDITOR.with(|c| c.borrow().as_ref().is_some_and(|s| !s.loading_form))
                    && let Ok(value) = text(l as HWND).parse::<i32>()
                    && (0..=100).contains(&value)
                {
                    let slider = field("BGStrength").value;
                    unsafe {
                        if SendMessageW(slider, TBM_GETPOS, 0, 0) != value as isize {
                            SendMessageW(slider, TBM_SETPOS, 1, value as isize);
                            update_preview();
                        }
                    }
                }
                return 0;
            }
            if matches!(id, MORE_PANEL | MORE_COLORS) && code == BN_CLICKED {
                let page = EDITOR.with(|c| {
                    let mut editor = c.borrow_mut();
                    let s = editor.as_mut().unwrap();
                    if id == MORE_PANEL {
                        s.more_panel = !s.more_panel;
                        s.scroll = 0;
                    } else {
                        s.more_colors = !s.more_colors;
                    }
                    s.page
                });
                show_page(page);
                return 0;
            }
            if id == FONT_FAMILY && code == BN_CLICKED {
                choose_panel_font(hwnd);
                return 0;
            }
            if id == TAB && code == LBN_SELCHANGE {
                let page = unsafe { SendMessageW(l as HWND, LB_GETCURSEL, 0, 0) };
                if EDITOR.with(|c| c.borrow().as_ref().unwrap().page) != page as usize {
                    show_page(page as usize);
                }
                return 0;
            }
            if id == PREVIEW_QUERY && code == EN_CHANGE {
                let preview = EDITOR.with(|c| {
                    c.borrow_mut().as_mut().map(|e| {
                        e.preview.selected = 0;
                        e.preview_controls[1]
                    })
                });
                if let Some(preview) = preview {
                    align_preview();
                    unsafe {
                        InvalidateRect(preview, std::ptr::null(), 0);
                    }
                }
                return 0;
            }
            if id == PREVIEW && code == STN_CLICKED {
                let canvas = l as HWND;
                let input = unsafe { GetDlgItem(hwnd, PREVIEW_QUERY as i32) };
                let query = text(input);
                let mut cursor = POINT::default();
                if unsafe {
                    GetCursorPos(&mut cursor) != 0 && ScreenToClient(canvas, &mut cursor) != 0
                } {
                    let selected = EDITOR.with(|c| {
                        let mut state = c.borrow_mut();
                        let Some(s) = state.as_mut() else {
                            return false;
                        };
                        let count = s.preview.matches(&query).len();
                        let (panel, input_height, row_height) = s.preview.geometry(count);
                        let px = |n| scaled(n, s.dpi);
                        let top = panel.y + input_height + 39;
                        if cursor.x < px(panel.x + 9) || cursor.x >= px(panel.x + panel.w - 9) {
                            return false;
                        }
                        if let Some(index) = (0..count).find(|i| {
                            let y = top + *i as i32 * row_height;
                            cursor.y >= px(y) && cursor.y < px(y + row_height)
                        }) {
                            s.preview.selected = index;
                            return true;
                        }
                        false
                    });
                    if selected {
                        unsafe {
                            SetFocus(input);
                            InvalidateRect(canvas, std::ptr::null(), 0);
                        }
                    }
                }
                return 0;
            }
            if let Some(index) = id.checked_sub(FIELD).filter(|i| *i < SPECS.len()) {
                let spec = &SPECS[index];
                if spec.key == "ListFormat" && code == EN_CHANGE {
                    if EDITOR.with(|c| c.borrow().as_ref().is_some_and(|s| !s.loading_form)) {
                        refresh_format(hwnd);
                    }
                    return 0;
                }
                let changed = if matches!(spec.kind, Kind::Bool) {
                    code == BN_CLICKED
                } else if !choices(spec.key).is_empty() {
                    code == CBN_SELCHANGE
                } else if matches!(spec.kind, Kind::Font | Kind::Number(..)) {
                    code == EN_CHANGE
                } else {
                    code == EN_KILLFOCUS
                };
                if changed && EDITOR.with(|c| c.borrow().as_ref().is_some_and(|s| !s.loading_form))
                {
                    if spec.key == "PlayPopupNotify" {
                        let checked = unsafe { SendMessageW(l as HWND, BM_GETCHECK, 0, 0) != 0 };
                        sync_sound_picker(checked);
                    }
                    update_preview();
                }
                return 0;
            }
            if let Some(index) = id.checked_sub(PICK).filter(|index| *index < SPECS.len()) {
                let spec = &SPECS[index];
                stop_capture();
                if spec.key.ends_with("Color") {
                    choose_colour(hwnd, spec.key);
                } else {
                    let value = fields()[index].value;
                    if let Some(chosen) =
                        crate::settings_pickers::choose(hwnd, spec.key, &text(value))
                    {
                        unsafe {
                            SetWindowTextW(value, wide(&chosen).as_ptr());
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
                            update_background_mode();
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
                2314 | TITLE_CLOSE => unsafe {
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
            let rail = unsafe { CreateSolidBrush(theme::chrome()) };
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
                    WS_POPUP | WS_SYSMENU,
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
                let dpi = crate::window_geometry::fit(hwnd, layout::WIDTH, layout::HEIGHT, None);
                let font = make_font(15, 400, "Microsoft YaHei UI", dpi);
                let heading_font = make_font(27, 700, "Microsoft YaHei UI", dpi);
                let section_font = make_font(14, 650, "Microsoft YaHei UI", dpi);
                let small_font = make_font(13, 400, "Microsoft YaHei UI", dpi);
                let caption_font = make_font(13, 600, "Microsoft YaHei UI", dpi);
                let filename_font = make_font(12, 400, "Consolas", dpi);
                let hotkey_font = make_font(14, 600, "Consolas", dpi);
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
                        (scaled(x + width, dpi) - scaled(x, dpi)).max(1),
                        (scaled(y + height, dpi) - scaled(y, dpi)).max(1),
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
                    13,
                    67,
                    160,
                    315,
                    TAB,
                );
                SendMessageW(tabs, LB_SETITEMHEIGHT, 0, scaled(45, dpi) as isize);
                for page in PAGES {
                    SendMessageW(tabs, LB_ADDSTRING, 0, ui(page.name).as_ptr() as isize);
                }
                let title = control("STATIC", "", 0, 214, 68, 859, 38, 0);
                SendMessageW(title, WM_SETFONT, heading_font as usize, 0);
                let hint = control("STATIC", "", 0, 214, 110, 859, 26, 0);
                SendMessageW(hint, WM_SETFONT, small_font as usize, 0);
                let status = control("STATIC", "", OWNER_STATIC, 267, 610, 650, 24, STATUS);
                SendMessageW(status, WM_SETFONT, small_font as usize, 0);
                let close = control("BUTTON", "关闭", WS_TABSTOP, 1055, 1, 46, 41, TITLE_CLOSE);
                theme_ui::style_caption_close(close);
                let preview = layout::PREVIEW;
                let preview_controls = [
                    control("STATIC", "启动面板预览", 0, layout::RIGHT, 152, 220, 23, 0),
                    control(
                        "STATIC",
                        "",
                        0x0d | STATIC_NOTIFY | WS_CLIPSIBLINGS,
                        preview.x,
                        preview.y,
                        preview.w,
                        preview.h,
                        PREVIEW,
                    ),
                    control(
                        "EDIT",
                        "",
                        WS_CLIPSIBLINGS | WS_TABSTOP | ES_AUTOHSCROLL as u32,
                        597,
                        250,
                        402,
                        33,
                        PREVIEW_QUERY,
                    ),
                ];
                theme_ui::style_edit(preview_controls[2], dpi, false);
                SendMessageW(preview_controls[0], WM_SETFONT, section_font as usize, 0);
                SendMessageW(
                    preview_controls[2],
                    EM_SETCUEBANNER,
                    0,
                    ui("输入快捷字").as_ptr() as isize,
                );
                SetWindowTextW(preview_controls[2], wide("d").as_ptr());
                let mut sections = Vec::new();
                for card in layout::CARDS {
                    let group = control(
                        "STATIC",
                        card.title,
                        0,
                        card.bounds.x,
                        card.bounds.y - 28,
                        card.bounds.w,
                        21,
                        0,
                    );
                    SendMessageW(group, WM_SETFONT, section_font as usize, 0);
                    sections.push((card.page, group));
                }
                let mut extra_controls = Vec::new();
                for (page, caption, bounds) in [
                    (
                        0,
                        "保存后重新打开设置即可查看。",
                        BoxRect::new(666, 513, 392, 24),
                    ),
                    (3, "可试输快捷字，例如 db", BoxRect::new(866, 152, 207, 21)),
                    (3, "保存后应用到启动面板。", BoxRect::new(524, 435, 549, 22)),
                    (4, "可试输快捷字，例如 db", BoxRect::new(866, 152, 207, 21)),
                    (4, "面板字体", BoxRect::new(228, 196, 72, 26)),
                ] {
                    let style = if bounds.x == 866 { 2 } else { 0 };
                    let control = control(
                        "STATIC",
                        caption,
                        style,
                        bounds.x,
                        bounds.y,
                        bounds.w,
                        bounds.h,
                        APPEARANCE_NOTE,
                    );
                    if bounds.x != 228 {
                        SendMessageW(control, WM_SETFONT, small_font as usize, 0);
                    }
                    extra_controls.push((page, control));
                }
                for (id, caption, x, y, w, h) in [
                    (
                        ADVANCED_NOTE,
                        "仅在启用正则匹配时生效。开启后，匹配以 d 开头的快捷字可用 ^d.*；关闭时可直接输入 d*。",
                        189,
                        170,
                        367,
                        42,
                    ),
                    (0, "结果显示样式", 189, 225, 367, 22),
                    (
                        ADVANCED_NOTE,
                        "让输入参数或上下文决定打开哪个程序或路径。普通快捷项保持关闭。",
                        593,
                        171,
                        352,
                        40,
                    ),
                    (
                        ADVANCED_NOTE,
                        "在快捷项数据文件旁记录启动成功或失败。",
                        593,
                        290,
                        352,
                        29,
                    ),
                    (ALPHA_HELP, "100% 完全不透明。", 189, 448, 242, 24),
                    (RADIUS_HELP, "0 为直角，数值越大角越圆。", 189, 511, 255, 24),
                    (
                        FORMAT_GUIDE,
                        "第一个 %s 显示快捷字，第二个显示名称。\n%-25s 左对齐并补足到 25 字符。宽度可填 0–256。\n%% 显示一个百分号。可填写 1–2 个占位符。",
                        189,
                        442,
                        367,
                        59,
                    ),
                    (ALPHA_UNIT, "%", 544, 438, 18, 22),
                    (RADIUS_UNIT, "DIP", 533, 501, 31, 22),
                ] {
                    let label = control("STATIC", caption, 0, x, y, w, h, id);
                    if caption != "结果显示样式" && !matches!(id, ALPHA_UNIT | RADIUS_UNIT) {
                        SendMessageW(label, WM_SETFONT, small_font as usize, 0);
                    }
                    extra_controls.push((6, label));
                }
                let format_mode = control(
                    "COMBOBOX",
                    "",
                    WS_TABSTOP
                        | WS_VSCROLL
                        | (CBS_DROPDOWNLIST | CBS_OWNERDRAWFIXED | CBS_HASSTRINGS) as u32,
                    189,
                    251,
                    367,
                    148,
                    FORMAT_MODE,
                );
                SendMessageW(
                    format_mode,
                    CB_SETITEMHEIGHT,
                    usize::MAX,
                    scaled(28, dpi) as isize,
                );
                SendMessageW(format_mode, CB_SETITEMHEIGHT, 0, scaled(34, dpi) as isize);
                theme_ui::style_combo(format_mode, dpi);
                for caption in [
                    "默认双列 · 快捷字　名称",
                    "快捷字 — 名称",
                    "仅显示快捷字",
                    "自定义格式",
                ] {
                    SendMessageW(format_mode, CB_ADDSTRING, 0, ui(caption).as_ptr() as isize);
                }
                extra_controls.push((6, format_mode));
                extra_controls.push((
                    6,
                    control(
                        "STATIC",
                        "",
                        OWNER_STATIC,
                        189,
                        292,
                        367,
                        54,
                        FORMAT_PREVIEW,
                    ),
                ));
                let disclose = control(
                    "BUTTON",
                    "自定义写法",
                    WS_TABSTOP,
                    188,
                    349,
                    368,
                    25,
                    FORMAT_DISCLOSE,
                );
                SendMessageW(disclose, WM_SETFONT, small_font as usize, 0);
                extra_controls.push((6, disclose));
                for (id, label, x, y, w, h, page) in [
                    (SEGMENT, "跟随系统", 230, 195, 84, 30, 3),
                    (SEGMENT + 1, "浅色", 317, 195, 84, 30, 3),
                    (SEGMENT + 2, "深色", 404, 195, 84, 30, 3),
                    (SEGMENT + 3, "纯色", 230, 299, 128, 30, 3),
                    (SEGMENT + 4, "图片", 361, 299, 127, 30, 3),
                    (MORE_PANEL, "其他面板选项", 528, 587, 541, 42, 3),
                    (FONT_FAMILY, "", 310, 191, 180, 34, 4),
                    (MORE_COLORS, "其他颜色", 215, 517, 288, 40, 4),
                ] {
                    let button = control("BUTTON", label, WS_TABSTOP, x, y, w, h, id);
                    if (SEGMENT..SEGMENT + 5).contains(&id) {
                        SendMessageW(button, WM_SETFONT, small_font as usize, 0);
                    }
                    extra_controls.push((page, button));
                }
                let thumbnail = control("STATIC", "", 0x0d, 227, 347, 29, 26, IMAGE_THUMB);
                extra_controls.push((3, thumbnail));
                let strength = control(
                    "EDIT",
                    "",
                    WS_TABSTOP | ES_AUTOHSCROLL as u32,
                    405,
                    382,
                    66,
                    30,
                    STRENGTH_VALUE,
                );
                theme_ui::style_number(strength, dpi);
                extra_controls.push((3, strength));
                extra_controls.push((
                    3,
                    control("STATIC", "%", 0, 479, 388, 15, 20, STRENGTH_UNIT),
                ));
                extra_controls.push((
                    3,
                    control("STATIC", "面板底色", 0, 227, 340, 140, 22, SOLID_LABEL),
                ));
                for (index, caption) in [
                    "主题默认色",
                    PRESETS[0].0,
                    PRESETS[1].0,
                    PRESETS[2].0,
                    "自定义…",
                ]
                .into_iter()
                .enumerate()
                {
                    let chip = control(
                        "BUTTON",
                        caption,
                        WS_TABSTOP,
                        227 + index as i32 * 40,
                        367,
                        if index == 4 { 104 } else { 34 },
                        27,
                        SOLID_DEFAULT + index,
                    );
                    extra_controls.push((3, chip));
                }
                let mut fields = vec![None; SPECS.len()];
                for (page, design) in PAGES.iter().enumerate() {
                    for key in design.keys {
                        let index = SPECS
                            .iter()
                            .position(|spec| spec.key == *key)
                            .expect("settings specification");
                        let spec = &SPECS[index];
                        let placement = layout::field(key);
                        let BoxRect { x, y, w, h } = placement.bounds;
                        let label = if placement.caption.is_empty() {
                            spec.label
                        } else {
                            placement.caption
                        };
                        let mut field = Field {
                            page,
                            label: null_mut(),
                            value: null_mut(),
                            pick: null_mut(),
                            clear: null_mut(),
                        };
                        let input_height = 34;
                        let input_y = if *key == "WorkingDirectory" {
                            212
                        } else {
                            y + (h - input_height) / 2
                        };
                        let right = if *key == "BGCrop" { x + w } else { x + w - 13 };
                        let value_x = right - placement.value_width;
                        if matches!(spec.kind, Kind::Bool) {
                            field.value = control(
                                "BUTTON",
                                label,
                                BS_AUTOCHECKBOX as u32 | WS_TABSTOP,
                                x + 4,
                                y + 1,
                                (w - 8).max(1),
                                (h - 1).max(1),
                                FIELD + index,
                            );
                            theme_ui::switch(field.value, dpi, !placement.description.is_empty());
                        } else {
                            let name = matches!(*key, "PopupFileName" | "BGFileName");
                            let hotkey = matches!(spec.kind, Kind::Hotkey);
                            let numeric = matches!(spec.kind, Kind::Font | Kind::Number(..));
                            let color = key.ends_with("Color");
                            let picker = color
                                || matches!(
                                    *key,
                                    "BGFileName" | "PopupFileName" | "WorkingDirectory"
                                );
                            let options = choices(key);
                            let mut label_width = (value_x - x - 20).max(1);
                            if *key == "BGStrength" {
                                label_width = w - 98;
                            } else if matches!(*key, "WorkingDirectory" | "ListFormat") {
                                label_width = 352;
                            }
                            if !matches!(*key, "Theme" | "BGFileName") {
                                field.label = control(
                                    "STATIC",
                                    label,
                                    0,
                                    if matches!(*key, "BGStrength" | "BGCrop") {
                                        x
                                    } else {
                                        x + 13
                                    },
                                    if *key == "WorkingDirectory" {
                                        146
                                    } else if *key == "ListFormat" {
                                        379
                                    } else if *key == "BGStrength" {
                                        y + 2
                                    } else {
                                        y + (h - 21) / 2
                                    },
                                    label_width,
                                    23,
                                    match *key {
                                        "BGStrength" => STRENGTH_LABEL,
                                        "BGCrop" => CROP_LABEL,
                                        _ => 0,
                                    },
                                );
                            }
                            field.value = if hotkey {
                                let handle = control(
                                    "BUTTON",
                                    "",
                                    WS_TABSTOP,
                                    right - 258,
                                    input_y,
                                    194,
                                    input_height,
                                    FIELD + index,
                                );
                                SendMessageW(handle, WM_SETFONT, hotkey_font as usize, 0);
                                SetWindowSubclass(handle, Some(hotkey_proc), 1, index);
                                handle
                            } else if color {
                                control("STATIC", "", 0, 0, 0, 1, 1, FIELD + index)
                            } else if *key == "BGStrength" {
                                let handle = control(
                                    "msctls_trackbar32",
                                    "",
                                    WS_TABSTOP | TBS_NOTICKS,
                                    x,
                                    y + 29,
                                    w,
                                    20,
                                    FIELD + index,
                                );
                                SendMessageW(handle, TBM_SETRANGEMAX, 1, 100);
                                theme_ui::style_slider(handle, dpi);
                                handle
                            } else if !options.is_empty() {
                                let handle = control(
                                    "COMBOBOX",
                                    "",
                                    (CBS_DROPDOWNLIST | CBS_OWNERDRAWFIXED | CBS_HASSTRINGS) as u32
                                        | WS_VSCROLL
                                        | WS_TABSTOP,
                                    value_x,
                                    input_y,
                                    placement.value_width.max(1),
                                    148,
                                    FIELD + index,
                                );
                                SendMessageW(
                                    handle,
                                    CB_SETITEMHEIGHT,
                                    usize::MAX,
                                    scaled(if *key == "BGCrop" { 24 } else { 28 }, dpi) as isize,
                                );
                                SendMessageW(handle, CB_SETITEMHEIGHT, 0, scaled(34, dpi) as isize);
                                theme_ui::style_combo(handle, dpi);
                                for (_, title) in options {
                                    SendMessageW(
                                        handle,
                                        CB_ADDSTRING,
                                        0,
                                        ui(title).as_ptr() as isize,
                                    );
                                }
                                handle
                            } else if name {
                                let left = if *key == "BGFileName" {
                                    x + 37
                                } else {
                                    value_x
                                };
                                let handle = control(
                                    "STATIC",
                                    "",
                                    if *key == "BGFileName" {
                                        OWNER_STATIC
                                    } else {
                                        0x8000
                                    },
                                    left,
                                    y + (h - 18) / 2,
                                    (right - left - 83).max(1),
                                    20,
                                    FIELD + index,
                                );
                                SendMessageW(handle, WM_SETFONT, filename_font as usize, 0);
                                handle
                            } else {
                                let buttons = if *key == "WorkingDirectory" {
                                    69
                                } else if picker {
                                    84
                                } else {
                                    0
                                };
                                let style = ES_AUTOHSCROLL as u32 | WS_TABSTOP;
                                let handle = control(
                                    "EDIT",
                                    "",
                                    style,
                                    value_x,
                                    input_y,
                                    (placement.value_width - buttons).max(1),
                                    input_height,
                                    FIELD + index,
                                );
                                if numeric {
                                    theme_ui::style_number(handle, dpi);
                                } else {
                                    theme_ui::style_edit(handle, dpi, true);
                                }
                                if *key == "WorkingDirectory" {
                                    SendMessageW(
                                        handle,
                                        EM_SETCUEBANNER,
                                        0,
                                        ui("可留空，自动处理").as_ptr() as isize,
                                    );
                                }
                                handle
                            };
                            if hotkey {
                                field.clear = control(
                                    "BUTTON",
                                    "清除",
                                    WS_TABSTOP,
                                    right - 56,
                                    input_y,
                                    56,
                                    34,
                                    CLEAR + index,
                                );
                            } else if picker {
                                field.pick = control(
                                    "BUTTON",
                                    if *key == "BGFileName" {
                                        "选图…"
                                    } else if color {
                                        label
                                    } else {
                                        "选择…"
                                    },
                                    WS_TABSTOP,
                                    if *key == "WorkingDirectory" {
                                        right - 63
                                    } else if color {
                                        right - 50
                                    } else {
                                        right - 73
                                    },
                                    if color { y + (h - 27) / 2 } else { input_y },
                                    if *key == "WorkingDirectory" {
                                        63
                                    } else if color {
                                        50
                                    } else {
                                        73
                                    },
                                    if color { 27 } else { 34 },
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
                let footer_controls = [
                    ("恢复默认", 2311),
                    ("导入设置…", 2312),
                    ("导出设置…", 2313),
                    ("关闭", 2314),
                    ("保存并应用", 2310),
                ]
                .map(|(label, id)| control("BUTTON", label, WS_TABSTOP, 0, 0, 1, 1, id));
                SendMessageW(footer_controls[4], WM_SETFONT, section_font as usize, 0);
                let fields = fields
                    .into_iter()
                    .map(|field| field.expect("missing settings field"))
                    .collect::<Vec<_>>();
                let client_rect = |child: HWND| {
                    let mut bounds: RECT = std::mem::zeroed();
                    GetWindowRect(child, &mut bounds);
                    MapWindowPoints(null_mut(), hwnd, &mut bounds as *mut RECT as *mut POINT, 2);
                    bounds
                };
                let mut appearance_children = Vec::new();
                for (spec, field) in SPECS.iter().zip(&fields) {
                    if field.page != 3 || hidden_field(spec.key) {
                        continue;
                    }
                    let needs_expansion = layout::field(spec.key).bounds.y >= 630;
                    for child in [field.label, field.value, field.pick, field.clear]
                        .into_iter()
                        .filter(|h| !h.is_null())
                    {
                        appearance_children.push((child, client_rect(child), needs_expansion));
                    }
                }
                for (_, child) in sections
                    .iter()
                    .chain(&extra_controls)
                    .filter(|(page, _)| *page == 3)
                {
                    appearance_children.push((*child, client_rect(*child), false));
                }
                for child in [preview_controls[0], preview_controls[1]] {
                    appearance_children.push((child, client_rect(child), false));
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
                        rail,
                        font,
                        heading_font,
                        section_font,
                        small_font,
                        caption_font,
                        filename_font,
                        hotkey_font,
                        footer_controls,
                        sections,
                        extra_controls,
                        appearance_children,
                        dpi,
                        page: 0,
                        height: scaled(layout::HEIGHT, dpi),
                        more_panel: false,
                        more_colors: false,
                        format_expanded: false,
                        scroll: 0,
                        preview: Preview::new(prefs.clone()),
                        preview_controls,
                        preview_font: null_mut(),
                        preview_paper: null_mut(),
                        preview_font_key: String::new(),
                        preview_position: None,
                        loading_form: false,
                        chrome_colors: chrome_colors(),
                        saved: prefs.clone(),
                        draft: prefs.clone(),
                        fields,
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
                    if msg.hwnd == preview_controls[2]
                        && msg.message == WM_KEYDOWN
                        && matches!(msg.wParam as u16, VK_UP | VK_DOWN)
                    {
                        let input = text(preview_controls[2]);
                        EDITOR.with(|c| {
                            let mut editor = c.borrow_mut();
                            let preview = &mut editor.as_mut().unwrap().preview;
                            let last = preview.matches(&input).len().saturating_sub(1);
                            preview.selected = if msg.wParam as u16 == VK_UP {
                                preview.selected.saturating_sub(1)
                            } else {
                                (preview.selected + 1).min(last)
                            };
                        });
                        InvalidateRect(preview_controls[1], std::ptr::null(), 0);
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
                        DeleteObject(e.small_font);
                        DeleteObject(e.caption_font);
                        DeleteObject(e.filename_font);
                        DeleteObject(e.hotkey_font);
                        DeleteObject(e.rail);
                        if !e.preview_font.is_null() {
                            DeleteObject(e.preview_font);
                        }
                        if !e.preview_paper.is_null() {
                            DeleteObject(e.preview_paper);
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
    theme::install_chrome(prefs);
    let colours = chrome_colors();
    let hwnd = EDITOR.with(|c| {
        let mut state = c.borrow_mut();
        let s = state.as_mut().unwrap();
        if s.chrome_colors == colours {
            return None;
        }
        s.chrome_colors = colours;
        Some(s.hwnd)
    });
    let Some(hwnd) = hwnd else { return };
    theme::frame(hwnd);
    unsafe {
        let background = CreateSolidBrush(crate::theme::surface());
        let paper = CreateSolidBrush(crate::theme::paper());
        EDITOR.with(|c| {
            let mut s = c.borrow_mut();
            let s = s.as_mut().unwrap();
            DeleteObject(std::mem::replace(&mut s.background, background));
            DeleteObject(std::mem::replace(&mut s.paper, paper));
            DeleteObject(std::mem::replace(
                &mut s.rail,
                CreateSolidBrush(theme::chrome()),
            ));
        });
        RedrawWindow(
            hwnd,
            std::ptr::null(),
            null_mut(),
            RDW_INVALIDATE | RDW_ALLCHILDREN | RDW_NOERASE,
        );
    }
}

fn chrome_colors() -> [u32; 5] {
    [
        theme::surface(),
        theme::paper(),
        theme::chrome(),
        theme::text(),
        theme::accent(),
    ]
}
