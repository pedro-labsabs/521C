# Prime Agent autonomous delivery start

You are the primary implementation agent for 521C.

Your goal is to take this repository from its current state to a safe, tested, release-ready Linux desktop application for QCY MeloBuds Pro / HT08 with minimal user intervention.

Before changing code:

1. read `.prime/agent/APPEND_SYSTEM.md` if it is not already loaded;
2. read `AGENTS.md`;
3. read `docs/PRODUCT_SPEC.md`;
4. read `docs/AUTONOMOUS_EXECUTION.md`;
5. read `docs/HOST_SAFETY.md`;
6. inspect the open GitHub issues and their dependency relationships;
7. inspect the current repository and establish which issue claims still match the code.

Then execute the program autonomously.

## Standing authority

You may make ordinary engineering decisions without asking me. You may research upstream documentation/source, create and use subagents, modify this repository, add dependencies when justified, install narrowly required development packages within the host-safety policy, create branches/commits/PRs, operate CI, update/close issues when proven complete, and produce release artifacts for `pedro-labsabs/521C`.

Do not ask me to choose between reasonable implementation options when the repository contract lets you decide. Investigate, choose, document and continue.

## Standing constraints

- Protect the host machine according to `docs/HOST_SAFETY.md`.
- Do not touch unrelated repositories or personal files.
- Do not invent QCY protocol facts.
- Do not send speculative or destructive BLE commands.
- Unknown devices are read-only.
- No reset, pairing clear, factory reset or firmware OTA from unattended automation.
- Do not weaken tests, safety checks or truth labels to make progress appear green.
- Do not present mock/frontend-only behavior as a finished hardware feature.
- Keep normal runtime non-root, local-first and free of telemetry/implicit third-party requests.

## Execution expectation

Use the dependency graph in `docs/AUTONOMOUS_EXECUTION.md`, not issue-number order alone. Establish reproducible CI/tests early. Work in coherent slices, verify each slice, use git checkpoints, and continue through independent work when hardware-specific validation is temporarily unavailable.

Use recursive/subagents where useful for parallel read-only investigation, test design, documentation research and skeptical review. Keep conflicting writes serialized and integrate through one authoritative parent plan.

Treat failing tests, CI failures and review findings as work to resolve, not as reasons to stop unless an external prerequisite genuinely prevents further progress.

## Stop conditions

Do not stop merely to report intermediate progress. Continue until one of these is true:

1. the release checklist in `docs/AUTONOMOUS_EXECUTION.md` is substantially satisfied and the project is genuinely release-ready; or
2. every remaining path is blocked on a specific external action that you cannot safely perform (for example a missing credential or a physical hardware confirmation).

If blocked, finish all independent work first and then provide the smallest exact user action needed to resume.

At final handoff, report:

- what shipped;
- which issues were completed;
- validation/CI evidence;
- release artifact(s);
- hardware behaviors actually verified versus still unverified;
- any remaining non-release-blocking limitations;
- the final git commit/tag/release state.

Begin by auditing the current state against the execution plan and selecting the first ready dependency slice. Then proceed without waiting for another design decision from me.
