# 521C

**Independent, unofficial Linux-first control surface for QCY earbuds.**

The name comes from Bluetooth Company ID `0x521C`, used in QCY manufacturer data. 521C is not affiliated with, endorsed by, or related to QCY / Dongguan Hele Electronics.

The first device profile is **QCY MeloBuds Pro (HT08)**. The project combines a protocol-focused core, a control UI, a CLI, diagnostics, and host-side profiles while keeping unsupported features explicitly out of the trusted control path.

## Current capabilities

- Left / right / case battery and charging flags
- Firmware readout
- ANC and transparency scenes
- Game / low-latency mode
- Device EQ
- Touch mapping
- Wear detection and sleep mode
- Find-earbuds chime with safety guardrails
- Host-side smart profiles and diagnostics
- Mock transport for development without hardware

Three sources of truth stay separate:

| Source | Meaning |
| --- | --- |
| Hardware | What the device is advertised to support |
| Protocol | What captured/public BLE behavior can prove |
| App | What 521C actually implements and tests |

A feature in the official mobile application is **not** automatically considered a supported 521C control. Unproven controls remain `experimental`, `unknown`, or `requires-protocol-research`.

## Repository map

```text
.
├── AGENTS.md                 # operating contract for coding agents
├── CONTRIBUTING.md
├── docs/
│   ├── ARCHITECTURE.md
│   ├── DEVELOPMENT.md
│   ├── PROTOCOL.md
│   ├── SECURITY_MODEL.md
│   ├── SUPPORTED_DEVICES.md
│   └── devices/HT08.md
├── native/
│   └── crates/
│       ├── qcy-protocol/     # Rust framing / advertisement parser
│       └── 521cctl/          # native CLI
├── scripts/                  # focused repository tests
└── src/
    ├── components/           # UI
    ├── lib/qcy/              # protocol, device profiles, transport, state
    └── routes/               # TanStack Start routes
```

## Web control surface

Requirements: Node.js 22+ and npm.

```bash
npm install
npm run dev
```

Validation:

```bash
npm test
npm run typecheck
npm run lint
npm run build
```

## Native core / CLI

Requirements: stable Rust toolchain.

```bash
cd native
cargo test --workspace
cargo run -p 521cctl -- status
```

Examples:

```bash
521cctl status
521cctl battery
521cctl anc adaptive
521cctl game-mode on
```

The current native CLI uses a mock HT08 transport. Real BlueZ/GATT integration belongs behind the transport boundary and must preserve the protocol safety rules.

## Protocol

The independent protocol notes live in [`docs/PROTOCOL.md`](docs/PROTOCOL.md). The main service currently documented is `0000a001-…`, with command write `00001001` and notify `00001002`.

Do **not** invent UUIDs, vendor IDs, opcodes, checksums, firmware formats, or capability mappings. Evidence and uncertainty are part of the data model.

## Safety invariants

- No root requirement and no Bluetooth daemon replacement.
- BLE input is untrusted: validate framing, lengths, bounds, enums, and timeouts.
- Destructive opcodes `0x01`, `0x02`, and `0x03` are never sent by automations.
- Firmware OTA is not supported until format, integrity checks, failure behavior, and recovery are proven.
- No telemetry.

See [`AGENTS.md`](AGENTS.md) before making repository changes.

## License

MIT. QCY and product names are trademarks of their respective owners.
