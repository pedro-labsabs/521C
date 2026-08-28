# Supported devices

**Role:** derived status matrix. Readiness below must stay derivable from the
capability truth in code; update it in the same PR as any capability change
(see `docs/README.md` §4).

Independent project. Not affiliated with QCY.

Every capability is described by four independent truths (see
`src/lib/qcy/device/capabilities.ts`): **hardware** (is it a feature of the model?),
**protocol** (is the behavior evidenced?), **implementation** (does this build do it?),
and **write** (is it writable / experimental / read-only / forbidden). The table below
reports the **app-level readiness** a user actually sees, derived from those truths — it
never conflates "the device/protocol can do this" with "this build implements it".

Legend:
**S** supported (implemented, writable) · **RO** read-only · **E** experimental (session
opt-in) · **M** mock only · **Host** implemented as a Linux host integration (never an
earbud write) · **P** protocol known, app pending · **R** needs protocol research ·
**U** unsupported · **F** forbidden · **?** unknown

| Capability | HT08 MeloBuds Pro | Notes |
| --- | --- | --- |
| Detect / connect | S | Name match; vendorId learned from adv |
| Battery L/R/case | RO | 0x2F / char 00000008 |
| Charging flags | RO | bit7 |
| Firmware read | RO | 0x30 |
| RSSI proximity | RO | Host BLE, not GPS |
| ANC off / on | S | 0x17 scenes (2,0,0) / (1,1,2) — live-validated; 0x0C falsified (#52) |
| Adaptive ANC | S | 0x17 (1,5,2), ACK (1,5,0) + voice prompt; 0x32 EnvAdaptation unvalidated on this model |
| Indoor / commute / noisy | S | 0x17 scenes (1,1,2) / (1,2,2) / (1,3,2) — live-validated |
| Wind reduction | S | 0x17 (1,4,2), ACK (1,4,0) — live-validated |
| Transparency | S | 0x17 (3,2,4), ACK (3,2,0) — live-validated; adjustable levels not validated (read-only) |
| Vocal enhance | R | Mentioned in reviews; no named opcode yet |
| Game mode | S | 0x09 |
| Auto game mode | Host | qcy-host: MPRIS player-presence signal + debounce + keyword allowlist, no polling; tracks all active candidates as a set (one player leaving never clears another). Wired by the desktop app (#8); no BLE traffic while idle |
| Device EQ | S | 0x22 |
| System EQ | Host | qcy-host manages one user-scoped PipeWire filter-chain artifact — a complete 10-band biquad graph exposed as an effect sink, live-validated on PipeWire 1.0.5 (`521cctl system-eq on/off/status`; routing documented, user-controlled). Never written to buds |
| Touch mapping | S | char 0000000D |
| Wear detection | S | 0x06 / 0x2C |
| Sleep mode | S | 0x10 |
| Spatial | E | 0x2D unverified on HT08 FW |
| Multipoint status | ? | Stack property |
| Multipoint control | R | No public command |
| Find chime | S | 0x05 / 0x3D; interactive preflight (#9) |
| Find GPS | U | Device has none |
| LDAC toggle | E | Linux codec is usually PipeWire |
| LDAC bitrate | U | Not exposed; never invented |
| Codec status | Host | Read passively from BlueZ MediaTransport1 (codec/sample rate/profile); unknown when unavailable — never invented |
| Firmware OTA | F | Not yet safely supported; no flash path sent |

Linux control transport (issue #50, resolved by #52): the HT08 control path
is BLE GATT. BlueZ resolves the vendor service `0000a001` on the earbuds'
**separate LE control identity** (paired alongside the BR/EDR audio
identity); scanning only the audio MAC is what originally hid it. Live HT08
validation: battery/firmware reads, the full 0x17 ANC scene table with
firmware ACKs, and a resident LE session coexisting with BR/EDR audio. An
active HFP/SCO (hands-free) session blocks LE connection initiation
host-side (`le-connection-abort-by-local`); the native transport preflights
it and the app holds the session with a resident supervisor (#54/#57/#58).
SPP/RFCOMM (`native/crates/qcy-transport/src/rfcomm.rs`, `521cctl --spp`)
remains as a generic backend for models whose evidence points there, but it
is **not** the HT08 path: HT08 SPP channel 1 ("COM5") only byte-ACKs frames
and executes nothing. The Web Bluetooth path uses the same GATT surface.

Future models plug in as additional `QcyDeviceProfile` entries. Do not scatter
`if model == "HT08"` through the UI. Host-side features (System EQ, Auto game mode, Codec status) are implemented in the
native host layer (`native/crates/qcy-host`, issue #13); they are never presented as QCY
protocol capabilities and never generate earbud writes.
