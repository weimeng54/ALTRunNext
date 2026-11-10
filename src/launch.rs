//! Resolve an explicitly selected command without implicitly invoking a shell.
#[derive(Debug, PartialEq, Eq)]
pub struct LaunchRequest {
    pub file: String,
    pub parameters: String,
}

pub fn resolve(command: &str, parameter: Option<&str>) -> Result<LaunchRequest, String> {
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
            .iter().any(|extension| first.to_ascii_lowercase().ends_with(extension));
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
        if remaining.starts_with("%p")
            && !remaining
                .as_bytes()
                .get(2)
                .is_some_and(u8::is_ascii_alphanumeric)
        {
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
