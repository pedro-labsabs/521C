# Desktop architecture (issue #8)

**Authority:** normative for the native desktop application. Protocol facts remain
governed by `docs/PROTOCOL.md`; safety by `docs/SECURITY_MODEL.md` and
`docs/HOST_SAFETY.md`.

## 1. Decision: single native process, no IPC in v1

The desktop shell is **one Rust process**: a Slint UI thread plus the `qcy-app`
core worker thread. There is no separate backend daemon and no IPC serialization
boundary in v1.

Evaluated options:

| Option | RAM overhead | Mint support | Maintainability | Verdict |
| --- | --- | --- | --- | --- |
| Single process, Slint UI over in-process core | lowest (one runtime, no browser) | first-class (X11/Wayland via winit; software renderer fallback) | one language, one test ladder | **chosen** |
| Native backend + browser UI (Tauri/webview) | higher (webview runtime) | depends on system webview quality | two runtimes, JS/Rust split | rejected: violates the "no embedded browser requirement" product principle |
| Native backend + separate GUI process over D-Bus/Unix IPC | higher (two processes) | good | extra serialization boundary and lifecycle management for no v1 benefit | deferred: the typed API below is shaped so this can be added without changing the UI contract |

The existing React/TanStack surface stays as the development/reference/mock
surface per the fixed product decisions; it is not the desktop shell.

## 2. Layering

```text
Slint UI (native/crates/521c-desktop)
  │  typed AppCommand ↓ / AppEvent ↑  (std::sync::mpsc channels)
qcy-app core worker (native/crates/qcy-app)
  ├─ qcy-protocol   framing · codecs
  ├─ qcy-device     HT08 profile · capability truth
  ├─ qcy-transport  mock | BlueZ — central WritePolicy enforced inside write()
  └─ qcy-host       MPRIS · codec · system EQ (host services, never device writes)
```

Boundary rules:

- The UI only sees typed state (`DeviceSnapshot`, `MediaStatus`, `CodecInfo`,
  `SystemEqStatus`) and user-readable strings. Raw GATT bytes, opcodes and
  policy decisions never reach the Slint layer; policy denials arrive as
  `AppEvent::Denied(message)`.
