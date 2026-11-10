# 已停止执行

2026-11-10：用户明确以实际体验可接受为性能标准，允许感知不到的轻微指标退步。本观测器计划归档，不再增加采集代码或要求重复合成自测。后续推进启动器体验包和旧功能实现，现有拒收数据不代表启动器性能不达标。

---

# Desktop frame observer implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the slow GDI timing path with a calibrated, conservative desktop-frame observer before any feature expansion.

**Architecture:** Keep the Rust launcher unchanged. Reuse the independently calibrated Windows SDK DXGI readback through an owned, versioned C ABI; Rust validates frame timestamps and retains existing logical/focus/content diagnostics. System desktop-update timestamps and CPU readback completion remain separate, and all reports stay pending until independent content and full performance evidence exist.

**Tech Stack:** Existing Rust/windows-sys and cloud Windows SDK C++; no local toolchain installation or new runtime service.

**Spec:** `docs/validation/capture-calibration.md`, `docs/validation/performance-gate.md`, user-approved performance-first plan.

## Global constraints

- No change to original configuration/lifecycle, security/privacy settings, or launcher hot paths.
- Private data and binaries remain in ignored D-drive directories. No saved desktop pixels or hashes in public artifacts.
- Use software input QPC and matching desktop-image QPC on the same machine. Neither is physical input/display latency.
- Reject stale, accumulated, protected or malformed frames; no implicit GDI fallback that hides different measurement semantics.
- No full feature development or parity declaration from synthetic/calibration evidence.

## Review focus

- No desktop update vs mouse-only update: no fabricated zero response duration.
- Frame accumulation: latest image cannot impersonate each omitted frame.
- Slow GPU/CPU readback: do not add it to the image's timestamp or conceal observer interference.
- Multiple capture regions on the same monitor: share one duplication owner; no double-release or aliasing of mapped memory.
- Foreground transitions and protected content with desktop automation: reject, preserve diagnosis, use manual launch when necessary.

### Task 1: Validate desktop-frame timestamps

**Files:** `src/desktop_frame.rs`, `tests/desktop_frame.rs`, module declaration in `src/lib.rs`.

**Interface:** `DesktopFrame::stamp_after(action_qpc: i64, frequency: i64) -> Result<Stamp, Rejected>`; fields present/acquire-begin/acquire-end/readback-end QPC, accumulated-frame count, protection flag. `Stamp` exposes `elapsed_ns`, `acquisition_lag_ns`, `readback_lag_ns`; it is timing metadata only.

- [ ] Write cases for exact QPC conversion, unchanged image timestamp with delayed readback, pre-action frame, mouse-only, accumulated/protected frames, nonpositive frequency and reversed/future timestamps.
- [ ] Observe actual RED in standard cloud tests using a rejection-only stub.
- [ ] Implement checked nonnegative time differences and explicit rejection reasons; no default/clamping on stale images.
- [ ] Run complete cloud checks and commit verified source.

### Task 2: Owned capture bridge

**Files:** dedicated C ABI header/implementation under `tools/`, Rust wrapper under `src/`, cloud build/package step.

**Interface:** versioned opaque capture owner, absolute physical region, bounded poll, caller-owned BGRA output and frame metadata, explicit close; one duplication owner per output shared by region handles. No pointers to temporary mapped memory cross the ABI.

ABI version 1: `Region` is four i32 values (`left, top, width, height`). `FrameInfo` is `size:u32, version:u32`, four i64 QPC values (`present, acquire_begin, acquire_end, readback_end`), then `accumulated_frames:u32, protected_content:u32`. Exports: `altrun_capture_abi_version()->u32`, `altrun_capture_open(const Region*, Capture**)->i32`, `altrun_capture_poll(Capture*, timeout_ms:u32, bgra:*mut u8, length:u64, FrameInfo*)->i32`, `altrun_capture_close(Capture*)`. Status 0 is a frame/success, 1 is timeout; failures are negative HRESULT values. Poll timeout is at most 50ms; dimensions at most 4096 and region must fit one unrotated output. Close accepts null. Handles and mapped memory remain owned by the DLL, pixels copied only into the caller's checked capacity.

- [ ] Add synthetic ABI/crop/stride/lifetime tests, watch RED, then implement with existing WARP content fixture and RAII.
- [ ] Verify missing DLL/version mismatch/invalid region are explicit errors. Load only the exact adjacent verified DLL.
- [ ] Run complete cloud suite; verify DLL+EXE manifest and exact source before local use.

### Task 3: Integrate diagnostic replay and original comparison

**Files:** `src/bin/altrun-measure.rs`, observation tests, private profiles, public validation status.

- [ ] Add replay regressions for stale/accumulated/protected frames and preserve partial reports on rejection; observe RED before integration.
- [ ] Keep capture buffers/data/fonts ready before timed actions, use QPC across the boundary, attach both image timestamp and observation/readback times.
- [ ] Preserve focus/geometry/list transition checks and content-unverified classification; retain strict original identity/restore/cleanup behavior.
- [ ] Verify synthetic replay without original access first. Coordinate desktop availability only for input replay.
- [ ] Only after valid replay prepare a concrete same-integrity original comparison package for manual administrator launch; never automate UAC.
- [ ] Review raw traces and observer gaps before expanding samples; idle/IME/load/long-run gates remain required.

Self-review: this plan changes only measurement infrastructure. Independent glyph correctness and full user acceptance remain unresolved, explicitly required, and cannot be inferred from timestamp precision.
