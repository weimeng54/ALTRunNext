//! Import into a new directory; original files are only ever read.
use crate::{
    Entry,
    library::{self, EntryKind, ParamType},
    preferences::{Preferences, SPECS},
    usage::Usage,
};
use std::path::{Path, PathBuf};
pub struct Migration {
    pub entries: Vec<Entry>,
    pub preferences: Preferences,
    pub usage: Usage,
    pub report: String,
    pub originals: Vec<(String, Vec<u8>)>,
}
fn read_optional(path: &Path) -> Result<Option<Vec<u8>>, String> {
    match std::fs::read(path) {
        Ok(b) if b.len() <= 16 * 1024 * 1024 => Ok(Some(b)),
        Ok(_) => Err(format!("文件过大：{}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}
impl Migration {
    pub fn read(directory: &Path, legacy_appearance: bool) -> Result<Self, String> {
        let bytes =
            read_optional(&directory.join("ShortCutList.txt"))?.ok_or("未找到 ShortCutList.txt")?;
        let text = library::decode(&bytes)?;
        let mut originals = vec![(String::from("ShortCutList.txt"), bytes)];
        let mut entries = Vec::new();
        let mut old_indices = Vec::new();
        let mut notes = Vec::<String>::new();
        if text.contains('\t') {
            entries = crate::parse_entries(&text)?;
            old_indices.extend(entries.iter().map(|e| e.id.clone()));
        } else {
            for (row, line) in text.lines().enumerate() {
                let parsed = library::import_legacy(line)?;
                if let Some(mut entry) = parsed.entries.into_iter().next() {
                    entry.id = library::legacy_id(&entry.alias, &entry.name, &entry.command, row);
                    old_indices.push(entry.id.clone());
                    entries.push(entry);
                } else {
                    let blank = line.trim().is_empty();
                    let id = library::legacy_id("", line, "", row);
                    if blank {
                        old_indices.push(id.clone());
                    }
                    entries.push(Entry {
                        alias: if blank { "---".into() } else { "备注".into() },
                        name: line.trim().to_owned(),
                        command: String::new(),
                        frequency: 0,
                        param_type: ParamType::None,
                        id,
                        working_directory: String::new(),
                        enabled: false,
                        kind: if blank {
                            EntryKind::Separator
                        } else {
                            EntryKind::Remark
                        },
                    });
                }
            }
        }
        let mut preferences = Preferences::default();
        let mut usage = Usage::default();
        let ini = read_optional(&directory.join("ALTRun.ini"))?;
        if let Some(bytes) = ini {
            let old = Preferences::parse(&library::decode(&bytes)?)?;
            for spec in SPECS {
                let appearance = [
                    "TitleFont",
                    "KeywordFont",
                    "ListFont",
                    "ListFormat",
                    "FormWidth",
                    "Alpha",
                    "AlphaColor",
                    "RoundBorderRadius",
                    "BGFileName",
                    "ShowSkin",
                    "Theme",
                    "RowHeight",
                    "MaxRows",
                    "TitleColor",
                    "KeywordColor",
                    "ListColor",
                    "TextColor",
                    "SelectionColor",
                ]
                .contains(&spec.key);
                if !appearance || legacy_appearance {
                    preferences.set(spec.key, &old.get(spec.key))?;
                }
            }
            for (k, v) in &old.values {
                if k.starts_with('@') {
                    preferences.values.insert(k.clone(), v.clone());
                }
            }
            if let Some((_, latest)) = old
                .values
                .iter()
                .find(|(k, _)| k.to_ascii_lowercase().ends_with(".latestlist"))
            {
                for index in latest
                    .split(',')
                    .filter_map(|s| s.trim().parse::<usize>().ok())
                {
                    if let Some(id) = old_indices.get(index)
                        && entries
                            .iter()
                            .any(|e| &e.id == id && e.kind == EntryKind::Command)
                    {
                        usage.recent.push(id.clone());
                    }
                }
                usage.recent.dedup();
                usage.recent.truncate(10);
            }
            originals.push(("ALTRun.ini".into(), bytes));
        }
        for key in [
            "HotKey1",
            "HotKey2",
            "LastItemHotKey",
            "AutoRun",
            "AddToSendTo",
        ] {
            preferences.set(key, &Preferences::default().get(key))?;
        }
        preferences.set("WorkingDirectory", &directory.to_string_lossy())?;
        if let Some(bytes) = read_optional(&directory.join("FavoriteList.txt"))? {
            for line in library::decode(&bytes)?.lines() {
                if let Some((query, target)) = line.split_once('|') {
                    let hits = entries
                        .iter()
                        .filter(|e| e.name == target.trim() || e.alias == target.trim())
                        .collect::<Vec<_>>();
                    if let Some(entry) = hits.first() {
                        usage
                            .favourites
                            .insert(query.trim().into(), entry.id.clone());
                        if hits.len() > 1 {
                            notes.push("同名偏好按文件顺序取第一项。".into());
                        }
                    } else {
                        notes.push("部分偏好未找到对应快捷项，未强行关联。".into());
                    }
                }
            }
            originals.push(("FavoriteList.txt".into(), bytes));
        }
        if let Some(bytes) = read_optional(&directory.join("ParamHistory.txt"))? {
            let mut parameters = Vec::new();
            for line in library::decode(&bytes)?.lines() {
                if let Some((_, value)) = line.split_once('|') {
                    let value = value.trim();
                    if !value.is_empty() && !parameters.iter().any(|v| v == value) {
                        parameters.push(value.to_owned());
                    }
                }
            }
            parameters.truncate(preferences.number("ParamHistoryLimit") as usize);
            usage.parameters.insert("*".into(), parameters);
            originals.push(("ParamHistory.txt".into(), bytes));
        }
        notes.sort();
        notes.dedup();
        let commands = entries
            .iter()
            .filter(|e| e.kind == EntryKind::Command)
            .count();
        let report = format!(
            "ALTRun 迁移结果\n来源：{}\n启动项：{commands}\n分隔 / 备注：{}\n\n原文件只读，副本保存在 originals。\n默认呼出键为 Alt+R；开机启动和发送到默认关闭。\n旧版 HandleID 不复用；旧窗口句柄没有迁移意义。\n{}\n旧程序内存中尚未写回磁盘的修改不在本次迁移中。\n{}\n",
            directory.display(),
            entries.len() - commands,
            if legacy_appearance {
                "已导入旧外观。"
            } else {
                "采用现代外观，旧外观仍保存在原始 INI 副本。"
            },
            notes.join("\n")
        );
        Ok(Self {
            entries,
            preferences,
            usage,
            report,
            originals,
        })
    }
    pub fn write_new(&self, destination: &Path) -> Result<PathBuf, String> {
        // Exclusive directory creation is the transaction boundary: no existing
        // user's files can be replaced, even when paths refer to the same folder.
        std::fs::create_dir(destination)
            .map_err(|e| format!("迁移目标必须是尚不存在的新目录：{e}"))?;
        let write = |name: &str, bytes: &[u8]| -> Result<(), String> {
            use std::io::Write;
            let path = destination.join(name);
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
                .map_err(|e| e.to_string())?;
            file.write_all(bytes)
                .and_then(|_| file.sync_all())
                .map_err(|e| e.to_string())
        };
        write(
            "shortcuts.tsv",
            library::serialize(&self.entries)?.as_bytes(),
        )?;
        write("shortcuts.settings.ini", self.preferences.text().as_bytes())?;
        write("shortcuts.state.tsv", self.usage.text().as_bytes())?;
        write("migration-report.txt", self.report.as_bytes())?;
        std::fs::create_dir(destination.join("originals")).map_err(|e| e.to_string())?;
        for (name, bytes) in &self.originals {
            write(&format!("originals/{name}"), bytes)?;
        }
        Ok(destination.join("shortcuts.tsv"))
    }
}
