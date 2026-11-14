// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

mod format;
mod import;
mod snapshot;

pub use format::{decode, parse_entries, serialize};
pub use import::{ShortcutImport, decode_shortcut_import, import_shortcuts};
pub use snapshot::Snapshot;
pub(crate) use snapshot::replace;

use crate::Entry;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryKind {
    Command,
    Separator,
    Remark,
}
impl EntryKind {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "command" => Ok(Self::Command),
            "separator" => Ok(Self::Separator),
            "remark" => Ok(Self::Remark),
            _ => Err("ui.unknown_shortcut_type".into()),
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Command => "command",
            Self::Separator => "separator",
            Self::Remark => "remark",
        }
    }
}
pub fn derived_id(alias: &str, name: &str, command: &str, row: usize) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in alias
        .bytes()
        .chain([0])
        .chain(name.bytes())
        .chain([0])
        .chain(command.bytes())
        .chain(row.to_le_bytes())
    {
        hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
    }
    format!("item-{hash:016x}")
}

pub(crate) fn normalize_id(id: &str) -> String {
    if let Some(hash) = id.strip_prefix("legacy-")
        && hash.len() == 16
        && hash.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return format!("item-{hash}");
    }
    id.to_owned()
}
pub fn new_id() -> String {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    format!(
        "item-{:x}-{:x}-{:x}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )
}
use std::{
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParamType {
    Auto,
    None,
    Raw,
    AnsiUrl,
    Utf8Url,
}
impl ParamType {
    pub fn needs_input(self, command: &str) -> bool {
        self != Self::None && (self != Self::Auto || crate::launch::has_parameter(command))
    }
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "auto" => Ok(Self::Auto),
            "" | "none" => Ok(Self::None),
            "raw" => Ok(Self::Raw),
            "url_query" => Ok(Self::AnsiUrl),
            "utf8_query" => Ok(Self::Utf8Url),
            _ => Err("ui.unknown_parameter_type".into()),
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::None => "none",
            Self::Raw => "raw",
            Self::AnsiUrl => "url_query",
            Self::Utf8Url => "utf8_query",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OpenTarget {
    Path,
    Document(Option<String>),
    LinkArguments(String),
}

impl OpenTarget {
    pub(crate) fn parse(kind: &str, program: &str) -> Result<Option<Self>, String> {
        match (kind, program) {
            ("", "") => Ok(None),
            ("path", "") => Ok(Some(Self::Path)),
            ("document", "") => Ok(Some(Self::Document(None))),
            ("document", program) => Ok(Some(Self::Document(Some(program.into())))),
            ("link", arguments) if !arguments.is_empty() => {
                Ok(Some(Self::LinkArguments(arguments.into())))
            }
            _ => Err("ui.unrecognized_file_opening_method".into()),
        }
    }

    fn fields(&self) -> (&str, &str) {
        match self {
            Self::Path => ("path", ""),
            Self::Document(program) => ("document", program.as_deref().unwrap_or("")),
            Self::LinkArguments(arguments) => ("link", arguments),
        }
    }
}

pub(crate) fn validate(e: &Entry) -> Result<(), String> {
    if e.alias.trim().is_empty() || (e.kind == EntryKind::Command && e.command.trim().is_empty()) {
        return Err("ui.an_alias_and_a_command_are_required".into());
    }
    for s in [&e.alias, &e.name, &e.command, &e.id, &e.working_directory] {
        if s.chars().any(|c| matches!(c, '\0' | '\t' | '\r' | '\n')) {
            return Err("ui.shortcuts_cannot_contain_line_breaks_tabs_or".into());
        }
    }
    if let Some(target) = &e.open_target {
        let executable = matches!(target, OpenTarget::Path)
            && crate::management::is_executable(Path::new(&e.command));
        if e.kind != EntryKind::Command
            || (!executable && e.param_type != ParamType::None)
            || !crate::management::valid_absolute_path(&e.command)
        {
            return Err("ui.enter_the_full_file_or_folder_path".into());
        }
        if let OpenTarget::Document(Some(program)) = target
            && (!crate::management::valid_absolute_path(program)
                || !crate::management::is_executable(Path::new(program)))
        {
            return Err("ui.select_a_valid_exe_application".into());
        }
        if let OpenTarget::LinkArguments(arguments) = target
            && (arguments.is_empty() || arguments.contains(['\0', '\t', '\r', '\n']))
        {
            return Err("ui.the_command_or_arguments_contain_invalid_control".into());
        }
    }
    Ok(())
}
