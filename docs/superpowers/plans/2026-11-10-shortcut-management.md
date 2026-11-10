# Shortcut management and parameter compatibility

> Execute inline with superpowers:executing-plans. User authorized continued implementation; retain existing codex/performance-spike branch and accepted compact UI. No new approval loop or performance-observer work.

**Goal:** Make the portable launcher maintain its own shortcut library, preserve imported parameter types, and support common wildcard searches.
**Architecture:** Extend the backward-compatible UTF-8 TSV with an optional parameter-type column. Read-only legacy import decodes BOM/GB18030 and preserves header frequency/type and command tails. An on-demand native manager runs on its own UI thread, validates edits, detects ordinary external-file conflicts, and saves with same-directory staging plus a previous-version backup. Only after save succeeds does the resident launcher adopt the new snapshot. Launch encoding remains on its execution worker.
**Spec:** DESIGN.md; original feature matrix docs/ALTRun-modernization-plan.md; user accepted actual feel and current UI. Upstream behavioral evidence: untShortCutMan.pas StringToShortCutItem/StringToParamType/FilterKeyWord, untUtilities.pas GetURLQueryString/GetUTF8QueryString. Old encoder leaves ASCII letters and *@._- unchanged, replaces spaces with +, and encodes digits too. Original parameter markers include %p and {%p}.

## Boundaries

- No local compiler/runtime installation; all private files stay on D:, no personal data in GitHub.
- Original files/process remain unchanged. Update only the launcher-owned TSV, retain backup.
- Main show/search paths use resident data and do not await manager disk operations.
- Native manager reuses current theme, fonts, labels and validation. No browser/animations.
- Full regex/backreferences, clipboard/foreground templates, history/settings and complete legacy parity remain unfinished. This iteration supplies common * and ? matching, not a full regex claim.
- Legacy URL mode uses the original Windows ANSI code page; unrepresentable characters fail visibly. UTF-8 URL mode preserves Unicode.

## Tasks

1. Define Entry ParamType, import/store/encoding interfaces and regression tests with stubs; observe cloud failures. Tests: 4/5-column compatibility, legacy delimiters and command tails, malformed rows, type roundtrip, safe saves/conflicts, %p vs %path%, raw/UTF8/ANSI encoding, wildcard selection.
2. Implement pure functions and Windows codecs/atomic save. Add lazy native manager (Alt+S/tray), add/edit/delete/import, clear dirty/error states, Save/Close, parent data refresh only after success. Integrate typed parameter launch; ensure default safe mode remains non-executing. Native synthetic tests exercise management persistence without touching original data.
3. One whole-change read-only review, fix important findings with regressions; cloud fmt/tests/clippy/release; inspect synthetic UI; verify commit/hashes and update the existing D: trial with backup. Record actual supported scope and no false parity claims.

## Review focus

External edits/failed atomic replacement must not silently discard data; duplicate aliases and command separators must survive import/save; parameter encoding may not rewrite executable targets; a manager save must cancel stale frozen selection safely; management window/disk work must not stall the main window; unsaved edits and empty datasets need recovery.
