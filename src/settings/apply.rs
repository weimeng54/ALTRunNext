// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::*;
mod writer;
use std::time::Duration;

pub(super) const READY: u32 = WM_APP + 44;
pub(super) const EXIT: u32 = WM_APP + 45;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum After {
    Close,
    Exit,
    Backup(bool),
}

struct Update {
    previous: Preferences,
    next: Preferences,
}

pub(super) struct Writes {
    sender: mpsc::Sender<Update>,
    results: mpsc::Receiver<Result<(), String>>,
    active: Option<Preferences>,
    after: Option<After>,
}

impl Writes {
    pub(super) fn start(
        mut file: TextFile,
        data: PathBuf,
        launcher: mpsc::Sender<Change>,
        owner: usize,
        window: HWND,
        safe_mode: bool,
    ) -> Result<Self, String> {
        let (sender, updates) = mpsc::channel::<Update>();
        let (done, results) = mpsc::channel();
        let window = window as usize;
        std::thread::Builder::new()
            .name("settings-writer".into())
            .spawn(move || {
                let delay = if safe_mode
                    && std::env::var_os("ALTRUN_TEST_SETTINGS_DATA").as_deref()
                        == Some(data.as_os_str())
                {
                    std::env::var("ALTRUN_TEST_SETTINGS_WRITE_DELAY_MS")
                        .ok()
                        .and_then(|s| s.parse::<u64>().ok())
                        .unwrap_or(0)
                        .min(5000)
                } else {
                    0
                };
                for update in updates {
                    let result = writer::transact(&launcher, owner, &update.next, || {
                        if delay != 0 {
                            std::thread::sleep(Duration::from_millis(delay));
                        }
                        writer::persist(&mut file, &data, &update)
                    });
                    if done.send(result).is_err() {
                        break;
                    }
                    unsafe {
                        PostMessageW(window as HWND, READY, 0, 0);
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(Self {
            sender,
            results,
            active: None,
            after: None,
        })
    }
}

pub(super) fn submit(prefs: Preferences) {
    let window = EDITOR.with(|cell| {
        let mut state = cell.borrow_mut();
        let s = state.as_mut()?;
        if s.loading_form || s.draft == prefs {
            return None;
        }
        s.draft = prefs;
        Some(s.hwnd)
    });
    if let Some(hwnd) = window {
        status("");
        dispatch(hwnd);
    }
}

fn dispatch(hwnd: HWND) {
    let failure = EDITOR.with(|cell| {
        let mut state = cell.borrow_mut();
        let Some(s) = state.as_mut() else {
            return false;
        };
        if s.writes.active.is_some() || s.saved == s.draft {
            return false;
        }
        s.writes.active = Some(s.draft.clone());
        s.writes
            .sender
            .send(Update {
                previous: s.saved.clone(),
                next: s.draft.clone(),
            })
            .is_err()
    });
    if failure {
        completed(hwnd);
    }
}

pub(super) fn completed(hwnd: HWND) {
    let (failure, after) = EDITOR.with(|cell| {
        let mut state = cell.borrow_mut();
        let s = state.as_mut().unwrap();
        let result = match s.writes.results.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return (None, None),
            Err(_) => Err("ui.the_launcher_is_temporarily_unavailable".into()),
        };
        let Some(request) = s.writes.active.take() else {
            return (None, None);
        };
        let failure = if let Err(error) = result {
            let before = s.draft.clone();
            let mut names = Vec::new();
            for spec in SPECS {
                if request.values.get(spec.key) != s.saved.values.get(spec.key) {
                    names.push(localization::tr(spec.label));
                    if request.values.get(spec.key) == s.draft.values.get(spec.key) {
                        s.draft
                            .values
                            .insert(spec.key.to_owned(), s.saved.get(spec.key));
                    }
                }
            }
            Some((
                before,
                s.draft.clone(),
                format!("{} — {}", names.join(", "), localization::tr(&error)),
            ))
        } else {
            s.saved = request;
            None
        };
        (failure, s.writes.after.take())
    });
    let failed = failure.is_some();
    if let Some((before, corrected, detail)) = failure {
        form::restore_fields(&before, &corrected);
        update_preview();
        let message =
            localization::message("ui.changes_were_not_saved_detail", &[("detail", &detail)]);
        status(&message);
        if matches!(after, Some(After::Close | After::Exit)) {
            ui::show_error(&message);
        }
    }
    dispatch(hwnd);
    if let Some(action) = after
        && !(failed && matches!(action, After::Backup(_)))
    {
        after_edits(hwnd, action);
    }
}

pub(super) fn reopen() {
    EDITOR.with(|c| {
        if let Some(s) = c.borrow_mut().as_mut()
            && s.writes.after == Some(After::Close)
        {
            s.writes.after = None;
        }
    });
}

pub(super) fn after_edits(hwnd: HWND, action: After) {
    dispatch(hwnd);
    let pending = EDITOR.with(|c| {
        let mut state = c.borrow_mut();
        let s = state.as_mut().unwrap();
        if s.writes.active.is_some() {
            s.writes.after = Some(action);
            true
        } else {
            false
        }
    });
    if pending {
        if matches!(action, After::Close | After::Exit) {
            unsafe {
                ShowWindow(hwnd, SW_HIDE);
            }
        }
        return;
    }
    match action {
        After::Close | After::Exit => unsafe {
            DestroyWindow(hwnd);
        },
        After::Backup(restore) => {
            backup::begin(hwnd, restore);
        }
    }
}
