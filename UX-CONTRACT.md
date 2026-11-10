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

Keyboard: Ctrl+Alt+R launcher hotkey; Alt+S manager; Up/Down/Tab result selection. Manager controls use native Tab/Shift+Tab navigation and Space for focused buttons. Search is immediate, with no debounce. Search state is transient and private; no URL state, telemetry or network lookup.

Theme values live in theme.rs and are documented in DESIGN.md. Native controls own input, IME, selection and dropdown geometry. Light theme only for this iteration; high contrast and dynamic cross-monitor DPI remain explicitly unfinished.
