// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use crate::{
    Entry,
    library::{EntryKind, OpenTarget, ParamType, new_id},
};
use std::{collections::HashSet, io::Read, path::Path};

const MAX_BYTES: usize = 32 * 1024 * 1024;
const MAX_BOOKMARKS: usize = 50_000;

#[derive(Debug)]
pub struct Bookmark {
    pub title: String,
    pub url: String,
    pub folders: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BookmarkStatus {
    Ready,
    Existing,
    Duplicate,
    Unsupported,
}

impl BookmarkStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Ready => "",
            Self::Existing => "ui.browser_bookmarks_existing",
            Self::Duplicate => "ui.browser_bookmarks_duplicate",
            Self::Unsupported => "ui.browser_bookmarks_unsupported",
        }
    }
}

pub struct PreviewRow {
    pub bookmark: Bookmark,
    pub alias: String,
    pub status: BookmarkStatus,
    pub checked: bool,
}

pub fn read(path: &Path) -> Result<Vec<Bookmark>, String> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .and_then(|file| file.take(MAX_BYTES as u64 + 1).read_to_end(&mut bytes))
        .map_err(|error| {
            crate::localization::message(
                "ui.could_not_read_shortcuts_error",
                &[("error", &error.to_string())],
            )
        })?;
    parse(&bytes)
}

enum Capture {
    Folder(String),
    Anchor(String, String),
}

pub fn parse(bytes: &[u8]) -> Result<Vec<Bookmark>, String> {
    if bytes.len() > MAX_BYTES {
        return Err("ui.browser_bookmarks_limit".into());
    }
    let decoded =
        crate::library::decode(bytes).map_err(|_| "ui.browser_bookmarks_bad_file".to_owned())?;
    let mut remaining = decoded.as_str();
    let mut output = Vec::new();
    let mut levels: Vec<Option<String>> = Vec::new();
    let mut pending_folder = None;
    let mut capture = None;
    let mut browser_format = false;
    let mut raw_text = "";
    let mut copied_folder_bytes = 0;
    while let Some(index) = remaining.find('<') {
        if let Some(capture) = capture.as_mut()
            && raw_text.is_empty()
        {
            match capture {
                Capture::Folder(text) | Capture::Anchor(_, text) => {
                    text.push_str(&remaining[..index]);
                }
            }
        }
        remaining = &remaining[index + 1..];
        if let Some(comment) = remaining.strip_prefix("!--") {
            let Some(end) = comment.find("-->") else {
                break;
            };
            remaining = &comment[end + 3..];
            continue;
        }
        if !remaining.starts_with(|ch: char| ch.is_ascii_alphabetic() || matches!(ch, '/' | '!')) {
            if let Some(capture) = capture.as_mut() {
                match capture {
                    Capture::Folder(text) | Capture::Anchor(_, text) => text.push('<'),
                }
            }
            continue;
        }
        let Some(end) = tag_end(remaining) else { break };
        let tag = &remaining[..end];
        remaining = &remaining[end + 1..];
        let mut parts = tag.trim().splitn(2, char::is_whitespace);
        let name = parts.next().unwrap_or("").to_ascii_lowercase();
        let attrs = parts.next().unwrap_or("");
        if !raw_text.is_empty() {
            if name.strip_prefix('/') == Some(raw_text) {
                raw_text = "";
            }
            continue;
        }
        match name.as_str() {
            "!doctype" => {
                browser_format |= attrs
                    .trim()
                    .eq_ignore_ascii_case("NETSCAPE-Bookmark-file-1")
            }
            "script" | "style" => raw_text = if name == "script" { "script" } else { "style" },
            "h3" if !levels.is_empty() => capture = Some(Capture::Folder(String::new())),
            "/h3" => {
                if let Some(Capture::Folder(text)) = capture.take() {
                    pending_folder = Some(display_text(&text));
                }
            }
            "dl" => {
                if levels.len() == 64 {
                    return Err("ui.browser_bookmarks_limit".into());
                }
                levels.push(pending_folder.take());
            }
            "/dl" => {
                levels.pop();
                pending_folder = None;
            }
            "dt" => pending_folder = None,
            "a" if !levels.is_empty() => {
                capture = Some(Capture::Anchor(
                    attribute(attrs, "href").map_or_else(String::new, decode_entities),
                    String::new(),
                ));
            }
            "/a" => {
                if let Some(Capture::Anchor(url, text)) = capture.take() {
                    if output.len() == MAX_BOOKMARKS {
                        return Err("ui.browser_bookmarks_limit".into());
                    }
                    let folders: Vec<_> = levels
                        .iter()
                        .flatten()
                        .filter(|folder| !folder.is_empty())
                        .cloned()
                        .collect();
                    copied_folder_bytes += folders.iter().map(String::len).sum::<usize>();
                    if copied_folder_bytes > MAX_BYTES {
                        return Err("ui.browser_bookmarks_limit".into());
                    }
                    output.push(Bookmark {
                        title: display_text(&text),
                        url: url.trim().to_owned(),
                        folders,
                    });
                }
            }
            _ => {}
        }
    }
    if !browser_format || output.is_empty() {
        return Err("ui.browser_bookmarks_bad_file".into());
    }
    Ok(output)
}

