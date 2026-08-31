# 521C product specification

**Authority:** normative product intent. Technical protocol facts remain governed by protocol evidence and device documentation.

## 1. Product statement

521C is a **Linux-first local audio control and orchestration system**.

Its product domain is how audio enters, leaves, is routed, configured and processed on the local host, together with the supported capabilities of attached audio devices. It should provide a small, transparent native application for host-audio state, routing, processing configuration, device control and audio-specific automation without requiring cloud telemetry, root privileges, or replacement of the Linux Bluetooth/audio stack.

521C began as an independent control surface for QCY earbuds. QCY support remains a first-class implementation path and **QCY MeloBuds Pro / HT08 remains the first release-quality device profile**, but QCY is no longer the architectural definition of the product.

This specification describes accepted product direction. A capability described here is not considered implemented until code, tests and support evidence prove it.

521C must prefer an honest unavailable/unknown state over a control that merely looks complete.

## 2. Product boundary

521C owns the **local audio control domain**.

A useful ownership test is:

> If the question is how audio enters, leaves, is routed, transformed, configured, or which audio device the host is using, it probably belongs to 521C.

The architecture should keep three concerns distinct:

```text
521C
  |
  |-- Host audio
  |     PipeWire / WirePlumber
  |     input/output selection
  |     volume / mute
  |     application streams and routing
  |     host-side EQ / processing configuration
  |     audio-specific automation
  |
  |-- Devices
  |     Bluetooth / USB / other supported endpoints
  |     profiles / codecs / sample-rate state where exposed
  |     device capabilities and state
  |
  `-- Vendor adapters
        QCY / HT08 first
        future vendor-specific protocols only when justified and evidenced
