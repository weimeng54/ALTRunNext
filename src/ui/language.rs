// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

//! Refresh app-owned text on the thread that owns its HWND.
use super::{ui, wide};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    UI::{
        Controls::EM_SETCUEBANNER,
        Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
        WindowsAndMessaging::*,
    },
};

pub(crate) const CHANGED: u32 = WM_APP + 41;
const REFRESH: u32 = WM_APP + 42;
type Update = Rc<dyn Fn(HWND)>;
thread_local! {
    static BINDINGS: RefCell<HashMap<HWND, Update>> = RefCell::new(HashMap::new());
    static CAPTION_POINTER: Cell<LPARAM> = const { Cell::new(0) };
}

pub(crate) fn publish(lang: &str) {
    crate::localization::set_active(lang);
    crate::theme::notify_language();
}

pub(crate) fn bind(hwnd: HWND, update: impl Fn(HWND) + 'static) {
    register(hwnd, false, update);
}

fn register(hwnd: HWND, caption: bool, update: impl Fn(HWND) + 'static) {
    if unsafe { SetWindowSubclass(hwnd, Some(procedure), 1, usize::from(caption)) } != 0 {
        BINDINGS.with(|items| items.borrow_mut().insert(hwnd, Rc::new(update)));
    }
}

pub(crate) fn caption(hwnd: HWND, key: &str) {
    if crate::localization::is_key(key) {
        let key = key.to_owned();
        dynamic_caption(hwnd, move || crate::localization::tr(&key));
    }
}

pub(crate) fn dynamic_caption(hwnd: HWND, text: impl Fn() -> String + 'static) {
    register(hwnd, true, move |control| {
        let value = wide(&text());
        let old = CAPTION_POINTER.replace(value.as_ptr() as LPARAM);
        unsafe { SetWindowTextW(control, value.as_ptr()) };
        CAPTION_POINTER.set(old);
    });
}

pub(crate) fn cue(hwnd: HWND, key: &'static str, focused: WPARAM) {
    bind(hwnd, move |control| unsafe {
        SendMessageW(
            control,
            EM_SETCUEBANNER,
            focused,
            ui(key).as_ptr() as LPARAM,
        );
    });
}

pub(crate) fn combo(hwnd: HWND, labels: impl Iterator<Item = &'static str>) {
    let labels: Vec<_> = labels.collect();
    bind(hwnd, move |control| unsafe {
        let selection = SendMessageW(control, CB_GETCURSEL, 0, 0);
        let top = SendMessageW(control, CB_GETTOPINDEX, 0, 0);
        SendMessageW(control, CB_RESETCONTENT, 0, 0);
        for key in &labels {
            SendMessageW(control, CB_ADDSTRING, 0, ui(key).as_ptr() as LPARAM);
        }
        SendMessageW(control, CB_SETCURSEL, selection as usize, 0);
        SendMessageW(control, CB_SETTOPINDEX, top as usize, 0);
    });
}

unsafe extern "system" fn procedure(
    hwnd: HWND,
    msg: u32,
    w: WPARAM,
    l: LPARAM,
    id: usize,
    caption: usize,
) -> LRESULT {
    unsafe {
        match msg {
            REFRESH => {
                let update = BINDINGS.with(|items| items.borrow().get(&hwnd).cloned());
                if let Some(update) = update {
                    update(hwnd);
                }
                return 0;
            }
            WM_NCDESTROY | WM_SETTEXT
                if msg == WM_NCDESTROY || (caption != 0 && l != CAPTION_POINTER.get()) =>
            {
                BINDINGS.with(|items| items.borrow_mut().remove(&hwnd));
                RemoveWindowSubclass(hwnd, Some(procedure), id);
            }
            _ => {}
        }
        DefSubclassProc(hwnd, msg, w, l)
    }
}

unsafe extern "system" fn child(hwnd: HWND, _: LPARAM) -> i32 {
    unsafe {
        SendMessageW(hwnd, REFRESH, 0, 0);
    }
    1
}

pub(crate) fn refresh(hwnd: HWND, update_window: impl FnOnce() -> LRESULT) -> LRESULT {
    crate::localization::follow_active();
    unsafe {
        EndMenu();
        // Native STATIC and BUTTON controls may paint directly from WM_SETTEXT.
        // Finish translating the whole window before any of them can draw.
        let visible = IsWindowVisible(hwnd) != 0;
        if visible {
            SendMessageW(hwnd, WM_SETREDRAW, 0, 0);
        }
        SendMessageW(hwnd, REFRESH, 0, 0);
        EnumChildWindows(hwnd, Some(child), 0);
        let result = update_window();
        if visible && IsWindow(hwnd) != 0 {
            SendMessageW(hwnd, WM_SETREDRAW, 1, 0);
            RedrawWindow(
                hwnd,
                std::ptr::null(),
                std::ptr::null_mut(),
                RDW_INVALIDATE | RDW_FRAME | RDW_ALLCHILDREN | RDW_NOERASE | RDW_UPDATENOW,
            );
        }
        result
    }
}