fn tag_end(tag: &str) -> Option<usize> {
    let mut quote = None;
    for (index, byte) in tag.bytes().enumerate() {
        if let Some(delimiter) = quote {
            if byte == delimiter {
                quote = None;
            }
        } else if matches!(byte, b'\'' | b'"') {
            quote = Some(byte);
        } else if byte == b'>' {
            return Some(index);
        }
    }
    None
}

fn attribute<'a>(mut attrs: &'a str, wanted: &str) -> Option<&'a str> {
    while !attrs.is_empty() {
        attrs = attrs.trim_start();
        let index = attrs
            .find(|ch: char| ch.is_whitespace() || ch == '=')
            .unwrap_or(attrs.len());
        let name = &attrs[..index];
        attrs = attrs[index..].trim_start();
        let Some(value) = attrs.strip_prefix('=') else {
            continue;
        };
        let value = value.trim_start();
        let (value, rest) = if let Some(delimiter @ ('\'' | '"')) = value.chars().next() {
            value[1..].split_once(delimiter)?
        } else {
            let end = value.find(char::is_whitespace).unwrap_or(value.len());
            value.split_at(end)
        };
        if name.eq_ignore_ascii_case(wanted) {
            return Some(value);
        }
        attrs = rest;
    }
    None
}

fn decode_entities(mut text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    while let Some((before, rest)) = text.split_once('&') {
        output.push_str(before);
        let token = rest
            .get(..rest.char_indices().nth(16).map_or(rest.len(), |(i, _)| i))
            .and_then(|prefix| prefix.split_once(';'))
            .map(|(name, _)| name);
        let value = token.and_then(|entity| match entity {
            "amp" | "AMP" => Some('&'),
            "quot" | "QUOT" => Some('"'),
            "apos" => Some('\''),
            "lt" | "LT" => Some('<'),
            "gt" | "GT" => Some('>'),
            "nbsp" => Some('\u{a0}'),
            _ => entity.strip_prefix('#').and_then(|number| {
                let number = if let Some(hex) = number.strip_prefix(['x', 'X']) {
                    u32::from_str_radix(hex, 16)
                } else {
                    number.parse()
                };
                number.ok().and_then(char::from_u32)
            }),
        });
        if let Some((entity, ch)) = token.zip(value) {
            output.push(ch);
            text = &rest[entity.len() + 1..];
        } else {
            output.push('&');
            text = rest;
        }
    }
    output.push_str(text);
    output
}

fn display_text(text: &str) -> String {
    decode_entities(text)
        .split(|ch: char| ch.is_whitespace() || ch.is_control())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

pub(crate) fn url_key(url: &str) -> Option<(String, String)> {
    if url.chars().any(|ch| {
        ch.is_control()
            || ch.is_whitespace()
            || matches!(ch, '\\' | '"' | '<' | '>' | '\u{e000}' | '\u{e001}')
    }) {
        return None;
    }
    let (scheme, rest) = url.split_once("://")?;
    let scheme = if scheme.eq_ignore_ascii_case("http") {
        "http"
    } else if scheme.eq_ignore_ascii_case("https") {
        "https"
    } else {
        return None;
    };
    let authority_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let (authority, tail) = rest.split_at(authority_end);
    let (user, server) = authority
        .rsplit_once('@')
        .map_or(("", authority), |(user, server)| {
            (&authority[..user.len() + 1], server)
        });
    if user.matches('@').count() > 1 {
        return None;
    }
    let (host, port) = if let Some(rest) = server.strip_prefix('[') {
        let (ip, port) = rest.split_once(']')?;
        ip.parse::<std::net::Ipv6Addr>().ok()?;
        (&server[..ip.len() + 2], port)
    } else {
        let host_end = server.find(':').unwrap_or(server.len());
        let (host, port) = server.split_at(host_end);
        let labels = host.strip_suffix('.').unwrap_or(host);
        if labels.split('.').any(|label| {
            label.is_empty()
                || label.starts_with('-')
                || label.ends_with('-')
                || label
                    .chars()
                    .any(|ch| !ch.is_alphanumeric() && !matches!(ch, '_' | '-'))
        }) {
            return None;
        }
        (host, port)
    };
    let default_port = if scheme == "https" { 443 } else { 80 };
    let port = if port.is_empty() {
        None
    } else {
        let port = port.strip_prefix(':')?;
        if port.is_empty() || port.bytes().any(|ch| !ch.is_ascii_digit()) {
            return None;
        }
        Some(port.parse::<u16>().ok()?).filter(|&port| port != default_port)
    };
    let host = canonical_host(host)?;
    let port = port.map_or_else(String::new, |port| format!(":{port}"));
    let separator = if tail.starts_with('/') { "" } else { "/" };
    Some((
        format!("{scheme}://{user}{host}{port}{separator}{tail}"),
        host,
    ))
}

fn canonical_host(host: &str) -> Option<String> {
    if host.is_ascii() {
        return Some(host.to_ascii_lowercase());
    }
    #[cfg(windows)]
    {
        use std::ptr::null_mut;
        use windows_sys::Win32::Globalization::IdnToAscii;
        let host: Vec<u16> = host.encode_utf16().collect();
        let len = i32::try_from(host.len()).ok()?;
        let size = unsafe { IdnToAscii(0, host.as_ptr(), len, null_mut(), 0) };
        if size <= 0 {
            return None;
        }
        let mut ascii = vec![0; size as usize];
        let written = unsafe { IdnToAscii(0, host.as_ptr(), len, ascii.as_mut_ptr(), size) };
        (written == size).then(|| String::from_utf16_lossy(&ascii).to_ascii_lowercase())
    }
    #[cfg(not(windows))]
    Some(host.to_lowercase())
}

fn entry_url(command: &str) -> Option<String> {
    let command = command.trim();
    let command = ["@+", "@-", "@"]
        .into_iter()
        .find_map(|prefix| command.strip_prefix(prefix))
        .unwrap_or(command)
        .trim_start();
    let command = command
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or(command);
    url_key(command).map(|(key, _)| key)
}

fn alias_for(title: &str, host: &str) -> String {
    let mut out = String::new();
    let mut separator = false;
    if title.is_ascii() {
        for ch in title.chars() {
            if ch.is_ascii_alphanumeric() {
                if separator && !out.is_empty() {
                    out.push('-');
                }
                out.push(ch.to_ascii_lowercase());
                separator = false;
                if out.len() >= 48 {
                    break;
                }
            } else {
                separator = true;
            }
        }
    }
    if out.is_empty() {
        let host = host
            .strip_prefix("www.")
            .unwrap_or(host)
            .trim_end_matches('.');
        if host.is_ascii()
            && host
                .bytes()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, b'.' | b'-' | b'_'))
        {
            out = host.to_owned();
        } else if host.starts_with('[') && host.ends_with(']') {
            out = format!("ipv6-{}", host[1..host.len() - 1].replace(':', "-"));
        } else {
            out = "website".into();
        }
    }
    out
}

