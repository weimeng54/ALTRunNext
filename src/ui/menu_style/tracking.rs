// Copyright (c) 2026 GPT-6
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::*;
use std::{cell::RefCell, ptr::null};
use windows_sys::Win32::UI::HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi};
use windows_sys::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};

thread_local! {
    static OPEN_BUTTON: Cell<Option<(HWND, bool)>> = const { Cell::new(None) };
}

pub(in crate::ui) fn button_direction(button: HWND) -> Option<bool> {
    OPEN_BUTTON
        .get()
        .filter(|&(hwnd, _)| hwnd == button)
        .map(|(_, above)| above)
}

fn set_open_button(button: HWND, above: Option<bool>) {
    let state = above.map(|above| (button, above));
    if OPEN_BUTTON.replace(state) != state {
        unsafe {
            InvalidateRect(button, null(), 0);
        }
    }
}

impl PopupStyle {
    pub(crate) fn track(&self, hwnd: HWND, anchor: Anchor) -> usize {
        let gap = scaled(3, self.dpi);
        self.button_width.set(None);
        let (point, exclude, above, button) = match anchor {
            Anchor::Pointer(point) => (point, None, false, None),
            Anchor::Below(r) => (
                POINT {
                    x: r.left,
                    y: r.bottom + gap,
                },
                Some(r),
                false,
                None,
            ),
            Anchor::Button {
                hwnd: button,
                above,
            } => {
                let mut r = RECT::default();
                if unsafe { GetWindowRect(button, &mut r) } == 0 {
                    return 0;
                }
                // USER32 adds its submenu-arrow slot and nonclient frame after WM_MEASUREITEM.
                // Their size follows the physical monitor even when the UI uses a custom scale.
                let dpi = unsafe { GetDpiForWindow(button) }.max(96);
                let frame = 2 * unsafe { GetSystemMetricsForDpi(SM_CXDLGFRAME, dpi) };
                let gutter = scaled(14, dpi as i32);
                self.button_width
                    .set(Some((r.right - r.left - frame, gutter)));
                (
                    POINT {
                        x: r.left,
                        y: if above { r.top - gap } else { r.bottom + gap },
                    },
                    Some(r),
                    above,
                    Some(button),
                )
            }
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
            let tracking = Tracking {
                owner: hwnd,
                button,
                popup: self,
                windows: RefCell::new(Vec::new()),
                shaping: Cell::new(false),
            };
            let data = &tracking as *const Tracking<'_> as usize;
            if SetWindowSubclass(hwnd, Some(owner_proc), data, data) == 0 {
                return 0;
            }
            let params = exclude.map(|mut rc| {
                rc.top -= gap;
                rc.bottom += gap;
                TPMPARAMS {
                    cbSize: size_of::<TPMPARAMS>() as u32,
                    rcExclude: rc,
                }
            });
            if let Some(button) = tracking.button {
                set_open_button(button, Some(above));
            }
            let result = TrackPopupMenuEx(
                self.root,
                TPM_RETURNCMD
                    | TPM_NOANIMATION
                    | TPM_RIGHTBUTTON
                    | if above { TPM_BOTTOMALIGN } else { TPM_TOPALIGN },
                point.x,
                point.y,
                hwnd,
                params.as_ref().map_or(null(), |p| p),
            );
            if let Some(button) = tracking.button {
                set_open_button(button, None);
            }
            KillTimer(hwnd, data);
            for (_, window) in tracking.windows.take() {
                RemoveWindowSubclass(window, Some(frame_proc), data);
            }
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
        let tracking = &*(data as *const Tracking<'_>);
        match msg {
            WM_MEASUREITEM
                if l != 0 && tracking.popup.measure(&mut *(l as *mut MEASUREITEMSTRUCT)) =>
            {
                1
            }
            WM_DRAWITEM if l != 0 && tracking.draw_item(&*(l as *const DRAWITEMSTRUCT)) => 1,
            // USER32 can dispatch built-in menu messages entirely in the system window
            // procedure, bypassing thread call-window hooks. WM_ENTERIDLE supplies
            // the actual popup HWND once the native tracking loop has displayed it.
            WM_ENTERIDLE if w == MSGF_MENU as usize => {
                tracking.attach(l as HWND);
                DefSubclassProc(hwnd, msg, w, l)
            }
            WM_TIMER if w == data => {
                KillTimer(hwnd, data);
                // The native tracking loop also dispatches owner window timers.
                // Force a real paint after USER32 has presented the menu. A
                // posted WM_NCPAINT alone doesn't invalidate its cached frame.
                let windows = tracking.windows.borrow().clone();
                for (_, window) in windows {
                    if IsWindowVisible(window) != 0 {
                        RedrawWindow(
                            window,
                            null(),
                            null_mut(),
                            RDW_FRAME | RDW_INVALIDATE | RDW_UPDATENOW,
                        );
                    }
                }
                0
            }
            _ => DefSubclassProc(hwnd, msg, w, l),
        }
    }
}

struct Tracking<'a> {
    owner: HWND,
    button: Option<HWND>,
    popup: &'a PopupStyle,
    windows: RefCell<Vec<(HMENU, HWND)>>,
    shaping: Cell<bool>,
}

impl Tracking<'_> {
    fn sync_button(&self, window: HWND) {
        if let Some(button) = self.button {
            let mut popup = RECT::default();
            let mut bounds = RECT::default();
            unsafe {
                if GetWindowRect(window, &mut popup) != 0 && GetWindowRect(button, &mut bounds) != 0
                {
                    set_open_button(button, Some(popup.bottom <= bounds.top));
                }
            }
        }
    }
    fn draw_item(&self, item: &DRAWITEMSTRUCT) -> bool {
        if item.CtlType != ODT_MENU {
            return false;
        }
        self.attach(unsafe { WindowFromDC(item.hDC) });
        // A buffered item DC can have no associated HWND. Match the item's
        // HMENU to its tracked popup, including when a cascade is open.
        let window = self
            .windows
            .borrow()
            .iter()
            .find(|(menu, _)| *menu == item.hwndItem)
            .map_or(null_mut(), |&(_, hwnd)| hwnd);
        self.popup.draw(item, window)
    }

