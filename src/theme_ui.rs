// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

//! Shared painting for native controls which do not honor application colors.
use crate::{
    native::{scaled, wide},
    theme,
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::LibraryLoader::GetModuleHandleW,
    UI::{
        Controls::*,
        Input::KeyboardAndMouse::GetFocus,
        Shell::{DefSubclassProc, SetWindowSubclass},
        WindowsAndMessaging::*,
    },
};

pub const STATIC_CENTER_IMAGE: u32 = 0x0200;

pub fn fill(dc: HDC, rect: &RECT, colour: u32) {
    unsafe {
        let brush = CreateSolidBrush(colour);
        FillRect(dc, rect, brush);
        DeleteObject(brush);
    }
}

#[derive(Clone, Copy)]
pub enum Chevron {
    Right,
    Down,
    Up,
}

pub fn chevron(dc: HDC, center: POINT, direction: Chevron, dpi: i32, ink: u32) {
    let points = [(-3, -4), (-1, -4), (3, 0), (-1, 4), (-3, 4), (1, 0)].map(|(x, y)| {
        let (x, y) = match direction {
            Chevron::Right => (x, y),
            Chevron::Down => (-y, x),
            Chevron::Up => (y, -x),
        };
        POINT {
            x: center.x + scaled(x, dpi),
            y: center.y + scaled(y, dpi),
        }
    });
    unsafe {
        let brush = CreateSolidBrush(ink);
        let old_brush = SelectObject(dc, brush);
        let old_pen = SelectObject(dc, GetStockObject(NULL_PEN));
        Polygon(dc, points.as_ptr(), points.len() as i32);
        SelectObject(dc, old_pen);
        SelectObject(dc, old_brush);
        DeleteObject(brush);
    }
}

pub fn vertical_scrollbar(hwnd: HWND) -> Option<SCROLLBARINFO> {
    scrollbar_info(hwnd, OBJID_VSCROLL)
}

fn scrollbar_info(hwnd: HWND, object: OBJECT_IDENTIFIER) -> Option<SCROLLBARINFO> {
    let mut bar = SCROLLBARINFO {
        cbSize: std::mem::size_of::<SCROLLBARINFO>() as u32,
        ..Default::default()
    };
    unsafe {
        (GetScrollBarInfo(hwnd, object, &mut bar) != 0
            && bar.rgstate[0] & STATE_SYSTEM_INVISIBLE == 0)
            .then_some(bar)
    }
}

pub fn draw_scrollbar(dc: HDC, track: RECT, bar: &SCROLLBARINFO, dpi: i32) {
    draw_scrollbar_on_axis(dc, track, bar, dpi, false);
}

fn draw_scrollbar_on_axis(dc: HDC, track: RECT, bar: &SCROLLBARINFO, dpi: i32, horizontal: bool) {
    fill(dc, &track, theme::paper());
    if bar.xyThumbBottom > bar.xyThumbTop {
        let width = scaled(4, dpi).max(3);
        let thumb = if horizontal {
            let top = (track.top + track.bottom - width) / 2;
            RECT {
                left: track.left + bar.xyThumbTop,
                right: track.left + bar.xyThumbBottom,
                top,
                bottom: top + width,
            }
        } else {
            let left = (track.left + track.right - width) / 2;
            RECT {
                left,
                right: left + width,
                top: track.top + bar.xyThumbTop,
                bottom: track.top + bar.xyThumbBottom,
            }
        };
        rounded(dc, thumb, width, theme::muted(), theme::muted());
    }
}

pub fn style_vertical_scrollbar(control: HWND, id: usize, dpi: i32) {
    style_scrollbar(control, id, dpi, false);
}

pub fn style_horizontal_scrollbar(control: HWND, id: usize, dpi: i32) {
    style_scrollbar(control, id, dpi, true);
}

fn style_scrollbar(control: HWND, id: usize, dpi: i32, horizontal: bool) {
    unsafe {
        let scrollbar = CreateWindowExW(
            0,
            wide("STATIC").as_ptr(),
            std::ptr::null(),
            WS_CHILD | WS_CLIPSIBLINGS,
            0,
            0,
            0,
            0,
            GetParent(control),
            id as HMENU,
            std::ptr::null_mut(),
            std::ptr::null(),
        );
        if !scrollbar.is_null()
            && SetWindowSubclass(
                scrollbar,
                Some(scrollbar_proc),
                dpi as usize * 2 + usize::from(horizontal),
                control as usize,
            ) != 0
        {
            SetWindowSubclass(
                control,
                Some(scroll_control_proc),
                1 + usize::from(horizontal),
                scrollbar as usize,
            );
        }
    }
}

unsafe extern "system" fn scroll_control_proc(
    hwnd: HWND,
    msg: u32,
    w: WPARAM,
    l: LPARAM,
    subclass: usize,
    scrollbar: usize,
) -> LRESULT {
    let result = unsafe { DefSubclassProc(hwnd, msg, w, l) };
    if matches!(
        msg,
        WM_NCPAINT
            | WM_PAINT
            | WM_VSCROLL
            | WM_HSCROLL
            | WM_MOUSEWHEEL
            | WM_KEYDOWN
            | WM_NCLBUTTONDOWN
            | WM_SIZE
            | LB_SETTOPINDEX
            | LB_SETCURSEL
            | EM_LINESCROLL
            | EM_SCROLL
            | WM_SETTEXT
            | LVM_SCROLL
            | LVM_SETCOLUMNWIDTH
            | LVM_ENSUREVISIBLE
    ) {
        update_scrollbar(hwnd, scrollbar as HWND, subclass == 2);
    }
    result
}

