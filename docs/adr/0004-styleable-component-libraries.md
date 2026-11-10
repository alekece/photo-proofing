# ADR 0004 — Prefer well-known, styleable component libraries

Status: Accepted through the approved [review polish design](../designs/2026-11-10-review-polish-design.md), issue [#319](https://github.com/alekece/photo-proofing/issues/319). Supersedes the "no new dependency" rule of the [frontend design system design](../designs/2026-10-31-frontend-design-system-design.md).

## Context

[ADR 0001](0001-technology-baseline.md) chose Tailwind with shadcn-svelte as the frontend starting point, but the design system was then built from plain-element primitives so it would need no headless-UI dependency. That was fine for Dialog, Tabs and Checkbox. A date picker is different: the native `<input type="date">` cannot be styled to match the app, and a hand-written calendar would have to re-solve keyboard navigation, locale-aware weeks and ARIA grid semantics.

## Decision

Prefer a well-known, widely used library over reimplementing a component, as long as the result can be fully styled with the app's tokens. The default source is shadcn-svelte: its components are copied into `web/src/lib/components/ui/` in the repository's folder layout and restyled, and they run on Bits UI. The first such component is the Calendar, used by the `DatePicker` with `@internationalized/date` for calendar-day values.

## Alternatives and consequences

A hand-written calendar was rejected for the accessibility and locale work above. Keeping the native date input was rejected because its popup cannot be themed and differs per browser.

`bits-ui` and `@internationalized/date` become runtime dependencies, so `pnpm audit` and upgrades cover them. Copied shadcn-svelte source is ours to maintain. Existing hand-built primitives keep working; moving them to shadcn-svelte is tracked separately in [#320](https://github.com/alekece/photo-proofing/issues/320).
