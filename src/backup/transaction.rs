// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::{Backup, io_error};
use std::{
    fs::{self, File, OpenOptions},
    io::{ErrorKind, Write},
    path::{Path, PathBuf},
};

const FILES: [&str; 3] = ["shortcuts.tsv", "settings.ini", "state.tsv"];

fn write_file(
    path: &Path,
    writer: impl FnOnce(&mut File) -> std::io::Result<()>,
) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .map_err(io_error)?;
    writer(&mut file)
        .and_then(|()| file.sync_all())
        .map_err(io_error)
}

#[cfg(windows)]
fn move_into_place(path: &Path, stage: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };
    let wide = |p: &Path| {
        p.as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<_>>()
    };
    let result = unsafe {
        MoveFileExW(
            wide(stage).as_ptr(),
            wide(path).as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if result == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
fn move_into_place(path: &Path, stage: &Path) -> std::io::Result<()> {
    fs::rename(stage, path)
}

pub(super) fn write_atomic(
    path: &Path,
    writer: impl FnOnce(&mut File) -> std::io::Result<()>,
) -> Result<(), String> {
    let stage = path.with_file_name(format!(".altrun-write-{}", crate::library::new_id()));
    let result =
        write_file(&stage, writer).and_then(|()| move_into_place(path, &stage).map_err(io_error));
    let _ = fs::remove_file(stage);
    result
}

fn target(data: &Path, index: usize) -> PathBuf {
    match index {
        0 => data.to_path_buf(),
        _ => data.with_extension(FILES[index]),
    }
}

fn journal(data: &Path) -> PathBuf {
    let mut name = data
        .file_name()
        .unwrap_or_else(|| std::ffi::OsStr::new("shortcuts.tsv"))
        .to_os_string();
    name.push(".restore");
    data.with_file_name(name)
}

fn regular(path: &Path, directory: bool) -> Result<bool, String> {
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(false),
        Err(err) => return Err(io_error(err)),
    };
    if (if directory {
        meta.is_dir()
    } else {
        meta.is_file()
    }) && !meta.file_type().is_symlink()
    {
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if meta.file_attributes()
                & windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT
                != 0
            {
                return Err(io_error(path.display()));
            }
        }
        return Ok(true);
    }
    Err(io_error(path.display()))
}

fn ready_marker(dir: &Path) -> PathBuf {
    dir.join("prepared")
}

fn clean(dir: &Path) -> Result<(), String> {
    // Detach the journal atomically so a crash during cleanup cannot cause a rollback.
    let done = dir.with_file_name(format!(
        ".altrun-restore-finished-{}",
        crate::library::new_id()
    ));
    move_into_place(&done, dir).map_err(io_error)?;
    let _ = fs::remove_dir_all(done);
    Ok(())
}

fn checkpoint(dir: &Path, data: &Path) -> Result<(), String> {
    for (i, file) in FILES.iter().enumerate() {
        let live = target(data, i);
        if regular(&live, false)? {
            let old = super::required(&live, file)?;
            write_file(&dir.join(format!("old-{i}")), |f| f.write_all(&old))?;
        } else {
            write_file(&dir.join(format!("absent-{i}")), |_| Ok(()))?;
        }
    }
    Ok(())
}

