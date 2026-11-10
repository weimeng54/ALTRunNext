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

状态：本迭代三项任务已完成。精确源码 319e5e1 / Actions 42776482751 全部检查通过，launcher_ui_review 一次完整审查的两个 Important 问题已修复，62 项 Rust/Python 测试通过，合成 UI 已查看，产物哈希与本地安装已核对。后续工作回到完整旧功能对照表；本计划不重新启动截图计时实验。

1. Add launch request/resolution tests and stub; cloud test must demonstrate missing implementation. Tests cover quoted executable and arguments, URL, %p, unsupported placeholders, unmatched quotes and embedded NUL. Commit/sync one change at a time.
2. Implement resolver and native UI refinement, opt-in execution, parameter input, recoverable errors, context-menu exit and scaled font/layout checks. Dynamic monitor-DPI rebuilding remains a follow-up. Verify cloud fmt/tests/clippy/release; capture synthetic screenshots in cloud, not user's desktop.
3. One whole-change review, fix important issues, verify exact artifact SHA256 and install launcher-only trial package on D:. Provide concise usage and actual unfinished compatibility scope. User feedback replaces strict statistical performance gating.
