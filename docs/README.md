# Documentation index

**Role:** informative navigation. This page maps every repository document to
its role and authority so contributors know which document to consult and how
much weight each carries.

## 1. Document roles

| Role | Meaning |
| --- | --- |
| Normative | Binding contract. Code and other docs must not contradict it; conflicts are resolved in its favor (see `AGENTS.md` §2 precedence). |
| Informative | Describes the current design or workflow. Kept accurate, but a mismatch means the doc is stale, not that the code is wrong. |
| Derived | Generated/maintained from code truth (capability matrices, changelog). Updated in the same PR as the behavior change. |
| Research/experimental | Records investigation or unproven ideas. Never a source of writable protocol facts. |
| Historical | Superseded; kept for provenance. |

Every major document declares its role in its first lines with a
`**Authority:**` or `**Role:**` marker. `scripts/check-docs.mjs` (run as
`npm run docs:check`) enforces the marker for top-level docs.

## 2. The documents

### Contracts (normative)

| Document | Scope |
| --- | --- |
| `AGENTS.md` (repo root) | engineering contract for all contributors and agents |
| `docs/PRODUCT_SPEC.md` | product intent, scope boundaries, truth model, definition of done |
| `docs/AUTONOMOUS_EXECUTION.md` | delivery graph, per-issue loop, release checklist |
| `docs/HOST_SAFETY.md` | developer-machine and hardware permission boundary |
| `docs/SECURITY_MODEL.md` | trust boundaries, write authorization, network behavior |
| `docs/PROTOCOL.md` | protocol evidence notes (independent reverse-engineering; not an official spec) |
| `docs/GOVERNANCE.md` | review/merge standards, release/versioning policy |
| `docs/TRIAGE.md` | labels, milestones, triage conventions |
| `docs/DESKTOP_ARCHITECTURE.md` | desktop application decisions and boundaries |

### Guides (informative)

| Document | Scope |
| --- | --- |
| `docs/ARCHITECTURE.md` | layer boundaries, transports, host services, scheduling, persistence |
| `docs/DEVELOPMENT.md` | setup, commands, packaging, validation ladder |
| `CONTRIBUTING.md` (repo root) | contribution rules and workflow |
| `README.md` (repo root) | project overview and entry points |

### Device notes

| Document | Scope |
| --- | --- |
| `docs/devices/HT08.md` | QCY MeloBuds Pro / HT08 hardware + protocol notes and open questions |

New devices use `docs/templates/device-notes.template.md`.

### Derived

| Document | Source of truth |
| --- | --- |
| `docs/SUPPORTED_DEVICES.md` | capability truth in code (`src/lib/qcy/device/capabilities.ts`, evidence ledger `src/lib/qcy/protocol/evidence.ts`, native policy `native/crates/qcy-transport/src/policy.rs`) |
| `CHANGELOG.md` (repo root) | merged behavior changes, per `docs/GOVERNANCE.md` §4 |

### Research and decisions

| Path | Scope |
| --- | --- |
| `docs/decisions/` | decision log (ADR-style); format in `docs/decisions/README.md` |
| `docs/templates/` | templates for device notes, protocol research and decisions |

## 3. Terminology

Use these terms consistently across docs, UI strings, issues and commits:

| Term | Meaning |
| --- | --- |
| 521C | this project/application (brand; `521c` for filesystem/config identifiers, `521cctl` for the CLI) |
| HT08 | QCY MeloBuds Pro device profile (first full profile) |
| QCY protocol | the independent, reverse-engineered BLE vendor protocol described in `docs/PROTOCOL.md` |
| capability truth | the four independent truths: hardware / protocol / implementation / write (issue #3 model) |
| evidence class | provenance of a protocol fact: `protocol-doc`, `hardware-capture`, `community-catalog`, `official-app` |
| trust level | writability of an opcode: `write-supported`, `write-experimental`, `read`, `catalog-only`, `destructive` |
| host-side feature | Linux host integration (MPRIS, codec status, system EQ, Auto Game Mode) — never an earbud capability |
| mock | deterministic development backend, always visibly labelled; never presented as hardware |
| experimental | session opt-in behavior, never persisted |
| preflight | interactive confirmation gate (Find Earbuds) |

Do not use "supported" for a feature that is only mock-implemented or only
protocol-known; use the readiness legend in `docs/SUPPORTED_DEVICES.md`.

## 4. Keeping docs synchronized

Rules:

1. A PR that changes behavior or protocol knowledge updates the affected docs
   in the same PR (`AGENTS.md` §8).
2. `docs/SUPPORTED_DEVICES.md` is updated whenever capability truth changes:
   the table must stay derivable from the code truth model; when in doubt,
   downgrade the claim (honest unknown beats inflated support).
3. `docs/PROTOCOL.md` and the evidence ledger change together; a new trusted
   write requires evidence recorded in the ledger first.
4. Never copy large sections between README and `docs/`; link to the canonical
   document instead.
5. Docs-only PRs still require review that no claim exceeds shipped behavior.

## 5. Documentation review checklist

Lightweight step for maintainers/CI on every PR touching `docs/`, `README.md`,
capability tables or user-visible strings:

- [ ] every touched top-level doc still declares its role marker;
- [ ] no normative claim contradicts a higher-authority document;
- [ ] capability/support statements match the code truth model;
- [ ] protocol statements carry evidence wording (class + trust level), never
      invented facts;
- [ ] mock/experimental/hardware-verified states are distinguished;
- [ ] links resolve; no duplicated source of truth introduced;
- [ ] `npm run docs:check` passes.
