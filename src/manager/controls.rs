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
        let children = crate::ui::ChildControls {
            parent: hwnd,
            instance,
            font,
        };
        let menu_width = |key| (crate::ui::caption_width(hwnd, font, dpi, key) + 40).clamp(78, 135);
        let tools_width = menu_width("ui.tools");
        let add_width = menu_width("ui.add");
        let tools_x = RAIL - 16 - tools_width;
        let control = |class: &str, label: &str, style: u32, (x, y, width, height), id: usize| {
            let style = if class == "COMBOBOX" {
                style | (CBS_OWNERDRAWFIXED | CBS_HASSTRINGS) as u32
            } else {
                style
            };
            let mut rect = crate::ui::geometry::BoxRect::new(x, y, width, height).pixels(dpi);
            if id == LIST {
                rect.bottom = rect.top + table::list_height(dpi);
            }
            let child = children.create(class, label, style, rect, id);
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
        let close_caption = control(
            "BUTTON",
            "ui.close",
            WS_TABSTOP,
            (WIDTH - 47, 1, 46, CAPTION - 1),
            CAPTION_CLOSE,
        );
        crate::ui::style_caption_close(close_caption);
        let title = control(
            "STATIC",
            "ui.edit_shortcut",
            0,
            (FORM_LEFT, 60, FIELD_WIDTH, 41),
            FORM_HEADING,
        );
        control(
            "BUTTON",
            "ui.add",
            WS_TABSTOP,
            (tools_x - 8 - add_width, 62, add_width, 32),
            NEW_MENU,
        );
        control(
            "BUTTON",
            "ui.tools",
            WS_TABSTOP,
            (tools_x, 62, tools_width, 32),
            TOOLS_MENU,
        );
        SendMessageW(title, WM_SETFONT, heading as WPARAM, 0);
        let filter = control(
            "EDIT",
            "",
            WS_TABSTOP | ES_AUTOHSCROLL as u32,
            (17, 103, RAIL - 34, 35),
            FILTER,
        );
        SendMessageW(
            filter,
            EM_SETCUEBANNER,
            1,
            ui("ui.filter_launch_items").as_ptr() as LPARAM,
        );
        crate::ui::language::cue(filter, "ui.filter_launch_items", 1);
        let filter_hint = control(
            "STATIC",
            "ui.filtered_launch_item",
            0,
            (
                FORM_LEFT,
                FORM_TOP + COMMAND_FORM_HEIGHT + 16,
                FIELD_WIDTH,
                34,
            ),
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
            report::STYLE | LVS_NOSORTHEADER,
            (17, TABLE_TOP + 1, RAIL - 34, 0),
            LIST,
        );
        if list.is_null() {
            return None;
        }
        table::configure(list, dpi);
        control(
            "STATIC",
            "ui.alias",
            0,
            (FIELD_LEFT, 128, HALF_FIELD, 21),
            ALIAS_LABEL,
        );
        control(
            "STATIC",
            "ui.name",
            0,
            (FIELD_RIGHT - HALF_FIELD, 128, HALF_FIELD, 21),
            NAME_LABEL,
        );
        let alias = control(
            "EDIT",
            "",
            WS_TABSTOP | ES_AUTOHSCROLL as u32,
            (FIELD_LEFT, 154, HALF_FIELD, 36),
            ALIAS,
        );
        let name = control(
            "EDIT",
            "",
            WS_TABSTOP | ES_AUTOHSCROLL as u32,
            (FIELD_RIGHT - HALF_FIELD, 154, HALF_FIELD, 36),
            NAME,
        );
        control(
            "STATIC",
            "ui.command_label",
            0,
            (FIELD_LEFT, 212, FIELD_WIDTH, 21),
            COMMAND_LABEL,
        );
        let command = control(
            "EDIT",
            "",
            WS_TABSTOP | ES_AUTOHSCROLL as u32,
            (FIELD_LEFT, 238, FIELD_WIDTH - 116, 36),
            COMMAND,
        );
        control(
            "BUTTON",
            "ui.browse_command_target",
            WS_TABSTOP,
            (FIELD_RIGHT - 106, 238, 106, 36),
            CHOOSE_TARGET,
        );
        let ask_parameter = control(
            "BUTTON",
            "ui.ask_for_input_on_launch",
            BS_AUTOCHECKBOX as u32 | WS_TABSTOP,
            (FIELD_LEFT, 293, FIELD_WIDTH, 40),
            ASK_PARAMETER,
        );
        crate::ui::switch(ask_parameter, dpi);
        let run_as_admin = control(
            "BUTTON",
            "ui.run_as_administrator",
            BS_AUTOCHECKBOX as u32 | WS_TABSTOP,
            (FIELD_LEFT, 343, FIELD_WIDTH, 40),
            RUN_AS_ADMIN,
        );
        crate::ui::switch(run_as_admin, dpi);
        control(
            "STATIC",
            "ui.input_format",
            0,
            (FIELD_LEFT, 352, FIELD_WIDTH, 21),
            FORMAT_LABEL,
        );
        let kind = control(
            "COMBOBOX",
            "",
            WS_TABSTOP | CBS_DROPDOWNLIST as u32 | WS_VSCROLL,
            (FIELD_LEFT, 378, FIELD_WIDTH, 190),
            KIND,
        );
        crate::ui::language::combo(kind, PARAMETER_CHOICES.iter().map(|(_, label)| *label));
        for (_, label) in PARAMETER_CHOICES {
            SendMessageW(
                kind,
                CB_ADDSTRING,
                0,
                wide(&crate::localization::tr(label)).as_ptr() as LPARAM,
            );
        }
        control(
            "STATIC",
            "ui.open_with",
            0,
            (FIELD_LEFT, 296, FIELD_WIDTH, 21),
            OPEN_LABEL,
        );
        control(
            "BUTTON",
            "ui.choose_app",
            WS_TABSTOP,
            (FIELD_LEFT, 322, FIELD_WIDTH - 144, 36),
            OPEN_WITH,
        );
        control(
            "BUTTON",
            "ui.system_default",
            WS_TABSTOP,
            (FIELD_RIGHT - 128, 322, 128, 36),
            OPEN_DEFAULT,
        );
        control(
            "STATIC",
            "ui.note_text",
            0,
            (FIELD_LEFT, 128, FIELD_WIDTH, 21),
            NOTE_LABEL,
        );
        control(
            "STATIC",
            "ui.select_new_to_create_your_first_shortcut",
            0,
            (FIELD_LEFT, 128, FIELD_WIDTH, 40),
            EMPTY_HELP,
        );
        let status = control(
            "STATIC",
            "",
            crate::ui::STATIC_CENTER_IMAGE,
            (112, FOOTER_TOP + 13, 600, 37),
            0,
        );

        let delete = control(
            "BUTTON",
            "ui.delete",
            WS_TABSTOP,
            (17, FOOTER_TOP + 13, 86, 37),
            DELETE,
        );
        control(
            "BUTTON",
            "ui.close",
            WS_TABSTOP,
            (WIDTH - 338, FOOTER_TOP + 13, 65, 37),
            CLOSE,
        );
        control(
            "BUTTON",
            "ui.try_opening",
            WS_TABSTOP,
            (WIDTH - 265, FOOTER_TOP + 13, 110, 37),
            TEST,
        );
        let save = control(
            "BUTTON",
            "ui.save",
            WS_TABSTOP | BS_DEFPUSHBUTTON as u32,
            (WIDTH - 147, FOOTER_TOP + 13, 130, 37),
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
        let admin_top = if parameter_visible || document {
            419
        } else {
            343
        };
        SetWindowPos(
            GetDlgItem(hwnd, RUN_AS_ADMIN as i32),
            null_mut(),
            scaled(FIELD_LEFT, dpi),
            scaled(admin_top, dpi),
            0,
            0,
            SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
        );
        for (id, show) in [
            (ALIAS, true),
            (COMMAND, true),
            (CHOOSE_TARGET, true),
            (KIND, parameter_visible),
            (ASK_PARAMETER, parameter_supported),
            (RUN_AS_ADMIN, true),
            (ALIAS_LABEL, true),
            (NAME_LABEL, true),
            (COMMAND_LABEL, true),
            (FORMAT_LABEL, parameter_visible),
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
