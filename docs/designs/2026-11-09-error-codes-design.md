# Translatable API errors

Status: approved by @alekece on 2026-11-09. Issue: #285.

## Intent

The web shows every screen in the workspace language, but a few errors still reach the user as
English text from the API: the reason an export ZIP failed, and some `409`/`503` answers. The API
should say *what* went wrong with a stable code, and the web should decide *how* to say it.

## Goals

- Every error the API returns carries a stable, documented `code`.
- An export failure is stored and returned as a code, never as a sentence. Existing rows stay
  readable.
- The web translates each code in `en.json` and `fr.json`; an unknown or missing code falls back to
  today's status sentence.
- In French, no user-visible error text stays in English.

## Non-goals

- Translating logs: they keep the detailed English text.
- Server-side translation or an `Accept-Language` negotiation.
- Codes for non-error answers.

## Anti-goals

- Codes that tell apart answers that must look the same (see Decisions).
- Domain crates (database, worker, storage) learning about the HTTP vocabulary.
- A code renamed by accident, silently breaking the web mapping.

## Approaches

1. **One `ErrorCode` enum owned by the API crate (chosen by @alekece).** Every conversion into
   `ErrorResponse` picks the code. The domain layers keep their own typed errors. One reviewable
   list, compiler-checked, testable.
2. **Free strings at each call site.** Least code, but typos go unnoticed and there is no single
   list to document or test.
3. **A shared errors crate used by API, database and worker.** Lets the worker emit codes directly,
   but pulls display concerns into the domain layers.

Code values are strings, not numbers (chosen by @alekece): readable in `curl`, logs and tests,
no number registry to keep, and no confusion with HTTP statuses. The web maps codes through an
explicit table, never by building a message key from the code.

## Decisions

### Response body

```json
{ "code": "zip_not_ready", "context": ["the ZIP is not ready"] }
```

- `code` is always present: a snake_case value from the list below. The HTTP status and the
  `Retry-After` header are unchanged.
- `context` holds the English detail meant for people reading the response; it is never shown to the
  user. In release builds it is omitted, so production answers carry only the code. *My call.*
- The old `error` field is removed. The web never reads it, and `context[0]` stops carrying the
  refused field name, because per-field codes replace it. *My call: with `code` as a new key,
  keeping the English `error` too would duplicate `context`.*

### Code list

Frozen: a unit test asserts the serialized list, so renaming a variant fails CI.

| Code | Status | Meaning |
|---|---|---|
| `unauthorized` | 401 | Session expired or sign-in required |
| `invalid_login` | 401 | Wrong email or password |
| `forbidden_origin` | 403 | Request origin not allowed |
| `not_found` | 404 | Anything missing or not visible (see below) |
| `setup_closed` | 409 | Initial setup already done |
| `gallery_delivered` | 409 | Gallery already delivered |
| `gallery_not_shared` | 409 | Gallery is not shared |
| `selection_empty` | 409 | Nothing selected |
| `zip_not_ready` | 409 | The ZIP is not ready |
| `zip_in_progress` | 409 | A ZIP is already being prepared |
| `request_timeout` | 408 | Request body timed out |
| `payload_too_large` | 413 | Request body too large |
| `selection_too_large` | 413 | Selection too large for one ZIP |
| `unsupported_media_type` | 415 | Wrong content type |
| `bad_request` | 400 | Malformed request |
| `invalid_image` | 422 | Not a valid JPEG |
| `dimensions_too_large` | 422 | Image dimensions over the limit |
| `invalid_logo` | 422 | Logo image invalid |
| `logo_too_large` | 422 | Logo too large |
| `logo_required` | 422 | A logo is required |
| `invalid_title` | 422 | Gallery title refused |
| `invalid_included_photos` | 422 | Included photo count refused |
| `invalid_studio_name` | 422 | Studio name refused |
| `invalid_position` | 422 | Watermark position refused |
| `invalid_size` | 422 | Watermark size refused |
| `too_many_attempts` | 429 | Too many sign-in attempts |
| `busy` | 429 | Server busy, retry later |
| `storage_full` | 507 | Photo storage is full |
| `unavailable` | 503 | Temporary server problem |
| `photo_removed` | n/a | Export failure: a selected photo was deleted |
| `export_failed` | n/a | Export failure: gave up after several attempts |

The builder confirms this list against the code (every `ErrorResponse` constructor and conversion)
and adds a code for any case found missing; the table above is the contract once merged.

- **Privacy.** All "missing or not visible" cases (unknown gallery, wrong private link, other
  workspace) keep the single `not_found` code, so codes never tell apart what the status already
  hides.
- **One code, several sources.** `unavailable` covers every internal failure; the cause stays in
  logs. `storage_full` is also the export failure for a full disk.

### Where codes live

- `ErrorCode` is a `#[serde(rename_all = "snake_case")]` enum in `crates/api/src/error.rs`.
  `ErrorResponse` carries one; the existing constructors and every `From<…Error>` impl choose it.
- Gallery and settings errors map to the per-field codes, replacing the `context[0]` field name.

### Export failure

- The database stores a typed `ExportFailure` (`attempts_exhausted`, `photo_removed`, `disk_full`)
  instead of a string, serialized in `ExportStatus` as `{"kind":"failed","reason":"photo_removed"}`.
  It holds no HTTP vocabulary.
- A migration rewrites the legacy rows: the three known English sentences become the matching
  variant, and any other text becomes `attempts_exhausted`. *My call.* The migration has a
  regression test in the style of the other migration tests.
- The API maps `ExportFailure` to an `ErrorCode` and returns it as `code` in the failed status
  (`{"kind":"failed","code":"photo_removed"}`). The `reason` text is no longer returned.

### Web

- `errors.ts` takes the code first: an explicit `code → m.*` table, then the existing status and
  field lookup, then the generic sentence. Existing field messages move under their per-field
  codes.
- The valibot schemas accept an optional `code` string (unknown codes allowed) so an older or newer
  API never breaks parsing.
- Favorites shows the translated message for a failed export instead of `reason`.
- Every new message exists in `en.json` and `fr.json`; the existing key-parity test covers it.

### Documentation

- The README limitation noted in #284 is removed.
- The code list is documented in `ErrorCode`'s doc comment, which the frozen-list test mirrors.
  No PRD change: translated errors are already covered by the English/French requirement.

## Constraints

- Logs keep the detailed English text; codes are for display only.
- The body change is breaking for the old `error` field, but nothing reads it: the web is the only
  client and ships with the API.
- Secrets never appear in `context`; its strings stay the fixed ones used today.

## Left to the builder

- Exact English and French wording of the new messages.
- The final code list, within the rules above, and the exact migration number.
- Test layout: frozen list, release-build omission of `context`, one test per conversion, legacy
  row migration, and web fallback for an unknown code.
