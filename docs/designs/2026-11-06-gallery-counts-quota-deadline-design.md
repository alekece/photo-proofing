# Gallery counts, quota and deadline

Issue: [#208](https://github.com/alekece/photo-proofing/issues/208). Part of stack #199.

## Goal

Show the numbers the redesign prototype shows: photo and selection counts on dashboard cards, totals in the footer, an "Oldest created" sort, the included-photos limit on the client page, and the deadline on the client cover.

## Decisions

- **Counts are computed, never stored.** `GET /api/galleries` returns, per gallery, `photo_count` and `favorite_count`. The list query computes them with a lateral aggregate over `photos` (`count(*)` and `count(*) FILTER (WHERE favorite)`). No new columns, no migration, nothing to keep in sync.
- **Only `ready` photos count.** That is what clients see and the only photos that can be favorited. Waiting, processing and failed photos are excluded. Galleries without photos report 0 and 0.
- **Only the list changes.** `get`, `update` and `share` still return a plain gallery; the list returns a `GalleryOverview` (gallery fields plus the two counts).
- **Sort.** `sort=oldest` orders by `created_at, id`. The menu order matches the prototype: Recently updated, Newest created, Oldest created, Title A–Z.

## Dashboard

- Card line: "N photos · N selected" (selected = favorites). The selected number is highlighted in the warning colour when it exceeds the gallery's included photos. Drafts show "N photos" (or "No photos yet"); expired, archived and delivered galleries keep their existing status wording.
- Footer: "N galleries · N photos", archived galleries excluded, as in the prototype.
- The New gallery dialog keeps its helper line removed (#203).

## Client page

- The client API already exposes `included_photos` and `expires_at`; no API change.
- The favorites pill reads "12 of 10" when the gallery has a limit, next to the existing "N extra" warning. No limit, no "of".
- The cover shows "Open until <date>" (previously "Access until").

## Out of scope

KPI greetings, notification bell and news chips (#72), studio logo and name (#67).

## Tests

Database tests for counts (zero photos, favorites, non-ready excluded) and sort; API route test; web unit and component tests for the schema, summary text, card highlight, footer totals, sort menu and client pill.
