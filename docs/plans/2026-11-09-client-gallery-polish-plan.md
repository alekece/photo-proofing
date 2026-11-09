# Client gallery polish: implementation plan

Design: [`docs/designs/2026-11-09-client-gallery-polish-design.md`](../designs/2026-11-09-client-gallery-polish-design.md). Issue: #295. Frontend only.

1. **Expiry.** `ExpiryBadge` renders "Open until <date>" only; drop `now`, `days`, `remaining`, and the `expiry_today` / `expiry_days_left` messages (en + fr, then `pnpm --dir web paraglide`). Update `ExpiryBadge.test.ts`.
2. **Sticky bar.** In `ClientGallery.svelte`, observe the hero `header` with an `IntersectionObserver`. The bar (title, All / Favorites switch, Download all) is hidden and inert while the hero is visible, and slides in once it is not. It is always shown when the observer is unavailable. The transition is disabled under reduced motion. `ClientPreview` uses the same component, so it follows.
3. **Tests.** `ClientGallery.test.ts` stubs the observer: hero visible gives one title, one "Download all" and no bar; hero out of view shows the bar.
4. **Verify.** `pnpm --dir web verify`, then open the PR.