pub fn recover(data: &Path) -> Result<(), String> {
    let dir = journal(data);
    if !regular(&dir, true)? {
        return Ok(());
    }
    if regular(&ready_marker(&dir), false)? {
        if regular(&dir.join("committed"), false)? {
            return clean(&dir);
        }
        // Validate the entire journal before changing any live file.
        let mut originals = Vec::new();
        for (i, file) in FILES.iter().enumerate() {
            let old = dir.join(format!("old-{i}"));
            let absent = dir.join(format!("absent-{i}"));
            let exists = regular(&old, false)?;
            if exists == regular(&absent, false)? {
                return Err(super::invalid());
            }
            originals.push(if exists {
                Some(super::required(&old, file)?)
            } else {
                None
            });
        }
        for (i, old) in originals.into_iter().enumerate() {
            // A successful replacement consumes its staged file atomically.
            if !regular(&dir.join(format!("pending-{i}")), false)?
                || regular(&dir.join(format!("next-{i}")), false)?
            {
                continue;
            }
            let live = target(data, i);
            let actual = if regular(&live, false)? {
                super::file(&live, FILES[i])?
            } else {
                None
            };
            if old != actual {
                if let Some(bytes) = old {
                    write_atomic(&live, |f| f.write_all(&bytes))?;
                } else {
                    fs::remove_file(&live).map_err(io_error)?;
                }
            }
        }
        let media_name = super::required(&dir.join("media"), super::MANIFEST)?;
        let name = std::str::from_utf8(&media_name).map_err(|_| super::invalid())?;
        if let Some(suffix) = name.strip_prefix("altrun-media-")
            && !suffix.is_empty()
            && suffix
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        {
            let media = data.with_file_name(name);
            if regular(&media, true)? {
                for name in [super::BACKGROUND, super::SOUND] {
                    let path = media.join(name);
                    if regular(&path, false)? {
                        fs::remove_file(path).map_err(io_error)?;
                    }
                }
                fs::remove_dir(media).map_err(io_error)?;
            }
        }
    }
    clean(&dir)
}

pub fn recovery_pending(data: &Path) -> bool {
    let dir = journal(data);
    dir.join("prepared").exists() && !dir.join("committed").exists()
}

