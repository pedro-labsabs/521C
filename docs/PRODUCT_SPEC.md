# 521C product specification

**Authority:** normative product intent. Technical protocol facts remain governed by protocol evidence and device documentation.

## 1. Product statement

521C is an independent, unofficial, Linux-first desktop control surface for QCY earbuds. It exists to provide a small, transparent native Linux application for device status and proven controls without requiring the official mobile application, cloud telemetry, root privileges, or replacement of the Linux Bluetooth/audio stack.

The first release-quality device profile is **QCY MeloBuds Pro / HT08**.

521C must prefer an honest unavailable/unknown state over a control that merely looks complete.

## 2. Primary user

A Linux Mint user who already pairs and uses QCY earbuds through the normal desktop Bluetooth stack and wants:

- clear battery/device state;
- convenient access to proven device features;
- reliable low-latency/game-mode and sound controls where protocol evidence exists;
- host-side media/audio conveniences integrated with Linux services;
- a lightweight native application that behaves like a normal desktop utility.

The user should not need to understand GATT UUIDs, packet bytes, BlueZ object paths or protocol confidence levels to use normal supported features.

## 3. Release target

The baseline release target is a native Linux desktop application for Linux Mint-class systems using:

- Rust for the native runtime;
- Slint for the final desktop UI;
- BlueZ over D-Bus for Bluetooth device access;
- PipeWire/WirePlumber-compatible host audio integration when host audio manipulation is required;
- MPRIS over D-Bus for media-player integration;
- AppImage as the first self-contained distribution artifact.

Normal application use must not require root.

## 4. Product principles

### Local first

No telemetry. No account. No hidden cloud requirement. No implicit third-party runtime fetches. Local configuration stays local unless the user explicitly exports it.

### Evidence before capability

Hardware marketing, the official QCY app and community claims may suggest features but do not by themselves prove a safe Linux implementation. Protocol support and app implementation readiness are separate truths.

### Safe by construction

Unknown devices are read-only. Writes are centrally authorized. Dangerous commands remain unreachable. Audible locator behavior requires deliberate human interaction.

### Linux-native behavior

521C cooperates with BlueZ, PipeWire/WirePlumber and MPRIS. It does not replace system daemons or invent its own competing audio/Bluetooth stack.

### Lightweight

Prefer event-driven native code, bounded work queues and small dependencies. The final desktop app should not require an embedded browser runtime.

### Observable failure

Connection, permission, unsupported-feature, timeout and protocol errors should be visible and actionable. Do not silently convert failure into fake/default state.

## 5. Core user journeys

### Launch and device discovery

1. User starts 521C as a normal desktop application.
2. App reports Bluetooth availability/state without requiring root.
3. App discovers relevant QCY devices exposed by BlueZ.
4. Known HT08 devices receive the HT08 profile only when identification evidence is sufficient.
5. Unknown QCY devices remain visibly generic/read-only.

### Connect and status

1. User explicitly selects/connects a device or resumes a previously known device safely.
2. App resolves required services/characteristics and subscribes to proven notifications.
3. UI shows only observed/proven state; unavailable fields remain unknown rather than receiving mock values.
4. Disconnect, adapter-off, range loss and permission failures recover predictably.

### Device controls

Supported HT08 controls are exposed only when both the protocol evidence and current application implementation permit them. Candidate target surfaces include:

- ANC/transparency modes and proven scene/level controls;
- game/low-latency mode;
- device EQ and presets;
- touch mapping;
- wear detection state and proven configuration;
- sleep-related controls where proven;
- firmware version readout;
- Find Earbuds only behind the dedicated interactive safety preflight.

The actual enabled set is governed by the evidence ledger and support matrix, not by this wish list.

### Profiles

A profile is a deliberate bundle of supported device/host actions. Applying one must report partial failure instead of pretending the whole profile succeeded. High-frequency values should be coalesced and final observed state reconciled where the protocol permits it.

### Host services

Host-only features are presented as Linux host behavior, not QCY firmware features. They may include:

