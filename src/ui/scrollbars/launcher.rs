// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::{fill, rounded, scaled};
use crate::{theme, ui::geometry::window_dpi};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    UI::{
        Input::KeyboardAndMouse::{GetCapture, ReleaseCapture, SetCapture},
        Shell::DefSubclassProc,
        WindowsAndMessaging::*,
    },
};

const TRACK_INSET: i32 = 4;
const TRACK_WIDTH: i32 = 3;
const THUMB_WIDTH: i32 = 10;
const MINIMUM_THUMB: i32 = 24;
const MINIMUM_TRAVEL: i32 = 8;

struct Thumb {
    start: i32,
    top: i32,
    length: i32,
    travel: i32,
    last: i32,
    current: i32,
}

impl Thumb {
    fn measure(list: HWND, height: i32, dpi: i32) -> Option<Self> {
        let mut info = SCROLLINFO {
            cbSize: size_of::<SCROLLINFO>() as u32,
            fMask: SIF_RANGE | SIF_PAGE | SIF_POS,
            ..Default::default()
        };
        if unsafe { GetScrollInfo(list, SB_VERT, &mut info) } == 0 {
            return None;
        }
        let total = info.nMax as i64 - info.nMin as i64 + 1;
        let page = i64::from(info.nPage.max(1));
        if total <= page || height <= 1 {
            return None;
        }
        let last = (total - page).min(i64::from(i32::MAX)) as i32;
        let start = scaled(TRACK_INSET, dpi).min((height - 2) / 2);
        let usable = height - 2 * start;
        let minimum =
            scaled(MINIMUM_THUMB, dpi).clamp(1, (usable - scaled(MINIMUM_TRAVEL, dpi)).max(1));
        let length = ((i64::from(usable) * page / total) as i32).clamp(minimum, usable - 1);
        let travel = usable - length;
        let current =
            (i64::from(info.nPos) - i64::from(info.nMin)).clamp(0, i64::from(last)) as i32;
        let top = start
            + ((i64::from(current) * i64::from(travel) + i64::from(last) / 2) / i64::from(last))
                as i32;
        Some(Self {
            start,
            top,
            length,
            travel,
            last,
            current,
        })
    }

    fn scroll_to(&self, list: HWND, y: i32, grab: i32) {
        let pointer = y - grab - self.start;
        // Several row indices can share one pixel in a long list.
        if pointer == self.top - self.start {
            return;
        }
        let offset = pointer.clamp(0, self.travel);
        let position = ((i64::from(offset) * i64::from(self.last) + i64::from(self.travel) / 2)
            / i64::from(self.travel)) as i32;
        if position != self.current {
            unsafe { SendMessageW(list, LB_SETTOPINDEX, position as usize, 0) };
        }
    }
}

fn centered_bar(bounds: RECT, width: i32, top: i32, bottom: i32) -> RECT {
    let width = width.clamp(1, (bounds.right - bounds.left).max(1));
    let left = bounds.left + (bounds.right - bounds.left - width) / 2;
    RECT {
        left,
        top,
        right: left + width,
        bottom,
    }
}

fn paint(hwnd: HWND, list: HWND, dc: HDC) {
    let mut bounds = RECT::default();
    unsafe { GetClientRect(hwnd, &mut bounds) };
    fill(dc, &bounds, theme::paper());
    let dpi = window_dpi(list);
    if let Some(thumb) = Thumb::measure(list, bounds.bottom, dpi) {
        let track = centered_bar(
            bounds,
            scaled(TRACK_WIDTH, dpi),
            thumb.start,
            bounds.bottom - thumb.start,
        );
        rounded(
            dc,
            track,
            track.right - track.left,
            theme::divider(),
            theme::divider(),
        );
        let bar = centered_bar(
            bounds,
            scaled(THUMB_WIDTH, dpi),
            thumb.top,
            thumb.top + thumb.length,
        );
        let colour = if !theme::high_contrast() && unsafe { GetCapture() == hwnd } {
            theme::accent()
        } else {
            theme::muted()
        };
        rounded(dc, bar, bar.right - bar.left, colour, colour);
    }
}

pub(super) unsafe extern "system" fn procedure(
    hwnd: HWND,
    msg: u32,
    w: WPARAM,
    l: LPARAM,
    _: usize,
    list: usize,
) -> LRESULT {
    let list = list as HWND;
    unsafe {
        match msg {
            WM_NCHITTEST => HTCLIENT as isize,
            WM_ERASEBKGND => 1,
            WM_PRINTCLIENT => {
                paint(hwnd, list, w as HDC);
                0
            }
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let dc = BeginPaint(hwnd, &mut ps);
                super::super::buffered(dc, ps.rcPaint, |target| paint(hwnd, list, target));
                EndPaint(hwnd, &ps);
                0
            }
            WM_LBUTTONDOWN | WM_MOUSEMOVE => {
                let dragging = GetCapture() == hwnd;
                if msg == WM_LBUTTONDOWN || dragging {
                    let mut bounds = RECT::default();
                    GetClientRect(hwnd, &mut bounds);
                    if let Some(thumb) = Thumb::measure(list, bounds.bottom, window_dpi(list)) {
                        let y = (l >> 16) as i16 as i32;
                        if msg == WM_LBUTTONDOWN {
                            let on_thumb = (thumb.top..thumb.top + thumb.length).contains(&y);
                            let grab = if on_thumb {
                                y - thumb.top
                            } else {
                                thumb.length / 2
                            };
                            SetWindowLongPtrW(hwnd, GWLP_USERDATA, grab as isize);
                            SetCapture(hwnd);
                            if !on_thumb {
                                thumb.scroll_to(list, y, grab);
                            }
                            InvalidateRect(hwnd, std::ptr::null(), 0);
                        } else {
                            thumb.scroll_to(list, y, GetWindowLongPtrW(hwnd, GWLP_USERDATA) as i32);
                        }
                    }
                }
                0
            }
            WM_LBUTTONUP | WM_CANCELMODE | WM_SHOWWINDOW if msg != WM_SHOWWINDOW || w == 0 => {
                if GetCapture() == hwnd {
                    ReleaseCapture();
                }
                DefSubclassProc(hwnd, msg, w, l)
            }
            WM_CAPTURECHANGED => {
                InvalidateRect(hwnd, std::ptr::null(), 0);
                0
            }
            WM_MOUSEWHEEL => SendMessageW(list, msg, w, l),
            _ => DefSubclassProc(hwnd, msg, w, l),
        }
    }
}
