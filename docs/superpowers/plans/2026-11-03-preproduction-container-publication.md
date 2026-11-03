# Pre-production Container Publication Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Publish tested API, worker and production frontend images for each eligible `main` commit and mark the latest complete set with the `preprod` Git tag.

**Architecture:** Extend the existing main-branch CI workflow so a publication job runs only after applicable checks pass. Build private GHCR packages with immutable full-commit tags, then advance `preprod` after all three images exist. Use the existing Rust configuration flow for a `Secure` cookie setting and a Node-adapter frontend image; do not implement the consumer that deploys these artifacts.

**Tech Stack:** GitHub Actions, GitHub Container Registry, Docker Buildx, Rust/Actix, SvelteKit Node adapter, pnpm.

**Spec:** `docs/superpowers/specs/2026-11-03-preproduction-deployment-design.md` (approved); issue [#135](https://github.com/alekece/photo-proofing/issues/135).

## Global Constraints

- GHCR packages are private.
- Use `ghcr.io/alekece/pp-api`, `ghcr.io/alekece/pp-image-worker`, and `ghcr.io/alekece/pp-web`.
- Tag images `sha-<full-commit-sha>`; do not publish moving `dev` image aliases.
- Advance Git tag `preprod` only after all three images are published; do not move it backwards on out-of-order runs.
- The deployment consumer is responsible for authenticating to pull private images.
- Server polling, server credentials, Compose deployment, LAN/DNS/TLS setup, and persistent server configuration are out of scope.
- Secure cookies default to false for local HTTP; production can enable the setting.
- Keep secrets and user data out of images and build logs.

## Review Focus

- **A pull request, failed check, cancellation, or failed image build must not publish images or advance `preprod`.** Pin the job condition to the push-to-main event and require every applicable CI result to be successful or intentionally skipped.
- **Overlapping or stale workflow runs must not move `preprod` backwards.** Serialize publication and compare the current tag to the candidate commit before updating it.
- **A retry after partial publication must not replace an existing commit-tagged image.** Check each exact image tag and build only missing images.
- **HTTP development must retain `Secure=false`, while configured HTTPS cookies—including removal cookies—must use the configured flag.** Test both configurations at the config and API-cookie boundaries.
- **The production frontend image must not launch Vite.** Build and run the Node-adapter image as a smoke check.

---

### Task 1: Configurable session cookie security

**Files:**
- Modify: `crates/api/src/config.rs`
- Modify: `crates/api/src/api.rs`
- Modify: `crates/api/src/tests.rs`
- Modify: `crates/api/src/middlewares/failed_attempts.rs`
- Modify: `crates/api/src/routes/auth.rs`
- Modify: `config.example.toml`
- Modify: `README.md`
- Modify: `docs/adr/0002-account-sessions.md`

**Interfaces:**
- `SessionConfig` gains `secure_cookie: bool`, exposed as `PP_SESSION_COOKIE_SECURE` and `--session-cookie-secure <true|false>`, defaulting to `false`.
- `ApiState` uses the parsed setting for both session and removal cookies; no request-handler signature changes.

- [ ] **Step 1: Add configuration regression tests.** In `crates/api/src/config.rs`, extend the defaults test to assert `secure_cookie == false`; add a TOML test for `[session] secure_cookie = true`, a CLI override test, and an environment-precedence assertion in the existing subprocess-isolated test. Update existing `SessionConfig` literals in `crates/api/src/tests.rs` and `crates/api/src/middlewares/failed_attempts.rs` to specify `false`.
- [ ] **Step 2: Run the focused config tests and confirm they fail.**

Run: `cargo test -p pp-api config::tests`

Expected: compilation/test failure because `SessionConfig` has no secure-cookie setting.

- [ ] **Step 3: Implement the setting in `SessionConfig`.** Add the bool field with default `false`, `ArgAction::Set` for explicit `true`/`false` values via `--session-cookie-secure <true|false>`, and environment variable `PP_SESSION_COOKIE_SECURE`.
- [ ] **Step 4: Add API cookie regression coverage.** In `crates/api/src/routes/auth.rs`, extend the login/setup-renew-logout cookie test to assert the default is false for issued and removal cookies; add a configured-true case that asserts both issued and removal cookies have `Secure=true`. Update test-state construction to accept the setting without changing unrelated callers.
- [ ] **Step 5: Run the focused API cookie tests and confirm the configured-true case fails before implementation.**

Run: `cargo test -p pp-api login_cookie_logout_and_copied_token_denial`

Expected: the existing default behavior remains false; the new configured-true regression assertion fails.

- [ ] **Step 6: Apply the setting in `ApiState`.** Store the validated config value and use it in the shared cookie builder instead of hard-coding `.secure(false)`, so both normal and removal cookies match.
- [ ] **Step 7: Document the default and override.** Add `secure_cookie = false` under `[session]` in `config.example.toml`; update the README configuration text and ADR 0002 to state that HTTP development defaults off and HTTPS deployments can enable it.
- [ ] **Step 8: Run Rust formatting, lint and focused tests.**

Run: `cargo +nightly fmt --all --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test -p pp-api config::tests && cargo test -p pp-api login_cookie_logout_and_copied_token_denial`

Expected: all commands pass.

- [ ] **Step 9: Commit the task.**

```bash
git add crates/api/src/config.rs crates/api/src/api.rs crates/api/src/tests.rs crates/api/src/middlewares/failed_attempts.rs crates/api/src/routes/auth.rs config.example.toml README.md docs/adr/0002-account-sessions.md
git commit -m "feat(api): configure secure session cookies"
```

### Task 2: Production frontend image

**Files:**
- Modify: `web/package.json`
- Modify: `web/pnpm-lock.yaml`
- Modify: `web/vite.config.ts`
- Create: `docker/web-production.Dockerfile`

**Interfaces:**
- SvelteKit build output uses `@sveltejs/adapter-node`; local `vite dev` behavior remains available.
- The production image runs the adapter-generated Node server; CI builds it with repository-root context.

- [ ] **Step 1: Replace the frontend adapter dependency.** Use pnpm to replace `@sveltejs/adapter-auto` with `@sveltejs/adapter-node` in `web/package.json` and the lockfile.
- [ ] **Step 2: Configure SvelteKit for Node output.** In `web/vite.config.ts`, replace the adapter-auto import/configuration with the Node adapter; keep the existing development proxy and test plugins unchanged.
- [ ] **Step 3: Build the frontend and confirm the production output is generated.**

Run: `pnpm --dir web build`

Expected: the build succeeds and emits the adapter-node runtime output.

- [ ] **Step 4: Add a multi-stage production Dockerfile.** Create `docker/web-production.Dockerfile` using the existing Node 24 and pnpm 10.13.1 toolchain, install from the frozen lockfile, build the SvelteKit app, and copy only required runtime output/dependencies into a non-development runtime stage. Do not start Vite.
- [ ] **Step 5: Build and smoke-run the production image.**

Run:

```bash
docker build -f docker/web-production.Dockerfile -t pp-web:plan-check .
docker run -d --name pp-web-plan-check -p 127.0.0.1:13000:3000 -e PP_API_INTERNAL_URL=http://127.0.0.1:18080 pp-web:plan-check
docker inspect -f '{{.State.Running}}' pp-web-plan-check
docker rm -f pp-web-plan-check
```

Expected: the image builds successfully. Run it with the test API address, verify the container remains running with the adapter-node process, and stop/remove the test container afterward. `/login` requires a reachable API, so do not treat its response without an API as an image failure.

- [ ] **Step 6: Run frontend verification.**

Run: `pnpm --dir web verify`

Expected: format, lint, type-check, tests and build pass.

- [ ] **Step 7: Commit the task.**

```bash
git add web/package.json web/pnpm-lock.yaml web/vite.config.ts docker/web-production.Dockerfile
git commit -m "build(web): add production Node image"
```

### Task 3: Publish commit-pinned images and `preprod`

**Files:**
- Modify: `.github/workflows/ci.yml`
- Uses: `docker/api.Dockerfile`
- Uses: `docker/worker.Dockerfile`
- Uses: `docker/web-production.Dockerfile`

**Interfaces:**
- The publisher consumes `github.sha` for the tested commit.
- It publishes the packages and tags specified in Global Constraints, then updates Git tag `preprod`.
- The existing workflow's pull-request and main-push CI jobs remain otherwise unchanged.

- [ ] **Step 1: Define the publish gate.** Add a main-push-only job in `.github/workflows/ci.yml` depending on `changes`, `rust-fmt`, `rust-lint`, `rust-deny`, `rust-test`, `rust-build`, `web-fmt`, `web-lint`, `web-audit`, `web-test`, and `web-build`. Its condition must run after skipped path-filter jobs, accept only `success` or `skipped` dependency results, and reject failures/cancellations. Confirm the job does not run for pull requests.
- [ ] **Step 2: Verify the gate behavior before enabling package writes.** Check the workflow syntax through GitHub Actions validation on the PR, then verify the condition against success, skipped, failed, cancelled, pull-request and non-main cases before enabling publication. Expected: only a `main` push with every applicable CI job successful or intentionally skipped can reach publication.
- [ ] **Step 3: Build and publish private GHCR packages.** Give only the publishing job `packages: write` and `contents: write`; authenticate with its `GITHUB_TOKEN`. Build the existing API and worker Dockerfiles and the Task 2 frontend Dockerfile, tagging each with `sha-${{ github.sha }}`. After first publication, verify all three package visibility settings are Private.
- [ ] **Step 4: Make retries reuse existing commit tags.** Before building each package, check whether that exact SHA tag already exists. Skip publication for existing tags and build only missing images; do not replace the contents of an existing commit tag.
- [ ] **Step 5: Advance the `preprod` Git tag after all image publication succeeds.** Fetch the current tag; create it for the first successful publication, do nothing if it already points at the candidate, and update it only when the current target is an ancestor of the candidate. Serialize the publisher so a stale run cannot overwrite a newer pointer. If the ancestry check or tag update fails, fail publication rather than force a backward move.
- [ ] **Step 6: Verify the workflow end-to-end on a disposable branch or repository state.** Confirm a successful main run publishes exactly the three private images with one SHA tag and moves `preprod`; confirm pull-request and failed-check runs do not publish or move the tag. Confirm a retry reuses existing image tags.
- [ ] **Step 7: Commit the task.**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: publish preproduction container images"
```

## Final verification

- [ ] Run `cargo +nightly fmt --all --check`.
- [ ] Run `cargo clippy --workspace --all-targets --locked -- -D warnings`.
- [ ] Run `cargo test --workspace --locked`.
- [ ] Run `pnpm --dir web verify`.
- [ ] Review the final workflow permissions, package visibility, immutable tag behavior, and `preprod` update ordering against the approved design.
- [ ] Confirm no workflow or documentation changes configure polling, server credentials, Compose deployment, DNS/TLS, or server-owned configuration.
