# Contributing

Read `AGENTS.md` before changing this repository. For long-running autonomous work, also read `docs/PRODUCT_SPEC.md`, `docs/AUTONOMOUS_EXECUTION.md` and `docs/HOST_SAFETY.md`.

## Core rules

1. Do not invent UUIDs, opcodes, vendor IDs, payload formats or capability mappings.
2. Validate every BLE frame (SOF, declared length, bounds, ranges and malformed input).
3. Keep hardware evidence, protocol evidence, implementation readiness and write authorization honest and separate.
4. Unknown/generic QCY devices are read-only by default.
5. Never send `0x01` / `0x02` / `0x03` from automations.
6. Never flash firmware unless format, integrity checks, interruption behavior and recovery are independently proven.
7. Add/update a parser/codec fixture before enabling a new write path.
8. Keep HT08-specific behavior inside the device/profile layer, not scattered model checks in UI code.
9. Route all outbound device writes through the central authorization policy once implemented; no raw-path bypasses.
10. Keep host-only MPRIS/PipeWire behavior separate from QCY protocol capability claims.
11. Normal runtime must not require root, replace BlueZ/PipeWire, add telemetry, or make hidden third-party requests.
12. Do not commit secrets, private captures, build caches or generated noise.

## Change workflow

- Start from an issue or a clearly bounded defect/feature contract.
- Inspect the current implementation before trusting stale issue prose.
- Keep a change coherent and avoid unrelated refactors.
- Add deterministic tests for behavior and safety boundaries.
- Document protocol evidence/provenance when semantics change.
- Run focused checks while iterating and the full relevant validation gate before merge.
- Distinguish behavior verified on real HT08 hardware from behavior validated only with mocks/fakes.

## Validation

```bash
npm test
npm run typecheck
npm run lint
npm run build
cd native
cargo test --workspace
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
```

After reproducible dependency locking lands, use `npm ci` for clean validation environments.

## Autonomous agents

Agents may make ordinary engineering decisions without user hand-holding when the repository contracts provide enough direction. They must still respect `docs/HOST_SAFETY.md`, use evidence rather than guesses for proprietary protocol work, and close issues only when acceptance criteria are actually proven.

See `PRIME_AGENT_START.md` for the canonical autonomous delivery bootstrap.
