// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::*;
use altrun_probe::integration::Integration;
use std::path::Path;

pub(super) fn transact(
    launcher: &mpsc::Sender<Change>,
    owner: usize,
    next: &Preferences,
    save: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    static NEXT: AtomicUsize = AtomicUsize::new(1);
    let token = NEXT.fetch_add(1, Ordering::Relaxed) as u64;
    let queue = |change| {
        launcher
            .send(change)
            .ok()
            .and_then(|()| {
                (unsafe { PostMessageW(owner as HWND, UPDATED, 0, 0) } != 0).then_some(())
            })
            .ok_or_else(|| "ui.the_launcher_is_temporarily_unavailable".to_owned())
    };
    let (tx, rx) = mpsc::channel();
    queue(Change::Prepare(token, next.clone(), tx))?;
    rx.recv()
        .map_err(|_| "ui.the_launcher_is_temporarily_unavailable")??;
    let result = save();
    let (tx, rx) = mpsc::channel();
    queue(Change::Finish(token, result.is_ok(), tx))?;
    rx.recv()
        .map_err(|_| "ui.the_launcher_is_temporarily_unavailable")?;
    result
}

pub(super) fn persist(file: &mut TextFile, data: &Path, update: &Update) -> Result<(), String> {
    let flags = |p: &Preferences| (p.flag("AutoRun"), p.flag("AddToSendTo"));
    let old = flags(&update.previous);
    let new = flags(&update.next);
    if old == new {
        return file.save(&update.next.text());
    }
    let integration = Integration::for_data(data)?;
    let actual = integration.status()?;
    let startup = if old.0 != new.0 { new.0 } else { actual.0 };
    let send_to = if old.1 != new.1 { new.1 } else { actual.1 };
    file.save(&update.next.text())?;
    if (startup, send_to) != actual
        && let Err(error) = integration.apply(startup, send_to)
    {
        // Keep the running instance and the next startup on the same saved settings.
        let restore_system = integration.apply(actual.0, actual.1);
        let restore_file = file.save(&update.previous.text());
        let mut errors = vec![error];
        errors.extend(restore_system.err());
        errors.extend(restore_file.err());
        return Err(errors.join("\n"));
    }
    Ok(())
}
