# ALTRun native interaction contract

Audience: experienced Chinese Windows ALTRun users. Sources: user-approved compact design, DESIGN.md, docs/ALTRun-modernization-plan.md feature matrix and current execution plan. Actual perceived responsiveness is the acceptance criterion.

## Canonical ownership

| Capability | Owner | Contract | Verification |
|---|---|---|---|
| Launcher selection | native.rs resident owner-drawn LISTBOX | mouse selects; input retains focus; frozen parameter target cannot change | native regression |
| Management list/form | manager.rs on-demand native window | draft edits, add/delete/import, explicit Save, unsaved-close confirmation | management regression and synthetic screenshots |
| Parameter type | library.rs ParamType plus native COMBOBOX | Automatic, None, Raw, ANSI URL, UTF-8 URL share one enum/serializer | roundtrip and encoding tests |
| Scrollbars/focus | Windows native controls | visible native scrollbars, tab navigation, focus cue | native interaction |
| Validation/persistence | library.rs Snapshot | validate complete dataset; stage/flush; backup previous file; reject changed source; publish after success | file save/failure/conflict tests |
| Feedback | owning window's status field | errors remain visible with edits intact; successful save stays in manager | UI regression |

## State and recovery

Main launcher uses only its resident snapshot. Management opens on a separate UI thread; it never makes hotkey/search wait for file operations. The manager loads a fresh file snapshot when opened. Failed saves leave the draft and active launcher data intact. Successful saves update the resident launcher without stealing focus and cancel any stale parameter selection. Original files are read-only import sources; the new data file is the editing destination.

List changes remain a draft until Save. Duplicate aliases are supported. Delete confirms the named entry; importing replaces only the draft after confirmation. Closing a dirty manager asks before discarding. A main-program exit with a manager open directs the user to save/close it first. Empty libraries are valid and keep Add/Import available. Parameter input Enter only launches after composition is committed; manager form Enter does not implicitly save while typing.

Keyboard: Alt+R launcher hotkey; Alt+S manager; Up/Down/Tab result selection. Manager controls use native Tab/Shift+Tab navigation and Space for focused buttons. Search is immediate, with no debounce. Queries are private; selected-query preferences and execution/parameter history remain only in the local sidecar. No URL state, telemetry or network lookup.

Theme values live in theme.rs and are documented in DESIGN.md. Native controls own input, IME, selection and dropdown geometry. Light/dark/system and high contrast are supported; main fonts and layout update on DPI changes. Real multi-monitor behavior is also subject to user day-to-day acceptance.

## Settings, background state and execution

Settings run on an independent UI thread. Validate values and language files before saving. Hotkey reservation failure preserves current bindings; file failure cancels reservations and restores integrations where possible. Startup/SendTo changes occur only on explicit Save, under separate names. Reload/import cannot overwrite a newer saved library.

Stable IDs associate counts, favourites, recents and parameters across edits and reorderings. State writes coalesce off the UI thread; errors are visible, and shutdown flushes the writer. Advanced regex runs in a bounded cancellable child with generation checks; stale results cannot launch or replace a newer query. Automatic unique-result execution requires user text input, and asynchronous results require the launch window still active.

No-match Enter confirms adding; cancelling leaves the search visible. F2/Delete require a selected result, Insert prefills the query. Parameter entry freezes its target and supports history; Esc returns without execution. Sorting, path conversion and missing-target removal remain drafts, can be undone, and require Save. Unknown/network/special targets are not automatically classified as missing.