    fn shape(&self, hwnd: HWND) {
        // SetWindowRgn synchronously sends WM_WINDOWPOSCHANGED.
        if !self.shaping.replace(true) {
            ui::popup_frame::shape(hwnd, self.popup.dpi);
            self.shaping.set(false);
        }
    }

    fn attach(&self, hwnd: HWND) {
        if hwnd.is_null() || self.windows.borrow().iter().any(|&(_, w)| w == hwnd) {
            return;
        }
        unsafe {
            if GetClassLongW(hwnd, GCW_ATOM) != 32768 {
                return;
            }
            let menu = SendMessageW(hwnd, MN_GETHMENU, 0, 0) as HMENU;
            if !self.popup.columns.contains_key(&menu) {
                return;
            }
            let data = self as *const Self as usize;
            if SetWindowSubclass(hwnd, Some(frame_proc), data, data) == 0 {
                return;
            }
            self.windows.borrow_mut().push((menu, hwnd));
            if menu == self.popup.root {
                self.sync_button(hwnd);
            }
            self.shape(hwnd);
            // TPM_NOANIMATION applies to the root only. A hover-opened
            // cascade can still present a buffered native frame after its
            // first paint. Schedule one repaint after the opening transition,
            // including when the pointer remains stationary.
            let delay = if menu == self.popup.root {
                USER_TIMER_MINIMUM
            } else {
                200
            };
            SetTimer(self.owner, data, delay, None);
        }
    }
}

unsafe extern "system" fn frame_proc(
    hwnd: HWND,
    msg: u32,
    w: WPARAM,
    l: LPARAM,
    id: usize,
    data: usize,
) -> LRESULT {
    unsafe {
        let tracking = &*(data as *const Tracking<'_>);
        if msg == WM_NCDESTROY {
            tracking
                .windows
                .borrow_mut()
                .retain(|&(_, window)| window != hwnd);
            RemoveWindowSubclass(hwnd, Some(frame_proc), id);
            return DefSubclassProc(hwnd, msg, w, l);
        }
        let result = DefSubclassProc(hwnd, msg, w, l);
        match msg {
            WM_WINDOWPOSCHANGED => {
                tracking.shape(hwnd);
                if tracking
                    .windows
                    .borrow()
                    .contains(&(tracking.popup.root, hwnd))
                {
                    tracking.sync_button(hwnd);
                }
                draw_window_frame(hwnd, tracking.popup.dpi);
            }
            WM_NCPAINT | WM_PAINT => draw_window_frame(hwnd, tracking.popup.dpi),
            WM_PRINT if w != 0 => paint_frame(hwnd, w as HDC, tracking.popup.dpi),
            _ => {}
        }
        result
    }
}

fn draw_window_frame(hwnd: HWND, dpi: i32) {
    unsafe {
        let dc = GetWindowDC(hwnd);
        if !dc.is_null() {
            paint_frame(hwnd, dc, dpi);
            ReleaseDC(hwnd, dc);
        }
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
