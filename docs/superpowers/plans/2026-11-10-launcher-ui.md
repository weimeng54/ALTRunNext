# Compact launcher UI and basic execution plan

> Implement inline with superpowers:executing-plans; use existing isolated codex/performance-spike branch.

**Goal:** Deliver a polished compact native launcher with basic real execution, without another measurement project.
**Architecture:** Retain persistent EDIT/LISTBOX/GDI. Theme constants and fonts cached at startup. A pure command resolver feeds ShellExecuteExW on an execution worker; UI/search never wait for execution. Keep safe default for synthetic tests; user shortcut opts in.
**Tech Stack:** Existing Rust/windows-sys, Windows cloud build, no local compiler.
**Spec:** DESIGN.md; docs/validation/performance-gate.md; user approved current feel and requested UI improvement and continued development.

## Constraints and decisions

- User has authorized direct implementation; no new design approval loop.
- No screenshot observer changes or quiet-desktop requests.
- Keep original configuration/process untouched. New package separate from running trial.
- No full compatibility claim. Unsupported placeholders fail visibly.
- Review focus: quoted/spaced paths, shell metacharacters without implicit shell, IME Enter, launch failure recovery, DPI/empty-state clipping.

## Tasks

1. Add launch request/resolution tests and stub; cloud test must demonstrate missing implementation. Tests cover quoted executable and arguments, URL, %p, unsupported placeholders, unmatched quotes and embedded NUL. Commit/sync one change at a time.
2. Implement resolver and native UI refinement, opt-in execution, parameter input, recoverable errors, context-menu exit and DPI font rebuild. Verify cloud fmt/tests/clippy/release; capture synthetic screenshots in cloud, not user's desktop.
3. One whole-change review, fix important issues, verify exact artifact SHA256 and install launcher-only trial package on D:. Provide concise usage and actual unfinished compatibility scope. User feedback replaces strict statistical performance gating.
