// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use crate::{
    Entry,
    library::{EntryKind, OpenTarget, ParamType, new_id},
};
use std::collections::HashSet;

mod html;
pub use html::{parse, read};

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
