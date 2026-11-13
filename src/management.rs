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
pub fn validate(entry: &Entry, base: &Path) -> Validation {
    if entry.kind != EntryKind::Command {
        return Validation::NonCommand;
    }
    if let Some(target) = &entry.open_target {
        let program = match target {
            OpenTarget::Document(Some(program)) => Some(program.as_str()),
            _ => None,
        };
        for path in [Some(entry.command.as_str()), program]
            .into_iter()
            .flatten()
        {
            if launch::is_network_path(path) {
                return Validation::Unconfirmed("ui.network_location_not_checked".into());
            }
            match Path::new(path).try_exists() {
                Ok(true) => {}
                Ok(false) => return Validation::Missing,
                Err(_) => return Validation::Unconfirmed("ui.could_not_read_target_path".into()),
            }
        }
        return Validation::Present;
    }
    if entry.command.contains('%') || entry.command.starts_with(['@']) {
        return Validation::Unconfirmed("ui.contains_placeholders_or_a_window_mode".into());
    }
    let Ok(directory) = launch::effective_directory(&entry.working_directory, base) else {
        return Validation::Unconfirmed("ui.invalid_working_directory".into());
    };
    if launch::is_network_path(&directory.to_string_lossy()) {
        return Validation::Unconfirmed("ui.network_working_directory_not_checked".into());
    }
    let Ok(request) =
        launch::resolve_typed_in(&entry.command, None, ParamType::None, Some(&directory))
    else {
        return Validation::Unconfirmed("ui.command_requires_manual_review".into());
    };
    if launch::is_network_path(&request.file)
        || request.file.contains("://")
        || request.file.starts_with("shell:")
        || request.file.starts_with("::")
    {
        return Validation::Unconfirmed("ui.network_or_system_location_not_checked".into());
    }
    let path = Path::new(&request.file);
    if !path.is_absolute() && !request.file.contains(['\\', '/']) {
        return Validation::Unconfirmed("ui.resolved_by_windows".into());
    }
    match directory.join(path).try_exists() {
        Ok(true) => Validation::Present,
        Ok(false) => Validation::Missing,
        Err(_) => Validation::Unconfirmed("ui.could_not_read_target_path".into()),
    }
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
