// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::*;
use crate::ui::dialogs::input::{Choice, choose, confirm_details};
use altrun_probe::{
    Entry,
    backup::{Backup, Contents, Media, RestoreSelection},
};

mod dialog;

enum Complete {
    Create(usize),
    Preview(Box<RestorePreview>),
    Saved,
    Restored,
    Failed {
        error: String,
        restore: bool,
        pending: bool,
    },
    Cancelled,
}

struct RestorePreview {
    backup: Backup,
    current: Vec<Entry>,
    options: dialog::Options,
}

thread_local! {
    static WORK: RefCell<Option<mpsc::Receiver<Complete>>> = const { RefCell::new(None) };
}

pub(super) fn busy() -> bool {
    WORK.with(|work| work.borrow().is_some())
}

fn context() -> (PathBuf, mpsc::Sender<Change>, usize) {
    EDITOR.with(|editor| {
        let editor = editor.borrow();
        let editor = editor.as_ref().unwrap();
        (editor.data.clone(), editor.sender.clone(), editor.owner)
    })
}

fn queue(sender: &mpsc::Sender<Change>, launcher: usize, change: Change) {
    if sender.send(change).is_ok() {
        unsafe {
            PostMessageW(launcher as HWND, UPDATED, 0, 0);
        }
    }
}

fn flush(sender: &mpsc::Sender<Change>, launcher: usize, restore: bool) -> Result<(), String> {
    use std::time::Duration;
    let history = if restore {
        let (reply, result) = mpsc::channel();
        queue(sender, launcher, Change::BeginRestore(reply));
        result
            .recv_timeout(Duration::from_secs(8))
            .map_err(|_| "backup.busy")??
    } else {
        let (reply, result) = mpsc::channel();
        queue(sender, launcher, Change::FlushHistory(reply));
        result
            .recv_timeout(Duration::from_secs(8))
            .map_err(|_| "backup.busy")?
    };
    history
        .recv_timeout(Duration::from_secs(8))
        .map_err(|_| "backup.busy")?
}

fn work(hwnd: HWND, job: impl FnOnce() -> Complete + Send + 'static) {
    status("backup.busy");
    let window = hwnd as usize;
    let (sender, receiver) = mpsc::channel();
    let language = localization::current();
    WORK.with(|work| *work.borrow_mut() = Some(receiver));
    unsafe { EnableWindow(GetDlgItem(hwnd, BACKUP_MENU as i32), 0) };
    let result = std::thread::Builder::new().spawn(move || {
        localization::install_snapshot(language);
        let _ = sender.send(job());
        unsafe {
            PostMessageW(window as HWND, BACKUP_RESULT, 0, 0);
        }
    });
    if let Err(error) = result {
        WORK.with(|work| work.borrow_mut().take());
        unsafe { EnableWindow(GetDlgItem(hwnd, BACKUP_MENU as i32), 1) };
        status(&error.to_string());
    }
}

fn selection(hwnd: HWND, save: bool) -> Option<PathBuf> {
    use crate::windows::file_picker::{pick_path_filtered, save_file};

    if save {
        let filter = wide(&format!(
            "{}\0*.zip\0",
            localization::tr("backup.zip_filter")
        ));
        save_file(hwnd, &filter, "ALTRunNext-backup.zip", "zip")
    } else {
        let filter = wide(&format!(
            "{}\0*.zip;*.ini\0{}\0*.*\0",
            localization::tr("backup.restore_filter"),
            localization::tr("ui.all_files"),
        ));
        pick_path_filtered(hwnd, false, false, Some(&filter))
    }
}

pub(super) fn menu(hwnd: HWND) {
    stop_capture();
    let (dpi, font) = EDITOR.with(|editor| {
        let state = editor.borrow();
        let state = state.as_ref().unwrap();
        (state.dpi, state.preview.prefs.get("MenuFont"))
    });
    let Some(mut style) = ui::menu_style::PopupStyle::with_font(dpi, &font) else {
        return;
    };
    let menu = style.root();
    style.add(menu, CREATE_BACKUP, "backup.create", false, false);
    style.add(menu, RESTORE_BACKUP, "backup.restore", false, false);
    let id = style.track(
        hwnd,
        ui::menu_style::Anchor::Button {
            hwnd: unsafe { GetDlgItem(hwnd, BACKUP_MENU as i32) },
            above: true,
        },
    );
    if matches!(id, CREATE_BACKUP | RESTORE_BACKUP) {
        open(hwnd, id == RESTORE_BACKUP);
    }
}

