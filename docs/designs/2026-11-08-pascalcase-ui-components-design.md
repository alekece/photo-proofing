# PascalCase names for `ui/` components

Issue: [#232](https://github.com/alekece/photo-proofing/issues/232).

## Problem

Feature components in `web/src/lib/components/` use PascalCase (`ProofViewer.svelte`), while the primitives in `ui/` used kebab-case (`page-shell/page-shell.svelte`). Nothing explained why.

## Approaches

1. Keep kebab-case in `ui/` and document it. Two conventions remain.
2. PascalCase files, kebab-case folders. Folder and file would differ for the same component.
3. PascalCase files and folders, tests and harnesses named after the component. One convention, matches the top-level `PhotosPanel.test.ts` / `PhotosPanelHarness.test.svelte` pattern.

## Decision

Approach 3. `ui/page-shell/page-shell.svelte` becomes `ui/PageShell/PageShell.svelte`; `page-shell.test.ts` becomes `PageShell.test.ts`; `x-harness.test.svelte` becomes `XHarness.test.svelte`. `toaster.svelte` becomes `Toast/Toaster.svelte`. Non-component modules (`menu.ts`, `toasts.svelte.ts`) keep camelCase names. Imports are updated; behaviour is unchanged. The convention is recorded in `web/AGENTS.md`.
