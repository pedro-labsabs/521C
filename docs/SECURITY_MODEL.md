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
- Device sessions are transactional: a failed connect, a failed service/
  characteristic resolution, a remote disconnect, or a replacement connect
  invalidates the whole session (device identity and every cached GATT handle).
  Read/write/subscribe report a disconnected error until a complete new
  connection succeeds; no I/O may ever be routed to a stale characteristic
  from a previous device/session.

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

**Interactive model confirmation (renamed devices).** When advertisement/name
evidence cannot prove the model — for example earbuds the owner renamed, so the
name no longer contains `MeloBuds Pro`/`HT08` — the device stays read-only and the
UI may offer an explicit confirmation ("this is a QCY MeloBuds Pro (HT08)"). The
confirmation is an explicit human attestation, treated as identification evidence:

- it applies only to the currently connected device; the core refuses
  confirmations for any other address;
- it lifts the read-only state for that connection and is remembered for the rest
  of the session (reconnects do not re-ask);
- the application layer persists it as the **local-only** config field
  `knownDevices` (a bounded address list), never exported and never synced,
  because Bluetooth addresses are privacy-sensitive;
- it never changes the policy itself: destructive opcodes stay forbidden and
  experimental opcodes still require the session opt-in.

Direct (unframed) writes are additionally restricted to the profile's allowlisted
characteristics. Denials return a structured `{ code, message, opcode? }` result and
surface as a `WriteDeniedError` at the transport, which the store reports as a toast.

The native transport enforces the same contract in Rust
(`native/crates/qcy-transport/src/policy.rs`): every framed write is fully decoded
and **every** command block is authorized before any byte reaches BlueZ, so a
supported first block can never smuggle a destructive or unauthorized block past
the policy; undecodable frames are denied as malformed. The Rust policy is
aligned with the browser policy with one deliberate fail-safe difference:
(a) both policies allow `RequestData` (`0xFE`) frames even on read-only
devices — a read-back request, not a state mutation — and the native policy
pins additionally that a `0xFE` block can never smuggle a state-changing
block past the read-only verdict; (b) the native policy has **no**
pure-disable exception for experimental opcodes — no native flow sends an
experimental write without a session opt-in, even one that looks like a
disable. If a future native flow needs that exception, align this document
and both policy implementations in one change.

The per-profile opcode/characteristic allowlists live with the device profile
(`src/lib/qcy/device/catalog.ts`) and must follow the evidence model; issue #6 adds
explicit provenance. Opcodes in the community catalog but not in the documented
trusted table are excluded until evidenced.

## Reporting

For a security-sensitive bug, open a minimal issue without publishing secrets, private device identifiers, or unnecessary personal data. If a future private reporting channel is added, this document should be updated before relying on it.
