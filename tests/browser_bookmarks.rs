// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use altrun_probe::{
    browser_bookmarks::{BookmarkStatus, parse, preview, selected_entries},
    launch::{LaunchContext, resolve_entry},
    library,
};

const CHROMIUM: &str = r#"<!DOCTYPE NETSCAPE-Bookmark-file-1>
<!-- This is an automatically generated file. -->
<META HTTP-EQUIV="Content-Type" CONTENT="text/html; charset=UTF-8">
<TITLE>Bookmarks</TITLE>
<H1>Bookmarks</H1>
<DL><p>
  <DT><H3 PERSONAL_TOOLBAR_FOLDER="true">工作 &amp; 学习</H3>
  <DL><p>
    <DT><A HREF="https://Example.TEST/path?q=1&amp;k=%p#A" ICON="data:ignored">中文资料</A>
    <DT><H3>语言</H3>
    <DL><p>
      <DT><A HREF='https://example.test/path?q=1&amp;k=%p#B'>Rust &gt; Guide</A>
    </DL><p>
  </DL><p>
  <DT><A HREF="https://example.test/path?q=1&amp;k=%p#A">Duplicate</A>
  <DT><A HREF="javascript:alert(1)">Unsafe</A>
</DL><p>"#;

#[test]
fn browser_preview_preserves_folder_url_and_skips_unsafe_duplicates() {
    let bookmarks = parse(CHROMIUM.as_bytes()).unwrap();
    assert_eq!(bookmarks[0].folders, ["工作 & 学习"]);
    assert_eq!(bookmarks[1].folders, ["工作 & 学习", "语言"]);
    assert_eq!(bookmarks[1].title, "Rust > Guide");
    let current =
        altrun_probe::parse_entries("example.test\tExisting alias\tnotepad.exe\t0\n").unwrap();
    let rows = preview(bookmarks, &current);
    assert_eq!(rows[0].alias, "example.test-2");
    assert_eq!(rows[1].alias, "rust-guide");
    assert_eq!(rows[2].status, BookmarkStatus::Duplicate);
    assert_eq!(rows[3].status, BookmarkStatus::Unsupported);
    assert!(rows[0].checked && rows[1].checked);
    assert!(!rows[2].checked && !rows[3].checked);

    let rows = selected_entries(&rows, &current).unwrap();
    let data = library::serialize(&rows).unwrap();
    let restored = altrun_probe::parse_entries(&data).unwrap();
    assert_eq!(restored, rows);
    let (launch, _, _) = resolve_entry(&restored[0], None, &LaunchContext::default()).unwrap();
    assert_eq!(launch.file, "https://Example.TEST/path?q=1&k=%p#A");
    assert!(launch.parameters.is_empty());
}

#[test]
fn firefox_style_html_and_external_existing_urls_are_supported() {
    let html = br#"<!DOCTYPE NETSCAPE-Bookmark-file-1>
<DL><p><DT><A href=https://example.test>Test &#x2713;</A>
<DT><A href='https://example.test/?q=1'>Query</A>
<DT><A href='https://example.test:bad/'>Bad port</A>
<DT><A href='https:///no-host'>No host</A></DL>"#;
    let existing =
        altrun_probe::parse_entries("old\tExisting\tHTTPS://EXAMPLE.test:443/\t0\n").unwrap();
    let rows = preview(parse(html).unwrap(), &existing);
    assert_eq!(rows[0].bookmark.title, "Test ✓");
    assert_eq!(rows[0].status, BookmarkStatus::Existing);
    assert_eq!(rows[1].status, BookmarkStatus::Ready);
    assert_eq!(rows[2].status, BookmarkStatus::Unsupported);
    assert_eq!(rows[3].status, BookmarkStatus::Unsupported);
    assert!(
        parse(br#"<html><a href="https://example.test/">Regular web page</a></html>"#).is_err()
    );
}

#[test]
fn edited_aliases_are_checked_against_draft_and_other_selected_rows() {
    let mut rows = preview(parse(CHROMIUM.as_bytes()).unwrap(), &[]);
    rows[0].alias = " ClAsH ".into();
    rows[1].alias = "clash".into();
    assert!(selected_entries(&rows, &[]).is_err());
    rows[1].checked = false;
    assert_eq!(selected_entries(&rows, &[]).unwrap()[0].alias, "ClAsH");
}
