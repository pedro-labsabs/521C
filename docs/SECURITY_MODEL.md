# Security and safety model

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

The per-profile opcode/characteristic allowlists live with the device profile
(`src/lib/qcy/device/catalog.ts`) and must follow the evidence model; issue #6 adds
explicit provenance. Opcodes in the community catalog but not in the documented
trusted table are excluded until evidenced.

## Reporting

For a security-sensitive bug, open a minimal issue without publishing secrets, private device identifiers, or unnecessary personal data. If a future private reporting channel is added, this document should be updated before relying on it.
