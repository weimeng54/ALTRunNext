// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::{
    Axis, Target,
    appearance::{INSET, MINIMUM_THUMB, Visual},
    fill, scaled, scrollbar_info,
};
use crate::{theme, ui::geometry::window_dpi};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    UI::{
        Controls::WM_MOUSELEAVE,
        Input::KeyboardAndMouse::{
            GetCapture, ReleaseCapture, SetCapture, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent,
        },
        Shell::DefSubclassProc,
        WindowsAndMessaging::*,
    },
};

const MINIMUM_TRAVEL: i32 = 8;

enum Pointer {
    Outside,
    Hover,
    Drag(i32),
}

impl Pointer {
    fn read(hwnd: HWND) -> Self {
        // The drag offset is nonnegative. Zero is the window's initial state.
        match unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } {
            0 => Self::Outside,
            -1 => Self::Hover,
            offset => Self::Drag((offset - 1) as i32),
        }
    }

    fn apply(self, hwnd: HWND) {
        let value = match self {
            Self::Outside => 0,
            Self::Hover => -1,
            Self::Drag(offset) => offset as isize + 1,
        };
        unsafe {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, value);
            InvalidateRect(hwnd, std::ptr::null(), 0);
        }
    }

    fn enter(hwnd: HWND) {
        unsafe {
            TrackMouseEvent(&mut TRACKMOUSEEVENT {
                cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                dwFlags: TME_LEAVE,
                hwndTrack: hwnd,
                dwHoverTime: 0,
            });
        }
        Self::Hover.apply(hwnd);
    }
}

struct Thumb {
    start: i32,
    top: i32,
    length: i32,
    travel: i32,
    last: i32,
    current: i32,
    minimum: i32,
}

impl Thumb {
    fn for_window(hwnd: HWND, control: HWND, axis: Axis, dpi: i32) -> Option<Self> {
        let length = track_length(hwnd, control, axis);
        Self::measure(control, axis, length, dpi)
    }

    fn measure(control: HWND, axis: Axis, length: i32, dpi: i32) -> Option<Self> {
        let mut info = SCROLLINFO {
            cbSize: size_of::<SCROLLINFO>() as u32,
            fMask: SIF_RANGE | SIF_PAGE | SIF_POS,
            ..Default::default()
        };
        if unsafe { GetScrollInfo(control, axis.scroll_bar(), &mut info) } == 0 {
            return None;
        }
        let total = info.nMax as i64 - info.nMin as i64 + 1;
        let page = i64::from(info.nPage.max(1));
        if total <= page || length <= 1 {
            return None;
        }
        let last = (total - page).min(i64::from(i32::MAX)) as i32;
        let start = scaled(INSET, dpi).min((length - 2) / 2);
        let usable = length - 2 * start;
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
            minimum: info.nMin,
        })
    }

    fn scroll_to(&self, control: HWND, target: Target, coordinate: i32, grab: i32) {
        let pointer = coordinate - grab - self.start;
        // Several row indices can share one pixel in a long list.
        if pointer == self.top - self.start {
            return;
        }
        let offset = pointer.clamp(0, self.travel);
        let position = ((i64::from(offset) * i64::from(self.last) + i64::from(self.travel) / 2)
            / i64::from(self.travel)) as i32;
        if position != self.current {
            target.set_position(control, self.minimum, self.current, position);
        }
    }
}

fn track_length(hwnd: HWND, control: HWND, axis: Axis) -> i32 {
    if axis == Axis::Horizontal {
        return scrollbar_info(control, axis).map_or(0, |bar| axis.length(bar.rcScrollBar));
    }
    let mut bounds = RECT::default();
    unsafe { GetClientRect(hwnd, &mut bounds) };
    bounds.bottom
}

