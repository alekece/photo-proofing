# Frontend Guidance (`web/`)

These rules apply to the SvelteKit/TypeScript frontend. They add to the repository-wide policy in the root `AGENTS.md`; on frontend-specific questions this file wins.

- Follow the design decisions in `docs/superpowers/specs/2026-10-31-frontend-design-system-design.md` and the page designs under `docs/superpowers/specs/`.
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
