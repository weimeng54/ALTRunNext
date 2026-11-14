// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

mod model;
mod paint;

use super::{Accepted, Preview};
use crate::{localization as l10n, manager::table, theme, ui};
use altrun_probe::browser_bookmarks::BookmarkStatus;
use model::{Model, check_mark};
use paint::{draw_mode, draw_row, paint};
use std::{
    cell::{Cell, RefCell},
    ptr::{null, null_mut},
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::LibraryLoader::GetModuleHandleW,
    UI::{
        Controls::*,
        Input::KeyboardAndMouse::*,
        Shell::{DefSubclassProc, SetWindowSubclass},
        WindowsAndMessaging::*,
    },
};

const WIDTH: i32 = 928;
const HEIGHT: i32 = 523;
const HEADER: i32 = 42;
const FOOTER: i32 = 58;
const TABLE: usize = 2901;
const APPLY: usize = 2902;
const CANCEL: usize = 2903;
const CAPTION_CLOSE: usize = 2904;
const APPEND: usize = 2905;
const REPLACE: usize = 2906;
const FILTER: usize = 2907;
const ALL: usize = 2908;
const INVERT: usize = 2909;
const CLEAR: usize = 2910;
const ALIAS: usize = 2911;
const STATUS: usize = 2912;
const ERROR: usize = 2913;
const TABLE_TOP: i32 = 115;
const ROW_HEIGHT: i32 = 30;
const BOOKMARK_ROW_HEIGHT: i32 = 38;
const ALIAS_COLUMN: usize = 2;
const STATIC_NOPREFIX: u32 = 0x80;
const STATIC_PATHELLIPSIS: u32 = 0x4000;

const IMPORT_COLUMNS: &[(&str, i32)] = &[
    ("ui.alias", 146),
    ("ui.name", 206),
    ("ui.command_label", 344),
    ("ui.browser_bookmarks_status", 177),
];
const BOOKMARK_COLUMNS: &[(&str, i32)] = &[
    ("ui.browser_bookmarks_checkbox", 66),
    ("ui.name", 145),
    ("ui.alias", 125),
    ("ui.browser_bookmarks_folder", 124),
    ("ui.browser_bookmarks_url", 248),
    ("ui.browser_bookmarks_status", 165),
];

pub(super) enum Outcome {
    Accepted(Accepted),
    Retry,
    Cancelled,
}

struct Dialog {
    model: RefCell<Model>,
    composing: Cell<HWND>,
    dpi: i32,
    height: i32,
    row_height: i32,
    caption: HFONT,
    font: HFONT,
    surface: HBRUSH,
    paper: HBRUSH,
    title: String,
}

unsafe fn child(hwnd: HWND, id: usize) -> HWND {
    unsafe { GetDlgItem(hwnd, id as i32) }
}

fn clear_error(hwnd: HWND) {
    unsafe { SetWindowTextW(child(hwnd, ERROR), ui::wide("").as_ptr()) };
}

fn search_colours() -> (u32, u32) {
    (theme::surface(), theme::paper())
}

fn alias_colours() -> (u32, u32) {
    (theme::selection(), theme::paper())
}

fn finish_alias(hwnd: HWND, shared: &Dialog) {
    if shared.model.borrow_mut().editing_alias.take().is_some() {
        unsafe {
            ShowWindow(child(hwnd, ALIAS), SW_HIDE);
            InvalidateRect(child(hwnd, TABLE), null(), 0);
        }
    }
}

unsafe fn alias_bounds(list: HWND, row: usize, dpi: i32) -> Option<RECT> {
    unsafe {
        let mut bounds = RECT {
            top: ALIAS_COLUMN as i32,
            left: LVIR_BOUNDS as i32,
            ..Default::default()
        };
        if SendMessageW(
            list,
            LVM_GETSUBITEMRECT,
            row,
            &mut bounds as *mut _ as LPARAM,
        ) == 0
        {
            return None;
        }
        InflateRect(&mut bounds, -ui::scaled(4, dpi), -ui::scaled(3, dpi));
        (bounds.right > bounds.left && bounds.bottom > bounds.top).then_some(bounds)
    }
}