fn update_scrollbar(list: HWND, scrollbar: HWND, horizontal: bool) {
    unsafe {
        let visible = GetWindowLongW(scrollbar, GWL_STYLE) as u32 & WS_VISIBLE != 0;
        let object = if horizontal {
            OBJID_HSCROLL
        } else {
            OBJID_VSCROLL
        };
        let Some(bar) = scrollbar_info(list, object) else {
            if visible {
                ShowWindow(scrollbar, SW_HIDE);
            }
            return;
        };
        let mut bounds = RECT::default();
        GetWindowRect(scrollbar, &mut bounds);
        let mut track = bar.rcScrollBar;
        if horizontal && let Some(vertical) = vertical_scrollbar(list) {
            // The native size box between the two scrollbars uses the system
            // background. Extend the themed track across that corner.
            track.right = track.right.max(vertical.rcScrollBar.right);
        }
        if !visible || EqualRect(&bounds, &track) == 0 {
            let mut origin = POINT {
                x: track.left,
                y: track.top,
            };
            ScreenToClient(GetParent(scrollbar), &mut origin);
            SetWindowPos(
                scrollbar,
                HWND_TOP,
                origin.x,
                origin.y,
                track.right - track.left,
                track.bottom - track.top,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
        }
        // Native scrollbar tracking can draw directly into its list window.
        // A separate, clipped sibling keeps our colors visible during that loop.
        RedrawWindow(
            scrollbar,
            std::ptr::null(),
            std::ptr::null_mut(),
            RDW_INVALIDATE | RDW_UPDATENOW,
        );
    }
}

unsafe extern "system" fn scrollbar_proc(
    hwnd: HWND,
    msg: u32,
    w: WPARAM,
    l: LPARAM,
    style: usize,
    list: usize,
) -> LRESULT {
    match msg {
        WM_NCHITTEST => HTTRANSPARENT as isize,
        WM_ERASEBKGND => 1,
        WM_PAINT | WM_PRINTCLIENT => unsafe {
            let mut paint = PAINTSTRUCT::default();
            let dc = if msg == WM_PAINT {
                BeginPaint(hwnd, &mut paint)
            } else {
                w as HDC
            };
            let mut track = RECT::default();
            GetClientRect(hwnd, &mut track);
            let horizontal = style & 1 != 0;
            let object = if horizontal {
                OBJID_HSCROLL
            } else {
                OBJID_VSCROLL
            };
            if let Some(bar) = scrollbar_info(list as HWND, object) {
                draw_scrollbar_on_axis(dc, track, &bar, (style / 2) as i32, horizontal);
            } else {
                fill(dc, &track, theme::paper());
            }
            if msg == WM_PAINT {
                EndPaint(hwnd, &paint);
            }
            0
        },
        _ => unsafe { DefSubclassProc(hwnd, msg, w, l) },
    }
}

struct AppIcon(HICON);

impl Drop for AppIcon {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                DestroyIcon(self.0);
            }
        }
    }
}

pub fn app_icon(size: i32) -> HICON {
    thread_local! {
        static ICONS: std::cell::RefCell<std::collections::BTreeMap<i32, AppIcon>> =
            const { std::cell::RefCell::new(std::collections::BTreeMap::new()) };
    }
    ICONS.with(|icons| {
        icons
            .borrow_mut()
            .entry(size)
            .or_insert_with(|| {
                AppIcon(unsafe {
                    LoadImageW(
                        GetModuleHandleW(std::ptr::null()),
                        1usize as _,
                        IMAGE_ICON,
                        size,
                        size,
                        0,
                    ) as HICON
                })
            })
            .0
    })
}

pub fn draw_app_icon(dc: HDC, bounds: RECT) {
    let size = bounds.right - bounds.left;
    let icon = app_icon(size);
    if !icon.is_null() {
        unsafe {
            DrawIconEx(
                dc,
                bounds.left,
                bounds.top,
                icon,
                size,
                size,
                0,
                std::ptr::null_mut(),
                DI_NORMAL,
            );
        }
    }
}

pub fn panel_caption(dc: HDC, dpi: i32, font: HFONT, title: &str) {
    draw_app_icon(
        dc,
        crate::settings::layout::BoxRect::new(16, 12, 18, 18).pixels(dpi),
    );
    unsafe {
        let old = SelectObject(dc, font);
        line_text(
            dc,
            RECT {
                left: scaled(44, dpi),
                top: 0,
                right: scaled(334, dpi),
                bottom: scaled(crate::settings::layout::CAPTION, dpi),
            },
            &crate::localization::tr(title),
            theme::text(),
            0,
        );
        SelectObject(dc, old);
    }
}

