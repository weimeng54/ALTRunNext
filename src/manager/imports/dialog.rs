// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::{Accepted, Preview};
use crate::{localization as l10n, manager::table, theme, ui};
use altrun_probe::browser_bookmarks::{BookmarkStatus, selected_entries};
use std::{
    cell::RefCell,
    ptr::{null, null_mut},
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::LibraryLoader::GetModuleHandleW,
    UI::{Controls::*, Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
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

struct Model {
    preview: Option<Preview>,
    shown: Vec<usize>,
    selected: Option<usize>,
    query: String,
    updating: bool,
    outcome: Outcome,
}
struct Dialog {
    model: RefCell<Model>,
    dpi: i32,
    height: i32,
    caption: HFONT,
    font: HFONT,
    surface: HBRUSH,
    paper: HBRUSH,
    title: String,
}

impl Model {
    fn visible_rows(&mut self) -> Vec<Vec<String>> {
        let mut shown = Vec::new();
        let table = match self.preview.as_ref() {
            Some(Preview::LaunchItems { rows, replace, .. }) => {
                shown.extend(0..rows.len());
                rows.iter()
                    .map(|r| {
                        vec![
                            r.entry.alias.clone(),
                            r.entry.name.clone(),
                            r.entry.command.clone(),
                            l10n::tr(if !replace && !r.conflict.is_empty() {
                                r.conflict
                            } else {
                                "ui.import_ready"
                            }),
                        ]
                    })
                    .collect()
            }
            Some(Preview::Bookmarks { rows, .. }) => rows
                .iter()
                .enumerate()
                .filter_map(|(index, row)| {
                    let folder = row.bookmark.folders.join(" / ");
                    let matches = [&row.bookmark.title, &row.bookmark.url, &folder]
                        .iter()
                        .any(|text| text.to_lowercase().contains(&self.query));
                    if !matches {
                        return None;
                    }
                    shown.push(index);
                    let mark = if row.status != BookmarkStatus::Ready {
                        "—"
                    } else if row.checked {
                        "☑"
                    } else {
                        "☐"
                    };
                    Some(vec![
                        mark.into(),
                        row.bookmark.title.clone(),
                        row.alias.clone(),
                        folder,
                        row.bookmark.url.clone(),
                        l10n::tr(row.status.label()),
                    ])
                })
                .collect(),
            None => Vec::new(),
        };
        self.shown = shown;
        if !self.selected.is_some_and(|row| self.shown.contains(&row)) {
            self.selected = self.shown.first().copied();
        }
        table
    }

    fn status(&self) -> (String, String, bool) {
        let label = l10n::tr("ui.choose_again");
        match self.preview.as_ref() {
            Some(Preview::LaunchItems {
                existing,
                rows,
                replace,
                ignored,
            }) => {
                let included = rows
                    .iter()
                    .filter(|row| *replace || row.conflict.is_empty())
                    .count();
                let added = included.to_string();
                let skipped = (rows.len() - included + ignored).to_string();
                let (key, args, label) = if *replace {
                    (
                        "ui.import_replace_summary",
                        vec![
                            ("existing", existing.to_string()),
                            ("count", added),
                            ("skipped", skipped),
                        ],
                        "ui.import_replace_draft",
                    )
                } else {
                    (
                        "ui.import_append_summary",
                        vec![("count", added), ("skipped", skipped)],
                        "ui.import_append_draft",
                    )
                };
                let args = args
                    .iter()
                    .map(|(label, value)| (*label, value.as_str()))
                    .collect::<Vec<_>>();
                (l10n::message(key, &args), l10n::tr(label), included > 0)
            }
            Some(Preview::Bookmarks { rows, .. }) => {
                let total = rows
                    .iter()
                    .filter(|row| row.status == BookmarkStatus::Ready)
                    .count();
                let selected = rows
                    .iter()
                    .filter(|row| row.checked && row.status == BookmarkStatus::Ready)
                    .count();
                let selected_text = selected.to_string();
                (
                    l10n::message(
                        "ui.browser_bookmarks_selected",
                        &[
                            ("selected", &selected_text),
                            ("total", &total.to_string()),
                            ("skipped", &(rows.len() - total).to_string()),
                        ],
                    ),
                    l10n::message("ui.browser_bookmarks_confirm", &[("count", &selected_text)]),
                    selected > 0,
                )
            }
            None => (String::new(), label, true),
        }
    }

    fn accept(&self) -> Result<Option<Accepted>, String> {
        match self.preview.as_ref() {
            Some(Preview::LaunchItems { rows, replace, .. }) => {
                let entries: Vec<_> = rows
                    .iter()
                    .filter(|row| *replace || row.conflict.is_empty())
                    .map(|row| row.entry.clone())
                    .collect();
                Ok((!entries.is_empty()).then_some(Accepted {
                    entries,
                    replace: *replace,
                }))
            }
            Some(Preview::Bookmarks { current, rows }) => {
                let entries = selected_entries(rows, current)?;
                Ok((!entries.is_empty()).then_some(Accepted {
                    entries,
                    replace: false,
                }))
            }
            None => Ok(None),
        }
    }
}

unsafe fn child(hwnd: HWND, id: usize) -> HWND {
    unsafe { GetDlgItem(hwnd, id as i32) }
}

fn clear_error(hwnd: HWND) {
    unsafe { SetWindowTextW(child(hwnd, ERROR), ui::wide("").as_ptr()) };
}

fn sync_alias(hwnd: HWND, shared: &Dialog) {
    let alias = {
        let mut model = shared.model.borrow_mut();
        model.updating = true;
        match (&model.preview, model.selected) {
            (Some(Preview::Bookmarks { rows, .. }), Some(index))
                if rows[index].status == BookmarkStatus::Ready =>
            {
                Some(rows[index].alias.clone())
            }
            _ => None,
        }
    };
    unsafe {
        let edit = child(hwnd, ALIAS);
        if !edit.is_null() {
            EnableWindow(edit, i32::from(alias.is_some()));
            SetWindowTextW(edit, ui::wide(alias.as_deref().unwrap_or("")).as_ptr());
        }
    }
    shared.model.borrow_mut().updating = false;
}

fn refresh(hwnd: HWND, shared: &Dialog) {
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
            ui::scaled(ROW_HEIGHT, shared.dpi),
        );
    }
    sync_alias(hwnd, shared);
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
    let mut text = ui::wide(if toggle { "☑" } else { "☐" });
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

fn read_text(hwnd: HWND) -> String {
    unsafe {
        let mut text = vec![0; GetWindowTextLengthW(hwnd) as usize + 1];
        let len = GetWindowTextW(hwnd, text.as_mut_ptr(), text.len() as i32);
        String::from_utf16_lossy(&text[..len.max(0) as usize])
    }
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
                for (control, checked) in [(APPEND, id == APPEND), (REPLACE, id == REPLACE)] {
                    SendMessageW(child(hwnd, control), BM_SETCHECK, usize::from(checked), 0);
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
            shared.model.borrow_mut().query = read_text(control).trim().to_lowercase();
            refresh(hwnd, shared);
        }
        ALIAS if code == EN_CHANGE => {
            let value = read_text(control);
            let changed = {
                let mut model = shared.model.borrow_mut();
                if model.updating {
                    return;
                }
                let Some(index) = model.selected else { return };
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
                iSubItem: 2,
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
                    let index = {
                        let mut model = shared.model.borrow_mut();
                        model.selected = model.shown.get(change.iItem as usize).copied();
                        model.selected
                    };
                    if index.is_some() {
                        sync_alias(hwnd, shared);
                    }
                }
            }
            NM_CLICK | NM_DBLCLK => {
                let item = &*(l as *const NMITEMACTIVATE);
                if item.iItem < 0 {
                    return;
                }
                if header.code == NM_CLICK && item.iSubItem == 0 {
                    toggle(hwnd, shared, item.iItem as usize);
                } else if header.code == NM_DBLCLK {
                    let alias = child(hwnd, ALIAS);
                    if IsWindowEnabled(alias) != 0 {
                        SetFocus(alias);
                        SendMessageW(alias, EM_SETSEL, 0, -1);
                    }
                }
            }
            LVN_KEYDOWN => {
                let key = &*(l as *const NMLVKEYDOWN);
                if key.wVKey == VK_SPACE {
                    let index = SendMessageW(
                        header.hwndFrom,
                        LVM_GETNEXTITEM,
                        usize::MAX,
                        LVNI_SELECTED as LPARAM,
                    );
                    if index >= 0 {
                        toggle(hwnd, shared, index as usize);
                    }
                }
            }
            _ => {}
        }
    }
}

