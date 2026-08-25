# Security policy

521C talks to physical Bluetooth devices through a reverse-engineered vendor
protocol, so "security" includes both conventional software security and
device-safety behavior.

## Supported versions

Pre-1.0, only the latest `main` and the latest tagged release receive security
fixes.

## Reporting a vulnerability

Open a GitHub issue (or contact the repository owner directly) with:

- a minimal reproduction or description;
- the affected layer (protocol parsing, write policy, transport, host
  integration, packaging);
- whether the issue could lead to unsafe device commands or host mutation.

Do **not** include secrets, private device identifiers, or unnecessary personal
data in public reports.

## Scope notes

The security/safety model is documented in `docs/SECURITY_MODEL.md`:

- BLE advertisements/notifications are treated as untrusted input;
- every outbound write converges on the central authorization policy;
- destructive opcodes (`0x01`/`0x02`/`0x03`) are unreachable from automation;
- unknown devices stay read-only;
- the default runtime makes no implicit third-party network requests.

Host-safety boundaries for automated/agent work are in `docs/HOST_SAFETY.md`.
