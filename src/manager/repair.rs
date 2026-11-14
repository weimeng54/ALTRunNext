// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::*;
use altrun_probe::management::{self, MissingPath, PathInspection, RepairTarget};
use std::{collections::HashSet, path::Path};

const CLASS: &str = "ALTRun.Native.MissingPaths";
const WIDTH: i32 = 912;
const HEIGHT: i32 = 460;
const FOOTER: i32 = HEIGHT - 60;
const LIST: usize = 2180;
const REPAIR: usize = 2181;
const REMOVE: usize = 2182;
const CLOSE: usize = 2183;
const CAPTION_CLOSE: usize = 2184;
const COLUMNS: [(&str, i32); 4] = [
    ("ui.alias", 126),
    ("ui.name", 160),
    ("ui.missing_path", 344),
    ("ui.missing_path_reason", 207),
];

struct Finding {
    id: String,
    path: MissingPath,
    cells: Vec<String>,
}

pub(super) fn inspect(owner: HWND, rows: &[Entry], base: &Path) {
    let mut present = 0;
    let mut skipped = 0;
    let mut findings = Vec::new();
    for entry in rows {
        match management::inspect_path(entry, base) {
            PathInspection::Missing(path) => {
                let reason = if path.target == RepairTarget::OpenWith {
                    "ui.missing_open_with_app"
                } else {
                    "ui.missing_target_file"
                };
                findings.push(Finding {
                    id: entry.id.clone(),
                    cells: vec![
                        entry.alias.clone(),
                        entry.name.clone(),
                        path.path.clone(),
                        crate::localization::tr(reason),
                    ],
                    path,
                });
            }
            PathInspection::Present => present += 1,
            PathInspection::Unconfirmed(_) => skipped += 1,
            PathInspection::NonCommand => {}
        }
    }
    let summary = crate::localization::message(
        "ui.missing_path_summary",
        &[
            ("available", &present.to_string()),
            ("missing", &findings.len().to_string()),
            ("skipped", &skipped.to_string()),
        ],
    );
    if findings.is_empty() {
        let summary = format!(
            "{}\n{}",
            summary,
            crate::localization::tr("ui.no_missing_local_targets_were_found")
        );
        crate::ui::dialogs::input::report(
            owner,
            "ui.check_missing_paths_2",
            crate::ui::dialogs::input::Report {
                summary,
                rows: Vec::new(),
            },
        );
    } else {
        open(owner, findings, &summary);
    }
}

fn pick_replacement(owner: HWND, target: RepairTarget) -> Result<Option<PathBuf>, String> {
    match target {
        RepairTarget::OpenWith => {
            crate::shortcut_intake::pick_program(owner).map(|path| path.map(PathBuf::from))
        }
        RepairTarget::Program => {
            let filter =
                wide("Programs (*.exe;*.com;*.bat;*.cmd;*.lnk)\0*.exe;*.com;*.bat;*.cmd;*.lnk\0");
            Ok(crate::windows::file_picker::pick_path_filtered(
                owner,
                false,
                false,
                Some(&filter),
            ))
        }
        _ => Ok(pick_path(owner, false, target == RepairTarget::Folder)),
    }
}

fn repair_path(owner: HWND, finding: &Finding) -> Result<bool, String> {
    let Some(replacement) = pick_replacement(owner, finding.path.target)? else {
        return Ok(false);
    };
    EDITOR.with(|editor| {
        let mut editor = editor.borrow_mut();
        let s = editor.as_mut().unwrap();
        let index = index_of(&s.draft, &finding.id).ok_or("ui.launch_item_no_longer_in_draft")?;
        let updated =
            management::replace_missing_path(&s.draft[index], finding.path.target, &replacement)?;
        s.draft[index] = updated;
        s.selected = index;
        Ok(true)
    })
}

fn remove_paths(owner: HWND, findings: &[Finding]) -> bool {
    if !confirm(
        owner,
        &crate::localization::message(
            "ui.remove_missing_items_confirm",
            &[("count", &findings.len().to_string())],
        ),
    ) {
        return false;
    }
    let missing: HashSet<_> = findings.iter().map(|finding| finding.id.as_str()).collect();
    EDITOR.with(|editor| {
        let mut editor = editor.borrow_mut();
        let editor = editor.as_mut().unwrap();
        editor
            .draft
            .retain(|entry| !missing.contains(entry.id.as_str()));
        editor.selected = editor.selected.min(editor.draft.len().saturating_sub(1));
    });
    true
}

struct Report {
    findings: Vec<Finding>,
    font: HFONT,
    caption: HFONT,
    dpi: i32,
    completion: Option<&'static str>,
    busy: bool,
}

