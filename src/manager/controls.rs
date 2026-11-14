// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::*;

pub(super) struct Controls {
    pub(super) list: HWND,
    pub(super) filter: HWND,
    pub(super) filter_hint: HWND,
    pub(super) alias: HWND,
    pub(super) name: HWND,
    pub(super) command: HWND,
    pub(super) kind: HWND,
    pub(super) ask_parameter: HWND,
    pub(super) status: HWND,
    pub(super) save: HWND,
    pub(super) delete: HWND,
}

pub(super) fn create(
    hwnd: HWND,
    instance: HINSTANCE,
    dpi: i32,
    font: HFONT,
    heading: HFONT,
) -> Option<Controls> {
    unsafe {
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
                FILTER => (17, 103, RAIL - 34, 35),
                FILTER_HINT => (
                    FORM_LEFT,
                    FORM_TOP + COMMAND_FORM_HEIGHT + 16,
                    FIELD_WIDTH,
                    34,
                ),
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
                    table::list_height(dpi)
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
        let filter = control(
            "EDIT",
            "",
            WS_TABSTOP | ES_AUTOHSCROLL as u32,
            0,
            0,
            0,
            0,
            FILTER,
        );
        SendMessageW(
            filter,
            EM_SETCUEBANNER,
            1,
            ui("ui.filter_launch_items").as_ptr() as LPARAM,
        );
        let filter_hint = control(
            "STATIC",
            "ui.filtered_launch_item",
            0,
            0,
            0,
            0,
            0,
            FILTER_HINT,
        );
        ShowWindow(filter_hint, SW_HIDE);
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
            return None;
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
        crate::ui::switch(ask_parameter, dpi);
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
        Some(Controls {
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
        })
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