pub fn preview(bookmarks: Vec<Bookmark>, current: &[Entry]) -> Vec<PreviewRow> {
    use std::collections::HashMap;
    let known: HashSet<String> = current
        .iter()
        .filter(|entry| entry.kind == EntryKind::Command)
        .filter_map(|entry| entry_url(&entry.command))
        .collect();
    let mut seen = HashSet::new();
    let mut aliases: HashSet<String> = current
        .iter()
        .map(|entry| entry.alias.trim().to_lowercase())
        .collect();
    let mut suffixes = HashMap::new();
    bookmarks
        .into_iter()
        .map(|mut bookmark| {
            let target = url_key(&bookmark.url);
            let status = match &target {
                None => BookmarkStatus::Unsupported,
                Some((key, _)) if known.contains(key) => BookmarkStatus::Existing,
                Some((key, _)) if !seen.insert(key.clone()) => BookmarkStatus::Duplicate,
                Some(_) => BookmarkStatus::Ready,
            };
            let alias = if let Some((_, host)) = target.filter(|_| status == BookmarkStatus::Ready)
            {
                let alias = alias_for(&bookmark.title, &host);
                if bookmark.title.is_empty() {
                    bookmark.title = host;
                }
                if aliases.insert(alias.clone()) {
                    alias
                } else {
                    let suffix = suffixes.entry(alias.clone()).or_insert(2);
                    loop {
                        let candidate = format!("{alias}-{suffix}");
                        *suffix += 1;
                        if aliases.insert(candidate.clone()) {
                            break candidate;
                        }
                    }
                }
            } else {
                String::new()
            };
            PreviewRow {
                bookmark,
                alias,
                status,
                checked: status == BookmarkStatus::Ready,
            }
        })
        .collect()
}

pub fn selected_entries(rows: &[PreviewRow], current: &[Entry]) -> Result<Vec<Entry>, String> {
    let mut aliases: HashSet<String> = current
        .iter()
        .map(|entry| entry.alias.trim().to_lowercase())
        .collect();
    rows.iter()
        .filter(|row| row.checked && row.status == BookmarkStatus::Ready)
        .map(|row| {
            let alias = row.alias.trim();
            if alias.is_empty() {
                return Err("ui.enter_a_shortcut_keyword".into());
            }
            if !aliases.insert(alias.to_lowercase()) {
                return Err(crate::localization::message(
                    "ui.browser_bookmarks_alias_collision",
                    &[("alias", alias)],
                ));
            }
            let entry = Entry {
                alias: alias.into(),
                name: row.bookmark.title.clone(),
                command: row.bookmark.url.clone(),
                frequency: 0,
                param_type: ParamType::None,
                id: new_id(),
                working_directory: String::new(),
                open_target: Some(OpenTarget::WebUrl),
                kind: EntryKind::Command,
            };
            crate::library::validate(&entry)?;
            Ok(entry)
        })
        .collect()
}
