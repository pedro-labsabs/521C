# Supported devices

Independent project. Not affiliated with QCY.

Every capability is described by four independent truths (see
`src/lib/qcy/device/capabilities.ts`): **hardware** (is it a feature of the model?),
**protocol** (is the behavior evidenced?), **implementation** (does this build do it?),
and **write** (is it writable / experimental / read-only / forbidden). The table below
reports the **app-level readiness** a user actually sees, derived from those truths — it
never conflates "the device/protocol can do this" with "this build implements it".

Legend:
**S** supported (implemented, writable) · **RO** read-only · **E** experimental (session
opt-in) · **M** mock only · **H** host integration pending (#13) · **P** protocol known,
app pending · **R** needs protocol research · **U** unsupported · **F** forbidden · **?** unknown

| Capability | HT08 MeloBuds Pro | Notes |
| --- | --- | --- |
| Detect / connect | S | Name match; vendorId learned from adv |
| Battery L/R/case | RO | 0x2F / char 00000008 |
| Charging flags | RO | bit7 |
| Firmware read | RO | 0x30 |
| RSSI proximity | RO | Host BLE, not GPS |
| ANC off / on | S | 0x0C |
| Adaptive ANC | E | Hardware yes; mapped via 0x32 |
| Indoor / commute / noisy | S | 0x17 scenes |
| Wind reduction | R | Official app only so far |
| Transparency + levels | S | 0x0C / 0x17 0x0A |
| Vocal enhance | R | Mentioned in reviews; no named opcode yet |
| Game mode | S | 0x09 |
| Auto game mode | H | Host automation; no observer yet (#13). No BLE traffic |
| Device EQ | S | 0x22 |
| System EQ | H | Host PipeWire EQ; not implemented yet (#13). Never written to buds |
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
| Codec status | M | Host audio graph; currently mocked (#13) |
| Firmware OTA | F | Not yet safely supported; no flash path sent |

Future models plug in as additional `QcyDeviceProfile` entries. Do not scatter
`if model == "HT08"` through the UI. Host-side features (System EQ, Auto game mode,
Codec status) stay **H**/**M** until a real host backend exists; they are never presented
as QCY protocol capabilities and never generate earbud writes.
