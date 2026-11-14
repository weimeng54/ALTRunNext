// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::Bookmark;
use std::{io::Read, path::Path};

const MAX_BYTES: usize = 32 * 1024 * 1024;
const MAX_BOOKMARKS: usize = 50_000;

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
