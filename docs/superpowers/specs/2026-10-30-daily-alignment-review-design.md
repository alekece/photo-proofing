# Daily specification alignment review

Issue: #75. Replaces the missing `2026-10-29-daily-alignment-review-design.md`
referenced by the issue; this document is the design of record.

## Intent

Maintainers want documentation, implementation, and tests checked for
contradictions every day instead of weekly, without duplicate findings,
roadmap noise, or any implementation changes by the agent.

## Goals

- Exactly one scheduled run per day at 08:00 Europe/Paris
  (`cron: "0 8 * * *"`, `timezone: Europe/Paris`); manual dispatch is retained.
- At most three novel, actionable issues per run, labeled `agent` and
  `needs-attention`; none when no useful discrepancy exists (`noop`).
- Existing behavior is preserved: deduplication against issues and PRs,
  planned-work exclusions, uncertainty reporting, commit-pinned evidence,
  issue structure, and `report_incomplete` on failed reads.
- Runtime stays read-only except `create_issue`.
- Usage instructions, the lock file, and the labels match the workflow.

## Non-goals and anti-goals

- No backpressure or `skip-if-match` gating, no new safe outputs, no
  changes to authentication, permissions, network policy, or the pinned
  action revision.
  <!-- delete `needs-review` label - it is replaced by `needs-attention` -->
- No changes to the `needs-review` label or to existing issues.
- Must not become a source of roadmap backlog or repeated findings.
- No live dispatch or automatic merge during delivery.

## Constraints

- Improve `.github/workflows/specification-alignment-review.md` in place and
  regenerate `specification-alignment-review.lock.yml` with the documented
  `gh aw compile ... --validate --actionlint --shellcheck --action-mode action
  --action-tag 221ba4f4319de6b8e809d1a15528b3ae6a818aa5` command.
- `needs-attention` supersedes `needs-review` for this workflow's new findings
  (user-approved amendment recorded in #75). Both labels already exist.

## Approaches

- **A. Issue scope only:** schedule, label, wording. Safest, but daily runs
  could re-surface findings if duplicate searches miss them.
- **B. A plus prompt hardening for daily cadence (chosen):** tighter
  duplicate handling and explicit clean-run `noop`, all inside the prompt.
- **C. B plus `skip-if-match` backpressure:** caps cost and backlog growth,
  but pauses findings and exceeds #75's scope; candidate follow-up if noise
  appears.

B was chosen because it makes daily runs safer without adding mechanics
beyond what #75 specifies.

## Design

Workflow front matter:

- Cron becomes `0 8 * * *`; the comment states "Daily at 08:00 Paris time".
- `create-issue.labels` and `allowed-labels` become `[agent, needs-attention]`.

Prompt:

- Replace "not just changes from the previous week" with wording that does
  not imply a weekly cadence (each run reviews the full default-branch snapshot).
- Duplicate check: always search the stable
  `specification-alignment-review:<key>` in open and closed issues and PRs
  before proposing, since the same contract is re-examined every day; a
  matching closed issue is treated as a maintainer decision.
- State the label pair as `agent` and `needs-attention`.
- Keep `noop` for clean runs; never create a summary issue.

Usage section:

- Fix the stale workflow name ("Specification alignment review").
- Describe the daily schedule and the label names.
- Keep the authentication guidance and compile command; drop wording that
  claims weekly cadence. Leave the unverified-eligibility caveat since it is
  still accurate.

## Verification

Recompile with the documented command and confirm the lock file contains the
daily cron and `needs-attention`, and that the diff changes only expected
files. No dispatch.

## Left to the builder

Exact prompt wording within the constraints above.