- Write authorization is **not** reimplemented in the UI or the core. Every
  device write is encoded by `qcy-protocol` and converges on the transport's
  central `WritePolicy` (the issue #1 layer). Destructive opcodes are not
  expressible as `AppCommand` at all.
- The Find Earbuds chime requires the interactive preflight (issue #9 mirror):
  `AppCommand::FindChime { confirmed_not_worn: true, .. }` is the only shape the
  UI can produce after the confirmation dialog, and the core still refuses the
  command if the flag is false.
- Unknown/generic devices stay read-only: the core reports `model_known` from
  discovery evidence and the transport policy denies their writes.
- Renamed devices can be unlocked interactively: when the advertised name does
  not prove the model, the UI offers an explicit confirmation ("this is a
  MeloBuds Pro (HT08)"). `AppCommand::ConfirmModel { address }` attests the
  connected device only; the core emits `ModelConfirmed { address }` and the
  desktop persists the address in the local-only config field `knownDevices`
  (never exported). See `docs/SECURITY_MODEL.md`.
- Host-only features (MPRIS, codec status, system EQ) are presented as Linux
  host behavior, never as earbud DSP/protocol capabilities (issue #13 contract).

## 3. Typed frontend/backend API

Defined in `native/crates/qcy-app/src/core.rs`:

- **Commands (UI → core):** `Scan`, `Connect(address)`,
  `ConfirmModel { address }`, `Disconnect`,
  `RefreshStatus`, `SetNoise`, `SetAncScene`, `SetGameMode`, `SetSleepMode`,
  `SetInEarDetection`, `FindChime { .. }`, `SetExperimentalOptIn`,
  `MediaStatus`, `MediaControl`, `CodecStatus`, `SystemEqOn/Off/Status`,
  `Shutdown`.
- **Events (core → UI):** `Discovered(list)`, `StateChanged(snapshot)`,
  `HostMedia`, `HostCodec`, `HostSystemEq`, `Error(message)`,
  `Denied(message)`, `ModelConfirmed { address }`, `Info(message)`.

The command enum deliberately cannot express destructive opcodes (`0x01`/`0x02`/
`0x03`), arbitrary opcode writes, or un-preflighted chimes. If an IPC boundary
is ever added, these same types become the IPC schema unchanged.

## 4. Transports and modes

- Default: **BlueZ** over the system D-Bus GATT API (issue #7 transport),
  no root, no daemon reconfiguration.
- Dual-mode earbuds: the buds usually pair as a BR/EDR audio device while the
  QCY vendor protocol lives on a separate BLE/GATT identity. `scan` watches
  discovery for a bounded window, and `connect` falls back to the BLE identity
  (same advertised name, or the vendor main service in the device `UUIDs`)
  when the selected object resolves no usable GATT. If nothing resolves, the
  error tells the user to wake the BLE side (open the charging case or
  disconnect audio) and retry.
- Already-connected attach: at startup (BlueZ mode) the app lists devices the
  host is already connected to (`Transport::connected_devices`) and attaches
  the first candidate automatically — no manual scan/connect needed when the
  earbuds were connected for audio before the app started. A redundant
  `Device1.Connect()` is never issued for a device already marked
  `Connected`, and BlueZ `br-connection-busy` / `AlreadyConnected` answers are
  treated as "link already up" and proceed to characteristic resolution.
- Live-validated reachability model (HT08, #52, 2026-08-27): the LE control
  identity keeps advertising connectable `ADV_IND` during normal use, and LE
  control coexists with BR/EDR audio once the session is up. Intermittent
  connection failures were host-side: an active HFP/SCO (hands-free) session
  makes this controller abort LE connection initiation before any HCI
  command (`le-connection-abort-by-local`). The transport preflights
  `MediaTransport1` for active HFP/HSP UUIDs and fails with an actionable
  diagnostic (release the microphone / switch the card to A2DP); a resident
  session supervisor holds the LE link and re-bootstraps on link loss
  (#54/#57). Fully idle LE links are dropped by the earbuds firmware, so the
  supervisor keeps the session busy (settings-notify subscription plus
  periodic battery/firmware reads, #58).
- `--mock`: deterministic mock transport, visibly labelled in the UI
  ("MOCK transport (development)" badge). Mock mode never pretends to be
  hardware.
- If the BlueZ system bus is unavailable at startup, the app falls back to the
  mock transport and says so in the status line — it never fails silently and
  never presents mock data as real.
- The packaged app does not use or require Web Bluetooth; the primary hardware
  path is native BlueZ.

## 5. Configuration and persistence

- Storage: `~/.config/521c/config.json` (XDG Base Directory compliant;
  `$XDG_CONFIG_HOME` honored).
- Schema: the exact JSON contract of the browser config schema (issue #11),
  mirrored in `native/crates/qcy-app/src/config.rs` and pinned by the shared
  corpus `conformance/config_vectors.json`. The schema version travels inside
  the payload, so browser and desktop persistence cannot drift.
- Field classes are preserved: portable + local-only fields persist; runtime
  state (experimental opt-in, logs, pending chime) never persists.
- Migration from browser-only state: browser state lives in `localStorage`,
  which a native process cannot read. The supported migration path is the
  documented export/import file flow — export from the web surface, import the
  same validated JSON into the desktop app. Import is atomic and validated;
  malformed files cannot partially mutate state.

## 6. Window/tray lifecycle — deliberate alternative

Slint 1.x has no first-class system-tray / StatusNotifier API. Rather than add
an unmaintained tray dependency, v1 ships a deliberate alternative:

- 521C is a normal windowed desktop application with a small utility window;
- configuration is persisted on clean window close, and the close handler then
  ends the Slint event loop (`slint::quit_event_loop`), so closing the only
  window exits the process — no invisible survivor process, no orphaned
  BlueZ/MPRIS workers (issue #40);
- there is **no background daemon** (consistent with the lightweight/no-daemon
  product principles); device state is re-read on launch and on demand.

If a tray becomes a proven product need later, it should be added as a bounded
follow-up (e.g. a StatusNotifier D-Bus integration) without changing the core
API.

## 7. Auto Game Mode wiring (issue #13 → #8)

The desktop app owns the device-write side of Auto Game Mode:

- Source: MPRIS player presence on the session bus (`NameOwnerChanged`,
  event-driven; no polling). Absent session bus → feature logs unavailable and
  stays off.
- Rule: the config's `autoGameKeyword` allowlist (case-insensitive substring on
  the MPRIS bus-name suffix, e.g. `steam`, `vlc`).
- Controller: `qcy-host` `GameModeController` with a 30s transition cooldown
  (player presence changes are infrequent; the cooldown suppresses churn when
  players restart or hand over).
- Writes: only `AppCommand::SetGameMode` through the core → central
  `WritePolicy`. The host layer itself never writes to the device.
- Gating: transitions are only sent while connected. On (re)connect the app
  reconciles: if a matching player is present it turns game mode on; a
  desired-off state never forces the device, so manual settings survive
  reconnects.
- Off by default (`autoGame: false` in config). With the feature off or no
  matching player, there is zero BLE traffic from this subsystem.

## 8. Packaging

- Baseline artifact: **AppImage** (self-contained, suitable for Linux Mint),
  produced by `scripts/package-appimage.sh`.
- Desktop metadata: `packaging/linux/521c.desktop` plus the `521c` icon
  (SVG source in `native/crates/521c-desktop/assets/`, PNG renders committed
  for AppImage/desktop use).
- Install/uninstall: documented in `docs/DEVELOPMENT.md` → "Desktop app".

## 9. Runtime dependencies (Linux Mint-class systems)

| Dependency | Why | Required |
| --- | --- | --- |
| BlueZ + system D-Bus | Bluetooth device access (GATT API) | for real hardware; mock mode runs without it |
| D-Bus session bus | MPRIS media integration | optional; degrades gracefully |
| PipeWire user config dir | System EQ artifact | optional; degrades gracefully |
| fontconfig, libxkbcommon, OpenGL-capable display server | Slint window rendering | yes for the GUI (software renderer fallback via `SLINT_BACKEND`) |
| X11 or Wayland session | window display | yes for interactive use |

Normal use never requires root.

## 10. Launch verification

`521c --self-test` creates the window, starts the event pump, processes events
briefly and exits 0. The packaging gate and CI use it to verify launch without
interactive display time. Interactive behavior still requires a display.

`521c --mock --close-self-test` additionally verifies the close lifecycle
(issue #40): after the window opens, the app dispatches the same
`WindowEvent::CloseRequested` a window manager's close button produces, and
the run only passes when the close handler persists the configuration, ends
the event loop, and the process exits 0 with the persisted config still
valid. `scripts/test-desktop-close.sh` wraps this with a timeout so a hidden
survivor process fails the gate; CI runs it against both the dev binary and
the packaged AppImage under a virtual display.
