// Copyright (c) 2026 GPT-6
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::*;
use std::{cell::RefCell, ptr::null};
use windows_sys::Win32::{
    System::Threading::GetCurrentThreadId,
    UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
};

impl PopupStyle {
    pub(crate) fn track(&self, hwnd: HWND, anchor: Anchor) -> usize {
        let gap = scaled(3, self.dpi);
        let (point, exclude, above) = match anchor {
            Anchor::Pointer(point) => (point, None, false),
            Anchor::Below(r) => (
                POINT {
                    x: r.left,
                    y: r.bottom + gap,
                },
                Some(r),
                false,
            ),
            Anchor::Above(r) => (
                POINT {
                    x: r.left,
                    y: r.top - gap,
                },
                Some(r),
                true,
            ),
        };
        unsafe {
            let mut work = MONITORINFO {
                cbSize: size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            if GetMonitorInfoW(MonitorFromPoint(point, MONITOR_DEFAULTTONEAREST), &mut work) != 0 {
                self.max_width
                    .set((work.rcWork.right - work.rcWork.left - scaled(32, self.dpi)).max(1));
            }
            let info = MENUINFO {
                cbSize: size_of::<MENUINFO>() as u32,
                fMask: MIM_BACKGROUND | MIM_APPLYTOSUBMENUS,
                hbrBack: self.background,
                ..Default::default()
            };
            SetMenuInfo(self.root, &info);
            let data = self as *const Self as usize;
            if SetWindowSubclass(hwnd, Some(owner_proc), data, data) == 0 {
                return 0;
            }
            let first = ACTIVE.with(|s| {
                let mut s = s.borrow_mut();
                let first = s.is_empty();
                s.push(TrackingMenus {
                    menus: self.columns.keys().copied().collect(),
                    dpi: self.dpi,
                });
                first
            });
            let hook = if first {
                SetWindowsHookExW(
                    WH_CALLWNDPROCRET,
                    Some(after_message),
                    null_mut(),
                    GetCurrentThreadId(),
                )
            } else {
                null_mut()
            };
            let params = exclude.map(|mut rc| {
                rc.top -= gap;
                rc.bottom += gap;
                TPMPARAMS {
                    cbSize: size_of::<TPMPARAMS>() as u32,
                    rcExclude: rc,
                }
            });
            let result = TrackPopupMenuEx(
                self.root,
                TPM_RETURNCMD
                    | TPM_RIGHTBUTTON
                    | if above { TPM_BOTTOMALIGN } else { TPM_TOPALIGN },
                point.x,
                point.y,
                hwnd,
                params.as_ref().map_or(null(), |p| p),
            );
            if !hook.is_null() {
                UnhookWindowsHookEx(hook);
            }
            ACTIVE.with(|s| s.borrow_mut().pop());
            RemoveWindowSubclass(hwnd, Some(owner_proc), data);
            result as usize
        }
    }
}

unsafe extern "system" fn owner_proc(
    hwnd: HWND,
    msg: u32,
    w: WPARAM,
    l: LPARAM,
    _: usize,
    data: usize,
) -> LRESULT {
    unsafe {
        let popup = &*(data as *const PopupStyle);
        match msg {
            WM_MEASUREITEM if l != 0 && popup.measure(&mut *(l as *mut MEASUREITEMSTRUCT)) => 1,
            WM_DRAWITEM if l != 0 && popup.draw(&*(l as *const DRAWITEMSTRUCT)) => 1,
            _ => DefSubclassProc(hwnd, msg, w, l),
        }
    }
}

struct TrackingMenus {
    menus: Vec<HMENU>,
    dpi: i32,
}

thread_local! {
    static ACTIVE: RefCell<Vec<TrackingMenus>> = const { RefCell::new(Vec::new()) };
    static FRAMING: Cell<bool> = const { Cell::new(false) };
}

unsafe extern "system" fn after_message(code: i32, w: WPARAM, l: LPARAM) -> LRESULT {
    unsafe {
        if code >= 0 && l != 0 {
            let msg = &*(l as *const CWPRETSTRUCT);
            if matches!(
                msg.message,
                WM_WINDOWPOSCHANGED | WM_NCPAINT | WM_PAINT | WM_PRINT
            ) && GetClassLongW(msg.hwnd, GCW_ATOM) == 32768
                && !FRAMING.replace(true)
            {
                let menu = SendMessageW(msg.hwnd, MN_GETHMENU, 0, 0) as HMENU;
                let dpi = ACTIVE.with(|s| {
                    s.borrow()
                        .iter()
                        .rev()
                        .find(|tracking| tracking.menus.contains(&menu))
                        .map(|tracking| tracking.dpi)
                });
                if let Some(dpi) = dpi {
                    ui::popup_frame::shape(msg.hwnd, dpi);
                    let printing = msg.message == WM_PRINT;
                    let dc = if printing {
                        msg.wParam as HDC
                    } else {
                        GetWindowDC(msg.hwnd)
                    };
                    if !dc.is_null() {
                        paint_frame(msg.hwnd, dc, dpi);
                        if !printing {
                            ReleaseDC(msg.hwnd, dc);
                        }
                    }
                }
                FRAMING.set(false);
            }
        }
        CallNextHookEx(null_mut(), code, w, l)
    }
}

fn paint_frame(hwnd: HWND, dc: HDC, dpi: i32) {
    // Leave USER32's scrolling affordances and all client pixels intact.
    unsafe {
        let mut window = RECT::default();
        let mut client = RECT::default();
        let mut origin = POINT::default();
        if GetWindowRect(hwnd, &mut window) == 0
            || GetClientRect(hwnd, &mut client) == 0
            || ClientToScreen(hwnd, &mut origin) == 0
        {
            return;
        }
        let saved = SaveDC(dc);
        if saved != 0 {
            let x = origin.x - window.left;
            let y = origin.y - window.top;
            ExcludeClipRect(dc, x, y, x + client.right, y + client.bottom);
            let bounds = RECT {
                left: 0,
                top: 0,
                right: window.right - window.left,
                bottom: window.bottom - window.top,
            };
            ui::fill(dc, &bounds, theme::control_surface());
            RestoreDC(dc, saved);
        }
        ui::popup_frame::draw(hwnd, dc, dpi);
    }
}
