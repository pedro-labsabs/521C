# <MODEL NAME> / <vendor model code> notes

**Role:** device notes (research/informative). Copy this file to
`docs/devices/<MODEL>.md` when adding a device. Marketing items are hardware
context, not protocol evidence; open items must not be invented.

Hardware (marketing / reviews, not protocol):

- <chipset, Bluetooth version, codecs, drivers, mics, battery claims…>

Protocol (public opcodes we implement, with evidence class):

- <opcode/behavior — evidence: protocol-doc | hardware-capture | community-catalog | official-app>

Identification evidence:

- <advertised name fragments, manufacturer data layout, vendor IDs — only what
  was actually observed; otherwise "unknown">

Still open:

- <questions that must be answered with evidence before writes are enabled>

Do not invent the open items. Opcode/UUID provenance and writability are
governed by the evidence ledger (`src/lib/qcy/protocol/evidence.ts`); see
`docs/PROTOCOL.md` → "Evidence and trust levels".
