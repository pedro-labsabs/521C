# Changelog

All notable changes to 521C are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versioning policy is
defined in `docs/GOVERNANCE.md` §4.

## [Unreleased]

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

### Security

- Native transports now enforce read-only for unknown/generic models at
  connect time; previously the BlueZ transport kept the HT08 write policy
  regardless of the connected device.
