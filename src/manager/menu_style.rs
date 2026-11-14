// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

//! Win32 owns popup placement, keyboard navigation and cascading. The manager
//! supplies the item drawing so the selected ALTRun theme applies to both levels.
use crate::{
    theme,
    ui::{self, scaled, wide},
};
use std::cell::RefCell;
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    UI::{Controls::*, WindowsAndMessaging::*},
};

struct Item {
    label: String,
    width: i32,
    height: i32,
    child: bool,
    rule: bool,
}

struct Appearance {
    dpi: i32,
    font: HFONT,
    items: Vec<Item>,
}

thread_local! {
    static CURRENT: RefCell<Option<Appearance>> = const { RefCell::new(None) };
}

pub struct PopupStyle(HBRUSH);

impl PopupStyle {
    pub fn new(dpi: i32, font: HFONT) -> Self {
        CURRENT.with(|current| {
            *current.borrow_mut() = Some(Appearance {
                dpi,
                font,
                items: Vec::new(),
            })
        });
        Self(unsafe { CreateSolidBrush(theme::control_surface()) })
    }

    pub fn background(&self, menu: HMENU) {
        unsafe {
            SetMenuInfo(
                menu,
                &MENUINFO {
                    cbSize: std::mem::size_of::<MENUINFO>() as u32,
                    fMask: MIM_BACKGROUND | MIM_APPLYTOSUBMENUS,
                    hbrBack: self.0,
                    ..Default::default()
                },
            );
        }
    }

    pub fn add(
        &self,
        menu: HMENU,
        id: usize,
        label: &str,
        width: i32,
        checked: bool,
        disabled: bool,
    ) {
        let state =
            (if checked { MFS_CHECKED } else { 0 }) | (if disabled { MFS_DISABLED } else { 0 });
        self.insert(menu, id, label, width, state, std::ptr::null_mut());
    }

    pub fn submenu(&self, menu: HMENU, child: HMENU, label: &str) {
        self.insert(menu, 0, label, 183, 0, child);
    }

    fn insert(&self, menu: HMENU, id: usize, label: &str, width: i32, state: u32, child: HMENU) {
        let rule = label.is_empty();
        let label = crate::localization::tr(label);
        let mut text = wide(&label);
        let data = CURRENT.with(|current| {
            let mut state = current.borrow_mut();
            let skin = state.as_mut().unwrap();
            skin.items.push(Item {
                label,
                width,
                height: if rule { 7 } else { 29 },
                child: !child.is_null(),
                rule,
            });
            skin.items.len()
        });
        let info = MENUITEMINFOW {
            cbSize: std::mem::size_of::<MENUITEMINFOW>() as u32,
            fMask: MIIM_FTYPE
                | MIIM_DATA
                | MIIM_STATE
                | MIIM_ID
                | MIIM_STRING
                | if child.is_null() { 0 } else { MIIM_SUBMENU },
            fType: MFT_OWNERDRAW,
            fState: state | if rule { MFS_DISABLED } else { 0 },
            wID: id as u32,
            hSubMenu: child,
            dwItemData: data,
            dwTypeData: text.as_mut_ptr(),
            ..Default::default()
        };
        unsafe {
            InsertMenuItemW(menu, GetMenuItemCount(menu).max(0) as u32, 1, &info);
        }
    }
}

impl Drop for PopupStyle {
    fn drop(&mut self) {
        CURRENT.with(|current| *current.borrow_mut() = None);
        unsafe {
            DeleteObject(self.0);
        }
    }
}

pub fn measure(info: &mut MEASUREITEMSTRUCT) -> bool {
    if info.CtlType != ODT_MENU {
        return false;
    }
    CURRENT.with(|current| {
        let state = current.borrow();
        let Some(skin) = state.as_ref() else {
            return false;
        };
        let Some(item) = skin.items.get(info.itemData.wrapping_sub(1)) else {
            return false;
        };
        info.itemWidth = scaled(item.width, skin.dpi) as u32;
        info.itemHeight = scaled(item.height, skin.dpi) as u32;
        true
    })
}

