# RustSec triage outside PR CI

Tracking issue: [#436](https://github.com/alekece/photo-proofing/issues/436).

## Outcome

Keep unrelated PRs mergeable when RustSec publishes an advisory, without losing the existing dependency policy checks or letting new advisories go unnoticed.

## Approaches

- **Scheduled scan only:** keeps advisories out of PR CI and makes findings visible in Actions, but leaves triage and fixes to the maintainer.
- **Scheduled scan with conditional gh-aw remediation (chosen):** keeps the standalone scan and its Actions report, and opens an actionable PR or issue only for a newly discovered, untracked finding. Clean and already-tracked runs need no agent.

## Design

- Keep the required `Rust dependencies` check and its existing scope. Run `cargo deny --workspace check bans licenses sources` instead of all checks. Preserve `deny.toml` and the `Protect main` ruleset. Raise the locked `url` version on `main` from `2.5.8` to `2.5.9` to resolve [RUSTSEC-2026-0592](https://rustsec.org/advisories/RUSTSEC-2026-0592); `main` already has the patched `lettre`.
- Add a separate, non-required workflow scheduled daily and runnable manually on `main`. Adapt Velo's deterministic scan: check the locked workspace against RustSec using existing ignores, report findings in the Actions summary, and compare exact advisory IDs against titles of open issues and PRs. A scanner or lookup failure fails the scan visibly; it is not a clean result and must not invoke the agent.
- Run gh-aw only when the scan finds an untracked advisory. Permit at most one ready-for-review PR per run, labelled `agent`. Prefer a version-only update to existing Cargo manifests and the lockfile. If an upgrade requires a small source migration or a dependency replacement, open a PR only when a focused implementation is verified; request maintainer review and explain the alternatives and tradeoffs. Require targeted advisory rechecks, the existing dependency policy checks, and relevant Rust tests before publishing either kind of PR.
- If no verified fix is available but there is an actionable next step, open a deduplicated issue identifying the advisory, affected dependency, and proposed remediation. Apply the repository's issue labels and milestone rules, including `needs-attention` when a maintainer decision or design is needed. Never open both an issue and a PR for the same remediation in one run.
- Use read-only repository access in the agent and gh-aw safe outputs for writes. The safe-output job reuses the repository's existing GitHub App to open PRs and trigger ordinary CI; the agent never receives the App token. Treat advisory and GitHub text as data. Do not change advisory ignores, publish speculative issues, enable the paused issue automation, or auto-merge anything. Do not count workflow-local verification as ordinary PR CI.

## Acceptance

- The existing Rust dependency status check still runs license, ban, and source checks, but an advisory alone cannot fail that job.
- A locked advisory scan confirms the `url` advisory is gone. Check the Rust workspace with the patched lockfile.
- Validate the generated gh-aw workflow and cover clean, tracked, untracked, and failed scan cases. A clean or tracked scan publishes no issue or PR and makes no agent call; a failed scan reports its error and publishes nothing.
- A verified version bump can be published ready for review. A verified migration can be published for maintainer review with its alternatives documented. Neither can be merged by the workflow.