pub fn round_panel(hwnd: HWND, width: i32, height: i32, dpi: i32) {
    unsafe {
        let radius = scaled(22, dpi);
        let region = CreateRoundRectRgn(0, 0, width + 1, height + 1, radius, radius);
        if !region.is_null() && SetWindowRgn(hwnd, region, 0) == 0 {
            DeleteObject(region);
        }
    }
}

pub fn line_text(dc: HDC, mut rect: RECT, value: &str, colour: u32, flags: u32) {
    unsafe {
        SetBkMode(dc, TRANSPARENT as i32);
        SetTextColor(dc, colour);
        DrawTextW(
            dc,
            wide(value).as_ptr(),
            -1,
            &mut rect,
            DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS | DT_NOPREFIX | flags,
        );
    }
}

pub fn window_text(hwnd: HWND) -> String {
    unsafe {
        let mut value = vec![0; GetWindowTextLengthW(hwnd).max(0) as usize + 1];
        let n = GetWindowTextW(hwnd, value.as_mut_ptr(), value.len() as i32);
        String::from_utf16_lossy(&value[..n.max(0) as usize])
    }
}

pub fn rounded(dc: HDC, rect: RECT, radius: i32, colour: u32, border: u32) {
    if smooth_round(dc, rect, radius, colour, border) {
        return;
    }
    unsafe {
        let brush = CreateSolidBrush(colour);
        let pen = CreatePen(PS_SOLID, 1, border);
        let old_brush = SelectObject(dc, brush);
        let old_pen = SelectObject(dc, pen);
        RoundRect(
            dc,
            rect.left,
            rect.top,
            rect.right,
            rect.bottom,
            radius,
            radius,
        );
        SelectObject(dc, old_pen);
        SelectObject(dc, old_brush);
        DeleteObject(pen);
        DeleteObject(brush);
    }
}

// Only the four corners need compositing. The body is filled directly with
// GDI, so rounded cards never allocate or rasterize a window-sized surface.
const CORNER_TILE: i32 = 64;

struct Corners {
    dc: HDC,
    bitmap: HBITMAP,
    pixels: *mut u32,
    key: (i32, u32, u32),
}

impl Corners {
    fn new() -> Option<Self> {
        unsafe {
            let dc = CreateCompatibleDC(std::ptr::null_mut());
            if dc.is_null() {
                return None;
            }
            let mut info = BITMAPINFO::default();
            info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
            info.bmiHeader.biWidth = CORNER_TILE;
            info.bmiHeader.biHeight = -CORNER_TILE;
            info.bmiHeader.biPlanes = 1;
            info.bmiHeader.biBitCount = 32;
            let mut pixels = std::ptr::null_mut();
            let bitmap = CreateDIBSection(
                dc,
                &info,
                DIB_RGB_COLORS,
                &mut pixels,
                std::ptr::null_mut(),
                0,
            );
            if bitmap.is_null() {
                DeleteDC(dc);
                return None;
            }
            SelectObject(dc, bitmap);
            Some(Self {
                dc,
                bitmap,
                pixels: pixels.cast(),
                key: (0, 0, 0),
            })
        }
    }

    fn prepare(&mut self, radius: i32, colour: u32, border: u32) {
        let key = (radius, colour, border);
        if self.key == key {
            return;
        }
        self.key = key;
        let outer = radius * 8;
        let inner = outer - 8;
        let channel = |shift: u32, fill_samples: u32, stroke_samples: u32| {
            (((colour >> shift) & 255u32) * fill_samples
                + ((border >> shift) & 255u32) * stroke_samples
                + 8)
                / 16
        };
        unsafe {
            // Flush reads of this DIB before replacing its pixels.
            GdiFlush();
            for y in 0..radius {
                for x in 0..radius {
                    let mut coverage = 0;
                    let mut inside = 0;
                    for dy in [1, 3, 5, 7] {
                        for dx in [1, 3, 5, 7] {
                            let vx = outer - (x * 8 + dx);
                            let vy = outer - (y * 8 + dy);
                            let distance = vx * vx + vy * vy;
                            coverage += u32::from(distance <= outer * outer);
                            inside += u32::from(distance <= inner * inner);
                        }
                    }
                    let stroke = coverage - inside;
                    let pixel = ((coverage * 255 + 8) / 16) << 24
                        | channel(0, inside, stroke) << 16
                        | channel(8, inside, stroke) << 8
                        | channel(16, inside, stroke);
                    for (tx, ty) in [
                        (x, y),
                        (2 * radius - 1 - x, y),
                        (x, 2 * radius - 1 - y),
                        (2 * radius - 1 - x, 2 * radius - 1 - y),
                    ] {
                        *self.pixels.add((ty * CORNER_TILE + tx) as usize) = pixel;
                    }
                }
            }
        }
    }
}

impl Drop for Corners {
    fn drop(&mut self) {
        unsafe {
            DeleteDC(self.dc);
            DeleteObject(self.bitmap);
        }
    }
}

thread_local! {
    static CORNERS: std::cell::RefCell<Option<Corners>> = std::cell::RefCell::new(Corners::new());
}

