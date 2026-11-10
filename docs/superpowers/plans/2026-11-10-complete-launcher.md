# Complete ALTRun implementation

> Execute continuously inline with superpowers:executing-plans. User explicitly requires completion of the original whole-software scope; do not end the task at an intermediate feature checkpoint. Use the existing codex/performance-spike branch and ledger. One whole-change review after implementation, then fix findings and finish verification.

**Goal:** Complete the original Windows-only launcher feature matrix, with native compact UI and user-acceptable responsiveness.
**Sources:** docs/ALTRun-modernization-plan.md; DESIGN.md; UX-CONTRACT.md; upstream source audit cached under downloads/upstream-audit (all 25 Form/Unit files). Keep original binary/configuration and current user data intact.

## Execution and completion checklist

- [ ] Audit every original menu/action/config key; keep an implementation/evidence map, not a list of future promises.
- [ ] Matching: literal/prefix/anywhere, legacy wildcard preprocessing, real regex/classes/anchors/alternation, isolated bounded advanced regex, case handling, stable ties, frequency/favourite/recent sorting, top ten/all results.
- [ ] Persistent runtime: stable shortcut IDs, usage counts, latest list, query favourites, parameter histories and limits, window positions; background serialized atomic persistence and orderly shutdown.
- [ ] Template/execution: raw/ANSI/UTF8 types, %p/{%p}, clipboard and foreground HWND/title/class, environment, explicit working directory and relative paths, @/@-/@+ window modes, shell namespace/URLs/files/programs/scripts, copy command/open folder, failure recovery.
- [ ] Keyboard/mouse: Enter/Space, digits/numpad and Ctrl/Alt digits with 0/1 indexing, semicolon/quote, Up/Down/Tab/Page, Ctrl+C/D/L, F1/F2/Insert/Delete, middle mouse/wheel, IME and cancellation, unique-result execution policy.
- [ ] Settings: all original functional preferences, hotkeys including latest-list key and conflict rollback, data/compatibility options, fonts/colours/width/opacity/corners/background/skin/visibility/sound/language, defaults/reset and validation.
- [ ] Native lifecycle: resident resources, single instance/argument forwarding, drag/drop/SendTo intake, tray/restart/help/about, auto hide/deactivate/exit policy, monitor positioning/DPI changes/high contrast/theme, shutdown handling.
- [ ] Management completion: explicit file/folder pickers, drag/drop, duplicates, separators/remarks, validation preview with unconfirmed network/special targets, reversible explicit deletion, absolute/relative conversion, text/export/backup/restore, persisted selection/geometry.
- [ ] OS integration: per-user startup and SendTo with real status/error recovery; runtime changes only on explicit settings actions. Verify in isolated cloud test namespaces; do not enable them on the user's machine during development.
- [ ] Migration: old shortcut/INI/favourite/parameter-history data, retained originals and provenance, modern appearance default with legacy appearance import option, explicit differences. Coexistence keeps Ctrl+Alt+R until user selects original keys; conflicts never silently remove working bindings.
- [ ] Verification: synthetic core/native regression, worker timeout/cancellation, stale result isolation, persistence/conflicts, integration fixtures, UI snapshots/DPI/empty/error/IME flows, idle/resource sanity and meaningful stress. No desktop-frame timing experiments and no statistical-parity gate.
- [ ] Package: pin/commit dependency lock, cloud fmt/tests/clippy/release, one complete review and fix pass, verify exact artifact/hash, update D: trial and final documentation. Do not call a partial feature matrix complete.

## Architecture decisions

Retain Rust/windows-sys and cached native Win32/GDI controls. Simple queries bypass regex compilation; regular expressions use a small linear engine, with backtracking features on a bounded on-demand worker process and generation-checked results. Settings and state live beside each data file; no hotpath file/network access. Managers/settings run on-demand away from the resident UI. Persistence publishes only newer snapshots. Side effects remain explicit user actions. New crates are pinned and their lock file is bootstrapped once in cloud, then --locked is restored.

## Scope interpretation

Functional parity covers usable original capabilities, not replication of crashes, unsafe command ambiguity, dead websites, random tied-row order or identical old pixels. Preserve old settings/data and expose compatibility choices. Missing external WinCtl behavior must be implemented or mapped through the retained external command with explicit errors; never silently mark it supported.
