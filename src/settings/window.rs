// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::{
    paint::{draw, paint_settings},
    *,
};

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
        WM_DRAWITEM if l != 0 => {
            draw(unsafe { &*(l as *const DRAWITEMSTRUCT) });
            1
        }
        WM_NOTIFY if l != 0 => {
            let header = unsafe { &*(l as *const NMHDR) };
            if header.idFrom == EMAIL_LINK && matches!(header.code, NM_CLICK | NM_RETURN) {
                open_about_link(hwnd, CONTACT_URI, "ui.couldn_t_open_the_default_mail_app");
                return 0;
            }
            if header.code == NM_CUSTOMDRAW
                && let Some(spec) = header
                    .idFrom
                    .checked_sub(FIELD)
                    .and_then(|index| SPECS.get(index))
                && spec.key == "BGStrength"
            {
                return ui::slider(unsafe { &*(l as *const NMCUSTOMDRAW) });
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
                        if control_id == EMAIL_LINK {
                            theme::accent()
                        } else if l as HWND == s.status
                            || filename
                            || control_id == ABOUT_SECONDARY
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
        preview::READY => {
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
        OPEN_PAGE if w < PAGES.len() => {
            show_page(w);
            0
        }
        WM_COMMAND => handle_command(hwnd, w, l),
        BACKUP_RESULT => {
            backup::complete(hwnd);
            0
        }
        WM_CLOSE => {
            if backup::busy() {
                return 0;
            }
            stop_capture();
            let dirty = form_is_dirty();
            if dirty
                && !crate::ui::dialogs::input::confirm(
                    hwnd,
                    "ui.close_settings",
                    "ui.discard_unsaved_settings",
                )
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

fn handle_command(hwnd: HWND, w: WPARAM, l: LPARAM) -> LRESULT {
    let id = w & 65535;
    let code = (w >> 16) as u32;
    if backup::busy() {
        return 0;
    }
    if code == BN_CLICKED && matches!(id, CREATE_BACKUP | RESTORE_BACKUP) {
        backup::open(hwnd, id == RESTORE_BACKUP);
        return 0;
    }
    if code == BN_CLICKED
        && let Some((_, _, _, url)) = PROJECT_LINKS.iter().find(|(action, _, _, _)| *action == id)
    {
        open_about_link(hwnd, url, "ui.couldn_t_open_the_link_open_the");
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
    if code == ui::NUMBER_STEP {
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
    if id == MORE_COLORS && code == BN_CLICKED {
        toggle_extra_colors(hwnd);
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
        if unsafe { GetCursorPos(&mut cursor) != 0 && ScreenToClient(canvas, &mut cursor) != 0 } {
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
        let changed = if switch_field(spec) {
            code == BN_CLICKED
        } else if spec.key == "Lang" || !choices(spec.key).is_empty() {
            code == CBN_SELCHANGE
        } else if matches!(spec.kind, Kind::Font | Kind::Number(..)) {
            code == EN_CHANGE
        } else {
            code == EN_KILLFOCUS
        };
        if changed && EDITOR.with(|c| c.borrow().as_ref().is_some_and(|s| !s.loading_form)) {
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
            if let Some(chosen) = pickers::choose(hwnd, spec.key, &text(value)) {
                unsafe {
                    SetWindowTextW(value, wide(&chosen).as_ptr());
                }
                if spec.key == "BGFileName" {
                    let skin = SPECS
                        .iter()
                        .position(|spec| spec.key == "ShowSkin")
                        .unwrap();
                    unsafe {
                        SendMessageW(fields()[skin].value, BM_SETCHECK, BST_CHECKED as usize, 0);
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
        status("ui.shortcut_cleared_select_save_and_apply");
        return 0;
    }
    match id {
        2310 => {
            stop_capture();
            apply();
        }
        2311 => {
            stop_capture();
            if crate::ui::dialogs::input::confirm(
                hwnd,
                "ui.reset_defaults",
                "ui.load_default_settings_save_to_apply_them",
            ) {
                load_form(&Preferences::default());
                update_preview();
                sync_save_button(hwnd);
                status("ui.defaults_loaded_save_to_apply_them");
            }
        }
        2312 => {
            stop_capture();
            if let Some(path) = choose_file(false) {
                let result = std::fs::read(&path)
                    .map_err(|e| e.to_string())
                    .and_then(|b| {
                        altrun_probe::library::decode(&b).map_err(|_| {
                            altrun_probe::preferences::SETTINGS_IMPORT_FORMAT_ERROR.to_owned()
                        })
                    })
                    .and_then(|t| Preferences::import(&t));
                match result {
                    Ok(mut prefs) => {
                        prefs.set("AutoRun", "0").unwrap();
                        prefs.set("AddToSendTo", "0").unwrap();
                        load_form(&prefs);
                        update_preview();
                        sync_save_button(hwnd);
                        status("ui.settings_imported_review_system_integration_options_and");
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
                let source = EDITOR.with(|c| c.borrow().as_ref().unwrap().file.path.clone());
                if path == source
                    || std::fs::canonicalize(&path)
                        .ok()
                        .is_some_and(|p| Some(p) == std::fs::canonicalize(&source).ok())
                {
                    status("ui.choose_a_different_file_for_export");
                    return 0;
                }
                match TextFile::open(&path).and_then(|(mut file, _)| file.save(&prefs.text())) {
                    Ok(()) => status("ui.settings_exported"),
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

pub(crate) fn open(
    path: PathBuf,
    prefs: Preferences,
    owner: HWND,
    sender: mpsc::Sender<Change>,
    page: Option<usize>,
) {
    if WINDOW
        .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        let h = WINDOW.load(Ordering::Acquire);
        if h > 1 {
            unsafe {
                if let Some(page) = page {
                    PostMessageW(h as HWND, OPEN_PAGE, page, 0);
                }
                ShowWindow(h as HWND, SW_SHOW);
                SetForegroundWindow(h as HWND);
            }
        }
        return;
    }
    let owner = owner as usize;
    std::thread::spawn(move || {
        let result = (|| -> Result<(), String> {
            localization::refresh_packs();
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
                wc.hIcon = ui::app_icon(GetSystemMetrics(SM_CXICON));
                wc.hbrBackground = null_mut();
                RegisterClassW(&wc);
                let hwnd = CreateWindowExW(
                    0,
                    class.as_ptr(),
                    ui("settings.window_title").as_ptr(),
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
                    return Err("ui.could_not_create_the_settings_window".into());
                }
                theme::frame(hwnd);
                let dpi = crate::ui::geometry::fit(hwnd, layout::WIDTH, layout::HEIGHT);
                ui::round_panel(
                    hwnd,
                    scaled(layout::WIDTH, dpi),
                    scaled(layout::HEIGHT, dpi),
                    dpi,
                );
                let font = make_font(15, 400, "Microsoft YaHei UI", dpi);
                let heading_font = make_font(27, 700, "Microsoft YaHei UI", dpi);
                let about_font = make_font(23, 700, "Microsoft YaHei UI", dpi);
                let section_font = make_font(14, 650, "Microsoft YaHei UI", dpi);
                let small_font = make_font(13, 400, "Microsoft YaHei UI", dpi);
                let caption_font = make_font(13, 600, "Microsoft YaHei UI", dpi);
                let filename_font = make_font(12, 400, "Consolas", dpi);
                let hotkey_font = make_font(14, 600, "Consolas", dpi);
                let common = INITCOMMONCONTROLSEX {
                    dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
                    dwICC: ICC_BAR_CLASSES | ICC_LINK_CLASS,
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
                let tab_height = scaled(45, dpi);
                SendMessageW(tabs, LB_SETITEMHEIGHT, 0, tab_height as isize);
                SetWindowPos(
                    tabs,
                    null_mut(),
                    0,
                    0,
                    scaled(160, dpi),
                    tab_height * PAGES.len() as i32,
                    SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
                );
                for page in PAGES {
                    SendMessageW(tabs, LB_ADDSTRING, 0, ui(page.name).as_ptr() as isize);
                }
                let title = control("STATIC", "", 0, 214, 68, 859, 38, 0);
                SendMessageW(title, WM_SETFONT, heading_font as usize, 0);
                let status_label = control(
                    "STATIC",
                    "",
                    OWNER_STATIC,
                    449,
                    layout::FOOTER_BUTTON_Y,
                    405,
                    layout::FOOTER_BUTTON_HEIGHT,
                    STATUS,
                );
                SendMessageW(status_label, WM_SETFONT, small_font as usize, 0);
                let close = control(
                    "BUTTON",
                    "ui.close",
                    WS_TABSTOP,
                    1055,
                    1,
                    46,
                    41,
                    TITLE_CLOSE,
                );
                ui::style_caption_close(close);
                let preview = layout::PREVIEW;
                let preview_controls = [
                    control(
                        "STATIC",
                        "ui.launcher_preview",
                        0,
                        layout::RIGHT,
                        152,
                        220,
                        23,
                        0,
                    ),
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
                ui::style_edit_with_colours(preview_controls[2], dpi, preview_edit_colours);
                SendMessageW(preview_controls[0], WM_SETFONT, section_font as usize, 0);
                SendMessageW(
                    preview_controls[2],
                    EM_SETCUEBANNER,
                    0,
                    ui("ui.type_a_shortcut").as_ptr() as isize,
                );
                SetWindowTextW(preview_controls[2], wide("cl").as_ptr());
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
                let version = format!(
                    "{} {}",
                    crate::localization::tr("about.version"),
                    env!("CARGO_PKG_VERSION")
                );
                let about_page = layout::ABOUT_PAGE;
                let centered_text = ui::STATIC_CENTER_IMAGE;
                for (label, x, y, width, height, style, face, id) in [
                    (
                        "ALTRunNext",
                        286,
                        145,
                        500,
                        35,
                        centered_text,
                        about_font,
                        0,
                    ),
                    (
                        version.as_str(),
                        286,
                        176,
                        500,
                        23,
                        centered_text,
                        small_font,
                        ABOUT_SECONDARY,
                    ),
                    ("about.author", 230, 226, 180, 24, centered_text, font, 0),
                    ("weimeng54", 590, 226, 467, 24, centered_text | 2, font, 0),
                    ("about.email", 230, 281, 180, 24, centered_text, font, 0),
                    (
                        "about.project_links",
                        214,
                        348,
                        500,
                        26,
                        centered_text,
                        section_font,
                        0,
                    ),
                ] {
                    let child = control("STATIC", label, style, x, y, width, height, id);
                    SendMessageW(child, WM_SETFONT, face as usize, 0);
                    extra_controls.push((about_page, child));
                }
                let email = control(
                    "SysLink",
                    &format!(r#"<a href="{CONTACT_URI}">{CONTACT_EMAIL}</a>"#),
                    WS_TABSTOP | LWS_TRANSPARENT | LWS_RIGHT,
                    785,
                    283,
                    272,
                    25,
                    EMAIL_LINK,
                );
                let link_colors = LITEM {
                    mask: LIF_ITEMINDEX | LIF_STATE,
                    state: LIS_DEFAULTCOLORS,
                    stateMask: LIS_DEFAULTCOLORS,
                    ..Default::default()
                };
                SendMessageW(email, LM_SETITEM, 0, &link_colors as *const LITEM as isize);
                extra_controls.push((about_page, email));
                for (row, (id, label, button, url)) in PROJECT_LINKS.into_iter().enumerate() {
                    let y = layout::ABOUT_LINKS.y + row as i32 * 68;
                    let heading = control("STATIC", label, centered_text, 230, y + 10, 590, 25, 0);
                    SendMessageW(heading, WM_SETFONT, section_font as usize, 0);
                    let address = control("STATIC", url, 0, 230, y + 36, 590, 30, ABOUT_SECONDARY);
                    SendMessageW(address, WM_SETFONT, small_font as usize, 0);
                    let button_width =
                        if matches!(localization::current().locale(), "en" | "zh-Hans") {
                            126
                        } else {
                            178
                        };
                    let action = control(
                        "BUTTON",
                        button,
                        WS_TABSTOP,
                        1057 - button_width,
                        y + 16,
                        button_width,
                        36,
                        id,
                    );
                    if id == OPEN_RELEASES {
                        SendMessageW(action, WM_SETFONT, section_font as usize, 0);
                    }
                    extra_controls.extend([
                        (about_page, heading),
                        (about_page, address),
                        (about_page, action),
                    ]);
                }
                let panel_font_label = control("STATIC", "ui.panel_font", 0, 228, 196, 72, 26, 0);
                extra_controls.push((4, panel_font_label));
                for (id, label, x, y, w, h, page) in [
                    (SEGMENT, "settings.follow_system", 230, 195, 84, 30, 3),
                    (SEGMENT + 1, "ui.light", 317, 195, 84, 30, 3),
                    (SEGMENT + 2, "ui.dark", 404, 195, 84, 30, 3),
                    (SEGMENT + 3, "ui.solid", 230, 299, 128, 30, 3),
                    (SEGMENT + 4, "ui.image", 361, 299, 127, 30, 3),
                    (FONT_FAMILY, "", 310, 191, 180, 34, 4),
                    (MORE_COLORS, "ui.other_colors", 215, 517, 288, 40, 4),
                    (CREATE_BACKUP, "backup.create", 229, 590, 175, 34, 0),
                    (RESTORE_BACKUP, "backup.restore", 415, 590, 187, 34, 0),
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
                ui::style_number(strength, dpi);
                extra_controls.push((3, strength));
                extra_controls.push((
                    3,
                    control("STATIC", "%", 0, 479, 388, 15, 20, STRENGTH_UNIT),
                ));
                extra_controls.push((
                    3,
                    control(
                        "STATIC",
                        "ui.panel_color",
                        0,
                        227,
                        340,
                        140,
                        22,
                        SOLID_LABEL,
                    ),
                ));
                for (index, caption) in [
                    "ui.theme_default_color",
                    PRESETS[0].0,
                    PRESETS[1].0,
                    PRESETS[2].0,
                    "ui.custom",
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
                let mut fields = vec![
                    Field {
                        page: usize::MAX,
                        label: null_mut(),
                        value: null_mut(),
                        pick: null_mut(),
                        clear: null_mut(),
                    };
                    SPECS.len()
                ];
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
                        let input_y = y + (h - input_height) / 2;
                        let right = if *key == "BGCrop" { x + w } else { x + w - 13 };
                        let value_x = right - placement.value_width;
                        if switch_field(spec) {
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
                            ui::switch(field.value, dpi);
                        } else {
                            let name = matches!(*key, "PopupFileName" | "BGFileName");
                            let hotkey = matches!(spec.kind, Kind::Hotkey);
                            let numeric = matches!(spec.kind, Kind::Font | Kind::Number(..));
                            let color = key.ends_with("Color");
                            let picker = color || matches!(*key, "BGFileName" | "PopupFileName");
                            let options = choices(key);
                            let mut label_width = (value_x - x - 20).max(1);
                            if *key == "BGStrength" {
                                label_width = w - 98;
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
                                    if *key == "BGStrength" {
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
                                ui::style_slider(handle, dpi);
                                handle
                            } else if *key == "Lang" || !options.is_empty() {
                                let handle = control(
                                    "COMBOBOX",
                                    "",
                                    (CBS_DROPDOWNLIST | CBS_OWNERDRAWFIXED | CBS_HASSTRINGS) as u32
                                        | WS_VSCROLL
                                        | WS_TABSTOP,
                                    value_x,
                                    input_y,
                                    placement.value_width.max(1),
                                    if *key == "Lang" { 280 } else { 148 },
                                    FIELD + index,
                                );
                                SendMessageW(
                                    handle,
                                    CB_SETITEMHEIGHT,
                                    usize::MAX,
                                    scaled(if *key == "BGCrop" { 24 } else { 28 }, dpi) as isize,
                                );
                                SendMessageW(handle, CB_SETITEMHEIGHT, 0, scaled(34, dpi) as isize);
                                ui::style_combo(handle, dpi);
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
                                let style = ES_AUTOHSCROLL as u32 | WS_TABSTOP;
                                let handle = control(
                                    "EDIT",
                                    "",
                                    style,
                                    value_x,
                                    input_y,
                                    placement.value_width.max(1),
                                    input_height,
                                    FIELD + index,
                                );
                                if numeric {
                                    ui::style_number(handle, dpi);
                                } else {
                                    ui::style_edit(handle, dpi);
                                }
                                handle
                            };
                            if hotkey {
                                field.clear = control(
                                    "BUTTON",
                                    "ui.clear",
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
                                        "ui.browse"
                                    } else if color {
                                        label
                                    } else {
                                        "ui.choose"
                                    },
                                    WS_TABSTOP,
                                    if color { right - 50 } else { right - 73 },
                                    if color { y + (h - 27) / 2 } else { input_y },
                                    if color { 50 } else { 73 },
                                    if color { 27 } else { 34 },
                                    PICK + index,
                                );
                            }
                        }
                        assert!(fields[index].value.is_null(), "duplicate settings field");
                        fields[index] = field;
                    }
                }
                for (label, id, x, width) in [
                    ("ui.reset_defaults", 2311, 17, 132),
                    ("ui.import_settings", 2312, 157, 136),
                    ("ui.export_settings", 2313, 301, 136),
                    ("ui.close", 2314, 862, 85),
                    ("ui.save_and_apply", 2310, 955, 130),
                ] {
                    let button = control(
                        "BUTTON",
                        label,
                        WS_TABSTOP,
                        x,
                        layout::FOOTER_BUTTON_Y,
                        width,
                        layout::FOOTER_BUTTON_HEIGHT,
                        id,
                    );
                    if id == 2310 {
                        SendMessageW(button, WM_SETFONT, section_font as usize, 0);
                    }
                }
                let appearance_children = fields
                    .iter()
                    .filter(|field| field.page == 3)
                    .flat_map(|field| [field.label, field.value, field.pick, field.clear])
                    .chain(
                        extra_controls
                            .iter()
                            .filter(|(page, _)| *page == 3)
                            .map(|(_, child)| *child),
                    )
                    .filter(|child| !child.is_null())
                    .collect();
                EDITOR.with(|c| {
                    *c.borrow_mut() = Some(Editor {
                        hwnd,
                        tabs,
                        title,
                        status: status_label,
                        background,
                        paper,
                        rail,
                        font,
                        heading_font,
                        about_font,
                        section_font,
                        small_font,
                        caption_font,
                        filename_font,
                        hotkey_font,
                        sections,
                        extra_controls,
                        appearance_children,
                        dpi,
                        page: 0,
                        height: scaled(layout::HEIGHT, dpi),
                        more_colors: false,
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
                        languages: Vec::new(),
                        file,
                        data: path,
                        sender,
                        owner,
                    });
                });
                load_form(&prefs);
                report_language_packs(&prefs);
                show_page(page.unwrap_or(0));
                WINDOW.store(hwnd as usize, Ordering::Release);
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
                        DeleteObject(e.about_font);
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
            crate::ui::show_error(&e);
        }
        WINDOW.store(0, Ordering::Release);
    });
}
