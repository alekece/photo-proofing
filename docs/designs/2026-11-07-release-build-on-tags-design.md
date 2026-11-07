# Release build on release tags only

Status: Approved by the maintainer on 2026-11-07, including the *my call* decisions.
Issue: [#210](https://github.com/alekece/photo-proofing/issues/210).

## Intent

The `Rust release build` job (`cargo build --workspace --release`) runs on every Rust-touching PR and is often the last check holding up a merge. Clippy and the tests already catch compile errors, so the release profile only matters when shipping. Run it when a release tag is created instead, and make its failure block that release.

## Current state

- `.github/workflows/ci.yml` runs on `pull_request` and `push` to `main`. Job `rust-build` (display name `Rust release build`) runs when the `rust` path filter matches, and always on pushes to `main`.
- `.github/workflows/cd.yml` is a `workflow_run` of CI limited to `main`; it publishes staging images and moves the moving tag `staging`.
- No release workflow, no release tags (only `staging`), and no git-cliff configuration exist yet. `docs/releases/MVP.md` records release scope but defines no tag scheme.
- The `Build application images` CI job (runs when `containers` filter matches) already builds the API and worker Dockerfiles, which run `cargo build --release -p …`.
- The repository docs do not reference branch protection. At the time of writing the API reports no classic protection or rulesets on `main` (may be a permission limit of the reading account), so the required-checks list must be inspected in the settings UI.

## Goals

- PRs no longer run `cargo build --workspace --release`.
- A pushed release tag runs the release build; a failure fails the tag's workflow run and must stop any later release step.
- `Rust release build` is no longer a required check in branch protection.

## Non-goals

- Creating the release process itself (changelog, GitHub Release, artifacts, publishing). Only the gate is added.
- Changing the `Build application images` job (see Risks). *My call*: it is a separate cost decision.
- Keeping a release build on `main` pushes (*my call*: removed with the job; `Build application images` still compiles release binaries there).

## Constraints

- Branch protection is a manual repository-settings step; no workflow or file in Git can perform it. It must be done by someone with admin rights, and its verification is part of the PR's evidence.
- Skipped jobs count as passing for required checks (existing note in `ci.yml`), so a required check that no longer reports on PRs would block merging forever. The setting therefore must change **before or together with** the merge, not after (see Rollout).
- Workflow changes cannot be exercised by a PR for tag triggers; verification needs a manual run (see Verification).

## Approaches

1. **New `release.yml` triggered by tag pushes (recommended).** `on: push: tags: ["v[0-9]*"]`, one job `Rust release build` with the same steps as today. Later release/publish jobs declare `needs: rust-release-build`. Keeps CI as the PR/main gate and CD as the staging pipeline; tag-only trigger cannot affect them. Cost: a few duplicated setup lines (checkout, toolchain, libvips, cache).
2. **Reuse CI**: add `tags` to `ci.yml`'s `push` trigger and gate `rust-build` on `github.ref_type == 'tag'`. No duplicated steps, but every other CI job would also run on tags (rust filter forces `true` on push), the concurrency group and the `changes` job semantic get tag-specific exceptions, and CD's `workflow_run` would see tag runs. More coupling for little gain.
3. **Put it in `cd.yml`.** CD only reacts to `main` after CI and publishes staging; a release tag is a different event and would need a second trigger and conditionals in a workflow whose staging steps must not run for tags. Rejected.

## Decisions

- **Approach 1** (*my call*).
- **Tag pattern: `v[0-9]*`** (*my call*): conventional `vMAJOR.MINOR.PATCH`, and it cannot match the existing moving `staging` tag. No scheme exists yet; if the owner prefers another, only this pattern changes.
- **Job name stays `Rust release build`** (*my call*) for log and search continuity; it is not a required check anywhere after this change.
- **Same command** `cargo build --workspace --release` (*my call*): the issue is about where it runs, not what it does.
- **"Blocks the release"** (*my call*): a pushed tag cannot be undone by a workflow, so no design can stop the tag itself. What is blockable is anything *after* the tag. No release/publish step exists yet, so today a failure only turns the tag's run red; acceptance criterion 2 is met in full once a release job exists that declares `needs:` on this job. Contract, stated in a comment at the top of `release.yml`: every future release/publish job must `needs:` the release build and be created in the same workflow. Tag creation is currently manual by the owner; tags created by a workflow with `GITHUB_TOKEN` do not trigger `on: push: tags`, so any future automated tagging must use a PAT/app token or call this workflow via `workflow_call`.
- **Workflow shape** (*my call*): name `Release`; job id `rust-release-build`; `permissions: contents: read`; `CARGO_TERM_COLOR: always`; no `concurrency` (one run per tag); default timeout. No check that the tag is on `main`; the owner tags `main` by convention. Add `workflow_dispatch` (*my call*, reversing the earlier draft) so the job can be tried without creating a tag.
- **Remove `rust-build` from `ci.yml`** entirely, including the now-unneeded `changes` dependency for it. The `rust` path filter and other Rust jobs are untouched.
- **Branch protection (manual).** In *Settings → Branches* (or *Rules → Rulesets*, whichever protects `main`): open the rule for `main`, under "Require status checks to pass", remove `Rust release build` from the list and save. Leave all other checks (`Rust format`, `Rust clippy`, `Rust dependencies`, `Rust tests`, web and deployment checks) as they are.

## Rollout order

1. Remove `Rust release build` from required checks (safe in advance: an unrequired existing check is just informational).
2. Merge the workflow PR.
3. Confirm the merged PR itself (or the next Rust-touching PR) no longer shows the check.

Reversing steps 1 and 2 would leave PRs waiting for a check that never reports.

## Risks

- Release-profile-only breakage (e.g. `[profile.release]` edits, optimization-dependent failures) surfaces at tag time rather than in the PR. Accepted by the issue.
- While `Build application images` remains, PRs matching its `containers` filter still compile `pp-api` and `pp-image-worker` in release mode (not `--workspace`), so the time saving is partial. On `main` pushes that job is not forced on, so release coverage there is conditional; other crates and profile-only breakage are covered only at tag time. Accepted.
- A bad tag can only be fixed with a new tag; acceptable for a gate that mirrors what was previously caught pre-merge.

## Verification

- PR: the Checks tab for a Rust-touching PR shows no `Rust release build`.
- Workflow: after merge, run `Release` via `workflow_dispatch` on `main` and confirm the job succeeds. This avoids a throwaway tag that would match `v[0-9]*` and could later trigger real release jobs. The tag trigger itself is exercised by the first real release tag.
- Settings: the API reports no protection on `main`, so first record the current required-checks list from the settings UI (before), then after the change confirm it lacks `Rust release build` (after); put both in the PR.
- `actionlint`-style validity is covered by GitHub accepting the workflow; no local tool is required.

## Left to the builder

- Exact `release.yml` content (copying today's job steps), top-of-file comment on the `needs:` contract, and the `rust-build` removal in `ci.yml`.
- Add one line to `README.md` where CI/verification is described, noting the tag-triggered release build (README currently does not mention the job).
- Performing, or asking the owner to perform, the settings step and recording the evidence.
