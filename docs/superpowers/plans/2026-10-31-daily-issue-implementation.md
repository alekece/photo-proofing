# Daily Issue Implementation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Two gh-aw workflows: a daily read-only selector that dispatches one worker run per eligible issue, and a worker that takes the issue through plan, implementation, fresh review and PR.

**Architecture:** `daily-issue-selection.md` computes eligibility deterministically in a `steps:` pre-step (`gh api`/`gh issue`), the agent only confirms and dispatches via `safe-outputs.dispatch-workflow`. `implement-issue.md` is `workflow_dispatch` with `issue_number`, writes only through safe-outputs, and reviews with an inline sub-agent.

**Tech Stack:** gh-aw v1.0.1 (`gh aw compile --strict`), copilot engine, GitHub Actions, bash + `gh` + `jq`.

**Spec:** `docs/superpowers/specs/2026-10-31-daily-issue-implementation-design.md` (issue #105)

## Global Constraints

- Frontmatter style copies `.github/workflows/specification-alignment-review.md`: `engine: copilot`, `strict: true`, safe-outputs only for writes.
- Defaults: X = 2, cron `0 9 * * *` with `timezone: Europe/Paris`, assignee `alekece`; all in frontmatter/inputs.
- Eligible issue: no `needs-attention`/`blocked` label; a design spec link (`docs/superpowers/specs/*-design.md` on a branch) that resolves; no open blocker (GitHub "blocked by" wins over "Dependencies" body text; transitive; an eligible blocker is chosen instead and counts toward X); no open PR for it.
- Labels: clean PR `agent` + `needs-review`; red-flag PR is draft with `agent`, `blocked`, `needs-attention` (never `needs-review`) and the issue gets `blocked` + `needs-attention`.
- No auto-merge or approval. Worker merges `main` into the design branch (no rebase, no force-push); PR head keeps the design commits.
- Worker follows `AGENTS.md`; plan goes in `docs/superpowers/plans/`; `report-failure-as-issue` stays on in the worker.
- Reviewer sub-agent has fresh context; material findings fixed once; a second failing review is a red flag.
- Postgres-dependent tests are left to PR CI; a missing database is not a red flag.
- PR CI needs secret `GH_AW_CI_TRIGGER_TOKEN` (GitHub App token; `APP_ID`, `APP_PRIVATE_KEY`); document it as setup.

## Review Focus

- Issue whose spec link points to a deleted/unknown branch or file: skipped, not dispatched.
- Blocker cycle (A blocks B blocks A): neither is selected, no infinite loop.
- Issue with an open PR or existing `feature/<n>-*` branch with a PR: never dispatched twice.
- Fewer than X eligible issues (including zero): run succeeds and dispatches nothing.
- Blocker already closed in body text but "blocked by" still open (or vice versa): GitHub relationship wins.

---

### Task 1: Selector workflow

**Files:**
- Create: `.github/workflows/daily-issue-selection.md`, `.github/scripts/select-issues.sh`, generated `.github/workflows/daily-issue-selection.lock.yml`

**Interfaces:**
- Produces: `select-issues.sh <max> ` printing a JSON array of issue numbers (length ≤ max, blockers first), using `gh` with `GH_TOKEN`; consumed by the workflow pre-step which writes it to `/tmp/gh-aw/agent/selected.json`. Dispatch input name is `issue_number` (string) for workflow `implement-issue`.

- [ ] **Step 1: Write `select-issues.sh` against a fixture mode.** With `SELECT_FIXTURE=<dir>` the script reads `issues.json`, `blockers.json`, `prs.json`, `branches.txt` from that dir instead of calling `gh`, so it is testable offline. Add `.github/scripts/select-issues.test.sh` with fixtures covering each Review Focus line and each eligibility rule; assert the exact output array.
- [ ] **Step 2: Run `bash .github/scripts/select-issues.test.sh`; expect FAIL (script missing logic).**
- [ ] **Step 3: Implement the script.** Live mode: `gh issue list --state open --json number,title,body,labels`, `gh api repos/:owner/:repo/issues/<n>/dependencies/blocked_by` per issue (fall back to `#N` under a `Dependencies` heading in the body when the API is empty/404), `gh pr list --state open --json number,body,headRefName`, `gh api repos/:owner/:repo/branches/<branch>` for the spec branch. Spec link is parsed from the issue body References as `…/blob/<branch>/docs/superpowers/specs/<file>-design.md` or `branch:path`.
- [ ] **Step 4: Run the test script; expect PASS.**
- [ ] **Step 5: Write `daily-issue-selection.md`.** Frontmatter: `schedule` (`0 9 * * *`, Europe/Paris) + `workflow_dispatch`; `permissions: contents: read, issues: read, pull-requests: read, actions: read, copilot-requests: write`; `steps:` runs the script with `MAX=2` and `GH_TOKEN`; `safe-outputs.dispatch-workflow: {workflows: [implement-issue], max: 2}`; no other writes. Body: read `selected.json`, dispatch `implement-issue` once per number with `issue_number`; if empty, finish with no output.
- [ ] **Step 6: Run `gh aw compile --strict .github/workflows/daily-issue-selection.md`; expect 0 errors, lock file generated.**
- [ ] **Step 7: Commit** (`feat: add daily issue selection workflow`, with the Co-authored-by trailer).

### Task 2: Worker workflow

**Files:**
- Create: `.github/workflows/implement-issue.md`, generated `.github/workflows/implement-issue.lock.yml`

**Interfaces:**
- Consumes: dispatch input `issue_number` (string) from Task 1.
- Produces: a PR per the Global Constraints; issue comment linking it.

- [ ] **Step 1: Write the frontmatter.** `workflow_dispatch` with required input `issue_number`; `permissions` read-only plus `copilot-requests: write`; `engine: copilot`, `strict: true`; `network.allowed: [defaults, rust, node]`; `tools.bash` allowlist `cargo:*`, `pnpm:*`, `git:*`; `concurrency` group `implement-issue-${{ inputs.issue_number }}` without cancel; safe-outputs: `create-pull-request` (draft policy allowing both modes, `labels`/`allowed-labels` of `agent`, `needs-review`, `blocked`, `needs-attention`, assignee `alekece`), `add-labels`/`remove-labels` and `add-comment` on the issue (allowed: `blocked`, `needs-attention`), `report-failure-as-issue` default on; `github-token` from `GH_AW_CI_TRIGGER_TOKEN`.
- [ ] **Step 2: Write the prompt body.** Steps: read `AGENTS.md`, the issue and its linked spec; verify the spec resolves (else comment and stop); check out/mirror the design branch and merge `main`; write the plan to `docs/superpowers/plans/`; implement per plan with TDD; run `cargo +nightly fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked` (skip DB tests) and `pnpm check`/`pnpm test` in `web/` when touched; invoke `## agent: reviewer` inline sub-agent with fresh context for whole-diff, spec-compliance review; fix material findings once; re-review; then open the PR. Define red flag exactly: unresolved critical/major finding, spec contradiction or scope gap, lint/tests that cannot pass, second failed review. Clean: ready PR. Red flag: draft PR, review comments/summary of problems, labels per constraints, issue comment.
- [ ] **Step 3: Add the `## agent: reviewer` section** with read-only tools and a fixed output format (`VERDICT: clean|findings`, findings with severity critical/major/minor).
- [ ] **Step 4: Run `gh aw compile --strict`; expect 0 errors for both workflows.**
- [ ] **Step 5: Commit** (`feat: add implement-issue worker workflow`).

### Task 3: Docs and setup

**Files:**
- Modify: `README.md` (workflow list/setup section), `docs/superpowers/specs/2026-10-31-daily-issue-implementation-design.md` (resolve "Open for the builder" with the chosen mechanisms)

- [ ] **Step 1: Document in README** the two workflows, how to trigger manually (`gh workflow run daily-issue-selection`), eligibility in two lines, and the secret setup: create/install the GitHub App, add `APP_ID`, `APP_PRIVATE_KEY`, and `GH_AW_CI_TRIGGER_TOKEN`; keep claims consistent with the code.
- [ ] **Step 2: Record the branch/PR mechanism and blocker-reading approach in the spec.**
- [ ] **Step 3: Verify** `gh aw compile --strict` is clean and `git status` shows lock files committed; manually `gh workflow run daily-issue-selection` is not possible before merge, so state that first live run happens after merge and the first worker run should be watched.
- [ ] **Step 4: Commit** (`docs: document issue implementation workflows`).
