# QCY BLE protocol (independent notes)

**Role:** normative evidence notes for the independent reverse-engineered
protocol. Opcodes/UUIDs not recorded here (or in the evidence ledger) are not
invented.

Source: public reverse-engineering of QCY earphone GATT traffic, cross-checked against community documentation. **Not an official spec.** Opcodes that are not in this file are not invented.

Byte-level framing, advertisement, battery and firmware behavior are pinned by the shared conformance corpus at `conformance/protocol_vectors.json` (consumed by both the TypeScript and Rust test suites). Add a vector there before changing a codec or enabling a write path; see `conformance/README.md`.

## Evidence and trust levels

Every opcode and GATT UUID that remains in this repository is recorded in the
canonical evidence ledger at `src/lib/qcy/protocol/evidence.ts`. An opcode being
present in the `Cmd` catalog is **not** by itself enough to make it writable.

Each entry records a provenance **evidence class** and a 521C **trust level**:

| Evidence class | Meaning |
| --- | --- |
| `protocol-doc` | Documented in this file from public reverse-engineering. |
| `hardware-capture` | Observed on real hardware (strongest). |
| `community-catalog` | Community opcode list — a research lead, not proof. |
| `official-app` | Feature exists in the official app — not proof of a Linux command. |

| Trust level | Meaning |
| --- | --- |
| `write-supported` | Safe write enabled for the HT08 profile. |
| `write-experimental` | Write enabled only behind a session opt-in. |
| `read` | Read-back / status only. |
| `catalog-only` | Known entry, not writable (insufficient evidence). |
| `destructive` | Forbidden; never written by unattended automation. |

The central write policy (`src/lib/qcy/policy.ts`) derives the HT08 writable set
directly from this ledger, and `evidence.test.ts` fails if a trusted write lacks a
sufficient evidence entry or if a community/official-app opcode is marked writable.
To promote a command, add real evidence and raise its trust level in the ledger —
do not edit the write policy allowlist by hand.

## Discovery

- Manufacturer company ID: `0x521c`
- Vendor ID: bytes 0–1 of manufacturer data, big-endian
- Battery L/R/case: bytes 5/6/7, bit7 charging, bits0–6 level
- Control MAC scrambled: display `[12]:[11]:[13]:[16]:[15]:[14]`
- HT08 is identified by advertised name (`QCY MeloBuds Pro` / `HT08`). Confirmed vendor IDs are recorded only after a real advertisement is captured — none are fabricated here.

## GATT

| UUID | Role |
| --- | --- |
| `0000a001-0000-1000-8000-00805f9b34fb` | Main service |
| `00001001-…` | Command write (`0xFF` framed) |
| `00001002-…` | Notify / settings |
| `00000007-…` | Version read |
| `00000008-…` | Battery read |
| `0000000B-…` | EQ direct (no frame) |
| `0000000D-…` | Key function V2 (no frame) |

## Transport paths

The `0xFF` framing below is transport-agnostic. QCY dual-mode earbuds can
expose two identities, and the control path is model-specific:

| Path | Where it works | Status |
| --- | --- | --- |
| BLE GATT (`0000a001`) on the **BLE control identity** | Android (official app), Linux/BlueZ (HT08-confirmed), Web Bluetooth | HT08-confirmed on Linux (issue #50): reads, ANC writes with ACK, coexistence with BR/EDR audio |
| SPP/RFCOMM (`00001101`, same framing over a byte stream) | model-dependent | generic backend implemented behind the same `Transport` contract (`rfcomm.rs`, PR #51); **not** the HT08 control path |

Live HT08 findings (issues #50/#52, 2026-08-27):

- The earbuds expose a BR/EDR audio identity (`84:AC:60:62:69:DA`) and a
  separate BLE control identity (`C4:AC:60:62:69:DB`, advertisement
  manufacturer company ID `0x521C`, random address type). BlueZ resolves the
  vendor GATT service `0000a001` on the LE identity; scanning only the audio
  MAC is what hid it.
- Confirmed over BLE GATT on Linux (no root): battery/version reads, and ANC
  writes to char `00001001` (write-without-response) with notify ACKs on
  `00001002`. LE control and BR/EDR audio hold simultaneously.
- The control identity advertises intermittently; after bonding, the LE link
  must be kept resident (reconnect on demand fails outside advertisement
  windows). See `docs/devices/HT08.md` and issue #52 for the bootstrap model.
- HT08 SPP channel 1 ("COM5") only byte-ACKs frames and executes nothing;
  channels 4/5 are silent. SPP remains a valid generic path only for models
  whose evidence points there.
- Reads over SPP (where SPP applies) are `RequestData(0xFE)` exchanges:
  battery `FF 03 FE 01 2F`, version `FF 03 FE 01 30`. `0xFE` is a read-back
  request, not a state mutation, so the write policy authorizes it even for
  read-only devices. On HT08 over GATT, direct characteristic reads are the
  observed read path.
- SPP has no GATT characteristics: unframed direct writes (`0000000B`,
  `0000000D`) do not exist on that path; EQ and key-function travel as framed
  opcodes.
- `scripts/sdp-rfcomm-channel.py` is the read-only Stage-1 SDP query tool for
  the SPP path.

## Frame

```
[0xFF] [body_len] [cmd] [param_len] [params…] …
```

`body_len` = total length − 2. Multiple command blocks may share a packet. There is no extra checksum; length and bounds are the integrity checks.

## Commands used by 521C

| Cmd | Name | Notes |
| --- | --- | --- |
| 0x05 | LightFlash | Find-earbuds LED |
| 0x06 | InEarDetection | 0x01 on, 0x02 off |
| 0x09 | LowLatency | Game mode |
| 0x0C | NoiseCancelMode | 0 off, 1 ANC, 2 outdoor, 3 transparency |
| 0x10 | SleepMode | |
| 0x16 | SoundBalance | 0–100, 50 center |
| 0x17 | AncSetting | mode, subScene, noiseValue |
| 0x22 | EqParamsV2 | parametric bands |
| 0x23 | LDAC | experimental on Linux |
| 0x2C | WearingDetection | |
| 0x2D | SpatialAudio | experimental |
| 0x2F | Battery | |
| 0x30 | Version | |
| 0x32 | EnvAdaptation | experimental Adaptive ANC mapping |
| 0x3D | TonePlay | locator chime |
| 0xFE | RequestData | read-back any cmd |

Destructive and never automated: `0x01` reset default, `0x02` clear pairing, `0x03` factory reset.

### ANC scenes (0x17)

| mode | sub | meaning |
| --- | --- | --- |
| 0x00 | 0x00 | Off |
| 0x02 | 1–3 | Silent / indoor |
| 0x03 | 1–3 | Working / commuting |
| 0x04 | 1–3 | Noisy |
| 0x0A | 1–7 | Transparency |

Wind reduction: **requires protocol research**.

## Firmware

Read-only. OTA/flash is not implemented.
