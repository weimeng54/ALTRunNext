// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::invalid;
use std::{collections::BTreeMap, io::Write};

const LOCAL: u32 = 0x0403_4b50;
const CENTRAL: u32 = 0x0201_4b50;
const END: u32 = 0x0605_4b50;

const fn crc_table() -> [u32; 256] {
    let mut table = [0; 256];
    let mut i = 0;
    while i < table.len() {
        let mut crc = i as u32;
        let mut bit = 0;
        while bit < 8 {
            crc = (crc >> 1) ^ if crc & 1 == 0 { 0 } else { 0xedb8_8320 };
            bit += 1;
        }
        table[i] = crc;
        i += 1;
    }
    table
}

fn checksum(bytes: &[u8]) -> u32 {
    const TABLE: [u32; 256] = crc_table();
    !bytes.iter().fold(!0u32, |crc, &byte| {
        (crc >> 8) ^ TABLE[((crc ^ u32::from(byte)) & 255) as usize]
    })
}

// The backup format uses standard stored ZIP entries with ASCII filenames.
pub(super) fn write(out: &mut impl Write, entries: &[(&str, &[u8])]) -> std::io::Result<()> {
    let mut directory = Vec::new();
    let mut position = 0u32;
    for &(name, bytes) in entries {
        let checksum = checksum(bytes);
        let size = bytes.len() as u32;
        let mut common = Vec::with_capacity(26);
        common.extend(20u16.to_le_bytes());
        common.extend(0u16.to_le_bytes());
        common.extend(0u16.to_le_bytes());
        common.extend(0u16.to_le_bytes());
        common.extend(33u16.to_le_bytes()); // January 1, 1980 in DOS date format.
        common.extend(checksum.to_le_bytes());
        common.extend(size.to_le_bytes());
        common.extend(size.to_le_bytes());
        common.extend((name.len() as u16).to_le_bytes());
        common.extend(0u16.to_le_bytes());

        out.write_all(&LOCAL.to_le_bytes())?;
        out.write_all(&common)?;
        out.write_all(name.as_bytes())?;
        out.write_all(bytes)?;

        directory.extend(CENTRAL.to_le_bytes());
        directory.extend(20u16.to_le_bytes());
        directory.extend(common);
        directory.extend([0; 10]);
        directory.extend(position.to_le_bytes());
        directory.extend(name.as_bytes());
        position += 30 + name.len() as u32 + size;
    }
    out.write_all(&directory)?;
    let count = entries.len() as u16;
    out.write_all(&END.to_le_bytes())?;
    out.write_all(&[0; 4])?;
    out.write_all(&count.to_le_bytes())?;
    out.write_all(&count.to_le_bytes())?;
    out.write_all(&(directory.len() as u32).to_le_bytes())?;
    out.write_all(&position.to_le_bytes())?;
    out.write_all(&[0; 2])
}

pub(super) fn read<'a>(
    bytes: &'a [u8],
    limits: &[(&'static str, usize)],
) -> Result<BTreeMap<&'static str, &'a [u8]>, String> {
    let short = |i: usize| Some(u16::from_le_bytes(bytes.get(i..i + 2)?.try_into().ok()?));
    let long = |i: usize| Some(u32::from_le_bytes(bytes.get(i..i + 4)?.try_into().ok()?));
    let end = bytes.len().checked_sub(22).ok_or_else(invalid)?;
    let seek = |i| long(i).map(|n| n as usize).ok_or_else(invalid);
    let count = short(end + 10).ok_or_else(invalid)? as usize;
    let directory = seek(end + 16)?;
    if long(end) != Some(END)
        || long(end + 4) != Some(0)
        || short(end + 8) != Some(count as u16)
        || short(end + 20) != Some(0)
        || directory.checked_add(seek(end + 12)?) != Some(end)
        || count > limits.len()
    {
        return Err(invalid());
    }

    let mut result = BTreeMap::new();
    let mut index = directory;
    let mut local = 0;
    for _ in 0..count {
        let name_size = short(index + 28).ok_or_else(invalid)? as usize;
        let size = seek(index + 24)?;
        let name = bytes
            .get(index + 46..index + 46 + name_size)
            .ok_or_else(invalid)?;
        let &(name, limit) = limits
            .iter()
            .find(|&&(key, _)| key.as_bytes() == name)
            .ok_or_else(invalid)?;
        if long(index) != Some(CENTRAL)
            || long(index + 8) != Some(0)
            || seek(index + 20)? != size
            || long(index + 30) != Some(0)
            || short(index + 34) != Some(0)
            || seek(index + 42)? != local
            || size > limit
            || long(local) != Some(LOCAL)
            || long(local + 6) != Some(0)
            || seek(local + 18)? != size
            || seek(local + 22)? != size
            || short(local + 26) != Some(name_size as u16)
            || short(local + 28) != Some(0)
            || long(local + 14) != long(index + 16)
            || bytes.get(local + 30..local + 30 + name_size) != Some(name.as_bytes())
        {
            return Err(invalid());
        }
        let data_start = local + 30 + name_size;
        local = data_start
            .checked_add(size)
            .filter(|&n| n <= directory)
            .ok_or_else(invalid)?;
        let data = bytes.get(data_start..local).ok_or_else(invalid)?;
        if long(index + 16) != Some(checksum(data)) || result.insert(name, data).is_some() {
            return Err(invalid());
        }
        index += 46 + name_size;
    }
    if index != end || local != directory {
        return Err(invalid());
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stored_zip_checks_content_and_entry_boundaries() {
        let mut bytes = Vec::new();
        write(
            &mut bytes,
            &[("manifest.json", b"version 1"), ("shortcuts.tsv", b"app")],
        )
        .unwrap();
        assert_eq!(checksum(b"123456789"), 0xcbf4_3926);
        let limits = [("manifest.json", 24), ("shortcuts.tsv", 24)];
        assert_eq!(read(&bytes, &limits).unwrap()["shortcuts.tsv"], b"app");
        assert!(read(&bytes, &[("../manifest.json", 24), ("shortcuts.tsv", 24)]).is_err());
        bytes[30 + "manifest.json".len()] ^= 1;
        assert!(read(&bytes, &limits).is_err());
    }
}
