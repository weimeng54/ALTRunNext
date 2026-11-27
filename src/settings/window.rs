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
        ui::language::CHANGED => {
            if let Some((tabs, page, language_control)) = EDITOR.with(|c| {
                c.borrow()
                    .as_ref()
                    .map(|s| (s.tabs, s.page, s.fields[spec_index("Lang")].value))
            }) {
                let index = unsafe { SendMessageW(language_control, CB_GETCURSEL, 0, 0) };
                let selected = EDITOR.with(|c| {
                    c.borrow()
                        .as_ref()
                        .and_then(|s| s.languages.get(index as usize).map(|(key, _)| key.clone()))
                });
                unsafe {
                    SendMessageW(tabs, LB_RESETCONTENT, 0, 0);
                    for page in PAGES {
                        SendMessageW(tabs, LB_ADDSTRING, 0, ui(page.name).as_ptr() as LPARAM);
                    }
                    SendMessageW(tabs, LB_SETCURSEL, page, 0);
                }
                if let Some(selected) = selected {
                    super::form::load_language_choice(field("Lang").value, &selected);
                }
            }
            0
        }
        theme::CHANGED => {
            refresh_style();
            super::appearance::refresh_preview(true);
            0
        }
        WM_ERASEBKGND => 1,
        WM_PAINT => unsafe {
            let mut paint = PAINTSTRUCT::default();
            let dc = BeginPaint(hwnd, &mut paint);
            // Transparent labels and native controls share the page background.
            // Present them together, clipped to the region invalidated by this change.
            ui::buffered(dc, paint.rcPaint, |target| {
                SendMessageW(
                    hwnd,
                    WM_PRINT,
                    target as WPARAM,
                    (PRF_CLIENT | PRF_CHILDREN) as LPARAM,
                );
            });
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
            let item = unsafe { &*(l as *const DRAWITEMSTRUCT) };
            draw(item);
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
            {
                let info = unsafe { &*(l as *const NMCUSTOMDRAW) };
                if spec.key == "BGStrength" {
                    return ui::slider(info);
                }
                if pages::divider_checkbox(spec.key)
                    && let Some((dpi, font)) =
                        EDITOR.with(|c| c.borrow().as_ref().map(|s| (s.dpi, s.font)))
                {
                    return ui::check_option(info, dpi, font, theme::control_surface(), false);
                }
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
        WM_HSCROLL if l != 0 && EDITOR.with(|c| c.borrow().is_some()) => {
            let slider = field("BGStrength");
            if l as HWND == slider.value {
                let strength = unsafe { SendMessageW(slider.value, TBM_GETPOS, 0, 0) } as i32;
                strength_label(slider, strength);
                form::commit_fields(&["BGStrength"]);
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
        OPEN_PAGE => {
            let requested = WINDOW.lock().unwrap().requested_page.take();
            let Some(page) = requested else { return 0 };
            apply::reopen();
            if page < PAGES.len() {
                show_page(page);
            }
            unsafe {
                ShowWindow(hwnd, SW_SHOW);
                SetForegroundWindow(hwnd);
            }
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
            form::finish_edit(hwnd);
            apply::after_edits(hwnd, apply::After::Close);
            0
        }
        apply::EXIT => {
            stop_capture();
            form::finish_edit(hwnd);
            apply::after_edits(hwnd, apply::After::Exit);
            0
        }
        apply::READY => {
            apply::completed(hwnd);
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
    safe_mode: bool,
) {
    {
        let mut slot = WINDOW.lock().unwrap();
        if slot.exiting {
            return;
        }
        if slot.handle != 0 {
            slot.requested_page = Some(page.unwrap_or(usize::MAX));
            if slot.handle > 1 {
                unsafe {
                    PostMessageW(slot.handle as HWND, OPEN_PAGE, 0, 0);
                }
            }
            return;
        }
        slot.handle = 1;
    }
    let owner = owner as usize;
    std::thread::spawn(move || {
        let exit_sender = sender.clone();
        let result = (|| -> Result<(), String> {
            localization::refresh_packs();
            let mut prefs = prefs;
            crate::localization::install(&prefs.get("Lang"));
            theme::install_chrome(&prefs);
            let (file, _) = TextFile::open(&path.with_extension("settings.ini"))?;
            if let Ok(integration) = altrun_probe::integration::Integration::for_data(&path)
                && let Ok((startup, send_to)) = integration.status()
            {
                let _ = prefs.set("AutoRun", if startup { "1" } else { "0" });
                let _ = prefs.set("AddToSendTo", if send_to { "1" } else { "0" });
            }
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
                    crate::ui::window_frame::STYLE,
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

                ui::language::caption(hwnd, "settings.window_title");
                let dpi = crate::ui::window_frame::fit(hwnd, layout::WIDTH, layout::HEIGHT);
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
                let writes = apply::Writes::start(
                    file,
                    path.clone(),
                    sender.clone(),
                    owner,
                    hwnd,
                    safe_mode,
                )?;
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
                        writes,
                        data: path,
                        sender,
                        owner,
                    });
                });
                load_form(&prefs);
                report_language_packs(&prefs);
                show_page(page.unwrap_or(0));
                {
                    let mut slot = WINDOW.lock().unwrap();
                    slot.handle = hwnd as usize;
                    if slot.exiting {
                        PostMessageW(hwnd, apply::EXIT, 0, 0);
                    } else if slot.requested_page.is_some() {
                        PostMessageW(hwnd, OPEN_PAGE, 0, 0);
                    }
                }
                ShowWindow(hwnd, SW_SHOW);
                SetFocus(tabs);
                let mut msg: MSG = std::mem::zeroed();
                while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
                    ui::option_focus_cues(hwnd, &msg);
                    if matches!(msg.message, WM_KEYDOWN | WM_SYSKEYDOWN)
                        && record_key(msg.wParam as u32, modifier_bits(|key| GetKeyState(key)))
                    {
                        continue;
                    }
                    if msg.message == WM_KEYDOWN
                        && msg.wParam as u16 == VK_RETURN
                        && let Some(key) = form::edit_key(hwnd, msg.hwnd)
                        && !crate::search_ime::is_composing(msg.hwnd)
                    {
                        form::commit_fields(&[key]);
                        continue;
                    }
                    if ui::dialogs::close_on_escape(hwnd, &msg) {
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
        let followup = {
            let mut slot = WINDOW.lock().unwrap();
            slot.handle = 0;
            let page = slot.requested_page.take();
            if slot.exiting {
                Some(Change::ExitForRecovery)
            } else {
                page.map(|index| Change::Reopen((index < PAGES.len()).then_some(index)))
            }
        };
        if let Some(change) = followup {
            let _ = exit_sender.send(change);
            unsafe {
                PostMessageW(owner as HWND, UPDATED, 0, 0);
            }
        }
    });
}
