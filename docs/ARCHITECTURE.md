# Architecture

This document describes the intended layer boundaries for 521C.

```
ui (React shell in this preview / Slint on a Mint host)
  └─ hub store  (commands, profiles, persistence)
       ├─ qcy-protocol   framing · advertisement · codecs
       ├─ qcy-device     HT08 profile · capability matrix
       └─ transport      mock | Web Bluetooth | (BlueZ on native)
```

Layers do not leak raw GATT bytes into widgets. The UI renders state derived from the
four-dimensional capability truth (hardware / protocol / implementation / write) in
`src/lib/qcy/device/capabilities.ts`, so it never conflates "the device or protocol can do
this" with "this build implements it".

## Transports

| Transport | When |
| --- | --- |
| Mock HT08 | Default. Full battery/ANC/EQ/events without hardware |
| Web Bluetooth | Chromium + user gesture, real buds |
| BlueZ/zbus | Native Linux (`native/crates/qcy-transport`), system D-Bus GATT |

## Native transport (`native/crates/qcy-transport`)

The native transport sits above `qcy-protocol` (framing/codecs) and below the CLI/GUI.
A single `Transport` trait has two backends:

- `mock::MockTransport` — deterministic, hardware-free; the default for tests and dev.
- `bluez::BlueZTransport` — talks to the system BlueZ stack over the D-Bus GATT API
  (`org.bluez` / `GattCharacteristic1`). Event-driven, no root, no daemon reconfiguration.

All D-Bus access is isolated behind the `bluez::BlueZBus` trait, so object-path mapping,
discovery filtering, characteristic resolution and policy enforcement are unit-tested
against a fake bus when no Bluetooth daemon is present. Outbound writes pass through the
central `policy::WritePolicy` (the Rust mirror of issue #1) before reaching the wire:
destructive opcodes (`0x01`/`0x02`/`0x03`) are never sent, unknown models stay read-only,
and experimental opcodes need a session opt-in. The live D-Bus boundary is compiled behind
the `bluez` Cargo feature (default on); the trait, mapping, policy and mock always build.

## Host services (`native/crates/qcy-host`)

The host-services layer is the host-side counterpart to the transport. It owns
functionality that lives on the Linux machine, NOT in the earbuds, and is a separate
interface from `Transport` and the protocol codecs:

| Service | What it does | Deliberately does NOT do |
| --- | --- | --- |
| MPRIS | media discovery/state/control over `org.mpris.MediaPlayer2` (session bus, zbus) | fabricate metadata; touch the buds |
| Codec | reads codec/sample-rate/profile passively from BlueZ `MediaTransport1` (system bus); unknown when unavailable | invent a value; acquire/modify any transport |
| Auto Game Mode | MPRIS player-presence signal (`NameOwnerChanged`) + debounce + keyword allowlist | busy-poll; write while idle; write outside the central policy |
| System EQ | one user-scoped PipeWire config artifact, create/remove lifecycle, disk-backed status | edit system-wide PipeWire config |

Host-only state is never written to the device and is never presented as earbud
DSP/protocol support; in the capability/truth model these stay `hardware: unknown`,
`protocol: unknown`, `write: read-only`. Every external boundary (D-Bus, filesystem,
audio graph) is isolated behind a trait and unit-tested against fakes. Missing services
(no session bus, no MPRIS player, no PipeWire) are handled gracefully as a normal state.
The live D-Bus integration is behind the `dbus` Cargo feature (default on); the traits,
rule engine, debouncer and lifecycle logic always build and test.

## Command scheduling

Device writes pass through a per-connection command scheduler
(`src/lib/qcy/scheduler.ts`): writes are serialized, high-frequency latest-value controls
(EQ, balance, ANC level) are coalesced so a drag cannot enqueue an unbounded backlog, and an
optional bounded read-back reconciliation distinguishes "sent" from "confirmed". Queued work
is cancelled on disconnect. The scheduler is event-driven — no permanent polling loop.

## Persistence & configuration schema

Configuration is governed by a versioned, validated schema
(`src/lib/qcy/config-schema.ts`, issue #11). External data — localStorage payloads and
import files — is never trusted: it is validated field-by-field (shapes, enums, numeric
bounds, array lengths, string sizes, profile/EQ identifiers) before it can touch
application state, and invalid input is rejected atomically with structured errors.

Every field belongs to exactly one class:

| Class | Fields | Persisted | Exported |
| --- | --- | --- | --- |
| Portable | theme, notify, customEq, customProfiles, activeProfileId, autoGame, autoGameKeyword | yes | yes |
| Local-only | hideMac, sleepTimerMin, lastSeen | yes | **no** |
| Runtime-only | connection/telemetry, log, toasts, experimental opt-in, pending chime | no | no |

The schema version travels inside the payload (`schema` field), not the storage key, so
future versions migrate instead of relying on key suffixes. The current browser key is
`521c-config`; the legacy `521c-config-v1` payload is migrated on load and left in place
as a fallback. Corrupt or newer-than-supported payloads fall back to defaults without
destroying stored data. Imported profiles never carry `builtin` trust.

Browser: `localStorage` via the schema layer.  
Native target: `~/.config/521c/` via XDG (issue #8), reusing this same JSON contract.

## Performance notes (native target)

Event-driven GATT notifications. RSSI smoothing on a 1.6s timer only while connected. Hidden window must not keep a full render loop — the tray view is a static chip.
