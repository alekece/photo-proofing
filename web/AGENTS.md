# Frontend Guidance (`web/`)

These rules apply to the SvelteKit/TypeScript frontend. They add to the repository-wide policy in the root `AGENTS.md`; on frontend-specific questions this file wins.

## Visual workflow

Follow the root Superpowers workflow and keep the discussion proportional to the change. For ordinary screen work, read the [UI guide](../docs/designs/ui/DESIGN.md) first, then the screen's design and pictures. Open the [prototype](../docs/designs/ui/prototype.html) to check behaviour and motion. Follow the [frontend design system](../docs/designs/2026-10-31-frontend-design-system-design.md) for tooling and component decisions.

Choose the skill for the question, not all three in sequence: `ui-ux-pro-max` for visual direction, layout, usability and accessibility; `design-system` for shared tokens, states and variants; `ui-styling` for implementing the decision in Svelte and Tailwind. Their suggestions do not override the approved design.

- **A reusable component:** inspect its variants and consumers before editing. When the maintainer wants to tune it visually, share a direct local Storybook link early and iterate on the real component there. If Storybook or a suitable story is unavailable, show the component in the app instead. Change a shared default when its uses should change together; check the affected states and a representative use in the app.
- **A screen or journey:** reuse the shared components. If buttons crowd a toolbar on a phone, fix the toolbar rather than shrinking buttons across the site. For meaningful visual or interaction changes, use Playwright to inspect the rendered app on phone and desktop. Compare with the approved reference, exercise the affected interactions and fix visible regressions. Use synthetic data in shared previews or screenshots; no blanket snapshot suite is needed.
- **A new page, redesign or UX audit:** the existing design is a baseline, not the required outcome of an explicitly requested audit. Inspect the actual journeys and show what works or falls short. For a new page or significant redesign, show the proposed visual direction and get approval before implementing it.

## Implementation conventions

- Svelte components are named in PascalCase everywhere, including `src/lib/components/ui/`: each primitive lives in a PascalCase folder (`ui/PageShell/PageShell.svelte`) next to its `PageShell.test.ts` and `PageShellHarness.test.svelte`. Plain TypeScript modules (`menu.ts`, `toasts.svelte.ts`) stay camelCase.
- Every user-visible text, including `aria-label`, `alt` and `title`, is a Paraglide message (`m.*` from `#lib/paraglide/messages.js`) with an entry in both `messages/en.json` and `messages/fr.json`; plurals use a `*` catch-all variant. Format dates with the helpers in `#lib/gallery/format.js` and `getLocale()`, never a fixed locale. Run `pnpm --dir web paraglide` after editing messages.
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
