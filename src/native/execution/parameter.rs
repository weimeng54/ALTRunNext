// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::*;

thread_local! {
    static PENDING_PARAMETER: RefCell<Option<(Entry, String)>> = const { RefCell::new(None) };
    static DEFERRED_INTAKE: Cell<bool> = const { Cell::new(false) };
    static PARAMETER_WINDOW: Cell<Option<HWND>> = const { Cell::new(None) };
}

pub(in crate::native) fn parameter_open() -> bool {
    PARAMETER_WINDOW.get().is_some()
}

pub(in crate::native) fn focus_parameter() {
    if let Some(prompt) = PARAMETER_WINDOW
        .get()
        .filter(|prompt| unsafe { IsWindow(*prompt) != 0 })
    {
        crate::ui::dialogs::input::activate_parameter(prompt);
    }
}

pub(in crate::native) fn defer_intake() -> bool {
    if parameter_open() {
        DEFERRED_INTAKE.set(true);
        true
    } else {
        false
    }
}

pub(super) fn enqueue(hwnd: HWND, entry: Entry, query: String) {
    PARAMETER_WINDOW.set(Some(null_mut()));
    PENDING_PARAMETER.replace(Some((entry, query)));
    if unsafe { PostMessageW(hwnd, PARAMETER_OPEN, 0, 0) } == 0 {
        PARAMETER_WINDOW.set(None);
        PENDING_PARAMETER.take();
    }
}

pub(in crate::native) fn open_parameter(hwnd: HWND) {
    use crate::ui::dialogs::input::{ParameterOptions, ask_parameter};
    let Some((entry, query)) = PENDING_PARAMETER.take() else {
        return;
    };
    let (history, english, delay) = STATE.with(|cell| {
        let state = cell.borrow();
        let s = state.as_ref().unwrap();
        (
            s.usage
                .parameters
                .get(&entry.id)
                .or_else(|| s.usage.parameters.get("*"))
                .cloned()
                .unwrap_or_default(),
            s.preferences.flag("EnglishSearch"),
            s.preferences.number("HideDelay"),
        )
    });
    update_search_ime(false);
    unsafe { KillTimer(hwnd, AUTO_HIDE) };
    let name = entry.name.clone();
    let launched = ask_parameter(
        hwnd,
        &name,
        ParameterOptions {
            english,
            history,
            opened: Some(|prompt| PARAMETER_WINDOW.set(Some(prompt))),
            submit: Some(Box::new(move |prompt, value| {
                start_launch(entry.clone(), query.clone(), Some(value), Some(prompt));
            })),
        },
    )
    .is_some();
    PARAMETER_WINDOW.set(None);
    STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        if let Some(attempt) = state.as_mut().and_then(|s| s.launching.as_mut())
            && attempt.prompt.is_some()
        {
            // Cancellation cannot undo a shell request already in progress.
            // Ignore its UI result and keep recording successful launches.
            attempt.prompt = Some(null_mut());
        }
    });
    if launched {
        unsafe { ShowWindow(hwnd, SW_HIDE) };
    } else {
        update_search_ime(true);
        if delay > 0 {
            unsafe { SetTimer(hwnd, AUTO_HIDE, delay as u32 * 1000, None) };
        }
    }
    unsafe { PostMessageW(hwnd, crate::manager::UPDATED, 0, 0) };
    if DEFERRED_INTAKE.replace(false) {
        open_manager_window(false);
    }
}
