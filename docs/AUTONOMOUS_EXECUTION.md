# Autonomous execution plan

**Authority:** normative execution plan for Jcode, the primary autonomous implementation agent, and any delegated coding agents.

This document converts the repository backlog into a bounded delivery program. The goal is not to process issue numbers mechanically; it is to reach a safe, tested Linux desktop release while respecting the dependency graph and preserving protocol evidence.

## 1. End state

521C is considered delivery-ready when all of the following are true:

- the application runs as a native, non-root Linux desktop application;
- QCY MeloBuds Pro / HT08 is the first fully supported device profile;
- unknown/generic QCY devices are read-only by default;
- outbound BLE writes pass through one central authorization layer;
- destructive opcodes and speculative firmware OTA are unreachable from unattended operation;
- the native Linux path uses BlueZ over D-Bus rather than replacing or bypassing the system Bluetooth daemon;
- the native UI is implemented with Rust + Slint and consumes typed state/actions rather than raw GATT bytes;
- the application can scan/select/connect/disconnect, report proven state, and perform only evidence-backed device operations;
- host-only functionality (MPRIS, PipeWire/system EQ, codec/audio state, Auto Game Mode) is implemented behind a host-services boundary rather than misrepresented as QCY protocol functionality;
- persistence/import is versioned and validated;
- the default runtime has no telemetry or implicit third-party network traffic;
- tests exercise production protocol code, shared protocol vectors, write authorization, scheduling, config validation and mocked native boundaries;
- Node and Rust quality gates pass from a clean checkout using locked/reproducible dependencies;
- at least one AppImage suitable for Linux Mint is produced with desktop metadata and documented installation/removal;
- docs describe the architecture, evidence model, support matrix, development workflow, safety model and remaining hardware-verified limitations accurately;
- every closed GitHub issue has evidence that its acceptance criteria were met.

A polished mock UI without real native integration is not completion. A working real-device prototype without safety, tests, evidence provenance and packaging is also not completion.

## 2. Fixed product/architecture decisions

These choices are already delegated by the repository owner and should be treated as defaults:

| Area | Decision |
| --- | --- |
| Product name | 521C |
| Primary OS | Linux Mint / modern Linux |
| First full device | QCY MeloBuds Pro / HT08 |
| Desktop UI | Slint |
| Native language | Rust |
| Bluetooth | BlueZ D-Bus via a Rust D-Bus stack such as zbus |
| Audio stack | PipeWire/WirePlumber integration where needed; never replace them |
| Media control | MPRIS over D-Bus |
| Distribution | AppImage first |
| Web UI | development/reference/mock surface until native parity; secondary runtime |
| Generic QCY behavior | discovery/read-only unless device/profile evidence proves more |
| Telemetry | none by default |
| Firmware OTA | out of scope until independently proven safe |

If implementation evidence shows a chosen library is technically infeasible, document the failure and select the closest architecture-preserving alternative. Do not reopen settled choices merely because another stack is fashionable or easier for a short demo.

## 3. Delivery graph

The existing issues are the executable backlog. Use their acceptance criteria as task contracts.

### Phase A — make future work trustworthy

Recommended order:

1. **#5 CI / reproducible dependency locking**
2. **#4 production protocol tests + shared conformance vectors**
3. **#1 central BLE write authorization**
4. **#6 protocol evidence provenance**
5. **#3 capability truth model**
6. **#9 Find Earbuds preflight safety**

Rationale: the project needs deterministic gates before large changes, then real tests of shipped protocol code, then the safety/evidence model that every real transport depends on.

Parallelism is acceptable only where files/interfaces do not overlap. For example, CI scaffolding can be investigated in parallel with fixture design, but write-policy and capability-model changes should not be independently redesigned by separate agents.

### Phase B — real transport and command reliability

After #1/#4/#6 are satisfied:

7. **#2 Web Bluetooth real-device transport** — useful as a secondary development path and fake-GATT proving ground.
8. **#7 native BlueZ transport + real `521cctl` I/O** — primary Linux device path.
9. **#10 BLE command scheduling/coalescing/confirmation** — integrate against real transport semantics.

