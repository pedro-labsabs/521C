# Architecture

This document describes the intended layer boundaries for 521C.

```
ui (React shell in this preview / Slint on a Mint host)
  └─ hub store  (commands, profiles, persistence)
       ├─ qcy-protocol   framing · advertisement · codecs
       ├─ qcy-device     HT08 profile · capability matrix
       └─ transport      mock | Web Bluetooth | (BlueZ on native)
```

Layers do not leak raw GATT bytes into widgets. The UI renders only controls whose capability state is `supported` or `experimental`.

## Transports

| Transport | When |
| --- | --- |
| Mock HT08 | Default. Full battery/ANC/EQ/events without hardware |
| Web Bluetooth | Chromium + user gesture, real buds |
| BlueZ/zbus | Native Linux crate (not runnable in this preview) |

## Persistence

Browser: `localStorage` key `521c-config-v1` (theme, profiles, notify, EQ).  
Native target: `~/.config/521c/` via XDG.

## Performance notes (native target)

Event-driven GATT notifications. RSSI smoothing on a 1.6s timer only while connected. Hidden window must not keep a full render loop — the tray view is a static chip.
