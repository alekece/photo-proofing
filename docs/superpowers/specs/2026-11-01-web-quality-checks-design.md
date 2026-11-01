# Web formatting, linting, and dependency checks in CI

Status: Awaiting approval of this written design (approach A selected in conversation on 2026-11-01).
Issue: [#130](https://github.com/alekece/photo-proofing/issues/130).

## Intent

Make the `web` CI job enforce formatting, lint, Svelte warnings, and dependency advisories, the way `cargo fmt`, `clippy`, and `cargo deny` gate the Rust side, so style and dependency problems cannot reach `main`.

## Goals and non-goals

- Goals: Prettier formatting (including Svelte and Tailwind class order), ESLint for TypeScript and Svelte, `svelte-check` failing on warnings, `pnpm audit --prod`, local scripts and README commands, green `main`.
- Non-goals: Playwright e2e in CI (separate issue once the upload journey in #66 works end to end), behavior changes, visual redesign, Biome.
- Anti-goal: a rule set so strict or noisy that it forces unrelated refactors or is disabled piecemeal.

## Current state

The `web` job runs `pnpm check`, `pnpm test`, and `pnpm build` only. `web/package.json` has no formatter, linter, or audit step. Tooling: SvelteKit 3, Svelte 5, TypeScript 6, Tailwind 4 via `@tailwindcss/vite`, vitest, pnpm 10.13.1.

## Approaches

1. **Prettier + ESLint with a single `verify` script (selected).** Established SvelteKit combination; each tool has one job; Tailwind class order comes from the official Prettier plugin.
2. Biome only. One fast tool, but partial Svelte support and no official Tailwind class sorting for Svelte.
3. Separate CI steps without a shared script. Works, but local and CI runs can drift.

## Design

- Dev dependencies: `prettier`, `prettier-plugin-svelte`, `prettier-plugin-tailwindcss`, `eslint`, `typescript-eslint`, `eslint-plugin-svelte`, `eslint-config-prettier`, `globals`.
- Config: `web/.prettierrc` matching the existing style (tabs, single quotes, as in current files), `web/.prettierignore` (build output, `.svelte-kit`, lockfile), and a flat `web/eslint.config.js` using recommended presets for typescript-eslint and eslint-plugin-svelte, with `eslint-config-prettier` last.
- Scripts in `web/package.json`: `format` (write), `format:check`, `lint` (`eslint . --max-warnings 0`), `audit` (`pnpm audit --prod`), and `verify` running `format:check`, `lint`, `check`, `test`, and `build`. `check` gains `--fail-on-warnings`.
- CI: split the `web` job into one job per check, mirroring the Rust jobs (`Web format`, `Web lint and check`, `Web dependencies`, `Web tests`, `Web build`), all path-filtered and sharing a `.github/actions/setup-web` composite action for pnpm, Node, and the frozen install. Changed after review of PR #131 at the user's request.
- README: document `pnpm format`, `pnpm lint`, and `pnpm verify` beside the existing web verification commands.
- Existing code is reformatted and lint-fixed in a dedicated commit, with no behavior changes.

## Left to the builder

Exact Prettier options (taken from the existing code), any narrowly justified rule overrides, whether audit failures use a severity threshold (`--audit-level`), and the dependency version pins.

## Verification

`pnpm verify` passes locally; CI is green on the PR; introducing a misformatted file, a lint violation, and a Svelte warning each fails the corresponding step.
