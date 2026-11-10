use crate::{
    Entry, launch,
    library::{EntryKind, ParamType},
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
    if entry.command.contains('%') || entry.command.starts_with(['@']) {
        return Validation::Unconfirmed("含模板或显示模式".into());
    }
    let Ok(request) = launch::resolve_typed(&entry.command, None, ParamType::None) else {
        return Validation::Unconfirmed("命令需要人工检查".into());
    };
    if request.file.starts_with("\\\\")
        || request.file.contains("://")
        || request.file.starts_with("shell:")
        || request.file.starts_with("::")
    {
        return Validation::Unconfirmed("网络或系统目标，不探测".into());
    }
    let path = Path::new(&request.file);
    if !path.is_absolute() && !request.file.contains(['\\', '/']) {
        return Validation::Unconfirmed("由 Windows 搜索路径解析".into());
    }
    if (if path.is_absolute() {
        path.to_path_buf()
    } else {
        base.join(path)
    })
    .exists()
    {
        Validation::Present
    } else {
        Validation::Missing
    }
}
pub fn convert_path(entry: &mut Entry, base: &Path, relative: bool) -> Result<bool, String> {
    if entry.kind != EntryKind::Command || entry.command.contains('%') {
        return Ok(false);
    }
    let request = launch::resolve_typed(&entry.command, None, ParamType::None)?;
    let path = Path::new(&request.file);
    if request.file.contains("://")
        || request.file.starts_with("\\\\")
        || (!path.is_absolute() && !request.file.contains(['\\', '/']))
    {
        return Ok(false);
    }
    let converted = if relative {
        if !path.is_absolute() {
            return Ok(false);
        }
        PathBuf::from(".").join(
            path.strip_prefix(base)
                .map_err(|_| "目标不在数据目录下，保留绝对路径")?,
        )
    } else {
        if path.is_absolute() {
            return Ok(false);
        }
        base.join(path)
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
pub fn from_path(path: &Path) -> Result<Entry, String> {
    let text = path.to_str().ok_or("路径不是有效 Unicode")?;
    if text.contains(['\0', '\r', '\n', '\t', '"']) {
        return Err("路径包含无效字符".into());
    }
    let name = path
        .file_stem()
        .or_else(|| path.file_name())
        .unwrap_or(path.as_os_str())
        .to_string_lossy()
        .into_owned();
    Ok(Entry {
        alias: name.clone(),
        name,
        command: format!("\"{text}\""),
        frequency: 0,
        param_type: ParamType::None,
        id: crate::library::new_id(),
        working_directory: String::new(),
        enabled: true,
        kind: EntryKind::Command,
    })
}
