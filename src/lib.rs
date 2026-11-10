#[derive(Clone, Debug)]
pub struct Entry {
    pub alias: String,
    pub name: String,
    pub command: String,
    pub frequency: i64,
    pub param_type: library::ParamType,
    pub id: String,
    pub working_directory: String,
    pub enabled: bool,
    pub kind: library::EntryKind,
}

pub fn parse_entries(input: &str) -> Result<Vec<Entry>, String> {
    input
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.is_empty())
        .map(|(i, line)| {
            let fields: Vec<_> = line.split('\t').collect();
            if !(4..=9).contains(&fields.len()) || fields[0].is_empty() {
                return Err(format!("Invalid dataset row {}", i + 1));
            }
            Ok(Entry {
                alias: fields[0].into(),
                name: fields[1].into(),
                command: fields[2].into(),
                frequency: fields[3]
                    .parse()
                    .map_err(|_| format!("Invalid frequency at row {}", i + 1))?,
                id: fields
                    .get(5)
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| library::legacy_id(fields[0], fields[1], fields[2], i)),
                working_directory: fields.get(6).unwrap_or(&"").to_string(),
                kind: library::EntryKind::parse(fields.get(7).unwrap_or(&"command"))?,
                enabled: match fields.get(8).copied().unwrap_or("1") {
                    "1" => true,
                    "0" => false,
                    _ => return Err(format!("Invalid enabled flag at row {}", i + 1)),
                },
                param_type: if fields.len() >= 5 {
                    library::ParamType::parse(fields[4])?
                } else {
                    library::ParamType::Auto
                },
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .and_then(|rows| {
            let mut ids = std::collections::HashSet::new();
            if rows.iter().any(|e| !ids.insert(&e.id)) {
                Err("Duplicate shortcut identity".into())
            } else {
                Ok(rows)
            }
        })
}

pub struct SearchIndex {
    aliases: Vec<String>,
    frequencies: Vec<i64>,
    characters: Vec<Vec<char>>,
    active: Vec<bool>,
}

impl SearchIndex {
    pub fn aliases(&self) -> &[String] {
        &self.aliases
    }
    pub fn rank_positions(&self, positions: &[Option<usize>], input: &str) -> Vec<usize> {
        let length = input.chars().count() as i64;
        let mut hits: Vec<_> = positions
            .iter()
            .enumerate()
            .filter_map(|(i, p)| {
                if !self.active.get(i).copied().unwrap_or(false) {
                    return None;
                }
                p.map(|p| {
                    (
                        i,
                        1024i64
                            .saturating_add(self.frequencies[i].saturating_mul(4))
                            .saturating_sub((p as i64 + 1).saturating_mul(128))
                            .saturating_sub(
                                (self.characters[i].len() as i64 - length).saturating_mul(16),
                            ),
                    )
                })
            })
            .collect();
        hits.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        hits.into_iter().map(|h| h.0).collect()
    }
    pub fn query_options(
        &self,
        input: &str,
        regex: bool,
        anywhere: bool,
        standard: bool,
    ) -> Result<Vec<usize>, String> {
        if input.chars().count() > 256 {
            return Ok(Vec::new());
        }
        if regex
            && input.contains([
                '*', '?', '[', ']', '(', ')', '|', '^', '$', '\\', '.', '+', '{',
            ])
        {
            let pattern = if standard {
                input.to_owned()
            } else {
                matching::prepare_legacy(input)
            };
            if matching::advanced(&pattern) {
                return Err("advanced".into());
            }
            return Ok(self.rank_positions(
                &matching::positions(&pattern, &self.aliases, !anywhere)?,
                input,
            ));
        }
        let needle = input.to_lowercase();
        let positions: Vec<_> = self
            .aliases
            .iter()
            .map(|a| {
                a.find(&needle)
                    .filter(|p| anywhere || *p == 0)
                    .map(|p| a[..p].chars().count())
            })
            .collect();
        Ok(self.rank_positions(&positions, input))
    }
    pub fn new(entries: &[Entry]) -> Self {
        Self {
            active: entries
                .iter()
                .map(|e| e.enabled && e.kind == library::EntryKind::Command)
                .collect(),
            aliases: entries.iter().map(|e| e.alias.to_lowercase()).collect(),
            frequencies: entries.iter().map(|e| e.frequency).collect(),
            characters: entries
                .iter()
                .map(|e| e.alias.to_lowercase().chars().collect())
                .collect(),
        }
    }

    pub fn query(&self, input: &str) -> Vec<usize> {
        if input == " " || input.chars().count() > 256 {
            return Vec::new();
        }
        let input = input.to_lowercase();
        let wildcard = input.contains(['*', '?']);
        let pattern: Vec<char> = input.chars().collect();
        let mut hits: Vec<(usize, i64)> = self
            .aliases
            .iter()
            .enumerate()
            .filter_map(|(i, alias)| {
                if !self.active[i] {
                    return None;
                }
                if input.is_empty() {
                    return Some((i, self.frequencies[i]));
                }
                let position = if wildcard {
                    wildcard_position(&pattern, &self.characters[i])
                } else {
                    alias.find(&input).map(|pos| alias[..pos].chars().count())
                };
                position.map(|pos| {
                    let position = pos as i64 + 1;
                    let length_delta = alias.chars().count() as i64 - input.chars().count() as i64;
                    let score = 1024_i64
                        .saturating_add(self.frequencies[i].saturating_mul(4))
                        .saturating_sub(position.saturating_mul(128))
                        .saturating_sub(length_delta.saturating_mul(16));
                    (i, score)
                })
            })
            .collect();
        hits.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        hits.into_iter().map(|hit| hit.0).collect()
    }
}

pub fn search(entries: &[Entry], query: &str) -> Vec<usize> {
    SearchIndex::new(entries).query(query)
}

pub fn advance(selected: usize, length: usize, delta: isize) -> usize {
    if length == 0 {
        0
    } else {
        (selected as isize + delta).rem_euclid(length as isize) as usize
    }
}

#[cfg(windows)]
pub fn safe_notepad_path() -> Result<std::path::PathBuf, String> {
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Win32::System::SystemInformation::GetSystemDirectoryW;
    let mut buffer = vec![0_u16; 512];
    loop {
        let size = unsafe { GetSystemDirectoryW(buffer.as_mut_ptr(), buffer.len() as u32) };
        if size == 0 {
            return Err("Cannot resolve the Windows system directory".into());
        }
        if size as usize >= buffer.len() {
            buffer.resize(size as usize + 1, 0);
            continue;
        }
        let directory = std::ffi::OsString::from_wide(&buffer[..size as usize]);
        return Ok(std::path::PathBuf::from(directory).join("notepad.exe"));
    }
}
#[cfg(windows)]
pub mod capture_bridge;
pub mod desktop_frame;
pub mod desktop_observation;
#[cfg(windows)]
pub mod integration;
pub mod isolated_matching;
pub mod launch;
pub mod library;
pub mod library_updates;
pub mod management;
pub mod matching;
pub mod migration;
pub mod observation;
pub mod preferences;
pub mod state_store;
pub mod textcodec;
pub mod usage;

fn wildcard_position(pattern: &[char], alias: &[char]) -> Option<usize> {
    fn merge(slot: &mut Option<usize>, value: usize) {
        *slot = Some(slot.map_or(value, |old| old.min(value)));
    }
    let mut states = vec![None; pattern.len() + 1];
    let mut next = states.clone();
    let mut best = None;
    for offset in 0..=alias.len() {
        merge(&mut states[0], offset);
        for i in 0..pattern.len() {
            if pattern[i] == '*'
                && let Some(start) = states[i]
            {
                merge(&mut states[i + 1], start);
            }
        }
        if let Some(start) = states[pattern.len()] {
            merge(&mut best, start);
        }
        if offset == alias.len() {
            break;
        }
        next.fill(None);
        for i in 0..pattern.len() {
            if let Some(start) = states[i] {
                if pattern[i] == '*' {
                    merge(&mut next[i], start);
                } else if pattern[i] == '?' || pattern[i] == alias[offset] {
                    merge(&mut next[i + 1], start);
                }
            }
        }
        std::mem::swap(&mut states, &mut next);
    }
    best
}
