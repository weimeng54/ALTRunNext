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
    io::Write,
    path::{Path, PathBuf},
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
pub fn serialize(entries: &[Entry]) -> Result<String, String> {
    let mut output = String::new();
    for (i, e) in entries.iter().enumerate() {
        validate(e).map_err(|s| {
            crate::localization::message(
                "ui.shortcut_row_detail",
                &[
                    ("row", &(i + 1).to_string()),
                    ("detail", &crate::localization::tr(&s)),
                ],
            )
        })?;
        output.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t1",
            e.alias,
            e.name,
            e.command,
            e.frequency,
            e.param_type.name(),
            if e.id.is_empty() {
                derived_id(&e.alias, &e.name, &e.command, i)
            } else {
                normalize_id(&e.id)
            },
            e.working_directory,
            e.kind.name()
        ));
        if let Some(target) = &e.open_target {
            let (kind, program) = target.fields();
            output.push_str(&format!("\t{kind}\t{program}"));
        }
        output.push('\n');
    }
    Ok(output)
}
pub fn decode(bytes: &[u8]) -> Result<String, String> {
    if let Some(raw) = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]) {
        return String::from_utf8(raw.to_vec())
            .map_err(|_| "ui.the_utf_8_file_contains_invalid_characters".into());
    }
    if bytes.starts_with(&[0xff, 0xfe]) || bytes.starts_with(&[0xfe, 0xff]) {
        if !bytes.len().is_multiple_of(2) {
            return Err("ui.the_utf_16_file_has_an_invalid".into());
        }
        let be = bytes[0] == 0xfe;
        let text: Vec<_> = bytes[2..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|p| {
                if be {
                    u16::from_be_bytes([p[0], p[1]])
                } else {
                    u16::from_le_bytes([p[0], p[1]])
                }
            })
            .collect();
        return String::from_utf16(&text)
            .map_err(|_| "ui.the_utf_16_file_contains_invalid_characters".into());
    }
    if let Ok(text) = std::str::from_utf8(bytes) {
        return Ok(text.into());
    }
    Err("ui.use_utf_8_or_utf_16_text".into())
}

pub struct ShortcutImport {
    pub entries: Vec<Entry>,
    pub ignored_lines: usize,
}

pub fn import_shortcuts(path: &Path) -> Result<ShortcutImport, String> {
    use std::io::Read;
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .and_then(|file| file.take(16 * 1024 * 1024 + 1).read_to_end(&mut bytes))
        .map_err(|error| {
            crate::localization::message(
                "ui.could_not_read_shortcuts_error",
                &[("error", &error.to_string())],
            )
        })?;
    decode_shortcut_import(&bytes)
}

pub fn decode_shortcut_import(bytes: &[u8]) -> Result<ShortcutImport, String> {
    if bytes.len() > 16 * 1024 * 1024 {
        return Err("ui.the_shortcuts_file_exceeds_16_mib".into());
    }
    let text = match decode(bytes) {
        Ok(text) => text,
        Err(error)
            if bytes.contains(&b'\t')
                || bytes.starts_with(&[0xef, 0xbb, 0xbf])
                || bytes.starts_with(&[0xff, 0xfe])
                || bytes.starts_with(&[0xfe, 0xff]) =>
        {
            return Err(error);
        }
        Err(_) => crate::textcodec::decode_gb18030(bytes)?,
    };
    let import = if text
        .lines()
        .any(|line| !line.trim().is_empty() && line.contains('\t'))
    {
        ShortcutImport {
            entries: crate::parse_entries(&text)?,
            ignored_lines: 0,
        }
    } else {
        parse_original_shortcuts(&text)?
    };
    if !import
        .entries
        .iter()
        .any(|entry| entry.kind == EntryKind::Command)
    {
        return Err("ui.there_are_no_shortcuts_to_import".into());
    }
    Ok(import)
}