fn paint(hwnd: HWND, control: HWND, axis: Axis, dpi: i32, dc: HDC) {
    let mut bounds = RECT::default();
    unsafe { GetClientRect(hwnd, &mut bounds) };
    let length = track_length(hwnd, control, axis);
    if let Some(thumb) = Thumb::measure(control, axis, length, dpi) {
        Visual {
            track_start: thumb.start,
            track_end: length - thumb.start,
            thumb_start: thumb.top,
            thumb_end: thumb.top + thumb.length,
            active: !matches!(Pointer::read(hwnd), Pointer::Outside),
        }
        .draw(dc, bounds, axis, dpi);
    } else {
        fill(dc, &bounds, theme::paper());
    }
}

pub(super) unsafe extern "system" fn procedure(
    hwnd: HWND,
    msg: u32,
    w: WPARAM,
    l: LPARAM,
    style: usize,
    control: usize,
) -> LRESULT {
    let control = control as HWND;
    let target = Target::decode(style);
    let axis = target.axis();
    let dpi = match style / 4 {
        0 => window_dpi(control),
        dpi => dpi as i32,
    };
    unsafe {
        match msg {
            WM_NCHITTEST => {
                if axis == Axis::Horizontal
                    && scrollbar_info(control, axis)
                        .is_none_or(|bar| axis.mouse_position(l) >= bar.rcScrollBar.right)
                {
                    HTTRANSPARENT as isize
                } else {
                    HTCLIENT as isize
                }
            }
            WM_ERASEBKGND => 1,
            WM_PRINTCLIENT => {
                paint(hwnd, control, axis, dpi, w as HDC);
                0
            }
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let dc = BeginPaint(hwnd, &mut ps);
                super::super::buffered(dc, ps.rcPaint, |dc| {
                    paint(hwnd, control, axis, dpi, dc);
                });
                EndPaint(hwnd, &ps);
                0
            }
            WM_MOUSEMOVE => {
                match Pointer::read(hwnd) {
                    Pointer::Outside => Pointer::enter(hwnd),
                    Pointer::Hover => {}
                    Pointer::Drag(grab) => {
                        if let Some(thumb) = Thumb::for_window(hwnd, control, axis, dpi) {
                            thumb.scroll_to(control, target, axis.mouse_position(l), grab);
                        }
                    }
                }
                0
            }
            WM_MOUSELEAVE => {
                if matches!(Pointer::read(hwnd), Pointer::Hover) {
                    Pointer::Outside.apply(hwnd);
                }
                0
            }
            WM_LBUTTONDOWN => {
                if let Some(thumb) = Thumb::for_window(hwnd, control, axis, dpi) {
                    let coordinate = axis.mouse_position(l);
                    let on_thumb = (thumb.top..thumb.top + thumb.length).contains(&coordinate);
                    let grab = if on_thumb {
                        coordinate - thumb.top
                    } else {
                        thumb.length / 2
                    };
                    SetCapture(hwnd);
                    if GetCapture() == hwnd {
                        Pointer::Drag(grab).apply(hwnd);
                    }
                    if !on_thumb {
                        thumb.scroll_to(control, target, coordinate, grab);
                    }
                }
                0
            }
            WM_LBUTTONUP | WM_CANCELMODE | WM_SHOWWINDOW if msg != WM_SHOWWINDOW || w == 0 => {
                if msg != WM_LBUTTONUP && !matches!(Pointer::read(hwnd), Pointer::Outside) {
                    Pointer::Outside.apply(hwnd);
                }
                if GetCapture() == hwnd {
                    ReleaseCapture();
                }
                DefSubclassProc(hwnd, msg, w, l)
            }
            WM_CAPTURECHANGED => {
                if matches!(Pointer::read(hwnd), Pointer::Drag(_)) {
                    let mut cursor = POINT::default();
                    let mut bounds = RECT::default();
                    if l == 0
                        && GetCursorPos(&mut cursor) != 0
                        && GetWindowRect(hwnd, &mut bounds) != 0
                        && PtInRect(&bounds, cursor) != 0
                    {
                        Pointer::enter(hwnd);
                    } else {
                        Pointer::Outside.apply(hwnd);
                    }
                }
                0
            }
            WM_MOUSEWHEEL | WM_MOUSEHWHEEL => SendMessageW(control, msg, w, l),
            _ => DefSubclassProc(hwnd, msg, w, l),
        }
    }
}
