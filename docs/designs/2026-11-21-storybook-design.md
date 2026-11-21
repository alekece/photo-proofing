# Storybook for the web UI primitives — design

Status: approved by the maintainer. Issue: [#429](https://github.com/alekece/photo-proofing/issues/429).

## Intent

Working on a component today means running the whole app and navigating to a page that uses it. Copilot CLI and Codex cannot iterate on one component in isolation, so a UI tweak risks touching unrelated pages.

Add Storybook to `web/` so every shared primitive can be viewed and changed on its own, and expose Storybook's MCP server so both agents can list stories, read component docs and get preview URLs.

## Goals

- Storybook runs from `web/` with the existing Tailwind theme, fonts and Paraglide messages, so a story looks like the real app.
- Every primitive in `src/lib/components/ui/`, including composites such as `ConfirmDialog` and `WorkspaceToolbar`, has a story file next to it showing its main variants and states.
- Copilot CLI and Codex can connect to Storybook MCP using their local user configuration.
- Reusable components are covered progressively over time; this slice covers `ui/` only.

## Non-goals / anti-goals

- No visual redesign and no component behaviour change. A component that looks different in the app after this change is a bug.
- No stories for non-`ui/` components in this slice, and never added in bulk afterwards. A business component gets a story only when someone needs to iterate on it. Components bound to the API, stores or routes (the panels) are not good candidates and keep relying on their Vitest tests.
- Storybook is not a test runner here: no interaction tests, no `@storybook/addon-vitest`, no `@storybook/addon-a11y` (accessibility is already covered by `vitest-axe`).
- Storybook does not join `pnpm --dir web verify`, and CI does not build it in this slice.

## Constraints

- Frontend stack stays SvelteKit 3, Svelte 5, Vite 8, Tailwind 4, pnpm. Pin Storybook, its SvelteKit framework, and the MCP addon to matching releases as dev dependencies. The Svelte CSF addon has its own version.
- Follow `web/AGENTS.md`: PascalCase component folders, user-visible text through Paraglide, zero lint/type warnings.
- `pnpm --dir web verify` must pass unchanged: stories and `.storybook/` are linted, formatted and type-checked like other source, and `storybook-static/` is ignored by Git, ESLint and Prettier.
- The MCP server is only available while `storybook dev` runs. It is not part of a static build.

## Approaches

- **A. `@storybook/sveltekit`, stories next to components (chosen).** Reuses `vite.config.ts` (Tailwind, Paraglide, `#lib` alias) and handles `$app/*` imports. Heaviest dependency set but closest to the real app.
- **B. `@storybook/svelte-vite`, no SvelteKit.** Lighter, but needs a second Vite config and mocks for `$app/*`; the two configs would drift.
- **C. Storybook in a separate workspace package.** Isolates dependencies but adds plumbing for a repository with a single frontend.

The maintainer chose A.

## Design

### Compatibility (checked in a throwaway copy, not in the repository)

A copy of `web/` with `storybook`, `@storybook/sveltekit` and `@storybook/addon-mcp` at 10.6.8 and `@storybook/addon-svelte-csf` at 5.1.6 built successfully with Vite 8 and SvelteKit 3, using the project's Tailwind CSS. `storybook dev` then served `/mcp`, which answered MCP `initialize` and `tools/list`.

Known limitations to respect:

- `@storybook/sveltekit` does not list SvelteKit 3 in its peer dependencies (it lists Vite 5–8 and Svelte 5). It worked in the check, but it is not a declared guarantee; the first implementation step re-runs the build and the dev server in the real tree.
- Storybook renders on the client only: `$env/*/private` and `$service-worker` are unsupported, and `$app/navigation`, `$app/state` and `$app/stores` are experimental and need mocks. None of the `ui/` primitives are expected to need them.
- `bits-ui` already warns about an unmet `@sveltejs/kit@^2` peer; this is existing and unrelated.

### Layout

- `web/.storybook/main.ts`: framework `@storybook/sveltekit`, stories glob `../src/lib/components/ui/**/*.stories.svelte`, addons `@storybook/addon-svelte-csf` and `@storybook/addon-mcp`.
- `web/.storybook/preview.ts`: imports `src/app.css` and sets the Paraglide locale so `m.*` messages render.
- `*.stories.svelte` (Svelte CSF, the format the SvelteKit framework installs by default) next to each primitive, e.g. `ui/Button/Button.stories.svelte`, one story per meaningful variant or state.
- `web/package.json` scripts `storybook` (`storybook dev -p 6006 --host localhost`, so the unauthenticated `/mcp` endpoint is not exposed on the network) and `build-storybook`, neither called by `verify`.
- `README.md`: how to start Storybook and use the MCP server.

### MCP configuration

- Each developer opts in by registering `http://localhost:6006/mcp` in the local user configuration for Copilot CLI or Codex. The README documents both setups and explains that Storybook must be running to connect.
- Do not add a Storybook entry to `.github/mcp.json`, create a repository `.codex/config.toml`, or commit any personal MCP settings. The Storybook project configuration and stories are still versioned.
- Verify that both clients can list MCP tools using local settings, without modifying the repository's MCP configuration.

### Conventions for later stories

- A story uses the real component and real Paraglide messages, no copies of markup or styles.
- Business components get a story when needed, following the same layout (`Component.stories.svelte` beside the component).

## Left to the builder

- Exact Storybook version pin and `pnpm-workspace.yaml` adjustments (`onlyBuiltDependencies`, audit config) if the install needs them.
- Which locale and how to apply it in `preview.ts`, and decorators for primitives that need a wrapper (for example `Toast`, dialogs, popovers).
- Story-only text: `web/AGENTS.md` requires Paraglide messages for user-visible text, so default to messages and raise it only if that makes stories unreasonable.
- Lint, Prettier and `svelte-check` settings so `verify` stays green with zero warnings. Narrow overrides limited to `*.stories.svelte` and `.storybook/` are allowed if the Storybook types need them; no rule is weakened for other files.
- The README wording.
- If `@storybook/sveltekit` fails with SvelteKit 3 in the real tree, stop and report to the maintainer; do not switch to approach B on your own.

## Implementation decisions

- Svelte CSF stories rather than CSF in TypeScript, since the SvelteKit framework installs it by default and it lets stories pass snippets as children.
- Port 6006, the Storybook default.
- No CI job and no `addon-vitest`/`addon-a11y` in this slice.
- Dependabot needs no change: its existing `npm-dev-minor-patch` group already bumps the Storybook dev dependencies together.
