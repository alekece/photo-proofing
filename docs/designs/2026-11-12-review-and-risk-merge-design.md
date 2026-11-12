# Review knowledge, security review and risk-based merging

Issue: [#336](https://github.com/alekece/photo-proofing/issues/336)

## Intent

The maintainer owns what gets built and why; agents own how it is built, tested, reviewed and validated. Today every PR waits for the maintainer, and the same review remarks come back PR after PR. This change:

- writes the maintainer's recurring review remarks down in `REVIEW.md`, so reviewers catch them before the maintainer does;
- adds a security lens to the existing fresh-reviewer step;
- classifies each PR as low, medium or high risk and lets low-risk PRs merge without the maintainer.

## Goals

- Fewer repeated human review remarks over time.
- Every PR gets an independent code review and a security review, recorded on the PR.
- Low-risk PRs (docs, tests, visual web changes, UI wording) merge on green CI with no blocking findings, without human approval.
- Medium and high risk still wait for the maintainer; high risk gets extra technical validation.

## Non-goals

- No new GitHub workflow, bot or orchestration layer (approach A below).
- No change to brainstorming, design approval, issue or worktree steps.
- No change to the `pick-issues` / `implement-issue` automation files. Both workflows are disabled on GitHub for now; `AGENTS.md` gets one line saying automation is paused, and they never merge when re-enabled.
- No weakening of CI.

## Anti-goals

- A `REVIEW.md` that copies `AGENTS.md` or lists one-off remarks.
- Generic security advice unrelated to the diff.
- Endless review/fix loops.
- Auto-merging anything the classifier is unsure about.

## Constraints

- One human maintainer. Agents push under the maintainer's account, so GitHub "Approve" is impossible on those PRs; approval is the maintainer's explicit statement in the conversation or a PR comment.
- `main` currently has no protection. The maintainer creates the ruleset below (repository setting, outside this change); auto-merge relies on it.
- A merge must be performed with a user or GitHub App token, never the default `GITHUB_TOKEN`, or CI/CD on `main` does not start.
- Repository rules: non-test Rust free functions are forbidden; docs in `docs/designs`, plans in `docs/plans`; never add a Co-authored-by trailer.

## Approaches

- **A. Instructions only (chosen).** Extend the existing review step in `AGENTS.md`; add `REVIEW.md`. Fresh reviewers run as sub-agents, as the delivery workflow already does. Cheapest; no new moving parts. Trade-off: a review is guaranteed by instructions, not by a GitHub check.
- B. A `review-pr` gh-aw workflow on every PR that reviews, labels risk and enables auto-merge. Independent by construction and portable, but costs Actions minutes and AI requests per push and adds a workflow. Rejected by the maintainer.
- Copilot's built-in PR review: no separate security lens, no risk output. Rejected.

## Decisions

### Repository settings (done by the maintainer, documented in README)

Branch ruleset **`Protect main`**, target the default branch, active, empty bypass list:

- require a pull request, 0 approvals;
- require status checks: `Detect changes`, `Deployment checks`, `Rust format`, `Rust clippy`, `Rust dependencies`, `Rust tests`, `Web format`, `Web lint and check`, `Web dependencies`, `Web tests`, `Web build` (not the image builds: their matrix check names change when skipped, and `docker/` is high risk anyway);
- require branches to be up to date before merging;
- block force pushes; restrict deletions.

Also: allow auto-merge; enable CodeQL default setup and secret scanning.

### `REVIEW.md` (new, repository root)

Short, three sections:

1. **Rules learned from review** — remarks that appeared in two or more PRs and are not in `AGENTS.md`, each one line with the PR numbers as evidence:
   - Prefer extending an existing route, type or enum with a parameter over a parallel handler, route or type (#132, #145, #147, #231).
   - Return dedicated response types; do not build JSON or headers inline (#76, #78, #221, #279).
   - Keep responsibilities where they belong: extractors have no database side effects, the caller opens transactions, the database does not enforce caller policy, a repository does not depend on unrelated options (#132, #169, #177, #231).
   - One representation per concept; no near-duplicate enums or structs (#132, #147, #231).
   - Put failure detail in the status variant (`Failed(reason)`), not a parallel field (#132, #221).
   - Use short domain names, and rename consistently across file, module, type, table, migration and environment variable (#148, #221, #284).
   - Use framework and library features (serde attributes, strum, actix extractors, `.service`, middleware) before hand-written code (#78, #109, #166, #177, #289).
   - Group related settings in one config or options type (#76, #177).
2. **Check strictly** — `AGENTS.md` rules the maintainer still repeats, linked not copied: no free functions, value-named variables, blank-line layout, no newtype without an invariant, operator limits are configuration, file item order.
3. **Risk levels** for this repository:
   - **High:** authentication and access (`crates/api` auth, extractors, middlewares, share links), database migrations, `deploy/`, `docker/`, `.github/`, dependency manifests or lockfiles, agent instruction files (`AGENTS.md`, `REVIEW.md`, `.github/skills/`).
   - **Low:** documentation only, tests only, purely visual web changes, UI wording (`web/messages/`).
   - **Medium:** everything else.

   The highest level touched wins. Reviewers may raise the level, never lower it. When unsure, go higher. Size alone never makes a PR low risk.

`REVIEW.md` also states how it grows: when the maintainer's PR feedback repeats a point already raised in an earlier PR, the agent proposes a one-line rule in that PR; it becomes permanent only when the maintainer approves.

### Review step (`AGENTS.md` delivery workflow, step 6)

After checks pass, dispatch two fresh sub-agents in parallel, neither of which wrote the code:

- **Code reviewer**: correctness, edge cases, error handling, tests and regressions, maintainability, compliance with the approved design, `AGENTS.md` and `REVIEW.md`. Uses the harness's code-review agent when available.
- **Security reviewer**: picks only the areas the diff touches (auth, input handling, secrets, crypto, dependencies, file paths, network, concurrency and resource limits, deployment and CI, logging and leakage, business-logic bypass), inspects the diff with enough surrounding context, and reports actionable findings with severity, location and evidence, marking each as confirmed or hypothesis. Uses the harness's security-review agent when available.

Each finding is marked blocking or non-blocking. Blocking findings are fixed, checks rerun, and a fresh reviewer re-checks only the changed code and its context. After three rounds with blocking findings left, stop and ask the maintainer with a short summary. The existing refactor pass follows as today.

### Risk, PR record and merge (`AGENTS.md` steps 7–8)

- The PR description gets a short **Review and risk** section: risk level with a one-line reason, each reviewer's verdict, checks run. Reclassify after every push.
- **Low:** once CI is green, no blocking findings remain, no maintainer review thread or request is open and no `blocked` label is set, the agent enables auto-merge (`gh pr merge --squash --auto`). Branch protection performs the merge.
- **Medium:** wait for the maintainer's explicit approval, then squash-merge.
- **High:** as medium, plus the security review is mandatory at full depth and the PR adds targeted validation for the risky part (for example a migration test, an authorization test, or a deployment check), listed in the PR.

The security reviewer runs on every PR; for docs-, tests- or wording-only diffs it may simply report that no security area is touched. "Full depth" for high risk means every touched area is covered with realistic failure or exploitation paths.

`AGENTS.md` step 8 ("never merge automatically") is narrowed to: low-risk PRs may auto-merge in enabled mode; everything else, and every automation workflow, still never merges without the maintainer.

If no fresh reviewer can be started, the agent records "not independently reviewed" and the PR is treated as at least medium.

GitHub's auto-merge does not update a branch that fell behind `main`. Such a PR waits; the next session or the maintainer uses "Update branch". Merging `main` into the branch does not change the PR's own diff, so it needs no new review, only green CI.

### Rollout: recommendation first

`AGENTS.md` states the current auto-merge mode, and only the maintainer changes it. It starts as **recommend**: the agent classifies and records the risk, and a low-risk PR is handled like medium (the maintainer approves, the agent squash-merges). The maintainer switches it to **enabled** when the classifications have proved reliable.

### Plans

Implementation plans are no longer committed: they stay untracked in the worktree under `docs/plans/`, which `.gitignore` excludes. The `AGENTS.md` rules about committing and later removing plans are replaced accordingly, so nothing needs cleaning up before a merge.

### Report location

The reviewers' results live in the PR description's **Review and risk** section, updated after each push. No separate report files.

## Left to the builder

Exact wording of prompts and `REVIEW.md`; README placement of the ruleset instructions; how the PR body section is formatted.
