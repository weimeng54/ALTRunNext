// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

//! Resolve an explicitly selected command without implicitly invoking a shell.
use std::path::{Path, PathBuf};

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
pub fn has_context(command: &str) -> bool {
    ["c", "wd", "wt", "wc"].iter().any(|token| {
        let marker = format!("%{token}");
        command.match_indices(&marker).any(|(index, _)| {
            !command[index + marker.len()..]
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
        })
    })
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

pub fn resolve_context(
    command: &str,
    parameter: Option<&str>,
    kind: crate::library::ParamType,
    working_directory: &str,
    context: &LaunchContext,
) -> Result<(LaunchRequest, ShowMode, Option<String>), String> {
    use crate::library::ParamType;
    let command = command.trim();
    let (command, mode) = if let Some(s) = command.strip_prefix("@+") {
        (s.trim_start(), ShowMode::Maximized)
    } else if let Some(s) = command.strip_prefix("@-") {
        (s.trim_start(), ShowMode::Minimized)
    } else if let Some(s) = command.strip_prefix('@') {
        (s.trim_start(), ShowMode::Hidden)
    } else {
        (command, ShowMode::Normal)
    };
    if command.contains(['\u{e000}', '\u{e001}'])
        || parameter.is_some_and(|p| p.contains(['\u{e000}', '\u{e001}']))
    {
        return Err("ui.the_command_contains_reserved_characters".into());
    }
    let mut values = vec![
        context.clipboard.clone().unwrap_or_default(),
        context.window_id.to_string(),
        context.window_title.clone(),
        context.window_class.clone(),
    ];
    let mut tokens = vec!["c", "wd", "wt", "wc"];
    if context.allow_dynamic_target && has_parameter(command) {
        if kind == ParamType::None {
            return Err("ui.choose_an_input_format_for_the_parameter".into());
        }
        values.push(parameter.ok_or("ui.enter_a_parameter")?.to_owned());
        tokens.push("p");
    }
    let mut masked = command.to_owned();
    let mut had_context = false;
    for (i, token) in tokens.iter().enumerate() {
        let brace = format!("{{%{token}}}");
        let bare = format!("%{token}");
        let marker = format!("\u{e000}{i}\u{e001}");
        if masked.contains(&brace)
            || masked.match_indices(&bare).any(|(index, _)| {
                !masked[index + bare.len()..]
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
            })
        {
            had_context = true;
            if i == 0 && context.clipboard.is_none() {
                return Err("ui.the_clipboard_has_no_text".into());
            }
            masked = masked.replace(&brace, &marker);
            let mut result = String::new();
            let mut rest = masked.as_str();
            while let Some(index) = rest.find(&bare) {
                result.push_str(&rest[..index]);
                let tail = &rest[index + bare.len()..];
                if tail
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
                {
                    result.push_str(&bare);
                } else {
                    result.push_str(&marker);
                }
                rest = tail;
            }
            result.push_str(rest);
            masked = result;
        }
    }
    let actual_kind =
        if values.len() == 5 || (had_context && !has_parameter(command) && parameter.is_none()) {
            ParamType::Auto
        } else {
            kind
        };
    let directory = if working_directory.trim().is_empty() && context.base_directory.is_empty() {
        None
    } else {
        Some(effective_directory(
            working_directory,
            Path::new(&context.base_directory),
        )?)
    };
    let mut result = resolve_typed_in(&masked, parameter, actual_kind, directory.as_deref())?;
    if result.file.contains('\u{e000}')
        && !context.allow_dynamic_target
        && !result.file.to_ascii_lowercase().starts_with("https://")
        && !result.file.to_ascii_lowercase().starts_with("http://")
    {
        return Err("ui.dynamic_input_cannot_replace_the_program_path".into());
    }
    let substitute = |text: &str| -> Result<String, String> {
        let mut out = String::new();
        let mut rest = text;
        while let Some(start) = rest.find('\u{e000}') {
            out.push_str(&rest[..start]);
            let tail = &rest[start + '\u{e000}'.len_utf8()..];
            let end = tail
                .find('\u{e001}')
                .ok_or("ui.invalid_context_placeholder")?;
            let i = tail[..end]
                .parse::<usize>()
                .map_err(|_| "ui.invalid_context_placeholder")?;
            let value = values.get(i).ok_or("ui.invalid_context_placeholder")?;
            if value.contains('\0')
                || (value.contains(['\r', '\n'])
                    && !matches!(kind, ParamType::AnsiUrl | ParamType::Utf8Url))
            {
                return Err("ui.copy_one_line_of_text_without_null".into());
            }
            out.push_str(&match kind {
                ParamType::AnsiUrl => percent_encode(&crate::textcodec::ansi_bytes(value)?),
                ParamType::Utf8Url => percent_encode(value.as_bytes()),
                _ => value.clone(),
            });
            rest = &tail[end + '\u{e001}'.len_utf8()..];
        }
        out.push_str(rest);
        Ok(out)
    };
    result.file = substitute(&result.file)?;
    result.parameters = substitute(&result.parameters)?;
    if let Some(base) = &directory {
        let target = Path::new(&result.file);
        if explicit_path(&result.file) && !target.is_absolute() {
            result.file = base.join(target).to_string_lossy().into_owned();
        } else if !result.file.contains(['/', '\\', ':'])
            && !is_network_path(&base.to_string_lossy())
            && matches!(
                target.components().next(),
                Some(std::path::Component::Normal(_))
            )
        {
            // Resolve local bare names before assigning a different directory to the child.
            let local = base.join(target);
            if local.exists() {
                result.file = local.to_string_lossy().into_owned();
            } else if local.extension().is_none() && local.with_extension("exe").is_file() {
                result.file = local.with_extension("exe").to_string_lossy().into_owned();
            }
        }
    }
    let has_working_directory =
        !working_directory.trim().is_empty() || context.base_is_working_directory;
    Ok((
        result,
        mode,
        directory
            .filter(|_| has_working_directory)
            .map(|d| d.to_string_lossy().into_owned()),
    ))
}

pub fn has_parameter(command: &str) -> bool {
    let mut rest = command;
    while let Some(index) = rest.find('%') {
        rest = &rest[index..];
        if parameter_marker(rest) {
            return true;
        }
        if let Some(end) = rest[1..].find('%') {
            rest = &rest[end + 2..];
        } else {
            rest = &rest[1..];
        }
    }
    false
}

pub fn resolve_typed(
    command: &str,
    parameter: Option<&str>,
    kind: crate::library::ParamType,
) -> Result<LaunchRequest, String> {
    resolve_typed_in(command, parameter, kind, None)
}

pub(crate) fn resolve_typed_in(
    command: &str,
    parameter: Option<&str>,
    kind: crate::library::ParamType,
    base: Option<&Path>,
) -> Result<LaunchRequest, String> {
    use crate::library::ParamType;
    let marked = has_parameter(command);
    if kind == ParamType::None && marked {
        return Err("ui.choose_an_input_format_for_the_parameter".into());
    }
    let active = kind.needs_input(command);
    let encoded = if active {
        let value = parameter.ok_or("ui.enter_a_parameter")?;
        Some(encode_parameter(kind, value)?)
    } else {
        None
    };
    let normalized = command.replace("{%p}", "%p");
    let mut result = resolve_base(&normalized, encoded.as_deref(), base)?;
    if active && !marked {
        let value = encoded.as_deref().unwrap_or("");
        if result.file.to_ascii_lowercase().starts_with("http://")
            || result.file.to_ascii_lowercase().starts_with("https://")
        {
            result.file.push_str(value);
        } else {
            if !result.parameters.is_empty() && !value.is_empty() {
                result.parameters.push(' ');
            }
            result.parameters.push_str(value);
        }
    }
    Ok(result)
}
fn encode_parameter(kind: crate::library::ParamType, value: &str) -> Result<String, String> {
    use crate::library::ParamType;
    if value.contains(['\0', '\r', '\n']) {
        return Err("ui.the_command_or_arguments_contain_invalid_control".into());
    }
    Ok(match kind {
        ParamType::AnsiUrl => percent_encode(&crate::textcodec::ansi_bytes(value)?),
        ParamType::Utf8Url => percent_encode(value.as_bytes()),
        _ => value.to_owned(),
    })
}

fn percent_encode(bytes: &[u8]) -> String {
    let mut result = String::new();
    for &byte in bytes {
        if byte.is_ascii_alphabetic() || b"*@._-".contains(&byte) {
            result.push(byte as char);
        } else if byte == b' ' {
            result.push('+');
        } else {
            result.push_str(&format!("%{byte:02X}"));
        }
    }
    result
}

pub fn resolve(command: &str, parameter: Option<&str>) -> Result<LaunchRequest, String> {
    resolve_typed(command, parameter, crate::library::ParamType::Auto)
}

fn is_web_address(command: &str) -> bool {
    let lower = command.to_ascii_lowercase();
    ["http://", "https://", "mailto:"]
        .iter()
        .any(|prefix| lower.starts_with(prefix))
}

fn split_target<'a>(
    command: &'a str,
    base: Option<&Path>,
    check_files: bool,
) -> Result<(&'a str, &'a str), &'static str> {
    if is_web_address(command)
        || command.starts_with("::")
        || command.to_ascii_lowercase().starts_with("shell:")
    {
        return Ok((command, ""));
    }
    if let Some(quoted) = command.strip_prefix('"') {
        let (file, rest) = quoted
            .split_once('"')
            .ok_or("ui.close_the_quote_around_the_program_path")?;
        if !rest.is_empty() && !rest.starts_with(char::is_whitespace) {
            return Err("ui.add_a_space_after_the_program_path");
        }
        return Ok((file, rest.trim_start()));
    }
    let split = command.find(char::is_whitespace).unwrap_or(command.len());
    let first = &command[..split];
    let explicit = |text: &str| explicit_path(text) || text.as_bytes().get(1) == Some(&b':');
    let executable_extension = |file: &str| {
        Path::new(file)
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| {
                ["exe", "com", "bat", "cmd", "lnk"]
                    .iter()
                    .any(|kind| ext.eq_ignore_ascii_case(kind))
            })
    };
    if split == command.len() || (!explicit(first) && executable_extension(first)) {
        return Ok((first, command[split..].trim_start()));
    }
    let local_path = |value: &str| {
        let path = base.unwrap_or(Path::new("")).join(value);
        check_files
            && !is_network_path(&path.to_string_lossy())
            && path.is_absolute()
            && path.exists()
    };
    // A path can contain spaces before its extension. Stop scanning when the
    // following tokens begin an option, quoted argument or separate rooted path.
    let candidate = (!command.contains('"') && local_path(command)).then_some((command, ""));
    let mut suffix: Option<(&str, &str)> = None;
    for (index, _) in command
        .char_indices()
        .filter(|&(_, ch)| ch.is_whitespace())
        .chain(std::iter::once((command.len(), ' ')))
    {
        let file = &command[..index];
        if file.ends_with(char::is_whitespace) {
            continue;
        }
        let rest = command[index..].trim_start();
        if explicit(file) && executable_extension(file) {
            if suffix.is_some_and(|(previous, _)| file[previous.len()..].contains(['\\', '/'])) {
                return Err("ui.ambiguous_command_target");
            }
            suffix.get_or_insert((file, rest));
        }
        if rest.is_empty() {
            break;
        }
        let next = rest.split_whitespace().next().unwrap_or("");
        if next.starts_with(['"', '-', '/'])
            || is_network_path(next)
            || next.as_bytes().get(1) == Some(&b':')
        {
            break;
        }
    }
    match (candidate, suffix) {
        (Some(file), Some(executable)) if file != executable => Err("ui.ambiguous_command_target"),
        (Some(target), _) | (_, Some(target)) => Ok(target),
        _ if !explicit(first) => Ok((first, command[split..].trim_start())),
        _ => Err("ui.ambiguous_command_target"),
    }
}