unsafe extern "system" fn proc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    unsafe {
        if msg == WM_NCCREATE {
            let create = &*(l as *const CREATESTRUCTW);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize);
        }
        let state = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Report;
        if state.is_null() {
            return DefWindowProcW(hwnd, msg, w, l);
        }
        match msg {
            WM_ERASEBKGND => 1,
            WM_PAINT => {
                let mut paint = PAINTSTRUCT::default();
                let dc = BeginPaint(hwnd, &mut paint);
                paint_report(dc, (*state).dpi, (*state).caption);
                EndPaint(hwnd, &paint);
                0
            }
            WM_PRINTCLIENT => {
                paint_report(w as HDC, (*state).dpi, (*state).caption);
                0
            }
            WM_NCHITTEST => {
                let result = DefWindowProcW(hwnd, msg, w, l);
                let dpi = (*state).dpi;
                let mut point = POINT {
                    x: l as i16 as i32,
                    y: (l >> 16) as i16 as i32,
                };
                ScreenToClient(hwnd, &mut point);
                if result == HTCLIENT as isize
                    && (0..scaled(CAPTION, dpi)).contains(&point.y)
                    && point.x < scaled(WIDTH - 47, dpi)
                {
                    HTCAPTION as isize
                } else {
                    result
                }
            }
            WM_CTLCOLORSTATIC => {
                SetTextColor(w as HDC, theme::text());
                SetBkColor(w as HDC, theme::surface());
                SetDCBrushColor(w as HDC, theme::surface());
                GetStockObject(DC_BRUSH) as LRESULT
            }
            WM_MEASUREITEM if w == LIST && l != 0 => {
                (*(l as *mut MEASUREITEMSTRUCT)).itemHeight =
                    scaled(ROW_HEIGHT, (*state).dpi) as u32;
                1
            }
            WM_DRAWITEM if l != 0 => {
                let item = &*(l as *const DRAWITEMSTRUCT);
                let state = &*state;
                match item.CtlID as usize {
                    LIST => {
                        if let Some(finding) = state.findings.get(item.itemID as usize) {
                            let cells = &finding.cells;
                            table::draw_columns(
                                item,
                                &[&cells[0], &cells[1], &cells[2], &cells[3]],
                                state.font,
                                state.dpi,
                            );
                        }
                    }
                    CAPTION_CLOSE => crate::ui::caption_close(item, state.dpi),
                    _ => crate::ui::button_on(
                        item,
                        item.CtlID as usize == REPAIR,
                        theme::chrome(),
                        state.dpi,
                    ),
                }
                1
            }
            WM_NOTIFY if l != 0 => {
                let note = &*(l as *const NMHDR);
                if note.idFrom == LIST {
                    if note.code == NM_DBLCLK {
                        let hit = &*(l as *const NMITEMACTIVATE);
                        if hit.iItem >= 0 {
                            SendMessageW(hwnd, WM_COMMAND, REPAIR, 0);
                        }
                    } else if note.code == LVN_ITEMCHANGED {
                        let selection = SendMessageW(
                            note.hwndFrom,
                            LVM_GETNEXTITEM,
                            usize::MAX,
                            LVNI_SELECTED as LPARAM,
                        );
                        EnableWindow(GetDlgItem(hwnd, REPAIR as i32), i32::from(selection >= 0));
                    }
                }
                0
            }
            WM_COMMAND => {
                if (*state).busy {
                    return 0;
                }
                let button = w & 0xffff;
                if matches!(button, CLOSE | CAPTION_CLOSE) {
                    DestroyWindow(hwnd);
                } else if button == REPAIR {
                    let list = GetDlgItem(hwnd, LIST as i32);
                    let row =
                        SendMessageW(list, LVM_GETNEXTITEM, usize::MAX, LVNI_SELECTED as LPARAM);
                    let findings = &(*state).findings;
                    if let Some(finding) = findings.get(row as usize) {
                        (*state).busy = true;
                        match repair_path(hwnd, finding) {
                            Ok(true) => (*state).completion = Some("ui.repaired_path_save"),
                            Ok(false) => {}
                            Err(err) => {
                                MessageBoxW(
                                    hwnd,
                                    ui(&err).as_ptr(),
                                    ui("ui.repair_missing_path").as_ptr(),
                                    MB_OK | MB_ICONERROR,
                                );
                            }
                        }
                        (*state).busy = false;
                        if (*state).completion.is_some() {
                            DestroyWindow(hwnd);
                        }
                    }
                } else if button == REMOVE {
                    (*state).busy = true;
                    let removed = remove_paths(hwnd, &(*state).findings);
                    (*state).busy = false;
                    if removed {
                        (*state).completion =
                            Some("ui.missing_entries_removed_from_the_draft_save");
                        DestroyWindow(hwnd);
                    }
                }
                0
            }
            WM_CLOSE => {
                if !(*state).busy {
                    DestroyWindow(hwnd);
                }
                0
            }
            _ => DefWindowProcW(hwnd, msg, w, l),
        }
    }
}

fn paint_report(dc: HDC, dpi: i32, caption: HFONT) {
    let rect = |top, height| RECT {
        top: scaled(top, dpi),
        right: scaled(WIDTH, dpi),
        bottom: scaled(top + height, dpi),
        ..Default::default()
    };
    crate::ui::fill(dc, &rect(0, HEIGHT), theme::surface());
    for (top, height) in [(0, CAPTION), (FOOTER, HEIGHT - FOOTER)] {
        crate::ui::fill(dc, &rect(top, height), theme::chrome());
    }
    for top in [CAPTION - 1, FOOTER] {
        crate::ui::fill(dc, &rect(top, 1), theme::divider());
    }
    crate::ui::panel_caption(dc, dpi, caption, "ui.check_missing_paths_2");
}

