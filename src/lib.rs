#[derive(Clone, Debug)]
pub struct Entry {
    pub alias: String,
    pub name: String,
    pub command: String,
    pub frequency: i64,
}

pub fn parse_entries(_input: &str) -> Result<Vec<Entry>, String> {
    Ok(Vec::new())
}

pub fn search(_entries: &[Entry], _query: &str) -> Vec<usize> {
    Vec::new()
}

pub fn advance(_selected: usize, _length: usize, _delta: isize) -> usize {
    0
}