fn begin_alias(hwnd: HWND, shared: &Dialog, position: usize) {
    let alias = {
        let model = shared.model.borrow();
        let (Some(&index), Some(Preview::Bookmarks { rows, .. })) =
            (model.shown.get(position), model.preview.as_ref())
        else {
            return;
        };
        if rows[index].status != BookmarkStatus::Ready {
            return;
        }
        (index, rows[index].alias.clone())
    };
    unsafe {
        let list = child(hwnd, TABLE);
        let edit = child(hwnd, ALIAS);
        let Some(mut bounds) = alias_bounds(list, position, shared.dpi) else {
            return;
        };
        let mut visible = RECT::default();
        GetClientRect(list, &mut visible);
        let header = SendMessageW(list, LVM_GETHEADER, 0, 0) as HWND;
        let mut header_bounds = RECT::default();
        GetClientRect(header, &mut header_bounds);
        visible.top = header_bounds.bottom;
        let mut clipped = RECT::default();
        if IntersectRect(&mut clipped, &visible, &bounds) == 0 {
            return;
        }
        bounds = clipped;
        MapWindowPoints(list, hwnd, (&raw mut bounds).cast::<POINT>(), 2);
        finish_alias(hwnd, shared);
        SetWindowTextW(edit, ui::wide(&alias.1).as_ptr());
        shared.model.borrow_mut().editing_alias = Some(alias.0);
        SetWindowPos(
            edit,
            HWND_TOP,
            bounds.left,
            bounds.top,
            bounds.right - bounds.left,
            bounds.bottom - bounds.top,
            SWP_SHOWWINDOW,
        );
        SetFocus(edit);
        SendMessageW(edit, EM_SETSEL, 0, -1);
    }
}

fn refresh(hwnd: HWND, shared: &Dialog) {
    finish_alias(hwnd, shared);
    let (rows, selected) = {
        let mut model = shared.model.borrow_mut();
        let rows = model.visible_rows();
        let selected = model
            .selected
            .and_then(|selected| model.shown.iter().position(|&index| index == selected));
        model.updating = true;
        (rows, selected)
    };
    unsafe {
        table::replace_report_rows(
            child(hwnd, TABLE),
            &rows,
            selected,
            ui::scaled(shared.row_height, shared.dpi),
        );
    }
    shared.model.borrow_mut().updating = false;
    refresh_status(hwnd, shared);
}

fn refresh_status(hwnd: HWND, shared: &Dialog) {
    let (summary, action, enabled) = shared.model.borrow().status();
    unsafe {
        SetWindowTextW(child(hwnd, STATUS), ui::wide(&summary).as_ptr());
        SetWindowTextW(child(hwnd, APPLY), ui::wide(&action).as_ptr());
        EnableWindow(child(hwnd, APPLY), i32::from(enabled));
    }
}

fn toggle(hwnd: HWND, shared: &Dialog, selection: usize) {
    let toggle = {
        let mut model = shared.model.borrow_mut();
        let Some(&index) = model.shown.get(selection) else {
            return;
        };
        let Some(Preview::Bookmarks { rows, .. }) = model.preview.as_mut() else {
            return;
        };
        let row = &mut rows[index];
        if row.status != BookmarkStatus::Ready {
            return;
        }
        row.checked = !row.checked;
        row.checked
    };
    let mut text = ui::wide(check_mark(BookmarkStatus::Ready, toggle));
    unsafe {
        let list = child(hwnd, TABLE);
        let item = LVITEMW {
            pszText: text.as_mut_ptr(),
            ..Default::default()
        };
        SendMessageW(
            list,
            LVM_SETITEMTEXTW,
            selection,
            &item as *const _ as LPARAM,
        );
        InvalidateRect(list, null(), 0);
    }
    clear_error(hwnd);
    refresh_status(hwnd, shared);
}

