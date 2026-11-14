// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::{
    actions::{handle_command, prepare_backup_restore},
    controls::Controls,
    paint::paint_manager,
    *,
};

unsafe extern "system" fn procedure(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    if msg == WM_NCDESTROY {
        unsafe {
            RemovePropW(hwnd, wide(TABLE_ROW_HEIGHT_PROP).as_ptr());
            return DefWindowProcW(hwnd, msg, w, l);
        }
    }
    if msg == WM_DESTROY {
        unsafe {
            PostQuitMessage(0);
        }
        return 0;
    }
    if msg == FOCUS {
        if unsafe { IsWindowEnabled(hwnd) } == 0 {
            return 0;
        }
        if crate::shortcut_intake::is_open() {
            if w != 0 {
                unsafe {
                    ShowWindow(hwnd, SW_SHOWNOACTIVATE);
                }
            }
            return 0;
        }
        if w != 0 {
            unsafe {
                ShowWindow(hwnd, SW_SHOWNORMAL);
                SetForegroundWindow(hwnd);
            }
        }
        apply_intake(hwnd);
        return 0;
    }
    if msg == WM_ENABLE && w != 0 {
        unsafe { PostMessageW(hwnd, FOCUS, 0, 0) };
    }
    if msg == PREPARE_BACKUP_RESTORE {
        if let Some(reply) = RESTORE_REPLY.lock().ok().and_then(|mut slot| slot.take()) {
            let _ = reply.send(prepare_backup_restore(hwnd));
        }
        return 0;
    }
    if msg == WM_CLOSE {
        let dirty = EDITOR.with(|c| c.borrow().as_ref().is_some_and(Editor::is_dirty));
        if !dirty || confirm(hwnd, "ui.discard_unsaved_changes_and_close") {
            publish_selection();
            unsafe {
                DestroyWindow(hwnd);
            }
        }
        return 0;
    }
    if msg == WM_MEASUREITEM && w == LIST && l != 0 {
        unsafe {
            let height = GetPropW(hwnd, wide(TABLE_ROW_HEIGHT_PROP).as_ptr()) as u32;
            if height != 0 {
                (*(l as *mut MEASUREITEMSTRUCT)).itemHeight = height;
                return 1;
            }
        }
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
        WM_DROPFILES => {
            let paths = drop_paths(w as _);
            add_paths(paths);
            0
        }
        WM_CONTEXTMENU if w as HWND == EDITOR.with(|c| c.borrow().as_ref().unwrap().list) => {
            popup(hwnd, LIST, Some(l));
            0
        }
        WM_NOTIFY if l != 0 => {
            let notice = unsafe { &*(l as *const NMHDR) };
            if notice.idFrom == LIST && notice.code == LVN_ITEMCHANGED {
                let change = unsafe { &*(l as *const NMLISTVIEW) };
                if change.uChanged & LVIF_STATE != 0
                    && EDITOR.with(|c| !c.borrow().as_ref().unwrap().refreshing_list)
                {
                    if change.iItem >= 0
                        && change.uNewState & LVIS_SELECTED != 0
                        && change.uOldState & LVIS_SELECTED == 0
                        && select_view_row(change.iItem as usize)
                    {
                        sync_fields();
                    } else if change.uOldState & LVIS_SELECTED != 0
                        && change.uNewState & LVIS_SELECTED == 0
                    {
                        unsafe {
                            PostMessageW(hwnd, RESTORE_TABLE_SELECTION, 0, 0);
                        }
                    }
                }
            }
            unsafe { DefWindowProcW(hwnd, msg, w, l) }
        }
        RESTORE_TABLE_SELECTION => {
            let (list, selected) = EDITOR.with(|c| {
                let state = c.borrow();
                let s = state.as_ref().unwrap();
                (s.list, s.view.iter().position(|&i| i == s.selected))
            });
            if let Some(row) = selected
                && unsafe {
                    SendMessageW(list, LVM_GETNEXTITEM, usize::MAX, LVNI_SELECTED as LPARAM)
                } < 0
            {
                unsafe {
                    table::select(list, row);
                }
            }
            0
        }
        WM_COMMAND => handle_command(hwnd, w),
        WM_MEASUREITEM if l != 0 => {
            let item = unsafe { &mut *(l as *mut MEASUREITEMSTRUCT) };
            if menu_style::measure(item) {
                1
            } else {
                unsafe { DefWindowProcW(hwnd, msg, w, l) }
            }
        }
        WM_DRAWITEM if l != 0 => {
            let d = unsafe { &*(l as *const DRAWITEMSTRUCT) };
            if menu_style::draw(d) {
                return 1;
            }
            if d.CtlType == ODT_BUTTON {
                if matches!(d.CtlID as usize, NEW_MENU | TOOLS_MENU) {
                    let dpi = EDITOR.with(|c| c.borrow().as_ref().unwrap().dpi);
                    crate::ui::menu_button(d, dpi);
                } else if d.CtlID as usize == CAPTION_CLOSE {
                    crate::ui::caption_close(d, EDITOR.with(|c| c.borrow().as_ref().unwrap().dpi));
                } else {
                    let background = if [DELETE, CLOSE, TEST, SAVE].contains(&(d.CtlID as usize)) {
                        theme::chrome()
                    } else {
                        theme::surface()
                    };
                    let dpi = EDITOR.with(|c| c.borrow().as_ref().unwrap().dpi);
                    crate::ui::button_on(d, d.CtlID as usize == SAVE, background, dpi);
                }
                return 1;
            }
            if d.CtlType == ODT_COMBOBOX {
                let dpi = EDITOR.with(|c| c.borrow().as_ref().unwrap().dpi);
                crate::ui::combo(d, dpi);
                return 1;
            }
            if d.CtlID as usize == LIST {
                EDITOR.with(|c| {
                    let state = c.borrow();
                    let s = state.as_ref().unwrap();
                    let Some(entry) = s.view.get(d.itemID as usize).and_then(|&i| s.draft.get(i))
                    else {
                        return;
                    };
                    table::draw_row(d, &entry.alias, &entry.name, s.font, s.dpi);
                });
            }
            1
        }
        WM_CTLCOLORSTATIC | WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX => EDITOR.with(|c| {
            let state = c.borrow();
            let s = state.as_ref().unwrap();
            unsafe {
                let id = GetDlgCtrlID(l as HWND) as usize;
                let form = (2170..=2174).contains(&id) || id == NOTE_LABEL;
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
                    } else if l as HWND == s.filter_hint {
                        theme::muted()
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

pub(super) fn run(
    path: PathBuf,
    initial: Vec<Entry>,
    parent: usize,
    sender: altrun_probe::library_updates::LibraryWriter,
    launch_enabled: bool,
    show: bool,
) -> Result<(), String> {
    if !path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("tsv"))
    {
        return Err("ui.select_an_altrunnext_shortcut_data_file_in".into());
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
            ui("ui.altrunnext_manage_shortcuts").as_ptr(),
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
            return Err("ui.could_not_create_the_shortcut_manager_window".into());
        }
        theme::frame(hwnd);
        let remembered = window_state();
        let dpi = crate::ui::geometry::fit(hwnd, WIDTH, HEIGHT);
        crate::ui::round_panel(hwnd, scaled(WIDTH, dpi), scaled(HEIGHT, dpi), dpi);
        let common = INITCOMMONCONTROLSEX {
            dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_LISTVIEW_CLASSES,
        };
        InitCommonControlsEx(&common);
        let font = make_font(14, 400, "Microsoft YaHei UI", dpi);
        let heading = make_font(27, 700, "Microsoft YaHei UI", dpi);
        let caption = make_font(13, 600, "Microsoft YaHei UI", dpi);
        let Some(Controls {
            list,
            filter,
            filter_hint,
            alias,
            name,
            command,
            kind,
            ask_parameter,
            status,
            save,
            delete,
        }) = super::controls::create(hwnd, instance, dpi, font, heading)
        else {
            DestroyWindow(hwnd);
            for font in [font, heading, caption] {
                DeleteObject(font);
            }
            UnregisterClassW(class_name.as_ptr(), instance);
            return Err("ui.could_not_create_the_shortcut_manager_window".into());
        };
        let draft = file.entries.clone();
        EDITOR.with(|c| {
            *c.borrow_mut() = Some(Editor {
                launch_enabled,
                sort: SORT_ORIGINAL,
                view: Vec::new(),
                file,
                draft,
                selected: 0,
                loading: false,
                refreshing_list: false,
                laid_out_kind: None,
                list,
                filter,
                filter_hint,
                query: String::new(),
                alias,
                name,
                command,
                kind,
                ask_parameter,
                status,
                save,
                delete,
                font,
                heading,
                caption,
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
        if show {
            ShowWindow(hwnd, SW_SHOWNORMAL);
            SetForegroundWindow(hwnd);
            SetFocus(list);
        }
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
                            crate::ui::show_error("ui.select_a_shortcut_to_edit_tools_can");
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
                for object in [s.font, s.heading, s.caption] {
                    DeleteObject(object);
                }
            }
        });
        UnregisterClassW(class_name.as_ptr(), instance);
    }
    Ok(())
}
