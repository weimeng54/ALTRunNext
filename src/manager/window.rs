// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::*;

fn list_height(dpi: i32) -> i32 {
    let row = scaled(ROW_HEIGHT, dpi);
    scaled(LIST_TOP, dpi) - scaled(TABLE_TOP, dpi)
        + scaled(FOOTER_TOP - LIST_TOP - 33, dpi) / row * row
}

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
        WM_COMMAND => {
            let id = w & 0xffff;
            let notification = (w >> 16) & 0xffff;
            if [ALIAS, NAME, COMMAND].contains(&id) && notification == EN_CHANGE as usize
                || id == KIND && notification == CBN_SELCHANGE as usize
                || id == ASK_PARAMETER && notification == BN_CLICKED as usize
            {
                update_draft(id);
            } else if notification == BN_CLICKED as usize {
                match id {
                    CAPTION_CLOSE => unsafe {
                        PostMessageW(hwnd, WM_CLOSE, 0, 0);
                    },
                    NEW_MENU | TOOLS_MENU => popup(hwnd, id, None),
                    OPEN_WITH | OPEN_DEFAULT => change_open_with(hwnd, id == OPEN_DEFAULT),
                    CHOOSE_TARGET => choose_command_target(hwnd),
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
                                open_target: None,
                                kind: library::EntryKind::Command,
                            });
                            s.selected = s.draft.len() - 1;
                        });
                        refresh_list();
                        status("ui.enter_an_alias_and_a_command_for", false);
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
                            && confirm(
                                hwnd,
                                &crate::localization::message(
                                    "ui.remove_name_from_the_draft_save_to",
                                    &[("name", &name)],
                                ),
                            )
                        {
                            EDITOR.with(|c| {
                                let mut state = c.borrow_mut();
                                let s = state.as_mut().unwrap();
                                s.draft.remove(s.selected);
                                s.selected = s.selected.min(s.draft.len().saturating_sub(1));
                            });
                            refresh_list();
                            pending_status("ui.removed_from_the_draft_changes_are_unsaved");
                        }
                    }
                    SAVE => save(),
                    TEST => test_selected(hwnd),
                    CLOSE => unsafe {
                        PostMessageW(hwnd, WM_CLOSE, 0, 0);
                    },
                    ADD_FILE | ADD_FOLDER | VALIDATE | EXPORT | RESTORE | SORT_ORIGINAL
                    | SORT_ALIAS | SORT_NAME | MOVE_UP | MOVE_DOWN => tool(hwnd, id),
                    _ => {}
                }
            }
            0
        }
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
        let control = |class: &str, label: &str, style: u32, x, y, width, height, id: usize| {
            let id = if class == "STATIC" && id == 0 {
                match label {
                    "ui.alias" => 2170,
                    "ui.name" => 2171,
                    "ui.command_or_path" => 2172,
                    "ui.input_format" => 2173,
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
                CAPTION_CLOSE => (WIDTH - 47, 1, 46, CAPTION - 1),
                NEW_MENU => (RAIL - 180, 62, 78, 32),
                TOOLS_MENU => (RAIL - 94, 62, 78, 32),
                LIST => (17, TABLE_TOP + 1, RAIL - 34, 0),
                DELETE => (17, FOOTER_TOP + 13, 86, 37),
                CLOSE => (WIDTH - 338, FOOTER_TOP + 13, 65, 37),
                TEST => (WIDTH - 265, FOOTER_TOP + 13, 110, 37),
                SAVE => (WIDTH - 147, FOOTER_TOP + 13, 130, 37),
                NOTE_LABEL => (FIELD_LEFT, 128, FIELD_WIDTH, 21),
                EMPTY_HELP => (FIELD_LEFT, 128, FIELD_WIDTH, 40),
                ALIAS => (FIELD_LEFT, 154, HALF_FIELD, 36),
                NAME => (FIELD_RIGHT - HALF_FIELD, 154, HALF_FIELD, 36),
                COMMAND => (FIELD_LEFT, 238, FIELD_WIDTH - 116, 36),
                CHOOSE_TARGET => (FIELD_RIGHT - 106, 238, 106, 36),
                KIND => (FIELD_LEFT, 378, FIELD_WIDTH, 190),
                ASK_PARAMETER => (FIELD_LEFT, 293, FIELD_WIDTH, 40),
                OPEN_LABEL => (FIELD_LEFT, 296, FIELD_WIDTH, 21),
                OPEN_WITH => (FIELD_LEFT, 322, FIELD_WIDTH - 144, 36),
                OPEN_DEFAULT => (FIELD_RIGHT - 128, 322, 128, 36),
                _ if label == "ui.save_to_apply_your_changes" => (112, FOOTER_TOP + 13, 600, 37),
                _ if label == "ui.manage_shortcuts" => (RAIL + 25, 60, FIELD_WIDTH, 41),
                _ if label == "ui.alias" => (FIELD_LEFT, 128, HALF_FIELD, 21),
                _ if label == "ui.name" => (FIELD_RIGHT - HALF_FIELD, 128, HALF_FIELD, 21),
                _ if label == "ui.command_or_path" => (FIELD_LEFT, 212, FIELD_WIDTH, 21),
                _ if label == "ui.input_format" => (FIELD_LEFT, 352, FIELD_WIDTH, 21),
                _ => (x, y, width, height),
            };
            let child = CreateWindowExW(
                0,
                wide(class).as_ptr(),
                ui(label).as_ptr(),
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
                crate::ui::style_combo(child, dpi);
            } else if class == "EDIT" {
                crate::ui::style_edit(child, dpi);
            }
            child
        };
        let close_caption = control("BUTTON", "ui.close", WS_TABSTOP, 0, 0, 0, 0, CAPTION_CLOSE);
        crate::ui::style_caption_close(close_caption);
        let title = control("STATIC", "ui.manage_shortcuts", 0, 280, 12, 300, 25, 0);
        control("BUTTON", "ui.add", WS_TABSTOP, 12, 12, 116, 30, NEW_MENU);
        control(
            "BUTTON", "ui.tools", WS_TABSTOP, 138, 12, 124, 30, TOOLS_MENU,
        );
        SetWindowTextW(title, ui("ui.edit_shortcut").as_ptr());
        SendMessageW(title, WM_SETFONT, heading as WPARAM, 0);
        SetPropW(
            hwnd,
            wide(TABLE_ROW_HEIGHT_PROP).as_ptr(),
            scaled(ROW_HEIGHT, dpi) as HANDLE,
        );
        let list = control(
            "SysListView32",
            "",
            WS_CLIPSIBLINGS
                | WS_TABSTOP
                | LVS_REPORT
                | LVS_SINGLESEL
                | LVS_SHOWSELALWAYS
                | LVS_NOSORTHEADER
                | LVS_OWNERDRAWFIXED,
            12,
            52,
            250,
            304,
            LIST,
        );
        if list.is_null() {
            DestroyWindow(hwnd);
            for font in [font, heading, caption] {
                DeleteObject(font);
            }
            UnregisterClassW(class_name.as_ptr(), instance);
            return Err("ui.could_not_create_the_shortcut_manager_window".into());
        }
        table::configure(list, dpi);
        control("STATIC", "ui.alias", 0, 280, 52, 210, 20, 0);
        control("STATIC", "ui.name", 0, 512, 52, 232, 20, 0);
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
        control("STATIC", "ui.command_or_path", 0, 280, 118, 464, 20, 0);
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
        control(
            "BUTTON",
            "ui.browse_command_target",
            WS_TABSTOP,
            0,
            0,
            0,
            0,
            CHOOSE_TARGET,
        );
        let ask_parameter = control(
            "BUTTON",
            "ui.ask_for_input_on_launch",
            BS_AUTOCHECKBOX as u32 | WS_TABSTOP,
            0,
            0,
            0,
            0,
            ASK_PARAMETER,
        );
        crate::ui::switch(ask_parameter, dpi, None);
        control("STATIC", "ui.input_format", 0, 0, 0, 0, 0, 0);
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
        for (_, label) in PARAMETER_CHOICES {
            SendMessageW(
                kind,
                CB_ADDSTRING,
                0,
                wide(&crate::localization::tr(label)).as_ptr() as LPARAM,
            );
        }
        control("STATIC", "ui.open_with", 0, 0, 0, 0, 0, OPEN_LABEL);
        control("BUTTON", "ui.choose_app", WS_TABSTOP, 0, 0, 0, 0, OPEN_WITH);
        control(
            "BUTTON",
            "ui.system_default",
            WS_TABSTOP,
            0,
            0,
            0,
            0,
            OPEN_DEFAULT,
        );
        control("STATIC", "ui.note_text", 0, 0, 0, 0, 0, NOTE_LABEL);
        control(
            "STATIC",
            "ui.select_new_to_create_your_first_shortcut",
            0,
            0,
            0,
            0,
            0,
            EMPTY_HELP,
        );
        let status = control(
            "STATIC",
            "ui.save_to_apply_your_changes",
            crate::ui::STATIC_CENTER_IMAGE,
            0,
            0,
            0,
            0,
            0,
        );
        SetWindowTextW(status, wide("").as_ptr());

        let delete = control("BUTTON", "ui.delete", WS_TABSTOP, 92, 374, 72, 30, DELETE);
        control("BUTTON", "ui.close", WS_TABSTOP, 0, 0, 0, 0, CLOSE);
        control("BUTTON", "ui.try_opening_2", WS_TABSTOP, 0, 0, 0, 0, TEST);
        let save = control(
            "BUTTON",
            "ui.save",
            WS_TABSTOP | BS_DEFPUSHBUTTON as u32,
            560,
            374,
            88,
            30,
            SAVE,
        );
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

fn paint_manager(hwnd: HWND) {
    unsafe {
        let mut ps: PAINTSTRUCT = zeroed();
        let dc = BeginPaint(hwnd, &mut ps);
        let mut bounds: RECT = zeroed();
        GetClientRect(hwnd, &mut bounds);
        let (dpi, font, caption) = EDITOR.with(|c| {
            let s = c.borrow();
            let s = s.as_ref().unwrap();
            (s.dpi, s.font, s.caption)
        });
        let rect = |x, y, w, h| RECT {
            left: scaled(x, dpi),
            top: scaled(y, dpi),
            right: scaled(x + w, dpi),
            bottom: scaled(y + h, dpi),
        };
        crate::ui::fill(dc, &bounds, theme::surface());
        crate::ui::fill(dc, &rect(0, 0, WIDTH, CAPTION), theme::chrome());
        crate::ui::fill(
            dc,
            &rect(0, CAPTION, RAIL, FOOTER_TOP - CAPTION),
            theme::chrome(),
        );
        crate::ui::fill(
            dc,
            &rect(0, FOOTER_TOP, WIDTH, HEIGHT - FOOTER_TOP),
            theme::chrome(),
        );
        let mut list_card = rect(16, TABLE_TOP, RAIL - 32, 0);
        list_card.bottom = scaled(TABLE_TOP + 1, dpi) + list_height(dpi) + scaled(2, dpi);
        crate::ui::rounded(
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
            crate::ui::rounded(
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
        }
        for line in [
            rect(0, CAPTION, WIDTH, 1),
            rect(RAIL, CAPTION, 1, FOOTER_TOP - CAPTION),
            rect(0, FOOTER_TOP - 1, WIDTH, 1),
        ] {
            crate::ui::fill(dc, &line, theme::divider());
        }
        crate::ui::panel_caption(dc, dpi, caption, "ui.altrunnext_manage_shortcuts_2");
        SetBkMode(dc, TRANSPARENT as i32);
        SetTextColor(dc, theme::text());
        SelectObject(dc, font);
        let mut library_title = rect(17, 62, 120, 32);
        DrawTextW(
            dc,
            ui("ui.shortcuts").as_ptr(),
            -1,
            &mut library_title,
            DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX,
        );
        EndPaint(hwnd, &ps);
    }
}

pub(super) fn sync_kind_layout() {
    let layout = EDITOR.with(|c| {
        let mut state = c.borrow_mut();
        let s = state.as_mut().unwrap();
        let entry = s.draft.get(s.selected);
        let kind = entry.map(|e| e.kind);
        let parameter_supported = entry.is_some_and(supports_parameter);
        let parameter_visible =
            parameter_supported && entry.is_some_and(|e| e.param_type.needs_input(&e.command));
        let document =
            entry.is_some_and(|e| matches!(e.open_target, Some(OpenTarget::Document(_))));
        if s.laid_out_kind == Some((kind, parameter_supported, parameter_visible, document)) {
            return None;
        }
        s.laid_out_kind = Some((kind, parameter_supported, parameter_visible, document));
        Some((
            unsafe { GetParent(s.list) },
            kind,
            parameter_supported,
            parameter_visible,
            document,
            s.name,
            s.dpi,
        ))
    });
    let Some((hwnd, kind, parameter_supported, parameter_visible, document, name, dpi)) = layout
    else {
        return;
    };
    unsafe {
        for (id, show) in [
            (ALIAS, true),
            (COMMAND, true),
            (CHOOSE_TARGET, true),
            (KIND, parameter_visible),
            (ASK_PARAMETER, parameter_supported),
            (2170, true),
            (2171, true),
            (2172, true),
            (2173, parameter_visible),
            (OPEN_LABEL, document),
            (OPEN_WITH, document),
            (OPEN_DEFAULT, document),
        ] {
            ShowWindow(
                GetDlgItem(hwnd, id as i32),
                if show && kind == Some(library::EntryKind::Command) {
                    SW_SHOW
                } else {
                    SW_HIDE
                },
            );
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
                scaled(154, dpi),
                scaled(x + width, dpi) - scaled(x, dpi),
                scaled(190, dpi) - scaled(154, dpi),
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
