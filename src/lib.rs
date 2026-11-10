#[derive(Clone, Debug)]
pub struct Entry {
    pub alias: String,
    pub name: String,
    pub command: String,
    pub frequency: i64,
}

pub fn parse_entries(input: &str) -> Result<Vec<Entry>, String> {
    input
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.is_empty())
        .map(|(i, line)| {
            let fields: Vec<_> = line.split('\t').collect();
            if fields.len() != 4 || fields[0].is_empty() {
                return Err(format!("Invalid dataset row {}", i + 1));
            }
            Ok(Entry {
                alias: fields[0].into(),
                name: fields[1].into(),
                command: fields[2].into(),
                frequency: fields[3]
                    .parse()
                    .map_err(|_| format!("Invalid frequency at row {}", i + 1))?,
            })
        })
        .collect()
}

pub struct SearchIndex {
    aliases: Vec<String>,
    frequencies: Vec<i64>,
}

impl SearchIndex {
    pub fn new(entries: &[Entry]) -> Self {
        Self {
            aliases: entries.iter().map(|e| e.alias.to_lowercase()).collect(),
            frequencies: entries.iter().map(|e| e.frequency).collect(),
        }
    }

    pub fn query(&self, input: &str) -> Vec<usize> {
        let input = input.to_lowercase();
        let mut hits: Vec<(usize, i64)> = self
            .aliases
            .iter()
            .enumerate()
            .filter_map(|(i, alias)| {
                if input.is_empty() {
                    return Some((i, self.frequencies[i]));
                }
                alias.find(&input).map(|pos| {
                    let position = alias[..pos].chars().count() as i64 + 1;
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
pub mod observation;
pub mod launch;
