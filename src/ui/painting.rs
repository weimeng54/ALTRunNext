// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::{
    geometry::{BoxRect, CAPTION},
    scaled, wide,
};
use crate::theme;
use windows_sys::Win32::{
    Foundation::*, Graphics::Gdi::*, System::LibraryLoader::GetModuleHandleW,
    UI::WindowsAndMessaging::*,
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
    draw_app_icon(dc, BoxRect::new(16, 12, 18, 18).pixels(dpi));
    unsafe {
        let old = SelectObject(dc, font);
        line_text(
            dc,
            RECT {
                left: scaled(44, dpi),
                top: 0,
                right: scaled(334, dpi),
                bottom: scaled(CAPTION, dpi),
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
