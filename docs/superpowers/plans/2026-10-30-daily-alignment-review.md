# Daily Specification Alignment Review Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Run the specification alignment review daily at 08:00 Europe/Paris, label findings `agent` + `needs-attention`, and harden the prompt for daily cadence.

**Architecture:** Edit the gh-aw source `.github/workflows/specification-alignment-review.md` in place and regenerate its `.lock.yml`. No new files besides docs.

**Tech Stack:** gh-aw (`gh aw compile`), GitHub Actions.

**Spec:** `docs/superpowers/specs/2026-10-30-daily-alignment-review-design.md` (issue #75)

## Global Constraints

- Cron `0 8 * * *` with `timezone: Europe/Paris`; `workflow_dispatch` retained.
- `create-issue`: `max: 3`, `labels` and `allowed-labels` both `[agent, needs-attention]`.
- Permissions, `network`, `tools`, `engine`, authentication, and the action pin `221ba4f4319de6b8e809d1a15528b3ae6a818aa5` are unchanged.
- `needs-review` label and existing issues untouched.
- No live dispatch, no automatic merge.
- Compile command: `gh aw compile specification-alignment-review --validate --actionlint --shellcheck --action-mode action --action-tag 221ba4f4319de6b8e809d1a15528b3ae6a818aa5`

## Review Focus

- A matching *closed* issue for a finding's key: must be treated as a maintainer decision, not re-reported.
- Run finds no discrepancy: `noop`, never a summary issue.
- Duplicate searches or GitHub reads fail: `report_incomplete`, no issues created.
- Lock file out of sync with the source: caught by recompiling and checking `git diff` is empty.
- Stale wording ("weekly", "previous week", "needs-review", "Documentation and implementation audit") left behind: caught by grep.

---

### Task 1: Isolated worktree

**Files:**
- Create: worktree `.worktrees/75-daily-alignment-review` on branch `feature/75-daily-alignment-review`

- [ ] **Step 1:** Use superpowers:using-git-worktrees to create the worktree from `main`.
- [ ] **Step 2:** Move the uncommitted design and this plan from the main checkout into the worktree's `docs/superpowers/{specs,plans}/` and commit them: `git commit -m "docs: design and plan for daily alignment review (#75)"`.

### Task 2: Daily schedule, labels, and prompt

**Files:**
- Modify: `.github/workflows/specification-alignment-review.md`
- Modify (generated): `.github/workflows/specification-alignment-review.lock.yml`

- [ ] **Step 1: Schedule.** Change the cron to `"0 8 * * *"` and its comment to "Daily at 08:00 Paris time. Change this cron, then recompile this file."
- [ ] **Step 2: Labels.** Set `labels` and `allowed-labels` to `[agent, needs-attention]`; update the Output section sentence to name `agent` and `needs-attention`.
- [ ] **Step 3: Cadence wording.** Replace "not just changes from the previous week" with wording that each run reviews the full default-branch snapshot.
- [ ] **Step 4: Duplicate check (investigation item 4 and the Duplicate check bullet).** State that every candidate's `specification-alignment-review:<key>` is searched in open and closed issues and PRs, a closed match counts as a maintainer decision, and the run repeats daily so the check is mandatory before each proposal.
- [ ] **Step 5: Usage section.** Fix the workflow name to "Specification alignment review", describe the daily schedule, and name the `needs-attention` label; keep auth guidance, compile command, and the eligibility caveat.
- [ ] **Step 6: Recompile.** Run the compile command from Global Constraints. Expected: succeeds with no errors from actionlint/shellcheck.
- [ ] **Step 7: Verify.**
  - `grep -n "0 8 \* \* \*\|needs-attention" .github/workflows/specification-alignment-review.lock.yml` shows both.
  - `grep -n -i "needs-review\|previous week\|weekly\|Monday\|implementation audit" .github/workflows/specification-alignment-review.md .github/workflows/specification-alignment-review.lock.yml` returns nothing.
  - Rerun the compile command, then `git status --short` shows only the two workflow files modified.
- [ ] **Step 8: Commit.** `git add .github/workflows/specification-alignment-review.* && git commit -m "ci: run specification alignment review daily (#75)"`

### Task 3: Review and delivery

- [ ] **Step 1:** Request an independent fresh-agent review of correctness and compliance with the design; fix material findings and recompile.
- [ ] **Step 2:** Push the branch and open a PR referencing #75 with verification evidence (compile output, grep checks) and the limitation that no live dispatch was run.
- [ ] **Step 3:** After review, remove `docs/superpowers/plans/2026-10-30-daily-alignment-review.md`, then request explicit approval before a squash merge.