fn parse_original_shortcuts(text: &str) -> Result<ShortcutImport, String> {
    let mut entries = Vec::new();
    let mut ignored_lines = 0;
    for (row, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.contains('\0') {
            return Err(crate::localization::message(
                "ui.line_row_contains_invalid_characters",
                &[("row", &(row + 1).to_string())],
            ));
        }
        if line.starts_with('[') && line.ends_with(']') {
            return Err("ui.choose_a_shortcuts_file_import_settings_from".into());
        }
        if line.starts_with([';', '#']) {
            ignored_lines += 1;
            continue;
        }
        let delimiter = if line.starts_with('F') && line.contains('|') {
            Some('|')
        } else {
            line.chars().find(|c| matches!(c, '|' | ',' | '，'))
        };
        let Some(delimiter) = delimiter else {
            ignored_lines += 1;
            continue;
        };
        let invalid = || {
            crate::localization::message(
                "ui.the_shortcut_on_line_row_has_an",
                &[("row", &(row + 1).to_string())],
            )
        };
        let (first, rest) = line.split_once(delimiter).ok_or_else(invalid)?;
        let first = first.trim();
        let header = first.strip_prefix('F').filter(|digits| {
            !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit())
        });
        let mut frequency = 0;
        let (first, rest) = if let Some(digits) = header {
            frequency = if digits.len() > 8 {
                10000
            } else {
                digits.parse().map_err(|_| invalid())?
            };
            rest.split_once(delimiter).ok_or_else(invalid)?
        } else {
            (first, rest)
        };
        let (alias, rest, param_type) = match first.trim().to_ascii_lowercase().as_str() {
            kind @ ("" | "no_encoding" | "url_query" | "utf8_query") => {
                let (alias, tail) = rest.split_once(delimiter).ok_or_else(invalid)?;
                let kind = match kind {
                    "no_encoding" => ParamType::Raw,
                    kind => ParamType::parse(kind)?,
                };
                (alias, tail, kind)
            }
            _ => (first, rest, ParamType::None),
        };
        let (name, command) = rest.split_once(delimiter).ok_or_else(invalid)?;
        let (alias, name, command) = (alias.trim(), name.trim(), command.trim());
        let entry = Entry {
            alias: alias.into(),
            name: name.into(),
            command: crate::launch::normalize_original_command(command),
            frequency,
            param_type,
            id: derived_id(alias, name, command, row),
            working_directory: String::new(),
            open_target: None,
            kind: EntryKind::Command,
        };
        validate(&entry).map_err(|error| {
            crate::localization::message(
                "ui.line_row_detail",
                &[
                    ("row", &(row + 1).to_string()),
                    ("detail", &crate::localization::tr(&error)),
                ],
            )
        })?;
        entries.push(entry);
    }
    Ok(ShortcutImport {
        entries,
        ignored_lines,
    })
}
pub struct Snapshot {
    pub path: PathBuf,
    pub entries: Vec<Entry>,
    original: Option<Vec<u8>>,
}
impl Snapshot {
    pub fn new(path: PathBuf, entries: Vec<Entry>) -> Result<Self, String> {
        serialize(&entries)?;
        Ok(Self {
            path,
            entries,
            original: None,
        })
    }
    pub fn open(path: &Path) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|e| {
            crate::localization::message(
                "ui.could_not_read_shortcuts_error",
                &[("error", &e.to_string())],
            )
        })?;
        if bytes.len() > 16 * 1024 * 1024 {
            return Err("ui.the_shortcuts_file_exceeds_16_mib".into());
        }
        let text = decode(&bytes)?;
        let entries = crate::parse_entries(&text)?;
        Ok(Self {
            path: path.into(),
            entries,
            original: Some(bytes),
        })
    }
    pub fn save(&mut self, entries: &[Entry]) -> Result<(), String> {
        let content = serialize(entries)?;
        let current = match std::fs::read(&self.path) {
            Ok(b) => Some(b),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e.to_string()),
        };
        if current != self.original {
            return Err("ui.the_data_file_changed_outside_altrunnext_keep".into());
        }
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let stage = self.path.with_extension(format!(
            "altrun-tmp-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        struct Cleanup(PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(&self.0);
            }
        }
        let mut file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&stage)
            .map_err(|e| {
                crate::localization::message(
                    "ui.could_not_create_a_temporary_file_e",
                    &[("e", &e.to_string())],
                )
            })?;
        let _cleanup = Cleanup(stage.clone());
        file.write_all(content.as_bytes())
            .and_then(|_| file.sync_all())
            .map_err(|e| {
                crate::localization::message(
                    "ui.could_not_write_the_changes_the_original",
                    &[("e", &e.to_string())],
                )
            })?;
        drop(file);
        replace(&stage, &self.path, self.original.is_some())?;
        self.original = Some(content.into_bytes());
        self.entries = entries.to_vec();
        Ok(())
    }
}
pub(crate) fn replace(stage: &Path, destination: &Path, existed: bool) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::{Foundation::GetLastError, Storage::FileSystem::*};
        let wide = |p: &Path| {
            p.as_os_str()
                .encode_wide()
                .chain(Some(0))
                .collect::<Vec<_>>()
        };
        let stage = wide(stage);
        let target = wide(destination);
        let extension = format!(
            "{}.bak",
            destination
                .extension()
                .and_then(|p| p.to_str())
                .unwrap_or("data")
        );
        let backup = wide(&destination.with_extension(extension));
        let ok = unsafe {
            if existed {
                ReplaceFileW(
                    target.as_ptr(),
                    stage.as_ptr(),
                    backup.as_ptr(),
                    0,
                    std::ptr::null(),
                    std::ptr::null(),
                )
            } else {
                MoveFileExW(stage.as_ptr(), target.as_ptr(), MOVEFILE_WRITE_THROUGH)
            }
        };
        if ok == 0 {
            return Err(crate::localization::message(
                "ui.could_not_save_error_code_keep_a",
                &[("code", &unsafe { GetLastError() }.to_string())],
            ));
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = (stage, destination, existed);
        Err("Windows only".into())
    }
}