pub fn draw(info: &DRAWITEMSTRUCT) -> bool {
    if info.CtlType != ODT_MENU {
        return false;
    }
    CURRENT.with(|current| {
        let state = current.borrow();
        let Some(skin) = state.as_ref() else {
            return false;
        };
        let Some(item) = skin.items.get(info.itemData.wrapping_sub(1)) else {
            return false;
        };
        let px = |value| scaled(value, skin.dpi);
        let rect = info.rcItem;
        let dc = info.hDC;
        let surface = theme::control_surface();
        ui::fill(dc, &rect, surface);
        if item.rule {
            let top = (rect.top + rect.bottom) / 2;
            ui::fill(
                dc,
                &RECT {
                    left: rect.left + px(6),
                    right: rect.right - px(6),
                    top,
                    bottom: top + 1,
                },
                theme::divider(),
            );
            return true;
        }
        let disabled = info.itemState & (ODS_GRAYED | ODS_DISABLED) != 0;
        if !disabled && info.itemState & ODS_SELECTED != 0 {
            let hover = RECT {
                left: rect.left + px(3),
                right: rect.right - px(3),
                ..rect
            };
            ui::rounded(dc, hover, px(8), theme::selection(), theme::selection());
        }
        let ink = if disabled {
            theme::muted()
        } else {
            theme::text()
        };
        let body = RECT {
            left: rect.left + px(25),
            right: rect.right - px(if item.child { 26 } else { 14 }),
            ..rect
        };
        unsafe {
            let font = SelectObject(dc, skin.font);
            ui::line_text(dc, body, &item.label, ink, 0);
            if info.itemState & ODS_CHECKED != 0 {
                let tick = RECT {
                    left: rect.left + px(5),
                    right: body.left,
                    ..rect
                };
                ui::line_text(dc, tick, "✓", theme::accent(), DT_CENTER);
            }
            if item.child {
                ui::chevron(
                    dc,
                    POINT {
                        x: rect.right - px(14),
                        y: (rect.top + rect.bottom) / 2,
                    },
                    ui::Chevron::Right,
                    skin.dpi,
                    ink,
                );
            }
            SelectObject(dc, font);
            if item.child {
                // USER32 draws an additional cascade glyph after this callback.
                // Exclude only this row's indicator gutter from that draw.
                ExcludeClipRect(
                    dc,
                    rect.right - px(23),
                    rect.top,
                    rect.right + px(12),
                    rect.bottom,
                );
            }
        }
        true
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ptr::null_mut;

    #[test]
    fn cascade_arrow_is_a_single_shape_inside_each_scaled_row() {
        for dpi in [96, 120, 144, 168, 192, 240] {
            unsafe {
                let dc = CreateCompatibleDC(null_mut());
                assert!(!dc.is_null());
                let bitmap_info = BITMAPINFO {
                    bmiHeader: BITMAPINFOHEADER {
                        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                        biWidth: 540,
                        biHeight: -100,
                        biPlanes: 1,
                        biBitCount: 32,
                        ..Default::default()
                    },
                    ..Default::default()
                };
                let mut pixels = null_mut();
                let bitmap =
                    CreateDIBSection(dc, &bitmap_info, DIB_RGB_COLORS, &mut pixels, null_mut(), 0);
                assert!(!bitmap.is_null());
                let previous = SelectObject(dc, bitmap);
                let style = PopupStyle::new(dpi, GetStockObject(DEFAULT_GUI_FONT));
                let menu = CreatePopupMenu();
                style.submenu(menu, CreatePopupMenu(), "Sort");
                let rect = RECT {
                    left: 0,
                    top: 0,
                    right: scaled(195, dpi),
                    bottom: scaled(29, dpi),
                };
                let center = POINT {
                    x: rect.right - scaled(14, dpi),
                    y: rect.bottom / 2,
                };
                assert!(draw(&DRAWITEMSTRUCT {
                    CtlType: ODT_MENU,
                    hDC: dc,
                    rcItem: rect,
                    itemData: 1,
                    ..Default::default()
                }));
                assert_eq!(PtVisible(dc, rect.right - scaled(5, dpi), center.y), 0);
                assert_ne!(
                    PtVisible(dc, rect.right - scaled(5, dpi), rect.bottom + 1),
                    0
                );
                assert_ne!(PtVisible(dc, scaled(30, dpi), center.y), 0);
                SelectClipRgn(dc, null_mut());
                let radius = scaled(4, dpi);
                let mut ink_rows = 0;
                for y in center.y - radius..center.y + radius {
                    let ink = (center.x - radius..=center.x + radius)
                        .filter(|x| GetPixel(dc, *x, y) == theme::text())
                        .collect::<Vec<_>>();
                    if !ink.is_empty() {
                        ink_rows += 1;
                        assert!(
                            ink.windows(2).all(|pair| pair[1] == pair[0] + 1),
                            "{dpi} DPI"
                        );
                    }
                }
                assert!(ink_rows >= scaled(6, dpi), "{dpi} DPI");
                assert_eq!(
                    GetPixel(dc, rect.right - scaled(5, dpi), center.y),
                    theme::control_surface()
                );
                DestroyMenu(menu);
                SelectObject(dc, previous);
                DeleteObject(bitmap);
                DeleteDC(dc);
            }
        }
    }
}
