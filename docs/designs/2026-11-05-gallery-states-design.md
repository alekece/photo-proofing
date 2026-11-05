# Gallery states and dashboard filter

Issue: #86.

## Intent

The photographer sees at a glance where each gallery stands and can narrow the dashboard to the
galleries they care about.

## Goals

- Each gallery in the list carries its states, derived on the server from the gallery's data.
- The list can be filtered by state, together with the existing sort.
- Dashboard cards show the states; a filter control lists them all.
- A no-results screen, distinct from the empty dashboard, appears when search or filter matches
  nothing.

## Non-goals

- Setting `delivered_at` or `archived_at`: #71 (deliver) and #73 (archive) write them.
- Card counts and activity chips (#72), the hero thumbnail (#85).
- Pagination, or moving the title search to the server.

## States

A gallery has exactly one **progress** state and any number of **flags**:

| Kind | State | Rule |
|---|---|---|
| Progress | `delivered` | `delivered_at` is set |
| Progress | `shared` | else `shared_at` is set |
| Progress | `draft` | otherwise |
| Flag | `archived` | `archived_at` is set |
| Flag | `expired` | `expires_at` is in the past |

Flags are independent of progress, so a gallery can be a draft, archived and expired at once.
This replaces the issue's "archived wins" rule. `expired` is evaluated at request time and is
never stored.

## Data

Migration adding three nullable timestamps to `galleries`: `shared_at`, `delivered_at`,
`archived_at`. Existing galleries with a `share_token` get `shared_at = updated_at`, an
approximation. `share()` sets `shared_at` when it creates the link.

Nothing sets `delivered_at` or `archived_at` yet; their tests write them directly. Later issues
only have to set a column, so the list contract does not change.

## API

`GET /api/v1/galleries?sort=…&state=…`:

- `state` can repeat and combines with **plain OR**: a gallery matches if it has any chosen
  state, progress or flag. Without `state`, all galleries are returned.
- An unknown value is rejected with 400, like an unknown `sort`.
- The filter and sort run in the same query, scoped to the session's workspace.
- Each gallery gains `states`: its progress state first, then its flags, for example
  `["draft", "archived", "expired"]`.

## Dashboard

- The filter is a multi-select listing all five states, Archived and Expired included. It lives
  in the URL beside the sort (`?sort=title&state=shared&state=expired`), so it survives a reload.
  Unknown values in the URL are dropped, as the sort already falls back to its default.
- Cards show the progress state as a chip and the flags as badges.
- Title search stays in the browser and applies on top of the filter.
- No-results: shown when search or filter matches nothing, with a way to clear them. The
  empty-dashboard state is unchanged and only appears when the workspace has no galleries.

## Approaches considered

- **A. Derive on the server per request (chosen).** One place for the rule; filter, sort and
  workspace scoping stay consistent; later issues add one branch each.
- B. Filter only in the browser: simpler, but the issue asks for server filtering and rejected
  unknown values, and it breaks with long lists.
- C. Store a state column updated on transitions: trivial queries, but it can drift from the real
  data and needs a backfill.

## Testing

- Derivation: every progress state, each flag, and combinations, with `delivered_at` and
  `archived_at` set directly.
- Filter: a single value, several values (OR), unknown values (400), filter with each sort, and
  workspace isolation.
- Migration: existing shared galleries get `shared_at`; drafts stay null.
- Web: schema accepts and rejects `states`; the filter round-trips through the URL; cards show
  chips and badges; the no-results and empty states are distinct.

## Left to the builder

SQL shape, the exact list of columns, the filter control component, and chip and badge styling,
following the dashboard mockup.
