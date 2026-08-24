# Supported devices

Independent project. Not affiliated with QCY.

Legend: **S** supported · **E** experimental · **U** unsupported · **?** unknown · **R** requires protocol research

| Capability | HT08 MeloBuds Pro | Notes |
| --- | --- | --- |
| Detect / connect | S | Name match; vendorId learned from adv |
| Battery L/R/case | S | 0x2F / char 00000008 |
| Charging flags | S | bit7 |
| Firmware read | S | 0x30 |
| RSSI proximity | S | Host BLE, not GPS |
| ANC off / on | S | 0x0C |
| Adaptive ANC | E | Hardware yes; mapped via 0x32 |
| Indoor / commute / noisy | S | 0x17 scenes |
| Wind reduction | R | Official app only so far |
| Transparency + levels | S | 0x0C / 0x17 0x0A |
| Vocal enhance | E | Unnamed opcode |
| Game mode | S | 0x09 |
| Device EQ | S | 0x22 |
| System EQ | S | Host only, never claimed as DSP |
| Touch mapping | S | 0000000D |
| Wear detection | S | 0x06 / 0x2C |
| Sleep mode | S | 0x10 |
| Spatial | E | 0x2D unverified on HT08 FW |
| Multipoint status | ? | Stack property |
| Multipoint control | R | No public command |
| Find chime | S | 0x05 / 0x3D |
| Find GPS | U | Device has none |
| LDAC toggle | E | Linux codec is usually PipeWire |
| LDAC bitrate | U | Not exposed; never invented |
| Firmware OTA | U | Not yet safely supported |

Future models plug in as additional `QcyDeviceProfile` entries. Do not scatter `if model == "HT08"` through the UI.
