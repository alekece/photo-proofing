# Frontend Guidance (`web/`)

These rules apply to the SvelteKit/TypeScript frontend. They add to the repository-wide policy in the root `AGENTS.md`; on frontend-specific questions this file wins.

- Build screens to `docs/designs/ui/DESIGN.md`: read it first, then the doc for your screen and its pictures, and open `docs/designs/ui/prototype.html` to check behaviour and motion. Follow `docs/designs/2026-10-31-frontend-design-system-design.md` for tooling and component decisions.
- Svelte components are named in PascalCase everywhere, including `src/lib/components/ui/`: each primitive lives in a PascalCase folder (`ui/PageShell/PageShell.svelte`) next to its `PageShell.test.ts` and `PageShellHarness.test.svelte`. Plain TypeScript modules (`menu.ts`, `toasts.svelte.ts`) stay camelCase.
- Use `pnpm`; run commands from the repository root with `pnpm --dir web <script>`.
- Match the existing tooling configuration (`prettier`, `eslint`, `svelte-check`, `vitest`); lint and type checks run with zero warnings.

Command reference:

- Format check: `pnpm --dir web format:check` (`pnpm --dir web format` to fix)
- Lint: `pnpm --dir web lint`
- Type check: `pnpm --dir web check`
- Unit tests: `pnpm --dir web test`
- Build: `pnpm --dir web build`
- Dependency audit: `pnpm --dir web audit --prod`
- All of the above except audit: `pnpm --dir web verify`
- End-to-end auth tests (need a running stack, see `README.md`): `pnpm --dir web test:auth`
