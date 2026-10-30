# Gallery creation and configuration

Issue: #64. Depends on the account sessions slice (#63, merged). Product
requirements: `docs/SPECIFICATIONS.md` FR-02 (workspace ownership) and FR-03
(gallery configuration).

## Intent

A signed-in photographer can create a gallery in one click, open it, and
configure it with settings that save automatically. This is the first
persisted gallery slice; photo upload (#66) and the client flow build on it.

## Goals

- Create, list, reopen and edit galleries owned by the photographer's
  workspace; other workspaces' galleries are never visible or editable.
- Settings: title, default layout (uniform grid or masonry), optional expiry,
  optional included-photo count, watermark enabled flag.
- An unset included-photo count means unlimited selection. Editing or clearing
  it never deletes favorites (none exist yet; the contract is documented for
  later slices).
- "New gallery" immediately creates a gallery titled "Untitled gallery" and
  opens it; the title is edited inline.
- Settings autosave per field with visible saving, saved and error states.
- Dashboard lists galleries as cards with sort options.
- A reusable placeholder component for empty and loading areas: media, tables
  and any lazily loaded component.
- Responsive, keyboard-operable UI with empty, loading and error states, using
  SvelteKit, Tailwind and shadcn-svelte components.

## Non-goals and anti-goals

- No photo upload, hero picker or photo count (#66), no archive or delete, no
  branding editors, no photo reordering, no gallery lifecycle statuses.
- Photographers do not choose the placeholder style; it is a developer choice.
- No watermark rendering or configuration beyond the enabled flag.
- Must not add the superseded `selection_received` and `retouching` states.

## Constraints

- Follow `AGENTS.md`: typed APIs (no unit-suffixed primitives), `Repository`
  trait pattern in the database crate, ownership via `workspace_id`.
- Validation and ownership are enforced on the server; the UI is not trusted.
- Empty galleries and galleries without expiry must be valid.

## Approaches

- **A. Server-authoritative, per-field PATCH changeset (chosen):**
  `POST /galleries`, `GET /galleries?sort=`, `GET` and `PATCH
  /galleries/{id}`. PATCH carries only changed fields; the server validates
  and returns the full gallery. Fits autosave and avoids clobbering between
  concurrent saves.
- **B. Whole-form PUT:** simpler server, but concurrent autosaves overwrite
  each other and per-field save feedback is awkward.

A was chosen by the product owner for autosave fit.

## Design

### Data model

New migration creating `galleries`:

| Column | Type | Notes |
| --- | --- | --- |
| `id` | uuid | primary key; `GalleryId` newtype |
| `workspace_id` | uuid | FK to workspaces, ownership; indexed |
| `title` | text | trimmed; CHECK `char_length(title)` between 1 and 255 |
| `layout` | enum | `grid` (default) or `masonry` |
| `expires_at` | timestamptz null | null means no expiry |
| `included_photos` | integer null | null means unlimited; when set, at least 1 (CHECK) |
| `watermark_enabled` | boolean | default false |
| `created_at`, `updated_at` | timestamptz | `updated_at` bumped on each change |

The hero is not stored in this slice: there are no photos to reference. #66
adds a nullable hero reference, validated against the gallery's own photos.
This defers the "hero validated server-side" criterion to #66 (author's call).

### Database crate

A `GalleryRepository` implementing the existing `Repository` trait. Domain
types are kept thin (product owner's call): `Gallery`, `Layout`, and a
`GalleryChanges` changeset. The title is a plain `String`, trimmed by the
caller, and the database CHECK is the authority on length; the repository maps
the violation to a validation error. `included_photos` is a
`NonZeroUsize`, converted to the integer column at the sqlx boundary (sqlx has
no Postgres encoding for it). `GalleryChanges` is a struct of optional fields
where optional settings use `Option<Option<T>>`: the outer `None` keeps the
value, `Some(None)` clears it. The API deserializes it with `serde_with`'s
double-option support, so a JSON `null` becomes `Some(None)` and a missing key
`None`; `serde_with` is added to `[workspace.dependencies]`. Every query is
scoped by `workspace_id`; a gallery of another workspace behaves as not found.
Time values cross the crate boundary as `SystemTime`, as in the auth
repository.

### API

All routes require a session (a new `Identity` extractor) and use the
session's `workspace_id`.

- `POST /galleries`: no body; creates "Untitled gallery" with defaults,
  returns 201 and the gallery.
- `GET /galleries?sort=`: `sort` is `updated` (default), `created` or `title`;
  unknown values return 400. Updated and created sort newest first; title sorts
  ascending, case-insensitive.
- `GET /galleries/{id}`: the gallery, or 404.
- `PATCH /galleries/{id}`: JSON object with any subset of `title`, `layout`,
  `expires_at`, `included_photos`, `watermark_enabled`. A missing key
  keeps the value; JSON `null` clears the optional settings. Validation
  failures return 422 with the offending field; the response is the updated
  gallery. An empty changeset is a no-op and does not bump `updated_at`.
- `expires_at` accepts RFC 3339; a past expiry is allowed (the client flow
  decides how an expired gallery behaves later).

Errors share one generic response error type implementing `ResponseError`
(status, `{field?, message}` body, no internal details leaked). `AuthError`
converts into it with `From`, and the gallery routes add their own variants
(422 validation with the offending field, 404, 400 for malformed JSON).

### Web

- Dashboard shows the real list: cards with the placeholder
  hero, title, layout and last-edited time; a sort control; "New gallery"
  button; empty state for first-time users; loading and error states.
- Gallery page: cover placeholder, inline-editable title, settings side panel
  (layout, expiry, included photos, watermark, hero note "set once photos are
  added") and an empty photo area. On narrow screens the panel stacks below.
- Autosave: toggles and pickers save immediately; title and numeric fields
  debounce. Each save shows saving, saved, or "couldn't save, retry" (see
  Author's calls). An emptied title reverts on blur.
- Page reads use SvelteKit server `fetch` with the browser cookie; autosave
  writes go from the browser through the proxy (see Author's calls).

### Placeholder component

One generic component (for media, tables and any lazily loaded content) with a developer-chosen `background` prop (`sheet`: ghost contact
sheet, or `gradient`: soft gradient). An optional icon and label are off by
default; the empty hero enables them ("Your hero photo will appear here").
For loading it uses `sheet` with a slow lighter-to-darker grey pulse and no
label, disabled under `prefers-reduced-motion`.

### Testing

- Database: persistence round-trip, defaults, ownership isolation, changeset
  Keep/Set/Clear semantics, title and count validation, sort orders,
  `updated_at` bump.
- API: auth required, create, list sorts, patch (including null clearing and
  422 cases), cross-workspace 404.
- Web: component and flow tests following the existing web test setup, covering
  empty, loading, error and autosave states, and keyboard operation.

## Author's calls

These were settled by the author, not discussed, and can be overridden:

- **Expiry input:** a date-only picker (decided by the product owner). The UI
  sends the end of the chosen day (23:59:59) in the browser's time zone as RFC
  3339; the stored value is an exact instant, so a time-of-day picker can be
  added later without a migration. A past date shows a warning but is allowed.
- **Concurrency:** last write wins per field. No version or `If-Match`; a
  single photographer rarely edits one gallery from two places in the MVP.
- **Validation:** PATCH rejects `null` for `title`, `layout` and
  `watermark_enabled`, unknown keys and wrong types with 422. The 422 body is
  `{ "field": "<name>", "message": "<reason>" }`.
- **Failed save:** the field keeps the user's attempted value with an inline
  "couldn't save, retry" action; "revert" happens only when the user leaves the
  field or discards it. (This replaces the earlier "returns to the last saved
  value" wording.) An emptied title still reverts on blur.
- **Title length:** counted in Unicode characters after trimming, enforced in
  the domain type and by a database CHECK.
- **Pending autosave:** debounced changes flush on blur and on navigation away.
- **List:** `updated_at` is shown as "last edited"; sorts tiebreak on `id`; no
  pagination in the MVP.
- **Double click:** "New gallery" is disabled while its request is in flight.
- **Mocks:** the dashboard is an empty shell today. The gallery mocks and
  superseded `selection_received` and `retouching` states belong to the client
  route `/g/[gallery]`, which is out of scope and left untouched; they are
  removed in the client-flow slice.
- **Browser to API:** autosave writes go from the browser to `/api/v1/...`
  through the existing proxy, like `authRequest`; reads for pages use SvelteKit
  server `fetch`. The `Origin` check (`require_same_origin`) is generalised and
  applied to the gallery routes as well as `/auth`.
- **Identity:** a new extractor resolves the session to `Identity`
  (`account_id`, `workspace_id`) so handlers do not call `authenticate`
  themselves. Gallery routes share one generic response error with
  `AuthError` (see the API section), not a separate gallery-only type.
- **Session activity:** a successful save counts as user activity and reuses
  the existing browser-driven renewal; a 401 on save redirects to sign-in.
- **Layout reference:** card and page layouts are those of the brainstorming
  mockups, described in this document; builders may adapt visual details.

## Left to the builder

Exact module and file layout, SQL, component internals, debounce timing,
visual details within the mockups, and the Rust and TypeScript test mechanics.
