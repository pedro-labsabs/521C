# AGENTS.md — 521C repository contract

This file is the operating contract for coding agents working in this repository. Read it before editing code, documentation, issues, or pull requests.

## 0. Autonomous Prime Agent entrypoint

For a long-running autonomous delivery session, the canonical entrypoint is `PRIME_AGENT_START.md`.

Prime Agent also receives repository-specific standing policy from `.prime/agent/APPEND_SYSTEM.md`. All agents, regardless of harness, must additionally read:

- `docs/PRODUCT_SPEC.md` — what the finished product is;
- `docs/AUTONOMOUS_EXECUTION.md` — dependency graph, operating loop and release criteria;
- `docs/HOST_SAFETY.md` — what an autonomous agent may and may not do to the developer machine;
- relevant protocol/device/security docs for the surface being changed.

The repository owner delegates ordinary engineering decisions to the implementation agent. Do not block progress by repeatedly asking for choices that can be responsibly resolved from code, tests, upstream documentation and the contracts above. Preserve uncertainty for unproven protocol facts rather than asking the user to guess.

## 1. Mission

521C is an independent, unofficial, Linux-first control surface for QCY earbuds. The first device profile is QCY MeloBuds Pro / HT08. The project values protocol correctness, explicit uncertainty, safety, low overhead, and maintainable boundaries over feature count.

The final product target is a native Linux desktop application using Rust + Slint, BlueZ over D-Bus for Bluetooth, and an AppImage baseline release artifact. The existing React/TanStack surface is a development/reference/mock surface until native parity is established; it is not proof that a hardware feature is complete.

## 2. Sources of truth

Use this precedence when claims conflict:

1. Reproducible hardware observation or captured packet evidence.
2. Tested protocol implementation and fixtures.
3. Canonical protocol evidence and `docs/PROTOCOL.md` / device notes.
4. Capability matrix in code / `docs/SUPPORTED_DEVICES.md`.
5. `docs/PRODUCT_SPEC.md` for intended product behavior that does not assert protocol facts.
6. UI text, screenshots, marketing material, community claims.

Marketing or behavior in the official mobile app does not prove that a Linux-accessible protocol command exists.

## 3. Repository map

- `PRIME_AGENT_START.md` — bootstrap instruction for autonomous delivery.
- `.prime/agent/APPEND_SYSTEM.md` — Prime Agent standing operating policy.
- `docs/PRODUCT_SPEC.md` — product target and scope.
- `docs/AUTONOMOUS_EXECUTION.md` — issue dependency graph and release checklist.
- `docs/HOST_SAFETY.md` — developer-machine permission boundary.
- `src/lib/qcy/protocol/` — TypeScript framing, commands, UUIDs, advertisement parsing.
- `src/lib/qcy/device/` — device profiles and capability matrix.
- `src/lib/qcy/transport.ts` — transport boundary; mock today, native/Web Bluetooth adapters behind the same contract.
- `src/lib/qcy/hub-store.ts` — application state and command orchestration.
- `src/components/` — web/reference UI. Raw GATT bytes must not leak here.
- `native/crates/qcy-protocol/` — Rust protocol core.
- `native/crates/521cctl/` — native CLI.
- `docs/` — protocol, architecture, device support, development, product and safety documentation.
- `conformance/` — shared byte-level protocol vectors consumed by both the TypeScript and Rust test suites.
- `src/routeTree.gen.ts` — generated; do not hand-edit in ordinary work.

Tests are colocated with the code they exercise (`src/**/*.test.ts` via vitest, `native/**/tests/*.rs`); they import production modules rather than re-implementing them.

As the native application grows, preserve equivalent boundaries between UI, application/orchestration, device/profile truth, protocol codecs, transport, and Linux host services.

## 4. Non-negotiable protocol rules

- Do not invent UUIDs, opcodes, vendor IDs, payload formats, checksums, firmware fields, or capability mappings.
- Treat BLE advertisements/notifications as untrusted input. Validate SOF, declared length, block bounds, enums/ranges, and timeouts.
- Add or update a fixture/parser test before enabling a new write path.
- Keep model-specific behavior in device profiles, not scattered `if model == ...` checks in UI code.
- Capability state must remain honest. Issue #3 may evolve the concrete model, but hardware evidence, protocol evidence, implementation readiness, and write authorization must not be collapsed into a misleading single truth.
- If evidence is incomplete, preserve uncertainty. Do not upgrade a capability merely to complete a UI flow.
- A catalogued/community opcode is not automatically a trusted HT08 write.

