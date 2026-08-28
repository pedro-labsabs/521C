---
name: 521c-bluez
description: Use when working on Bluetooth transport in 521C, such as BlueZ D-Bus code in qcy-transport or 521c-desktop, RFCOMM/SPP probing, mock transport, or write authorization. Grounds the agent in the repository's transport architecture and host-safety rules.
allowed-tools: bash, read, write, edit, apply_patch, agentgrep, todo
---

# 521C BlueZ / Bluetooth transport

The native path talks to the system BlueZ stack through its D-Bus GATT API. It
never shells out to interactive tools, never requires root, and never
reconfigures the daemon.

## Architecture facts (read the code before editing)

- `native/crates/qcy-transport/src/bluez.rs` — BlueZ D-Bus backend (issue #7):
  object-path mapping (`normalize_mac`, `device_path`), discovery,
  characteristic resolution; isolated behind the `BlueZBus` trait so logic is
  unit-testable against a fake bus.
- `native/crates/qcy-transport/src/mock.rs` — deterministic hardware-free
  transport used by tests and development.
- `native/crates/qcy-transport/src/policy.rs` — central `WritePolicy` /
  `Denial`; every outbound operation is checked against it.
- `native/crates/qcy-transport/src/rfcomm.rs` — classic/SPP path; helper
  scripts `scripts/sdp-rfcomm-channel.py`, `scripts/spp-probe.sh` exist for
  diagnostics.
- `native/crates/qcy-transport/src/lib.rs` — `Transport` trait, error types.
- 521C must cooperate with BlueZ, never replace it.

## Hard rules

- Normal runtime stays non-root; do not reconfigure, mask, or restart BlueZ as
  part of application operation.
- Unknown/generic QCY devices are read-only by default.
- No write may skip the central `WritePolicy`. Destructive opcodes (`0x01` reset
  defaults, `0x02` clear pairing, `0x03` factory reset), firmware OTA and Find
  Earbuds firing are unreachable from unattended automation.
- `0x01`–`0x03` are forbidden to unattended automation at every layer.
- A failed/ambiguous real-device operation is evidence to investigate, not
  permission to send increasingly speculative commands. Do not probe unknown
  opcodes against hardware.

## Workflow hints

- Prefer `MockTransport` for tests; keep BlueZ specifics behind traits.
- Run `cargo test -p qcy-transport` (and the full `cargo test`) before
  declaring transport changes complete.
- Confirm any protocol assumption against `docs/PROTOCOL.md` and the shared
  conformance vectors in `conformance/`; never invent bytes.
- Read `docs/HOST_SAFETY.md` before touching the host or hardware.