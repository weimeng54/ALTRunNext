// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::*;

pub(crate) fn run() -> Result<(), String> {
    crate::localization::refresh_packs();
    crate::localization::set_active("system");
    let args: Vec<_> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--ui-scale") {
        let dpi = match args.get(i + 1).map(String::as_str) {
            Some("100") => 96,
            Some("125") => 120,
            Some("150") => 144,
            Some("200") => 192,
            _ => return Err("--ui-scale accepts 100, 125, 150 or 200".into()),
        };
        crate::ui::geometry::set_test_dpi(dpi);
    }
    let data_path = if let Some(i) = args.iter().position(|a| a == "--data") {
        std::path::absolute(args.get(i + 1).ok_or("--data needs a path")?)
            .map_err(|e| e.to_string())?
    } else {
        std::env::current_exe()
            .map_err(|e| e.to_string())?
            .with_file_name("shortcuts.tsv")
    };
    let intake = args
        .iter()
        .position(|a| a == "--add")
        .map(|i| {
            args[i + 1..]
                .iter()
                .map(std::path::absolute)
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()
        .map_err(|error| error.to_string())?
        .unwrap_or_default();
    let Some(instance_lock) = crate::instance::acquire(&data_path, &intake)? else {
        return Ok(());
    };
    altrun_probe::backup::recover(&data_path)?;
    let (settings_file, settings_text) =
        altrun_probe::state_store::TextFile::open(&data_path.with_extension("settings.ini"))?;
    drop(settings_file);
    let preferences = altrun_probe::preferences::Preferences::parse(&settings_text)?;
    crate::localization::set_active(&preferences.get("Lang"));
    let entries = if args.iter().any(|a| a == "--data") || data_path.exists() {
        altrun_probe::library::Snapshot::open(&data_path)?.entries
    } else {
        let mut entries = parse_entries(include_str!("../../assets/shortcuts.tsv"))?;
        for entry in &mut entries {
            entry.name = crate::localization::tr(&entry.name);
        }
        altrun_probe::library::Snapshot::new(data_path.clone(), entries.clone())?.save(&entries)?;
        entries
    };
    let (base_directory, base_is_working_directory) = working_directory(&preferences, &data_path);
    let allow_dynamic_target = preferences.flag("AllowDynamicTarget");
    let (state_file, state_text) =
        altrun_probe::state_store::TextFile::open(&data_path.with_extension("state.tsv"))?;
    let usage = altrun_probe::usage::Usage::parse(&state_text)?;
    let index = super::search::index_with_usage(&entries, &usage);
    let (settings_sender, settings_updates) = std::sync::mpsc::channel();
    let (skin_sender, skin_results) = std::sync::mpsc::channel();
    let (sound_sender, sound_tasks) = std::sync::mpsc::sync_channel::<PopupSound>(1);
    std::thread::spawn(move || {
        use windows_sys::Win32::Media::Audio::*;
        while let Ok(mut sound) = sound_tasks.recv() {
            while let Ok(newer) = sound_tasks.try_recv() {
                sound = newer;
            }
            let source = wide(&sound.path.to_string_lossy());
            unsafe {
                if PlaySoundW(
                    source.as_ptr(),
                    null_mut(),
                    SND_FILENAME | SND_NODEFAULT | SND_ASYNC,
                ) == 0
                    && sound.use_bundled_fallback
                {
                    // The asynchronous WinMM player needs the buffer to outlive playback.
                    let wav = include_bytes!("../../assets/Popup.wav");
                    PlaySoundW(
                        wav.as_ptr().cast(),
                        null_mut(),
                        SND_MEMORY | SND_NODEFAULT | SND_ASYNC,
                    );
                }
            }
        }
    });
    theme::install(&preferences);
    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let instance = GetModuleHandleW(std::ptr::null());
        let class = wide("ALTRun.Native.PerformanceProbe");
        let background = CreateSolidBrush(theme::surface());
        let selected_brush = CreateSolidBrush(theme::selection());
        let paper = CreateSolidBrush(theme::paper());
        let input_paper = CreateSolidBrush(theme::keyword());
        let alias_font = make_font(13, 600, "Microsoft YaHei UI", 96);
        let accent = CreateSolidBrush(theme::accent());
        let border = CreatePen(PS_SOLID, 1, theme::border());
        let mut wc: WNDCLASSW = zeroed();
        wc.lpfnWndProc = Some(window_proc);
        wc.hInstance = instance;
        wc.lpszClassName = class.as_ptr();
        wc.hbrBackground = null_mut();
        wc.hCursor = LoadCursorW(null_mut(), IDC_ARROW);
        wc.hIcon = crate::ui::app_icon(GetSystemMetrics(SM_CXICON));
        if RegisterClassW(&wc) == 0 {
            return Err("Cannot register native window".into());
        }
        let hwnd = CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
            class.as_ptr(),
            wide("ALTRunNext").as_ptr(),
            WS_POPUP | WS_BORDER | WS_CLIPCHILDREN,
            400,
            220,
            420,
            280,
            null_mut(),
            null_mut(),
            instance,
            std::ptr::null(),
        );
        if hwnd.is_null() {
            return Err("Cannot create native window".into());
        }
        crate::instance::publish(hwnd, instance_lock.identity);
        DragAcceptFiles(hwnd, 1);
        let dpi = crate::ui::geometry::window_dpi(hwnd);
        let rounded = 2_i32;
        DwmSetWindowAttribute(hwnd, 33, &rounded as *const _ as *const _, 4);
        let no_transitions = 1i32;
        DwmSetWindowAttribute(hwnd, 3, &no_transitions as *const _ as _, 4);
        DwmSetWindowAttribute(hwnd, 34, &theme::border() as *const _ as *const _, 4);
        let font = CreateFontW(
            -scaled(13, dpi),
            0,
            0,
            0,
            400,
            0,
            0,
            0,
            DEFAULT_CHARSET as u32,
            OUT_DEFAULT_PRECIS as u32,
            CLIP_DEFAULT_PRECIS as u32,
            CLEARTYPE_QUALITY as u32,
            0,
            wide("Microsoft YaHei UI").as_ptr(),
        );
        let title_font = make_font(13, 600, "Microsoft YaHei UI", dpi);
        let input_font = make_font(16, 400, "Microsoft YaHei UI", dpi);
        let command_font = make_font(12, 400, "Consolas", dpi);
        let control = |class_name: &str, style: u32, x, y, width, height, id: usize| {
            CreateWindowExW(
                0,
                wide(class_name).as_ptr(),
                wide("").as_ptr(),
                WS_CHILD | WS_VISIBLE | style,
                scaled(x, dpi),
                scaled(y, dpi),
                scaled(width, dpi),
                scaled(height, dpi),
                hwnd,
                id as HMENU,
                instance,
                std::ptr::null(),
            )
        };
        let title = control("STATIC", STATIC_OWNER_DRAW, 12, 6, 396, 20, TITLE);
        let edit = control("EDIT", ES_AUTOHSCROLL as u32, 18, 35, 384, 22, ID_EDIT);
        let list_style = LBS_NOTIFY as u32
            | LBS_OWNERDRAWFIXED as u32
            | LBS_NOINTEGRALHEIGHT as u32
            | WS_CLIPSIBLINGS
            | WS_VSCROLL;
        let create_list = |style| {
            control(
                "LISTBOX",
                list_style | style,
                8,
                theme::LIST_Y,
                404,
                120,
                ID_LIST,
            )
        };
        let mut list = create_list(LBS_NODATA as u32);
        let result_names = super::result_names::ResultNames::register(list);
        if result_names.is_none() {
            if !list.is_null() {
                DestroyWindow(list);
            }
            list = create_list(LBS_HASSTRINGS as u32);
        }
        let command = control(
            "EDIT",
            ES_AUTOHSCROLL as u32 | ES_READONLY as u32,
            12,
            178,
            396,
            16,
            1003,
        );
        let inline_error = control(
            "EDIT",
            (ES_MULTILINE | ES_READONLY) as u32,
            12,
            178,
            396,
            16,
            1005,
        );
        ShowWindow(inline_error, SW_HIDE);
        let error_font = make_font(12, 400, "Microsoft YaHei UI", dpi);
        SendMessageW(inline_error, WM_SETFONT, error_font as WPARAM, 0);
        let buttons = [
            control("BUTTON", BS_OWNERDRAW as u32, 0, 0, 22, 22, ID_MANAGE),
            control("BUTTON", BS_OWNERDRAW as u32, 0, 0, 22, 22, ID_SETTINGS),
            control("BUTTON", BS_OWNERDRAW as u32, 0, 0, 22, 22, ID_HIDE),
        ];
        for (button, text) in buttons.iter().zip(["≡", "⚙", "×"]) {
            SetWindowTextW(*button, wide(text).as_ptr());
            SendMessageW(*button, WM_SETFONT, font as usize, 0);
        }
        let empty = control("STATIC", 1, 12, theme::LIST_Y + 2, 396, 20, 1004);
        SetWindowTextW(empty, ui("ui.no_matches_try_another_shortcut").as_ptr());
        if [title, edit, list, command, inline_error, empty]
            .iter()
            .any(|h| h.is_null())
        {
            return Err("Cannot create native controls".into());
        }
        for child in [title, edit, list, command, empty] {
            SendMessageW(child, WM_SETFONT, font as WPARAM, 0);
        }
        SendMessageW(title, WM_SETFONT, title_font as WPARAM, 0);
        SendMessageW(edit, WM_SETFONT, input_font as WPARAM, 0);
        SendMessageW(
            edit,
            EM_SETCUEBANNER,
            0,
            ui("ui.type_a_shortcut").as_ptr() as LPARAM,
        );
        SendMessageW(command, WM_SETFONT, command_font as WPARAM, 0);
        SendMessageW(list, LB_SETITEMHEIGHT, 0, scaled(theme::ROW, dpi) as LPARAM);
        let (launch_sender, launch_results) = std::sync::mpsc::channel();
        let (library_sender, library_updates) = altrun_probe::library_updates::channel();
        let owner = hwnd as usize;
        let usage_writer =
            altrun_probe::state_store::Writer::start_usage(state_file, usage.clone(), move || {
                PostMessageW(owner as HWND, STATE_ERROR, 0, 0);
            });
        LAUNCH_SENDER.with(|s| *s.borrow_mut() = Some(launch_sender));
        let old_edit = SetWindowLongPtrW(edit, GWLP_WNDPROC, edit_proc as *const () as isize);
        let old_list = SetWindowLongPtrW(list, GWLP_WNDPROC, list_proc as *const () as isize);
        STATE.with(|c| {
            *c.borrow_mut() = Some(State {
                sound_sender,
                input_paper,
                alias_font,
                font_colours: [theme::text(); 3],
                buttons,
                skin: None,
                skin_request: None,
                skin_sender,
                skin_results,
                preferences,
                usage,
                usage_writer,
                settings_sender,
                settings_updates,
                hotkeys: Vec::new(),
                pending_settings: None,
                generation: 0,
                current_query: String::new(),
                default_list_ready: false,
                last_key: 0,
                context: altrun_probe::launch::LaunchContext {
                    base_directory,
                    base_is_working_directory,
                    allow_dynamic_target,
                    ..Default::default()
                },
                successful: None,
                history_position: usize::MAX,
                entries,
                index,
                hits: Vec::new(),
                list_dirty: true,
                virtual_results: result_names.is_some(),
                layout: None,
                selected: 0,
                hwnd,
                edit,
                list,
                title,
                command,
                inline_error,
                error_font,
                font,
                title_font,
                input_font,
                command_font,
                paper,
                accent,
                border,
                empty,
                enabled: !args.iter().any(|a| a == "--safe-mode")
                    && (args.iter().any(|a| a == "--enable-launch")
                        || (!cfg!(debug_assertions)
                            && !args.iter().any(|a| a == "--allow-safe-notepad"))),
                parameter_entry: None,
                saved_query: String::new(),
                error: false,
                launching: None,
                launch_results,
                background,
                selected_brush,
                old_edit,
                old_list,
                dpi,
                allow_launch: args.iter().any(|a| a == "--allow-safe-notepad"),
                composing: false,
                moving: false,
                data_path,
                library_sender,
                library_updates,
            })
        });
        crate::ui::style_vertical_scrollbar(list, ID_SCROLLBAR, 0);
        refresh();
        let prefs = STATE.with(|c| c.borrow().as_ref().unwrap().preferences.clone());
        let mut startup_warnings = Vec::new();
        let mut failed_primary = None;
        match register_bindings(
            hwnd,
            &prefs,
            &[],
            BindingUpdate::Startup(&mut startup_warnings, &mut failed_primary),
        ) {
            Ok(keys) => STATE.with(|c| c.borrow_mut().as_mut().unwrap().hotkeys = keys),
            Err(error) => startup_warnings.push(error),
        }
        apply_appearance();
        if !intake.is_empty() {
            crate::manager::enqueue(intake, None);
            open_manager_window(false);
        }
        if args
            .iter()
            .any(|a| matches!(a.as_str(), "--clean" | "Clean"))
        {
            PostMessageW(hwnd, WM_COMMAND, ID_CLEAN, 0);
        }
        if args.iter().any(|a| a == "--show")
            || !startup_warnings.is_empty()
            || failed_primary.is_some()
            || STATE.with(|c| {
                c.borrow()
                    .as_ref()
                    .unwrap()
                    .preferences
                    .flag("ShowMeWhenStart")
            })
        {
            SendMessageW(hwnd, SHOW, 0, 0);
        }
        if let Some(binding) = failed_primary
            && crate::ui::dialogs::warning::show(hwnd, &binding, prefs.flag("ShowTrayIcon"))
        {
            open_settings_page(Some(crate::settings::HOTKEYS_PAGE));
        }
        if !startup_warnings.is_empty() && IsWindowVisible(hwnd) != 0 {
            show_inline_error(&startup_warnings.join("\n"));
        }
        let accelerator = ACCEL {
            fVirt: FVIRTKEY | FALT,
            key: b'S' as u16,
            cmd: ID_MANAGE as u16,
        };
        let table = CreateAcceleratorTableW(&accelerator, 1);
        let mut message: MSG = zeroed();
        loop {
            let result = GetMessageW(&mut message, null_mut(), 0, 0);
            if result <= 0 {
                break;
            }
            if TranslateAcceleratorW(hwnd, table, &message) != 0 {
                continue;
            }
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
        if !table.is_null() {
            DestroyAcceleratorTable(table);
        }
        STATE.with(|c| {
            if let Some(s) = c.borrow_mut().take() {
                for font in [
                    s.font,
                    s.title_font,
                    s.input_font,
                    s.command_font,
                    s.error_font,
                    s.alias_font,
                ] {
                    DeleteObject(font);
                }
                DeleteObject(s.paper);
                DeleteObject(s.input_paper);
                DeleteObject(s.accent);
                DeleteObject(s.border);
                DeleteObject(s.selected_brush);
                DeleteObject(s.background);
            }
        });
        UnregisterClassW(class.as_ptr(), instance);
    }
    Ok(())
}