```

Vendor-specific capabilities must not leak into the generic host-audio model as if every device supported them.

## 3. Explicit non-responsibilities

521C must not become a catch-all product for anything involving sound.

The following are outside its domain unless a future product decision explicitly changes the boundary:

- music-library ownership, playlists or streaming-service product behavior;
- general media-player ownership;
- DAW/audio editing and professional content production;
- general-purpose recording workflows;
- speech recognition, transcription or TTS as broad platform services;
- calls/communications as a product domain;
- arbitrary multimedia orchestration unrelated to audio infrastructure/control.

MPRIS may be observed or used when required for audio-specific automation, but MPRIS integration does not make 521C the owner of media playback.

## 4. Primary user

A Linux Mint-class desktop user who wants a lightweight local utility for reliable audio control without needing to understand PipeWire graphs, D-Bus objects, Bluetooth profiles, vendor protocol packets or codec internals.

Typical needs include:

- clear audio input/output and device state;
- convenient switching between supported endpoints;
- volume, mute and microphone control;
- application-specific routing where the host stack permits it;
- host-side sound processing such as reversible EQ;
- codec/profile/sample-rate visibility when exposed reliably;
- context-aware audio automation;
- access to proven device-specific capabilities such as battery, ANC, transparency, low-latency mode or device EQ through supported adapters;
- a lightweight native application that behaves like a normal desktop utility.

## 5. Release and platform direction

The baseline product target is a native Linux desktop application for Linux Mint-class systems using:

- Rust for the native runtime;
- Slint for the final desktop UI;
- PipeWire/WirePlumber-compatible APIs for host audio integration;
- BlueZ over D-Bus for Bluetooth device access;
- MPRIS over D-Bus only where media state/control is needed for an audio-domain behavior;
- AppImage as the first self-contained distribution artifact.

Normal application use must not require root.

The first mature release may remain intentionally narrower than the full audio-domain direction. Broadening the domain does not require implementing every host-audio capability before shipping a useful QCY/HT08-focused release.

## 6. Product principles

### Local first

No telemetry. No account. No hidden cloud requirement. No implicit third-party runtime fetches. Local configuration stays local unless the user explicitly exports it.

### Linux-native cooperation

521C cooperates with PipeWire, WirePlumber, BlueZ and other normal system services. It does not replace system daemons or create a competing audio/Bluetooth stack.

### Clear layer boundaries

Host-audio behavior, generic audio-device state and vendor-specific protocol features are separate concerns. One layer must not invent or imply capabilities belonging to another.

### Evidence before device capability

Hardware marketing, vendor applications and community claims may suggest device features but do not by themselves prove a safe Linux implementation. Protocol support and app implementation readiness are separate truths.

### Safe by construction

Unknown vendor devices are read-only where vendor-protocol access is involved. Writes are centrally authorized. Dangerous commands remain unreachable. Audible locator behavior requires deliberate human interaction.

### Reversible host control

Host-side actions should be scoped, observable and reversible. 521C should avoid globally rewriting desktop audio configuration when a narrower runtime/session-level action can satisfy the request.

### Lightweight

Prefer event-driven native code, bounded work queues and small dependencies. The final desktop app should not require an embedded browser runtime. Idle/disconnected CPU use should be negligible.

### Observable failure

Connection, permission, unsupported-feature, timeout, host-service and protocol errors should be visible and actionable. Do not silently convert failure into fake/default state.

## 7. Core user journeys

### Launch and host-audio discovery

1. User starts 521C as a normal desktop application.
2. App reports availability of relevant host audio services without requiring root.
3. App discovers available input/output endpoints and current defaults where the host stack exposes them.
4. Unsupported or inaccessible host capabilities remain explicit rather than being simulated.

### Route and control host audio

Target host-audio operations may include:

- selecting default or requested input/output endpoints;
- volume and mute control;
- microphone state/control;
- inspecting and, where justified, routing application streams;
- applying reversible host-side EQ or processing configuration;
- exposing codec/profile/sample-rate information when the stack provides reliable state.

Each operation must report partial or unavailable capability honestly when PipeWire/WirePlumber or permissions do not provide the requested control.

### Audio-device discovery

1. 521C discovers supported audio endpoints through normal host services such as BlueZ.
2. Generic host-visible device information remains separate from vendor-specific capabilities.
3. A vendor adapter is selected only when identification evidence is sufficient.
4. Unknown or unsupported vendor devices must not inherit capabilities from a superficially similar profile.

### QCY / HT08 device controls

Supported HT08 controls are exposed only when both protocol evidence and current application implementation permit them. Candidate surfaces include:

- left/right/case battery and charging state;
- ANC/transparency modes and proven scene/level controls;
- game/low-latency mode;
- device EQ and presets;
- touch mapping;
- wear detection state and proven configuration;
- sleep-related controls where proven;
- firmware version readout;
- Find Earbuds only behind the dedicated interactive safety preflight.

The actual enabled set is governed by the evidence ledger and support matrix, not by this list.

### Profiles and audio automation

A profile is a deliberate bundle of supported host and/or device actions. Applying one must report partial failure instead of pretending the entire profile succeeded.

Target examples include:

- switch output to a preferred headset while keeping another microphone active;
- apply a host EQ when a specific endpoint becomes active;
- enable an evidenced device low-latency mode and adjust host processing when a matching game starts;
- restore a preferred routing state when a device reconnects.

Automation must be event-oriented where practical, bounded, explicit about unavailable actions and subject to the same device safety policies as manual control.

### Settings, backup and persistence

Configuration is versioned and validated. Import is atomic: malformed data must not partially mutate state. Export should avoid private device identifiers/runtime-only fields unless explicitly part of the documented privacy contract.

## 8. UI behavior

The final UI should be compact and utility-oriented rather than dashboard-heavy.

Required qualities:

- clearly distinguish host-service unavailable, disconnected, connecting, connected, unsupported and error states;
- identify active input/output endpoints and relevant device/profile state;
- distinguish host controls from vendor-device controls;
- never show mock data as real host or hardware data;
- hide or disable controls according to typed capability/implementation state;
- preserve accessible labels and keyboard/focus behavior;
- warn before experimental device operations and require explicit session opt-in where policy requires it;
- keep destructive/unimplemented functionality out of normal controls;
- provide enough diagnostic detail to troubleshoot without exposing raw protocol or PipeWire complexity to ordinary users.

The existing React interface is a useful behavior/visual reference, not a requirement to reproduce every layout literally in Slint.

## 9. Scope boundaries

### In scope for the product direction

- Linux Mint-class desktop systems;
- host audio state and endpoint selection;
- input/output, volume, mute and microphone control where supported;
- PipeWire/WirePlumber routing and application-stream integration where safe and maintainable;
- reversible host-side EQ/processing configuration;
- audio-specific automation;
- generic supported audio-device discovery/state;
- vendor-specific adapters backed by explicit evidence and safety policy;
- HT08 as the first complete vendor/device profile;
- native BlueZ transport for supported Bluetooth-device operations;
- native desktop UI;
- validated local persistence;
- AppImage distribution;
- mock/fake boundaries for deterministic development and testing;
- documented diagnostics and troubleshooting.

### Explicitly out of scope unless a future decision changes policy

- Windows/macOS support in the baseline product;
- Android/iOS application replacement;
- user accounts/cloud sync;
- telemetry/analytics;
- replacing PipeWire, WirePlumber or BlueZ;
- requiring root for normal operation;
- arbitrary unknown-vendor writes;
- speculative vendor opcode probing;
- unattended reset/clear-pairing/factory-reset;
- firmware flashing/OTA without independently proven format, integrity, interruption and recovery behavior;
- becoming a music player, streaming client, DAW, general recorder, TTS/transcription platform or communications suite.

## 10. Reliability requirements

Host-audio operations:

- must reconcile requested state with observed host state where possible;
- must not create unbounded event/write loops;
- must handle endpoint disappearance and service restart predictably;
- should prefer targeted reversible changes over destructive global rewrites;
- must report partial application of multi-action profiles.

Vendor-device operations:

- outbound commands are serialized or otherwise ordered according to proven protocol semantics;
- rapid sliders/continuous controls cannot create unbounded BLE write backlogs;
- connection loss invalidates/cancels stale queued operations;
- transport-write success is distinct from confirmed device-state success;
- retries and timeouts are bounded;
- parsing treats device input as untrusted;
- every write path converges on central authorization.

General configuration:

- imported config is bounded and validated;
- fake/mock boundaries reproduce major failure states, not only happy paths.

## 11. Privacy/security requirements

- default runtime performs no implicit third-party requests;
- local dev server binds to loopback by default;
- logs avoid unnecessary stable Bluetooth identifiers/private captures;
- secrets never enter repository config or exported diagnostics;
- untrusted device payloads cannot bypass parser bounds;
- every vendor write path converges on central authorization;
- host-side actions remain scoped/reversible and do not globally rewrite the user's desktop configuration without explicit justification and user intent.

## 12. Performance expectations

The final native application should feel appropriate for an always-available desktop utility:

- negligible CPU usage while idle/disconnected;
- event-driven host/device updates rather than aggressive polling;
- bounded queues and timers;
- no embedded browser requirement;
- memory footprint measured and tracked on a representative Linux Mint environment;
- no permanent helper daemon unless later evidence shows one is necessary.

Performance optimizations must not remove safety checks, evidence validation or useful error reporting.

## 13. Device capability truth model

Vendor/device capabilities must continue to answer four different questions rather than collapsing them into one label:

1. Is the feature associated with the hardware/model?
2. Is the protocol behavior evidenced for this model/firmware?
3. Is the feature implemented in this build/runtime?
4. Is the operation currently safe/authorized to read or write?

Product UI should derive enabled/disabled/unknown states from that model rather than maintain a second set of assumptions.

Host-audio capabilities need an analogous distinction between desired product direction, backend availability, runtime support and current authorization/observed state; they must not be mislabeled as vendor-device protocol capabilities.

## 14. Anakyklos integration

521C remains independently usable. Katherine, Ouroboros or any other Anakyklos component must not be required for basic supported local audio operation.

When integrated, other modules should consume explicit 521C audio-domain capabilities rather than directly owning PipeWire, BlueZ or vendor-protocol details. For example, a request to change output and input devices should be expressed as an audio-domain intent/capability and resolved by 521C under its own policies.

Integration does not transfer 521C's safety, evidence or authorization responsibilities to Ouroboros or Katherine.

## 15. Product definition of done

A feature is shipped only when its behavior is implemented, its capability/evidence status is accurate, its safety policy is enforced, relevant automated tests pass, and user-visible docs/status match reality.

A feature that only mutates frontend state, only works in mock mode, relies on an unverified protocol assumption, or exists only as future direction must not be presented as fully supported.