fn smooth_round(dc: HDC, rect: RECT, diameter: i32, colour: u32, border: u32) -> bool {
    let radius = ((diameter + 1) / 2)
        .min((rect.right - rect.left) / 2)
        .min((rect.bottom - rect.top) / 2);
    if radius <= 1 || radius > CORNER_TILE / 2 {
        return false;
    }
    CORNERS.with(|buffer| {
        let mut buffer = buffer.borrow_mut();
        let Some(tile) = buffer.as_mut() else {
            return false;
        };
        tile.prepare(radius, colour, border);
        let edge = i32::from(colour != border);
        let RECT {
            left,
            top,
            right,
            bottom,
        } = rect;
        if left + radius < right - radius {
            fill(
                dc,
                &RECT {
                    left: left + radius,
                    right: right - radius,
                    ..rect
                },
                border,
            );
            fill(
                dc,
                &RECT {
                    left: left + radius,
                    right: right - radius,
                    top: top + edge,
                    bottom: bottom - edge,
                },
                colour,
            );
        }
        if top + radius < bottom - radius {
            fill(
                dc,
                &RECT {
                    top: top + radius,
                    bottom: bottom - radius,
                    ..rect
                },
                border,
            );
            fill(
                dc,
                &RECT {
                    top: top + radius,
                    bottom: bottom - radius,
                    left: left + edge,
                    right: right - edge,
                },
                colour,
            );
        }
        unsafe {
            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };
            for (x, y, tx, ty) in [
                (left, top, 0, 0),
                (right - radius, top, radius, 0),
                (left, bottom - radius, 0, radius),
                (right - radius, bottom - radius, radius, radius),
            ] {
                GdiAlphaBlend(
                    dc, x, y, radius, radius, tile.dc, tx, ty, radius, radius, blend,
                );
            }
        }
        true
    })
}

pub fn button_on(item: &DRAWITEMSTRUCT, primary: bool, backdrop: u32, dpi: i32) {
    draw_button(item, primary, backdrop, dpi, false);
}

pub fn menu_button(item: &DRAWITEMSTRUCT, dpi: i32) {
    draw_button(item, false, theme::chrome(), dpi, true);
}

fn draw_button(item: &DRAWITEMSTRUCT, primary: bool, backdrop: u32, dpi: i32, menu: bool) {
    let dc = item.hDC;
    let rect = item.rcItem;
    let pressed = item.itemState & ODS_SELECTED != 0;
    let disabled = item.itemState & ODS_DISABLED != 0;
    let colour = if disabled && primary && !theme::high_contrast() {
        if theme::is_dark() {
            theme::rgb(32, 38, 46)
        } else {
            theme::rgb(227, 234, 242)
        }
    } else if disabled {
        theme::surface()
    } else if primary && !pressed {
        theme::accent()
    } else if pressed {
        theme::selection()
    } else {
        theme::control_surface()
    };
    let text = if disabled && primary && !theme::high_contrast() {
        if theme::is_dark() {
            theme::rgb(139, 152, 166)
        } else {
            theme::rgb(105, 122, 142)
        }
    } else if disabled {
        theme::muted()
    } else if primary && !pressed {
        theme::on_accent()
    } else {
        theme::text()
    };
    fill(dc, &rect, backdrop);
    rounded(
        dc,
        rect,
        scaled(12, dpi),
        colour,
        if disabled && primary {
            theme::divider()
        } else if item.itemState & ODS_FOCUS != 0 {
            theme::accent()
        } else {
            theme::border()
        },
    );
    unsafe {
        let font = SendMessageW(item.hwndItem, WM_GETFONT, 0, 0) as HFONT;
        let old = SelectObject(dc, font);
        let mut label = rect;
        if menu {
            label.right -= scaled(16, dpi);
            chevron(
                dc,
                POINT {
                    x: rect.right - scaled(13, dpi),
                    y: (rect.top + rect.bottom) / 2,
                },
                Chevron::Down,
                dpi,
                text,
            );
        }
        line_text(dc, label, &window_text(item.hwndItem), text, DT_CENTER);
        SelectObject(dc, old);
    }
}

pub fn combo(item: &DRAWITEMSTRUCT, dpi: i32) {
    let selected = item.itemState & ODS_SELECTED != 0;
    let dc = item.hDC;
    let mut rect = item.rcItem;
    fill(
        dc,
        &rect,
        if selected {
            theme::selection()
        } else {
            theme::paper()
        },
    );
    if item.itemID == u32::MAX {
        return;
    }
    unsafe {
        let hwnd = item.hwndItem;
        let n = SendMessageW(hwnd, CB_GETLBTEXTLEN, item.itemID as usize, 0);
        if n < 0 {
            return;
        }
        let mut label = vec![0u16; n as usize + 1];
        let n = SendMessageW(
            hwnd,
            CB_GETLBTEXT,
            item.itemID as usize,
            label.as_mut_ptr() as isize,
        );
        if n < 0 {
            return;
        }
        let old = SelectObject(dc, SendMessageW(hwnd, WM_GETFONT, 0, 0) as HFONT);
        rect.left += scaled(5, dpi);
        rect.right -= scaled(4, dpi);
        line_text(
            dc,
            rect,
            &String::from_utf16_lossy(&label[..n as usize]),
            theme::text(),
            0,
        );
        SelectObject(dc, old);
        if item.itemState & ODS_FOCUS != 0 {
            DrawFocusRect(dc, &rect);
        }
    }
}

