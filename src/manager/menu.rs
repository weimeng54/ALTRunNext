// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::*;

pub(super) fn popup(hwnd: HWND, group: usize, position: Option<LPARAM>) {
    unsafe {
        let menu = CreatePopupMenu();
        let (dpi, font) = EDITOR.with(|c| {
            let state = c.borrow();
            let s = state.as_ref().unwrap();
            (s.dpi, s.font)
        });
        let style = menu_style::PopupStyle::new(dpi, font);
        let append = |target, id, label: &str, checked: bool, disabled: bool| {
            let width = if target != menu || position.is_some() {
                136
            } else {
                183
            };
            style.add(target, id, label, width, checked, disabled);
        };
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
        let mut point = POINT { x: 0, y: 0 };
        if let Some(p) = position {
            if row_count == 0 {
                DestroyMenu(menu);
                return;
            }
            if p == -1 {
                let rect = table::row_bounds(list, selected_row.unwrap_or_default());
                point = POINT {
                    x: rect.left.max(0) + scaled(12, dpi),
                    y: rect.bottom,
                };
                ClientToScreen(list, &mut point);
            } else {
                point = POINT {
                    x: (p as u16) as i16 as i32,
                    y: ((p >> 16) as u16) as i16 as i32,
                };
                let mut local = point;
                ScreenToClient(list, &mut local);
                let Some(index) = table::row_at(list, local) else {
                    DestroyMenu(menu);
                    return;
                };
                if !select_view_row(index) {
                    DestroyMenu(menu);
                    return;
                }
                let already_selected =
                    SendMessageW(list, LVM_GETITEMSTATE, index, LVIS_SELECTED as LPARAM) != 0;
                table::select(list, index);
                if already_selected {
                    sync_fields();
                }
            }
            let selected = EDITOR.with(|c| c.borrow().as_ref().unwrap().selected);
            append(
                menu,
                MOVE_UP,
                "ui.move_selection_up",
                false,
                sort_mode != SORT_ORIGINAL || selected == 0,
            );
            append(
                menu,
                MOVE_DOWN,
                "ui.move_selection_down",
                false,
                sort_mode != SORT_ORIGINAL || selected + 1 >= row_count,
            );
        } else {
            let mut rect: RECT = zeroed();
            GetWindowRect(GetDlgItem(hwnd, group as i32), &mut rect);
            point.x = rect.left;
            point.y = rect.bottom;
            if group == NEW_MENU {
                for (id, label) in [
                    (ADD, "ui.add_manually"),
                    (ADD_FILE, "ui.add_from_file"),
                    (ADD_FOLDER, "ui.add_from_folder"),
                ] {
                    append(menu, id, label, false, false);
                }
            } else {
                append(
                    menu,
                    VALIDATE,
                    "ui.check_missing_paths",
                    false,
                    row_count == 0,
                );
                append(menu, 0, "", false, true);
                let sort = CreatePopupMenu();
                for (id, label) in [
                    (SORT_ORIGINAL, "ui.original_order"),
                    (SORT_ALIAS, "ui.by_alias"),
                    (SORT_NAME, "ui.by_name"),
                ] {
                    append(sort, id, label, sort_mode == id, row_count == 0);
                }
                style.submenu(menu, sort, "ui.sort");
                append(menu, 0, "", false, true);
                append(menu, EXPORT, "ui.export_shortcuts", false, false);
                append(menu, RESTORE, "ui.import_shortcuts", false, false);
                append(
                    menu,
                    IMPORT_BOOKMARKS,
                    "ui.browser_bookmarks_import",
                    false,
                    false,
                );
            }
        }
        style.background(menu);
        let command = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_RIGHTBUTTON,
            point.x,
            point.y,
            0,
            hwnd,
            std::ptr::null(),
        );
        DestroyMenu(menu);
        if command != 0 {
            SendMessageW(hwnd, WM_COMMAND, command as WPARAM, 0);
        }
    }
}
