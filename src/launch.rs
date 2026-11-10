//! Resolve an explicitly selected command without implicitly invoking a shell.
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
    pub allow_dynamic_target: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShowMode {
    Normal,
    Maximized,
    Minimized,
    Hidden,
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
    if command.contains(['\u{e000}', '\u{e001}']) {
        return Err("命令含保留字符".into());
    }
    let values = [
        context.clipboard.clone().unwrap_or_default(),
        context.window_id.to_string(),
        context.window_title.clone(),
        context.window_class.clone(),
    ];
    let tokens = ["c", "wd", "wt", "wc"];
    let mut masked = command.to_owned();
    let mut had_context = false;
    for (i, token) in tokens.iter().enumerate() {
        let brace = format!("{{%{token}}}");
        let bare = format!("%{token}");
        let marker = format!("\u{e000}{i}\u{e001}");
        if masked.contains(&brace) || masked.contains(&bare) {
            had_context = true;
            if i == 0 && context.clipboard.is_none() {
                return Err("剪贴板没有可用文本".into());
            }
            masked = masked.replace(&brace, &marker).replace(&bare, &marker);
        }
    }
    let actual_kind = if had_context && !has_parameter(command) && parameter.is_none() {
        ParamType::Auto
    } else {
        kind
    };
    let mut result = resolve_typed(&masked, parameter, actual_kind)?;
    if result.file.contains('\u{e000}')
        && !context.allow_dynamic_target
        && !result.file.starts_with("https://")
        && !result.file.starts_with("http://")
    {
        return Err("动态内容不能替换程序路径；可在设置中明确启用".into());
    }
    let substitute = |text: &str| -> Result<String, String> {
        let mut out = String::new();
        let mut rest = text;
        while let Some(start) = rest.find('\u{e000}') {
            out.push_str(&rest[..start]);
            let tail = &rest[start + '\u{e000}'.len_utf8()..];
            let end = tail.find('\u{e001}').ok_or("无效上下文标记")?;
            let i = tail[..end].parse::<usize>().map_err(|_| "无效上下文标记")?;
            let value = values.get(i).ok_or("无效上下文标记")?;
            if value.contains(['\0', '\r', '\n']) {
                return Err("上下文包含换行或空字符，请先复制单行文本".into());
            }
            out.push_str(&match kind {
                ParamType::LegacyUrl => percent_encode(&crate::textcodec::ansi_bytes(value)?),
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
    let directory = if !working_directory.trim().is_empty() {
        Some(expand_environment(working_directory, false)?)
    } else if !context.base_directory.is_empty() {
        Some(context.base_directory.clone())
    } else {
        None
    };
    Ok((result, mode, directory))
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
    use crate::library::ParamType;
    let marked = has_parameter(command);
    if kind == ParamType::None && marked {
        return Err("命令含参数标记，请先设置参数类型".into());
    }
    let active = kind.needs_input(command);
    let encoded = if active {
        let value = parameter.ok_or("请输入参数")?;
        Some(match kind {
            ParamType::LegacyUrl => percent_encode(&crate::textcodec::ansi_bytes(value)?),
            ParamType::Utf8Url => percent_encode(value.as_bytes()),
            _ => value.to_owned(),
        })
    } else {
        None
    };
    let normalized = command.replace("{%p}", "%p");
    let mut result = resolve_base(&normalized, encoded.as_deref())?;
    if active && !marked {
        let value = encoded.as_deref().unwrap_or("");
        if result.file.starts_with("http://") || result.file.starts_with("https://") {
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
fn resolve_base(command: &str, parameter: Option<&str>) -> Result<LaunchRequest, String> {
    if command.chars().any(|c| matches!(c, '\0' | '\r' | '\n'))
        || parameter.is_some_and(|p| p.chars().any(|c| matches!(c, '\0' | '\r' | '\n')))
    {
        return Err("命令或参数中包含无效控制字符".into());
    }
    let command = command.trim();
    if command.is_empty() {
        return Err("命令为空".into());
    }
    let url = command.starts_with("https://")
        || command.starts_with("http://")
        || command.starts_with("mailto:");
    let expanded = expand_environment(command, url)?;
    let (file, parameters) = if url {
        (expanded.as_str(), "")
    } else if let Some(quoted) = expanded.strip_prefix('"') {
        let end = quoted.find('"').ok_or("程序路径的引号没有闭合")?;
        let rest = &quoted[end + 1..];
        if !rest.is_empty() && !rest.starts_with(char::is_whitespace) {
            return Err("程序路径后需要空格".into());
        }
        (&quoted[..end], rest.trim_start())
    } else if std::path::Path::new(&expanded).exists() {
        // Accommodate existing unquoted paths with spaces. This disk check is
        // performed only after Enter, never on show/search.
        (expanded.as_str(), "")
    } else {
        let split = expanded.find(char::is_whitespace).unwrap_or(expanded.len());
        let first = &expanded[..split];
        let path = first.contains(['\\', '/']) || first.as_bytes().get(1) == Some(&b':');
        let explicit_executable = [".exe", ".com", ".bat", ".cmd", ".lnk"]
            .iter()
            .any(|extension| first.to_ascii_lowercase().ends_with(extension));
        if split < expanded.len() && path && !explicit_executable {
            return Err("请给带空格的程序路径加双引号，再填写启动参数".into());
        }
        (&expanded[..split], expanded[split..].trim_start())
    };
    if file.is_empty() || (!url && file.contains("%p")) {
        return Err("参数不能替换程序路径".into());
    }
    let substitute = |text: &str| -> Result<String, String> {
        if text.contains("%p") {
            Ok(text.replace("%p", parameter.ok_or("请输入参数")?))
        } else {
            Ok(text.to_owned())
        }
    };
    Ok(LaunchRequest {
        file: substitute(file)?,
        parameters: substitute(parameters)?,
    })
}

fn expand_environment(command: &str, url: bool) -> Result<String, String> {
    let mut output = String::new();
    let mut remaining = command;
    while let Some(index) = remaining.find('%') {
        output.push_str(&remaining[..index]);
        remaining = &remaining[index..];
        if parameter_marker(remaining) {
            if remaining.as_bytes().get(2) == Some(&b'%') {
                return Err("不支持的参数模板 %p%".into());
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
                return Err("尚未支持这个旧版命令模板".into());
            }
            let value = std::env::var(key).map_err(|_| format!("环境变量不存在：{key}"))?;
            if value.chars().any(|c| matches!(c, '\0' | '\r' | '\n')) {
                return Err("环境变量包含无效控制字符".into());
            }
            output.push_str(&value);
            remaining = &remaining[end + 2..];
        } else {
            return Err("尚未支持这个旧版命令模板".into());
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