pub fn style_combo(hwnd: HWND, dpi: i32) {
    unsafe { SetWindowSubclass(hwnd, Some(combo_proc), 1, dpi as usize) };
}

pub fn style_edit(hwnd: HWND, dpi: i32, card: bool) {
    let mode = if card { 0x10000 } else { 0 };
    set_edit_style(hwnd, dpi, mode);
}

pub const NUMBER_STEP: u32 = 0x670;

pub fn style_number(hwnd: HWND, dpi: i32) {
    set_edit_style(hwnd, dpi, 0x30000);
}

fn number_arrow(rect: RECT, dpi: i32) -> RECT {
    RECT {
        left: rect.right - scaled(25, dpi),
        right: rect.right - scaled(6, dpi),
        top: rect.top + scaled(5, dpi),
        bottom: rect.bottom - scaled(5, dpi),
    }
}

fn draw_arrows(dc: HDC, rect: RECT, dpi: i32) {
    let well = number_arrow(rect, dpi);
    let center = (well.left + well.right) / 2;
    let middle = (well.top + well.bottom) / 2;
    let offset = scaled(4, dpi);
    for (y, direction) in [
        (middle - offset, Chevron::Up),
        (middle + offset, Chevron::Down),
    ] {
        chevron(dc, POINT { x: center, y }, direction, dpi, theme::muted());
    }
}

fn set_edit_style(hwnd: HWND, dpi: i32, mode: usize) {
    unsafe {
        SetWindowSubclass(hwnd, Some(edit_proc), 1, mode | dpi as usize);
        SetWindowPos(
            hwnd,
            std::ptr::null_mut(),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED | SWP_NOACTIVATE,
        );
    }
}

unsafe extern "system" fn edit_proc(
    hwnd: HWND,
    msg: u32,
    w: WPARAM,
    l: LPARAM,
    _: usize,
    mode: usize,
) -> LRESULT {
    let dpi = (mode & 0xffff) as i32;
    let inset_x = scaled(9, dpi);
    let inset_y = scaled(7, dpi);
    let number = mode & 0x20000 != 0;
    let inset_right = if number { scaled(27, dpi) } else { inset_x };
    unsafe {
        if number {
            let delta = if msg == WM_KEYDOWN && matches!(w as u16, 0x26 | 0x28) {
                Some(if w as u16 == 0x26 { 1 } else { -1 })
            } else if matches!(msg, WM_NCHITTEST | WM_NCLBUTTONDOWN | WM_NCLBUTTONDBLCLK) {
                let mut rect = RECT::default();
                GetWindowRect(hwnd, &mut rect);
                let well = number_arrow(rect, dpi);
                let point = POINT {
                    x: l as i16 as i32,
                    y: (l >> 16) as i16 as i32,
                };
                if PtInRect(&well, point) != 0 {
                    if msg == WM_NCHITTEST {
                        return HTBORDER as isize;
                    }
                    windows_sys::Win32::UI::Input::KeyboardAndMouse::SetFocus(hwnd);
                    Some(if point.y < (well.top + well.bottom) / 2 {
                        1
                    } else {
                        -1
                    })
                } else {
                    None
                }
            } else {
                None
            };
            if let Some(delta) = delta {
                let id = GetDlgCtrlID(hwnd) as usize;
                SendMessageW(
                    GetParent(hwnd),
                    WM_COMMAND,
                    id | ((NUMBER_STEP as usize) << 16),
                    delta,
                );
                return 0;
            }
            if msg == WM_GETDLGCODE {
                return DefSubclassProc(hwnd, msg, w, l) | DLGC_WANTARROWS as isize;
            }
        }
        if msg == WM_NCCALCSIZE && l != 0 {
            let rect = &mut *(l as *mut RECT);
            let available = (rect.right - rect.left - 1).max(0);
            let x = inset_x.min(available / 2);
            let y = inset_y.min((rect.bottom - rect.top - 1).max(0) / 2);
            rect.left += x;
            rect.right -= inset_right.min(available - x);
            rect.top += y;
            rect.bottom -= y;
            return 0;
        }
        if msg == WM_NCPAINT {
            let dc = GetWindowDC(hwnd);
            if !dc.is_null() {
                let mut rect: RECT = std::mem::zeroed();
                GetWindowRect(hwnd, &mut rect);
                OffsetRect(&mut rect, -rect.left, -rect.top);
                ExcludeClipRect(
                    dc,
                    inset_x,
                    inset_y,
                    rect.right - inset_right,
                    rect.bottom - inset_y,
                );
                let (backdrop, paper) = if mode & 0x10000 != 0 {
                    (theme::control_surface(), theme::paper())
                } else {
                    crate::settings::preview_edit_colours()
                };
                fill(dc, &rect, backdrop);
                rounded(
                    dc,
                    rect,
                    scaled(12, dpi),
                    paper,
                    if GetFocus() == hwnd {
                        theme::accent()
                    } else {
                        theme::border()
                    },
                );
                if number {
                    draw_arrows(dc, rect, dpi);
                }
                ReleaseDC(hwnd, dc);
            }
            return 0;
        }
        let result = DefSubclassProc(hwnd, msg, w, l);
        if matches!(msg, WM_SETFOCUS | WM_KILLFOCUS | WM_ENABLE) {
            RedrawWindow(
                hwnd,
                std::ptr::null(),
                std::ptr::null_mut(),
                RDW_INVALIDATE | RDW_FRAME,
            );
        }
        result
    }
}

