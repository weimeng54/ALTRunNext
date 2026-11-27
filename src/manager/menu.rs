// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::*;

pub(super) fn popup(hwnd: HWND, group: usize, position: Option<LPARAM>) {
    unsafe {
        let dpi = EDITOR.with(|c| c.borrow().as_ref().unwrap().dpi);
        let Some(mut style) = menu_style::PopupStyle::new(dpi) else {
            return;
        };
        let menu = style.root();
        let (row_count, selected_row, sort_mode, list) = EDITOR.with(|c| {
            let s = c.borrow();
            let s = s.as_ref().unwrap();
            (
                s.view.len(),
                s.view.iter().position(|&i| i == s.selected),
                s.sort,
                s.list,
            )
        });
        let anchor;
        if let Some(p) = position {
            if row_count == 0 {
                return;
            }
            let point;
            if p == -1 {
                let rect = report::row_bounds(list, selected_row.unwrap_or_default());
                let mut row_point = POINT {
                    x: rect.left.max(0) + scaled(12, dpi),
                    y: rect.bottom,
                };
                ClientToScreen(list, &mut row_point);
                point = row_point;
            } else {
                point = POINT {
                    x: (p as u16) as i16 as i32,
                    y: ((p >> 16) as u16) as i16 as i32,
                };
                let mut local = point;
                ScreenToClient(list, &mut local);
                let Some(index) = report::row_at(list, local) else {
                    return;
                };
                if !select_view_row(index) {
                    return;
                }
                let already_selected =
                    SendMessageW(list, LVM_GETITEMSTATE, index, LVIS_SELECTED as LPARAM) != 0;
                report::select(list, index);
                if already_selected {
                    sync_fields();
                }
            }
            anchor = menu_style::Anchor::Pointer(point);
            let selected = EDITOR.with(|c| c.borrow().as_ref().unwrap().selected);
            style.add(
                menu,
                MOVE_UP,
                "ui.move_selection_up",
                false,
                sort_mode != SORT_ORIGINAL || selected == 0,
            );
            style.add(
                menu,
                MOVE_DOWN,
                "ui.move_selection_down",
                false,
                sort_mode != SORT_ORIGINAL || selected + 1 >= row_count,
            );
        } else {
            let mut rect: RECT = zeroed();
            GetWindowRect(GetDlgItem(hwnd, group as i32), &mut rect);
            anchor = menu_style::Anchor::Below(rect);
            if group == NEW_MENU {
                for (id, label) in [
                    (ADD, "ui.add_manually"),
                    (ADD_FILE, "ui.add_from_file"),
                    (ADD_FOLDER, "ui.add_from_folder"),
                ] {
                    style.add(menu, id, label, false, false);
                }
            } else {
                style.add(
                    menu,
                    VALIDATE,
                    "ui.check_missing_paths",
                    false,
                    row_count == 0,
                );
                style.separator(menu);
                let Some(sort) = style.submenu("ui.sort") else {
                    return;
                };
                for (id, label) in [
                    (SORT_ORIGINAL, "ui.original_order"),
                    (SORT_ALIAS, "ui.by_alias"),
                    (SORT_NAME, "ui.by_name"),
                ] {
                    style.add(sort, id, label, sort_mode == id, row_count == 0);
                }
                style.separator(menu);
                style.add(menu, EXPORT, "ui.export_shortcuts", false, false);
                style.add(menu, RESTORE, "ui.import_shortcuts", false, false);
                style.add(
                    menu,
                    IMPORT_BOOKMARKS,
                    "ui.browser_bookmarks_import",
                    false,
                    false,
                );
            }
        }
        let command = style.track(hwnd, anchor);
        if command != 0 {
            SendMessageW(hwnd, WM_COMMAND, command as WPARAM, 0);
        }
    }
}
