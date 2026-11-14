// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

//! Versioned archives for the portable profile. Archive names never become paths.
use crate::{library, preferences::Preferences, usage::Usage};
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io::Read,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

mod transaction;
mod zip;
pub use transaction::{apply, recover, recovery_pending};

const MANIFEST: &str = "manifest.json";
const SHORTCUTS: &str = "shortcuts.tsv";
const SETTINGS: &str = "settings.ini";
const HISTORY: &str = "state.tsv";
const BACKGROUND: &str = "background.bin";
const SOUND: &str = "notify.wav";
const LIMITS: &[(&str, usize)] = &[
    (MANIFEST, 8 * 1024),
    (SHORTCUTS, 16 * 1024 * 1024),
    (SETTINGS, 1024 * 1024),
    (HISTORY, 16 * 1024 * 1024),
    (BACKGROUND, 64 * 1024 * 1024),
    (SOUND, 32 * 1024 * 1024),
];

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Media {
    Default,
    Missing,
    Included,
}

#[derive(Debug, Deserialize, Serialize)]
struct Manifest {
    version: u8,
    created: u64,
    background: Media,
    sound: Media,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    background_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sound_name: Option<String>,
}

pub struct Backup {
    manifest: Manifest,
    count: usize,
    shortcuts: Vec<u8>,
    settings: Preferences,
    history: Vec<u8>,
    background: Option<Vec<u8>>,
    sound: Option<Vec<u8>>,
}

impl Backup {
    pub fn capture(data: &Path) -> Result<Self, String> {
        let shortcuts = required(data, SHORTCUTS)?;
        let count = crate::parse_entries(&library::decode(&shortcuts)?)?.len();
        let settings = Preferences::parse(&library::decode(
            &file(&data.with_extension("settings.ini"), SETTINGS)?.unwrap_or_default(),
        )?)?;
        let history = file(&data.with_extension("state.tsv"), HISTORY)?.unwrap_or_default();
        Usage::parse(&library::decode(&history)?)?;
        let base = data.parent().unwrap_or_else(|| Path::new("."));
        let background_name = settings.get("BGFileName");
        let (background_status, background) = media(base, &background_name, BACKGROUND)?;
        let sound_name = settings.get("PopupFileName");
        let (sound_status, sound) =
            if sound_name.eq_ignore_ascii_case(crate::preferences::DEFAULT_POPUP_SOUND) {
                (Media::Default, None)
            } else {
                media(base, &sound_name, SOUND)?
            };
        Ok(Self {
            manifest: Manifest {
                version: 1,
                created: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
                background: background_status,
                sound: sound_status,
                background_name: media_name(background_status, &background_name),
                sound_name: media_name(sound_status, &sound_name),
            },
            count,
            shortcuts,
            settings,
            history,
            background,
            sound,
        })
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let limit = LIMITS.iter().map(|(_, size)| size).sum::<usize>() + 4096;
        let bytes = read_bounded(path, limit)?.ok_or_else(invalid)?;
        let entries = zip::read(&bytes, LIMITS)?;
        let get = |name| entries.get(name).copied().ok_or_else(invalid);
        let manifest: Manifest = serde_json::from_slice(get(MANIFEST)?).map_err(|_| invalid())?;
        let background = entries.get(BACKGROUND).copied().map(Vec::from);
        let sound = entries.get(SOUND).copied().map(Vec::from);
        if manifest.version != 1
            || manifest.created > 253_402_300_799
            || (manifest.background == Media::Included) != background.is_some()
            || (manifest.sound == Media::Included) != sound.is_some()
        {
            return Err(invalid());
        }
        let settings = Preferences::import(&library::decode(get(SETTINGS)?)?)?;
        let shortcuts = get(SHORTCUTS)?.to_vec();
        let count = crate::parse_entries(&library::decode(&shortcuts)?)?.len();
        let history = get(HISTORY)?.to_vec();
        Usage::parse(&library::decode(&history)?)?;
        Ok(Self {
            manifest,
            count,
            shortcuts,
            settings,
            history,
            background,
            sound,
        })
    }

    pub fn write(&self, path: &Path) -> Result<(), String> {
        let manifest = serde_json::to_vec(&self.manifest).map_err(io_error)?;
        let settings = self.settings.text();
        let mut entries: Vec<(&str, &[u8])> = vec![
            (MANIFEST, &manifest),
            (SHORTCUTS, &self.shortcuts),
            (SETTINGS, settings.as_bytes()),
            (HISTORY, &self.history),
        ];
        if let Some(background) = &self.background {
            entries.push((BACKGROUND, background));
        }
        if let Some(sound) = &self.sound {
            entries.push((SOUND, sound));
        }
        transaction::write_atomic(path, |out| zip::write(out, &entries))
    }