fn handle_command(hwnd: HWND, shared: &Dialog, id: usize, code: u32, control: HWND) {
    match id {
        APPLY => {
            let accepted = {
                let model = shared.model.borrow();
                if model.preview.is_none() {
                    drop(model);
                    shared.model.borrow_mut().outcome = Outcome::Retry;
                    unsafe { DestroyWindow(hwnd) };
                    return;
                }
                model.accept()
            };
            match accepted {
                Ok(Some(result)) => {
                    shared.model.borrow_mut().outcome = Outcome::Accepted(result);
                    unsafe { DestroyWindow(hwnd) };
                }
                Ok(None) => {}
                Err(error) => unsafe {
                    SetWindowTextW(child(hwnd, ERROR), ui::ui(&error).as_ptr());
                },
            }
        }
        CANCEL | CAPTION_CLOSE => unsafe {
            DestroyWindow(hwnd);
        },
        APPEND | REPLACE => {
            if let Some(Preview::LaunchItems { replace, .. }) =
                shared.model.borrow_mut().preview.as_mut()
            {
                *replace = id == REPLACE;
            }
            unsafe {
                for control in [APPEND, REPLACE] {
                    InvalidateRect(child(hwnd, control), null(), 0);
                }
            }
            refresh(hwnd, shared);
        }
        ALL | INVERT | CLEAR => {
            if let Some(Preview::Bookmarks { rows, .. }) =
                shared.model.borrow_mut().preview.as_mut()
            {
                for row in rows
                    .iter_mut()
                    .filter(|row| row.status == BookmarkStatus::Ready)
                {
                    row.checked = match id {
                        ALL => true,
                        CLEAR => false,
                        _ => !row.checked,
                    };
                }
            }
            clear_error(hwnd);
            refresh(hwnd, shared);
        }
        FILTER if code == EN_CHANGE => {
            shared.model.borrow_mut().query = ui::window_text(control).trim().to_lowercase();
            refresh(hwnd, shared);
        }
        ALIAS if code == EN_CHANGE => {
            let value = ui::window_text(control);
            let changed = {
                let mut model = shared.model.borrow_mut();
                if model.updating {
                    return;
                }
                let Some(index) = model.editing_alias else {
                    return;
                };
                let Some(column) = model.shown.iter().position(|&row| row == index) else {
                    return;
                };
                let Some(Preview::Bookmarks { rows, .. }) = model.preview.as_mut() else {
                    return;
                };
                if rows[index].status != BookmarkStatus::Ready {
                    return;
                }
                rows[index].alias = value.clone();
                column
            };
            let mut value = ui::wide(&value);
            let item = LVITEMW {
                iSubItem: ALIAS_COLUMN as i32,
                pszText: value.as_mut_ptr(),
                ..Default::default()
            };
            unsafe {
                let list = child(hwnd, TABLE);
                SendMessageW(list, LVM_SETITEMTEXTW, changed, &item as *const _ as LPARAM);
                InvalidateRect(list, null(), 0);
            }
            clear_error(hwnd);
        }
        ALIAS if code == EN_KILLFOCUS => finish_alias(hwnd, shared),
        _ => {}
    }
}

