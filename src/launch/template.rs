// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::{
    LaunchContext, LaunchRequest, ShowMode, effective_directory, explicit_path, is_network_path,
    parser::resolve_base,
};
use std::path::Path;

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
pub(super) fn encode_parameter(
    kind: crate::library::ParamType,
    value: &str,
) -> Result<String, String> {
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

pub(super) fn expand_environment(command: &str, url: bool) -> Result<String, String> {
    expand_environment_mode(command, url, false)
}
pub(super) fn expand_environment_mode(
    command: &str,
    url: bool,
    shell: bool,
) -> Result<String, String> {
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
