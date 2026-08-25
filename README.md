# 521C

**Independent, unofficial Linux-first control surface for QCY earbuds.**

The name comes from Bluetooth Company ID `0x521C`, used in QCY manufacturer data. 521C is not affiliated with, endorsed by, or related to QCY / Dongguan Hele Electronics.

The first device profile is **QCY MeloBuds Pro (HT08)**. The project combines a protocol-focused core, a control UI, a CLI, diagnostics, and host-side profiles while keeping unsupported features explicitly out of the trusted control path.

## Autonomous development entrypoint

This repository is prepared for long-running autonomous coding agents, including Prime Agent.

- `PRIME_AGENT_START.md` — canonical initial instruction for taking the project through the backlog to release readiness.
- `.prime/agent/APPEND_SYSTEM.md` — Prime Agent-specific standing operating policy loaded by the harness.
- `AGENTS.md` — repository-wide engineering contract.
- `docs/PRODUCT_SPEC.md` — finished-product intent and scope.
- `docs/AUTONOMOUS_EXECUTION.md` — dependency-aware delivery graph, per-issue loop and final release checklist.
- `docs/HOST_SAFETY.md` — explicit boundary protecting the developer workstation during autonomous work.

The autonomous plan gives the implementation agent broad authority to make ordinary engineering decisions while keeping host changes, real-device writes and protocol claims tightly bounded.

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

These are current implementation surfaces, not a claim that every item is already verified end-to-end on real HT08 hardware. The support/evidence model and open issues remain authoritative for release readiness.

Three sources of truth stay separate:

| Source | Meaning |
| --- | --- |
| Hardware | What the device is advertised to support |
| Protocol | What captured/public BLE behavior can prove |
| App | What 521C actually implements and tests |

A feature in the official mobile application is **not** automatically considered a supported 521C control. Unproven controls remain `experimental`, `unknown`, or `requires-protocol-research` until the capability model is further separated by issue #3.

## Repository map

```text
.
├── .prime/agent/APPEND_SYSTEM.md # Prime Agent standing policy
├── PRIME_AGENT_START.md          # autonomous delivery bootstrap
├── AGENTS.md                     # operating contract for coding agents
├── CONTRIBUTING.md
├── docs/
│   ├── PRODUCT_SPEC.md
│   ├── AUTONOMOUS_EXECUTION.md
│   ├── HOST_SAFETY.md
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
    ├── components/           # React development/reference UI
    ├── lib/qcy/              # protocol, device profiles, transport, state
    └── routes/               # TanStack Start routes
```

## Intended Linux product architecture

The final desktop product is intentionally native and lightweight:

```text
Slint UI
  -> Rust application/orchestration
      -> device profile + protocol codec
      -> central write authorization
      -> BlueZ D-Bus transport
      -> Linux host services (MPRIS / PipeWire as applicable)
```

The current React/TanStack surface remains useful for mock development, behavior reference and secondary browser experimentation while the native path is built. It is not the primary release runtime.

## Web control surface

Requirements: Node.js 22+ and npm.

```bash
npm ci
npm run dev
```

Validation (also enforced by GitHub Actions on every push/PR):

```bash
npm ci
npm test
npm run typecheck
npm run lint
npm run build
```

`package-lock.json` is committed; use `npm ci` for clean, reproducible setup and reserve `npm install` for intentional dependency changes.

## Native core / CLI

Requirements: stable Rust toolchain.

```bash
cd native
cargo test --workspace
cargo run -p five21cctl -- status
```

Examples:

```bash
521cctl status
521cctl battery
521cctl anc adaptive
521cctl game-mode on
```

The current native CLI uses a mock HT08 transport. Real BlueZ/GATT integration belongs behind the transport boundary and must preserve the protocol safety rules. Issue #7 tracks the primary native transport.

## Protocol

The independent protocol notes live in [`docs/PROTOCOL.md`](docs/PROTOCOL.md). The main service currently documented is `0000a001-…`, with command write `00001001` and notify `00001002`.

Do **not** invent UUIDs, vendor IDs, opcodes, checksums, firmware formats, or capability mappings. Evidence and uncertainty are part of the data model.

## Safety invariants

- No root requirement and no Bluetooth daemon replacement.
- BLE input is untrusted: validate framing, lengths, bounds, enums, and timeouts.
- Unknown/generic QCY devices are read-only by default in the target architecture.
- Destructive opcodes `0x01`, `0x02`, and `0x03` are never sent by unattended automation.
- Firmware OTA is not supported until format, integrity checks, failure behavior, and recovery are proven.
- Find Earbuds/chime must remain interactive and preflight-gated before real use.
- No telemetry or implicit cloud dependency.
- Autonomous development must respect [`docs/HOST_SAFETY.md`](docs/HOST_SAFETY.md).

See [`AGENTS.md`](AGENTS.md) before making repository changes.

## License

MIT. QCY and product names are trademarks of their respective owners.
