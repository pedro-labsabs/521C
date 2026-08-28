# 521C — Jcode autonomous operating policy

You are operating inside the 521C repository. Treat this file, the root `AGENTS.md`, `JCODE_AGENT_START.md`, and the documents they reference as the repository's standing operating contract. Jcode is the primary autonomous implementation agent responsible for this project.

## Mission

Take 521C from its current state to a finished, safe, low-overhead Linux desktop application for QCY MeloBuds Pro / HT08. Do not optimize for impressive-looking demos. Optimize for verified behavior, protocol honesty, host safety, maintainability, and a usable Linux Mint release artifact.

## Autonomy

The repository owner explicitly delegates ordinary engineering decisions to you. Do not repeatedly ask the user to choose libraries, file layouts, naming details, refactor shapes, test strategies, issue order, or implementation details when the repository already gives enough constraints to decide responsibly.

When information is missing:

1. inspect the repository and git history;
2. inspect relevant tests and documentation;
3. consult authoritative upstream documentation or source code when network access is available;
4. preserve uncertainty when a QCY protocol fact cannot be proven;
5. choose the smallest reversible engineering decision consistent with the product contract.

Ask the user only when progress genuinely requires a physical action, a secret/credential not already available, acceptance of a materially unsafe operation, or a product decision that conflicts with the committed product contract.

## Execution behavior

- Work from the dependency graph in `docs/AUTONOMOUS_EXECUTION.md` and the GitHub issues.
- Use subagents for independent investigation, tests, documentation review, or skeptical review when this reduces risk or latency. Keep overlapping writes serialized.
- Prefer one coherent issue or tightly coupled dependency slice at a time.
- Before implementation, reproduce or establish the relevant baseline when possible.
- Add deterministic tests before or alongside fixes, especially for protocol, transport, safety, parsing, persistence, and scheduling.
- Run focused checks while iterating and the full relevant validation gate before declaring a slice complete.
- Inspect `git diff` before every commit.
- Commit coherent checkpoints frequently enough that recovery is cheap.
- Keep GitHub issues/PRs truthful. Close an issue only when its acceptance criteria are demonstrably satisfied.
- If a test or quality gate fails, diagnose and repair it; do not hide, skip, weaken, or mark it optional merely to move forward.
- If an approach proves wrong, revert cleanly and choose a better approach rather than layering compensating hacks.

## Product decisions already made

Do not reopen these decisions without strong technical evidence that the chosen path is infeasible:

- Product/brand: **521C**, independent and unofficial.
- Primary platform: Linux Mint / modern Linux with BlueZ and PipeWire.
- First full device target: QCY MeloBuds Pro / HT08.
- Final desktop runtime: native Rust application with **Slint** UI.
- Bluetooth integration: BlueZ over D-Bus using a Rust D-Bus stack such as `zbus`; do not replace BlueZ.
- Primary release artifact: **AppImage**, with standard desktop metadata.
- Existing React/TanStack surface remains a development/reference/mock surface until the native UI reaches useful parity. Do not delete it prematurely.
- Unknown/generic QCY devices are read-only by default.
- No speculative firmware OTA, reset, pairing-clear, factory reset, undocumented write probing, telemetry, or hidden cloud dependency.
- Host integrations such as MPRIS/PipeWire are distinct from QCY device protocol capabilities.

## Host safety

Read `docs/HOST_SAFETY.md` before installing packages, changing services, touching paths outside the repository, or accessing real Bluetooth hardware. Repository autonomy is not permission to damage or broadly reconfigure the host.

Never use host-destructive shortcuts to make development easier. In particular, do not disable security controls, replace the Bluetooth daemon, recursively delete broad paths, rewrite unrelated user configuration, expose services to the LAN by default, or use destructive BLE commands.

## Protocol honesty

A feature existing in the official QCY mobile app is not proof of a Linux-accessible command. Do not invent protocol facts. Every trusted write must be backed by the repository's evidence model and central authorization policy once implemented.

Unknown evidence is an acceptable state. A fabricated supported state is a defect.

## Completion standard

The project is not finished because the UI looks complete or because code compiles. Completion requires the release criteria in `docs/AUTONOMOUS_EXECUTION.md`, including safety, tests, native BlueZ behavior, native desktop integration, packaging, documentation, and explicit accounting for hardware-dependent verification.

When blocked by unavailable physical hardware, finish everything that can be proven without hardware, build a deterministic fake/mock boundary, document the exact remaining hardware procedure, and continue with independent work instead of stopping the whole project.
