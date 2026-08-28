# 521C

**Independent, unofficial Linux-first control surface for QCY earbuds.**

The name comes from Bluetooth Company ID `0x521C`, used in QCY manufacturer data. 521C is not affiliated with, endorsed by, or related to QCY / Dongguan Hele Electronics.

The first device profile is **QCY MeloBuds Pro (HT08)**. The project combines a protocol-focused core, a control UI, a CLI, diagnostics, and host-side profiles while keeping unsupported features explicitly out of the trusted control path.

## Autonomous development entrypoint

This repository is configured for Jcode as its primary long-running autonomous coding agent.

- `JCODE_AGENT_START.md` — canonical initial instruction for taking the project through the backlog to release readiness.
- `.jcode/prompt-overlay.md` — Jcode project overlay loaded automatically in new Jcode sessions.
- `AGENTS.md` — repository-wide engineering contract.
- `docs/PRODUCT_SPEC.md` — finished-product intent and scope.
- `docs/AUTONOMOUS_EXECUTION.md` — dependency-aware delivery graph, per-issue loop and final release checklist.
- `docs/HOST_SAFETY.md` — explicit boundary protecting the developer workstation during autonomous work.
- `docs/GOVERNANCE.md` — review/merge standards, release policy and maintainer checklist; `docs/TRIAGE.md` — labels and milestones.

The autonomous plan gives the implementation agent broad authority to make ordinary engineering decisions while keeping host changes, real-device writes and protocol claims tightly bounded.

## Current capabilities

- Left / right / case battery and charging flags
- Firmware readout
- ANC and transparency scenes
- Game / low-latency mode
- Device EQ
- Touch mapping
- Wear detection and sleep mode
- Find-earbuds chime gated by an interactive preflight: blocked by default while a target bud is worn, stronger confirmation when wear state is unknown, a short cooldown, and no unattended/automation path
- Host-side smart profiles and diagnostics
- Mock transport for development without hardware

These are current implementation surfaces, not a claim that every item is already verified end-to-end on real HT08 hardware. The per-feature readiness matrix in `docs/SUPPORTED_DEVICES.md` is canonical; the support/evidence model and open issues remain authoritative for release readiness.

Every capability keeps four truths separate (implemented in
`src/lib/qcy/device/capabilities.ts`), so the UI, CLI and docs can never confuse "the
device/protocol can do this" with "this build implements it":

| Truth | Question it answers |
| --- | --- |
| Hardware | Is the feature associated with this model? |
| Protocol | Is the behavior evidenced for this model/firmware? |
| Implementation | Does this build actually implement and test it? |
| Write | Is it writable, experimental (opt-in), read-only, or forbidden? |

Deterministic rules derive what is shown, enabled, and writable from those truths. A
feature in the official mobile application is **not** automatically a supported 521C
control, and a protocol/catalog opcode is **not** automatically implemented. Host-side
features (system EQ, auto game mode, codec status) stay `not-implemented` or `mock-only`
until a real host backend exists (issue #13); they are never presented as QCY protocol
capabilities and never generate earbud writes.

## Repository map

```text
.
├── .jcode/prompt-overlay.md      # Jcode project operating policy
├── JCODE_AGENT_START.md          # autonomous delivery bootstrap
├── AGENTS.md                     # operating contract for coding agents
├── CONTRIBUTING.md
├── CHANGELOG.md
├── SECURITY.md
├── conformance/              # shared byte-level protocol/config vectors (TS + Rust)
├── docs/
│   ├── PRODUCT_SPEC.md
│   ├── AUTONOMOUS_EXECUTION.md
│   ├── HOST_SAFETY.md
│   ├── SECURITY_MODEL.md
│   ├── PROTOCOL.md
│   ├── ARCHITECTURE.md
│   ├── DESKTOP_ARCHITECTURE.md
│   ├── DEVELOPMENT.md
│   ├── SUPPORTED_DEVICES.md
│   ├── GOVERNANCE.md
│   ├── TRIAGE.md
│   └── devices/HT08.md
├── native/
│   └── crates/
│       ├── qcy-protocol/     # Rust framing / advertisement parser
│       ├── qcy-device/       # HT08 profile + capability truth (Rust mirror)
│       ├── qcy-transport/    # transport boundary: mock + BlueZ, central WritePolicy
│       ├── qcy-host/         # host services: MPRIS, codec, system EQ, auto game mode
│       ├── qcy-app/          # application core: typed commands/events, config schema
│       ├── 521cctl/          # native CLI (binary `521cctl`)
│       └── 521c-desktop/     # Slint desktop app (binary `521c`)
├── packaging/linux/          # .desktop entry + AppStream metadata
├── scripts/                  # network/docs audits, AppImage packaging
└── src/
    ├── components/           # React development/reference UI
    ├── lib/qcy/              # protocol, evidence ledger, policy, profiles, transport, state
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
npm run audit:network
npm run docs:check
```

`package-lock.json` is committed; use `npm ci` for clean, reproducible setup and reserve `npm install` for intentional dependency changes.

## Native core / CLI

Requirements: stable Rust toolchain.

```bash
cd native
cargo test --workspace
cargo run -p five21cctl -- status
```

Examples (mock transport is the deliberate default — no hardware needed):

```bash
521cctl scan
521cctl status
521cctl battery
521cctl anc transparency
521cctl game-mode on
```

Operate a real device through the system BlueZ stack by passing `--bluez` and
selecting the target explicitly:

```bash
521cctl --bluez scan
521cctl --bluez --device F8:5C:7D:12:08:08 status
521cctl --bluez --device F8:5C:7D:12:08:08 anc transparency
```

The native transport lives in `native/crates/qcy-transport`: a single `Transport`
trait with a deterministic mock backend and a BlueZ/D-Bus backend. Every outbound
write passes the central write-authorization policy first — destructive opcodes are
never sent, unknown models stay read-only, and experimental opcodes need a session
opt-in. Discovery only surfaces candidate QCY devices and preserves unknown-model
status; it never invents capabilities.

## Protocol

The independent protocol notes live in [`docs/PROTOCOL.md`](docs/PROTOCOL.md). The main service currently documented is `0000a001-…`, with command write `00001001` and notify `00001002`.

Do **not** invent UUIDs, vendor IDs, opcodes, checksums, firmware formats, or capability mappings. Evidence and uncertainty are part of the data model.

## Safety invariants

- No root requirement and no Bluetooth daemon replacement.
- BLE input is untrusted: validate framing, lengths, bounds, enums, and timeouts.
- Unknown/generic QCY devices are read-only by default in the target architecture.
- Destructive opcodes `0x01`, `0x02`, and `0x03` are never sent by unattended automation.
- Firmware OTA is not supported until format, integrity checks, failure behavior, and recovery are proven.
- Find Earbuds/chime is interactive and preflight-gated: no locator tone is transmitted before confirmation completes, known-worn targets are blocked by default, unknown wear state requires a stronger explicit confirmation, a cooldown rate-limits repeats, and the CLI/automation path refuses it.
- No telemetry or implicit cloud dependency.
- Autonomous development must respect [`docs/HOST_SAFETY.md`](docs/HOST_SAFETY.md).

See [`AGENTS.md`](AGENTS.md) before making repository changes.

## License

MIT. QCY and product names are trademarks of their respective owners.
