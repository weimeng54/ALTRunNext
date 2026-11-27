// Copyright (c) 2026 GPT-6
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::Snapshot;
use altrun_probe::preferences::Preferences;
use std::sync::Mutex;
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    UI::{
        Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
        WindowsAndMessaging::*,
    },
};

const REFRESH: u32 = WM_APP + 38;
pub(crate) const CHANGED: u32 = WM_APP + 39;

struct Window {
    handle: usize,
    palette: Option<Snapshot>,
    queued: bool,
}

struct Subscribers {
    language_published: bool,
    mode: Option<String>,
    menu_font: Option<String>,
    windows: Vec<Window>,
}

static APPLIED: Mutex<Subscribers> = Mutex::new(Subscribers {
    language_published: false,
    mode: None,
    menu_font: None,
    windows: Vec::new(),
});

pub(crate) fn publish(prefs: &Preferences) {
    let mode = prefs.get("Theme");
    let mut applied = APPLIED.lock().unwrap();
    applied.menu_font = Some(prefs.get("MenuFont"));
    if applied.mode.as_ref() == Some(&mode) {
        return;
    }
    applied.mode = Some(mode);
    queue_refresh(&mut applied);
}

pub(crate) fn notify_language() {
    let mut applied = APPLIED.lock().unwrap();
    applied.language_published = true;
    for window in &applied.windows {
        unsafe {
            PostMessageW(window.handle as HWND, crate::ui::language::CHANGED, 0, 0);
        }
    }
}

fn queue_refresh(applied: &mut Subscribers) {
    for window in &mut applied.windows {
        if !window.queued {
            window.queued = unsafe { PostMessageW(window.handle as HWND, REFRESH, 0, 0) != 0 };
        }
    }
}

pub(crate) fn applied_mode() -> Option<String> {
    APPLIED.lock().unwrap().mode.clone()
}

pub(crate) fn menu_font() -> String {
    APPLIED
        .lock()
        .unwrap()
        .menu_font
        .clone()
        .unwrap_or_else(|| altrun_probe::preferences::DEFAULT_MENU_FONT.into())
}

fn install_mode(mode: Option<String>) {
    let mut prefs = Preferences::default();
    if let Some(mode) = mode {
        let _ = prefs.set("Theme", &mode);
    }
    super::install_chrome(&prefs);
}

pub(crate) fn watch(hwnd: HWND) {
    if unsafe { SetWindowSubclass(hwnd, Some(procedure), 1, 0) } == 0 {
        return;
    }
    let before = Snapshot::capture();
    let mode = {
        let mut applied = APPLIED.lock().unwrap();
        if applied.language_published {
            crate::localization::follow_active();
            unsafe {
                PostMessageW(hwnd, crate::ui::language::CHANGED, 0, 0);
            }
        }
        applied.windows.push(Window {
            handle: hwnd as usize,
            palette: None,
            queued: false,
        });
        applied.mode.clone()
    };
    install_mode(mode);
    let palette = Snapshot::capture();
    {
        let mut applied = APPLIED.lock().unwrap();
        let window = applied
            .windows
            .iter_mut()
            .find(|w| w.handle == hwnd as usize)
            .unwrap();
        if before == palette {
            window.palette = Some(palette);
        } else if !window.queued {
            // A commit raced window creation after its caller took the initial palette.
            window.queued = unsafe { PostMessageW(hwnd, REFRESH, 0, 0) != 0 };
        }
    }
    super::frame(hwnd);
}

unsafe extern "system" fn notify_control(hwnd: HWND, _: LPARAM) -> i32 {
    unsafe { SendMessageW(hwnd, CHANGED, 0, 0) };
    1
}

fn refresh(hwnd: HWND) {
    let mode = {
        let mut applied = APPLIED.lock().unwrap();
        let Some(window) = applied
            .windows
            .iter_mut()
            .find(|w| w.handle == hwnd as usize)
        else {
            return;
        };
        window.queued = false;
        applied.mode.clone()
    };
    install_mode(mode);
    let current = Snapshot::capture();
    let previous = {
        let mut applied = APPLIED.lock().unwrap();
        let window = applied
            .windows
            .iter_mut()
            .find(|w| w.handle == hwnd as usize)
            .unwrap();
        window.palette.replace(current)
    };
    unsafe {
        if previous == Some(current) {
            return;
        }
        if previous.is_some() {
            EndMenu();
        }
        super::frame(hwnd);
        SendMessageW(hwnd, CHANGED, 0, 0);
        EnumChildWindows(hwnd, Some(notify_control), 0);
        RedrawWindow(
            hwnd,
            std::ptr::null(),
            std::ptr::null_mut(),
            RDW_INVALIDATE | RDW_FRAME | RDW_ALLCHILDREN,
        );
    }
}

unsafe extern "system" fn procedure(
    hwnd: HWND,
    msg: u32,
    w: WPARAM,
    l: LPARAM,
    id: usize,
    _: usize,
) -> LRESULT {
    unsafe {
        match msg {
            WM_NCDESTROY => {
                APPLIED
                    .lock()
                    .unwrap()
                    .windows
                    .retain(|window| window.handle != hwnd as usize);
                RemoveWindowSubclass(hwnd, Some(procedure), id);
            }
            WM_THEMECHANGED | WM_SETTINGCHANGE | WM_SYSCOLORCHANGE => {
                queue_refresh(&mut APPLIED.lock().unwrap());
            }
            crate::ui::language::CHANGED => {
                return crate::ui::language::refresh(hwnd, || DefSubclassProc(hwnd, msg, w, l));
            }
            REFRESH => {
                refresh(hwnd);
                return 0;
            }
            _ => {}
        }
        DefSubclassProc(hwnd, msg, w, l)
    }
}