pub(crate) fn normalize_original_command(command: &str) -> String {
    let (prefix, body) = ["@+", "@-", "@"]
        .into_iter()
        .find_map(|prefix| command.strip_prefix(prefix).map(|rest| (prefix, rest)))
        .unwrap_or(("", command));
    let body = body.trim_start();
    let Ok((file, arguments)) = split_target(body, None, false) else {
        return command.to_owned();
    };
    if !body.starts_with('"') || file.contains('%') {
        return command.to_owned();
    }
    let unquoted = format!("{file}{}", &body[file.len() + 2..]);
    if split_target(&unquoted, None, false).ok() == Some((file, arguments)) {
        format!(
            "{prefix}{}{unquoted}",
            &command[prefix.len()..command.len() - body.len()]
        )
    } else {
        command.to_owned()
    }
}

pub fn replace_command_target(command: &str, path: &str) -> Option<String> {
    let command = command.trim();
    let (prefix, body) = ["@+", "@-", "@"]
        .into_iter()
        .find_map(|prefix| {
            command
                .strip_prefix(prefix)
                .map(|rest| (prefix, rest.trim_start()))
        })
        .unwrap_or(("", command));
    let parameters = if body
        .get(..path.len())
        .is_some_and(|start| start.eq_ignore_ascii_case(path))
        && body
            .get(path.len()..)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with(char::is_whitespace))
    {
        body[path.len()..].trim_start()
    } else {
        split_target(body, None, false).ok()?.1
    };
    let mut result = format!("{prefix}\"{path}\"");
    if !parameters.is_empty() {
        result.push(' ');
        result.push_str(parameters);
    }
    Some(result)
}

