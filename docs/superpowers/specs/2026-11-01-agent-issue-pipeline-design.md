# Agent issue pipeline

Issue: #125. Supersedes the removed `daily-issue-selection` / `implement-issue` workflows.

## Goal

Implement issues labelled `agent` that have a committed design, a bounded number per day, as draft PRs that automation shepherds to green CI. Automation never merges and never writes designs.

## Components

- `pick-issues.yml` (plain workflow, no LLM, so not a gh-aw source): daily cron plus `workflow_dispatch` with optional `max`. Limit = `max` input, else repo variable `AGENT_MAX_ISSUES`, else 1; 0 pauses. Runs `.github/scripts/pick-issues.sh`, then dispatches `implement-issue` once per picked issue.
- `.github/scripts/issue-eligible.sh <N>`: the single eligibility rule set, shared by the picker and by `implement-issue`'s pre-step. Open; `agent`; no `needs-attention`/`blocked`; no open "blocked by"; no open sub-issues; no open PR; exactly one `<type>/<N>-*` branch (the design branch) whose only changes against the default branch are under `docs/superpowers/` and include a spec. Branches are found by issue number only, never from issue text.
- Ordering (`pick-issues.sh`): dated milestone by due date, undated milestone, no milestone; then oldest first. Project-board priority is not supported because `GITHUB_TOKEN` cannot read Projects.
- `implement-issue.md` (gh-aw, Copilot engine, read-only agent job):
  - Phase 1 (`workflow_dispatch`, input `issue`): pre-step re-runs eligibility and switches to `<design-branch>-impl`; the agent implements the design, runs the CI checks locally (at most 3 fix rounds), revalidates just before output, and opens a draft PR via `create-pull-request`.
  - Phase 2 (`workflow_run` of `CI` on `*/*-impl` branches): green marks the PR ready; red is fixed and pushed (at most 3 rounds, counted by `CI fix round` comments); still red leaves the PR a draft with an explanatory comment. No labels.
  - Concurrency group per issue/branch, queued, never cancelled. All writes go through safe outputs minted with the repository's GitHub App, so PR and pushes trigger CI.

## Decisions and limits

- "No existing branch" is read as "no branch other than the design branch": the design lives on `<type>/<N>-slug`, so an implementation branch (`...-impl`) existing makes the issue ineligible (no double pickup, also after a closed PR).
- The agent job cannot see CI of a PR created by its own safe outputs, hence the `workflow_run` second phase. Tests needing PostgreSQL cannot run in the agent job; CI covers them in Phase 2.
- Design staleness beyond "design branch changed outside docs" is judged by the agent (design vs. issue vs. code) and ends in a `noop`.
- `docs/superpowers/specs/` is not excluded from the PR patch because the design commits are already on the branch; the prompt forbids edits to designs.
- Verified: a PR opened with the App identity triggers CI.