pub fn apply(data: &Path, backup: Backup, integrations: (bool, bool)) -> Result<PathBuf, String> {
    let dir = journal(data);
    // Handle an interrupted restore before creating another journal.
    recover(data)?;
    let current = Backup::capture(data)?;
    let current_zip = data.with_file_name(format!(
        "ALTRunNext-before-restore-{}.zip",
        crate::library::new_id()
    ));
    current.write(&current_zip)?;
    fs::create_dir(&dir).map_err(io_error)?;
    let media_name = format!("altrun-media-{}", crate::library::new_id());
    let result = (|| {
        checkpoint(&dir, data)?;
        write_file(&dir.join("media"), |file| {
            file.write_all(media_name.as_bytes())
        })?;
        let mut settings = backup.settings;
        settings.set("AutoRun", if integrations.0 { "1" } else { "0" })?;
        settings.set("AddToSendTo", if integrations.1 { "1" } else { "0" })?;
        if backup.background.is_some() {
            let path = Path::new(&media_name).join(super::BACKGROUND);
            settings.set("BGFileName", &path.to_string_lossy())?;
        } else {
            settings.set("BGFileName", "")?;
            settings.set("ShowSkin", "0")?;
        }
        let popup = if backup.sound.is_some() {
            Path::new(&media_name).join(super::SOUND)
        } else {
            crate::preferences::DEFAULT_POPUP_SOUND.into()
        };
        settings.set("PopupFileName", &popup.to_string_lossy())?;
        let text = settings.text();
        let updates = [
            backup.shortcuts.as_slice(),
            text.as_bytes(),
            backup.history.as_slice(),
        ];
        for (i, bytes) in updates.into_iter().enumerate() {
            write_file(&dir.join(format!("next-{i}")), |f| f.write_all(bytes))?;
        }
        write_atomic(&ready_marker(&dir), |_| Ok(()))?;
        let (bg, sound) = (backup.background, backup.sound);
        if bg.is_some() || sound.is_some() {
            let destination = data.with_file_name(&media_name);
            fs::create_dir(&destination).map_err(io_error)?;
            if let Some(bytes) = bg {
                write_file(&destination.join(super::BACKGROUND), |f| {
                    f.write_all(&bytes)
                })?;
            }
            if let Some(bytes) = sound {
                write_file(&destination.join(super::SOUND), |f| f.write_all(&bytes))?;
            }
        }
        for (i, file) in FILES.iter().enumerate() {
            let live = target(data, i);
            let old = super::file(&dir.join(format!("old-{i}")), file)?;
            let actual = if regular(&live, false)? {
                super::file(&live, file)?
            } else {
                None
            };
            if actual != old {
                return Err("backup.changed_since_prepare".into());
            }
            write_atomic(&dir.join(format!("pending-{i}")), |_| Ok(()))?;
            move_into_place(&live, &dir.join(format!("next-{i}"))).map_err(io_error)?;
        }
        write_atomic(&dir.join("committed"), |_| Ok(()))
    })();
    match result {
        Ok(()) => {
            let _ = clean(&dir);
            Ok(current_zip)
        }
        Err(error) => match recover(data) {
            Ok(()) => Err(error),
            Err(rollback) => Err(crate::localization::message(
                "backup.recovery_pending",
                &[(
                    "detail",
                    &format!(
                        "{} · {}",
                        crate::localization::tr(&error),
                        crate::localization::tr(&rollback)
                    ),
                )],
            )),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interrupted_restore_recovers_existing_and_absent_files() {
        let root =
            std::env::temp_dir().join(format!("altrun-recovery-{}", crate::library::new_id()));
        fs::create_dir(&root).unwrap();
        let data = root.join("shortcuts.tsv");
        fs::write(&data, b"original shortcuts").unwrap();
        let dir = journal(&data);
        fs::create_dir(&dir).unwrap();
        checkpoint(&dir, &data).unwrap();
        fs::write(dir.join("media"), b"altrun-media-test").unwrap();
        fs::write(ready_marker(&dir), "").unwrap();
        fs::write(dir.join("pending-0"), "").unwrap();
        fs::write(dir.join("pending-1"), "").unwrap();
        // The third replacement was about to start when the process stopped.
        fs::write(dir.join("pending-2"), "").unwrap();
        fs::write(dir.join("next-2"), "replacement state").unwrap();
        let state = data.with_extension("state.tsv");
        fs::write(&state, b"external state").unwrap();
        fs::write(&data, b"replacement shortcuts").unwrap();
        fs::write(data.with_extension("settings.ini"), b"replacement settings").unwrap();
        let media = root.join("altrun-media-test");
        fs::create_dir(&media).unwrap();
        recover(&data).unwrap();
        assert_eq!(fs::read(&data).unwrap(), b"original shortcuts");
        assert!(!data.with_extension("settings.ini").exists());
        assert_eq!(fs::read(&state).unwrap(), b"external state");
        assert!(!media.exists() && !dir.exists());
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(windows)]
    #[test]
    fn locked_settings_roll_back_shortcuts_without_touching_the_lock() {
        use std::os::windows::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::{FILE_SHARE_READ, FILE_SHARE_WRITE};

        let root = std::env::temp_dir().join(format!("altrun-locked-{}", crate::library::new_id()));
        fs::create_dir(&root).unwrap();
        let source = root.join("source.tsv");
        let data = root.join("shortcuts.tsv");
        let settings = data.with_extension("settings.ini");
        let shortcuts = b"calc\tCalculator\tcalc.exe\t0\n";
        let preferences = crate::preferences::Preferences::default().text();
        fs::write(&source, b"np\tNotepad\tnotepad.exe\t0\n").unwrap();
        fs::write(&data, shortcuts).unwrap();
        fs::write(&settings, &preferences).unwrap();
        let lock = OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
            .open(&settings)
            .unwrap();

        let backup = Backup::capture(&source).unwrap();
        assert!(apply(&data, backup, (false, false)).is_err());
        assert!(!journal(&data).exists());
        assert_eq!(fs::read(&data).unwrap(), shortcuts);
        assert_eq!(fs::read_to_string(&settings).unwrap(), preferences);
        assert!(!data.with_extension("state.tsv").exists());
        drop(lock);
        let _ = fs::remove_dir_all(root);
    }
}
