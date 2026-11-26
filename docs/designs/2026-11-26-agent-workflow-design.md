# Agent workflow: skills and reviewer agents

No issue yet: the maintainer asked to start from this design.

## Why

`AGENTS.md` loads into every task, yet about a third of it is the delivery procedure (review, PR, merge) and the automation rules. Agents follow short, always-true rules well and long procedures badly, and the procedure is loaded when it is not needed. The repository is also used with Copilot, Codex and, soon, Claude Code, so the guidance must not depend on one tool.

## Goals

- `AGENTS.md` keeps only rules that apply to every task, plus one-line pointers to skills.
- Each procedure becomes a skill, loaded only when its step is reached.
- Review is done by two fresh reviewer agents that did not write the code.
- The same files work in Copilot, Codex and Claude Code, with no copies to keep in sync.

## Non-goals

- No agents per language or area (Rust, Svelte). The nested `AGENTS.md` files already load by directory.
- No change to the delivery workflow itself (brainstorm, design, plan, implement, review, PR, merge), to the `REVIEW.md` rules, or to the risk levels. The one exception is when checks run (see Checks).
- No Codex (`.codex/agents/*.toml`) or Claude (`.claude/agents/*.md`) agent files yet. Add them when those tools are used for review.

## Approaches

No single skills folder is read by all three tools: Claude Code reads `.claude/skills/`, Codex reads `.agents/skills/`, and Copilot reads `.github/skills/`, `.claude/skills/` and `.agents/skills/`.

- **A. `.agents/skills/` as the source, with a `.claude/skills` symlink.** One set of files, read by all three tools. Chosen by the maintainer.
- B. `.claude/skills/` only. Codex would not see the skills.
- C. Copies in both folders. Works everywhere but the copies drift.

The symlink is one file with two paths, not synchronization tooling.

## Design

### Skills (`.agents/skills/`, `.claude/skills` symlinked to it)

- `create-issue`: rewritten. Used when the maintainer asks to file an issue during or after brainstorming. Sets one type label (`bug`, `enhancement`, `documentation`) and the earliest open milestone (`gh api repos/{owner}/{repo}/milestones`: MVP, then v1, and so on). Issues in the MVP milestone also get `mvp`, except report issues. The body states the outcome, acceptance criteria and constraints, never a task list, and links the design when one exists. It adds `needs-attention` only when the maintainer must act.
- `review-change`: delivery step 6. Runs the checks, starts `code-reviewer` and `security-reviewer` in parallel, applies the three-round limit on blocking findings, then the refactor pass, and records the result.
- `deliver-pr`: delivery steps 7 and 8. Opens the PR with the **Review and risk** section, then merges by risk level, including the auto-merge-mode switch that `AGENTS.md` holds today.

`AGENTS.md` keeps the numbered workflow as a short outline; each step that has a skill ends with "use `<skill>`".

### Reviewer agents (`.github/agents/`)

- `code-reviewer`: correctness, edge cases, error handling, tests, maintainability, compliance with the approved design, `AGENTS.md` and `REVIEW.md`.
- `security-reviewer`: the security review.
- Both start with a fresh context, are read-only, report findings in the format of `REVIEW.md`, and state a risk level.

### `REVIEW.md` stays one file

The security section is short and the risk levels are shared, so a second file would mostly hold links. Each agent file names the sections it reads: the code reviewer reads the learned rules, "Check strictly", "Findings" and "Risk levels"; the security reviewer reads "Security review", "Findings" and "Risk levels". This is the assistant's recommendation; the maintainer may prefer a separate security file.

### Checks

- While implementing a plan task: run only the tests for the new or changed behavior, with the smallest command that covers them. No format, lint or full test run.
- Once, after the last task (start of `review-change`): format, lint and the whole test suite for the changed area, to catch regressions.
- After fixing blocking findings: rerun the checks the fix could affect, and the full set again before the PR.

`AGENTS.md` "Testing and Verification" states this rule once; `review-change` and the plan tasks follow it.

### Labels

- `agent` is removed, since only automation read it. The label is deleted from GitHub after merge.
- `needs-attention`: only when the maintainer must act, for example an incomplete issue, or a PR that waits for a human decision. It is never added to a PR just because it is open or awaiting review. The agent's latest comment states what is needed, and the label is removed once done.
- `blocked`: unchanged, an external blocker such as a dependency or another PR.

### Removals

- Skills `delegate-issue` and `resume-issue`. The three issue skills were not used in the last 30 days and nothing references them.
- Workflows `pick-issues.yml`, `implement-issue.md` with its lock file, and `daily-repo-status.md` with its lock file.
- The "Automation is opt-in" section of `AGENTS.md`.
- `agent` from the label lists of the rustsec workflows, which keep their `rustsec` label.
- The superseded designs `2026-11-01-agent-issue-pipeline-design.md` and `2026-10-31-daily-issue-implementation-design.md` get a one-line note that this design replaces them.

Stay as they are: `agentic-workflows` in `.github/skills/` (specific to GitHub), `agentics-maintenance.yml`, `copilot-setup-steps.yml` and the rustsec workflows.

### Documentation

Update the README sections that describe the workflow, the agent guidance link and the merge rules so they point to the skills. `web/AGENTS.md` and `crates/AGENTS.md` keep referring to the root workflow.

## Risk

High: the change touches `AGENTS.md`, `REVIEW.md`, `.github/` and workflows. It needs the full-depth security review and the maintainer's explicit approval.

## Left to the builder

Skill and agent wording, the exact outline left in `AGENTS.md`, and committing the symlink as a relative link.
