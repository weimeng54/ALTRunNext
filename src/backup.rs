// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

//! Versioned archives for the portable profile. Archive names never become paths.
use crate::{Entry, library, preferences::Preferences, usage::Usage};
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io::Read,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

mod selection;
mod transaction;
mod zip;
pub use selection::{Contents, RestoreItems, RestoreSelection};
pub use transaction::{apply, apply_selected, recover, recovery_pending};

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    contents: Option<Contents>,
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
        Self::capture_selected(data, Contents::ALL)
    }

    pub fn capture_selected(data: &Path, contents: Contents) -> Result<Self, String> {
        let read = |selected, path: &Path, key| {
            if selected { file(path, key) } else { Ok(None) }
        };
        Self::from_profile(
            data,
            [
                read(contents.items, data, SHORTCUTS)?,
                read(
                    contents.settings,
                    &data.with_extension("settings.ini"),
                    SETTINGS,
                )?,
                read(contents.history, &data.with_extension("state.tsv"), HISTORY)?,
            ],
            contents,
        )
    }

    fn from_profile(
        data: &Path,
        profile: [Option<Vec<u8>>; 3],
        contents: Contents,
    ) -> Result<Self, String> {
        if !contents.any() {
            return Err(invalid());
        }
        let [shortcuts, settings, history] = profile;
        let shortcuts = if contents.items {
            shortcuts.ok_or_else(|| io_error(data.display()))?
        } else {
            Vec::new()
        };
        let count = entries(&shortcuts)?.len();
        let settings = if contents.settings {
            Preferences::parse(&library::decode(&settings.unwrap_or_default())?)?
        } else {
            Preferences::default()
        };
        let history = history.unwrap_or_default();
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
                version: if contents == Contents::ALL { 1 } else { 2 },
                created: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
                background: background_status,
                sound: sound_status,
                background_name: media_name(background_status, &background_name),
                sound_name: media_name(sound_status, &sound_name),
                contents: (contents != Contents::ALL).then_some(contents),
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
        let files = zip::read(&bytes, LIMITS)?;
        let get = |name| files.get(name).copied().ok_or_else(invalid);
        let manifest: Manifest = serde_json::from_slice(get(MANIFEST)?).map_err(|_| invalid())?;
        let background = files.get(BACKGROUND).copied().map(Vec::from);
        let sound = files.get(SOUND).copied().map(Vec::from);
        let contents = match (manifest.version, manifest.contents) {
            (1, None) => Contents::ALL,
            (2, Some(contents)) if contents.any() => contents,
            _ => return Err(invalid()),
        };
        if (contents.items != files.contains_key(SHORTCUTS))
            || (contents.settings != files.contains_key(SETTINGS))
            || (contents.history != files.contains_key(HISTORY))
            || (!contents.settings
                && (manifest.background != Media::Default || manifest.sound != Media::Default))
            || manifest.created > 253_402_300_799
            || (manifest.background == Media::Included) != background.is_some()
            || (manifest.sound == Media::Included) != sound.is_some()
        {
            return Err(invalid());
        }
        let settings = if contents.settings {
            Preferences::import(&library::decode(get(SETTINGS)?)?)?
        } else {
            Preferences::default()
        };
        let shortcuts = files.get(SHORTCUTS).copied().unwrap_or_default().to_vec();
        let count = entries(&shortcuts)?.len();
        let history = files.get(HISTORY).copied().unwrap_or_default().to_vec();
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

    /// Opens a backup ZIP or a settings INI previously exported by ALTRunNext.
    pub fn load_restore_source(path: &Path) -> Result<Self, String> {
        let mut signature = [0; 4];
        let size = File::open(path)
            .map_err(io_error)?
            .read(&mut signature)
            .map_err(io_error)?;
        if signature[..size].starts_with(b"PK")
            || path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"))
        {
            return Self::load(path);
        }
        let settings = Preferences::import(
            &library::decode(&required(path, SETTINGS)?)
                .map_err(|_| crate::preferences::SETTINGS_IMPORT_FORMAT_ERROR.to_owned())?,
        )?;
        let missing = |name: &str| {
            if name.trim().is_empty() {
                Media::Default
            } else {
                Media::Missing
            }
        };
        let sound = settings.get("PopupFileName");
        Ok(Self {
            manifest: Manifest {
                version: 2,
                created: 0,
                background: missing(&settings.get("BGFileName")),
                sound: if sound.eq_ignore_ascii_case(crate::preferences::DEFAULT_POPUP_SOUND) {
                    Media::Default
                } else {
                    missing(&sound)
                },
                background_name: None,
                sound_name: None,
                contents: Some(Contents {
                    settings: true,
                    ..Contents::default()
                }),
            },
            count: 0,
            shortcuts: Vec::new(),
            settings,
            history: Vec::new(),
            background: None,
            sound: None,
        })
    }

    pub fn write(&self, path: &Path) -> Result<(), String> {
        let manifest = serde_json::to_vec(&self.manifest).map_err(io_error)?;
        let settings = self.settings.text();
        let contents = self.contents();
        let mut entries: Vec<(&str, &[u8])> = vec![(MANIFEST, &manifest)];
        for (selected, name, bytes) in [
            (contents.items, SHORTCUTS, self.shortcuts.as_slice()),
            (contents.settings, SETTINGS, settings.as_bytes()),
            (contents.history, HISTORY, self.history.as_slice()),
        ] {
            if selected {
                entries.push((name, bytes));
            }
        }
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

    pub fn saved_launch_items(path: &Path) -> Result<Vec<Entry>, String> {
        entries(&required(path, SHORTCUTS)?)
    }
    pub fn contents(&self) -> Contents {
        self.manifest.contents.unwrap_or(Contents::ALL)
    }

    pub fn launch_items(&self) -> Result<Vec<Entry>, String> {
        entries(&self.shortcuts)
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

fn entries(bytes: &[u8]) -> Result<Vec<Entry>, String> {
    crate::parse_entries(&library::decode(bytes)?)
}

fn media_name(status: Media, path: &str) -> Option<String> {
    if status != Media::Included {
        return None;
    }
    Path::new(path)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
}

fn entry_limit(entry: &str) -> usize {
    LIMITS.iter().find(|&&(name, _)| name == entry).unwrap().1
}

fn validate_size(path: &Path, size: usize, limit: usize) -> Result<(), String> {
    if size > limit {
        Err(crate::localization::message(
            "backup.too_large",
            &[("file", &path.to_string_lossy())],
        ))
    } else {
        Ok(())
    }
}

fn file(path: &Path, entry: &str) -> Result<Option<Vec<u8>>, String> {
    read_bounded(path, entry_limit(entry))
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
    validate_size(path, bytes.len(), limit)?;
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
    fn selected_archive_appends_in_order_and_keeps_unselected_files() {
        let root = std::env::temp_dir().join(library::new_id());
        fs::create_dir(&root).unwrap();
        let source = root.join("source.tsv");
        let target = root.join("local.tsv");
        let archive = root.join("items.zip");
        fs::write(
            &source,
            "CALC\tConflicting alias\tmspaint.exe\t0\nnp\tNotepad\tnotepad.exe\t0\n",
        )
        .unwrap();
        fs::write(
            source.with_extension("state.tsv"),
            "invalid unselected state",
        )
        .unwrap();
        fs::write(&target, "calc\tCalculator\tcalc.exe\t0\n").unwrap();
        let settings = Preferences::default().text();
        fs::write(target.with_extension("settings.ini"), &settings).unwrap();
        let before = Backup::saved_launch_items(&target).unwrap();
        let mut history = Usage::default();
        history.record(&before[0].id, "cl", None, 50);
        fs::write(target.with_extension("state.tsv"), history.text()).unwrap();
        let contents = Contents {
            items: true,
            ..Contents::default()
        };
        Backup::capture_selected(&source, contents)
            .unwrap()
            .write(&archive)
            .unwrap();
        assert_eq!(Backup::load(&archive).unwrap().contents(), contents);
        let pick = RestoreSelection {
            contents,
            items: RestoreItems::Append,
        };
        assert!(
            apply_selected(
                &target,
                Backup::load(&archive).unwrap(),
                (true, true),
                pick,
                &[],
            )
            .is_err()
        );
        let previous = apply_selected(
            &target,
            Backup::load(&archive).unwrap(),
            (true, true),
            pick,
            &before,
        )
        .unwrap();
        assert_eq!(
            Backup::load(&previous).unwrap().launch_items().unwrap(),
            before
        );
        assert_eq!(
            Backup::saved_launch_items(&target)
                .unwrap()
                .iter()
                .map(|entry| entry.alias.as_str())
                .collect::<Vec<_>>(),
            ["calc", "np"],
        );
        assert_eq!(
            fs::read_to_string(target.with_extension("settings.ini")).unwrap(),
            settings
        );
        assert_eq!(
            fs::read_to_string(target.with_extension("state.tsv")).unwrap(),
            history.text()
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn unified_restore_accepts_altrunnext_ini_and_rejects_original_ini() {
        let root = std::env::temp_dir().join(library::new_id());
        fs::create_dir(&root).unwrap();
        let file = root.join("settings.ini");
        let mut prefs = Preferences::default();
        prefs.set("BGFileName", "example.png").unwrap();
        fs::write(&file, prefs.text()).unwrap();
        let backup = Backup::load_restore_source(&file).unwrap();
        assert_eq!(
            backup.contents(),
            Contents {
                settings: true,
                ..Contents::default()
            }
        );
        assert_eq!(backup.background(), Media::Missing);
        assert!(backup.background.is_none());
        fs::write(&file, "[Config]\nAutoRun=1\n").unwrap();
        assert_eq!(
            Backup::load_restore_source(&file).err().unwrap(),
            crate::preferences::SETTINGS_IMPORT_FORMAT_ERROR,
        );
        let _ = fs::remove_dir_all(root);
    }

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
        let previous = apply(&target, archive, (false, true)).unwrap();
        let previous = Backup::load(&previous).unwrap();
        assert_eq!(previous.shortcuts, b"calc\tCalculator\tcalc.exe\t0\n");
        assert_eq!(previous.background(), Media::Default);
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
