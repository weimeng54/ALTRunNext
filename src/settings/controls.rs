// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::*;

pub(super) struct Controls {
    pub(super) tabs: HWND,
    pub(super) title: HWND,
    pub(super) status_label: HWND,
    pub(super) font: HFONT,
    pub(super) heading_font: HFONT,
    pub(super) about_font: HFONT,
    pub(super) section_font: HFONT,
    pub(super) small_font: HFONT,
    pub(super) caption_font: HFONT,
    pub(super) filename_font: HFONT,
    pub(super) hotkey_font: HFONT,
    pub(super) sections: Vec<(usize, HWND)>,
    pub(super) extra_controls: Vec<(usize, HWND)>,
    pub(super) appearance_children: Vec<HWND>,
    pub(super) preview_controls: [HWND; 3],
    pub(super) fields: Vec<Field>,
}

pub(super) fn create(hwnd: HWND, instance: HINSTANCE, dpi: i32) -> Controls {
    unsafe {
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
                | (LBS_OWNERDRAWFIXED | LBS_HASSTRINGS | LBS_NOTIFY | LBS_NOINTEGRALHEIGHT) as u32,
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
            let button_width = if matches!(localization::current().locale(), "en" | "zh-Hans") {
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
        let fields = create_fields(&control, dpi, hotkey_font, filename_font);
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
        Controls {
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
        }
    }
}

fn create_fields(
    control: &impl Fn(&str, &str, u32, i32, i32, i32, i32, usize) -> HWND,
    dpi: i32,
    hotkey_font: HFONT,
    filename_font: HFONT,
) -> Vec<Field> {
    unsafe {
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
                            SendMessageW(handle, CB_ADDSTRING, 0, ui(title).as_ptr() as isize);
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
        fields
    }
}
