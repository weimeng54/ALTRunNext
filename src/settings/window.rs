// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::{
    commands::handle_command,
    controls::Controls,
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
                let Controls {
                    tabs,
                    title,
                    status_label,
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
                    preview_controls,
                    fields,
                } = super::controls::create(hwnd, instance, dpi);
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
