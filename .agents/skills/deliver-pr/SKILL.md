---
name: deliver-pr
description: Use when review is done and the change is ready for a pull request, to open the PR with its risk section and merge it according to its risk level.
---

# Deliver PR

Run after `review-change`. Merging always uses a squash merge, never a local merge, and never happens before the maintainer's explicit review and approval.

Auto-merge mode: `recommend`. Only the user changes it, to `enabled`, once the risk classifications have proved reliable.

## Open the PR

1. Create the PR referencing the original issue when there is one. Write for humans: a short summary of the changes in plain language, the verification evidence, and any material limitation. Assign the maintainer.
2. Add a **Review and risk** section: the risk level from `REVIEW.md` with a one-line reason, each reviewer's verdict with any non-blocking findings, the blocking findings that were fixed, and the checks run.
3. Before any push to a PR that has auto-merge enabled, other than updating it from `main`, disable it (`gh pr merge --disable-auto`), whoever pushes. After the push, update the section and repeat the merge decision from scratch.
4. Keep the worktree available through PR review and corrections.

## Merge by risk

- **Low**, when the auto-merge mode is `enabled` and the PR was opened by this workflow from a branch of this repository (never a fork or Dependabot PR): once CI is green, no blocking finding remains, no user review thread or request is open and the PR is not labelled `blocked`, enable auto-merge with `gh pr merge --squash --auto`. Branch protection performs the merge. A PR whose branch falls behind `main` waits until someone updates the branch; that needs green CI but no new review. If the user objects after auto-merge is enabled, disable it; a `blocked` label alone does not stop GitHub.
- **Low** in `recommend` mode, and **medium**: request the user's explicit approval, then squash-merge.
- **High**: as medium, plus a full-depth security review covering every touched area and targeted validation for the risky part (for example a migration, authorization or deployment test), listed in the PR.
