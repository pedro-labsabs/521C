---
name: speckit
description: Use when the user asks for a Spec Kit (speckit) spec-driven development step, such as establishing or updating the project constitution, creating a feature specification, clarifying requirements, planning, generating tasks or checklists, analyzing consistency, implementing tasks, converging the codebase, or converting tasks to GitHub issues.
allowed-tools: bash, read, write, edit, apply_patch, agentgrep, todo, open
---

# Spec Kit (speckit) workflow

521C uses GitHub Spec Kit for spec-driven development. The canonical command
files live in `.jcode/speckit/commands/` and are maintained by the Specify CLI
(do not hand-edit them; use `specify integration upgrade` to refresh). This
skill is the Jcode entry point: run the matching command file for the step the
user requests.

## Command map

| Step | Command file | When to use |
| --- | --- | --- |
| Constitution | `.jcode/speckit/commands/speckit.constitution.md` | Establish or update project principles |
| Spec | `.jcode/speckit/commands/speckit.specify.md` | Create or update the feature specification |
| Clarify | `.jcode/speckit/commands/speckit.clarify.md` | Ask targeted questions to de-risk ambiguous spec areas before planning |
| Plan | `.jcode/speckit/commands/speckit.plan.md` | Generate the implementation plan artifact |
| Checklist | `.jcode/speckit/commands/speckit.checklist.md` | Generate requirements-quality checklists after planning |
| Tasks | `.jcode/speckit/commands/speckit.tasks.md` | Generate dependency-ordered actionable tasks |
| Analyze | `.jcode/speckit/commands/speckit.analyze.md` | Cross-artifact consistency and alignment report after tasks |
| Implement | `.jcode/speckit/commands/speckit.implement.md` | Execute the planned tasks |
| Converge | `.jcode/speckit/commands/speckit.converge.md` | Assess the codebase against spec/plan/tasks and append remaining work |
| Tasks to issues | `.jcode/speckit/commands/speckit.taskstoissues.md` | Convert tasks to GitHub issues |

## How to run a step

1. Read `.jcode/speckit/commands/<step>.md`.
2. Substitute the user's request for `$ARGUMENTS`.
3. Follow the command file exactly: pre-execution hook checks, template usage
   from `.specify/templates/`, helper scripts from `.specify/scripts/`, and the
   project constitution in `.specify/memory/constitution.md`.
4. Write artifacts to the location the command file specifies; the CLI tracks
   the current feature via `.specify/feature.json` (local state, not committed).
5. When a command declares `handoffs`, propose the next step to the user or
   continue into it when the user already asked for the full cycle.

## Constraints

- Execute every step under the repository contract: read `AGENTS.md`,
  `.jcode/prompt-overlay.md`, `docs/HOST_SAFETY.md` and the relevant docs
  before generating spec, plan, or task content.
- Spec Kit artifacts are documentation and planning state. They never
  authorize BLE writes and never bypass the central write policy, the
  capability truth model, or destructive-command safety rules.
- Keep the Spec Kit surface intact: `.specify/` (project infrastructure) and
  `.jcode/speckit/` (CLI-managed commands) are the only Spec Kit locations.
  Skills stay exclusively in `.jcode/skills/`; do not create skills elsewhere
  (`.claude/skills`, `.agents/skills`, `.cursor`, etc.).