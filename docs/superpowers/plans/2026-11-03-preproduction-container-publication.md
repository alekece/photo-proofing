# Pre-production Container Publication Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Publish tested API, worker and production frontend images for each eligible `main` commit and mark the latest complete set with the `staging` Git tag.

**Architecture:** Extend the existing main-branch CI workflow so a publication job runs only after applicable checks pass. Build private GHCR packages with immutable full-commit tags, then advance `staging` after all three images exist. Use the existing Rust configuration flow for a `Secure` cookie setting and a Node-adapter frontend image; do not implement the consumer that deploys these artifacts.

**Tech Stack:** GitHub Actions, GitHub Container Registry, Docker Buildx, Rust/Actix, SvelteKit Node adapter, pnpm.

**Spec:** `docs/superpowers/specs/2026-11-03-preproduction-deployment-design.md` (approved, including the `staging` tag name); issue [#135](https://github.com/alekece/photo-proofing/issues/135).

## Global Constraints

- GHCR packages are private.
- Use `ghcr.io/alekece/pp-api`, `ghcr.io/alekece/pp-image-worker`, and `ghcr.io/alekece/pp-web`.
- Tag images `sha-<full-commit-sha>`; do not publish moving `dev` image aliases.
- Advance Git tag `staging` only after all three images are published; do not move it backwards on out-of-order runs.
- The deployment consumer is responsible for authenticating to pull private images.
- Server polling, server credentials, Compose deployment, LAN/DNS/TLS setup, and persistent server configuration are out of scope.
- Secure cookies default to false for local HTTP; production can enable the setting.
- Keep secrets and user data out of images and build logs.

## Review Focus

- **A pull request, failed check, cancellation, or failed image build must not publish images or advance `staging`.** Pin the job condition to the push-to-main event and require every applicable CI result to be successful or intentionally skipped.
- **Overlapping or stale workflow runs must not move `staging` backwards.** Serialize publication and compare the current tag to the candidate commit before updating it.
- **A newer main push must not automatically cancel an active publication.** Keep PR cancellation behavior, but let running main workflows finish so a cancellation cannot interrupt sequential image publication.
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
- Modify: `.env.example`
- Modify: `docker-compose.yml`
- Modify: `README.md`
- Modify: `docs/adr/0002-account-sessions.md`

**Interfaces:**
- `SessionConfig` gains `secure_cookie: Option<bool>`, exposed as scalar `PP_SESSION_COOKIE_SECURE` and `--session-cookie-secure <true|false>` values; `ApiState` treats an unset value as `false`.
- `ApiState` uses the parsed setting for both session and removal cookies; no request-handler signature changes.

- [x] **Step 1: Add configuration regression tests.** In `crates/api/src/config.rs`, extend the defaults test to assert the unset value defaults effectively to `false`; add TOML and CLI tests for explicit `true` and `false`, and an environment-precedence assertion in the existing subprocess-isolated test. Update existing `SessionConfig` literals in `crates/api/src/tests.rs` and `crates/api/src/middlewares/failed_attempts.rs` to specify `Some(false)`.
- [x] **Step 2: Run the focused config tests and confirm they fail.**

Run: `cargo test -p pp-api config::tests`

Expected: compilation/test failure because `SessionConfig` has no secure-cookie setting.

- [x] **Step 3: Implement the setting in `SessionConfig`.** Add `secure_cookie: Option<bool>` with scalar `ArgAction::Set`, the `--session-cookie-secure <true|false>` option, and environment variable `PP_SESSION_COOKIE_SECURE`. Normalize `None` to false when building API state; `Option<bool>` lets the config fallback preserve explicit false values.
- [x] **Step 4: Add API cookie regression coverage.** In `crates/api/src/routes/auth.rs`, extend the login/setup-renew-logout cookie test to assert the default is false for issued and removal cookies; add a configured-true case that asserts both issued and removal cookies have `Secure=true`. Update test-state construction to accept the setting without changing unrelated callers.
- [x] **Step 5: Run the focused API cookie tests and confirm the configured-true case fails before implementation.**

Run: `cargo test -p pp-api login_cookie_logout_and_copied_token_denial`

Expected: the existing default behavior remains false; the new configured-true regression assertion fails.

- [x] **Step 6: Apply the setting in `ApiState`.** Store the validated config value and use it in the shared cookie builder instead of hard-coding `.secure(false)`, so both normal and removal cookies match.
- [x] **Step 7: Wire the setting through local Compose.** Add `PP_SESSION_COOKIE_SECURE=false` to `.env.example` and pass `${PP_SESSION_COOKIE_SECURE:-false}` to the API service in `docker-compose.yml`. Verify the rendered default is false and an override of true reaches the API container.
- [x] **Step 8: Document the default and override.** Add `secure_cookie = false` under `[session]` in `config.example.toml`; update the README configuration text and ADR 0002 to state that HTTP development defaults off and HTTPS deployments can enable it.
- [x] **Step 9: Run Rust formatting, lint, focused tests and Compose validation.**

Run: `cargo +nightly fmt --all --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test -p pp-api config::tests && cargo test -p pp-api login_cookie_logout_and_copied_token_denial && cargo test -p pp-api secure_cookie_setting_applies_to_issue_and_removal && docker compose --env-file .env.example config --quiet`

Expected: all commands pass.

- [x] **Step 10: Commit the task.**

```bash
git add crates/api/src/config.rs crates/api/src/api.rs crates/api/src/tests.rs crates/api/src/middlewares/failed_attempts.rs crates/api/src/routes/auth.rs config.example.toml .env.example docker-compose.yml README.md docs/adr/0002-account-sessions.md
git commit -m "feat(api): configure secure session cookies"
```

### Task 2: Production frontend image

**Files:**
- Modify: `web/package.json`
- Modify: `web/pnpm-lock.yaml`
- Modify: `web/vite.config.ts`
- Create: `docker/web.production.Dockerfile`
- Modify: `docker/web.Dockerfile`
- Modify: `README.md`

**Interfaces:**
- SvelteKit build output uses `@sveltejs/adapter-node`; local `vite dev` behavior remains available.
- The production image runs the adapter-generated Node server; CI builds it with repository-root context.

- [x] **Step 1: Replace the frontend adapter dependency.** Use pnpm to replace `@sveltejs/adapter-auto` with `@sveltejs/adapter-node` in `web/package.json` and the lockfile.
- [x] **Step 2: Configure SvelteKit for Node output.** In `web/vite.config.ts`, replace the adapter-auto import/configuration with the Node adapter; keep the existing development proxy and test plugins unchanged.
- [x] **Step 3: Build the frontend and confirm the production output is generated.**

Run: `pnpm --dir web build`

Expected: the build succeeds and emits the adapter-node runtime output.

- [x] **Step 4: Add a multi-stage production Dockerfile.** Create `docker/web.production.Dockerfile` using the existing Node 24 and pnpm 10.13.1 toolchain, install from the frozen lockfile, build the SvelteKit app, and copy only required runtime output/dependencies into a non-development runtime stage. Do not start Vite.
- [x] **Step 5: Document the development and production distinction.** Update the existing dev Dockerfile comment and README so they distinguish the Compose Vite server from the Node-adapter production image.
- [x] **Step 6: Build and smoke-run the production image.**

Run:

```bash
docker build -f docker/web.production.Dockerfile -t pp-web:plan-check .
docker run -d --name pp-web-plan-check -p 127.0.0.1:13000:3000 -e PP_API_INTERNAL_URL=http://127.0.0.1:18080 pp-web:plan-check
docker inspect -f '{{.State.Running}}' pp-web-plan-check
docker rm -f pp-web-plan-check
```

Expected: the image builds successfully. Run it with the test API address, verify the container remains running with the adapter-node process, and stop/remove the test container afterward. `/login` requires a reachable API, so do not treat its response without an API as an image failure.

- [x] **Step 7: Run frontend verification.**

Run: `pnpm --dir web verify`

Expected: format, lint, type-check, tests and build pass.

- [x] **Step 8: Commit the task.**

```bash
git add web/package.json web/pnpm-lock.yaml web/vite.config.ts docker/web.production.Dockerfile docker/web.Dockerfile README.md
git commit -m "build(web): add production Node image"
```

### Task 3: Publish commit-pinned images and `staging`

**Files:**
- Modify: `.github/workflows/ci.yml`
- Create: `.github/workflows/cd.yml`
- Uses: `docker/api.Dockerfile`
- Uses: `docker/worker.Dockerfile`
- Uses: `docker/web.production.Dockerfile`

**Interfaces:**
- The publisher consumes the completed CI workflow run's head SHA.
- It publishes the packages and tags specified in Global Constraints, then updates Git tag `staging`.
- The CI workflow retains pull-request image-build validation; a separate CD workflow publishes only after successful CI on `main`.

- [x] **Step 1: Define the publish gate.** Run Rust and web checks on every push so a later push cannot skip changes from an earlier failed run. On pull requests, retain path-filtered Rust and web checks and build all three images whenever Rust, web, Dockerfile, Docker build-context or CI/CD workflow files change. Trigger a separate CD workflow only after the CI workflow completes successfully for a push to `main`.
- [x] **Step 2: Verify the gate behavior before enabling package writes.** Run local workflow lint and a gate truth table for success, skipped, failed, cancelled, pull-request and non-main cases. GitHub Actions validates CI and confirms the image publisher is skipped on the PR. The CD workflow additionally checks the completed upstream run's conclusion, event type and branch before publication.
- [x] **Step 3: Configure publication of private GHCR packages.** Give only the publishing job `packages: write` and `contents: write`; authenticate with its `GITHUB_TOKEN`. Check out the completed CI run's exact head SHA and build the existing API and worker Dockerfiles and the Task 2 frontend Dockerfile, tagging each with `sha-<full-commit-sha>`. Do not publish from this feature branch; verify package visibility after the first eligible `main` push.
- [x] **Step 4: Make retries reuse existing commit tags.** Before building each package, check whether that exact SHA tag already exists. Skip publication for existing tags and build only missing images; do not replace the contents of an existing commit tag.
- [x] **Step 5: Advance the `staging` Git tag after all image publication succeeds.** Fetch the current tag; create it for the first successful publication, do nothing if it already points at the candidate, and update it only when the current target is an ancestor of the candidate. Serialize the publisher so a stale run cannot overwrite a newer pointer. If the ancestry check or tag update fails, fail publication rather than force a backward move.
- [x] **Step 6: Verify the workflow on disposable local state.** Confirm the gate truth table rejects PR, non-main, failed and cancelled cases; build all three images locally; simulate `staging` creation, forward update, same-commit retry, stale run and divergence against a disposable Git remote. Actual GHCR publication and package visibility are verified only by the first eligible `main` push, not by bypassing the gate from this branch.
- [x] **Step 7: Document the published image contract.** Document the private package names, immutable commit tags, forward-only meaning of `staging`, and production web image runtime contract. Do not add server deployment steps.
- [x] **Step 8: Commit the task.**

```bash
git add .github/workflows/ci.yml .github/workflows/cd.yml
git commit -m "ci: publish preproduction container images"
```

## Final verification

- [x] Run `cargo +nightly fmt --all --check`.
- [x] Run `cargo clippy --workspace --all-targets --locked -- -D warnings`.
- [x] Run `cargo test --workspace --locked`.
- [x] Run `pnpm --dir web verify`.
- [x] Review workflow permissions, immutable tag behavior and `staging` update ordering against the approved design.
- [x] Confirm no workflow or documentation changes configure polling, server credentials, Compose deployment, DNS/TLS, or server-owned configuration.

GHCR package visibility and first publication remain unverified until the first eligible `main` push; no shared images or tags were written from this branch.
