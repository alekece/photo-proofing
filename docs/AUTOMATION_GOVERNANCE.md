# Automation Governance

## Purpose

This document defines how future repository automation may inspect and improve Enoam Gallery safely. It supports daily checks, issue triage, clearly scoped implementation, and merge preparation. It never authorises automation to make production, privacy, security, or policy decisions.

## Safety posture

Enoam Gallery handles private client photos and share links. Protecting access, privacy, deletion behaviour, asset integrity, and delivery quality takes priority over speed of change.

## Labels

- `triage`: needs classification or reproduction.
- `bug`: incorrect or broken behaviour.
- `security`: possible security or privacy concern.
- `enhancement`: new capability.
- `documentation`: documentation-only work.
- `blocked`: cannot proceed without a decision or dependency.
- `needs-human`: requires a product, privacy, security, legal, or hosting decision.
- `ready-for-build`: scope and acceptance criteria are clear.
- `automation-safe`: implementation is explicitly allowed by these rules.
- `do-not-automate`: never implement or merge automatically.
- `automerge-approved`: a human reviewer expressly permits an otherwise eligible merge.

## Daily review

Automation may read issues, pull requests, tests, dependency reports, and repository configuration. It may identify missing acceptance criteria, duplicates, missing labels, stale pull requests, and failed checks. It may run dependency and secret scans, recommend follow-up issues, apply safe triage labels, and post concise non-sensitive summaries.

It must not modify application code, delete content, close issues only because they are old, alter repository settings or workflow permissions, or publish security details. A potential security finding gets `security` and `needs-human`, plus only a minimal non-sensitive explanation.

## Readiness standard

Automation may implement an issue only when it has clear verifiable criteria, identified scope where known, `ready-for-build`, and `automation-safe`, with none of `blocked`, `needs-human`, `security`, or `do-not-automate`.

If requirements are ambiguous, automation stops, adds `needs-human`, and asks the specific missing question rather than guessing.

## Human-only areas

Automation must not implement or merge changes involving authentication, sessions, passcodes, share tokens, authorisation, object storage, signed URLs, CDN rules, image transformations, watermarks, download controls, ZIP delivery, expiry, revocation, retention, deletion, backups, audit logs, client data, legal terms, production deployment, infrastructure, secrets, permissions, new dependencies, major upgrades, licences, billing, public galleries, AI processing, or any document under `docs/`.

It may prepare analysis or a proposed pull request for these areas, but a human must review the plan before merging.

## Rules for eligible implementation

Automation uses one branch and pull request per issue, links the issue, restates its criteria, stays in scope, adds or updates tests, reports all quality results, and stops with `needs-human` on validation failure, ambiguity, or possible risk.

It must never commit credentials, example secrets, client images, personally identifiable production data, or private download links.

## Merge gate

An automated merge requires: eligible issue status, no human-only area changed, explicit approval for any dependency, passing build, lint, type-check, test, secret scan and dependency scan, no unresolved reviews or conflicts, human pull-request approval, and the `automerge-approved` label.

Use squash merge and remove the source branch. Do not expose private or security details in a merge comment.

## Required checks

Once the application exists, required checks include locked dependency installation, formatting, linting, type checking, unit and integration tests, production build, secret and dependency scans, and tests for authorisation, expiry, revocation, favourites isolation, and delivery access.

## Triage response format

```md
Status: ready-for-build | needs-human | blocked | duplicate
Labels added: ...
Why: ...
Next step: ...
```

Changes to this governance document require a dedicated issue and explicit human approval.