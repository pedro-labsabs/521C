# Changelog

All notable changes to 521C are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versioning policy is
defined in `docs/GOVERNANCE.md` §4.

## [Unreleased]

### Added

- Interactive model confirmation for renamed devices: when a connected device's
  name does not prove the model (e.g. earbuds renamed by their owner), the
  desktop UI offers an explicit "this is a MeloBuds Pro (HT08)" confirmation.
  The attestation lifts the read-only state for the connected device, is
  remembered for the session, and is persisted as the local-only config field
  `knownDevices` (never exported). Destructive opcodes stay forbidden either
  way. See `docs/SECURITY_MODEL.md`.
- Shared config-schema conformance vectors for the local-only `knownDevices`
  field (valid + invalid cases, consumed by both the TypeScript and Rust
  suites).

### Changed

- `Transport` gains `attest_model_known()` (session-scoped, connection-bound);
  `AppCore::start` now takes the persisted `knownDevices` list so previously
  confirmed devices start writable.
- BlueZ transport dual-mode handling: `scan` now watches discovery for a bounded
  window instead of taking a single instant snapshot, and `connect` falls back
  to the BLE identity of a dual-mode device (same advertised name, or the QCY
  vendor service in the device `UUIDs`) when the selected object has no usable
  GATT. Connection failures map to structured errors with actionable guidance
  (e.g. open the charging case / disconnect audio so the BLE identity wakes).

## [0.1.0-rc.1] - 2026-08-25

First desktop release candidate. Release notes, verification evidence and
known limitations (including the external CI-billing blocker and pending
real-HT08 hardware verification) are recorded on the GitHub release.

### Added

- Native desktop application (`521c`): Slint GUI over the `qcy-app` core with
  BlueZ transport by default, clearly labelled mock mode, XDG config
  persistence, interactive Find-Earbuds preflight, and Auto Game Mode wiring
  (issue #8).
- AppImage packaging (`scripts/package-appimage.sh`) with `.desktop` and
  AppStream metadata (issue #8).
- Behavioral tests for the application core safety contract (preflight
  refusal, unknown-model denial, opt-in forwarding).
- Label taxonomy, milestones and triage conventions (`docs/TRIAGE.md`,
  issue #16).
- Governance baseline: `docs/GOVERNANCE.md`, `CODEOWNERS`, `SECURITY.md`,
  this changelog (issue #14).
- Rust write-policy tests for multi-block frames and malformed frames,
  mirroring the existing TypeScript cases.

### Changed

- README repository map and web validation block synchronized with the
  shipped tree (all seven native crates, `conformance/`, `packaging/`,
  governance docs, `audit:network`/`docs:check` gates).
- `docs/SECURITY_MODEL.md` now documents the two fail-safe ways the native
  Rust policy is stricter than the browser policy (no `0xFE` read-back
  exception for read-only devices; no pure-disable exception for
  experimental opcodes).
- Removed the unused `set.music`/`set.volume`/`set.noiseValue`/`set.rename`
  builders for catalog-only (non-writable) opcodes; their byte layouts stay
  pinned by the conformance corpus.

### Fixed

- Config persistence is now atomic (temp file + rename): a crash mid-write
  can no longer truncate `~/.config/521c/config.json`.

### Security

- Native transports now enforce read-only for unknown/generic models at
  connect time; previously the BlueZ transport kept the HT08 write policy
  regardless of the connected device.
- Rust central write policy now decodes the entire frame and authorizes
  **every** command block, matching the documented contract and the
  TypeScript mirror. Previously only the first block was checked, so a
  multi-block frame could smuggle a destructive (`0x01`/`0x02`/`0x03`),
  catalog-only, or experimental block past the policy, and frames with a
  bogus declared length passed unvalidated. Found by the independent
  skeptical release review; no current GUI/CLI flow built such frames.
