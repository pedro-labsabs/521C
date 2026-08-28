# 521C project skills

This directory is the **only skills surface** in the repository (see
`.jcode/prompt-overlay.md` → Skills surface). Jcode loads each skill from
`.jcode/skills/<name>/SKILL.md`.

## Inventory

| Skill | Source | Covers | Installs (skills.sh, 2026-08) |
| --- | --- | --- | --- |
| `521c-appimage` | authored (project) | AppImage packaging, desktop/AppStream metadata | — |
| `521c-bluez` | authored (project) | BlueZ/D-Bus transport, write policy, host safety | — |
| `521c-protocol` | authored (project) | protocol evidence, conformance vectors, capability truth | — |
| `521c-slint` | authored (project) | Slint desktop UI, typed state boundary | — |
| `brainstorming` | obra/superpowers | requirements/design before implementation | 200K+ |
| `code-review` | mattpocock/skills | multi-axis code review | 436.8K |
| `executing-plans` | obra/superpowers | executing written plans with checkpoints | 200K+ |
| `find-skills` | vercel-labs/skills | discover/install ecosystem skills | 13.3K stars |
| `finishing-a-development-branch` | obra/superpowers | integrate completed branches | 100K+ |
| `github-actions-templates` | wshobson/agents | GitHub Actions CI workflows | 14.8K |
| `receiving-code-review` | obra/superpowers | respond to review feedback with rigor | 179.1K |
| `requesting-code-review` | obra/superpowers | request review before merge | 212.7K |
| `rust-async-patterns` | wshobson/agents | async Rust patterns (zbus/async) | 17.7K |
| `rust-best-practices` | apollographql/skills | Rust conventions and best practices | 16.6K |
| `security-review` | getsentry/skills | security review of changes | 14.8K |
| `speckit` | authored (project) + via `.specify/` | Spec Kit (specify CLI) workflow | — |
| `subagent-driven-development` | obra/superpowers | executing plans with subagents | 100K+ |
| `systematic-debugging` | obra/superpowers | root-cause debugging workflow | 240.3K |
| `test-driven-development` | obra/superpowers | TDD before implementation | 200K+ |
| `vercel-react-best-practices` | vercel-labs/agent-skills | React/Next.js performance (web surface) | 672.6K |
| `webapp-testing` | anthropics/skills | web application testing | 144.3K |
| `writing-documentation-with-diataxis` | sammcj/agentic-coding | structured documentation (Diátaxis) | 578 |
| `writing-plans` | obra/superpowers | implementation plans from specs | 200K+ |

## How to update

External skills are copied from their upstream repositories (via
`npx skills add <owner/repo> -s <skill> -y --copy` into a scratch directory,
then copied into `.jcode/skills/`). Do **not** run `npx skills update` inside
this repository: it would install into `.agents/skills`/`.hermes/skills`,
creating other agent surfaces. To update, re-fetch the upstream skill and
replace the folder, then open a PR.

Provenance rule: never hand-edit upstream skill content; if a skill needs
project-specific adjustments, keep them in the authored `521c-*` skills or in
`.jcode/prompt-overlay.md` instead.