use crate::Entry;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParamType { Auto, None, Raw, LegacyUrl, Utf8Url }

impl ParamType {
    pub fn needs_input(self, command: &str) -> bool { self != Self::None && (self != Self::Auto || crate::launch::has_parameter(command)) }
}

pub struct Import { pub entries: Vec<Entry>, pub ignored_lines: usize }
pub fn import_legacy(_text: &str) -> Result<Import, String> { Err("not implemented".into()) }
pub fn serialize(_entries: &[Entry]) -> Result<String, String> { Err("not implemented".into()) }
pub fn decode(_bytes: &[u8]) -> Result<String, String> { Err("not implemented".into()) }
pub struct Snapshot { pub path: PathBuf, pub entries: Vec<Entry> }
impl Snapshot {
    pub fn open(_path: &Path) -> Result<Self, String> { Err("not implemented".into()) }
    pub fn save(&mut self, _entries: &[Entry]) -> Result<(), String> { Err("not implemented".into()) }
}
