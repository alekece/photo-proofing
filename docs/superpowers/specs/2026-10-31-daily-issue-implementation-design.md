# Daily autonomous issue implementation (gh-aw)

Status: approved. Issue: #105.

## Intent

The maintainer designs issues (brainstorm, written design, approval) but implementing each one by hand is the bottleneck. Every morning a workflow should pick up issues that are already designed and take them through the repository's own delivery flow (plan, implementation, independent review, PR), so the maintainer's morning work is reviewing PRs, not starting them.

## Goals

- Daily, pick up to X ready issues, never ones that are not designed or are blocked.
- Run the full repo flow per issue and end with a PR that the maintainer reviews; nothing merges without them.
- When the run hits something seriously wrong, stop cleanly and flag it: `blocked` + `needs-attention` on the issue and the PR, with PR threads/comment explaining the problems.
- Failures stay local to one issue.

## Non-goals / anti-goals

- No auto-merge, no auto-approval, no design work: an issue without an approved design is never started.
- Not a replacement for brainstorming; the agent never invents scope beyond the linked design.
- Anti-goal: PRs that look done but hide a reviewer's red flag; a run that silently drops an issue.
- Not a general backlog groomer.

## Constraints

- Default branch is `main`; runs are on GitHub Actions via gh-aw (copilot engine, `strict`), safe-outputs only for writes, matching the existing `specification-alignment-review` workflow.
- `AGENTS.md` flow applies: design in `docs/superpowers/specs/`, plan in `docs/superpowers/plans/` on the working branch (removed before final merge), branch `feature/<issue>-<name>`, PR references the issue, assigned to the maintainer, fresh-agent review before delivery, squash merge by the maintainer only.
- A scheduled run sees `main`; designs live on feature branches until the implementation PR merges them (maintainer's decision), so the issue links its spec on that branch.
- CI must run on agent-created PRs, which the default `GITHUB_TOKEN` does not trigger.
- Labels that exist: `agent`, `needs-attention`, `blocked`, `needs-review`.

## Eligibility (decided with the maintainer)

An issue is eligible when ALL hold:
1. It has neither `needs-attention` nor `blocked`. (`needs-attention` is removed by the maintainer once the design is approved and linked.)
2. Its References link an existing design spec file on a branch (`docs/superpowers/specs/*-design.md`); the file must resolve.
3. No open blocker. A blocked issue is not started; if its blocker is itself eligible, the blocker is selected instead.
4. It has no open linked PR already.

Up to X eligible issues are selected per run. Blockers chosen this way count toward X.

## Approaches considered

1. One scheduled workflow does everything for all issues in sequence: simple, but long runs, one failure starves others, shared budget.
2. **Selector + one worker run per issue (chosen, with the maintainer).** A read-only scheduled selector computes eligibility and dispatches one worker run per chosen issue; the worker alone writes.
3. Hand issues to the Copilot coding agent: least to build, but no control over the independent reviewer or red-flag labelling.

## Design (as decided)

- **Selector** (`daily-issue-selection.md`): `schedule` daily plus `workflow_dispatch`; read-only permissions; reads open issues, labels, bodies, blockers, linked PRs; applies Eligibility; uses `safe-outputs.dispatch-workflow` (max = X) to start `implement-issue` once per selected issue with input `issue_number`. Posts nothing else.
- **Worker** (`implement-issue.md`): `workflow_dispatch` with input `issue_number`; checks out the issue's design branch; reads `AGENTS.md`, the issue and its design; rebases the branch on `main`; writes the plan under `docs/superpowers/plans/`; implements per the plan; runs `cargo fmt/clippy` and `pnpm check/test` where possible (network allowed for `rust` and `node` ecosystems); invokes an inline reviewer sub-agent (fresh context, not the author) for a whole-diff review; fixes material findings once; opens the PR (assigned to the maintainer, references the issue, `agent` + `needs-review` labels) via `create-pull-request`.
- **Red flag:** if the reviewer reports a critical/major problem the worker cannot resolve, a scope or design contradiction, or tests/lint that cannot pass, the worker still opens the PR (draft), adds `blocked` + `needs-attention` to both the issue and the PR, and posts the problems as review threads/comment. It never marks such a PR ready.
- **CI on the PR:** a repo secret `GH_AW_CI_TRIGGER_TOKEN` (GitHub App/PAT) is used so PR CI runs.
- **Observability:** each run ends with a short issue comment linking the PR (or explaining why nothing was done).

## Calls made by the author (change if you disagree)

- **Branch:** the issue's spec link names the design branch; the worker works on that branch and the PR's head must contain the design commits. It merges `main` into the branch (no rebase, no force-push). How `create-pull-request` is made to use or mirror that branch (for example a new `feature/<issue>-<name>` branch cut from the design branch tip) is the builder's call; the PR must not lose the design commits.
- **Blockers:** the GitHub "blocked by" relationship wins over "Dependencies" text; a blocker stays open until its issue is closed; chains resolve transitively; a blocked issue is started only when every blocker in its chain is eligible, otherwise it is skipped. Eligibility and blocker computation should be deterministic (`gh api` in a pre-step), not LLM judgement.
- **Idempotency:** an issue with any open PR (or an open branch for it) is ineligible. After a red-flag run it stays excluded until the maintainer closes the PR or clears the labels; reruns never reuse or duplicate a PR on their own.
- **Labels:** a clean PR gets `agent` + `needs-review`; a red-flag PR is a draft and gets `agent`, `blocked`, `needs-attention` (never `needs-review`), and the issue gets `blocked` + `needs-attention`.
- **Defaults:** X = 2, daily at 09:00 Europe/Paris (after the 08:00 alignment review), assignee `alekece`, all configurable in frontmatter.
- **Failures:** worker `report-failure-as-issue` stays on, so a timeout or crash before the PR raises an `agent` + `needs-attention` issue; the selector does not watch for dropped runs.
- **Review:** the inline reviewer sub-agent (fresh context) satisfies the repo's "fresh agent" rule. Material findings are fixed once; if a second review still fails, it is a red flag.
- **Tests:** the worker runs fmt/clippy and `pnpm check/test` where it can; tests needing Postgres are left to the PR's CI. A missing database is not a red flag; a failing PR CI is reported by CI itself.
- **Plan file:** the worker commits it on the branch; the maintainer has the plan removed before final merge, per AGENTS.md.
- **Prerequisite for you:** the repository needs the `GH_AW_CI_TRIGGER_TOKEN` secret (GitHub App or PAT) so CI runs on agent PRs.

## Open for the builder

- Reviewer criteria wording for "red flag" vs minor, and budgets (`max-turns`, credits, timeouts), tuned on the first runs.
- **Branch/PR mechanism (chosen):** the worker checks out the design branch, merges `origin/main` with a normal merge commit (never rebase, never force-push), implements and commits there, then opens `create-pull-request` with `base-branch: main`, `patch-format: bundle`, `signed-commits: false`, and `preserve-branch-name: true` targeting `feature/<issue>-<name>`. Bundle format mirrors the whole `main..HEAD` diff range — design commits, the main-merge, and the implementation — onto that branch, so the PR head keeps the design commits; `signed-commits: false` is required because the alternative GraphQL signed-commit path drops merge commits. `draft` tracks outcome at creation time (`false` when clean, `true` on a red flag), backed by a config default of `draft: true` plus `mark-pull-request-as-ready-for-review` gated on the `needs-review` label so a red-flag draft can never be promoted.
- **Blocker reading (chosen):** a deterministic pre-step script (`.github/scripts/select-issues.sh`, bash + `jq`, no LLM judgement) calls `gh issue list`, `gh api repos/:owner/:repo/issues/<n>/dependencies/blocked_by` per issue, and `gh pr list`/`gh api .../branches` to resolve each issue's spec link. GitHub's `blocked_by` relationship wins over a body `## Dependencies` section, but only when GitHub reports no blockers at all (so an empty/404 response falls back to the body text). Blocker resolution is transitive and cycle-safe: a blocked issue is replaced by its resolved blockers when every blocker in the chain is itself eligible, otherwise the whole chain is skipped; a blocker not itself in the open-issues list (closed or unknown) does not block.
