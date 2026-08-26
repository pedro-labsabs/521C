# Changelog

All notable changes to 521C are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versioning policy is
defined in `docs/GOVERNANCE.md` §4.

## [Unreleased]

### Added

- Auto-attach for already-connected earbuds: at startup (BlueZ mode) the app
  detects devices the host is already connected to (e.g. earbuds connected for
  audio before the app started) and attaches the first candidate
  automatically — no manual scan/connect needed. New transport method
  `connected_devices()` (default: none; BlueZ overrides) and new core command
  `AppCommand::AttachConnected`; model truth and the known-device attestation
  apply unchanged.
- Deterministic desktop close-lifecycle gate: `521c --mock --close-self-test`
  dispatches the same `WindowEvent::CloseRequested` a window manager sends and
  passes only when the event loop exits and the persisted config reloads
  valid; `scripts/test-desktop-close.sh` wraps it with a timeout. CI runs the
  gate on the dev binary and the packaged AppImage (issue #40).
- `docs/GOVERNANCE.md` §4 "Release gate enforcement": documents the Free-plan
  limitation (no paid runners / no branch protection for private repos), the
  enforced local-ladder workaround, the per-release audit record rule, and the
  owner action needed to restore the remote gate (issue #41).

### Changed

- System EQ now renders a complete, valid PipeWire filter-chain artifact (issue #13
  audit revalidation): a 10-band biquad graph (low shelf 31 Hz, peaking bands
  62 Hz–8 kHz, high shelf 16 kHz, Q = 1.0, gains ±12 dB) in the exact syntax of the
  target platform's filter-chain examples, exposed as the effect-sink pair
  `effect_input.521c_system_eq` / `effect_output.521c_system_eq`. The artifact moves
  to `~/.config/pipewire/filter-chain.conf.d/` (loaded by the dedicated filter-chain
  daemon on Ubuntu/Mint-family systems; fallback documented). Live-validated on
  PipeWire 1.0.5: enable → nodes join the main graph with correct ports → disable →
  clean removal. Routing through the EQ stays a documented, user-controlled step;
  521C never rewires the session automatically. Deterministic tests pin the rendered
  graph (band labels, frequencies, gains, link chain, effect-sink props).

### Fixed

- BlueZ connect no longer fails with `org.bluez.Error.Failed:
  br-connection-busy` when the earbuds are already connected at the host
  level (e.g. for audio): a redundant `Device1.Connect()` is skipped for
  devices already marked `Connected`, and busy/already-connected answers are
  treated as "link already up" and proceed to characteristic resolution
  (user-reported).
- Auto Game Mode tracks every active candidate as a set (issue #13 audit
  revalidation): with several concurrent MPRIS players, deactivating one no longer
  turns game mode off while another matching player is still active; non-matching
  players neither activate nor sustain it; deactivating an unknown candidate is a
  no-op. Multi-player interleaving and cooldown determinism are covered by tests.
- Transport sessions are now transactional (issue #39): a failed connect,
  failed service resolution, remote disconnect, or replacement connect
  invalidates the whole session in both the Web Bluetooth and BlueZ
  transports. Read/write/subscribe report a disconnected error until a full
  new connection succeeds; no I/O can reach a stale characteristic from a
  previous device/session. Remote disconnect also emits the disconnected
  state to `onState` consumers.
- Normal window close now terminates the desktop app (issue #40): the close
  handler persists config and ends the Slint event loop, so no invisible
  survivor process holds BlueZ/MPRIS workers after close.

### Verified (no code change needed)

- Issue #37 (capability vector EOF drift): not present in the committed tree —
  the audit's failing reproduction came from a reconstructed snapshot;
  byte-for-byte vector test and both language suites pass at HEAD.
- Issue #38 (missing AppImage script): the script was present in the committed
  tree at the audit base; verified end-to-end (build, metadata staging,
  explicit failure without appimagetool, launch + close gates on the
  artifact).

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