pub(super) fn open(hwnd: HWND, restore: bool) {
    begin(hwnd, restore);
}

fn begin(hwnd: HWND, restore: bool) -> bool {
    let (data, _, _) = context();
    let source = if restore {
        let Some(path) = selection(hwnd, false) else {
            return false;
        };
        Some(path)
    } else {
        None
    };
    work(hwnd, move || {
        let result = match source {
            Some(path) => {
                load_preview(&data, &path).map(|preview| Complete::Preview(Box::new(preview)))
            }
            None => Backup::saved_launch_items(&data).map(|items| Complete::Create(items.len())),
        };
        match result {
            Ok(result) => result,
            Err(error) => Complete::Failed {
                error,
                restore,
                pending: false,
            },
        }
    });
    true
}

fn media(value: Media, filename: Option<&str>, missing: &str) -> String {
    match value {
        Media::Default => localization::tr("backup.default"),
        Media::Missing => localization::tr(missing),
        Media::Included => {
            filename.map_or_else(|| localization::tr("backup.included"), str::to_owned)
        }
    }
}

fn date(unix: u64) -> String {
    use windows_sys::Win32::Storage::FileSystem::FileTimeToLocalFileTime;
    use windows_sys::Win32::System::Time::FileTimeToSystemTime;
    let ticks = (unix + 11_644_473_600) * 10_000_000;
    let utc = FILETIME {
        dwLowDateTime: ticks as u32,
        dwHighDateTime: (ticks >> 32) as u32,
    };
    let mut local: FILETIME = unsafe { std::mem::zeroed() };
    let mut time: SYSTEMTIME = unsafe { std::mem::zeroed() };
    unsafe {
        if FileTimeToLocalFileTime(&utc, &mut local) == 0
            || FileTimeToSystemTime(&local, &mut time) == 0
        {
            return "—".into();
        }
    }
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}",
        time.wYear, time.wMonth, time.wDay, time.wHour, time.wMinute
    )
}

fn create(hwnd: HWND, count: usize) {
    let Some(choice) = dialog::create(hwnd, count) else {
        status("");
        return;
    };
    let Some(dest) = selection(hwnd, true) else {
        status("");
        return;
    };
    let (data, settings, launcher) = context();
    if !dest
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"))
    {
        failed(hwnd, "backup.choose_other_file", false, false);
        return;
    }
    work(hwnd, move || {
        let result = if dest == data
            || std::fs::canonicalize(&dest)
                .ok()
                .is_some_and(|path| Some(path) == std::fs::canonicalize(&data).ok())
        {
            Err("backup.choose_other_file".into())
        } else {
            let captured = || Backup::capture_selected(&data, choice.contents)?.write(&dest);
            if choice.contents.history {
                flush(&settings, launcher, false).and_then(|()| captured())
            } else {
                captured()
            }
        };
        match result {
            Ok(()) => Complete::Saved,
            Err(error) => Complete::Failed {
                error,
                restore: false,
                pending: false,
            },
        }
    });
}

fn load_preview(
    data: &std::path::Path,
    source: &std::path::Path,
) -> Result<RestorePreview, String> {
    let backup = Backup::load_restore_source(source)?;
    let contents = backup.contents();
    let current = if contents.items || contents.history {
        Backup::saved_launch_items(data)?
    } else {
        Vec::new()
    };
    let header = source
        .file_name()
        .unwrap_or(source.as_os_str())
        .to_string_lossy();
    let heading = if backup.created() == 0 {
        header.into_owned()
    } else {
        format!(
            "{} · {}",
            header,
            localization::message("backup.created", &[("date", &date(backup.created()))],)
        )
    };
    let incoming = backup.launch_items()?;
    let item_count = incoming.len();
    let conflicts = altrun_probe::library::append_conflicts(&current, &incoming);
    let options = dialog::Options {
        heading,
        available: Contents {
            items: item_count != 0,
            ..contents
        },
        current: current.len(),
        incoming: item_count,
        conflicts: incoming
            .into_iter()
            .zip(conflicts)
            .filter_map(|(entry, reason)| (!reason.is_empty()).then_some((entry.alias, reason)))
            .collect(),
        media: [
            (
                "backup.background",
                media(
                    backup.background(),
                    backup.background_name(),
                    "backup.background_missing",
                ),
            ),
            (
                "backup.sound",
                media(backup.sound(), backup.sound_name(), "backup.sound_missing"),
            ),
        ],
    };
    Ok(RestorePreview {
        backup,
        current,
        options,
    })
}

