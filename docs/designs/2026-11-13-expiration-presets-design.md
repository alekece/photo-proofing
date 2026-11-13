# Quick relative presets for the expiration date

Issue: #343. Touches the "Open until" field in gallery settings and the restore-expired dialog (#175).

## Intent

Photographers usually pick the same expiry windows ("in two weeks", "in a month"). The native date input makes them page through months. A calendar popover with one-click presets makes the common case a single click while keeping manual picking and clearing.

## Goals

- A reusable `DatePicker`: a button showing the chosen date (or "No expiry") that opens a popover with a calendar and, optionally, preset buttons.
- Presets: In 1 week, In 2 weeks, In 1 month, In 2 months, In 6 months, In 1 year. Each is computed from today's local date, commits immediately and closes the popover, exactly like picking a day.
- The calendar and a Clear action stay in the same popover.
- Used by `GallerySettings` (Open until) and `RestoreExpiredDialog`, both with presets.
- Works at 390 px and on desktop, in English and French, with no `vitest-axe` violations.

## Non-goals

- Presets on other date fields; the picker takes an optional `presets` prop and shows none unless a caller passes it.
- Changing how expiry is stored or saved: callers keep the `YYYY-MM-DD` day string and the existing `endOfLocalDay` / save path.
- Configurable or user-defined presets.

## Constraints

- Past days stay disabled where today's input has `min` (restore dialog). The settings field keeps allowing past days as today.
- Reuse the existing `Popover` primitive (outside press, Escape, focus return) rather than a second popover implementation.
- Follow the design tokens in `docs/designs/ui/DESIGN.md`; no new visual language.

## Approaches

- **A. bits-ui `Calendar` + `@internationalized/date` inside our `Popover` (chosen).** Well-known, accessible (keyboard, ARIA, locale-aware); needs token styling and two dependencies. Matches the issue's suggested date library.
- **B. Hand-built month grid.** No dependencies, but we would own keyboard navigation, accessibility and localisation.
- **C. Native date input plus a chip row.** Smallest, but not a calendar popover with presets; rejected by the maintainer.

## Decisions

- New `web/src/lib/components/ui/DatePicker/DatePicker.svelte` with `value` (`YYYY-MM-DD` or empty, bindable), `min`, `presets` (boolean), `onchange`, plus id wiring so the existing `Label for` still works.
- Dates use `@internationalized/date` (`today(getLocalTimeZone())`, `.add({ weeks | months | years })`); the calendar locale follows the active Paraglide locale.
- Month overflow follows the library (1 month from 31 Jan is 28/29 Feb).
- New messages in `web/messages/{en,fr}.json` for the six presets, "No expiry" and "Clear".

## Left to the builder

- Exact popover layout (presets beside or above the calendar at 390 px), as long as nothing overflows.
- Tests: unit tests for `DatePicker` (preset values with fake timers, clear, min, keyboard) plus updated settings and restore tests.
- Docs: update the expiry wording in `docs/SPECIFICATIONS.md` and `docs/releases/MVP.md` if they describe the date input.
