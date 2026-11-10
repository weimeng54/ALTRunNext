use crate::Entry;
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum EntryKind{Command,Separator,Remark}
impl EntryKind {
    pub fn parse(s:&str)->Result<Self,String>{match s{"command"=>Ok(Self::Command),"separator"=>Ok(Self::Separator),"remark"=>Ok(Self::Remark),_=>Err("未知快捷项类型".into())}}
    pub fn name(self)->&'static str{match self{Self::Command=>"command",Self::Separator=>"separator",Self::Remark=>"remark"}}
}
pub fn legacy_id(alias:&str,name:&str,command:&str,row:usize)->String{
    let mut hash=0xcbf29ce484222325u64;
    for byte in alias.bytes().chain([0]).chain(name.bytes()).chain([0]).chain(command.bytes()).chain(row.to_le_bytes()) {hash=(hash^u64::from(byte)).wrapping_mul(0x100000001b3);}
    format!("legacy-{hash:016x}")
}
pub fn new_id()->String {static NEXT:AtomicU64=AtomicU64::new(0);format!("item-{:x}-{:x}-{:x}",std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos(),std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed))}
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
    LegacyUrl,
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
            "raw" | "no_encoding" => Ok(Self::Raw),
            "url_query" => Ok(Self::LegacyUrl),
            "utf8_query" => Ok(Self::Utf8Url),
            _ => Err("未知参数类型".into()),
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::None => "none",
            Self::Raw => "raw",
            Self::LegacyUrl => "url_query",
            Self::Utf8Url => "utf8_query",
        }
    }
}
pub struct Import {
    pub entries: Vec<Entry>,
    pub ignored_lines: usize,
}
pub fn import_legacy(text: &str) -> Result<Import, String> {
    if text.lines().any(|line| line.contains('\t')) {
        return Ok(Import {
            entries: crate::parse_entries(text)?,
            ignored_lines: 0,
        });
    }
    let mut entries = Vec::new();
    let mut ignored_lines = 0;
    for (number, line) in text.trim_start_matches('\u{feff}').lines().enumerate() {
        let line = line.trim();
        let Some(delimiter) = line.chars().find(|c| matches!(c, '|' | ',' | '，')) else {
            ignored_lines += 1;
            continue;
        };
        let mut fields = line.split(delimiter);
        let mut consumed = 0;
        let mut take = || {
            consumed += 1;
            fields
                .next()
                .map(str::trim)
                .ok_or_else(|| format!("第 {} 行快捷项不完整", number + 1))
        };
        let mut first = take()?;
        let mut frequency = 0;
        if let Some(digits) = first.strip_prefix('F')
            && !digits.is_empty()
            && digits.bytes().all(|b| b.is_ascii_digit())
        {
            frequency = if digits.len() > 8 {
                10000
            } else {
                digits.parse().map_err(|_| "无效频率")?
            };
            first = take()?;
        }
        let (kind, alias) = if first.is_empty()
            || ["no_encoding", "url_query", "utf8_query"]
                .contains(&first.to_ascii_lowercase().as_str())
        {
            (ParamType::parse(first)?, take()?.to_owned())
        } else {
            (ParamType::None, first.to_owned())
        };
        let name = take()?.to_owned();
        let command = line
            .splitn(consumed + 1, delimiter)
            .nth(consumed)
            .ok_or_else(|| format!("第 {} 行缺少命令", number + 1))?
            .trim()
            .to_owned();
        let id=legacy_id(&alias,&name,&command,number);
        let entry = Entry {
            alias,
            name,
            command,
            frequency,
            param_type: kind,
            id,working_directory:String::new(),enabled:true,kind:EntryKind::Command,
        };
        validate(&entry).map_err(|e| format!("第 {} 行：{e}", number + 1))?;
        entries.push(entry);
    }
    Ok(Import {
        entries,
        ignored_lines,
    })
}
fn validate(e: &Entry) -> Result<(), String> {
    if e.alias.trim().is_empty() || (e.kind==EntryKind::Command&&e.command.trim().is_empty()) {
        return Err("快捷字和命令不能为空".into());
    }
    for s in [&e.alias, &e.name, &e.command,&e.id,&e.working_directory] {
        if s.chars().any(|c| matches!(c, '\0' | '\t' | '\r' | '\n')) {
            return Err("快捷项不能包含换行、制表符或空字符".into());
        }
    }
    Ok(())
}
pub fn serialize(entries: &[Entry]) -> Result<String, String> {
    let mut output = String::new();
    for (i, e) in entries.iter().enumerate() {
        validate(e).map_err(|s| format!("第 {} 项：{s}", i + 1))?;
        output.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            e.alias,
            e.name,
            e.command,
            e.frequency,
            e.param_type.name(),if e.id.is_empty(){legacy_id(&e.alias,&e.name,&e.command,i)}else{e.id.clone()},e.working_directory,e.kind.name(),u8::from(e.enabled)
        ));
    }
    Ok(output)
}
pub fn decode(bytes: &[u8]) -> Result<String, String> {
    if let Some(raw) = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]) {
        return String::from_utf8(raw.to_vec()).map_err(|_| "UTF-8 文件包含无效字符".into());
    }
    if bytes.starts_with(&[0xff, 0xfe]) || bytes.starts_with(&[0xfe, 0xff]) {
        if !bytes.len().is_multiple_of(2) {
            return Err("UTF-16 文件长度不正确".into());
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
        return String::from_utf16(&text).map_err(|_| "UTF-16 文件包含无效字符".into());
    }
    if let Ok(text) = std::str::from_utf8(bytes) {
        return Ok(text.into());
    }
    crate::textcodec::decode_legacy(bytes)
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
        let bytes = std::fs::read(path).map_err(|e| format!("无法读取快捷项：{e}"))?;
        if bytes.len() > 16 * 1024 * 1024 {
            return Err("快捷项文件超过 16 MiB".into());
        }
        let text = decode(&bytes)?;
        let entries = if text.trim().is_empty() {
            Vec::new()
        } else {
            import_legacy(&text)?.entries
        };
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
            return Err(
                "数据文件已被其他程序修改。请先保留草稿并重新打开，避免覆盖外部修改。".into(),
            );
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
            .map_err(|e| format!("无法创建临时文件：{e}"))?;
        let _cleanup = Cleanup(stage.clone());
        file.write_all(content.as_bytes())
            .and_then(|_| file.sync_all())
            .map_err(|e| format!("写入失败，原文件未替换：{e}"))?;
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
            return Err(format!(
                "保存失败（错误 {}），请保留草稿并检查文件权限",
                unsafe { GetLastError() }
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
