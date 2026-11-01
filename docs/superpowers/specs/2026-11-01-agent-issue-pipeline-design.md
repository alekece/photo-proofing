# Agent issue pipeline

Issue: #125. Supersedes the removed `daily-issue-selection` / `implement-issue` workflows.

## Goal

Implement issues labelled `agent` that have a committed design, a bounded number per day, as draft PRs that automation shepherds to green CI. Automation never merges and never writes designs.

## Components

- `pick-issues.yml` (plain workflow, no LLM, so not a gh-aw source): daily cron plus `workflow_dispatch` with optional `max`. Limit = `max` input, else repo variable `AGENT_MAX_ISSUES`, else 1; 0 pauses. Dispatches `implement-issue` once per picked issue.
- The picker is one inline step doing only cheap filters (`agent` label, not `needs-attention`/`blocked`, no open sub-issues or blockers, no open PR), ordered by milestone due date then oldest. Design presence and branch cleanliness are checked only by `implement-issue`, so a picked issue may end as a no-op run. Project-board priority is not supported.
- `implement-issue.md` (gh-aw, Copilot engine, read-only agent job):
  - Phase 1 (`workflow_dispatch`, input `issue`): pre-step finds the design branch by number and switches to `<design-branch>-impl`; the agent implements the design, runs the CI checks locally (at most 3 fix rounds), revalidates just before output, and opens a draft PR via `create-pull-request`.
  - Phase 2 (`workflow_run` of `CI` on `*/*-impl` branches): green marks the PR ready; red is fixed and pushed (at most 3 rounds, counted by `CI fix round` comments); still red leaves the PR a draft with an explanatory comment. No labels.
  - Concurrency group per issue/branch, queued, never cancelled. All writes go through safe outputs minted with the repository's GitHub App, so PR and pushes trigger CI.

## Decisions and limits

- "No existing branch" is read as "no branch other than the design branch": the design lives on `<type>/<N>-slug`, so an implementation branch (`...-impl`) existing makes the issue ineligible (no double pickup, also after a closed PR).
- The agent job cannot see CI of a PR created by its own safe outputs, hence the `workflow_run` second phase. Tests needing PostgreSQL cannot run in the agent job; CI covers them in Phase 2.
- Design staleness beyond "design branch changed outside docs" is judged by the agent (design vs. issue vs. code) and ends in a `noop`.
- `docs/superpowers/specs/` is not excluded from the PR patch because the design commits are already on the branch; the prompt forbids edits to designs.
- Verified: a PR opened with the App identity triggers CI.