fn preview(hwnd: HWND, preview: RestorePreview) {
    let RestorePreview {
        backup,
        current,
        options,
    } = preview;
    let Some(selection) = dialog::restore(hwnd, options) else {
        status("");
        return;
    };
    restore(hwnd, backup, current, selection);
}

fn restore(hwnd: HWND, backup: Backup, current: Vec<Entry>, selection: RestoreSelection) {
    let (data, settings, launcher) = context();
    if form_is_dirty() {
        match choose(
            hwnd,
            "backup.unsaved_title",
            "backup.unsaved_settings",
            "backup.save_continue",
            "backup.discard_continue",
        ) {
            Choice::Primary => {
                apply();
                if form_is_dirty() {
                    return;
                }
            }
            Choice::Alternate => {}
            Choice::Cancel => {
                status("");
                return;
            }
        }
    }
    work(hwnd, move || {
        let result = flush(&settings, launcher, true).and_then(|()| {
            if !crate::manager::prepare_for_restore()? {
                return Ok(None);
            }
            let state = if selection.contents.settings {
                altrun_probe::integration::Integration::for_data(&data)?.status()?
            } else {
                (false, false)
            };
            altrun_probe::backup::apply_selected(&data, backup, state, selection, &current)
                .map(Some)
        });
        match result {
            Ok(Some(_)) => Complete::Restored,
            Ok(None) => {
                queue(&settings, launcher, Change::CancelRestore);
                Complete::Cancelled
            }
            Err(error) => {
                let pending = altrun_probe::backup::recovery_pending(&data);
                if !pending {
                    queue(&settings, launcher, Change::CancelRestore);
                }
                Complete::Failed {
                    error,
                    restore: true,
                    pending,
                }
            }
        }
    });
}

fn failed(hwnd: HWND, error: &str, restore: bool, pending: bool) {
    let message = localization::message("backup.failed", &[("detail", &localization::tr(error))]);
    status(&message);
    if pending {
        let _ = confirm_details(hwnd, "backup.preview_restore", &message, "ui.close");
        let (_, settings, launcher) = context();
        unsafe {
            DestroyWindow(hwnd);
        }
        queue(&settings, launcher, Change::ExitForRecovery);
    } else {
        let title = if restore {
            "backup.preview_restore"
        } else {
            "backup.preview_create"
        };
        while confirm_details(hwnd, title, &message, "backup.retry") {
            if begin(hwnd, restore) {
                break;
            }
        }
    }
}

pub(super) fn complete(hwnd: HWND) {
    let result = WORK.with(|work| {
        work.borrow_mut()
            .take()
            .and_then(|work| work.try_recv().ok())
    });
    unsafe {
        EnableWindow(GetDlgItem(hwnd, BACKUP_MENU as i32), 1);
    }
    match result {
        Some(Complete::Create(count)) => create(hwnd, count),
        Some(Complete::Preview(source)) => preview(hwnd, *source),
        Some(Complete::Saved) => status("backup.saved"),
        Some(Complete::Restored) => {
            let (_, settings, launcher) = context();
            unsafe {
                DestroyWindow(hwnd);
            }
            queue(&settings, launcher, Change::FinishRestore);
        }
        Some(Complete::Failed {
            error,
            restore,
            pending,
        }) => failed(hwnd, &error, restore, pending),
        Some(Complete::Cancelled) => status(""),
        None => {}
    }
}
