# Decision log

**Role:** decision index (ADR-style). Significant architecture/product
decisions are recorded here so future work does not reopen them without new
evidence.

Format: `docs/templates/adr.template.md`. Number sequentially
(`0001-<slug>.md`). A decision may also live in a dedicated document when it
needs more space; record it here with a pointer.

## Recorded decisions

| ID | Decision | Where |
| --- | --- | --- |
| 0000 | Fixed product/architecture defaults (Rust + Slint, BlueZ/D-Bus, AppImage, HT08 first, unknown devices read-only, no telemetry/OTA) | `docs/AUTONOMOUS_EXECUTION.md` §2 |
| 0001 | Desktop shell: single native process, Slint UI over the qcy-app core, no IPC in v1; deliberate no-tray alternative | `docs/DESKTOP_ARCHITECTURE.md` |

Reopening a recorded decision requires strong technical evidence that the
chosen path is infeasible (see `.prime/agent/APPEND_SYSTEM.md` /
`PRIME_AGENT_START.md` standing constraints), and produces a superseding ADR.