fn draw_row(item: &DRAWITEMSTRUCT, dialog: &Dialog) {
    let cells = {
        let model = dialog.model.borrow();
        let Some(&index) = model.shown.get(item.itemID as usize) else {
            return;
        };
        match model.preview.as_ref() {
            Some(Preview::LaunchItems { rows, replace, .. }) => {
                let row = &rows[index];
                vec![
                    row.entry.alias.clone(),
                    row.entry.name.clone(),
                    row.entry.command.clone(),
                    l10n::tr(if !replace && !row.conflict.is_empty() {
                        row.conflict
                    } else {
                        "ui.import_ready"
                    }),
                ]
            }
            Some(Preview::Bookmarks { rows, .. }) => {
                let row = &rows[index];
                vec![
                    String::new(),
                    row.bookmark.title.clone(),
                    row.alias.clone(),
                    row.bookmark.folders.join(" / "),
                    row.bookmark.url.clone(),
                    l10n::tr(row.status.label()),
                ]
            }
            None => return,
        }
    };
    let columns = cells.iter().map(String::as_str).collect::<Vec<_>>();
    table::draw_columns(item, &columns, dialog.font, dialog.dpi);
    let check = {
        let model = dialog.model.borrow();
        let index = model.shown[item.itemID as usize];
        match &model.preview {
            Some(Preview::Bookmarks { rows, .. }) => {
                Some((rows[index].status, rows[index].checked))
            }
            _ => None,
        }
    };
    if let Some((status, checked)) = check {
        let px = |n| ui::scaled(n, dialog.dpi);
        let left = -unsafe { GetScrollPos(item.hwndItem, SB_HORZ) } + px(19);
        let side = px(16);
        let top = (item.rcItem.top + item.rcItem.bottom - side) / 2;
        let bounds = RECT {
            left,
            top,
            right: left + side,
            bottom: top + side,
        };
        let active = status == BookmarkStatus::Ready;
        let fill = if active && checked {
            theme::accent()
        } else {
            theme::paper()
        };
        let border = if active && checked {
            theme::accent()
        } else {
            theme::border()
        };
        ui::rounded(item.hDC, bounds, px(4), fill, border);
        if active && checked {
            unsafe {
                let pen = CreatePen(PS_SOLID, px(2).max(1), theme::on_accent());
                let old = SelectObject(item.hDC, pen);
                MoveToEx(item.hDC, left + px(4), top + px(8), null_mut());
                LineTo(item.hDC, left + px(7), top + px(11));
                LineTo(item.hDC, left + px(13), top + px(5));
                SelectObject(item.hDC, old);
                DeleteObject(pen);
            }
        }
    }
}

