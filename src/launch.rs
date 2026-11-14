// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

//! Resolve an explicitly selected command without implicitly invoking a shell.
use std::path::{Path, PathBuf};

mod parser;
mod template;

pub use parser::replace_command_target;
pub(crate) use parser::{normalize_original_command, resolve_target};
pub(crate) use template::resolve_typed_in;
use template::{encode_parameter, expand_environment};
pub use template::{has_context, has_parameter, resolve, resolve_context, resolve_typed};

#[derive(Debug, PartialEq, Eq)]
pub struct LaunchRequest {
    pub file: String,
    pub parameters: String,
}
#[derive(Clone, Debug, Default)]
pub struct LaunchContext {
    pub clipboard: Option<String>,
    pub window_id: usize,
    pub window_title: String,
    pub window_class: String,
    pub base_directory: String,
    pub base_is_working_directory: bool,
    pub allow_dynamic_target: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShowMode {
    Normal,
    Maximized,
    Minimized,
    Hidden,
}
pub(crate) fn effective_directory(explicit: &str, base: &Path) -> Result<PathBuf, String> {
    let directory = if explicit.trim().is_empty() {
        base.to_path_buf()
    } else {
        base.join(expand_environment(explicit.trim(), false)?)
    };
    if directory.to_string_lossy().contains(['\0', '\r', '\n']) {
        return Err("ui.the_working_directory_contains_invalid_characters".into());
    }
    std::path::absolute(directory).map_err(|e| e.to_string())
}

pub(crate) fn is_network_path(path: &str) -> bool {
    let unc = path.starts_with(r"\\") || path.starts_with("//");
    unc && !path.starts_with(r"\\?\") || path.to_ascii_lowercase().starts_with(r"\\?\unc\")
}

pub(crate) fn explicit_path(file: &str) -> bool {
    file.contains(['\\', '/'])
        && !file.contains("://")
        && !file.to_ascii_lowercase().starts_with("shell:")
        && !file.starts_with("::")
}

pub fn resolve_entry(
    entry: &crate::Entry,
    parameter: Option<&str>,
    context: &LaunchContext,
) -> Result<(LaunchRequest, ShowMode, Option<String>), String> {
    use crate::library::OpenTarget;
    let Some(target) = &entry.open_target else {
        return resolve_context(
            &entry.command,
            parameter,
            entry.param_type,
            &entry.working_directory,
            context,
        );
    };
    crate::library::validate(entry)?;
    if matches!(target, OpenTarget::WebUrl) {
        return Ok((
            LaunchRequest {
                file: entry.command.clone(),
                parameters: String::new(),
            },
            ShowMode::Normal,
            None,
        ));
    }
    let (file, parameters) = match target {
        OpenTarget::Path => {
            let parameters = if entry.param_type == crate::library::ParamType::None {
                String::new()
            } else {
                encode_parameter(entry.param_type, parameter.ok_or("ui.enter_a_parameter")?)?
            };
            (entry.command.clone(), parameters)
        }
        OpenTarget::Document(None) => (entry.command.clone(), String::new()),
        OpenTarget::WebUrl => unreachable!(),
        OpenTarget::LinkArguments(arguments) => (entry.command.clone(), arguments.clone()),
        OpenTarget::Document(Some(program)) => {
            if !Path::new(program).is_file() {
                return Err("ui.the_selected_app_could_not_be_found".into());
            }
            // A Windows file name cannot contain a quote. A trailing backslash
            // must be doubled before the closing quote in a Windows argument.
            let tail = entry.command.len() - entry.command.trim_end_matches('\\').len();
            (
                program.clone(),
                format!("\"{}{}\"", entry.command, "\\".repeat(tail)),
            )
        }
    };
    let directory = (!entry.working_directory.is_empty())
        .then(|| Path::new(&entry.working_directory))
        .or_else(|| {
            crate::management::is_executable(Path::new(&file))
                .then(|| Path::new(&file).parent())
                .flatten()
        })
        .filter(|path| {
            *path != Path::new(&context.base_directory)
                && std::env::current_exe()
                    .ok()
                    .and_then(|exe| exe.parent().map(Path::to_path_buf))
                    .is_none_or(|owner| !path.starts_with(owner))
        })
        .map(|path| path.to_string_lossy().into_owned());
    Ok((
        LaunchRequest { file, parameters },
        ShowMode::Normal,
        directory,
    ))
}
