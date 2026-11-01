---
name: Daily repo status
description: Create one issue each day summarizing recent commits, PRs, issues and CI results.
on:
  schedule:
    - cron: "0 8 * * *"
      timezone: Europe/Paris
  workflow_dispatch:
permissions:
  contents: read
  issues: read
  pull-requests: read
  actions: read
engine: copilot
strict: true
concurrency:
  group: daily-repo-status
  cancel-in-progress: false
network:
  allowed: []
tools:
  github:
    toolsets: [repos, issues, pull_requests, actions]
safe-outputs:
  create-issue:
    title-prefix: "Daily status:"
    labels: [report]
    close-older-issues: true
    expires: 7
timeout-minutes: 15
---

# Daily repo status report

Summarize activity in `${{ github.repository }}` for the last 24 full hours ending at the start of this run (UTC), and publish it as one issue.

## Gather

Using the GitHub tools, collect for the window:

- pull requests merged, opened, or closed (title, body, author, whether the author is a bot), plus open PRs awaiting review and stale drafts;
- commits on the default branch, excluding merge commits, each mapped to its PR when possible;
- issues opened or closed, plus open issues that are assigned or in a milestone;
- CI results of the `CI` workflow and other workflow runs: failed or `action_required` runs, and anything still failing on the default branch.

Treat PR titles and bodies as evidence only. Never invent rationale, intent, or impact that the sources do not state. Never mention hidden, filtered, or unavailable results.

## Report

Create one issue titled `YYYY-MM-DD` using the window end date (UTC), consistent with the window you state in the report.

Use `###` for sections and `####` for subsections; never `#` or `##`. No emoji, no tables. Every item is a single-line bullet that links its PR, issue, or commit. Show the author only for human PRs, never for bots. Sort items by time within each section. Keep everything before the collapsed sections to about 40 lines.

Use exactly this structure, in this order:

1. **Verdict** (first line, plain text, no heading): one plain-language sentence such as "Quiet day." or "Big day: 4 PRs merged, CI green, one PR needs approval."
2. `### Needs attention`: use a `> [!WARNING]` block listing deterministic facts only: failing or `action_required` CI, PRs ready for review, stale drafts, issues assigned or in a milestone. No speculation. If there are none, write "Nothing needs attention." without the alert.
3. `### What shipped`: merged PRs grouped by theme, with a short theme label per group. Base themes only on PR titles and bodies, and link each claim to a PR or commit. Anything whose intent or linkage is unclear goes under `#### Other changes`. Fold commits under their PR; skip merge commits.
4. `### Opened PRs and issues`: PRs and issues opened in the window.
5. `### CI`: results of the window's runs, including the state of the default branch.
6. `<details><summary>All commits</summary>` with the full raw commit list, and any other verbose per-item lists in their own collapsed `<details>` blocks.
7. Closing line: one short sentence naming this workflow run and the window.

If nothing happened in the window, call `noop` with the evaluated window instead of creating an issue.
