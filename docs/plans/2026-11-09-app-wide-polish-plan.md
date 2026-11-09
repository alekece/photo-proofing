# App-wide polish plan

Design: `docs/designs/2026-11-09-app-wide-polish-design.md`. Issue: #291.

1. Footer: drop the copy button, `copy()`, manual-copy input, status line, `copyable` prop and the four unused messages; update `SiteFooter.test.ts` and the layout call.
2. French: replace "finale(s)" with "retouchée(s)" in `fr.json` and the `format.test.ts` expectation; add a test that no French message contains "finale".
3. Back link: add `header_back_to_galleries`; drop `crumb` and the unused `back` prop from `AppHeader`; render the link on Settings (above the title) and in the gallery hero; update tests.
4. Run `pnpm --dir web verify`, open the PR.
