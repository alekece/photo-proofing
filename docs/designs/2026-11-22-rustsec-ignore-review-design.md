# Weekly RustSec ignore review

Tracking issue: [#439](https://github.com/alekece/photo-proofing/issues/439).

## Outcome

Remove advisory exceptions that no longer apply, and surface verified fixes for exceptions that still do. Keep the daily RustSec scan and required PR dependency checks unchanged.

## Approach

Use a separate weekly and manual workflow on `main`, after [#438](https://github.com/alekece/photo-proofing/pull/438). Unlike extending the daily scan, this lets the weekly workflow change an exception without giving the daily workflow permission to edit `deny.toml`. A report-only workflow would make the maintainer do straightforward exception removals manually.

## Behavior

- Read the actual ignore entries from `deny.toml`. Probe each independently with a temporary config that removes exactly that entry, using a fresh advisory database and locked dependency resolution for the default and all-feature graphs. A scanner, registry, or configuration failure is not proof that the exception is obsolete.
- If an advisory no longer applies in either graph, open a ready-for-review PR removing only the obsolete entry or entries and any explanation that no longer applies. If a focused dependency update resolves an ignored advisory, verify the upgrade and exception removal together before opening a PR. If a fix still needs a maintainer decision, file an actionable issue only when there is new evidence.
- Reuse the daily workflow's exact-ID and `rustsec`-label tracking, same-commit verification, read-only agent credentials, and publication safeguards. Open at most one PR per run, labelled `agent` and `rustsec`. Do not duplicate active work, modify unrelated exceptions, add new ignores, or auto-merge.
- In a separate read-only job, reproduce a lockfile-only update from the scanned revision using Cargo and the exact affected crate or a direct parent. Escalate fixes that need a manifest or source change. Repeat both advisory scans and dependency policy checks; compile the locked workspace for dependency updates. Publish the verified patch on its original parent without rebasing. Queue daily and weekly runs together without displacing pending scans.
- Keep orchestration inline and share one standard-library Python helper for scan decisions and policy validation. Leave only the GitHub and gh-aw adapters in JavaScript.

## Acceptance

- An obsolete ignore produces a ready-for-review PR even when no dependency version needs to change. A still-applicable ignore without a verified fix and a failed scan cannot produce a removal PR.
- Validate the obsolete, still-applicable, and failed-probe cases locally and compile and lint the workflow. Any future auto-merge for `deny.toml`-only removals requires a separate policy change and green required CI.
