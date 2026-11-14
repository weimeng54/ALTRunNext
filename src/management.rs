// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use crate::{
    Entry, launch,
    library::{EntryKind, OpenTarget, ParamType},
};
use std::path::{Path, PathBuf};
#[derive(Debug, PartialEq, Eq)]
pub enum Validation {
    Present,
    Missing,
    Unconfirmed(String),
    NonCommand,
}

#[derive(Debug, PartialEq, Eq)]
pub enum PathInspection {
    Present,
    Missing(MissingPath),
    Unconfirmed(String),
    NonCommand,
}

#[derive(Debug, PartialEq, Eq)]
pub struct MissingPath {
    pub path: String,
    pub target: RepairTarget,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RepairTarget {
    File,
    Folder,
    Program,
    OpenWith,
}

pub fn validate(entry: &Entry, base: &Path) -> Validation {
    match inspect_path(entry, base) {
        PathInspection::Present => Validation::Present,
        PathInspection::Missing(_) => Validation::Missing,
        PathInspection::Unconfirmed(reason) => Validation::Unconfirmed(reason),
        PathInspection::NonCommand => Validation::NonCommand,
    }
}

pub fn inspect_path(entry: &Entry, base: &Path) -> PathInspection {
    if entry.kind != EntryKind::Command {
        return PathInspection::NonCommand;
    }
    if let Some(target) = &entry.open_target {
        if matches!(target, OpenTarget::WebUrl) {
            return PathInspection::Unconfirmed("ui.network_or_system_location_not_checked".into());
        }
        let program = match target {
            OpenTarget::Document(Some(program)) => Some((program.as_str(), RepairTarget::OpenWith)),
            _ => None,
        };
        let kind = match target {
            OpenTarget::Path if !is_executable(Path::new(&entry.command)) => RepairTarget::Folder,
            OpenTarget::Document(_) => RepairTarget::File,
            _ => RepairTarget::Program,
        };
        for (path, target) in [Some((entry.command.as_str(), kind)), program]
            .into_iter()
            .flatten()
        {
            let result = inspect_local_path(Path::new(path), target);
            if result != PathInspection::Present {
                return result;
            }
        }
        return PathInspection::Present;
    }
    let Ok(directory) = launch::effective_directory(&entry.working_directory, base) else {
        return PathInspection::Unconfirmed("ui.invalid_working_directory".into());
    };
    if launch::is_network_path(&directory.to_string_lossy()) {
        return PathInspection::Unconfirmed("ui.network_working_directory_not_checked".into());
    }
    let Some(file) = launch::resolve_target(&entry.command, &directory)
        .ok()
        .or_else(|| unquoted_local_path(&entry.command).map(str::to_owned))
    else {
        return PathInspection::Unconfirmed("ui.command_requires_manual_review".into());
    };
    let lower = file.to_ascii_lowercase();
    if launch::is_network_path(&file)
        || file.contains("://")
        || lower.starts_with("shell:")
        || lower.starts_with("mailto:")
        || file.starts_with("::")
    {
        return PathInspection::Unconfirmed("ui.network_or_system_location_not_checked".into());
    }
    let path = Path::new(&file);
    if !path.is_absolute() && !file.contains(['\\', '/']) {
        return PathInspection::Unconfirmed("ui.resolved_by_windows".into());
    }
    let kind = if file.ends_with(['\\', '/']) || path.extension().is_none() {
        RepairTarget::Folder
    } else if is_program(path) {
        RepairTarget::Program
    } else {
        RepairTarget::File
    };
    inspect_local_path(&directory.join(path), kind)
}

fn unquoted_local_path(command: &str) -> Option<&str> {
    let path = command.trim();
    (valid_absolute_path(path)
        && path.contains(char::is_whitespace)
        && !path.contains(['%', '<', '>', '*', '?', '|'])
        && !path
            .split_whitespace()
            .skip(1)
            .any(|part| part.starts_with(['-', '/']) || Path::new(part).is_absolute()))
    .then_some(path)
}

fn inspect_local_path(path: &Path, target: RepairTarget) -> PathInspection {
    if launch::is_network_path(&path.to_string_lossy()) {
        return PathInspection::Unconfirmed("ui.network_location_not_checked".into());
    }
    match path.try_exists() {
        Ok(true) => PathInspection::Present,
        Ok(false) => PathInspection::Missing(MissingPath {
            path: path.to_string_lossy().into_owned(),
            target,
        }),
        Err(_) => PathInspection::Unconfirmed("ui.could_not_read_target_path".into()),
    }
}

pub fn replace_missing_path(
    entry: &Entry,
    target: RepairTarget,
    path: &Path,
) -> Result<Entry, String> {
    let path = absolute_path(path)?;
    let mut updated = entry.clone();
    if target == RepairTarget::OpenWith {
        let Some(OpenTarget::Document(current)) = updated.open_target.as_mut() else {
            return Err("ui.command_requires_manual_review".into());
        };
        *current = Some(path);
    } else {
        if target == RepairTarget::Program && !is_program(Path::new(&path)) {
            return Err("ui.choose_a_program_for_repair".into());
        }
        updated.command = if updated.open_target.is_some() {
            path
        } else {
            match launch::replace_command_target(&updated.command, &path) {
                Some(replaced) => replaced,
                None if unquoted_local_path(&updated.command).is_some() => path,
                None => return Err("ui.command_requires_manual_review".into()),
            }
        };
    }
    crate::library::validate(&updated)?;
    Ok(updated)
}

fn is_program(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| {
            ["exe", "com", "bat", "cmd", "lnk"]
                .iter()
                .any(|kind| ext.eq_ignore_ascii_case(kind))
        })
}