- media state/control through MPRIS;
- codec/profile/sample-rate information where the Linux stack exposes it reliably;
- host-side/system EQ through a reversible PipeWire-compatible path;
- Auto Game Mode driven by an event-oriented host signal and explicit matching rules.

Absence of the relevant host service must degrade gracefully.

### Settings, backup and persistence

Configuration is versioned and validated. Import is atomic: malformed data must not partially mutate state. Export should avoid private device identifiers/runtime-only fields unless explicitly part of the documented privacy contract.

## 6. UI behavior

The final UI should be compact and utility-oriented rather than dashboard-heavy.

Required qualities:

- clearly distinguish disconnected, connecting, connected, unsupported and error states;
- clearly identify active device and profile;
- never show mock data as real hardware data;
- hide or disable controls according to typed capability/implementation state;
- preserve accessible labels and keyboard/focus behavior;
- warn before experimental operations and require explicit session opt-in where policy requires it;
- keep destructive/unimplemented functionality out of normal controls;
- provide enough diagnostic detail to troubleshoot without exposing raw packet complexity to ordinary users.

The existing React interface is a useful behavior/visual reference, not a requirement to reproduce every layout literally in Slint.

## 7. Scope boundaries

### In scope for the first mature release

- Linux Mint-class desktop systems;
- HT08 as the first complete profile;
- safe read/write device control backed by evidence;
- native BlueZ transport;
- native desktop UI;
- validated local persistence;
- AppImage distribution;
- mock/fake transport for deterministic development and testing;
- documented diagnostics and troubleshooting;
- Linux host integrations that can be implemented safely and reversibly.

### Explicitly out of scope unless a future issue changes policy

- Windows/macOS support;
- Android/iOS application replacement;
- user accounts/cloud sync;
- telemetry/analytics;
- arbitrary unknown-QCY writes;
- speculative opcode probing;
- unattended reset/clear-pairing/factory-reset;
- firmware flashing/OTA without independently proven format, integrity, interruption and recovery behavior;
- replacing BlueZ, PipeWire or WirePlumber;
- requiring root for normal operation.

## 8. Reliability requirements

- outbound device commands are serialized or otherwise ordered according to proven protocol semantics;
- rapid sliders/continuous controls cannot create unbounded BLE write backlogs;
- connection loss invalidates/cancels stale queued operations;
- transport-write success is distinct from confirmed device-state success;
- retries and timeouts are bounded;
- parsing treats device input as untrusted;
- imported config is bounded and validated;
- fake/mock boundaries reproduce major failure states, not only happy paths.

## 9. Privacy/security requirements

- default runtime performs no implicit third-party requests;
- local dev server binds to loopback by default;
- logs avoid unnecessary stable Bluetooth identifiers/private captures;
- secrets never enter repository config or exported diagnostics;
- untrusted BLE payloads cannot bypass parser bounds;
- every write path converges on central authorization;
- host-side actions remain scoped/reversible and do not globally rewrite the user's desktop configuration.

## 10. Performance expectations

The final native application should feel appropriate for an always-available desktop utility:

- negligible CPU usage while idle/disconnected;
- event-driven device updates rather than aggressive polling;
- bounded queues and timers;
- no embedded browser requirement;
- memory footprint measured and tracked on a representative Linux Mint environment;
- no permanent helper daemon unless later evidence shows one is necessary.

Performance optimizations must not remove safety checks, evidence validation or useful error reporting.

## 11. Truth model

Every user-visible capability should ultimately answer four different questions rather than collapsing them into one label:

1. Is the feature associated with the hardware/model?
2. Is the protocol behavior evidenced for this model/firmware?
3. Is the feature implemented in this build/runtime?
4. Is the operation currently safe/authorized to read or write?

Issue #3 owns the concrete data model. Product UI should derive enabled/disabled/unknown states from that model rather than maintain a second set of assumptions.

## 12. Product definition of done

A feature is shipped only when its behavior is implemented, its truth/evidence status is accurate, its safety policy is enforced, relevant automated tests pass, and user-visible docs/status match reality.

A feature that only mutates frontend state, only works in mock mode, or relies on an unverified protocol assumption must not be presented as fully supported.
