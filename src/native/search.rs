// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::*;

fn selected_text() -> (HWND, HWND, Vec<u16>, Vec<u16>) {
    STATE.with(|cell| {
        let s = cell.borrow();
        let s = s.as_ref().unwrap();
        let e = s.hits.get(s.selected).map(|&i| &s.entries[i]);
        (
            s.title,
            s.command,
            wide(e.map(|e| e.name.as_str()).unwrap_or("ALTRunNext")),
            wide(
                &e.map(|e| {
                    format!(
                        "{}  {}",
                        crate::localization::tr("ui.command_label"),
                        e.command
                    )
                })
                .unwrap_or_default(),
            ),
        )
    })
}

pub(super) fn update_selection() {
    let changed = STATE.with(|c| std::mem::take(&mut c.borrow_mut().as_mut().unwrap().error));
    if changed {
        layout_inline_error();
    }
    let (title, command, name, line) = selected_text();
    unsafe {
        SetWindowTextW(title, name.as_ptr());
        SetWindowTextW(command, line.as_ptr());
        InvalidateRect(command, std::ptr::null(), 1);
    }
}

pub(super) fn refresh() {
    if STATE.with(|c| c.borrow().as_ref().unwrap().parameter_entry.is_some()) {
        return;
    }
    let edit = STATE.with(|c| c.borrow().as_ref().unwrap().edit);
    let query = unsafe {
        let mut buffer = vec![0_u16; GetWindowTextLengthW(edit) as usize + 1];
        let len = GetWindowTextW(edit, buffer.as_mut_ptr(), buffer.len() as i32);
        String::from_utf16_lossy(&buffer[..len as usize])
    };
    // Release RefCell before calling native controls, which may synchronously
    // reenter WM_DRAWITEM. Never keep a mutable state borrow across SendMessage.
    STATE.with(|c| {
        let mut s = c.borrow_mut();
        let s = s.as_mut().unwrap();
        s.generation = s.generation.wrapping_add(1);
        s.current_query = query;
        let query = &s.current_query;
        s.default_list_ready = query.is_empty();
        let mut hits = s
            .index
            .query_options(query, s.preferences.flag("MatchAnywhere"));
        if !s.default_list_ready {
            promote_preference(s, &mut hits);
        }
        s.list_dirty |= hits != s.hits;
        s.hits = hits;
        s.selected = 0;
    });
    render_hits();
}

fn promote_preference(s: &State, hits: &mut [usize]) {
    if s.preferences.flag("RememberFavouratMatch")
        && let Some(id) = s.usage.favourites.get(&s.current_query)
        && let Some(pos) = hits.iter().position(|i| &s.entries[*i].id == id)
        && pos != 0
        && s.index.same_match_priority(
            &s.current_query,
            s.preferences.flag("MatchAnywhere"),
            hits[0],
            hits[pos],
        )
    {
        hits[..=pos].rotate_right(1);
    }
}

pub(super) fn move_selection(delta: isize) {
    if STATE.with(|c| c.borrow().as_ref().unwrap().parameter_entry.is_some()) {
        return;
    }
    let (list, selected) = STATE.with(|c| {
        let mut s = c.borrow_mut();
        let s = s.as_mut().unwrap();
        s.selected = advance(s.selected, s.hits.len(), delta);
        (s.list, s.selected)
    });
    unsafe {
        SendMessageW(list, LB_SETCURSEL, selected, 0);
    }
    update_selection();
}

pub(super) fn reset_search() {
    let (edit, list, was_parameter, show_operations, needs_refresh) = STATE.with(|c| {
        let mut state = c.borrow_mut();
        let s = state.as_mut().unwrap();
        let was_parameter = s.parameter_entry.take().is_some();
        s.saved_query.clear();
        s.history_position = usize::MAX;
        s.last_key = 0;
        s.composing = false;
        (
            s.edit,
            s.list,
            was_parameter,
            s.preferences.flag("ShowOperationHint"),
            was_parameter
                || !s.default_list_ready
                || s.selected != 0
                || s.launching.is_some()
                || s.error,
        )
    });
    unsafe {
        if was_parameter {
            EnableWindow(list, 1);
            SendMessageW(
                edit,
                EM_SETCUEBANNER,
                0,
                search_hint(show_operations).as_ptr() as LPARAM,
            );
        }
        if GetWindowTextLengthW(edit) != 0 {
            // The single-line edit sends EN_CHANGE synchronously.
            SetWindowTextW(edit, [0_u16].as_ptr());
        } else if needs_refresh {
            refresh();
        }
    }
    if was_parameter {
        update_search_ime(true);
    }
}

pub(super) fn rebuild_index(s: &mut State) {
    s.default_list_ready = false;
    s.list_dirty = true;
    let mut rows = s.entries.clone();
    for entry in &mut rows {
        entry.frequency = entry.frequency.saturating_add(
            s.usage
                .counts
                .get(&entry.id)
                .copied()
                .unwrap_or(0)
                .min(i64::MAX as u64) as i64,
        );
    }
    s.index = SearchIndex::new(&rows);
}

pub(super) fn choose_number(digit: usize) {
    let chosen = STATE.with(|c| {
        let mut s = c.borrow_mut();
        let s = s.as_mut().unwrap();
        if s.parameter_entry.is_some() {
            return false;
        }
        let index = (digit + 9) % 10;
        if index < s.hits.len() {
            s.selected = index;
            true
        } else {
            false
        }
    });
    if chosen {
        launch_selected();
    }
}

pub(super) fn recent() {
    reset_search();
    STATE.with(|c| {
        let mut s = c.borrow_mut();
        let s = s.as_mut().unwrap();
        s.default_list_ready = false;
        let hits = s
            .usage
            .recent
            .iter()
            .filter_map(|id| {
                s.entries.iter().position(|e| {
                    &e.id == id && e.kind == altrun_probe::library::EntryKind::Command
                })
            })
            .collect();
        s.list_dirty |= hits != s.hits;
        s.hits = hits;
        s.selected = 0;
    });
    render_hits();
}
