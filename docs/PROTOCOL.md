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

The `0xFF` framing below is transport-agnostic. Two control paths exist:

| Path | Where it works | Status |
| --- | --- | --- |
| BLE GATT (`0000a001`) | Android (official app; vendor GATT is exposed there), Web Bluetooth | implemented (`bluez.rs` for the GATT API shape, web transport for browsers) |
| SPP/RFCOMM (`00001101`, same framing over a byte stream) | Linux/BlueZ | implemented behind the same `Transport` contract (`rfcomm.rs`, issue #50); HT08 on-wire confirmation pending |

Direct observation on the HT08 test unit (`84:AC:60:62:69:DA`, BlueZ 5.72,
issue #50): BlueZ caches `00001101` (Serial Port) for the earbuds and never
resolves `0000a001`; while connected for A2DP audio the BLE identity is asleep,
and the earbuds auto-reconnect audio aggressively. Independent
reverse-engineering projects (Jieli RCSP over SPP; see issue #50 for
provenance) converge on the same conclusion. Consequences:

- On Linux the control channel is SPP/RFCOMM. It rides the same BR/EDR ACL as
  A2DP audio, so control coexists with audio.
- Reads over SPP are `RequestData(0xFE)` exchanges: battery is
  `FF 03 FE 01 2F` answered by a `0x2F` block; version is `FF 03 FE 01 30`
  answered by a `0x30` block. `0xFE` is a read-back request, not a state
  mutation, so the write policy authorizes it even for read-only devices.
- SPP has no GATT characteristics: unframed direct writes (`0000000B`,
  `0000000D`) do not exist on that path; EQ and key-function travel as framed
  opcodes.
- The RFCOMM channel is resolved via SDP for `00001101` (expected channel 1;
  corroborated by independent Jieli-earbud projects). HT08-specific channel
  and on-wire behavior remain pending hardware confirmation — see
  `docs/devices/HT08.md`. `scripts/sdp-rfcomm-channel.py` is the read-only
  Stage-1 query tool.

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