pub(crate) fn resolve_target(command: &str, base: &Path) -> Result<String, String> {
    let command = command.trim();
    let command = ["@+", "@-", "@"]
        .into_iter()
        .find_map(|prefix| command.strip_prefix(prefix))
        .unwrap_or(command)
        .trim_start();
    let (file, _) = split_target(command, Some(base), true)?;
    let file = expand_environment(file, is_web_address(file))?;
    if file.is_empty() || (!is_web_address(&file) && (has_parameter(&file) || has_context(&file))) {
        return Err("ui.an_argument_cannot_replace_the_program_path".into());
    }
    Ok(file)
}

fn target_parts(command: &str, base: Option<&Path>) -> Result<(String, String), String> {
    let executable = split_target(command, None, false).map_or("", |(file, _)| file);
    let name = executable
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    let shell = [
        "cmd",
        "cmd.exe",
        "powershell",
        "powershell.exe",
        "pwsh",
        "pwsh.exe",
        "%comspec%",
    ]
    .contains(&name.as_str());
    let expanded = expand_environment_mode(command, is_web_address(command), shell)?;
    let (file, parameters) = split_target(&expanded, base, true)?;
    Ok((file.to_owned(), parameters.to_owned()))
}

fn resolve_base(
    command: &str,
    parameter: Option<&str>,
    base: Option<&Path>,
) -> Result<LaunchRequest, String> {
    if command.chars().any(|c| matches!(c, '\0' | '\r' | '\n'))
        || parameter.is_some_and(|p| p.chars().any(|c| matches!(c, '\0' | '\r' | '\n')))
    {
        return Err("ui.the_command_or_arguments_contain_invalid_control".into());
    }
    let command = command.trim();
    if command.is_empty() {
        return Err("ui.the_command_is_empty".into());
    }
    let (file, parameters) = target_parts(command, base)?;
    if file.is_empty() || (!is_web_address(command) && file.contains("%p")) {
        return Err("ui.an_argument_cannot_replace_the_program_path".into());
    }
    let substitute = |text: &str| -> Result<String, String> {
        if text.contains("%p") {
            Ok(text.replace("%p", parameter.ok_or("ui.enter_a_parameter")?))
        } else {
            Ok(text.to_owned())
        }
    };
    Ok(LaunchRequest {
        file: substitute(&file)?,
        parameters: substitute(&parameters)?,
    })
}

