//! Win32 owns popup placement, keyboard navigation and cascading. The manager
//! supplies the item drawing so the selected ALTRun theme applies to both levels.
use crate::{
    native::{scaled, wide},
    theme, theme_ui,
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
    heading: bool,
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
        let heading = id == 0 && child.is_null() && !rule;
        let label = crate::localization::tr(label);
        let mut text = wide(&label);
        let data = CURRENT.with(|current| {
            let mut state = current.borrow_mut();
            let skin = state.as_mut().unwrap();
            skin.items.push(Item {
                label,
                width,
                height: if rule {
                    7
                } else if heading {
                    22
                } else {
                    29
                },
                child: !child.is_null(),
                heading,
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
        theme_ui::fill(dc, &rect, surface);
        if item.rule {
            let top = (rect.top + rect.bottom) / 2;
            theme_ui::fill(
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
            theme_ui::rounded(dc, hover, px(8), theme::selection(), theme::selection());
        }
        let ink = if disabled || item.heading {
            theme::muted()
        } else {
            theme::text()
        };
        let body = RECT {
            left: rect.left + px(25),
            right: rect.right - px(14),
            ..rect
        };
        unsafe {
            let font = SelectObject(dc, skin.font);
            theme_ui::line_text(dc, body, &item.label, ink, 0);
            if info.itemState & ODS_CHECKED != 0 {
                let tick = RECT {
                    left: rect.left + px(5),
                    right: body.left,
                    ..rect
                };
                theme_ui::line_text(dc, tick, "✓", theme::accent(), DT_CENTER);
            }
            if item.child {
                // Win32 paints its cascade arrow in the right gutter after
                // WM_DRAWITEM. Keep the theme stroke in that same footprint.
                let x = rect.right - px(6);
                let y = (rect.top + rect.bottom) / 2;
                let half = px(3).max(2);
                let pen = CreatePen(PS_SOLID, px(1).max(1), ink);
                let old_pen = SelectObject(dc, pen);
                MoveToEx(dc, x, y - half, std::ptr::null_mut());
                LineTo(dc, x + half, y);
                LineTo(dc, x, y + half);
                SelectObject(dc, old_pen);
                DeleteObject(pen);
            }
            SelectObject(dc, font);
        }
        true
    })
}