unsafe fn notify(hwnd: HWND, shared: &Dialog, l: LPARAM) {
    unsafe {
        let header = &*(l as *const NMHDR);
        if header.idFrom != TABLE || shared.model.borrow().updating {
            return;
        }
        match header.code {
            LVN_ITEMCHANGED => {
                let change = &*(l as *const NMLISTVIEW);
                if change.uNewState & LVIS_SELECTED != 0 && change.uOldState & LVIS_SELECTED == 0 {
                    let mut model = shared.model.borrow_mut();
                    model.selected = model.shown.get(change.iItem as usize).copied();
                }
            }
            NM_CLICK => {
                let item = &*(l as *const NMITEMACTIVATE);
                if item.iItem < 0 {
                    return;
                }
                if item.iSubItem == 0 {
                    toggle(hwnd, shared, item.iItem as usize);
                } else if item.iSubItem == ALIAS_COLUMN as i32 {
                    begin_alias(hwnd, shared, item.iItem as usize);
                }
            }
            LVN_KEYDOWN => {
                let key = &*(l as *const NMLVKEYDOWN);
                if matches!(key.wVKey, VK_SPACE | VK_F2) {
                    let index = SendMessageW(
                        header.hwndFrom,
                        LVM_GETNEXTITEM,
                        usize::MAX,
                        LVNI_SELECTED as LPARAM,
                    );
                    if index >= 0 {
                        if key.wVKey == VK_SPACE {
                            toggle(hwnd, shared, index as usize);
                        } else {
                            begin_alias(hwnd, shared, index as usize);
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

unsafe extern "system" fn procedure(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    unsafe {
        if msg == WM_NCCREATE {
            let params = (*(l as *const CREATESTRUCTW)).lpCreateParams;
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, params as isize);
        }
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const Dialog;
        if let Some(dialog) = ptr.as_ref() {
            match msg {
                WM_PAINT => {
                    let mut paint_data = PAINTSTRUCT::default();
                    let dc = BeginPaint(hwnd, &mut paint_data);
                    paint(hwnd, dialog, dc);
                    EndPaint(hwnd, &paint_data);
                    return 0;
                }
                WM_ERASEBKGND => return 1,
                WM_PRINTCLIENT => {
                    paint(hwnd, dialog, w as HDC);
                    return 0;
                }
                WM_NCHITTEST => {
                    let mut point = POINT {
                        x: l as i16 as i32,
                        y: (l >> 16) as i16 as i32,
                    };
                    ScreenToClient(hwnd, &mut point);
                    if point.y < ui::scaled(HEADER, dialog.dpi)
                        && point.x < ui::scaled(WIDTH - 47, dialog.dpi)
                    {
                        return HTCAPTION as isize;
                    }
                }
                WM_CTLCOLORSTATIC | WM_CTLCOLOREDIT | WM_CTLCOLORBTN => {
                    let id = GetDlgCtrlID(l as HWND) as usize;
                    let edit = msg == WM_CTLCOLOREDIT || id == ALIAS || id == FILTER;
                    let background = if edit {
                        theme::paper()
                    } else if id == STATUS {
                        theme::chrome()
                    } else {
                        theme::surface()
                    };
                    SetBkColor(w as HDC, background);
                    SetTextColor(
                        w as HDC,
                        if id == ERROR {
                            theme::error()
                        } else if id == STATUS {
                            theme::muted()
                        } else {
                            theme::text()
                        },
                    );
                    if id == STATUS {
                        SetDCBrushColor(w as HDC, background);
                        return GetStockObject(DC_BRUSH) as isize;
                    }
                    if edit {
                        return dialog.paper as isize;
                    }
                    return dialog.surface as isize;
                }
                WM_DRAWITEM if l != 0 => {
                    let item = &*(l as *const DRAWITEMSTRUCT);
                    match item.CtlID as usize {
                        TABLE => draw_row(item, dialog),
                        CAPTION_CLOSE => ui::caption_close(item, dialog.dpi),
                        APPEND | REPLACE => draw_mode(item, dialog),
                        APPLY | CANCEL => ui::button_on(
                            item,
                            item.CtlID as usize == APPLY,
                            theme::chrome(),
                            dialog.dpi,
                        ),
                        _ => ui::button_on(item, false, theme::surface(), dialog.dpi),
                    }
                    return 1;
                }
                WM_MEASUREITEM if w == TABLE && l != 0 => {
                    (*(l as *mut MEASUREITEMSTRUCT)).itemHeight =
                        ui::scaled(dialog.row_height, dialog.dpi) as u32;
                    return 1;
                }
                WM_COMMAND => {
                    handle_command(hwnd, dialog, w & 65535, (w >> 16) as u32, l as HWND);
                    return 0;
                }
                WM_NOTIFY if l != 0 => notify(hwnd, dialog, l),
                WM_CLOSE => {
                    DestroyWindow(hwnd);
                    return 0;
                }
                _ => {}
            }
        }
        DefWindowProcW(hwnd, msg, w, l)
    }
}

unsafe extern "system" fn edit_keys(
    hwnd: HWND,
    msg: u32,
    w: WPARAM,
    l: LPARAM,
    _: usize,
    data: usize,
) -> LRESULT {
    unsafe {
        let dialog = &*(data as *const Dialog);
        match msg {
            WM_IME_STARTCOMPOSITION => dialog.composing.set(hwnd),
            WM_IME_ENDCOMPOSITION | WM_KILLFOCUS | WM_NCDESTROY
                if dialog.composing.get() == hwnd =>
            {
                dialog.composing.set(null_mut());
            }
            _ => {}
        }
        DefSubclassProc(hwnd, msg, w, l)
    }
}

pub(super) fn open(
    owner: HWND,
    title: &str,
    filename: &str,
    preview: Option<Preview>,
    error: &str,
) -> Outcome {
    unsafe {
        let instance = GetModuleHandleW(null());
        let class = ui::wide("ALTRun.Native.ImportPreview");
        let title = l10n::tr(title);
        let is_bookmarks = matches!(&preview, Some(Preview::Bookmarks { .. }));
        let failed = preview.is_none();
        let height = if failed { 233 } else { HEIGHT };
        let mut dialog = Box::new(Dialog {
            model: RefCell::new(Model {
                preview,
                shown: Vec::new(),
                selected: None,
                editing_alias: None,
                query: String::new(),
                updating: true,
                outcome: Outcome::Cancelled,
            }),
            composing: Cell::new(null_mut()),
            dpi: 96,
            height,
            row_height: if is_bookmarks {
                BOOKMARK_ROW_HEIGHT
            } else {
                ROW_HEIGHT
            },
            caption: null_mut(),
            font: null_mut(),
            surface: CreateSolidBrush(theme::surface()),
            paper: CreateSolidBrush(theme::paper()),
            title,
        });
        let common = INITCOMMONCONTROLSEX {
            dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_LISTVIEW_CLASSES,
        };
        InitCommonControlsEx(&common);
        let registration = RegisterClassW(&WNDCLASSW {
            lpfnWndProc: Some(procedure),
            hInstance: instance,
            lpszClassName: class.as_ptr(),
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            ..Default::default()
        });
        let hwnd = CreateWindowExW(
            WS_EX_CONTROLPARENT,
            class.as_ptr(),
            ui::wide(&dialog.title).as_ptr(),
            WS_POPUP | WS_SYSMENU | WS_CLIPCHILDREN,
            0,
            0,
            1,
            1,
            owner,
            null_mut(),
            instance,
            (&raw mut *dialog).cast::<std::ffi::c_void>(),
        );
        if hwnd.is_null() {
            if registration != 0 {
                UnregisterClassW(class.as_ptr(), instance);
            }
            DeleteObject(dialog.surface);
            DeleteObject(dialog.paper);
            ui::show_error("ui.import_preview_failed");
            return Outcome::Cancelled;
        }
        let dpi = ui::geometry::fit(hwnd, WIDTH, height);
        dialog.dpi = dpi;
        dialog.caption = ui::make_font(13, 600, "Microsoft YaHei UI", dpi);
        dialog.font = ui::make_font(14, 400, "Microsoft YaHei UI", dpi);
        let scale = |n| ui::scaled(n, dpi);
        theme::frame(hwnd);
        ui::round_panel(hwnd, scale(WIDTH), scale(height), dpi);
        let control = |class: &str, text: &str, id: usize, (x, y, w, h), style| {
            let child = CreateWindowExW(
                0,
                ui::wide(class).as_ptr(),
                ui::wide(text).as_ptr(),
                WS_CHILD | WS_VISIBLE | style,
                scale(x),
                scale(y),
                scale(x + w) - scale(x),
                scale(y + h) - scale(y),
                hwnd,
                id as HMENU,
                instance,
                null(),
            );
            SendMessageW(child, WM_SETFONT, dialog.font as WPARAM, 0);
            child
        };
        let button = |label: &str, id, bounds| {
            control(
                "BUTTON",
                &l10n::tr(label),
                id,
                bounds,
                WS_TABSTOP | BS_OWNERDRAW as u32,
            )
        };
        ui::style_caption_close(button("ui.close", CAPTION_CLOSE, (WIDTH - 47, 1, 46, 40)));
        control(
            "STATIC",
            filename,
            0,
            (20, 53, WIDTH - 40, 23),
            STATIC_PATHELLIPSIS | STATIC_NOPREFIX,
        );
        let focus = if failed {
            control(
                "STATIC",
                error,
                ERROR,
                (20, 89, WIDTH - 40, 72),
                STATIC_NOPREFIX,
            );
            null_mut()
        } else {
            if is_bookmarks {
                button("ui.browser_bookmarks_select_all", ALL, (20, 77, 96, 29));
                button("ui.browser_bookmarks_invert", INVERT, (122, 77, 123, 29));
                button("ui.browser_bookmarks_clear", CLEAR, (251, 77, 113, 29));
                let filter = control(
                    "EDIT",
                    "",
                    FILTER,
                    (WIDTH - 329, 77, 309, 29),
                    WS_TABSTOP | ES_AUTOHSCROLL as u32,
                );
                ui::style_search(filter, dpi, "ui.browser_bookmarks_filter", search_colours);
                let alias = control(
                    "EDIT",
                    "",
                    ALIAS,
                    (0, 0, 1, 1),
                    WS_CLIPSIBLINGS | ES_AUTOHSCROLL as u32,
                );
                ShowWindow(alias, SW_HIDE);
                ui::style_edit_with_colours(alias, dpi, alias_colours);
                SendMessageW(alias, EM_LIMITTEXT, 256, 0);
                let data = &raw const *dialog as usize;
                for edit in [filter, alias] {
                    SetWindowSubclass(edit, Some(edit_keys), 1, data);
                }
            } else {
                for (id, label, x) in [
                    (APPEND, "ui.import_append_mode", 20),
                    (REPLACE, "ui.import_replace_mode", 282),
                ] {
                    button(label, id, (x, 77, 255, 29));
                }
            }
            let list = control(
                "SysListView32",
                "",
                TABLE,
                (20, TABLE_TOP, WIDTH - 40, 312),
                WS_TABSTOP
                    | WS_CLIPSIBLINGS
                    | LVS_REPORT
                    | LVS_OWNERDRAWFIXED
                    | LVS_SINGLESEL
                    | LVS_SHOWSELALWAYS,
            );
            if !list.is_null() {
                table::configure_report(
                    list,
                    dpi,
                    if is_bookmarks {
                        BOOKMARK_COLUMNS
                    } else {
                        IMPORT_COLUMNS
                    },
                    0,
                );
            }
            control(
                "STATIC",
                "",
                ERROR,
                (20, 434, WIDTH - 40, 28),
                STATIC_NOPREFIX,
            );
            if is_bookmarks {
                child(hwnd, FILTER)
            } else {
                list
            }
        };
        control(
            "STATIC",
            "",
            STATUS,
            (20, height - 42, WIDTH - 345, 28),
            STATIC_NOPREFIX | ui::STATIC_CENTER_IMAGE,
        );
        button(
            if failed { "ui.close" } else { "ui.cancel" },
            CANCEL,
            (WIDTH - 308, height - 43, 114, 31),
        );
        let apply = button(
            "ui.choose_again",
            APPLY,
            (WIDTH - 186, height - 43, 166, 31),
        );
        if failed {
            refresh_status(hwnd, &dialog);
        } else {
            refresh(hwnd, &dialog);
        }
        let modal_owner = ui::dialogs::ModalOwner::disable(owner);
        ShowWindow(hwnd, SW_SHOW);
        SetFocus(if focus.is_null() { apply } else { focus });
        let mut message = MSG::default();
        while IsWindow(hwnd) != 0 {
            let result = GetMessageW(&mut message, null_mut(), 0, 0);
            if result <= 0 {
                if result == 0 {
                    PostQuitMessage(message.wParam as i32);
                }
                break;
            }
            if message.message == WM_KEYDOWN
                && (message.hwnd == hwnd || IsChild(hwnd, message.hwnd) != 0)
            {
                let focus = GetFocus();
                if focus == dialog.composing.get() && !focus.is_null() {
                    TranslateMessage(&message);
                    DispatchMessageW(&message);
                    continue;
                }
                let id = GetDlgCtrlID(focus) as usize;
                if message.wParam == VK_ESCAPE as usize {
                    if id == ALIAS {
                        SetFocus(child(hwnd, TABLE));
                    } else {
                        SendMessageW(hwnd, WM_CLOSE, 0, 0);
                    }
                    continue;
                }
                if message.wParam == VK_RETURN as usize {
                    if id == ALIAS {
                        SetFocus(child(hwnd, TABLE));
                    } else {
                        ui::dialogs::accept_key(
                            hwnd,
                            APPLY,
                            &[CANCEL, CAPTION_CLOSE, APPEND, REPLACE, ALL, INVERT, CLEAR],
                        );
                    }
                    continue;
                }
            }
            if IsDialogMessageW(hwnd, &message) == 0 {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        if IsWindow(hwnd) != 0 {
            DestroyWindow(hwnd);
        }
        modal_owner.restore(true);
        DeleteObject(dialog.caption);
        DeleteObject(dialog.font);
        DeleteObject(dialog.surface);
        DeleteObject(dialog.paper);
        if registration != 0 {
            UnregisterClassW(class.as_ptr(), instance);
        }
        dialog.model.into_inner().outcome
    }
}
