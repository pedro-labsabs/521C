# Governance and maintenance

**Authority:** normative for repository maintenance, review, merge and release
process. Technical contract stays in `AGENTS.md`; triage labels/milestones in
`docs/TRIAGE.md`; product scope in `docs/PRODUCT_SPEC.md`.

## 1. Maintainers

521C is currently maintained by the repository owner (`@pedro-labsabs`).
`CODEOWNERS` (`.github/CODEOWNERS`) marks the protocol-truth and safety
surfaces as requiring the most careful review. When new maintainers join,
ownership is expanded there first, then in this document.

## 2. Contribution boundaries

Changes fall into four classes; each has its own review focus on top of the
shared rules in `CONTRIBUTING.md`:

| Class | Paths (typical) | Extra review focus |
| --- | --- | --- |
| Protocol | `src/lib/qcy/protocol/`, `native/crates/qcy-protocol/`, `conformance/`, `docs/PROTOCOL.md`, evidence ledger | provenance of every byte-level claim; conformance vector added **before** codec/write changes; no invented UUIDs/opcodes; trust levels only raised with real evidence |
| Linux backend | `native/crates/qcy-transport/`, `qcy-host/`, `qcy-app/`, `521cctl` | D-Bus boundaries stay behind traits with fake-bus tests; central write policy untouched or strengthened, never bypassed; no daemon replacement, no root |
| Frontend/UI | `src/components/`, `native/crates/521c-desktop/ui/` | consumes typed state/capability truth only; no raw GATT bytes; disabled/unknown states derived from the truth model, not hardcoded |
| Docs-only | `docs/`, `README.md`, `CONTRIBUTING.md` | no behavior claims beyond shipped code; support/evidence matrices stay synchronized with the code they describe |

Cross-class changes (e.g. a new write path touches protocol + backend + UI)
must satisfy every relevant class and are reviewed as one coherent PR.

## 3. Review and merge standards

Every PR is reviewed against this checklist before merge:

- [ ] links the issue(s) it closes, or states a bounded objective;
- [ ] acceptance criteria demonstrably satisfied (not "looks complete");
- [ ] validation ladder actually executed and reported (`just check` or the
      explicit commands in `docs/DEVELOPMENT.md`);
- [ ] protocol changes carry evidence/provenance; capability/truth labels not
      inflated;
- [ ] safety impact assessed: write-policy reachability, destructive-opcode
      unreachability, preflight gates, host mutations, network behavior;
- [ ] hardware-verified behavior explicitly distinguished from mock/fake
      behavior;
- [ ] no secrets, private captures, generated output or unrelated churn in the
      diff;
- [ ] docs/evidence matrices updated where behavior or knowledge changed;
- [ ] CI green. If CI is blocked for an external reason, the blocker is named
      in the PR and local validation evidence is posted instead.

Merge rules:

- branch → coherent commits → PR → CI → review → merge (squash or merge
  commit, keeping the issue reference);
- do not merge known-red work; do not bypass required checks;
- do not rewrite public history or force-push `main`;
- close issues only when their acceptance criteria are proven; record what was
  hardware-verified versus what remains hardware-dependent.

## 4. Release and versioning policy (pre-1.0)

- Version format: `0.MINOR.PATCH`.
  - MINOR bumps mark coherent delivery milestones (e.g. `0.1.0` = first
    desktop release candidate satisfying the release checklist in
    `docs/AUTONOMOUS_EXECUTION.md`);
  - PATCH bumps mark fix-only releases on top of a milestone.
- Tags: `v0.MINOR.PATCH` on `main`, only after the release checklist passes.
- Each release gets a GitHub Release containing: release notes (what shipped,
  what was hardware-verified, known limitations), and the AppImage artifact
  with its desktop metadata instructions.
- `CHANGELOG.md` at the repository root follows Keep-a-Changelog conventions
  (`Added/Changed/Fixed/Security` sections) and is updated in the same PR as
  the behavior change, not retroactively.
- Pre-1.0 there is no stability guarantee; breaking changes are noted in the
  changelog, not gated.

### Release gate enforcement (issue #41)

The enforced gate is: `Web · Node 22`, `Native · Rust stable` and
`Desktop · AppImage artifact` required on `main` before merge/tag.

**Current state (restored 2026-08-26):** the repository is **public**, so the
Free-plan organization has GitHub-hosted runners and branch protection. The
three check contexts are required on `main` (strict/up-to-date, enforced for
administrators, no direct pushes — every change goes through a pull request
with green checks). A fully green remote run on `main`
(run `32971542451`, commit `98c7d60`) evidences the restored gate.

Historical limitation and contingency: while the repository was **private**
in the Free-plan organization (audited 2026-08-26), GitHub-hosted runners
required available Actions minutes (when the monthly quota is exhausted, jobs
fail before any step runs with the annotation "recent account payments have
failed or your spending limit needs to be increased"), and branch protection
with required status checks was unavailable for private repositories on the
Free plan (API HTTP 403 "Upgrade to GitHub Pro or make this repository
public"). If the repository ever becomes private again on a Free plan, the
enforced workaround below applies:

1. every PR runs the full local validation ladder from a clean checkout and
   posts the exact commands and results in the PR (see §3);
2. every release carries a **release gate audit record** in its notes that
   lists each gate as PASSED (with evidence) or NOT STARTED/SKIPPED (with the
   external reason) — a skipped or not-started CI job is never presented as a
   passed gate;
3. the final release tag for a milestone waits on a green remote CI run
   whenever remote runners are available; while they are not, the tag is
   deferred and the blocker is named in the release notes.

Owner action required to restore the full gate (any one): make the
repository public (unlimited free minutes + branch protection), or upgrade
the org plan / add a payment method and spending limit, or register an
authorized self-hosted runner. Owner: the `pedro-labsabs` organization
owner.

## 5. Issue triage

Triage labels, milestones and conventions are defined in `docs/TRIAGE.md`.
Maintainers run the lightweight grooming checklist there at the start of each
delivery session and when a milestone completes.

## 6. Coding agents and repository policy

Jcode and any delegated autonomous agents operate under `AGENTS.md`,
`JCODE_AGENT_START.md` and `.jcode/prompt-overlay.md`. In short:

- agents may make ordinary engineering decisions delegated by those contracts;
- agents must not invent protocol facts, weaken tests/safety/truth labels, or
  exceed the host-safety boundary (`docs/HOST_SAFETY.md`);
- agent work follows the same review/merge standards as human work: agent
  claims are evidence inputs, and the integrating maintainer verifies diffs,
  tests and acceptance criteria before merge;
- agents close issues only with demonstrable evidence, and must keep GitHub
  state (issues, PRs, CI) truthful.

## 7. Community health files

- `CONTRIBUTING.md` — contribution rules and workflow (this document is linked
  from it);
- `SECURITY.md` — security reporting, including BLE-safety bugs;
- `LICENSE` — MIT;
- issue/PR templates in `.github/` enforce evidence, safety and
  hardware-verification sections.

Deliberately absent for now: a code of conduct and discussion forums — the
project is single-maintainer with agent-heavy execution; revisit when external
contributions begin.
