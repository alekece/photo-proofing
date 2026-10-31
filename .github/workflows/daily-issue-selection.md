---
name: Daily issue selection
description: Pick up to X ready issues per run and dispatch one implement-issue worker run per issue.
intent: Deterministically select designed, unblocked, not-yet-started issues and hand each one to the implement-issue worker, so the maintainer's morning work is reviewing PRs rather than starting them.
on:
  schedule:
    # Daily at 09:00 Paris time, after the 08:00 alignment review. Change this cron, then recompile this file.
    - cron: "0 9 * * *"
      timezone: Europe/Paris
  workflow_dispatch:
permissions:
  contents: read
  issues: read
  pull-requests: read
  actions: read
  copilot-requests: write
engine: copilot
strict: true
concurrency:
  group: daily-issue-selection
  cancel-in-progress: false
network:
  allowed: []
tools:
  edit: false
steps:
  - name: Select eligible issues
    env:
      GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
      GH_REPO: ${{ github.repository }}
    run: |
      # X = 2 here must match safe-outputs.dispatch-workflow.max below and the
      # "at most 2" / "quota of 2" prose in the body: the script's <max> caps
      # how many numbers it ever prints, so the three must change together.
      mkdir -p /tmp/gh-aw/agent
      bash .github/scripts/select-issues.sh 2 > /tmp/gh-aw/agent/selected.json
      echo "Selected issues:"
      cat /tmp/gh-aw/agent/selected.json
safe-outputs:
  # Keep operational failures in Actions; this workflow's only write is the dispatch below.
  report-failure-as-issue: false
  report-incomplete:
    create-issue: false
  threat-detection:
    report-as-issue: false
  dispatch-workflow:
    workflows: [implement-issue]
    max: 2
---

# Daily issue selection

Read `/tmp/gh-aw/agent/selected.json`, written by the `Select eligible issues`
step. It is a JSON array of issue numbers (length at most 2) already computed
deterministically from the repository's eligibility rules:

1. The issue has neither the `needs-attention` nor the `blocked` label.
2. Its body References link an existing design spec file
   (`docs/superpowers/specs/*-design.md`) on a branch, and that file resolves.
3. It has no open blocker. GitHub's "blocked by" relationship wins over a body
   `## Dependencies` section when both are present, resolved transitively;
   when every blocker in the chain is itself eligible, the blocker is
   selected in place of the blocked issue (and counts toward the run's quota
   of 2, already reflected in the array).
4. It has no open linked pull request already.

Do not recompute or second-guess this eligibility: the array is the final
decision for this run.

## Dispatch

For each issue number in the array, call the `dispatch-workflow` safe output
once to start the `implement-issue` workflow, passing `issue_number` set to
that number as a string (for example `"42"`). Dispatch at most 2 times total,
one per array entry, in the order the numbers appear.

If the array is empty, make no dispatch calls and finish with no output: no
issue, comment, pull request, or label changes come from this workflow, ever.
Call `noop` with a short reason (for example "no eligible issues this run" or
the number of issues selected) so the run is not silently empty.

## Usage

This workflow uses Copilot through the built-in Actions token with
`copilot-requests: write`; no PAT or `COPILOT_GITHUB_TOKEN` secret is
required, subject to the same organization Copilot subscription
prerequisites as `specification-alignment-review.md`.

The deterministic selection step uses the built-in `GITHUB_TOKEN` (read-only
here) to run `.github/scripts/select-issues.sh`; see that script and
`.github/scripts/select-issues.test.sh` for the exact eligibility rules and
offline test coverage.

Edit the cron above to change the schedule, then regenerate and validate the
lock file:

```sh
gh aw compile daily-issue-selection --validate --actionlint --shellcheck \
  --action-mode action --action-tag 221ba4f4319de6b8e809d1a15528b3ae6a818aa5
```

Commit this Markdown and its generated `.lock.yml` together. The schedule
becomes active on the default branch; manual runs are available through
Actions → Daily issue selection → Run workflow. This workflow only dispatches
`implement-issue` runs; it never writes issues, comments, or pull requests
itself.