pub fn style_caption_close(hwnd: HWND) {
    unsafe { SetWindowSubclass(hwnd, Some(caption_close_proc), 1, 0) };
}

unsafe extern "system" fn caption_close_proc(
    hwnd: HWND,
    msg: u32,
    w: WPARAM,
    l: LPARAM,
    _: usize,
    _: usize,
) -> LRESULT {
    unsafe {
        if msg == WM_MOUSEMOVE {
            use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
            TrackMouseEvent(&mut TRACKMOUSEEVENT {
                cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                dwFlags: TME_LEAVE,
                hwndTrack: hwnd,
                dwHoverTime: 0,
            });
        }
        let result = DefSubclassProc(hwnd, msg, w, l);
        if matches!(
            msg,
            WM_MOUSEMOVE | WM_MOUSELEAVE | WM_KILLFOCUS | WM_SETFOCUS | WM_UPDATEUISTATE
        ) {
            InvalidateRect(hwnd, std::ptr::null(), 0);
        }
        result
    }
}

pub fn caption_close(item: &DRAWITEMSTRUCT, dpi: i32) {
    let rect = item.rcItem;
    unsafe {
        let active = GetForegroundWindow() == GetParent(item.hwndItem);
        let focused = active
            && item.itemState & ODS_FOCUS != 0
            && SendMessageW(item.hwndItem, WM_QUERYUISTATE, 0, 0) as u32 & UISF_HIDEFOCUS == 0;
        let mut cursor = POINT::default();
        let hovered =
            active && GetCursorPos(&mut cursor) != 0 && WindowFromPoint(cursor) == item.hwndItem;
        fill(
            item.hDC,
            &rect,
            if hovered {
                theme::selection()
            } else {
                theme::chrome()
            },
        );
        if focused {
            let mut bounds = rect;
            InflateRect(&mut bounds, -scaled(4, dpi), -scaled(4, dpi));
            rounded(
                item.hDC,
                bounds,
                scaled(6, dpi),
                if hovered {
                    theme::selection()
                } else {
                    theme::chrome()
                },
                theme::accent(),
            );
        }
        let x = (rect.left + rect.right) / 2;
        let y = (rect.top + rect.bottom) / 2;
        let r = scaled(5, dpi).max(3);
        let pen = CreatePen(PS_SOLID, scaled(1, dpi).max(1), theme::text());
        let old = SelectObject(item.hDC, pen);
        for (from, to) in [(x - r, x + r), (x + r, x - r)] {
            MoveToEx(item.hDC, from, y - r, std::ptr::null_mut());
            LineTo(item.hDC, to, y + r);
        }
        SelectObject(item.hDC, old);
        DeleteObject(pen);
    }
}

pub fn style_slider(hwnd: HWND, dpi: i32) {
    unsafe {
        SetWindowSubclass(hwnd, Some(slider_proc), 1, dpi as usize);
    }
}

unsafe extern "system" fn slider_proc(
    hwnd: HWND,
    msg: u32,
    w: WPARAM,
    l: LPARAM,
    _: usize,
    dpi: usize,
) -> LRESULT {
    unsafe {
        if matches!(msg, WM_PAINT | WM_PRINTCLIENT) {
            let mut paint: PAINTSTRUCT = std::mem::zeroed();
            let dc = if msg == WM_PAINT {
                BeginPaint(hwnd, &mut paint)
            } else {
                w as HDC
            };
            let mut rect: RECT = std::mem::zeroed();
            GetClientRect(hwnd, &mut rect);
            let dpi = dpi as i32;
            fill(dc, &rect, theme::control_surface());
            let enabled =
                windows_sys::Win32::UI::Input::KeyboardAndMouse::IsWindowEnabled(hwnd) != 0;
            let mut thumb: RECT = std::mem::zeroed();
            DefSubclassProc(hwnd, TBM_GETTHUMBRECT, 0, &mut thumb as *mut _ as isize);
            let x = ((thumb.left + thumb.right) / 2).clamp(
                scaled(6, dpi),
                (rect.right - scaled(6, dpi)).max(scaled(6, dpi)),
            );
            let y = rect.bottom / 2;
            let left = scaled(1, dpi);
            let right = (rect.right - left).max(left + 1);
            let width = scaled(8, dpi).max(2);
            let track = RECT {
                left,
                top: y - width / 2,
                right,
                bottom: y + width / 2,
            };
            rounded(
                dc,
                track,
                width,
                theme::paper(),
                if enabled {
                    theme::border()
                } else {
                    theme::divider()
                },
            );
            let ink = if enabled {
                theme::accent()
            } else if theme::high_contrast() {
                theme::muted()
            } else {
                theme::rgb(89, 94, 101)
            };
            if x > left {
                rounded(dc, RECT { right: x, ..track }, width, ink, ink);
            }
            let radius = scaled(7, dpi);
            rounded(
                dc,
                RECT {
                    left: x - radius,
                    top: y - radius,
                    right: x + radius,
                    bottom: y + radius,
                },
                2 * radius,
                ink,
                ink,
            );
            if GetFocus() == hwnd {
                DrawFocusRect(dc, &rect);
            }
            if msg == WM_PAINT {
                EndPaint(hwnd, &paint);
            }
            return 0;
        }
        let result = DefSubclassProc(hwnd, msg, w, l);
        if matches!(
            msg,
            TBM_SETPOS
                | WM_MOUSEMOVE
                | WM_LBUTTONDOWN
                | WM_LBUTTONUP
                | WM_KEYDOWN
                | WM_SETFOCUS
                | WM_KILLFOCUS
                | WM_ENABLE
        ) {
            InvalidateRect(hwnd, std::ptr::null(), 0);
        }
        result
    }
}