fn expand_environment(command: &str, url: bool) -> Result<String, String> {
    expand_environment_mode(command, url, false)
}
fn expand_environment_mode(command: &str, url: bool, shell: bool) -> Result<String, String> {
    let mut output = String::new();
    let mut remaining = command;
    while let Some(index) = remaining.find('%') {
        output.push_str(&remaining[..index]);
        remaining = &remaining[index..];
        if parameter_marker(remaining) {
            if remaining.as_bytes().get(2) == Some(&b'%') {
                return Err("ui.the_parameter_placeholder_p_is_unsupported".into());
            }
            output.push_str("%p");
            remaining = &remaining[2..];
        } else if url
            && remaining
                .as_bytes()
                .get(1..3)
                .is_some_and(|p| p.iter().all(u8::is_ascii_hexdigit))
        {
            output.push_str(&remaining[..3]);
            remaining = &remaining[3..];
        } else if let Some(end) = remaining[1..].find('%') {
            let key = &remaining[1..end + 1];
            if key.is_empty()
                || !key
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "_()".contains(c))
            {
                if shell {
                    output.push('%');
                    remaining = &remaining[1..];
                    continue;
                }
                return Err("ui.the_command_contains_an_unknown_percent_placeholder".into());
            }
            let value = match std::env::var(key) {
                Ok(value) => value,
                Err(_) if shell => {
                    output.push_str(&remaining[..end + 2]);
                    remaining = &remaining[end + 2..];
                    continue;
                }
                Err(_) => {
                    return Err(crate::localization::message(
                        "ui.environment_variable_not_found_key",
                        &[("key", key)],
                    ));
                }
            };
            if value.chars().any(|c| matches!(c, '\0' | '\r' | '\n')) {
                return Err(
                    "ui.the_environment_variable_contains_invalid_control_characters".into(),
                );
            }
            output.push_str(&value);
            remaining = &remaining[end + 2..];
        } else {
            if shell {
                output.push('%');
                remaining = &remaining[1..];
                continue;
            }
            return Err("ui.the_command_contains_an_unknown_percent_placeholder".into());
        }
    }
    output.push_str(remaining);
    Ok(output)
}

fn parameter_marker(rest: &str) -> bool {
    rest.starts_with("%p")
        && !rest
            .as_bytes()
            .get(2)
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
}
