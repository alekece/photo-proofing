---
name: review-change
description: Use after the last plan task is implemented and before opening a PR, to run the checks, get independent code and security reviews, and do the refactor pass.
---

# Review change

Run once, after all plan tasks are implemented, unless the user explicitly opts out. The `code-reviewer` and `security-reviewer` agents read [`REVIEW.md`](../../../REVIEW.md).

## Steps

1. Confirm the change is documented for its area: `crates/AGENTS.md` (Comments and Docs) for Rust, `web/AGENTS.md` for the frontend, and the documents named in `AGENTS.md` for behavior changes.
2. Run the checks for the changed area once: the format, lint and test commands in `crates/AGENTS.md` for `crates/`, `pnpm --dir web verify` for `web/`. Documentation-only changes need none. Report any check that could not run.
3. Start two fresh agents in parallel, neither of which wrote the code: `code-reviewer` and `security-reviewer`. Use a stronger model than the lightweight one used for per-task passes, but not the strongest available. Each states the risk level from `REVIEW.md`.
4. Fix blocking findings, rerun the checks the fix could affect, and have a fresh reviewer re-check only the changed code and its context. After three rounds with blocking findings left, stop and ask the user with a short summary. Skip the security re-review for a small UI or copy follow-up that touches no security-sensitive area.
5. Do one refactor pass for evident improvements to responsibilities, naming, duplication and repository-guidance compliance. Stay within the approved scope, preserve behavior, and rerun the affected checks.
6. Record the result, including when no refactor was warranted.

If no fresh reviewer can be started, never substitute your own review: report the blocker, record "not independently reviewed", and treat the PR as at least medium risk.
