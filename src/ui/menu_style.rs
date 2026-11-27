// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

//! USER32 tracks the real HMENU, including its keyboard, accessibility and cascade behavior.
//! A scoped subclass frames the native popup windows belonging to this tracking call.
use crate::{
    theme,
    ui::{self, scaled, wide},
};
use std::{cell::Cell, collections::HashMap, mem::size_of, ptr::null_mut};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    UI::{
        Accessibility::{MSAA_MENU_SIG, MSAAMENUINFO},
        Controls::*,
        WindowsAndMessaging::*,
    },
};

pub(crate) enum Anchor {
    Pointer(POINT),
    Below(RECT),
    Above(RECT),
}

#[derive(Default)]
struct Columns {
    label: i32,
    shortcut: i32,
    cascade: bool,
}

struct Item {
    // USER32 exposes this allocation to MSAA through dwItemData. Both allocations
    // must remain stable until DestroyMenu, including during nested modal loops.
    _access: Box<MSAAMENUINFO>,
    _name: Vec<u16>,
    menu: HMENU,
    label: String,
    shortcut: String,
    child: bool,
    rule: bool,
}

pub(crate) struct PopupStyle {
    root: HMENU,
    background: HBRUSH,
    font: HFONT,
    dc: HDC,
    old_font: HGDIOBJ,
    dpi: i32,
    columns: HashMap<HMENU, Columns>,
    items: HashMap<usize, Item>,
    max_width: Cell<i32>,
}

impl PopupStyle {
    pub(crate) fn new(dpi: i32) -> Option<Self> {
        unsafe {
            let root = CreatePopupMenu();
            if root.is_null() {
                return None;
            }
            let font = ui::make_font(15, 400, "Microsoft YaHei UI", dpi);
            let dc = CreateCompatibleDC(null_mut());
            let old_font = SelectObject(dc, font);
            Some(Self {
                root,
                dpi,
                font,
                dc,
                old_font,
                background: CreateSolidBrush(theme::control_surface()),
                columns: HashMap::new(),
                items: HashMap::new(),
                max_width: Cell::new(i32::MAX),
            })
        }
    }

    pub(crate) fn root(&self) -> HMENU {
        self.root
    }

    pub(crate) fn add(&mut self, menu: HMENU, id: usize, key: &str, checked: bool, disabled: bool) {
        let state =
            (if checked { MFS_CHECKED } else { 0 }) | (if disabled { MFS_DISABLED } else { 0 });
        self.insert(
            menu,
            id,
            &crate::localization::tr(key),
            state,
            null_mut(),
            false,
        );
    }

    pub(crate) fn raw(&mut self, id: usize, value: &str) {
        self.insert(self.root, id, value, 0, null_mut(), true);
    }

    pub(crate) fn separator(&mut self, menu: HMENU) {
        self.insert(menu, 0, "", MFS_DISABLED, null_mut(), false);
    }

    pub(crate) fn submenu(&mut self, key: &str) -> Option<HMENU> {
        unsafe {
            let child = CreatePopupMenu();
            if child.is_null() {
                return None;
            }
            if self.insert(self.root, 0, &crate::localization::tr(key), 0, child, false) {
                Some(child)
            } else {
                DestroyMenu(child);
                None
            }
        }
    }

    fn text_width(&self, value: &str) -> i32 {
        let value = wide(value);
        let mut size = SIZE::default();
        unsafe {
            GetTextExtentPoint32W(self.dc, value.as_ptr(), (value.len() - 1) as i32, &mut size)
        };
        size.cx
    }

    fn insert(
        &mut self,
        menu: HMENU,
        id: usize,
        name: &str,
        state: u32,
        child: HMENU,
        raw: bool,
    ) -> bool {
        let rule = !raw && id == 0 && child.is_null();
        let (label, shortcut) = if raw {
            (name, "")
        } else {
            name.split_once('\t').unwrap_or((name, ""))
        };
        let label = label.replace('\t', "    ");
        let mut name = wide(name);
        let mut access = Box::new(MSAAMENUINFO {
            dwMSAASignature: MSAA_MENU_SIG as u32,
            cchWText: (name.len() - 1) as u32,
            pszWText: name.as_mut_ptr(),
        });
        let data = (&mut *access as *mut MSAAMENUINFO) as usize;
        let info = MENUITEMINFOW {
            cbSize: size_of::<MENUITEMINFOW>() as u32,
            fMask: MIIM_FTYPE
                | MIIM_DATA
                | MIIM_STATE
                | MIIM_ID
                | MIIM_STRING
                | if child.is_null() { 0 } else { MIIM_SUBMENU },
            fType: MFT_OWNERDRAW | if rule { MFT_SEPARATOR } else { 0 },
            fState: state,
            wID: id as u32,
            hSubMenu: child,
            dwItemData: data,
            dwTypeData: name.as_mut_ptr(),
            ..Default::default()
        };
        if unsafe { InsertMenuItemW(menu, u32::MAX, 1, &info) } == 0 {
            return false;
        }
        let label_width = self.text_width(&label);
        let shortcut_width = self.text_width(shortcut);
        let columns = self.columns.entry(menu).or_default();
        columns.label = columns.label.max(label_width);
        columns.shortcut = columns.shortcut.max(shortcut_width);
        columns.cascade |= !child.is_null();
        self.items.insert(
            data,
            Item {
                _access: access,
                _name: name,
                menu,
                label,
                shortcut: shortcut.to_owned(),
                child: !child.is_null(),
                rule,
            },
        );
        true
    }
}

impl Drop for PopupStyle {
    fn drop(&mut self) {
        unsafe {
            DestroyMenu(self.root);
            SelectObject(self.dc, self.old_font);
            DeleteDC(self.dc);
            DeleteObject(self.font);
            DeleteObject(self.background);
        }
    }
}

mod painting;
#[cfg(test)]
mod tests;
mod tracking;
