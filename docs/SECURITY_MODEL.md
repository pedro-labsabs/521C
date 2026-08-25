# Security and safety model

**Authority:** normative for trust boundaries, write authorization and
network behavior.

521C talks to physical Bluetooth devices over a reverse-engineered vendor protocol. Safety therefore includes both conventional software security and protection against unsafe device commands.

## Trust boundaries

- BLE advertisements and notifications are untrusted input.
- Device model identification is evidence-based and may remain unknown.
- UI controls consume typed state/capabilities, not arbitrary GATT bytes.
- Host-side automation is less trusted than an explicit, interactive user action for destructive operations.

## Mandatory invariants

- Validate SOF, declared lengths, block bounds, enum/range values, and timeouts.
- Never fabricate protocol constants to make a feature appear complete.
- Never automatically send reset (`0x01`), clear-pairing (`0x02`), or factory-reset (`0x03`).
- Firmware OTA remains disabled until the full image format, integrity mechanism, interruption behavior, and recovery path are independently verified.
- Avoid root and privileged daemon replacement.
- Do not add telemetry as a hidden dependency or default behavior.

## Network behavior (local-first)

521C is local-first. The default runtime makes **no implicit third-party network
requests** (issue #12):

- No web fonts, CDN stylesheets, analytics, or remote assets are fetched at
  runtime. The UI uses a system font stack; any bundled asset ships in the
  repository with compatible licensing.
- The only network traffic the app initiates is the user's explicit Bluetooth
  connection to their own device (Web Bluetooth in the browser, BlueZ/D-Bus on
  the native path).
- The dev server binds to loopback (`127.0.0.1`) by default. LAN exposure is
  opt-in via `npm run dev:lan`. Preview/release paths stay local unless a user
  deliberately exposes them.
- User-initiated navigation to documentation or source links is allowed by this
  contract; implicit runtime traffic is not.

This behavior is enforced by two automated guards: a source-level test
(`src/lib/privacy/network-audit.test.ts`, run by `npm test`) and a build audit
(`npm run audit:network`, run in CI after the build) that scans the compiled
output. Intentional exceptions are listed and justified in
`scripts/audit-network.mjs`; adding a new third-party runtime URL requires a
documented allowlist entry and a review of this section.

## Central write authorization

Every outbound BLE write passes through one policy layer, `src/lib/qcy/policy.ts`,
owned below the UI. The policy is enforced inside each transport's `write()` and
`writeDirect()`, so no caller — UI action, profile automation, in-app CLI, future
native bridge, or a raw frame built in a test — can bypass it by reaching for a
lower-level call.

Decision rules, in order, for every command block in a frame:

1. **Destructive opcodes** (`0x01` reset-defaults, `0x02` clear-pairing,
   `0x03` factory-reset) are rejected at this boundary regardless of caller,
   device profile, or opt-in. They are never writable.
2. **`RequestData` (`0xFE`)** is a read-back request, not a state mutation, and is
   allowed even for read-only profiles so status/identification can be read.
3. **Unknown/generic devices are read-only.** Any state-changing write to a profile
   flagged `readOnly` is denied until the model is identified.
4. **Supported opcodes** listed in the connected profile's `writePolicy` are allowed.
5. **Experimental opcodes** require an explicit, visible session opt-in to *enable*.
   A pure *disable* (single `0x02` param) is always allowed so supported flows can
   leave an experimental feature off. The opt-in is session-scoped, never persisted,
   and resets on restart or transport change.
6. Anything else is denied as not writable.

Direct (unframed) writes are additionally restricted to the profile's allowlisted
characteristics. Denials return a structured `{ code, message, opcode? }` result and
surface as a `WriteDeniedError` at the transport, which the store reports as a toast.

The native transport enforces the same contract in Rust
(`native/crates/qcy-transport/src/policy.rs`): every framed write is fully decoded
and **every** command block is authorized before any byte reaches BlueZ, so a
supported first block can never smuggle a destructive or unauthorized block past
the policy; undecodable frames are denied as malformed. The Rust policy is
deliberately stricter than the browser policy in two fail-safe ways: (a) no
`RequestData` (`0xFE`) exception for read-only devices — native status reads use
plain GATT characteristic reads, so unknown devices receive no framed writes at
all; and (b) no pure-disable exception for experimental opcodes — no native flow
sends experimental writes without a session opt-in. If a future native flow needs
either exception, align this document and both policy implementations in one
change.

The per-profile opcode/characteristic allowlists live with the device profile
(`src/lib/qcy/device/catalog.ts`) and must follow the evidence model; issue #6 adds
explicit provenance. Opcodes in the community catalog but not in the documented
trusted table are excluded until evidenced.

## Reporting

For a security-sensitive bug, open a minimal issue without publishing secrets, private device identifiers, or unnecessary personal data. If a future private reporting channel is added, this document should be updated before relying on it.
