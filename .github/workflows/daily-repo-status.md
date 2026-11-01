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

- commits merged to the default branch (author, subject);
- pull requests opened, merged, or closed, plus open PRs awaiting review;
- issues opened or closed, plus open issues labeled `needs-attention`;
- CI results of the `CI` workflow and other workflow runs: failures, and anything still failing on the default branch.

## Report

Create one issue titled with today's UTC date (for example `2026-11-01`). Follow these style rules:

- Use `###` for sections and `####` for subsections; never `#` or `##`.
- Start with a short overview (2-3 sentences), then the items that need the maintainer's attention.
- Use `> [!WARNING]` for failing CI or stuck PRs; do not use emoji.
- Put per-item lists (commits, PRs, issues) inside `<details><summary>...</summary>` blocks.
- Link every issue, PR, and commit.
- End with a line naming the workflow run and the window.

If nothing happened in the window, call `noop` with the evaluated window instead of creating an issue.