## 5. Destructive-command safety

The following opcodes are destructive and must never be issued by unattended automation:

- `0x01` — reset defaults
- `0x02` — clear pairing
- `0x03` — factory reset

Any future interactive path for destructive operations must require explicit user intent, explain the effect, and have dedicated tests. The autonomous delivery program does not require these features and must not add them merely for completeness.

Firmware OTA is out of scope until image format, integrity checks, interruption behavior, rollback/recovery, and hardware verification are proven. Agents must not add a speculative flashing path.

Find Earbuds/chime is also special: after issue #9 it must remain interactive and preflight-gated. It must never be fired by unattended automation.

## 6. Architecture boundaries

Preserve the conceptual flow:

```text
UI -> application/state/orchestration -> device profile + protocol codec -> authorized transport
                                 \\-> Linux host services (MPRIS/PipeWire/etc.)
```

- UI receives typed state and capability metadata.
- Protocol modules own byte-level encoding/decoding.
- Device profiles own model-specific capability/evidence decisions.
- Write authorization is a shared lower-level policy (`src/lib/qcy/policy.ts`) enforced inside every transport `write`/`writeDirect`; it cannot be bypassed by UI, CLI, profiles, raw frames or future IPC. See `docs/SECURITY_MODEL.md`.
- Transport implementations own I/O and connection lifecycle.
- Host-only features must not be presented as device DSP/protocol capabilities.
- The final native UI is Slint; Bluetooth stays in Rust/BlueZ rather than a browser-only path.

Do not bypass these boundaries for a shortcut.

## 7. Host-machine boundary

Repository autonomy is not permission to reconfigure the workstation broadly. `docs/HOST_SAFETY.md` is normative.

In summary:

- prefer project-local/user-scoped dependencies;
- narrowly required official distro development packages may be installed when justified;
- normal 521C runtime must not require root;
- do not run broad OS upgrades, remove system packages, add arbitrary package repositories, replace BlueZ/PipeWire/WirePlumber, expose services to the LAN by default, or modify unrelated personal files/configuration;
- use supplied credentials only for their intended provider and `pedro-labsabs/521C`;
- never commit secrets.

If an implementation seems to require violating host safety, redesign it before asking for broader permission.

## 8. Change workflow

Before editing:

1. Read the relevant issue and acceptance criteria.
2. Read the relevant code and nearby tests.
3. Read the relevant document under `docs/` for protocol/device/host work.
4. State the invariant the change must preserve.
5. Prefer the smallest coherent change that closes the task.

During implementation:

- Keep unrelated refactors out of the same change.
- Avoid new dependencies unless the existing stack cannot solve the problem cleanly; justify native dependencies against maintenance/resource cost.
- Do not commit generated build output, caches, `node_modules`, `native/target`, credentials or private captures.
- Do not modify `Cargo.lock` unless Rust dependency resolution actually changed.
- Keep the brand as **521C**; use `521c` for filesystem/config identifiers and `521cctl` for the CLI.
- Use authoritative upstream docs/source to resolve technical unknowns rather than guessing.
- Prefer reversible changes and checkpoint before a risky repository migration.

After implementation:

1. Run focused tests for the changed surface.
2. Run the full relevant validation gate.
3. Update docs/capability/evidence matrices when behavior or protocol knowledge changed.
4. Check `git diff` for accidental generated files, secrets or unrelated changes.
5. Report what was verified and what remains hardware-dependent.

## 9. Validation gates

Single entry point (mirrors CI, exits non-zero on any failure):

```bash
./scripts/check
```

It runs, in order: eslint → tsc typecheck → vitest → vite build → `cargo fmt --check` → `cargo test --workspace` → clippy (`-D warnings`). `just check` (see §14 for the repo-local `just`) runs the same ladder.

Individual components, when iterating on one surface:

```bash
npm test              # or: npx vitest run src/lib/qcy/protocol
npm run typecheck
npm run lint
npm run build
cd native && cargo test --workspace
cd native && cargo fmt --check
cd native && cargo clippy --all-targets --all-features -- -D warnings
```

Clean/reproducible setup uses the committed lockfile: `npm ci`. Use `npm install` only when intentionally changing the dependency graph, and commit the lockfile with it.

Release/native packaging adds its own build/launch checks as implemented by the desktop work.

Do not claim a gate passed unless it was actually run successfully. If the environment prevents a gate, state that explicitly and continue all other independent verification. Never hide, skip, or weaken a failing check to get green — fix the root cause.

