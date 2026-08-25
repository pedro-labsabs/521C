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

## Command scheduling

Device writes pass through a per-connection command scheduler
(`src/lib/qcy/scheduler.ts`): writes are serialized, high-frequency latest-value controls
(EQ, balance, ANC level) are coalesced so a drag cannot enqueue an unbounded backlog, and an
optional bounded read-back reconciliation distinguishes "sent" from "confirmed". Queued work
is cancelled on disconnect. The scheduler is event-driven — no permanent polling loop.

## Persistence

Browser: `localStorage` key `521c-config-v1` (theme, profiles, notify, EQ).  
Native target: `~/.config/521c/` via XDG.

## Performance notes (native target)

Event-driven GATT notifications. RSSI smoothing on a 1.6s timer only while connected. Hidden window must not keep a full render loop — the tray view is a static chip.