fn open(owner: HWND, findings: Vec<Finding>, summary: &str) {
    unsafe {
        let instance = GetModuleHandleW(std::ptr::null());
        let class = wide(CLASS);
        let mut wc: WNDCLASSW = zeroed();
        wc.lpfnWndProc = Some(proc);
        wc.hInstance = instance;
        wc.hCursor = LoadCursorW(null_mut(), IDC_ARROW);
        wc.lpszClassName = class.as_ptr();
        RegisterClassW(&wc);
        let mut state = Box::new(Report {
            findings,
            font: null_mut(),
            caption: null_mut(),
            dpi: 96,
            completion: None,
            busy: false,
        });
        let hwnd = CreateWindowExW(
            WS_EX_CONTROLPARENT,
            class.as_ptr(),
            ui("ui.check_missing_paths_2").as_ptr(),
            WS_POPUP | WS_SYSMENU | WS_CLIPCHILDREN,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            400,
            300,
            owner,
            null_mut(),
            instance,
            (&mut *state as *mut Report).cast(),
        );
        if hwnd.is_null() {
            UnregisterClassW(class.as_ptr(), instance);
            status("ui.could_not_create_the_shortcut_manager_window", true);
            return;
        }
        theme::frame(hwnd);
        let dpi = crate::ui::geometry::fit(hwnd, WIDTH, HEIGHT);
        state.dpi = dpi;
        state.font = make_font(14, 400, "Microsoft YaHei UI", dpi);
        state.caption = make_font(13, 600, "Microsoft YaHei UI", dpi);
        crate::ui::round_panel(hwnd, scaled(WIDTH, dpi), scaled(HEIGHT, dpi), dpi);
        let control = |class: &str, label: &str, style, id, (x, y, w, h)| {
            let child = CreateWindowExW(
                0,
                wide(class).as_ptr(),
                wide(label).as_ptr(),
                WS_CHILD | WS_VISIBLE | style,
                scaled(x, dpi),
                scaled(y, dpi),
                scaled(w, dpi),
                scaled(h, dpi),
                hwnd,
                id as HMENU,
                instance,
                std::ptr::null(),
            );
            SendMessageW(child, WM_SETFONT, state.font as usize, 0);
            child
        };
        let caption_close = control(
            "BUTTON",
            &crate::localization::tr("ui.close"),
            BS_OWNERDRAW as u32,
            CAPTION_CLOSE,
            (WIDTH - 47, 1, 46, CAPTION - 1),
        );
        crate::ui::style_caption_close(caption_close);
        control("STATIC", summary, 0, 0, (17, CAPTION + 14, WIDTH - 34, 23));
        let list = control(
            "SysListView32",
            "",
            WS_TABSTOP
                | WS_CLIPSIBLINGS
                | LVS_REPORT
                | LVS_SINGLESEL
                | LVS_SHOWSELALWAYS
                | LVS_NOSORTHEADER
                | LVS_OWNERDRAWFIXED,
            LIST,
            (17, CAPTION + 44, WIDTH - 34, FOOTER - CAPTION - 59),
        );
        table::configure_report(list, dpi, &COLUMNS, 0);
        let rows = state
            .findings
            .iter()
            .map(|f| f.cells.clone())
            .collect::<Vec<_>>();
        table::replace_report_rows(list, &rows, Some(0), scaled(ROW_HEIGHT, dpi));
        for (id, key, x, width) in [
            (CLOSE, "ui.close", WIDTH - 401, 81),
            (REMOVE, "ui.remove_missing_items", WIDTH - 311, 142),
            (REPAIR, "ui.repair_missing_path", WIDTH - 160, 143),
        ] {
            control(
                "BUTTON",
                &crate::localization::tr(key),
                WS_TABSTOP | BS_OWNERDRAW as u32,
                id,
                (x, FOOTER + 12, width, 36),
            );
        }
        EnableWindow(owner, 0);
        ShowWindow(hwnd, SW_SHOW);
        SetFocus(list);
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
                match message.wParam as u16 {
                    VK_ESCAPE => {
                        SendMessageW(hwnd, WM_COMMAND, CLOSE, 0);
                        continue;
                    }
                    VK_RETURN => {
                        crate::ui::dialogs::accept_key(
                            hwnd,
                            REPAIR,
                            &[REMOVE, CLOSE, CAPTION_CLOSE],
                        );
                        continue;
                    }
                    _ => {}
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
        EnableWindow(owner, 1);
        SetForegroundWindow(owner);
        UnregisterClassW(class.as_ptr(), instance);
        DeleteObject(state.font);
        DeleteObject(state.caption);
        if let Some(completion) = state.completion {
            if completion == "ui.repaired_path_save" {
                show_selected_in_list();
                let command = EDITOR.with(|editor| editor.borrow().as_ref().unwrap().command);
                SetFocus(command);
            } else {
                refresh_list();
            }
            pending_status(completion);
        }
    }
}
