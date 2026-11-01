# Web Quality Checks Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Enforce Prettier formatting, ESLint, Svelte warnings, and a production dependency audit for `web/` in CI and locally.

**Architecture:** Prettier (Svelte + Tailwind plugins) and flat-config ESLint (typescript-eslint, eslint-plugin-svelte, eslint-config-prettier last) exposed as pnpm scripts; the CI `web` job runs the same steps as a local `verify` script.

**Tech Stack:** pnpm 10.13.1, SvelteKit 3, Svelte 5, TypeScript 6, Tailwind 4, GitHub Actions.

**Spec:** `docs/superpowers/specs/2026-11-01-web-quality-checks-design.md` (issue #130)

## Global Constraints

- No behavior changes to application code; reformatting and lint fixes only.
- Keep the existing `check`, `test`, `build` steps and the path-filtered `web` job.
- Playwright e2e is out of scope.
- ESLint runs with `--max-warnings 0`; `svelte-check` runs with `--fail-on-warnings`.
- Audit command is `pnpm audit --prod`.
- Prettier options come from the existing code (tabs, single quotes).

## Review Focus

- Generated/ignored paths (`.svelte-kit`, `build`, `node_modules`, `pnpm-lock.yaml`) must not be formatted or linted.
- Prettier and ESLint must not disagree on any file (no format/lint loop).
- Tailwind class order in `.svelte` files is enforced, including `cn`/`tv` helper strings if configured.
- `pnpm audit` network failure must not be reported as a pass.
- Playwright specs in `web/e2e` are formatted and linted like other TypeScript.

---

### Task 1: Formatting

**Files:**
- Create: `web/.prettierrc`, `web/.prettierignore`
- Modify: `web/package.json` (devDependencies, scripts `format`, `format:check`), `web/pnpm-lock.yaml`

**Interfaces:**
- Produces: `pnpm format` (write), `pnpm format:check` (non-zero on diff).

- [ ] **Step 1: Add dev dependencies** `prettier`, `prettier-plugin-svelte`, `prettier-plugin-tailwindcss` with `pnpm add -D` (workspace pnpm 10.13.1).
- [ ] **Step 2: Configure** `.prettierrc` (tabs, single quotes, no trailing commas as in current files, plugins `prettier-plugin-svelte` then `prettier-plugin-tailwindcss` last, `tailwindStylesheet: ./src/app.css`, `.svelte` override with `parser: svelte`); `.prettierignore` listing Review Focus paths.
- [ ] **Step 3: Verify it fails on the baseline**: `pnpm format:check` → exits non-zero only if existing files differ; record which.
- [ ] **Step 4: Run `pnpm format`**, then `pnpm format:check` → exits 0.
- [ ] **Step 5: Commit** formatting config and the reformatting separately (`ci(web): add Prettier` then `style(web): apply Prettier`).

### Task 2: Linting and Svelte warnings

**Files:**
- Create: `web/eslint.config.js`
- Modify: `web/package.json` (devDependencies, scripts `lint`, `check`)

**Interfaces:**
- Produces: `pnpm lint` = `eslint . --max-warnings 0`; `pnpm check` = existing command plus `--fail-on-warnings`.

- [ ] **Step 1: Add dev dependencies** `eslint`, `typescript-eslint`, `eslint-plugin-svelte`, `eslint-config-prettier`, `globals`, `@eslint/js`.
- [ ] **Step 2: Write `eslint.config.js`** flat config: ignores per `.prettierignore`; `js.configs.recommended`, `ts.configs.recommended`, `svelte.configs.recommended`, `prettier`, `svelte.configs.prettier` last; browser+node globals; `.svelte` files use the TypeScript parser (`parserOptions.parser`).
- [ ] **Step 3: Run `pnpm lint` and `pnpm check`** → list findings; fix real issues in code without behavior change. Any rule override needs a one-line justification comment.
- [ ] **Step 4: Prove the gates**: add a throwaway unused variable and an unlabeled `<button>`-style a11y warning; `pnpm lint` and `pnpm check` must fail; remove them.
- [ ] **Step 5: Run `pnpm format:check && pnpm lint && pnpm check` → exit 0.** Commit (`ci(web): add ESLint and fail on Svelte warnings`).

### Task 3: Audit, verify script, CI, docs

**Files:**
- Modify: `web/package.json` (scripts `audit`, `verify`), `.github/workflows/ci.yml` (`web` job), `README.md` (web commands near lines 135-137)

**Interfaces:**
- Consumes: scripts from Tasks 1-2.
- Produces: `pnpm verify` = `format:check`, `lint`, `check`, `test`, `build` in sequence.

- [ ] **Step 1: Add `audit` (`pnpm audit --prod`) and `verify`** scripts; run `pnpm audit` and record the result (any existing advisory is reported to the user, not silently ignored or threshold-hidden).
- [ ] **Step 2: Update the CI `web` job**: rename to `Web format, lint, check, test, build, audit`; steps `pnpm format:check`, `pnpm lint`, `pnpm check`, `pnpm test`, `pnpm build`, `pnpm audit`, each its own named step after `pnpm install --frozen-lockfile`.
- [ ] **Step 3: Document** `pnpm --dir web format`, `lint`, `verify`, and `audit` in the README verification list.
- [ ] **Step 4: Run `pnpm verify` and `pnpm audit` locally** → both exit 0.
- [ ] **Step 5: Commit**, push, open the PR referencing #130 and confirm the `web` job is green.
