# Quick relative presets for the expiration date

Issue: [#343](https://github.com/alekece/photo-proofing/issues/343). Extends `DatePicker` (#319) for the "Open until" field in `GallerySettingsPanel` (#342) and `RestoreExpiredDialog` (#175).

## Intent

Photographers usually pick the same expiry windows ("in two weeks", "in a month"). Today `DatePicker` opens only the calendar grid, so they still page through months. Adding one-click presets inside the popover makes the common case a single click while keeping manual picking and clearing.

## Goals

- Extend `DatePicker` with an optional `presets` prop so shoot-date pickers (`NewGalleryDialog`, `GallerySettingsPanel` shoot date) stay clean while expiration pickers (`GallerySettingsPanel` Open until, `RestoreExpiredDialog`) show presets.
- Presets: In 1 week, In 2 weeks, In 1 month, In 2 months, In 6 months, In 1 year. Each is computed from today's local calendar date (`today(getLocalTimeZone())`), commits immediately through `onchange` and closes the popover, just like picking a calendar day.
- Keep manual day selection on the existing `Calendar` and clearing (`clearable`) inside the same popover.
- Fits cleanly on desktop and mobile (`390 px`) in both English and French, with zero `vitest-axe` violations.

## Non-goals

- Presets on shoot-date fields (`presets` defaults to `false`).
- Changing how expiry is stored or saved: callers keep receiving `YYYY-MM-DD | null` and using the existing `endOfLocalDay` / save path.
- Configurable or custom presets.

## Constraints

- Build on the existing `DatePicker`, `Calendar` (shadcn-svelte on Bits UI, [ADR 0004](../adr/0004-styleable-component-libraries.md)) and `Popover` primitives, styled with the tokens in `docs/designs/ui/DESIGN.md`.
- Every preset label is a Paraglide message in `web/messages/{en,fr}.json`.
- `RestoreExpiredDialog` keeps `min={today}` (and `presets`), while `GallerySettingsPanel` keeps `clearable` (and adds `presets`).

## Approaches

- **A. Preset list inside the existing `DatePicker` popover (chosen).** Reuses `DatePicker`, `Calendar` and `@internationalized/date` already in the repo; zero new dependencies; shoot-date callers stay untouched.
- **B. Separate `ExpirationDatePicker` wrapper.** Duplicates popover state (`open = false` on pick) or leaks internal close hooks out of `DatePicker`.

## Decisions

- Add `presets?: boolean` (default `false`) to `DatePicker.svelte`.
- Compute presets on click via `today(getLocalTimeZone()).add({ weeks | months | years })` from `@internationalized/date`; month overflow follows the library (`31 Jan + 1 month` -> `28/29 Feb`), and picking today's identical value is a no-op close just like `pick()`.
- Add the six localized preset messages in `web/messages/{en,fr}.json`.
- Enable `presets` on `GallerySettingsPanel` (`id="expiry"`) and `RestoreExpiredDialog` (`id="{id}-day"`).

## Left to the builder

- Responsive layout inside `DatePicker` (e.g. compact chip grid above or beside the calendar so `390 px` viewports never overflow horizontally).
- Unit tests in `DatePicker.test.ts` (fake timers covering all 6 presets including month-end rollover, immediate close, no presets when omitted, `vitest-axe` in EN/FR) plus call-site assertions in `GallerySettingsPanel` and `RestoreExpiredDialog` tests.
- Docs check (`docs/SPECIFICATIONS.md` / `docs/releases/MVP.md`) if expiration picker UI is mentioned.
