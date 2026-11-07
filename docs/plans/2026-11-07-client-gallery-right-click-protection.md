# Client Gallery Right-Click Protection Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stop the browser image context menu from opening on photos in public client galleries, as a casual deterrent.

**Architecture:** Add one context-menu handler to the public `/g/{token}` route. It cancels the event only when its target is an image, so it covers the cover, thumbnails, and viewer without affecting the photographer's preview or normal context menus elsewhere.

**Tech Stack:** SvelteKit, TypeScript, Vitest, Testing Library.

**Spec:** `docs/designs/2026-11-07-client-gallery-right-click-protection-design.md`

## Global Constraints

- Right-click blocking is a deterrent, not a security guarantee.
- Preserve existing explicit photo downloads and download permissions.
- Do not add or remove watermark behavior; enabled watermarks remain embedded in generated proof derivatives.
- Add no dependencies.

## Review Focus

- Client gallery image targets in the cover and grid must suppress the context menu; test both.
- The full-screen viewer photo must also suppress it; test the image after opening the viewer.
- Non-image targets must keep the browser's context menu; test a heading.
- Delivered photo download links must remain available and unchanged; retain the existing download regression test.
- Photographer previews must remain unaffected; the handler belongs only to the public gallery route.

---

### Task 1: Suppress context menus on public gallery photos

**Files:**
- Modify: `web/src/routes/g/[token]/+page.svelte`
- Test: `web/src/routes/g/[token]/page.test.ts`
- Modify: `docs/SPECIFICATIONS.md` (FR-06)
- Modify: `docs/designs/ui/screens/client.md`

**Interfaces:**
- Consumes: the route's existing `data` and `params`; no new component props.
- Produces: a route-local `oncontextmenu(event: MouseEvent): void` handler that prevents the default only when `event.target instanceof HTMLImageElement`.

- [ ] **Step 1: Write failing route tests**

Add a small proof fixture and a helper that dispatches a bubbling, cancelable `contextmenu` event and returns whether it was prevented. Add tests asserting that:
- the cover and grid photo events are prevented;
- the viewer photo event is prevented after opening the viewer;
- a heading's context-menu event is not prevented.

- [ ] **Step 2: Run the focused tests and verify they fail**

Run: `pnpm --dir web test -- 'src/routes/g/[token]/page.test.ts'`
Expected: the new image-target assertions fail because the route does not yet prevent the context menu.

- [ ] **Step 3: Add the route-scoped handler**

In `web/src/routes/g/[token]/+page.svelte`, define the handler above the markup and attach it with `<svelte:window oncontextmenu={oncontextmenu} />`. Keep the target check limited to `HTMLImageElement`.

- [ ] **Step 4: Update product and screen documentation**

In FR-06, document image context-menu suppression as a client-gallery deterrent and retain the warning that it is not security. In the client screen doc, note that right-clicking gallery photos does not open the browser context menu. Do not change the watermark or download descriptions.

- [ ] **Step 5: Run frontend verification**

Run: `pnpm --dir web verify`
Expected: format, lint, type check, all unit tests, and build pass.

- [ ] **Step 6: Commit the task**

```bash
git add web/src/routes/g/[token]/+page.svelte web/src/routes/g/[token]/page.test.ts docs/SPECIFICATIONS.md docs/designs/ui/screens/client.md
git commit -m "fix: deter right-click saving in client galleries"
```
