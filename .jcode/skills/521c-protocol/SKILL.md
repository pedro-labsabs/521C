---
name: 521c-protocol
description: Use when working on QCY protocol code or evidence in 521C, such as packet framing, advertisement parsing, capability truth, write authorization, conformance vectors, or docs/PROTOCOL.md. Enforces protocol honesty: no invented bytes, no claiming hardware support without evidence.
allowed-tools: bash, read, write, edit, apply_patch, agentgrep, todo
---

# 521C protocol honesty and evidence

Protocol work in 521C is governed by an evidence model. A feature in the
official QCY app, marketing material, or community claims does NOT prove a
Linux-accessible command. Unknown evidence is an acceptable state; a fabricated
supported state is a defect.

## Canonical sources

- `docs/PROTOCOL.md` — independent reverse-engineering notes; the reference
  for every vector.
- `conformance/protocol_vectors.json` — the single shared corpus of byte-level
  vectors consumed by BOTH implementations:
  - TS: `src/lib/qcy/protocol/conformance.test.ts` (vitest)
  - Rust: `native/crates/qcy-protocol/tests/conformance.rs` (cargo test)
  Any semantic divergence covered by a vector fails a repository gate. Every
  vector derives from `docs/PROTOCOL.md`; no guessed bytes.
- `conformance/config_vectors.json` / `capabilities_ht08.json` — config and
  capability truth vectors.

## Four truths (never conflate)

| Truth | Question |
| --- | --- |
| Hardware | Is the feature associated with this model? |
| Protocol | Is the behavior evidenced for this model/firmware? |
| Implementation | Does this build implement and test it? |
| Write | Writable, experimental (opt-in), read-only, or forbidden? |

Deterministic rules derive what is shown, enabled, and writable from those
truths. Host-side features (system EQ, auto game mode, codec status) are never
presented as QCY protocol capabilities and never generate earbud writes.

## Rules

- Never invent protocol facts. Every trusted write must be backed by the
  repository's evidence model and the central write policy
  (`qcy-transport::policy::WritePolicy`).
- `docs/SUPPORTED_DEVICES.md` is canonical for the per-feature readiness
  matrix; keep it in sync when evidence or implementation changes.
- When adding a vector, update both TS and Rust conformance consumers and the
  shared corpus; record provenance in the vector file.
- Writes to hardware require the central authorization layer plus sufficient
  evidence/provenance; unknown devices stay read-only.

## Workflow hints

- Read `docs/PROTOCOL.md` and the relevant device notes
  (`docs/devices/HT08.md`) before touching framing/parsing.
- Re-run `npm test` and `cargo test` when vectors change.
- Record uncertainty explicitly; prefer "unverified" over implied support.