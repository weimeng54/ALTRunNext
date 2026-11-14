// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::*;
use crate::ui::dialogs::input::confirm_details;
use altrun_probe::backup::{Backup, Media};

enum Complete {
    Preview(Box<Backup>, bool, Option<PathBuf>),
    Saved,
    Restored,
    Failed(String, bool),
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
    unsafe {
        EnableWindow(GetDlgItem(hwnd, CREATE_BACKUP as i32), 0);
        EnableWindow(GetDlgItem(hwnd, RESTORE_BACKUP as i32), 0);
    }
    let result = std::thread::Builder::new().spawn(move || {
        localization::install_snapshot(language);
        let _ = sender.send(job());
        unsafe {
            PostMessageW(window as HWND, BACKUP_RESULT, 0, 0);
        }
    });
    if let Err(error) = result {
        WORK.with(|work| work.borrow_mut().take());
        for id in [CREATE_BACKUP, RESTORE_BACKUP] {
            unsafe {
                EnableWindow(GetDlgItem(hwnd, id as i32), 1);
            }
        }
        status(&error.to_string());
    }
}

fn selection(hwnd: HWND, save: bool) -> Option<PathBuf> {
    use std::os::windows::ffi::OsStringExt;
    let descriptor = wide(&format!(
        "{}\0*.zip\0",
        localization::tr("backup.zip_filter")
    ));
    let mut name = vec![0u16; 32768];
    if save {
        let default_name = wide("ALTRunNext-backup.zip");
        name[..default_name.len()].copy_from_slice(&default_name);
    }
    let extension = wide("zip");
    let dialog = &mut OPENFILENAMEW {
        lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
        hwndOwner: hwnd,
        lpstrFile: name.as_mut_ptr(),
        nMaxFile: name.len() as u32,
        lpstrFilter: descriptor.as_ptr(),
        lpstrDefExt: extension.as_ptr(),
        Flags: OFN_NOCHANGEDIR
            | OFN_EXPLORER
            | if save {
                OFN_OVERWRITEPROMPT | OFN_PATHMUSTEXIST
            } else {
                OFN_FILEMUSTEXIST
            },
        ..unsafe { std::mem::zeroed() }
    };
    let ok = unsafe {
        if save {
            GetSaveFileNameW(dialog)
        } else {
            GetOpenFileNameW(dialog)
        }
    };
    (ok != 0).then(|| {
        let end = name.iter().position(|&ch| ch == 0).unwrap_or(name.len());
        PathBuf::from(std::ffi::OsString::from_wide(&name[..end]))
    })
}

pub(super) fn open(hwnd: HWND, restore: bool) {
    let (data, settings, launcher) = context();
    let source = if restore {
        let Some(path) = selection(hwnd, false) else {
            return;
        };
        Some(path)
    } else {
        None
    };
    work(hwnd, move || {
        let result = match &source {
            Some(path) => Backup::load(path),
            None => flush(&settings, launcher, false).and_then(|()| Backup::capture(&data)),
        };
        match result {
            Ok(backup) => Complete::Preview(Box::new(backup), restore, source),
            Err(error) => Complete::Failed(error, false),
        }
    });
}

fn media(value: Media, default: &str) -> String {
    localization::tr(match value {
        Media::Default => "backup.default",
        Media::Missing => default,
        Media::Included => "backup.included",
    })
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

fn preview(hwnd: HWND, backup: Backup, restore: bool, source: Option<PathBuf>) {
    let mut message = localization::message(
        "backup.preview",
        &[
            ("count", &backup.count().to_string()),
            (
                "background",
                &media(backup.background(), "backup.background_missing"),
            ),
            ("sound", &media(backup.sound(), "backup.sound_missing")),
        ],
    );
    if let Some(source) = source {
        let header = source
            .file_name()
            .unwrap_or(source.as_os_str())
            .to_string_lossy();
        message = format!(
            "{}\n{}\n\n{}\n\n{}",
            header,
            localization::message("backup.created", &[("date", &date(backup.created()))]),
            message,
            localization::tr("backup.restore_note")
        );
    }
    let (title, action) = if restore {
        ("backup.preview_restore", "backup.confirm_restore")
    } else {
        ("backup.preview_create", "backup.confirm_create")
    };
    if !confirm_details(hwnd, title, &message, action) {
        status("");
        return;
    }
    let (data, settings, launcher) = context();
    if !restore {
        let Some(dest) = selection(hwnd, true) else {
            status("");
            return;
        };
        let protected = [
            data.clone(),
            data.with_extension("settings.ini"),
            data.with_extension("state.tsv"),
        ];
        if protected.contains(&dest)
            || !dest
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"))
        {
            failed(hwnd, "backup.choose_other_file", false);
            return;
        }
        work(hwnd, move || match backup.write(&dest) {
            Ok(()) => Complete::Saved,
            Err(error) => Complete::Failed(error, false),
        });
        return;
    }
    if form_is_dirty() {
        if !confirm_details(
            hwnd,
            title,
            "backup.unsaved_settings",
            "ui.discard_and_close",
        ) {
            return;
        }
        let saved = EDITOR.with(|c| c.borrow().as_ref().unwrap().saved.clone());
        load_form(&saved);
        sync_save_button(hwnd);
    }
    work(hwnd, move || {
        let result = flush(&settings, launcher, true).and_then(|()| {
            let state = altrun_probe::integration::Integration::for_data(&data)?.status()?;
            altrun_probe::backup::apply(&data, backup, state)
        });
        match result {
            Ok(_) => Complete::Restored,
            Err(error) => {
                let pending = altrun_probe::backup::recovery_pending(&data);
                if !pending {
                    queue(&settings, launcher, Change::CancelRestore);
                }
                Complete::Failed(error, pending)
            }
        }
    });
}

fn failed(hwnd: HWND, error: &str, pending: bool) {
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
        let _ = confirm_details(hwnd, "backup.preview_restore", &message, "ui.ok");
    }
}

pub(super) fn complete(hwnd: HWND) {
    let result = WORK.with(|work| {
        work.borrow_mut()
            .take()
            .and_then(|work| work.try_recv().ok())
    });
    unsafe {
        EnableWindow(GetDlgItem(hwnd, CREATE_BACKUP as i32), 1);
        EnableWindow(GetDlgItem(hwnd, RESTORE_BACKUP as i32), 1);
    }
    match result {
        Some(Complete::Preview(backup, restore, source)) => preview(hwnd, *backup, restore, source),
        Some(Complete::Saved) => status("backup.saved"),
        Some(Complete::Restored) => {
            let (_, settings, launcher) = context();
            unsafe {
                DestroyWindow(hwnd);
            }
            queue(&settings, launcher, Change::FinishRestore);
        }
        Some(Complete::Failed(error, pending)) => failed(hwnd, &error, pending),
        None => {}
    }
}