pub fn convert_path(entry: &mut Entry, base: &Path, relative: bool) -> Result<bool, String> {
    if entry.kind != EntryKind::Command
        || entry.open_target.is_some()
        || entry.command.contains('%')
        || entry.command.starts_with('@')
    {
        return Ok(false);
    }
    let directory = launch::effective_directory(&entry.working_directory, base)?;
    if launch::is_network_path(&directory.to_string_lossy()) {
        return Ok(false);
    }
    let request =
        launch::resolve_typed_in(&entry.command, None, ParamType::None, Some(&directory))?;
    let path = Path::new(&request.file);
    if !launch::explicit_path(&request.file) || launch::is_network_path(&request.file) {
        return Ok(false);
    }
    let converted = if relative {
        if !path.is_absolute() {
            return Ok(false);
        }
        PathBuf::from(".").join(
            path.strip_prefix(&directory)
                .map_err(|_| "ui.target_is_outside_the_working_directory_keeping")?,
        )
    } else {
        if path.is_absolute() {
            return Ok(false);
        }
        directory.join(path)
    };
    entry.command = format!(
        "\"{}\"{}{}",
        converted.display(),
        if request.parameters.is_empty() {
            ""
        } else {
            " "
        },
        request.parameters
    );
    Ok(true)
}
pub(crate) fn valid_absolute_path(path: &str) -> bool {
    Path::new(path).is_absolute() && !path.contains(['\0', '\r', '\n', '\t', '"'])
}

pub fn is_executable(path: &Path) -> bool {
    path.extension()
        .and_then(|s| s.to_str())
        .is_some_and(|s| s.eq_ignore_ascii_case("exe"))
}

pub fn absolute_path(path: &Path) -> Result<String, String> {
    let path = std::path::absolute(path).map_err(|e| e.to_string())?;
    let text = path.to_str().ok_or("ui.the_path_is_not_valid_unicode")?;
    if !valid_absolute_path(text) {
        return Err("ui.the_path_contains_invalid_characters".into());
    }
    Ok(text.into())
}

pub fn from_path(path: &Path) -> Result<Entry, String> {
    #[cfg(windows)]
    if is_shell_link(path) {
        let mut current = path.to_owned();
        let mut arguments = String::new();
        let mut working_directory = String::new();
        for _ in 0..8 {
            let Some(target) = crate::shell_link::read(&current)
                .filter(|target| valid_absolute_path(&target.path))
            else {
                break;
            };
            if arguments.is_empty() {
                arguments = target.arguments;
            }
            if working_directory.is_empty() && valid_absolute_path(&target.working_directory) {
                working_directory = target.working_directory;
            }
            current = target.path.into();
            if !is_shell_link(&current) {
                let mut entry = from_direct_path(&current)?;
                if !arguments.is_empty() {
                    entry.open_target = Some(OpenTarget::LinkArguments(arguments));
                }
                entry.working_directory = working_directory;
                crate::library::validate(&entry)?;
                return Ok(entry);
            }
        }
        return Err(crate::localization::message(
            "shortcut.unreadable_link",
            &[(
                "file",
                &path
                    .file_name()
                    .unwrap_or(path.as_os_str())
                    .to_string_lossy(),
            )],
        ));
    }
    from_direct_path(path)
}

#[cfg(windows)]
fn is_shell_link(path: &Path) -> bool {
    path.extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("lnk"))
}

fn from_direct_path(path: &Path) -> Result<Entry, String> {
    let text = absolute_path(path)?;
    let directory = path.is_dir();
    let name = if directory {
        path.file_name()
    } else {
        path.file_stem()
    }
    .or_else(|| path.file_name())
    .unwrap_or(path.as_os_str())
    .to_string_lossy()
    .into_owned();
    let document = !directory && !is_executable(path);
    let display_name = if document {
        path.file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(&name)
            .to_owned()
    } else {
        name.clone()
    };
    Ok(Entry {
        alias: name,
        name: display_name,
        command: text,
        frequency: 0,
        param_type: ParamType::None,
        id: crate::library::new_id(),
        working_directory: String::new(),
        open_target: Some(if document {
            OpenTarget::Document(None)
        } else {
            OpenTarget::Path
        }),
        kind: EntryKind::Command,
    })
}