The native path is the release path. Do not allow browser limitations to dictate the Linux desktop architecture.

### Phase C — native product

After #1/#3/#7 are satisfied:

10. **#8 native desktop integration and AppImage packaging**
11. **#11 versioned/validated persistence and import schema**
12. **#12 local-only/privacy defaults**
13. **#13 real Linux host services (MPRIS/PipeWire/codec/Auto Game Mode)**

Issue #8 may establish shared native application structure needed by #11/#13. Keep protocol, transport, host services and UI separated so later work does not collapse into one monolithic crate.

### Phase D — repository/release maturity

14. **#15 documentation structure and conventions**
15. **#14 governance/contribution baseline**
16. **#16 labels, milestones and triage conventions**

These may be partially advanced earlier if they directly improve execution, but they must not distract from P0/P1 safety and native-runtime dependencies.

### Phase E — final convergence

After all required issue work:

- run the entire validation ladder from a clean checkout;
- build release artifacts;
- run static review for implicit network access, secrets, generated files and dead/mock-only code accidentally presented as production;
- run a skeptical architecture/safety review using a separate subagent/context;
- exercise mock/fake end-to-end paths;
- run read-only hardware discovery if HT08 is available;
- execute only the proven safe hardware validation matrix described below;
- reconcile README/support matrix with actual behavior;
- create a release candidate and record unresolved hardware/environment limitations explicitly.

## 4. Per-issue operating loop

For each issue or tightly coupled slice:

1. **Read the issue contract.** Extract acceptance criteria and dependencies.
2. **Inspect actual code.** Do not trust issue prose if implementation has moved.
3. **Establish baseline.** Reproduce the defect/gap or identify the missing behavior with tests/evidence where possible.
4. **Plan the smallest coherent implementation.** State which boundaries/invariants must remain true.
5. **Implement.** Avoid unrelated refactors.
6. **Test locally.** Add regression/unit/integration tests appropriate to the surface.
7. **Run relevant full gates.** A focused test is not a substitute for the repository gate.
8. **Review the diff skeptically.** Look for policy bypasses, generated noise, dependency creep and fake state.
9. **Update docs/evidence.** Only when behavior or knowledge changed.
10. **Commit/push a coherent checkpoint.** Use an issue-referencing message.
11. **Use CI as evidence.** Do not merge known-red work.
12. **Close/merge only when proven.** Record what was tested and what remains hardware-dependent.

If an issue reveals a separate defect, create a bounded follow-up issue unless fixing it is required for the current acceptance criteria. Do not silently expand scope until the entire repository is being rewritten.

## 5. Git/GitHub policy for autonomous execution

The agent is authorized to operate on **`pedro-labsabs/521C` only** using supplied credentials.

Preferred workflow after CI exists:

- create a branch per issue or tightly coupled issue slice;
- make small coherent commits;
- push the branch;
- open a PR referencing the issue(s);
- run/observe CI;
- repair failures rather than bypassing checks;
- merge only after required checks pass and acceptance criteria are satisfied;
- close the issue with evidence if GitHub automation does not do so automatically.

Before CI exists, establish #5 early and keep pre-CI changes especially small/reviewable.

Do not rewrite public history on `main`. Do not force-push `main`. Do not touch other repositories with these credentials.

## 6. Research policy

Research is expected when implementation requires current BlueZ, D-Bus, Slint, PipeWire, AppImage, browser or QCY information.

Priority of evidence:

1. reproducible local/hardware observation;
2. upstream project documentation/source;
3. standards/specifications;
4. well-maintained open-source implementations with inspectable code;
5. community reports as leads only.

For proprietary QCY protocol facts, community code may suggest what to investigate but does not automatically elevate a write to trusted/safe. Record provenance and confidence.

Never discover unknown write semantics by firing arbitrary opcodes at the user's earbuds.

## 7. Hardware validation matrix

Hardware testing is valuable but must be staged.

### Stage 1 — before central authorization/evidence gates

Allowed:

- adapter/device discovery;
- read-only BlueZ object inspection;
- service/characteristic enumeration;
- subscription to known notifications;
- known-safe characteristic reads;
- packet capture/observation that does not mutate device state.

Not allowed: device writes.

