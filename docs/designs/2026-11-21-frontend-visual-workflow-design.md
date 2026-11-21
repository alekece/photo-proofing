# Visual iteration on the frontend

Approved by the maintainer. Issue: [#430](https://github.com/alekece/photo-proofing/issues/430).

A maintainer working on one control should be able to see it, request an adjustment and judge the result without repeating a journey through the app. A layout or interaction that depends on the surrounding page needs that journey instead.

## Approaches

We considered making Storybook and a browser pass mandatory for every UI change. That would add work to small edits without showing whether a control fits its page. Using only the app has the opposite problem: it makes repeated feedback on one component unnecessarily slow. The chosen approach is to pick the view that answers the design question.

## Decision

- Keep Superpowers and the existing frontend conventions. Use UI/UX Pro Max for usability and visual direction, Design System for shared tokens and variants, and UI Styling for Svelte and Tailwind implementation details. These are aids, not three mandatory steps.
- For visual feedback on a reusable component, prefer a direct link to its Storybook story when available. Change the real component, check its affected states and inspect a representative use in the app. If the problem is how a page arranges controls, work on that page instead.
- For meaningful screen changes, inspect the affected journey in Playwright on phone and desktop. Compare with the approved reference and correct visible regressions; no blanket snapshot suite is needed.
- The current design is the baseline for ordinary changes. An explicitly requested audit can challenge it. Show the proposed direction for a new page or substantial redesign and get approval before implementing it.

This changes contributor guidance only. Storybook setup, the site-wide audit and any resulting redesign are separate work.
