// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::*;

type Submit = Box<dyn Fn(HWND, String)>;

pub(crate) struct Options {
    pub english: bool,
    pub history: Vec<String>,
    pub opened: Option<fn(HWND)>,
    pub submit: Option<Submit>,
}

pub(super) struct Prompt {
    pub options: Options,
    pub original_mode: Option<crate::search_ime::InputMode>,
    pub error: HWND,
    pub pending: Option<String>,
    position: Option<usize>,
}

impl Prompt {
    pub fn new(options: Options) -> Self {
        Self {
            options,
            original_mode: None,
            error: null_mut(),
            pending: None,
            position: None,
        }
    }

    fn history_step(&self, edit: HWND, msg: &MSG) -> Option<isize> {
        if self.options.history.is_empty()
            || self.pending.is_some()
            || !matches!(msg.message, WM_KEYDOWN | WM_SYSKEYDOWN)
            || msg.hwnd != edit
        {
            return None;
        }
        let key = msg.wParam as u16;
        match key {
            VK_UP | VK_DOWN if unsafe { GetKeyState(VK_CONTROL as i32) } < 0 => {
                Some(if key == VK_UP { -1 } else { 1 })
            }
            VK_DOWN if unsafe { GetKeyState(VK_MENU as i32) } < 0 => Some(0),
            _ => None,
        }
    }
}

pub(super) unsafe fn history_key(state: *mut Input, dialog: HWND, edit: HWND, msg: &MSG) -> bool {
    unsafe {
        let Some(step) = (*state)
            .parameter
            .as_ref()
            .and_then(|prompt| prompt.history_step(edit, msg))
        else {
            return false;
        };
        let index = if step == 0 {
            let values = (*state).parameter.as_ref().unwrap().options.history.clone();
            let Some(index) = history_menu(dialog, edit, &values) else {
                return true;
            };
            index
        } else {
            let prompt = (*state).parameter.as_ref().unwrap();
            prompt.position.map_or(0, |position| {
                altrun_probe::advance(position, prompt.options.history.len(), step)
            })
        };
        if !(*state).done {
            let prompt = (*state).parameter.as_mut().unwrap();
            prompt.position = Some(index);
            let value = wide(&prompt.options.history[index]);
            SetWindowTextW(edit, value.as_ptr());
            SendMessageW(edit, EM_SETSEL, 0, -1);
        }
        true
    }
}

fn history_menu(dialog: HWND, edit: HWND, values: &[String]) -> Option<usize> {
    unsafe {
        let menu = CreatePopupMenu();
        if menu.is_null() {
            return None;
        }
        for (index, value) in values.iter().take(1000).enumerate() {
            let label = value
                .chars()
                .take(80)
                .collect::<String>()
                .replace('&', "&&");
            AppendMenuW(menu, MF_STRING, index + 1, wide(&label).as_ptr());
        }
        let mut rect = RECT::default();
        GetWindowRect(edit, &mut rect);
        let selected = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_NONOTIFY,
            rect.left,
            rect.bottom,
            0,
            dialog,
            std::ptr::null(),
        );
        DestroyMenu(menu);
        (selected as usize).checked_sub(1)
    }
}