    pub fn created(&self) -> u64 {
        self.manifest.created
    }
    pub fn count(&self) -> usize {
        self.count
    }
    pub fn background(&self) -> Media {
        self.manifest.background
    }
    pub fn sound(&self) -> Media {
        self.manifest.sound
    }

    pub fn background_name(&self) -> Option<&str> {
        self.manifest.background_name.as_deref()
    }

    pub fn sound_name(&self) -> Option<&str> {
        self.manifest.sound_name.as_deref()
    }
}

fn media_name(status: Media, path: &str) -> Option<String> {
    if status != Media::Included {
        return None;
    }
    Path::new(path)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
}

fn file(path: &Path, entry: &str) -> Result<Option<Vec<u8>>, String> {
    let limit = LIMITS.iter().find(|&&(name, _)| name == entry).unwrap().1;
    read_bounded(path, limit)
}

fn required(path: &Path, entry: &str) -> Result<Vec<u8>, String> {
    file(path, entry)?.ok_or_else(|| io_error(path.display()))
}

fn read_bounded(path: &Path, limit: usize) -> Result<Option<Vec<u8>>, String> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(io_error(err)),
    };
    if !file.metadata().map_err(io_error)?.is_file() {
        return Err(io_error(path.display()));
    }
    let mut bytes = Vec::new();
    file.take((limit + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(io_error)?;
    if bytes.len() > limit {
        return Err(crate::localization::message(
            "backup.too_large",
            &[("file", &path.to_string_lossy())],
        ));
    }
    Ok(Some(bytes))
}

fn media(base: &Path, name: &str, key: &str) -> Result<(Media, Option<Vec<u8>>), String> {
    if name.trim().is_empty() {
        return Ok((Media::Default, None));
    }
    let bytes = file(&base.join(name), key)?;
    Ok((
        if bytes.is_some() {
            Media::Included
        } else {
            Media::Missing
        },
        bytes,
    ))
}

fn invalid() -> String {
    "backup.invalid".into()
}

fn io_error(error: impl std::fmt::Display) -> String {
    crate::localization::message("backup.file_error", &[("detail", &error.to_string())])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn backup_moves_settings_history_and_media_to_another_portable_directory() {
        let root = std::env::temp_dir().join(format!("altrun-backup-{}", crate::library::new_id()));
        let original = root.join("old").join(SHORTCUTS);
        let target = root.join("new").join(SHORTCUTS);
        for path in [&original, &target] {
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, b"calc\tCalculator\tcalc.exe\t0\n").unwrap();
        }
        let mut prefs = Preferences::default();
        prefs.set("AutoRun", "1").unwrap();
        prefs.set("BGFileName", "image.png").unwrap();
        prefs.set("PopupFileName", "sound.wav").unwrap();
        fs::write(original.with_extension("settings.ini"), prefs.text()).unwrap();
        fs::write(original.with_file_name("image.png"), b"image").unwrap();
        fs::write(original.with_file_name("sound.wav"), b"sound").unwrap();
        let mut usage = Usage::default();
        let rows = crate::parse_entries("calc\tCalculator\tcalc.exe\t0\n").unwrap();
        usage.record(&rows[0].id, "calc", Some("123"), 10);
        fs::write(original.with_extension("state.tsv"), usage.text()).unwrap();
        let zip = root.join("backup.zip");
        Backup::capture(&original).unwrap().write(&zip).unwrap();
        let archive = Backup::load(&zip).unwrap();
        assert_eq!(archive.count(), 1);
        assert_eq!(archive.background(), Media::Included);
        assert_eq!(archive.sound(), Media::Included);
        apply(&target, archive, (false, true)).unwrap();
        let restored = Backup::capture(&target).unwrap();
        assert!(!restored.settings.flag("AutoRun"));
        assert!(restored.settings.flag("AddToSendTo"));
        assert_eq!(restored.background.as_deref(), Some(&b"image"[..]));
        assert_eq!(restored.sound.as_deref(), Some(&b"sound"[..]));
        assert_eq!(
            Usage::parse(&library::decode(&restored.history).unwrap()).unwrap(),
            usage
        );
        let _ = fs::remove_dir_all(root);
    }
}
