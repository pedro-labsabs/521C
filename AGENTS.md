# AGENTS.md — 521C repository contract

This file is the operating contract for coding agents working in this repository. Read it before editing code, documentation, issues, or pull requests.

## 1. Mission

521C is an independent, unofficial, Linux-first control surface for QCY earbuds. The first device profile is QCY MeloBuds Pro / HT08. The project values protocol correctness, explicit uncertainty, safety, low overhead, and maintainable boundaries over feature count.

## 2. Sources of truth

Use this precedence when claims conflict:

1. Reproducible hardware observation or captured packet evidence.
2. Tested protocol implementation and fixtures.
3. `docs/PROTOCOL.md` and device notes.
4. Capability matrix in code / `docs/SUPPORTED_DEVICES.md`.
5. UI text, screenshots, marketing material, community claims.

Marketing or behavior in the official mobile app does not prove that a Linux-accessible protocol command exists.

## 3. Repository map

- `src/lib/qcy/protocol/` — TypeScript framing, commands, UUIDs, advertisement parsing.
- `src/lib/qcy/device/` — device profiles and capability matrix.
- `src/lib/qcy/transport.ts` — transport boundary; mock today, native/Web Bluetooth adapters behind the same contract.
- `src/lib/qcy/hub-store.ts` — application state and command orchestration.
- `src/components/` — UI. Raw GATT bytes must not leak here.
- `native/crates/qcy-protocol/` — Rust protocol core.
- `native/crates/521cctl/` — native CLI.
- `docs/` — protocol, architecture, device support, development and safety documentation.
- `scripts/` — focused repository checks.
- `src/routeTree.gen.ts` — generated; do not hand-edit in ordinary work.

## 4. Non-negotiable protocol rules

- Do not invent UUIDs, opcodes, vendor IDs, payload formats, checksums, firmware fields, or capability mappings.
- Treat BLE advertisements/notifications as untrusted input. Validate SOF, declared length, block bounds, enums/ranges, and timeouts.
- Add or update a fixture/parser test before enabling a new write path.
- Keep model-specific behavior in device profiles, not scattered `if model == ...` checks in UI code.
- Capability state must remain honest: `supported`, `experimental`, `unknown`, `requires-protocol-research`, or `unsupported`.
- If evidence is incomplete, preserve uncertainty. Do not upgrade a capability merely to complete a UI flow.

## 5. Destructive-command safety

The following opcodes are destructive and must never be issued by unattended automation:

- `0x01` — reset defaults
- `0x02` — clear pairing
- `0x03` — factory reset

Any future interactive path for destructive operations must require explicit user intent, explain the effect, and have dedicated tests.

Firmware OTA is out of scope until image format, integrity checks, interruption behavior, rollback/recovery, and hardware verification are proven. Agents must not add a speculative flashing path.

## 6. Architecture boundaries

Preserve the flow:

```text
UI -> state/orchestration -> device profile + protocol codec -> transport
```

- UI receives typed state and capability metadata.
- Protocol modules own byte-level encoding/decoding.
- Device profiles own model-specific capability decisions.
- Transport implementations own I/O and connection lifecycle.
- Host-only features must not be presented as device DSP/protocol capabilities.

Do not bypass these boundaries for a shortcut.

## 7. Change workflow

Before editing:

1. Read the relevant code and nearby tests.
2. Read the relevant document under `docs/` for protocol/device work.
3. State the invariant the change must preserve.
4. Prefer the smallest coherent change that closes the task.

During implementation:

- Keep unrelated refactors out of the same change.
- Avoid new dependencies unless the existing stack cannot solve the problem cleanly.
- Do not commit generated build output, caches, `node_modules`, or `native/target`.
- Do not modify `Cargo.lock` unless Rust dependency resolution actually changed.
- Keep the brand as **521C**; use `521c` for filesystem/config identifiers and `521cctl` for the CLI.

After implementation:

1. Run focused tests for the changed surface.
2. Run the full relevant validation gate.
3. Update docs/capability matrices when behavior or protocol knowledge changed.
4. Check `git diff` for accidental generated files or unrelated changes.
5. Report what was verified and what remains unverified.

## 8. Validation gates

Web / TypeScript:

```bash
npm test
npm run typecheck
npm run lint
npm run build
```

Rust:

```bash
cd native
cargo test --workspace
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
```

With `just`:

```bash
just check
```

Do not claim a gate passed unless it was actually run successfully. If the environment prevents a gate, state that explicitly in the PR/hand-off.

## 9. Pull-request expectations

A PR should contain:

- A concise problem statement and scope.
- The behavioral/architectural change.
- Evidence or protocol source when protocol semantics changed.
- Tests added/updated.
- Validation commands actually executed.
- Safety impact, especially for BLE writes.
- Remaining uncertainty or follow-up work.

Do not hide known failures behind broad wording such as “should work”.

## 10. Parallel agents

Parallel work is allowed only when file ownership and interfaces are clear. Split by non-overlapping surfaces (for example protocol fixtures vs documentation), establish shared types/contracts first, and integrate through one final validation pass. Two agents should not independently redesign the same protocol shape, device profile, state schema, or UI shell.

## 11. Definition of done

A task is done when the requested behavior is implemented, relevant tests pass, protocol/safety invariants remain true, docs are synchronized where necessary, and the repository contains no unrelated or generated noise.
