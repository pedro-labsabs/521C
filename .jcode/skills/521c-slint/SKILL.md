---
name: 521c-slint
description: Use when working on the native desktop UI of 521C in Slint (native/crates/521c-desktop), including main.slint, the Rust shell, typed state/actions, or UI tests. Grounds the agent in the desktop boundary rules and ADR 0001.
allowed-tools: bash, read, write, edit, apply_patch, agentgrep, todo
---

# 521C Slint desktop UI

The final desktop product is a native Rust application with a Slint UI:
`Slint UI -> Rust application/orchestration -> qcy-app core -> transport ->
host services`. See `docs/DESKTOP_ARCHITECTURE.md` (ADR 0001: single native
process, no IPC in v1, deliberate no-tray alternative) and
`docs/ARCHITECTURE.md`.

## Files

- `native/crates/521c-desktop/ui/main.slint` — the UI. Boundary rule: this file
  only sees typed state and capability metadata from the qcy-app core. Raw
  GATT bytes, opcodes and policy decisions never reach this layer; denials
  arrive as user-readable strings.
- `native/crates/521c-desktop/src/main.rs` — Rust shell: orchestration, event
  loop, wiring between qcy-app state and the Slint window.
- `native/crates/521c-desktop/Cargo.toml` — crate metadata used by AppImage
  packaging.
- `native/crates/521c-desktop/assets/` — icons (SVG + PNG sizes).

## Rules

- Keep the UI thin: no protocol codec, no raw byte handling, no write policy
  decisions in the UI layer. State flows through typed commands/events from
  qcy-app.
- A polished UI is not a finished feature: desktop work must still pass the
  release criteria in `docs/AUTONOMOUS_EXECUTION.md` and cannot present
  mock-only behavior as implemented hardware support.
- UI behavior (device state reflected after accepted writes, disable/deny
  presentation) must match what the core reports; do not fake state.

## Workflow hints

- Run `cargo build -p 521c-desktop` and `cargo test` for the workspace; use
  `scripts/test-desktop-close.sh` where relevant (see scripts/).
- When changing icons or metadata, verify `packaging/linux/` stays consistent
  with the crate (the AppImage uses both).
- Keep host-side features (MPRIS, PipeWire/system EQ, codec, Auto Game Mode)
  behind the host-services boundary; never present them as QCY protocol
  capabilities.