fn paint(hwnd: HWND, dialog: &Dialog, dc: HDC) {
    let mut bounds = RECT::default();
    unsafe { GetClientRect(hwnd, &mut bounds) };
    let y = ui::scaled(HEADER, dialog.dpi);
    let footer_y = ui::scaled(dialog.height - FOOTER, dialog.dpi);
    ui::fill(dc, &bounds, theme::surface());
    ui::fill(
        dc,
        &RECT {
            bottom: y,
            ..bounds
        },
        theme::chrome(),
    );
    ui::fill(
        dc,
        &RECT {
            top: footer_y,
            ..bounds
        },
        theme::chrome(),
    );
    for top in [y - 1, footer_y] {
        ui::fill(
            dc,
            &RECT {
                top,
                bottom: top + 1,
                ..bounds
            },
            theme::divider(),
        );
    }
    ui::panel_caption(dc, dialog.dpi, dialog.caption, &dialog.title);
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
                    SetBkMode(w as HDC, TRANSPARENT as i32);
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
                        SetDCBrushColor(w as HDC, theme::chrome());
                        return GetStockObject(DC_BRUSH) as isize;
                    }
                    if msg == WM_CTLCOLOREDIT || id == ALIAS || id == FILTER {
                        return dialog.paper as isize;
                    }
                    return dialog.surface as isize;
                }
                WM_DRAWITEM if l != 0 => {
                    let item = &*(l as *const DRAWITEMSTRUCT);
                    match item.CtlID as usize {
                        TABLE => draw_row(item, dialog),
                        CAPTION_CLOSE => ui::caption_close(item, dialog.dpi),
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
                        ui::scaled(ROW_HEIGHT, dialog.dpi) as u32;
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
        let initially_enabled = IsWindowEnabled(owner) != 0;
        let mut dialog = Box::new(Dialog {
            model: RefCell::new(Model {
                preview,
                shown: Vec::new(),
                selected: None,
                query: String::new(),
                updating: true,
                outcome: Outcome::Cancelled,
            }),
            dpi: 96,
            height,
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
                ui::style_edit(filter, dpi);
                SendMessageW(
                    filter,
                    EM_SETCUEBANNER,
                    0,
                    ui::ui("ui.browser_bookmarks_filter").as_ptr() as LPARAM,
                );
                control(
                    "STATIC",
                    &l10n::tr("ui.alias"),
                    0,
                    (20, 403, 104, 27),
                    ui::STATIC_CENTER_IMAGE,
                );
                let alias = control(
                    "EDIT",
                    "",
                    ALIAS,
                    (133, 402, 299, 29),
                    WS_TABSTOP | ES_AUTOHSCROLL as u32,
                );
                ui::style_edit(alias, dpi);
                SendMessageW(alias, EM_LIMITTEXT, 256, 0);
            } else {
                for (id, label, x) in [
                    (APPEND, "ui.import_append_mode", 20),
                    (REPLACE, "ui.import_replace_mode", 282),
                ] {
                    control(
                        "BUTTON",
                        &l10n::tr(label),
                        id,
                        (x, 77, 255, 29),
                        WS_TABSTOP | BS_RADIOBUTTON as u32,
                    );
                }
                SendMessageW(child(hwnd, APPEND), BM_SETCHECK, BST_CHECKED as usize, 0);
            }
            let list = control(
                "SysListView32",
                "",
                TABLE,
                (
                    20,
                    TABLE_TOP,
                    WIDTH - 40,
                    if is_bookmarks { 278 } else { 312 },
                ),
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
                child(hwnd, APPEND)
            }
        };
        control(
            "STATIC",
            "",
            STATUS,
            (20, height - 42, WIDTH - 345, 28),
            STATIC_NOPREFIX | ui::STATIC_CENTER_IMAGE,
        );
        let cancel = button("ui.cancel", CANCEL, (WIDTH - 308, height - 43, 114, 31));
        button(
            "ui.choose_again",
            APPLY,
            (WIDTH - 186, height - 43, 166, 31),
        );
        if failed {
            refresh_status(hwnd, &dialog);
        } else {
            refresh(hwnd, &dialog);
        }
        if initially_enabled {
            EnableWindow(owner, 0);
        }
        ShowWindow(hwnd, SW_SHOW);
        SetFocus(if focus.is_null() { cancel } else { focus });
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
                if message.wParam == VK_ESCAPE as usize {
                    SendMessageW(hwnd, WM_CLOSE, 0, 0);
                    continue;
                }
                if message.wParam == VK_RETURN as usize {
                    let id = GetDlgCtrlID(GetFocus()) as usize;
                    if id == APPLY && IsWindowEnabled(child(hwnd, APPLY)) != 0 {
                        SendMessageW(hwnd, WM_COMMAND, APPLY, 0);
                    } else if id == CANCEL {
                        SendMessageW(hwnd, WM_CLOSE, 0, 0);
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
        if initially_enabled {
            EnableWindow(owner, 1);
        }
        SetForegroundWindow(owner);
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