unsafe extern "system" fn combo_proc(
    hwnd: HWND,
    msg: u32,
    w: WPARAM,
    l: LPARAM,
    _: usize,
    dpi: usize,
) -> LRESULT {
    unsafe {
        if msg == WM_ERASEBKGND {
            return 1;
        }
        if matches!(msg, WM_PAINT | WM_PRINTCLIENT) {
            let mut paint: PAINTSTRUCT = std::mem::zeroed();
            let dc = if msg == WM_PAINT {
                BeginPaint(hwnd, &mut paint)
            } else {
                w as HDC
            };
            let mut bounds: RECT = std::mem::zeroed();
            GetClientRect(hwnd, &mut bounds);
            let dpi = dpi as i32;
            let enabled =
                windows_sys::Win32::UI::Input::KeyboardAndMouse::IsWindowEnabled(hwnd) != 0;
            let focused = GetFocus() == hwnd;
            fill(dc, &bounds, theme::surface());
            rounded(
                dc,
                bounds,
                scaled(6, dpi),
                theme::paper(),
                if focused {
                    theme::accent()
                } else {
                    theme::border()
                },
            );
            let ink = if enabled {
                theme::text()
            } else {
                theme::muted()
            };
            let font = DefSubclassProc(hwnd, WM_GETFONT, 0, 0) as HFONT;
            let old = SelectObject(dc, font);
            let label = RECT {
                left: scaled(8, dpi),
                right: bounds.right - scaled(25, dpi),
                ..bounds
            };
            line_text(dc, label, &window_text(hwnd), ink, 0);
            SelectObject(dc, old);
            chevron(
                dc,
                POINT {
                    x: bounds.right - scaled(14, dpi),
                    y: bounds.bottom / 2,
                },
                Chevron::Down,
                dpi,
                ink,
            );
            if msg == WM_PAINT {
                EndPaint(hwnd, &paint);
            }
            return 0;
        }
        let result = DefSubclassProc(hwnd, msg, w, l);
        if matches!(
            msg,
            CB_SETCURSEL
                | CB_SHOWDROPDOWN
                | WM_SETFOCUS
                | WM_KILLFOCUS
                | WM_ENABLE
                | WM_KEYDOWN
                | WM_LBUTTONUP
        ) {
            InvalidateRect(hwnd, std::ptr::null(), 0);
        }
        result
    }
}

pub fn slider(item: &NMCUSTOMDRAW) -> LRESULT {
    let dc = item.hdc;
    let mut rect = item.rc;
    if item.dwDrawStage == CDDS_PREPAINT {
        unsafe {
            GetClientRect(item.hdr.hwndFrom, &mut rect);
        }
        fill(dc, &rect, theme::surface());
        return CDRF_NOTIFYITEMDRAW as isize;
    }
    if item.dwDrawStage == CDDS_ITEMPREPAINT {
        match item.dwItemSpec as u32 {
            TBCD_CHANNEL => {
                let middle = (rect.top + rect.bottom) / 2;
                rect.top = middle - 2;
                rect.bottom = middle + 2;
                rounded(dc, rect, 4, theme::selection(), theme::border());
                return CDRF_SKIPDEFAULT as isize;
            }
            TBCD_THUMB => {
                let size = (rect.right - rect.left).min(rect.bottom - rect.top).max(5);
                let x = (rect.left + rect.right - size) / 2;
                let y = (rect.top + rect.bottom - size) / 2;
                let thumb = RECT {
                    left: x,
                    top: y,
                    right: x + size,
                    bottom: y + size,
                };
                let color = if item.uItemState & CDIS_DISABLED == 0 {
                    theme::accent()
                } else {
                    theme::border()
                };
                rounded(dc, thumb, size, color, color);
                return CDRF_SKIPDEFAULT as isize;
            }
            _ => {}
        }
    }
    CDRF_DODEFAULT as isize
}

pub fn switch(hwnd: HWND, dpi: i32, description: bool) {
    let mode = if description { 0x20000 } else { 0 };
    unsafe { SetWindowSubclass(hwnd, Some(switch_proc), 1, mode | dpi as usize) };
}

fn switch_track(bounds: RECT, dpi: i32) -> RECT {
    let height = scaled(20, dpi);
    let right = bounds.right - scaled(9, dpi);
    let top = (bounds.bottom - height) / 2;
    RECT {
        left: right - scaled(35, dpi),
        top,
        right,
        bottom: top + height,
    }
}