## 10. GitHub and pull-request expectations

For autonomous implementation, the preferred steady-state workflow is branch -> coherent commits -> PR -> CI -> review -> merge -> issue closure.

The agent is authorized to make GitHub changes for `pedro-labsabs/521C` when executing the repository plan. Do not use those credentials to modify other repositories.

A PR should contain:

- the issue/problem and scope;
- the behavioral/architectural change;
- evidence or protocol source when protocol semantics changed;
- tests added/updated;
- validation commands actually executed and CI status;
- safety impact, especially for BLE writes/host changes;
- hardware behavior actually observed versus mocked/unverified;
- remaining uncertainty or follow-up work.

Do not hide known failures behind broad wording such as “should work”. Do not bypass a failing required check to merge.

## 11. Parallel and recursive agents

Parallel work is allowed when file ownership and interfaces are clear. Good uses include independent research, fixture analysis, documentation review, or skeptical review.

Split implementation by non-overlapping surfaces, establish shared types/contracts first, and integrate through one final validation pass. Two agents should not independently redesign the same protocol shape, device profile, write policy, state schema, transport contract or UI shell.

Child/subagent claims are evidence inputs, not proof. The parent/integrator must inspect and verify their work.

## 12. Autonomous issue execution

Use `docs/AUTONOMOUS_EXECUTION.md` as the delivery graph. Do not simply process issue numbers in order.

For each issue/slice:

1. establish current baseline;
2. implement acceptance criteria;
3. add tests/evidence;
4. run focused and relevant full gates;
5. perform skeptical diff review;
6. commit/push a recoverable checkpoint;
7. use CI as evidence;
8. close/merge only when the issue is actually complete.

If hardware is unavailable, build deterministic fake/mock boundaries and continue independent work. Do not stop the entire project because one hardware verification step is pending.

## 13. Definition of done

A task is done when the requested behavior is implemented, relevant tests pass, protocol/safety/host invariants remain true, docs are synchronized where necessary, and the repository contains no unrelated/generated noise.

The **project** is done only when the release checklist in `docs/AUTONOMOUS_EXECUTION.md` is substantially evidenced. A complete-looking UI, a successful compile, or a mock-only flow is insufficient by itself.

## 14. Local agent tooling (isolated)

Repo-local portable tools live in `.tools/bin/` (git-ignored binaries, pinned and checksum-verified by `scripts/fetch-tools.sh`). Nothing is installed globally; `sudo`/`apt`/global npm/cargo installs are not allowed for agent tooling.

```bash
source scripts/env.sh   # current shell only: prepends .tools/bin to PATH
./scripts/doctor        # environment diagnostic; fetches missing .tools safely
./scripts/fetch-tools.sh  # (re)install pinned rtk + just into .tools/bin
```

`.tools/bin` contains:

- `rtk` — Rust Token Killer (rtk-ai/rtk), output-compressing proxy. There is another, unrelated project named `rtk`; verify with `rtk gain`.
- `just` — runs the `justfile` recipes without a system install.

RTK usage policy (explicit proxy, no `rtk init` — it has no Prime Agent integration):

- Prefer `rtk git ...`, `rtk cargo ...`, `rtk npm ...`, `rtk vitest ...`, `rtk tsc ...`, `rtk grep/rg ...`, `rtk diff ...`, `rtk read ...`, `rtk tree ...` when the compressed output preserves the needed information.
- Never use RTK blindly. For diagnosing a failure, rerun the original command with full output.
- Do not run `rtk init --global` (or any global init).

## 15. Agent communication and context discipline

- Investigate before asking. If code, tests, docs, or git history give enough evidence, decide and proceed; do not ask for confirmation on reversible, low-risk technical choices.
- Be concise. Do not narrate trivial commands or restate what gate output already proves.
- When finishing a task, report:

  ```text
  Status: PASS | FAIL | BLOCKED
  Alterações: <short summary>
  Validação: <checks actually run>
  Riscos: <only if they exist>
  ```

- No long retrospectives unless requested.
- Prefer targeted searches (`rtk grep`, `rtk rg`) over dumping whole files into context; read only the slices you need.
- Do not compress or truncate output when that would hide information needed for diagnosis.
- Do not recompile or re-run unchanged checks without reason; prefer the narrowest relevant test while iterating, full `./scripts/check` before declaring done.
- Do not mask problems with `any`, `eslint-disable`, `noqa`, `#[allow]`, unsafe casts, or skipped tests. Fix the root cause.
