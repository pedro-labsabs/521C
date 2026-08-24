# Security and safety model

521C talks to physical Bluetooth devices over a reverse-engineered vendor protocol. Safety therefore includes both conventional software security and protection against unsafe device commands.

## Trust boundaries

- BLE advertisements and notifications are untrusted input.
- Device model identification is evidence-based and may remain unknown.
- UI controls consume typed state/capabilities, not arbitrary GATT bytes.
- Host-side automation is less trusted than an explicit, interactive user action for destructive operations.

## Mandatory invariants

- Validate SOF, declared lengths, block bounds, enum/range values, and timeouts.
- Never fabricate protocol constants to make a feature appear complete.
- Never automatically send reset (`0x01`), clear-pairing (`0x02`), or factory-reset (`0x03`).
- Firmware OTA remains disabled until the full image format, integrity mechanism, interruption behavior, and recovery path are independently verified.
- Avoid root and privileged daemon replacement.
- Do not add telemetry as a hidden dependency or default behavior.

## Reporting

For a security-sensitive bug, open a minimal issue without publishing secrets, private device identifiers, or unnecessary personal data. If a future private reporting channel is added, this document should be updated before relying on it.