unsafe extern "system" fn switch_proc(
    hwnd: HWND,
    msg: u32,
    w: WPARAM,
    l: LPARAM,
    _: usize,
    dpi: usize,
) -> LRESULT {
    unsafe {
        let description = dpi & 0x20000 != 0;
        let dpi = (dpi & 0xffff) as i32;
        if matches!(
            msg,
            WM_NCHITTEST
                | WM_LBUTTONDOWN
                | WM_LBUTTONDBLCLK
                | WM_LBUTTONUP
                | WM_MOUSEMOVE
                | BM_CLICK
        ) {
            let mut bounds = RECT::default();
            GetClientRect(hwnd, &mut bounds);
            let track = switch_track(bounds, dpi);
            if msg == BM_CLICK {
                // Native button automation should click the track, not the row origin.
                let x = (track.left + track.right) / 2;
                let y = (track.top + track.bottom) / 2;
                let center = (x | (y << 16)) as LPARAM;
                DefSubclassProc(hwnd, WM_LBUTTONDOWN, 1, center);
                return DefSubclassProc(hwnd, WM_LBUTTONUP, 0, center);
            }
            let mut point = POINT {
                x: l as i16 as i32,
                y: (l >> 16) as i16 as i32,
            };
            if msg == WM_NCHITTEST {
                ScreenToClient(hwnd, &mut point);
            }
            let inside = (track.left..track.right).contains(&point.x)
                && (track.top..track.bottom).contains(&point.y);
            if !inside {
                return match msg {
                    WM_NCHITTEST => HTTRANSPARENT as LRESULT,
                    WM_LBUTTONUP => DefSubclassProc(hwnd, WM_CANCELMODE, 0, 0),
                    WM_MOUSEMOVE => DefSubclassProc(hwnd, msg, w, -1),
                    _ => 0,
                };
            }
        }
        if matches!(msg, WM_PAINT | WM_PRINTCLIENT) {
            let mut paint: PAINTSTRUCT = std::mem::zeroed();
            let dc = if msg == WM_PAINT {
                BeginPaint(hwnd, &mut paint)
            } else {
                w as HDC
            };
            let mut bounds: RECT = std::mem::zeroed();
            GetClientRect(hwnd, &mut bounds);
            let px = |n| scaled(n, dpi);
            let checked = DefSubclassProc(hwnd, BM_GETCHECK, 0, 0) == BST_CHECKED as isize;
            let enabled =
                windows_sys::Win32::UI::Input::KeyboardAndMouse::IsWindowEnabled(hwnd) != 0;
            let focused = GetFocus() == hwnd;
            let track = switch_track(bounds, dpi);
            let top = track.top;
            let h = track.bottom - track.top;
            fill(dc, &bounds, theme::control_surface());
            if focused && DefSubclassProc(hwnd, WM_QUERYUISTATE, 0, 0) as u32 & UISF_HIDEFOCUS == 0
            {
                let mut outline = track;
                InflateRect(&mut outline, px(5), px(5));
                rounded(dc, outline, px(30), theme::accent(), theme::accent());
                InflateRect(&mut outline, -px(2), -px(2));
                rounded(
                    dc,
                    outline,
                    px(26),
                    theme::control_surface(),
                    theme::control_surface(),
                );
            }
            rounded(
                dc,
                track,
                h,
                if checked && enabled {
                    theme::accent()
                } else {
                    theme::paper()
                },
                if focused || (checked && enabled) {
                    theme::accent()
                } else {
                    theme::border()
                },
            );
            let diameter = px(12);
            let inset = (h - diameter) / 2;
            let side = px(3);
            let left = if checked {
                track.right - side - diameter
            } else {
                track.left + side
            };
            let knob = RECT {
                left,
                top: top + inset,
                right: left + diameter,
                bottom: top + inset + diameter,
            };
            let ink = if checked && enabled {
                theme::on_accent()
            } else {
                theme::muted()
            };
            rounded(dc, knob, diameter, ink, ink);
            let font = DefSubclassProc(hwnd, WM_GETFONT, 0, 0) as HFONT;
            let old = SelectObject(dc, font);
            bounds.left += px(9);
            bounds.right = track.left - px(7);
            if description {
                bounds.bottom = px(30);
            }
            line_text(
                dc,
                bounds,
                &window_text(hwnd),
                if enabled {
                    theme::text()
                } else {
                    theme::muted()
                },
                0,
            );
            if description && let Some((caption, font)) = crate::settings::switch_description(hwnd)
            {
                SelectObject(dc, font);
                let detail = RECT {
                    top: px(27),
                    bottom: px(48),
                    ..bounds
                };
                line_text(
                    dc,
                    detail,
                    &crate::localization::tr(caption),
                    theme::muted(),
                    0,
                );
            }
            SelectObject(dc, old);
            if msg == WM_PAINT {
                EndPaint(hwnd, &paint);
            }
            return 0;
        }
        let result = DefSubclassProc(hwnd, msg, w, l);
        if matches!(
            msg,
            BM_SETCHECK | BM_SETSTATE | WM_SETFOCUS | WM_KILLFOCUS | WM_ENABLE
        ) {
            InvalidateRect(hwnd, std::ptr::null(), 0);
        }
        result
    }
}
