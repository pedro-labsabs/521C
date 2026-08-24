# QCY BLE protocol (independent notes)

Source: public reverse-engineering of QCY earphone GATT traffic, cross-checked against community documentation. **Not an official spec.** Opcodes that are not in this file are not invented.

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
