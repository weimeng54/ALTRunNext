// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::{
    LaunchRequest, explicit_path, has_context, has_parameter, is_network_path,
    template::{expand_environment, expand_environment_mode},
};
use std::path::Path;

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

pub(super) fn resolve_base(
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
