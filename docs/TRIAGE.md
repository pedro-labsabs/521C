# Triage, labels and milestones

**Authority:** normative for issue/PR management. Technical authority rules stay
in `AGENTS.md`; documentation structure conventions are owned by issue #15.

## 1. Label taxonomy

Labels are the canonical machine-readable classification. Title prefixes such as
`[P1][desktop]` are kept on existing issues for readability, but labels win in
every filter, query and automation decision. New issues should carry labels
first; a matching title prefix is optional.

Four dimensions may coexist on one issue:

```
exactly one priority  +  one or more area  +  exactly one type  +  optional status
```

### Priority (exactly one)

| Label | Meaning | Color logic |
| --- | --- | --- |
| `priority/P0` | Critical: safety, data loss, or blocks everything | red |
| `priority/P1` | High: required for the first release | orange |
| `priority/P2` | Medium: valuable, not release-critical | yellow |
| `priority/P3` | Low: nice to have | gray |

### Area (one or more)

| Label | Covers |
| --- | --- |
| `area/protocol` | framing, codecs, evidence ledger, conformance vectors |
| `area/transport` | Web Bluetooth + native BlueZ transports, scheduling, write enforcement points |
| `area/security-safety` | central write policy, preflight gates, host safety |
| `area/native-linux` | native Rust workspace, CLI, host services |
| `area/desktop-ui` | Slint desktop application and packaging |
| `area/web-ui` | React/TanStack reference surface |
| `area/testing-ci` | tests, conformance corpus, CI, quality gates |
| `area/config-privacy` | persistence, config schema, local-first privacy |
| `area/docs` | documentation structure and content |
| `area/repo-meta` | governance, labels, milestones, process |

### Type (exactly one)

| Label | Meaning |
| --- | --- |
| `bug` | something is not working as specified |
| `enhancement` | new behavior or capability |
| `type/research` | investigation without committed implementation |
| `type/refactor` | structure change without behavior change |
| `type/maintenance` | repo upkeep: process, docs system, tooling |

### Status/workflow (optional)

| Label | Meaning |
| --- | --- |
| `status/blocked` | waiting on an external action or dependency (say which, in the issue body) |
| `status/needs-design` | needs a design decision before implementation |
| `good first issue` | bounded, safe for external newcomers (see §4) |
| `help wanted` | maintainers want outside help |
| `duplicate`, `invalid`, `wontfix`, `question` | standard GitHub triage outcomes |

Colors follow one deliberate system: priority is a red→gray severity scale,
areas are cool blues/teals/purples, types are green/purple, statuses are pale
tints. Do not add near-duplicate labels; extend a dimension instead.

## 2. Milestones

Milestones are deliverable stages from `docs/AUTONOMOUS_EXECUTION.md`, not dates.
An issue joins a milestone because of its real delivery dependencies, never
because its area name matches.

| Milestone | Issues | Complete when |
| --- | --- | --- |
| Foundation & Safety | #1, #5, #9 | central write authorization, reproducible CI and interactive safety gates are closed with evidence |
| Protocol & Validation | #3, #4, #6 | capability truth, evidence provenance and shared conformance vectors are closed with evidence |
| Native Linux Backend | #2, #7, #10, #13 | real transports, scheduling and host services are closed with evidence |
| Desktop Integration | #8, #11, #12 | native desktop app, validated persistence and local-first privacy are closed with evidence |
| Polish & Operations | #14, #15, #16 | governance, documentation system and this triage taxonomy are closed with evidence |

Issues without a milestone are general backlog until a dependency pulls them
into a stage.

## 3. Triage conventions

Apply on creation or first review:

1. one priority — default `P2` when unsure; P0/P1 require a safety or
   release-dependency justification in the body;
2. one or more areas from the architecture table;
3. one type;
4. a milestone when the dependency graph already places the issue in a stage;
5. `status/blocked` plus the exact blocking action in the body when work cannot
   start.

Special cases:

- **Duplicates:** close with the `duplicate` label and a comment linking the
  canonical issue; keep the issue with the better acceptance criteria.
- **Not planned:** close with `wontfix` and one sentence of rationale
  (out-of-scope per `docs/PRODUCT_SPEC.md` §7 is a valid rationale).
- **Protocol research:** use the `protocol_research` issue template,
  `type/research` + `area/protocol`. Research issues never assert writable
  protocol facts; evidence rules stay in `docs/PROTOCOL.md`.
- **Follow-ups found in review:** open a new bounded issue referencing the PR;
  do not silently expand the current issue's scope (`AGENTS.md` §8).
- **Hardware-blocked work:** keep it open with `status/blocked` and the exact
  physical action needed; finish all independent work first
  (`docs/AUTONOMOUS_EXECUTION.md` §10).

## 4. Contributor labels

`good first issue` requires all of: bounded scope, no protocol-evidence
judgment needed, no safety-policy or host-mutation surface, and a passing
local validation ladder describable in the issue. `help wanted` marks work the
maintainers will not immediately pick up themselves. Coding agents working the
autonomous program are not "external contributors"; they follow `AGENTS.md`.

## 5. Backlog grooming

Lightweight cadence: at the start of each delivery session (and at least when
a milestone completes):

- [ ] every open issue has priority + area + type labels;
- [ ] blocked issues name their exact blocker;
- [ ] milestones reflect the dependency graph, not wishful ordering;
- [ ] closed issues were verified against their acceptance criteria;
- [ ] no orphan labels (labels used by zero issues/PRs get removed or merged).

## 6. Templates

Issue templates live in `.github/ISSUE_TEMPLATE/` (bug, feature, protocol
research). The PR template requires evidence, safety and hardware-verification
sections; labels on PRs should mirror the issue they close.