### Stage 2 — after #1 and #6

Allow only operations that are all of:

- mapped to the proven HT08 profile;
- represented by sufficient evidence/provenance;
- accepted by the central write policy;
- reversible/non-destructive;
- covered by serializer/policy tests.

Validate one operation at a time and reconcile observed state after the write.

### Stage 3 — special interactive actions

Find Earbuds/chime may only be validated after #9 and requires a person to confirm the earbuds are not being worn. It must not be run by background/autonomous loops.

Factory reset, pairing clear, reset-default opcodes and speculative OTA remain excluded from autonomous hardware validation.

## 8. Performance and resource expectations

521C is intended to be lightweight on ordinary Linux laptops.

Design expectations:

- event-driven Bluetooth and host integration;
- no permanent aggressive polling loops;
- bounded channels/queues and command coalescing;
- no embedded browser requirement in the final native runtime;
- avoid large frameworks/dependencies when a small native solution is sufficient;
- no background daemon unless a concrete product requirement proves it necessary;
- disconnected/idle state should approach negligible CPU usage;
- memory usage should be measured on a representative Linux Mint build and regressions documented.

Treat performance as an engineering constraint, not as an excuse to remove correctness checks.

## 9. When the agent may decide without the user

The agent should decide autonomously on:

- crate/module/file organization;
- naming of internal types/functions;
- test framework details within the existing stack;
- small dependency choices after evaluating maintenance/cost;
- retry/timeout values supported by tests and documented rationale;
- error types and logging structure;
- issue ordering within the dependency graph;
- refactors required to satisfy an accepted architecture boundary;
- CI cache/config details;
- packaging implementation details consistent with AppImage output;
- accessibility/usability details that do not alter the product contract.

Prefer documented, reversible decisions.

## 10. When user intervention is actually required

Only stop for the user when no safe autonomous path remains, such as:

- a required credential/secret is absent;
- a physical confirmation is required (for example safe chime validation);
- the OS requests interactive privilege authentication that cannot be avoided and the action is allowed by `docs/HOST_SAFETY.md`;
- a hardware behavior cannot be proven without the user connecting/positioning the device;
- a product choice would contradict the fixed decisions above rather than merely refine implementation.

When blocked on one of these, continue all independent work first. At handoff, ask for the smallest concrete action needed, not a broad design decision.

## 11. Failure and recovery

- Preserve failing test output or a concise reproduction record.
- Do not weaken tests merely because implementation is difficult.
- Do not turn unsupported/unknown capabilities into supported to satisfy UI expectations.
- Use git checkpoints before large migrations.
- If a dependency or architecture choice fails, revert the failed slice cleanly and try one better-supported approach.
- If upstream behavior is ambiguous, expose uncertainty in the type/model and continue with safe surfaces.

## 12. Final release checklist

Before declaring the autonomous program complete:

- [ ] all P0/P1 functional/safety issues are closed with evidence;
- [ ] remaining P2 issues required by the end-state are closed or explicitly justified as non-release-blocking;
- [ ] `npm ci` succeeds from a clean checkout;
- [ ] `npm test` passes;
- [ ] `npm run typecheck` passes;
- [ ] `npm run lint` passes;
- [ ] `npm run build` passes;
- [ ] Rust workspace tests pass;
- [ ] `cargo fmt --check` passes;
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` passes;
- [ ] release/native build succeeds;
- [ ] mock/fake integration path passes deterministically;
- [ ] write-policy bypass review finds no alternate raw write path;
- [ ] unknown device remains read-only in tests;
- [ ] destructive commands are rejected at the lowest practical boundary;
- [ ] AppImage is produced and launch-tested on Linux Mint or an equivalent clean VM;
- [ ] desktop metadata/install/uninstall path are documented;
- [ ] default runtime performs no implicit third-party network requests;
- [ ] README and support matrix match shipped behavior;
- [ ] hardware-verified and hardware-unverified claims are clearly distinguished;
- [ ] no credentials, private captures, caches or unrelated generated outputs are committed;
- [ ] final skeptical review finds no unresolved release-blocking defect.

Only after this checklist is substantially evidenced should the agent describe 521C as finished/release-ready.
